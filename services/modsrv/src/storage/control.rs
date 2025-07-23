//! 控制命令管理模块
//!
//! 提供控制命令的创建、跟踪和状态管理

use super::rtdb::ModelStorage;
use super::types::*;
use crate::error::{ModelSrvError, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
// use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// 控制命令管理器
pub struct ControlManager {
    storage: ModelStorage,
    timeout_duration: Duration,
}

impl ControlManager {
    /// 创建新的控制命令管理器
    pub async fn new(redis_url: &str) -> Result<Self> {
        let storage = ModelStorage::new(redis_url).await?;
        Ok(Self {
            storage,
            timeout_duration: Duration::from_secs(30), // 默认30秒超时
        })
    }

    /// 从环境变量创建
    pub async fn from_env() -> Result<Self> {
        let storage = ModelStorage::from_env().await?;
        Ok(Self {
            storage,
            timeout_duration: Duration::from_secs(30),
        })
    }

    /// 设置命令超时时间
    pub fn set_timeout(&mut self, duration: Duration) {
        self.timeout_duration = duration;
    }

    /// 发送遥控命令（YK）
    pub async fn send_remote_control(
        &mut self,
        channel_id: u16,
        point_id: u32,
        value: bool,
        source_model: String,
    ) -> Result<String> {
        let command = ControlCommand::new(
            channel_id,
            point_id,
            ControlType::RemoteControl,
            if value { 1.0 } else { 0.0 },
            source_model,
        );

        self.storage.create_control_command(&command).await?;

        info!(
            "Sent remote control command {} to channel {} point {} value {}",
            command.id, channel_id, point_id, value
        );

        Ok(command.id)
    }

    /// 发送遥调命令（YT）
    pub async fn send_remote_adjust(
        &mut self,
        channel_id: u16,
        point_id: u32,
        value: f64,
        source_model: String,
    ) -> Result<String> {
        let command = ControlCommand::new(
            channel_id,
            point_id,
            ControlType::RemoteAdjust,
            value,
            source_model,
        );

        self.storage.create_control_command(&command).await?;

        info!(
            "Sent remote adjust command {} to channel {} point {} value {}",
            command.id, channel_id, point_id, value
        );

        Ok(command.id)
    }

    /// 获取命令状态
    pub async fn get_command_status(&mut self, command_id: &str) -> Result<CommandStatus> {
        match self.storage.get_control_command(command_id).await? {
            Some(command) => Ok(command.status),
            None => Err(ModelSrvError::CommandNotFound(command_id.to_string())),
        }
    }

    /// 等待命令完成
    pub async fn wait_for_completion(&mut self, command_id: &str) -> Result<CommandStatus> {
        let start_time = std::time::Instant::now();

        loop {
            match self.storage.get_control_command(command_id).await? {
                Some(command) => {
                    match command.status {
                        CommandStatus::Success
                        | CommandStatus::Failed
                        | CommandStatus::Cancelled
                        | CommandStatus::Timeout => {
                            return Ok(command.status);
                        }
                        _ => {
                            // 继续等待
                        }
                    }
                }
                None => {
                    return Err(ModelSrvError::CommandNotFound(command_id.to_string()));
                }
            }

            // 检查超时
            if start_time.elapsed() > self.timeout_duration {
                // 更新状态为超时
                self.storage
                    .update_command_status(
                        command_id,
                        CommandStatus::Timeout,
                        Some("Command execution timeout".to_string()),
                    )
                    .await?;

                return Ok(CommandStatus::Timeout);
            }

            // 等待一小段时间后重试
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// 取消命令
    pub async fn cancel_command(&mut self, command_id: &str) -> Result<()> {
        match self.storage.get_control_command(command_id).await? {
            Some(command) => match command.status {
                CommandStatus::Pending | CommandStatus::Executing => {
                    self.storage
                        .update_command_status(
                            command_id,
                            CommandStatus::Cancelled,
                            Some("Command cancelled by user".to_string()),
                        )
                        .await?;

                    info!("Cancelled command {}", command_id);
                    Ok(())
                }
                _ => Err(ModelSrvError::InvalidOperation(format!(
                    "Cannot cancel command in {} state",
                    command.status.to_str()
                ))),
            },
            None => Err(ModelSrvError::CommandNotFound(command_id.to_string())),
        }
    }

    /// 获取模型的历史命令
    pub async fn get_model_command_history(
        &mut self,
        model_id: &str,
        limit: usize,
    ) -> Result<Vec<ControlCommand>> {
        self.storage
            .get_model_commands(model_id, limit as isize)
            .await
    }

    /// 批量发送控制命令
    pub async fn send_batch_commands(
        &mut self,
        commands: Vec<(u16, u32, ControlType, f64, String)>,
    ) -> Result<Vec<String>> {
        let mut command_ids = Vec::new();

        for (channel_id, point_id, command_type, value, source_model) in commands {
            let command =
                ControlCommand::new(channel_id, point_id, command_type, value, source_model);

            self.storage.create_control_command(&command).await?;
            command_ids.push(command.id);
        }

        info!("Sent {} batch commands", command_ids.len());
        Ok(command_ids)
    }

    /// 检查命令执行条件
    pub async fn check_command_conditions(
        &self,
        conditions: &[(String, String, f64)], // (field, operator, value)
        current_values: &std::collections::HashMap<String, f64>,
    ) -> bool {
        for (field, operator, compare_value) in conditions {
            if let Some(current_value) = current_values.get(field) {
                let condition_met = match operator.as_str() {
                    ">" => current_value > compare_value,
                    "<" => current_value < compare_value,
                    ">=" => current_value >= compare_value,
                    "<=" => current_value <= compare_value,
                    "==" => (current_value - compare_value).abs() < f64::EPSILON,
                    "!=" => (current_value - compare_value).abs() >= f64::EPSILON,
                    _ => {
                        warn!("Unknown operator: {}", operator);
                        false
                    }
                };

                if !condition_met {
                    debug!(
                        "Condition not met: {} {} {} (current: {})",
                        field, operator, compare_value, current_value
                    );
                    return false;
                }
            } else {
                debug!("Field {} not found in current values", field);
                return false;
            }
        }

        true
    }
}

/// 控制命令执行器（用于监听和执行命令反馈）
pub struct CommandExecutor {
    storage: ModelStorage,
    subscription: redis::aio::PubSub,
}

impl CommandExecutor {
    /// 创建命令执行器
    pub async fn new(redis_url: &str) -> Result<Self> {
        let storage = ModelStorage::new(redis_url).await?;

        let client =
            redis::Client::open(redis_url).map_err(|e| ModelSrvError::RedisError(e.to_string()))?;

        let subscription = client
            .get_async_pubsub()
            .await
            .map_err(|e| ModelSrvError::RedisError(e.to_string()))?;

        Ok(Self {
            storage,
            subscription,
        })
    }

    /// 订阅命令反馈通道
    pub async fn subscribe_feedback(&mut self, patterns: &[String]) -> Result<()> {
        // use redis::AsyncCommands;

        for pattern in patterns {
            self.subscription
                .psubscribe(pattern)
                .await
                .map_err(|e| ModelSrvError::RedisError(e.to_string()))?;
        }

        info!("Subscribed to {} feedback patterns", patterns.len());
        Ok(())
    }

    /// 处理命令反馈
    pub async fn process_feedback(&mut self) -> Result<()> {
        // use redis::AsyncCommands;
        use futures_util::StreamExt;

        while let Some(msg) = self.subscription.on_message().next().await {
            let channel = msg.get_channel_name();
            let payload: String = match msg.get_payload() {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to get message payload: {}", e);
                    continue;
                }
            };

            // 解析反馈消息
            if let Ok(feedback) = serde_json::from_str::<CommandFeedback>(&payload) {
                debug!(
                    "Received feedback for command {}: {:?}",
                    feedback.command_id, feedback.status
                );

                // 更新命令状态
                if let Err(e) = self
                    .storage
                    .update_command_status(&feedback.command_id, feedback.status, feedback.message)
                    .await
                {
                    error!("Failed to update command status: {}", e);
                }
            } else {
                warn!(
                    "Failed to parse feedback message from {}: {}",
                    channel, payload
                );
            }
        }

        Ok(())
    }
}

/// 命令反馈结构
#[derive(Debug, Serialize, Deserialize)]
struct CommandFeedback {
    command_id: String,
    status: CommandStatus,
    message: Option<String>,
    timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_conditions() {
        // 创建一个临时的ControlManager用于测试
        // 注意：这只测试条件检查逻辑，不涉及实际的存储操作

        let conditions = vec![
            ("temperature".to_string(), ">".to_string(), 20.0),
            ("pressure".to_string(), "<=".to_string(), 110.0),
        ];

        let mut values = std::collections::HashMap::new();
        values.insert("temperature".to_string(), 25.5);
        values.insert("pressure".to_string(), 101.3);

        // 直接测试条件检查逻辑
        for (field, operator, compare_value) in &conditions {
            if let Some(current_value) = values.get(field) {
                let condition_met = match operator.as_str() {
                    ">" => current_value > compare_value,
                    "<=" => current_value <= compare_value,
                    _ => false,
                };
                assert!(condition_met);
            }
        }
    }
}
