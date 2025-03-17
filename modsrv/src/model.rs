use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
use crate::comsrv_handler::{ComsrvHandler, CommandStatus};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMapping {
    pub source_key: String,
    pub source_field: String,
    pub target_field: String,
    pub transform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_mappings: Vec<DataMapping>,
    pub output_key: String,
    pub enabled: bool,
}

/// 控制动作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlActionType {
    /// 遥控动作 (开关量)
    RemoteControl,
    /// 遥调动作 (模拟量)
    RemoteAdjust,
}

/// 控制动作条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlActionCondition {
    /// 条件字段
    pub field: String,
    /// 比较运算符 (>, <, >=, <=, ==, !=)
    pub operator: String,
    /// 比较值
    pub value: String,
}

/// 控制动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAction {
    /// 动作ID
    pub id: String,
    /// 动作名称
    pub name: String,
    /// 动作类型
    pub action_type: ControlActionType,
    /// 通道名称
    pub channel: String,
    /// 点位名称
    pub point: String,
    /// 动作值
    pub value: String,
    /// 触发条件
    pub conditions: Vec<ControlActionCondition>,
    /// 是否启用
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWithActions {
    /// 基本模型定义
    pub model: ModelDefinition,
    /// 控制动作列表
    pub actions: Vec<ControlAction>,
}

pub struct ModelEngine {
    models: HashMap<String, ModelDefinition>,
    actions: HashMap<String, Vec<ControlAction>>,
    comsrv_handler: ComsrvHandler,
}

impl ModelEngine {
    pub fn new() -> Self {
        ModelEngine {
            models: HashMap::new(),
            actions: HashMap::new(),
            comsrv_handler: ComsrvHandler::new("ems:"),
        }
    }

    pub fn load_models(&mut self, redis: &mut RedisConnection, pattern: &str) -> Result<()> {
        // Clear existing models
        self.models.clear();
        self.actions.clear();

        // Get all model configuration keys
        let model_keys = redis.get_keys(pattern)?;
        info!("Found {} model configurations", model_keys.len());

        for key in model_keys {
            match self.load_model_from_redis(redis, &key) {
                Ok((model, actions)) => {
                    info!("Loaded model: {} ({})", model.name, model.id);
                    self.models.insert(model.id.clone(), model);
                    
                    if !actions.is_empty() {
                        info!("Loaded {} control actions for model {}", actions.len(), key);
                        self.actions.insert(key.clone(), actions);
                    }
                }
                Err(e) => {
                    error!("Failed to load model from key {}: {}", key, e);
                }
            }
        }

        info!("Successfully loaded {} models", self.models.len());
        Ok(())
    }

    fn load_model_from_redis(&self, redis: &mut RedisConnection, key: &str) -> Result<(ModelDefinition, Vec<ControlAction>)> {
        let model_json = redis.get_string(key)?;
        
        // 尝试解析为带控制动作的模型
        let model_with_actions: Result<ModelWithActions, _> = serde_json::from_str(&model_json);
        
        if let Ok(model_with_actions) = model_with_actions {
            return Ok((model_with_actions.model, model_with_actions.actions));
        }
        
        // 如果解析失败，尝试解析为基本模型
        let model: ModelDefinition = serde_json::from_str(&model_json)
            .map_err(|e| ModelSrvError::ModelError(format!("Failed to parse model JSON: {}", e)))?;
        
        Ok((model, Vec::new()))
    }

    pub fn execute_models(&self, redis: &mut RedisConnection) -> Result<()> {
        for (id, model) in &self.models {
            if !model.enabled {
                debug!("Skipping disabled model: {}", id);
                continue;
            }

            match self.execute_model(redis, model) {
                Ok(_) => {
                    debug!("Successfully executed model: {}", id);
                    
                    // 执行模型后检查是否需要触发控制动作
                    if let Some(actions) = self.actions.get(id) {
                        self.check_and_execute_actions(redis, model, actions)?;
                    }
                }
                Err(e) => {
                    error!("Failed to execute model {}: {}", id, e);
                }
            }
        }

        Ok(())
    }

    fn execute_model(&self, redis: &mut RedisConnection, model: &ModelDefinition) -> Result<()> {
        // Collect input data based on mappings
        let mut model_inputs: HashMap<String, String> = HashMap::new();
        
        for mapping in &model.input_mappings {
            // Get the source data
            let source_data = redis.get_hash(&mapping.source_key)?;
            
            // Check if the source field exists
            if let Some(value) = source_data.get(&mapping.source_field) {
                let transformed_value = self.apply_transform(value, &mapping.transform)?;
                model_inputs.insert(mapping.target_field.clone(), transformed_value);
            } else {
                warn!(
                    "Source field '{}' not found in key '{}'",
                    mapping.source_field, mapping.source_key
                );
            }
        }
        
        // Process the model (in a real implementation, this would apply the actual model logic)
        let model_outputs = self.process_model_data(model, &model_inputs)?;
        
        // Store the results in Redis
        redis.set_hash(&model.output_key, &model_outputs)?;
        
        Ok(())
    }
    
    fn apply_transform(&self, value: &str, transform: &Option<String>) -> Result<String> {
        if let Some(transform_expr) = transform {
            // In a real implementation, this would parse and apply the transformation expression
            // For now, we'll just implement a simple scaling transform as an example
            if transform_expr.starts_with("scale:") {
                let scale_factor: f64 = transform_expr
                    .strip_prefix("scale:")
                    .unwrap_or("1.0")
                    .parse()
                    .map_err(|_| {
                        ModelSrvError::DataMappingError(format!(
                            "Invalid scale factor in transform: {}",
                            transform_expr
                        ))
                    })?;
                
                let value_f64: f64 = value.parse().map_err(|_| {
                    ModelSrvError::DataMappingError(format!(
                        "Cannot apply scaling transform to non-numeric value: {}",
                        value
                    ))
                })?;
                
                return Ok((value_f64 * scale_factor).to_string());
            }
            
            // Add more transform types as needed
            
            warn!("Unknown transform expression: {}", transform_expr);
        }
        
        // If no transform or unknown transform, return the original value
        Ok(value.to_string())
    }
    
    fn process_model_data(
        &self,
        model: &ModelDefinition,
        inputs: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        // In a real implementation, this would apply the actual model logic
        // For this example, we'll just pass through the inputs to outputs
        // with a timestamp added
        
        let mut outputs = inputs.clone();
        
        // Add a timestamp
        let timestamp = chrono::Utc::now().to_rfc3339();
        outputs.insert("timestamp".to_string(), timestamp);
        outputs.insert("model_id".to_string(), model.id.clone());
        outputs.insert("model_name".to_string(), model.name.clone());
        
        Ok(outputs)
    }

    /// 检查并执行控制动作
    fn check_and_execute_actions(
        &self,
        redis: &mut RedisConnection,
        model: &ModelDefinition,
        actions: &[ControlAction],
    ) -> Result<()> {
        // 获取模型输出数据
        let model_outputs = redis.get_hash(&model.output_key)?;
        
        for action in actions {
            if !action.enabled {
                debug!("Skipping disabled action: {}", action.id);
                continue;
            }
            
            // 检查是否满足所有条件
            let mut conditions_met = true;
            
            for condition in &action.conditions {
                if let Some(field_value) = model_outputs.get(&condition.field) {
                    if !self.evaluate_condition(field_value, &condition.operator, &condition.value)? {
                        conditions_met = false;
                        break;
                    }
                } else {
                    warn!("Field '{}' not found in model outputs", condition.field);
                    conditions_met = false;
                    break;
                }
            }
            
            // 如果满足所有条件，执行动作
            if conditions_met {
                info!("Executing control action: {} ({})", action.name, action.id);
                
                match action.action_type {
                    ControlActionType::RemoteControl => {
                        let value = action.value.parse::<bool>().unwrap_or(false);
                        let command_id = self.comsrv_handler.send_remote_control(
                            redis,
                            &action.channel,
                            &action.point,
                            value,
                        )?;
                        info!("Sent remote control command: {}", command_id);
                    }
                    ControlActionType::RemoteAdjust => {
                        let value = action.value.parse::<f64>().unwrap_or(0.0);
                        let command_id = self.comsrv_handler.send_remote_adjust(
                            redis,
                            &action.channel,
                            &action.point,
                            value,
                        )?;
                        info!("Sent remote adjust command: {}", command_id);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// 评估条件
    fn evaluate_condition(&self, field_value: &str, operator: &str, compare_value: &str) -> Result<bool> {
        // 尝试将两个值都解析为数字进行比较
        let field_num = field_value.parse::<f64>();
        let compare_num = compare_value.parse::<f64>();
        
        if let (Ok(field_num), Ok(compare_num)) = (field_num, compare_num) {
            // 数值比较
            return match operator {
                ">" => Ok(field_num > compare_num),
                "<" => Ok(field_num < compare_num),
                ">=" => Ok(field_num >= compare_num),
                "<=" => Ok(field_num <= compare_num),
                "==" => Ok(field_num == compare_num),
                "!=" => Ok(field_num != compare_num),
                _ => Err(ModelSrvError::DataMappingError(format!(
                    "Unknown operator: {}", operator
                ))),
            };
        }
        
        // 字符串比较
        match operator {
            "==" => Ok(field_value == compare_value),
            "!=" => Ok(field_value != compare_value),
            _ => Err(ModelSrvError::DataMappingError(format!(
                "Operator {} not supported for string comparison", operator
            ))),
        }
    }

    /// 发送遥控命令
    pub fn send_remote_control(
        &self,
        redis: &mut RedisConnection,
        channel: &str,
        point: &str,
        value: bool,
    ) -> Result<String> {
        self.comsrv_handler.send_remote_control(redis, channel, point, value)
    }

    /// 发送遥调命令
    pub fn send_remote_adjust(
        &self,
        redis: &mut RedisConnection,
        channel: &str,
        point: &str,
        value: f64,
    ) -> Result<String> {
        self.comsrv_handler.send_remote_adjust(redis, channel, point, value)
    }

    /// 获取命令状态
    pub fn get_command_status(
        &self,
        redis: &mut RedisConnection,
        command_id: &str,
    ) -> Result<CommandStatus> {
        self.comsrv_handler.get_command_status(redis, command_id)
    }

    /// 取消命令
    pub fn cancel_command(
        &self,
        redis: &mut RedisConnection,
        command_id: &str,
    ) -> Result<()> {
        self.comsrv_handler.cancel_command(redis, command_id)
    }
} 