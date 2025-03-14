use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::storage::DataStore;
use crate::model::{self, CommandStatus};
use uuid;
use redis::Commands;

/// Control operation type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ControlOperationType {
    /// Start operation
    Start,
    /// Stop operation
    Stop,
    /// Pause operation
    Pause,
    /// Resume operation
    Resume,
    /// Reset operation
    Reset,
    /// Adjust parameters operation
    Adjust,
    /// Custom operation
    Custom(String),
}

/// Control target type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ControlTargetType {
    /// Device control
    Device,
    /// System control
    System,
    /// Model control
    Model,
    /// Custom control target
    Custom(String),
}

/// Control operation parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlParameter {
    /// Parameter name
    pub name: String,
    /// Parameter value
    pub value: String,
    /// Parameter description
    pub description: Option<String>,
}

/// Control operation condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCondition {
    /// Condition ID
    pub id: String,
    /// Condition description
    pub description: Option<String>,
    /// Data source key
    pub source_key: String,
    /// Data field
    pub field: String,
    /// Comparison operator (>, <, >=, <=, ==, !=)
    pub operator: String,
    /// Comparison value
    pub value: String,
    /// Duration (milliseconds), if specified, the condition must be satisfied for the specified duration
    pub duration_ms: Option<u64>,
}

/// Control operation definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlOperation {
    /// Operation ID
    pub id: String,
    /// Operation name
    pub name: String,
    /// Operation description
    pub description: Option<String>,
    /// Operation type
    pub operation_type: ControlOperationType,
    /// Control target type
    pub target_type: ControlTargetType,
    /// Target ID
    pub target_id: String,
    /// Operation parameters
    pub parameters: Vec<ControlParameter>,
    /// Trigger conditions
    pub conditions: Vec<ControlCondition>,
    /// Priority (lower number means higher priority)
    pub priority: i32,
    /// Whether enabled
    pub enabled: bool,
    /// Cooldown time (milliseconds), time to wait after execution before executing again
    pub cooldown_ms: Option<u64>,
    /// Last execution timestamp
    pub last_executed_at: Option<i64>,
}

impl ControlOperation {
    /// Get a parameter by name
    pub fn get_parameter(&self, name: &str) -> Option<&String> {
        for param in &self.parameters {
            if param.name == name {
                return Some(&param.value);
            }
        }
        None
    }
}

/// Control operation executor trait
pub trait ControlOperationExecutor {
    /// Execute a control operation
    fn execute(&mut self,
               redis: &mut RedisConnection,
               operation: &ControlOperation,
               channel: &str,
               point: &str,
               value: &str) -> Result<String>;
    
    /// Check if operation can be executed
    fn can_execute(&self, 
                  redis: &mut RedisConnection, 
                  operation: &ControlOperation) -> Result<bool>;
}

/// Device control operation executor
pub struct DeviceControlExecutor {
    /// Redis key prefix
    redis_prefix: String,
    /// Operation type to point mapping
    operation_point_map: HashMap<ControlOperationType, String>,
}

impl DeviceControlExecutor {
    pub fn new(redis_prefix: &str) -> Self {
        let mut operation_point_map = HashMap::new();
        
        // Initialize operation type to point mapping
        operation_point_map.insert(ControlOperationType::Start, "start_command".to_string());
        operation_point_map.insert(ControlOperationType::Stop, "stop_command".to_string());
        operation_point_map.insert(ControlOperationType::Reset, "reset_command".to_string());
        
        DeviceControlExecutor {
            redis_prefix: redis_prefix.to_string(),
            operation_point_map,
        }
    }
    
    /// Get operation point
    fn get_operation_point(&self, operation_type: &ControlOperationType) -> Option<&String> {
        self.operation_point_map.get(operation_type)
    }

    /// 发送远程控制命令（布尔值）
    fn send_remote_control(
        &mut self,
        redis: &mut RedisConnection,
        channel_name: &str,
        point: &str,
        value: bool,
    ) -> Result<String> {
        let command_id = format!("cmd:{}", uuid::Uuid::new_v4());
        let command = format!(
            r#"{{"id":"{}","channel":"{}","point":"{}","value":{}}}"#,
            command_id, channel_name, point, value
        );
        
        redis.publish("device:control", &command)?;
        debug!("Published control command: {}", command);
        
        Ok(command_id)
    }
    
    /// 发送远程调整命令（数值）
    fn send_remote_adjust(
        &mut self,
        redis: &mut RedisConnection,
        channel_name: &str,
        point: &str,
        value: f64,
    ) -> Result<String> {
        let command_id = format!("cmd:{}", uuid::Uuid::new_v4());
        let command = format!(
            r#"{{"id":"{}","channel":"{}","point":"{}","value":{}}}"#,
            command_id, channel_name, point, value
        );
        
        redis.publish("device:adjust", &command)?;
        debug!("Published adjust command: {}", command);
        
        Ok(command_id)
    }

    /// 执行设备控制操作
    pub fn execute_device_operation(
        &mut self,
        redis: &mut RedisConnection,
        operation: &ControlOperation,
        channel_name: &str,
        point: &str,
        value: &str,
    ) -> Result<String> {
        match operation.operation_type {
            ControlOperationType::Start | 
            ControlOperationType::Stop | 
            ControlOperationType::Pause | 
            ControlOperationType::Resume | 
            ControlOperationType::Reset => {
                // These operations are typically boolean values
                let bool_value = value.parse::<bool>().unwrap_or(true);
                self.send_remote_control(
                    redis,
                    channel_name,
                    point,
                    bool_value,
                )
            },
            ControlOperationType::Adjust | 
            ControlOperationType::Custom(_) => {
                // These operations are typically numeric values
                let float_value = value.parse::<f64>().unwrap_or(0.0);
                self.send_remote_adjust(
                    redis,
                    channel_name,
                    point,
                    float_value,
                )
            }
        }
    }
}

impl ControlOperationExecutor for DeviceControlExecutor {
    fn execute(&mut self,
               redis: &mut RedisConnection,
               operation: &ControlOperation,
               channel: &str,
               point: &str,
               value: &str) -> Result<String> {
        match operation.operation_type {
            ControlOperationType::Start | 
            ControlOperationType::Stop | 
            ControlOperationType::Pause | 
            ControlOperationType::Resume | 
            ControlOperationType::Reset => {
                // These operations are typically boolean values
                let bool_value = value.parse::<bool>().unwrap_or(true);
                self.send_remote_control(
                    redis,
                    channel,
                    point,
                    bool_value,
                )
            },
            ControlOperationType::Adjust | 
            ControlOperationType::Custom(_) => {
                // These operations are typically numeric values
                let float_value = value.parse::<f64>().unwrap_or(0.0);
                self.send_remote_adjust(
                    redis,
                    channel,
                    point,
                    float_value,
                )
            }
        }
    }
    
    fn can_execute(&self, 
                  redis: &mut RedisConnection, 
                  operation: &ControlOperation) -> Result<bool> {
        // Check if operation is enabled
        if !operation.enabled {
            return Ok(false);
        }
        
        // Check cooldown time
        if let (Some(cooldown), Some(last_executed)) = (operation.cooldown_ms, operation.last_executed_at) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_millis() as i64;
                
            if now - last_executed < cooldown as i64 {
                return Ok(false);
            }
        }
        
        // Check if device and operation are supported
        if !self.operation_point_map.contains_key(&operation.operation_type) {
            return Ok(false);
        }
        
        Ok(true)
    }
}

/// Control manager
pub struct ControlManager {
    /// Redis key prefix
    key_prefix: String,
    /// Loaded operations
    operations: HashMap<String, ControlOperation>,
    /// Executed operations
    executed_operations: HashMap<String, SystemTime>,
}

impl ControlManager {
    /// Create a new control manager
    pub fn new(key_prefix: &str) -> Self {
        Self {
            key_prefix: key_prefix.to_string(),
            operations: HashMap::new(),
            executed_operations: HashMap::new(),
        }
    }
    
    /// Load control operations
    pub fn load_operations<T: DataStore>(&mut self, store: &T, pattern: &str) -> Result<()> {
        // Clear existing operations
        self.operations.clear();
        
        // Get all control operation configuration keys
        let operation_keys = store.get_keys(pattern)?;
        info!("Found {} control operations", operation_keys.len());
        
        for key in &operation_keys {
            match self.load_operation_from_store(store, &key) {
                Ok(operation) => {
                    if operation.enabled {
                        info!("Loaded control operation: {} ({})", operation.name, operation.id);
                        self.operations.insert(operation.id.clone(), operation);
                    } else {
                        debug!("Skipped disabled control operation: {}", key);
                    }
                },
                Err(e) => {
                    error!("Failed to load control operation from {}: {}", key, e);
                }
            }
        }
        
        info!("Loaded {} control operations", self.operations.len());
        
        Ok(())
    }
    
    /// Load control operation from store
    fn load_operation_from_store<T: DataStore>(&self, store: &T, key: &str) -> Result<ControlOperation> {
        // Get operation definition from store
        let json_str = store.get_string(key)?;
        
        // Parse JSON to ControlOperation
        let operation: ControlOperation = serde_json::from_str(&json_str)
            .map_err(|e| ModelSrvError::JsonError(e))?;
            
        Ok(operation)
    }
    
    /// Check and execute control operations
    pub fn check_and_execute_operations<T: DataStore>(&mut self, store: &T) -> Result<()> {
        // 创建一个操作ID的副本，以避免可变借用冲突
        let operation_ids: Vec<String> = self.operations.keys().cloned().collect();
        
        // 检查所有操作
        for op_id in &operation_ids {
            if let Some(operation) = self.operations.get(op_id) {
                // Skip disabled operations
                if !operation.enabled {
                    continue;
                }
                
                // Check if operation has been executed
                if let Some(last_executed) = self.executed_operations.get(&operation.id) {
                    let now = SystemTime::now();
                    let elapsed = now.duration_since(*last_executed).unwrap_or(Duration::from_secs(0));
                    
                    // Skip if operation is in cooldown period
                    if let Some(cooldown_ms) = operation.cooldown_ms {
                        if elapsed < Duration::from_millis(cooldown_ms) {
                            continue;
                        }
                    }
                }
                
                // 创建操作的克隆，以避免可变借用冲突
                let operation_clone = operation.clone();
                
                // Check operation conditions
                if self.check_operation_conditions(store, &operation_clone)? {
                    // Execute operation
                    if let Err(e) = self.execute_operation(store, &operation_clone) {
                        error!("Failed to execute operation {}: {}", operation_clone.id, e);
                    } else {
                        // Update execution time
                        self.executed_operations.insert(operation_clone.id.clone(), SystemTime::now());
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 检查操作条件是否满足
    fn check_operation_conditions<T: DataStore>(&self, store: &T, operation: &ControlOperation) -> Result<bool> {
        if operation.conditions.is_empty() {
            return Ok(true);
        }
        
        let now = SystemTime::now();
        
        for condition in &operation.conditions {
            let field_key = &condition.field;
            let field_value = match store.get_string(field_key) {
                Ok(value) => value,
                Err(e) => {
                    warn!("Failed to get field value for condition: {}", e);
                    return Ok(false);
                }
            };
            
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
                "==" => field_value == condition.value,
                "!=" => field_value != condition.value,
                _ => {
                    warn!("Unknown operator: {}", condition.operator);
                    false
                }
            };
            
            // 如果有持续时间要求，需要检查条件持续满足的时间
            if let Some(duration) = condition.duration_ms {
                let condition_key = format!("{}:{}", condition.id, condition.field);
                
                // 使用HashMap来存储条件状态
                let mut condition_states = HashMap::new();
                let state_entry = condition_states.entry(condition_key.clone());
                
                match state_entry {
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        let (current_state, start_time) = entry.get_mut();
                        
                        if *current_state != condition_met {
                            // 状态改变，重置开始时间
                            *current_state = condition_met;
                            *start_time = Some(now);
                            return Ok(false);
                        } else if condition_met {
                            // 状态未变且满足条件，检查持续时间
                            if let Some(start) = *start_time {
                                let elapsed = now.duration_since(start).unwrap_or(Duration::from_secs(0));
                                if elapsed.as_millis() < duration as u128 {
                                    return Ok(false);
                                }
                            }
                        } else {
                            // 状态未变但不满足条件
                            return Ok(false);
                        }
                    },
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        // 首次检查该条件
                        entry.insert((condition_met, Some(now)));
                        return Ok(false);
                    }
                }
            } else if !condition_met {
                // 没有持续时间要求，但条件不满足
                return Ok(false);
            }
        }
        
        // 所有条件都满足
        Ok(true)
    }
    
    /// 执行控制操作
    fn execute_operation<T: DataStore>(&mut self, store: &T, operation: &ControlOperation) -> Result<()> {
        info!("Executing operation: {} ({})", operation.name, operation.id);
        
        // 根据操作类型和目标类型执行不同的操作
        match operation.target_type {
            ControlTargetType::Device => {
                // 获取设备控制执行器
                let mut executor = DeviceControlExecutor::new(&self.key_prefix);
                
                // 获取操作参数
                let channel_name = operation.get_parameter("channel")
                    .ok_or_else(|| ModelSrvError::InvalidOperation(
                        format!("Missing channel parameter for device operation: {}", operation.id)
                    ))?;
                
                let point = operation.get_parameter("point")
                    .ok_or_else(|| ModelSrvError::InvalidOperation(
                        format!("Missing point parameter for device operation: {}", operation.id)
                    ))?;
                
                // 使用let绑定创建一个更长生命周期的值
                let default_value = "true".to_string();
                let value = operation.get_parameter("value")
                    .unwrap_or(&default_value);
                
                // 获取Redis连接
                let mut redis = RedisConnection::new();
                let redis_url = format!("redis://{}:{}", 
                    store.get_string("redis.host")?, 
                    store.get_string("redis.port")?
                );
                redis.connect(&redis_url)?;
                
                // 执行操作
                let command_id = executor.execute(&mut redis, operation, channel_name, point, value)?;
                
                // 记录操作执行结果
                let result_key = format!("{}control:result:{}", self.key_prefix, operation.id);
                let result = format!(
                    r#"{{"operation_id":"{}","command_id":"{}","executed_at":{}}}"#,
                    operation.id, command_id, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
                );
                
                redis.set_string(&result_key, &result)?;
            },
            ControlTargetType::Model => {
                // 模型控制操作
                let model_id = &operation.target_id;
                let model_key = format!("{}model:config:{}", self.key_prefix, model_id);
                
                if !store.exists(&model_key)? {
                    return Err(ModelSrvError::InvalidOperation(
                        format!("Model not found: {}", model_id)
                    ));
                }
                
                // 根据操作类型执行不同的模型操作
                match operation.operation_type {
                    ControlOperationType::Start => {
                        // 启用模型
                        let model_json = store.get_string(&model_key)?;
                        let mut model: model::ModelDefinition = serde_json::from_str(&model_json)
                            .map_err(|e| ModelSrvError::JsonError(e))?;
                        
                        model.enabled = true;
                        
                        let updated_json = serde_json::to_string(&model)
                            .map_err(|e| ModelSrvError::JsonError(e))?;
                        
                        // 更新模型配置
                        let mut redis = RedisConnection::new();
                        let redis_url = format!("redis://{}:{}", 
                            store.get_string("redis.host")?, 
                            store.get_string("redis.port")?
                        );
                        redis.connect(&redis_url)?;
                        
                        redis.set_string(&model_key, &updated_json)?;
                    },
                    ControlOperationType::Stop => {
                        // 禁用模型
                        let model_json = store.get_string(&model_key)?;
                        let mut model: model::ModelDefinition = serde_json::from_str(&model_json)
                            .map_err(|e| ModelSrvError::JsonError(e))?;
                        
                        model.enabled = false;
                        
                        let updated_json = serde_json::to_string(&model)
                            .map_err(|e| ModelSrvError::JsonError(e))?;
                        
                        // 更新模型配置
                        let mut redis = RedisConnection::new();
                        let redis_url = format!("redis://{}:{}", 
                            store.get_string("redis.host")?, 
                            store.get_string("redis.port")?
                        );
                        redis.connect(&redis_url)?;
                        
                        redis.set_string(&model_key, &updated_json)?;
                    },
                    _ => {
                        return Err(ModelSrvError::InvalidOperation(
                            format!("Unsupported operation type for model: {:?}", operation.operation_type)
                        ));
                    }
                }
            },
            _ => {
                return Err(ModelSrvError::InvalidOperation(
                    format!("Unsupported target type: {:?}", operation.target_type)
                ));
            }
        }
        
        Ok(())
    }
}

// Command type enum
#[derive(Debug, Clone, PartialEq)]
pub enum CommandType {
    RemoteControl,
    RemoteAdjust,
    Custom(String),
}

// Operation status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationStatus {
    Pending,
    Executing,
    Executed,
    Failed,
    Cancelled,
}

// Operation status record struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStatusRecord {
    pub operation_id: String,
    pub status: OperationStatus,
    pub timestamp: u64,
    pub message: Option<String>,
} 