use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 遥控/遥调命令类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    /// 遥控命令 (开关量)
    RemoteControl,
    /// 遥调命令 (模拟量)
    RemoteAdjust,
}

/// 遥控/遥调命令状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandStatus {
    /// 等待执行
    Pending,
    /// 执行中
    Executing,
    /// 执行成功
    Success,
    /// 执行失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 遥控/遥调命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// 命令ID
    pub id: String,
    /// 通道名称
    pub channel: String,
    /// 点位名称
    pub point: String,
    /// 命令类型
    pub command_type: CommandType,
    /// 命令值
    pub value: String,
    /// 命令状态
    pub status: CommandStatus,
    /// 创建时间戳
    pub created_at: i64,
    /// 执行时间戳
    pub executed_at: Option<i64>,
    /// 完成时间戳
    pub completed_at: Option<i64>,
    /// 错误信息
    pub error: Option<String>,
}

/// Comsrv处理器
pub struct ComsrvHandler {
    /// Redis前缀
    prefix: String,
    /// 命令队列键
    command_queue_key: String,
    /// 命令状态键模式
    command_status_key_pattern: String,
}

impl ComsrvHandler {
    /// 创建新的Comsrv处理器
    pub fn new(redis_prefix: &str) -> Self {
        ComsrvHandler {
            prefix: redis_prefix.to_string(),
            command_queue_key: format!("{}command:queue", redis_prefix),
            command_status_key_pattern: format!("{}command:status:{{}}", redis_prefix),
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
        let command_id = self.generate_command_id();
        let value_str = if value { "1" } else { "0" };
        
        self.send_command(
            redis,
            &command_id,
            channel,
            point,
            CommandType::RemoteControl,
            value_str,
        )
    }

    /// 发送遥调命令
    pub fn send_remote_adjust(
        &self,
        redis: &mut RedisConnection,
        channel: &str,
        point: &str,
        value: f64,
    ) -> Result<String> {
        let command_id = self.generate_command_id();
        let value_str = value.to_string();
        
        self.send_command(
            redis,
            &command_id,
            channel,
            point,
            CommandType::RemoteAdjust,
            &value_str,
        )
    }

    /// 获取命令状态
    pub fn get_command_status(&self, redis: &mut RedisConnection, command_id: &str) -> Result<CommandStatus> {
        let status_key = self.command_status_key_pattern.replace("{}", command_id);
        
        let status_data = redis.get_hash(&status_key)?;
        
        if status_data.is_empty() {
            return Err(ModelSrvError::DataMappingError(format!(
                "Command status not found for ID: {}", command_id
            )));
        }
        
        let status_str = status_data.get("status").ok_or_else(|| {
            ModelSrvError::DataMappingError(format!(
                "Status field not found in command status for ID: {}", command_id
            ))
        })?;
        
        match status_str.as_str() {
            "pending" => Ok(CommandStatus::Pending),
            "executing" => Ok(CommandStatus::Executing),
            "success" => Ok(CommandStatus::Success),
            "failed" => Ok(CommandStatus::Failed),
            "cancelled" => Ok(CommandStatus::Cancelled),
            _ => Err(ModelSrvError::DataMappingError(format!(
                "Unknown command status: {}", status_str
            ))),
        }
    }

    /// 等待命令完成
    pub async fn wait_for_command_completion(
        &self,
        redis: &mut RedisConnection,
        command_id: &str,
        timeout_ms: u64,
    ) -> Result<CommandStatus> {
        use tokio::time::{sleep, Duration};
        
        let start_time = std::time::Instant::now();
        let timeout_duration = Duration::from_millis(timeout_ms);
        
        loop {
            let status = self.get_command_status(redis, command_id)?;
            
            match status {
                CommandStatus::Success | CommandStatus::Failed | CommandStatus::Cancelled => {
                    return Ok(status);
                }
                _ => {
                    // 检查是否超时
                    if start_time.elapsed() > timeout_duration {
                        return Err(ModelSrvError::ConnectionError(
                            format!("Command execution timed out after {} ms", timeout_ms)
                        ));
                    }
                    
                    // 等待一段时间后再次检查
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// 取消命令
    pub fn cancel_command(&self, redis: &mut RedisConnection, command_id: &str) -> Result<()> {
        let status_key = self.command_status_key_pattern.replace("{}", command_id);
        
        // 检查命令是否存在
        let status_data = redis.get_hash(&status_key)?;
        
        if status_data.is_empty() {
            return Err(ModelSrvError::DataMappingError(format!(
                "Command not found for ID: {}", command_id
            )));
        }
        
        // 检查命令状态
        let status_str = status_data.get("status").ok_or_else(|| {
            ModelSrvError::DataMappingError(format!(
                "Status field not found in command status for ID: {}", command_id
            ))
        })?;
        
        match status_str.as_str() {
            "pending" | "executing" => {
                // 只有等待中或执行中的命令可以取消
                redis.set_hash_field(&status_key, "status", "cancelled")?;
                
                // 设置完成时间
                let now = chrono::Utc::now().timestamp();
                redis.set_hash_field(&status_key, "completed_at", &now.to_string())?;
                
                Ok(())
            }
            _ => Err(ModelSrvError::DataMappingError(format!(
                "Cannot cancel command with status: {}", status_str
            ))),
        }
    }

    // 私有辅助方法

    /// 生成命令ID
    fn generate_command_id(&self) -> String {
        use uuid::Uuid;
        Uuid::new_v4().to_string()
    }

    /// 发送命令
    fn send_command(
        &self,
        redis: &mut RedisConnection,
        command_id: &str,
        channel: &str,
        point: &str,
        command_type: CommandType,
        value: &str,
    ) -> Result<String> {
        // 创建命令对象
        let now = chrono::Utc::now().timestamp();
        
        let command = Command {
            id: command_id.to_string(),
            channel: channel.to_string(),
            point: point.to_string(),
            command_type,
            value: value.to_string(),
            status: CommandStatus::Pending,
            created_at: now,
            executed_at: None,
            completed_at: None,
            error: None,
        };
        
        // 序列化命令
        let command_json = serde_json::to_string(&command)
            .map_err(|e| ModelSrvError::JsonError(e))?;
        
        // 将命令添加到队列
        redis.push_list(&self.command_queue_key, &command_json)?;
        
        // 创建命令状态记录
        let status_key = self.command_status_key_pattern.replace("{}", command_id);
        
        let mut status_data = HashMap::new();
        status_data.insert("id".to_string(), command_id.to_string());
        status_data.insert("channel".to_string(), channel.to_string());
        status_data.insert("point".to_string(), point.to_string());
        status_data.insert("value".to_string(), value.to_string());
        status_data.insert("status".to_string(), "pending".to_string());
        status_data.insert("created_at".to_string(), now.to_string());
        
        redis.set_hash(&status_key, &status_data)?;
        
        info!("Command sent: {} to {}.{} with value {}", 
              command_id, channel, point, value);
        
        Ok(command_id.to_string())
    }
} 