//! 控制命令订阅器
//!
//! 负责从Redis订阅控制命令并分发给相应的通道处理

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use voltage_libs::redis::RedisClient;

use super::core::ChannelCommand;
use crate::utils::error::Result;

/// 控制命令类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandType {
    Control,
    Adjustment,
}

/// 控制命令消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    /// 命令ID
    pub command_id: String,
    /// 通道ID
    pub channel_id: u16,
    /// 命令类型
    pub command_type: CommandType,
    /// 点位ID
    pub point_id: u32,
    /// 命令值
    pub value: f64,
    /// 时间戳
    pub timestamp: i64,
    /// 可选的元数据
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// 命令状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStatus {
    pub command_id: String,
    pub status: String, // pending, executing, success, failed
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub timestamp: i64,
}

/// 命令订阅器配置
#[derive(Debug, Clone)]
pub struct CommandSubscriberConfig {
    pub channel_id: u16,
    pub redis_url: String,
}

/// 命令订阅器
pub struct CommandSubscriber {
    config: CommandSubscriberConfig,
    redis_client: Arc<Mutex<RedisClient>>,
    command_tx: mpsc::Sender<ChannelCommand>,
    is_running: Arc<RwLock<bool>>,
    task_handle: Option<JoinHandle<()>>,
}

impl CommandSubscriber {
    /// 创建新的命令订阅器
    pub async fn new(
        config: CommandSubscriberConfig,
        command_tx: mpsc::Sender<ChannelCommand>,
    ) -> Result<Self> {
        let redis_client = RedisClient::new(&config.redis_url).await?;

        Ok(Self {
            config,
            redis_client: Arc::new(Mutex::new(redis_client)),
            command_tx,
            is_running: Arc::new(RwLock::new(false)),
            task_handle: None,
        })
    }

    /// 启动订阅
    pub async fn start(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                warn!(
                    "Command subscriber already running for channel {}",
                    self.config.channel_id
                );
                return Ok(());
            }
            *running = true;
        }

        // 订阅控制和调节命令通道
        let control_channel = format!("cmd:{}:control", self.config.channel_id);
        let adjustment_channel = format!("cmd:{}:adjustment", self.config.channel_id);

        info!(
            "Starting command subscriber for channel {}, subscribing to: {} and {}",
            self.config.channel_id, control_channel, adjustment_channel
        );

        // 克隆必要的对象用于任务
        let redis_client = self.redis_client.clone();
        let command_tx = self.command_tx.clone();
        let is_running = self.is_running.clone();
        let channel_id = self.config.channel_id;

        // 启动订阅任务
        let task_handle = tokio::spawn(async move {
            if let Err(e) = Self::subscription_loop(
                redis_client,
                command_tx,
                is_running,
                channel_id,
                vec![control_channel, adjustment_channel],
            )
            .await
            {
                error!(
                    "Command subscription error for channel {}: {}",
                    channel_id, e
                );
            }
        });

        self.task_handle = Some(task_handle);
        Ok(())
    }

    /// 停止订阅
    pub async fn stop(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // 等待任务结束
        if let Some(handle) = self.task_handle.take() {
            // 给任务一些时间优雅退出
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => info!(
                    "Command subscriber stopped for channel {}",
                    self.config.channel_id
                ),
                Ok(Err(e)) => warn!("Command subscriber task error: {}", e),
                Err(_) => warn!("Command subscriber task timeout, forcing stop"),
            }
        }

        Ok(())
    }

    /// 订阅循环
    async fn subscription_loop(
        redis_client: Arc<Mutex<RedisClient>>,
        command_tx: mpsc::Sender<ChannelCommand>,
        is_running: Arc<RwLock<bool>>,
        channel_id: u16,
        channels: Vec<String>,
    ) -> Result<()> {
        // 创建订阅
        let channel_refs: Vec<&str> = channels.iter().map(|s| s.as_str()).collect();
        let mut redis_client = redis_client.lock().await;
        let mut pubsub = redis_client.subscribe(&channel_refs).await.map_err(|e| {
            crate::error::ComSrvError::InternalError(format!(
                "Failed to create subscription: {}",
                e
            ))
        })?;

        info!(
            "Command subscription established for channel {}",
            channel_id
        );

        loop {
            // 检查是否应该停止
            if !*is_running.read().await {
                info!("Stopping command subscription for channel {}", channel_id);
                break;
            }

            // 接收消息（带超时）
            match tokio::time::timeout(
                std::time::Duration::from_secs(1),
                pubsub.on_message().next(),
            )
            .await
            {
                Ok(Some(msg)) => {
                    // 处理消息
                    if let Err(e) = Self::process_message(&command_tx, channel_id, msg).await {
                        error!("Failed to process command message: {}", e);
                    }
                }
                Ok(None) => {
                    warn!("Subscription closed for channel {}", channel_id);
                    break;
                }
                Err(_) => {
                    // 超时，继续循环
                    continue;
                }
            }
        }

        Ok(())
    }

    /// 处理单个消息
    async fn process_message(
        command_tx: &mpsc::Sender<ChannelCommand>,
        channel_id: u16,
        msg: redis::Msg,
    ) -> Result<()> {
        // 获取消息内容
        let payload: String = msg.get_payload().map_err(|e| {
            crate::error::ComSrvError::InternalError(format!(
                "Failed to get message payload: {}",
                e
            ))
        })?;

        debug!(
            "Received command message on channel {}: {}",
            channel_id, payload
        );

        // 解析命令
        let command: ControlCommand = serde_json::from_str(&payload).map_err(|e| {
            crate::error::ComSrvError::ParsingError(format!("Failed to parse command: {}", e))
        })?;

        // 确保命令是发给正确的通道
        if command.channel_id != channel_id {
            warn!(
                "Received command for wrong channel: expected {}, got {}",
                channel_id, command.channel_id
            );
            return Ok(());
        }

        // 转换为ChannelCommand并发送
        let channel_command = match command.command_type {
            CommandType::Control => ChannelCommand::Control {
                command_id: command.command_id,
                point_id: command.point_id,
                value: command.value,
                timestamp: command.timestamp,
            },
            CommandType::Adjustment => ChannelCommand::Adjustment {
                command_id: command.command_id,
                point_id: command.point_id,
                value: command.value,
                timestamp: command.timestamp,
            },
        };

        // 发送命令到协议处理器
        if let Err(e) = command_tx.send(channel_command).await {
            error!("Failed to send command to protocol handler: {}", e);
            return Err(crate::error::ComSrvError::InternalError(
                "Command channel closed".to_string(),
            ));
        }

        debug!("Command forwarded to protocol handler");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_command_parsing() {
        let json = r#"{
            "command_id": "test-123",
            "channel_id": 1,
            "command_type": "control",
            "point_id": 1001,
            "value": 1.0,
            "timestamp": 1234567890,
            "metadata": {}
        }"#;

        let command: ControlCommand = serde_json::from_str(json).unwrap();
        assert_eq!(command.command_id, "test-123");
        assert_eq!(command.channel_id, 1);
        assert!(matches!(command.command_type, CommandType::Control));
        assert_eq!(command.point_id, 1001);
        assert_eq!(command.value, 1.0);
    }
}
