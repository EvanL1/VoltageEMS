use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
use tracing::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// remote control and remote adjust command type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    /// remote control command (switch)
    RemoteControl,
    /// remote adjust command (analog)
    RemoteAdjust,
}

/// remote control and remote adjust command status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandStatus {
    /// pendinging
    Pending,
    /// executingcuting
    Executing,
    /// successess
    Success,
    /// failed
    Failed,
    /// cancelled
    Cancelled,
}

/// remote control and remote adjust command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// command id
    pub id: String,
    /// channel name
    pub channel: String,
    /// point name
    pub point: String,
    /// command type
    pub command_type: CommandType,
    /// command value
    pub value: String,
    /// command status
    pub status: CommandStatus,
    /// created timestamp
    pub created_at: i64,
    /// executed timestamp
    pub executed_at: Option<i64>,
    /// completed timestamp
    pub completed_at: Option<i64>,
    /// error message
    pub error: Option<String>,
}

/// Comsrv handler
pub struct ComsrvHandler {
    /// Redis prefix
    prefix: String,
    /// command queue key
    command_queue_key: String,
    /// command status key pattern
    command_status_key_pattern: String,
}

impl ComsrvHandler {
    /// create new Comsrv handler
    pub fn new(redis_prefix: &str) -> Self {
        ComsrvHandler {
            prefix: redis_prefix.to_string(),
            command_queue_key: format!("{}command:queue", redis_prefix),
            command_status_key_pattern: format!("{}command:status:{{}}", redis_prefix),
        }
    }

    /// send remote control command
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

    /// send remote adjust command
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

    /// get command status
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

    /// wait for command completion
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
                    // check if timeout
                    if start_time.elapsed() > timeout_duration {
                        return Err(ModelSrvError::ConnectionError(
                            format!("Command execution timed out after {} ms", timeout_ms)
                        ));
                    }
                    
                    // wait for a while and check again
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// cancel command
    pub fn cancel_command(&self, redis: &mut RedisConnection, command_id: &str) -> Result<()> {
        let status_key = self.command_status_key_pattern.replace("{}", command_id);
        
        // check if command exists
        let status_data = redis.get_hash(&status_key)?;
        
        if status_data.is_empty() {
            return Err(ModelSrvError::DataMappingError(format!(
                "Command not found for ID: {}", command_id
            )));
        }
        
        // check command status
        let status_str = status_data.get("status").ok_or_else(|| {
            ModelSrvError::DataMappingError(format!(
                "Status field not found in command status for ID: {}", command_id
            ))
        })?;
        
        match status_str.as_str() {
            "pending" | "executing" => {
                // only pending or executing commands can be cancelled
                redis.set_hash_field(&status_key, "status", "cancelled")?;
                
                // set completed timestamp
                let now = chrono::Utc::now().timestamp();
                redis.set_hash_field(&status_key, "completed_at", &now.to_string())?;
                
                Ok(())
            }
            _ => Err(ModelSrvError::DataMappingError(format!(
                "Cannot cancel command with status: {}", status_str
            ))),
        }
    }

    // private helper methods

    /// generate command id
    fn generate_command_id(&self) -> String {
        use uuid::Uuid;
        Uuid::new_v4().to_string()
    }

    /// send command
    fn send_command(
        &self,
        redis: &mut RedisConnection,
        command_id: &str,
        channel: &str,
        point: &str,
        command_type: CommandType,
        value: &str,
    ) -> Result<String> {
        // create command object
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
        
        // serialize command
        let command_json = serde_json::to_string(&command)
            .map_err(|e| ModelSrvError::JsonError(e))?;
        
        // add command to queue
        redis.push_list(&self.command_queue_key, &command_json)?;
        
        // create command status record
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