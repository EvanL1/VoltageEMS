use crate::error::{ModelSrvError, Result};
use crate::comsrv_handler::{ComsrvHandler, CommandType, CommandStatus};
use crate::redis_handler::RedisConnection;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 控制操作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ControlOperationType {
    /// 启动操作
    Start,
    /// 停止操作
    Stop,
    /// 暂停操作
    Pause,
    /// 恢复操作
    Resume,
    /// 重置操作
    Reset,
    /// 调整参数操作
    Adjust,
    /// 自定义操作
    Custom(String),
}

/// 控制目标类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ControlTargetType {
    /// 设备控制
    Device,
    /// 系统控制
    System,
    /// 模型控制
    Model,
    /// 自定义控制目标
    Custom(String),
}

/// 控制操作参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlParameter {
    /// 参数名称
    pub name: String,
    /// 参数值
    pub value: String,
    /// 参数描述
    pub description: Option<String>,
}

/// 控制操作条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCondition {
    /// 条件ID
    pub id: String,
    /// 条件描述
    pub description: Option<String>,
    /// 数据源键
    pub source_key: String,
    /// 数据字段
    pub field: String,
    /// 比较运算符 (>, <, >=, <=, ==, !=)
    pub operator: String,
    /// 比较值
    pub value: String,
    /// 持续时间（毫秒），如果指定，则条件必须持续满足指定时间才算满足
    pub duration_ms: Option<u64>,
}

/// 控制操作定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlOperation {
    /// 操作ID
    pub id: String,
    /// 操作名称
    pub name: String,
    /// 操作描述
    pub description: Option<String>,
    /// 操作类型
    pub operation_type: ControlOperationType,
    /// 控制目标类型
    pub target_type: ControlTargetType,
    /// 目标ID
    pub target_id: String,
    /// 操作参数
    pub parameters: Vec<ControlParameter>,
    /// 触发条件
    pub conditions: Vec<ControlCondition>,
    /// 优先级（数字越小优先级越高）
    pub priority: i32,
    /// 是否启用
    pub enabled: bool,
    /// 冷却时间（毫秒），操作执行后需要等待的时间才能再次执行
    pub cooldown_ms: Option<u64>,
    /// 上次执行时间戳
    pub last_executed_at: Option<i64>,
}

/// 控制操作执行器特性
pub trait ControlOperationExecutor {
    /// 执行控制操作
    fn execute(&self, 
               redis: &mut RedisConnection, 
               operation: &ControlOperation) -> Result<String>;
    
    /// 检查操作是否可执行
    fn can_execute(&self, 
                  redis: &mut RedisConnection, 
                  operation: &ControlOperation) -> Result<bool>;
}

/// 设备控制操作执行器
pub struct DeviceControlExecutor {
    comsrv_handler: ComsrvHandler,
    // 设备类型到通道映射
    device_channel_map: HashMap<String, String>,
    // 操作类型到点位映射
    operation_point_map: HashMap<ControlOperationType, String>,
}

impl DeviceControlExecutor {
    pub fn new(redis_prefix: &str) -> Self {
        let mut device_channel_map = HashMap::new();
        let mut operation_point_map = HashMap::new();
        
        // 初始化设备类型到通道的映射
        device_channel_map.insert("battery".to_string(), "Battery_Serial".to_string());
        device_channel_map.insert("pcs".to_string(), "PCS_Serial".to_string());
        device_channel_map.insert("diesel".to_string(), "Diesel_Serial".to_string());
        
        // 初始化操作类型到点位的映射
        operation_point_map.insert(ControlOperationType::Start, "start_command".to_string());
        operation_point_map.insert(ControlOperationType::Stop, "stop_command".to_string());
        operation_point_map.insert(ControlOperationType::Reset, "reset_command".to_string());
        
        DeviceControlExecutor {
            comsrv_handler: ComsrvHandler::new(redis_prefix),
            device_channel_map,
            operation_point_map,
        }
    }
    
    /// 获取设备通道
    fn get_device_channel(&self, device_id: &str) -> Option<&String> {
        self.device_channel_map.get(device_id)
    }
    
    /// 获取操作点位
    fn get_operation_point(&self, operation_type: &ControlOperationType) -> Option<&String> {
        self.operation_point_map.get(operation_type)
    }
}

impl ControlOperationExecutor for DeviceControlExecutor {
    fn execute(&self, 
               redis: &mut RedisConnection, 
               operation: &ControlOperation) -> Result<String> {
        // 检查目标类型
        if operation.target_type != ControlTargetType::Device {
            return Err(ModelSrvError::InvalidOperation(
                format!("DeviceControlExecutor can only execute Device operations, got {:?}", 
                        operation.target_type)));
        }
        
        // 获取设备通道
        let channel = match self.get_device_channel(&operation.target_id) {
            Some(ch) => ch,
            None => return Err(ModelSrvError::InvalidOperation(
                format!("Unknown device ID: {}", operation.target_id))),
        };
        
        // 获取操作点位
        let point = match self.get_operation_point(&operation.operation_type) {
            Some(pt) => pt,
            None => {
                // 对于自定义操作，从参数中获取点位
                if let ControlOperationType::Custom(_) = operation.operation_type {
                    let point_param = operation.parameters.iter()
                        .find(|p| p.name == "point");
                    
                    match point_param {
                        Some(p) => &p.value,
                        None => return Err(ModelSrvError::InvalidOperation(
                            "Custom operation requires 'point' parameter".to_string())),
                    }
                } else {
                    return Err(ModelSrvError::InvalidOperation(
                        format!("Unsupported operation type: {:?}", operation.operation_type)));
                }
            }
        };
        
        // 获取操作值
        let value = operation.parameters.iter()
            .find(|p| p.name == "value")
            .map(|p| p.value.clone())
            .unwrap_or_else(|| "1".to_string()); // 默认值为1
        
        // 根据操作类型发送遥控或遥调命令
        let command_id = match operation.operation_type {
            ControlOperationType::Start | 
            ControlOperationType::Stop | 
            ControlOperationType::Pause | 
            ControlOperationType::Resume | 
            ControlOperationType::Reset => {
                // 这些操作通常是布尔值
                let bool_value = value.parse::<bool>().unwrap_or(true);
                self.comsrv_handler.send_remote_control(
                    redis,
                    channel,
                    point,
                    bool_value,
                )?
            },
            ControlOperationType::Adjust | 
            ControlOperationType::Custom(_) => {
                // 这些操作通常是数值
                let float_value = value.parse::<f64>().unwrap_or(0.0);
                self.comsrv_handler.send_remote_adjust(
                    redis,
                    channel,
                    point,
                    float_value,
                )?
            }
        };
        
        // 更新操作的最后执行时间
        // 注意：这里只是返回命令ID，实际上应该在某处更新operation的last_executed_at字段
        
        Ok(command_id)
    }
    
    fn can_execute(&self, 
                  redis: &mut RedisConnection, 
                  operation: &ControlOperation) -> Result<bool> {
        // 检查操作是否启用
        if !operation.enabled {
            return Ok(false);
        }
        
        // 检查冷却时间
        if let (Some(cooldown), Some(last_executed)) = (operation.cooldown_ms, operation.last_executed_at) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_millis() as i64;
                
            if now - last_executed < cooldown as i64 {
                return Ok(false);
            }
        }
        
        // 检查设备和操作是否支持
        if !self.device_channel_map.contains_key(&operation.target_id) {
            return Ok(false);
        }
        
        if !matches!(operation.operation_type, ControlOperationType::Custom(_)) && 
           !self.operation_point_map.contains_key(&operation.operation_type) {
            return Ok(false);
        }
        
        Ok(true)
    }
}

/// 控制管理器
pub struct ControlManager {
    executors: HashMap<ControlTargetType, Box<dyn ControlOperationExecutor>>,
    operations: Vec<ControlOperation>,
    condition_states: HashMap<String, (bool, Option<i64>)>, // (当前状态, 状态开始时间)
}

impl ControlManager {
    pub fn new(redis_prefix: &str) -> Self {
        let mut executors = HashMap::new();
        
        // 注册设备控制执行器
        executors.insert(
            ControlTargetType::Device, 
            Box::new(DeviceControlExecutor::new(redis_prefix)) as Box<dyn ControlOperationExecutor>
        );
        
        // 可以注册其他类型的执行器
        
        ControlManager {
            executors,
            operations: Vec::new(),
            condition_states: HashMap::new(),
        }
    }
    
    /// 加载控制操作
    pub fn load_operations(&mut self, redis: &mut RedisConnection, pattern: &str) -> Result<()> {
        // 清除现有操作
        self.operations.clear();
        
        // 获取所有控制操作配置键
        let operation_keys = redis.get_keys(pattern)?;
        info!("Found {} control operations", operation_keys.len());
        
        for key in &operation_keys {
            match redis.get_json::<ControlOperation>(key) {
                Ok(operation) => {
                    info!("Loaded control operation: {} ({})", operation.name, operation.id);
                    self.operations.push(operation);
                },
                Err(err) => {
                    error!("Failed to load control operation from {}: {}", key, err);
                }
            }
        }
        
        // 按优先级排序
        self.operations.sort_by_key(|op| op.priority);
        
        Ok(())
    }
    
    /// 评估条件
    fn evaluate_condition(
        &mut self,
        redis: &mut RedisConnection,
        condition: &ControlCondition,
        now: i64,
    ) -> Result<bool> {
        // 获取数据源
        let data = redis.get_hash(&condition.source_key)?;
        
        // 获取字段值
        let field_value = match data.get(&condition.field) {
            Some(value) => value,
            None => {
                warn!("Field '{}' not found in {}", condition.field, condition.source_key);
                return Ok(false);
            }
        };
        
        // 评估条件
        let condition_met = match condition.operator.as_str() {
            ">" => {
                let field_num = field_value.parse::<f64>().unwrap_or(0.0);
                let value_num = condition.value.parse::<f64>().unwrap_or(0.0);
                field_num > value_num
            },
            "<" => {
                let field_num = field_value.parse::<f64>().unwrap_or(0.0);
                let value_num = condition.value.parse::<f64>().unwrap_or(0.0);
                field_num < value_num
            },
            ">=" => {
                let field_num = field_value.parse::<f64>().unwrap_or(0.0);
                let value_num = condition.value.parse::<f64>().unwrap_or(0.0);
                field_num >= value_num
            },
            "<=" => {
                let field_num = field_value.parse::<f64>().unwrap_or(0.0);
                let value_num = condition.value.parse::<f64>().unwrap_or(0.0);
                field_num <= value_num
            },
            "==" => field_value == &condition.value,
            "!=" => field_value != &condition.value,
            _ => {
                warn!("Unknown operator: {}", condition.operator);
                false
            }
        };
        
        // 处理持续时间条件
        if let Some(duration) = condition.duration_ms {
            let condition_key = format!("{}:{}", condition.id, condition.field);
            let state_entry = self.condition_states.entry(condition_key.clone());
            
            match state_entry {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let (current_state, start_time) = entry.get_mut();
                    
                    if *current_state != condition_met {
                        // 状态改变，重置开始时间
                        *current_state = condition_met;
                        *start_time = Some(now);
                        return Ok(false);
                    } else if condition_met {
                        // 状态未改变且条件满足，检查持续时间
                        if let Some(start) = *start_time {
                            return Ok(now - start >= duration as i64);
                        }
                    }
                    
                    Ok(false)
                },
                std::collections::hash_map::Entry::Vacant(entry) => {
                    // 首次检查该条件
                    entry.insert((condition_met, Some(now)));
                    Ok(false)
                }
            }
        } else {
            // 没有持续时间要求，直接返回条件结果
            Ok(condition_met)
        }
    }
    
    /// 检查并执行控制操作
    pub fn check_and_execute_operations(&mut self, redis: &mut RedisConnection) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_millis() as i64;
            
        // 按优先级检查操作
        for operation in &mut self.operations {
            // 检查操作是否可执行
            let executor = match self.executors.get(&operation.target_type) {
                Some(exec) => exec,
                None => {
                    warn!("No executor found for target type: {:?}", operation.target_type);
                    continue;
                }
            };
            
            if !executor.can_execute(redis, operation)? {
                continue;
            }
            
            // 检查是否满足所有条件
            let mut all_conditions_met = true;
            
            for condition in &operation.conditions {
                if !self.evaluate_condition(redis, condition, now)? {
                    all_conditions_met = false;
                    break;
                }
            }
            
            // 如果满足所有条件，执行操作
            if all_conditions_met {
                info!("Executing control operation: {} ({})", operation.name, operation.id);
                
                match executor.execute(redis, operation) {
                    Ok(command_id) => {
                        info!("Control operation executed successfully: {}", command_id);
                        operation.last_executed_at = Some(now);
                    },
                    Err(err) => {
                        error!("Failed to execute control operation: {}", err);
                    }
                }
            }
        }
        
        Ok(())
    }
} 