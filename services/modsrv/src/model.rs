use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::storage::DataStore;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Control action type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlActionType {
    /// Remote control action (boolean value)
    RemoteControl,
    /// Remote adjustment action (analog value)
    RemoteAdjust,
}

/// Control action condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlActionCondition {
    /// Condition field
    pub field: String,
    /// Comparison operator (>, <, >=, <=, ==, !=)
    pub operator: String,
    /// Comparison value
    pub value: String,
}

/// Control action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAction {
    /// Action ID
    pub id: String,
    /// Action name
    pub name: String,
    /// Action type
    pub action_type: ControlActionType,
    /// Channel name
    pub channel: String,
    /// Point name
    pub point: String,
    /// Action value
    pub value: String,
    /// Trigger conditions
    pub conditions: Vec<ControlActionCondition>,
    /// Whether enabled
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWithActions {
    /// Basic model definition
    pub model: ModelDefinition,
    /// Control action list
    pub actions: Vec<ControlAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

pub struct ModelEngine {
    models: HashMap<String, ModelDefinition>,
    actions: HashMap<String, Vec<ControlAction>>,
    key_prefix: String,
}

impl ModelEngine {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            actions: HashMap::new(),
            key_prefix: String::new(),
        }
    }

    pub fn load_models<T: DataStore>(&mut self, store: &T, pattern: &str) -> Result<()> {
        // Clear existing models
        self.models.clear();
        self.actions.clear();

        // Get all model configuration keys
        let model_keys = store.get_keys(pattern)?;
        info!("Found {} model configurations", model_keys.len());

        for key in model_keys {
            match self.load_model_from_store(store, &key) {
                Ok((model, actions)) => {
                    if model.enabled {
                        info!("Loaded model: {} ({})", model.name, model.id);
                        self.models.insert(model.id.clone(), model);
                        self.actions.insert(key.clone(), actions);
                    } else {
                        debug!("Skipped disabled model: {}", key);
                    }
                }
                Err(e) => {
                    error!("Failed to load model from {}: {}", key, e);
                }
            }
        }

        info!("Loaded {} models", self.models.len());
        Ok(())
    }

    fn load_model_from_store<T: DataStore>(&self, store: &T, key: &str) -> Result<(ModelDefinition, Vec<ControlAction>)> {
        // 尝试从哈希表中加载模型
        let model_hash = match store.get_hash(key) {
            Ok(hash) => hash,
            Err(e) => {
                debug!("Failed to load model as hash, trying string format: {}", e);
                // 如果哈希表加载失败，尝试旧的字符串格式
                let model_json = store.get_string(key)?;
                
                // 尝试解析为带控制动作的模型
                let model_with_actions = serde_json::from_str::<ModelWithActions>(&model_json);
                
                if let Ok(model_with_actions) = model_with_actions {
                    return Ok((model_with_actions.model, model_with_actions.actions));
                }
                
                // 如果解析失败，尝试解析为基本模型
                let model: ModelDefinition = serde_json::from_str(&model_json)
                    .map_err(|e| ModelSrvError::ModelError(format!("Failed to parse model JSON: {}", e)))?;
                
                return Ok((model, Vec::new()));
            }
        };
        
        debug!("Loading model from hash: {}", key);
        
        // 从哈希表构建模型定义
        let id = model_hash.get("id")
            .ok_or_else(|| ModelSrvError::ModelError("Missing id field in model hash".to_string()))?
            .to_string();
            
        let name = model_hash.get("name")
            .unwrap_or(&"Unnamed Model".to_string())
            .to_string();
            
        let description = model_hash.get("description")
            .unwrap_or(&"No description".to_string())
            .to_string();
            
        // 解析输入映射
        let input_mappings = if let Some(mappings_json) = model_hash.get("input_mappings") {
            serde_json::from_str::<Vec<DataMapping>>(mappings_json)
                .map_err(|e| ModelSrvError::ModelError(format!("Failed to parse input mappings: {}", e)))?
        } else {
            Vec::new()
        };
        
        let output_key = model_hash.get("output_key")
            .unwrap_or(&format!("ems:model:output:{}", id))
            .to_string();
            
        let enabled = model_hash.get("enabled")
            .map(|v| v == "true")
            .unwrap_or(true);
            
        let model = ModelDefinition {
            id,
            name,
            description,
            input_mappings,
            output_key,
            enabled,
        };
        
        // 加载动作
        let mut actions = Vec::new();
        
        if let Some(action_count_str) = model_hash.get("action_count") {
            if let Ok(action_count) = action_count_str.parse::<usize>() {
                for i in 0..action_count {
                    let action_key = format!("{}model:action:{}:{}", self.key_prefix, model.id, i);
                    match store.get_string(&action_key) {
                        Ok(action_json) => {
                            match serde_json::from_str::<ControlAction>(&action_json) {
                                Ok(action) => {
                                    actions.push(action);
                                },
                                Err(e) => {
                                    warn!("Failed to parse action JSON: {}", e);
                                }
                            }
                        },
                        Err(e) => {
                            warn!("Failed to load action {}: {}", i, e);
                        }
                    }
                }
            }
        }
        
        Ok((model, actions))
    }

    pub fn execute_models<T: DataStore>(&self, store: &T) -> Result<()> {
        for (id, model) in &self.models {
            if !model.enabled {
                debug!("Skipping disabled model: {}", id);
                continue;
            }

            match self.execute_model(store, model) {
                Ok(_) => {
                    debug!("Successfully executed model: {}", id);
                    
                    // 执行模型后检查是否需要触发控制动作
                    if let Some(actions) = self.actions.get(id) {
                        self.check_and_execute_actions(store, id)?;
                    }
                }
                Err(e) => {
                    error!("Failed to execute model {}: {}", id, e);
                }
            }
        }

        Ok(())
    }

    fn execute_model<T: DataStore>(&self, store: &T, model: &ModelDefinition) -> Result<()> {
        // Collect input data based on mappings
        let mut model_inputs: HashMap<String, String> = HashMap::new();
        
        for mapping in &model.input_mappings {
            // Get the source data
            if let Ok(value) = store.get_string(&mapping.source_key) {
                let transformed_value = self.apply_transform(value, &mapping.transform)?;
                model_inputs.insert(mapping.source_field.clone(), transformed_value);
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
        let output_json = serde_json::to_string(&model_outputs)
            .map_err(|e| ModelSrvError::JsonError(e))?;
            
        store.set_string(&model.output_key, &output_json)?;
        
        // 检查并执行控制动作
        if let Some(actions) = self.actions.get(&format!("model:config:{}", model.id)) {
            if let Err(e) = self.check_and_execute_actions(store, &model.id) {
                error!("Failed to execute actions for model {}: {}", model.id, e);
            }
        }
        
        Ok(())
    }
    
    fn apply_transform(&self, value: String, transform: &Option<String>) -> Result<String> {
        let value_str = value.as_str();
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
                
                let value_f64: f64 = value_str.parse().map_err(|_| {
                    ModelSrvError::DataMappingError(format!(
                        "Cannot apply scaling transform to non-numeric value: {}",
                        value_str
                    ))
                })?;
                
                return Ok((value_f64 * scale_factor).to_string());
            }
            
            // Add more transform types as needed
            
            warn!("Unknown transform expression: {}", transform_expr);
        }
        
        // If no transform or unknown transform, return the original value
        Ok(value_str.to_string())
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

    /// 检查并执行模型动作
    pub fn check_and_execute_actions<T: DataStore>(&self, store: &T, model_id: &str) -> Result<()> {
        let model_key = format!("{}model:config:{}", self.key_prefix, model_id);
        let model_json = store.get_string(&model_key)?;
        
        // 尝试解析为带控制动作的模型
        let model_with_actions = serde_json::from_str::<ModelWithActions>(&model_json);
        
        if let Err(_) = model_with_actions {
            // 如果不是带动作的模型，则跳过
            return Ok(());
        }
        
        let model_with_actions = model_with_actions.unwrap();
        let model = &model_with_actions.model;
        let actions = &model_with_actions.actions;
        
        // 如果模型未启用，则跳过
        if !model.enabled {
            debug!("Model {} is disabled, skipping actions", model_id);
            return Ok(());
        }
        
        // 获取模型输出
        let output_key = &model.output_key;
        let output_json = match store.get_string(output_key) {
            Ok(json) => json,
            Err(_) => {
                debug!("No output found for model {}", model_id);
                return Ok(());
            }
        };
        
        // 解析模型输出
        let model_outputs: HashMap<String, String> = match serde_json::from_str(&output_json) {
            Ok(outputs) => outputs,
            Err(e) => {
                warn!("Failed to parse model output: {}", e);
                return Ok(());
            }
        };
        
        for action in actions {
            if !action.enabled {
                debug!("Skipping disabled action: {}", action.id);
                continue;
            }
            
            // Check if all conditions are met
            let mut conditions_met = true;
            
            for condition in &action.conditions {
                // 使用String作为键查找
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
            
            if conditions_met {
                info!("Executing action: {} for model {}", action.id, model_id);
                
                // 执行动作
                // 在实际实现中，这里会根据action_type执行不同的操作
                // 例如，发送控制命令到设备、触发其他模型等
                
                // 记录动作执行
                let action_log_key = format!("{}model:action:{}:{}", self.key_prefix, model_id, action.id);
                let action_log = format!(
                    r#"{{"model_id":"{}","action_id":"{}","executed_at":{}}}"#,
                    model_id, action.id, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
                );
                
                // 这里应该使用一个实际的Redis连接来存储日志
                // 为简化示例，我们只打印日志
                debug!("Action log: {}", action_log);
            }
        }
        
        Ok(())
    }

    /// Evaluate condition
    fn evaluate_condition(&self, field_value: &str, operator: &str, compare_value: &str) -> Result<bool> {
        // Try to parse both values as numbers for comparison
        let field_num = field_value.parse::<f64>();
        let compare_num = compare_value.parse::<f64>();
        
        if let (Ok(field_num), Ok(compare_num)) = (field_num, compare_num) {
            // Numeric comparison
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
        
        // String comparison
        match operator {
            "==" => Ok(field_value == compare_value),
            "!=" => Ok(field_value != compare_value),
            _ => Err(ModelSrvError::DataMappingError(format!(
                "Operator {} not supported for string comparison", operator
            ))),
        }
    }

    /// Send remote control command
    pub fn send_remote_control<T: DataStore>(
        &self,
        store: &T,
        channel: &str,
        point: &str,
        value: bool,
    ) -> Result<String> {
        // Simplified implementation, just log the operation
        let command_id = format!("cmd_{}", uuid::Uuid::new_v4());
        info!("Remote control: channel={}, point={}, value={}", channel, point, value);
        Ok(command_id)
    }

    /// Send remote adjustment command
    pub fn send_remote_adjust<T: DataStore>(
        &self,
        store: &T,
        channel: &str,
        point: &str,
        value: f64,
    ) -> Result<String> {
        // Simplified implementation, just log the operation
        let command_id = format!("cmd_{}", uuid::Uuid::new_v4());
        info!("Remote adjust: channel={}, point={}, value={}", channel, point, value);
        Ok(command_id)
    }

    /// Get command status
    pub fn get_command_status<T: DataStore>(
        &self,
        store: &T,
        command_id: &str,
    ) -> Result<CommandStatus> {
        // Simplified implementation, always return completed
        Ok(CommandStatus::Completed)
    }

    /// Cancel command
    pub fn cancel_command<T: DataStore>(
        &self,
        store: &T,
        command_id: &str,
    ) -> Result<()> {
        // Simplified implementation, just log the operation
        info!("Cancel command: {}", command_id);
        Ok(())
    }
} 