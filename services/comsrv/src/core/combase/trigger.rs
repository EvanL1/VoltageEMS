//! Control command subscriber
//!
//! Responsible for subscribing to control commands from Redis and distributing them to corresponding channels for processing

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{debug, error, info, warn};
use voltage_libs::redis::RedisClient;

use super::core::ChannelCommand;
use crate::utils::error::Result;

/// Control command type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    #[serde(rename = "control", alias = "C")]
    Control,
    #[serde(rename = "adjustment", alias = "A")]
    Adjustment,
}

/// Control command message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    /// Command ID (auto-generated if not provided)
    #[serde(default = "generate_command_id")]
    pub command_id: String,
    /// Channel ID (will be inferred from topic if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<u16>,
    /// Command type (required - use "C" for control, "A" for adjustment)
    pub command_type: CommandType,
    /// Point ID
    pub point_id: u32,
    /// Command value
    pub value: f64,
    /// Timestamp (current time if not provided)
    #[serde(default = "current_timestamp")]
    pub timestamp: i64,
    /// Optional metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Generate unique command ID
fn generate_command_id() -> String {
    format!("cmd_{}", chrono::Utc::now().timestamp_millis())
}

/// Get current timestamp
fn current_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Command status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStatus {
    pub command_id: String,
    pub status: String, // pending, executing, success, failed
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub timestamp: i64,
}

/// Command trigger mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerMode {
    /// 监听Redis pub/sub频道（旧方式）
    PubSub,
    /// 监听Redis List，使用BLPOP阻塞等待（推荐）
    ListQueue,
}

impl Default for TriggerMode {
    fn default() -> Self {
        Self::ListQueue // 默认使用List队列
    }
}

/// Command trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandTriggerConfig {
    pub channel_id: u16,
    pub redis_url: String,
    /// 触发模式
    #[serde(default)]
    pub mode: TriggerMode,
    /// BLPOP的超时时间（秒），0表示永久阻塞
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    1 // 默认1秒超时，便于检查停止信号
}

/// Command trigger - listenRedis命令并triggerprotocolexecuting
#[derive(Debug)]
pub struct CommandTrigger {
    config: CommandTriggerConfig,
    redis_client: Arc<Mutex<RedisClient>>,
    command_tx: mpsc::Sender<ChannelCommand>,
    is_running: Arc<RwLock<bool>>,
    task_handle: Option<JoinHandle<()>>,
}

impl CommandTrigger {
    /// Create new command trigger
    pub async fn new(
        config: CommandTriggerConfig,
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

    /// Start subscription
    pub async fn start(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                warn!(
                    "Command trigger already running for channel {}",
                    self.config.channel_id
                );
                return Ok(());
            }
            *running = true;
        }

        let channel_id = self.config.channel_id;
        let mode = self.config.mode.clone();

        info!(
            "Starting command trigger for channel {} in {:?} mode",
            channel_id, mode
        );

        // Clone necessary objects for task
        let redis_client = self.redis_client.clone();
        let command_tx = self.command_tx.clone();
        let is_running = self.is_running.clone();
        let config = self.config.clone();

        // Start appropriate subscription task based on mode
        let task_handle = tokio::spawn(async move {
            let result = match mode {
                TriggerMode::PubSub => {
                    let control_channel = format!("cmd:{}:control", channel_id);
                    let adjustment_channel = format!("cmd:{}:adjustment", channel_id);
                    Self::pubsub_loop(
                        redis_client,
                        command_tx,
                        is_running,
                        channel_id,
                        vec![control_channel, adjustment_channel],
                    )
                    .await
                },
                TriggerMode::ListQueue => {
                    Self::list_queue_loop(redis_client, command_tx, is_running, config).await
                },
            };

            if let Err(e) = result {
                error!("Command trigger error for channel {}: {}", channel_id, e);
            }
        });

        self.task_handle = Some(task_handle);
        Ok(())
    }

    /// Stop subscription
    pub async fn stop(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Wait for task to finish
        if let Some(handle) = self.task_handle.take() {
            // Give the task some time to exit gracefully
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => info!(
                    "Command trigger stopped for channel {}",
                    self.config.channel_id
                ),
                Ok(Err(e)) => warn!("Command trigger task error: {}", e),
                Err(_) => warn!("Command trigger task timeout, forcing stop"),
            }
        }

        Ok(())
    }

    /// PubSub subscription loop (旧方式)
    async fn pubsub_loop(
        redis_client: Arc<Mutex<RedisClient>>,
        command_tx: mpsc::Sender<ChannelCommand>,
        is_running: Arc<RwLock<bool>>,
        channel_id: u16,
        channels: Vec<String>,
    ) -> Result<()> {
        // Create subscription
        let channel_refs: Vec<&str> = channels.iter().map(std::string::String::as_str).collect();
        let mut redis_client = redis_client.lock().await;
        let mut pubsub = redis_client.subscribe(&channel_refs).await.map_err(|e| {
            crate::error::ComSrvError::InternalError(format!("Failed to create subscription: {e}"))
        })?;

        info!(
            "Command subscription established for channel {}",
            channel_id
        );

        loop {
            // Check if should stop
            if !*is_running.read().await {
                info!("Stopping command subscription for channel {}", channel_id);
                break;
            }

            // Receive message (with timeout)
            match tokio::time::timeout(
                std::time::Duration::from_secs(1),
                pubsub.on_message().next(),
            )
            .await
            {
                Ok(Some(msg)) => {
                    // Process message
                    if let Err(e) = Self::process_message(&command_tx, channel_id, msg).await {
                        error!("Failed to process command message: {}", e);
                    }
                },
                Ok(None) => {
                    warn!("Subscription closed for channel {}", channel_id);
                    break;
                },
                Err(_) => {
                    // Timeout, continue loop
                    continue;
                },
            }
        }

        Ok(())
    }

    /// List queue loop (新方式 - 使用BLPOP阻塞等待)
    async fn list_queue_loop(
        redis_client: Arc<Mutex<RedisClient>>,
        command_tx: mpsc::Sender<ChannelCommand>,
        is_running: Arc<RwLock<bool>>,
        config: CommandTriggerConfig,
    ) -> Result<()> {
        let channel_id = config.channel_id;
        let timeout = config.timeout_seconds;

        // 定义要监听的队列
        let control_queue = format!("comsrv:trigger:{}:C", channel_id);
        let adjustment_queue = format!("comsrv:trigger:{}:A", channel_id);

        info!(
            "Command trigger listening on queues: {} and {} (BLPOP with {}s timeout)",
            control_queue, adjustment_queue, timeout
        );

        let mut redis_client = redis_client.lock().await;

        loop {
            // Check if should stop
            if !*is_running.read().await {
                info!("Stopping command trigger for channel {}", channel_id);
                break;
            }

            // 使用BLPOP阻塞等待命令
            // 这会同时监听两个队列，返回第一个有数据的队列
            let queues = vec![control_queue.as_str(), adjustment_queue.as_str()];

            // 使用BLPOP阻塞等待命令
            match redis_client.blpop(&queues, timeout as usize).await {
                Ok(Some((queue, data))) => {
                    // 判断命令类型
                    let is_control = queue.ends_with(":C");
                    let command_type = if is_control { "Control" } else { "Adjustment" };

                    // 解析命令数据
                    match serde_json::from_str::<serde_json::Value>(&data) {
                        Ok(cmd_data) => {
                            let point_id = cmd_data["point_id"].as_u64().unwrap_or(0) as u32;
                            let value = cmd_data["value"].as_f64().unwrap_or(0.0);
                            let source = cmd_data["source"].as_str().unwrap_or("");
                            let command_id = cmd_data["command_id"]
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| {
                                    format!("auto_{}", chrono::Utc::now().timestamp_millis())
                                });

                            if source.is_empty() {
                                info!(
                                    "🎯 {} command received: channel={}, point={}, value={}, cmd_id={}",
                                    command_type, channel_id, point_id, value, command_id
                                );
                            } else {
                                info!(
                                    "🎯 {} command received from {}: channel={}, point={}, value={}, cmd_id={}",
                                    command_type, source, channel_id, point_id, value, command_id
                                );
                            }

                            // 创建ChannelCommand
                            let channel_command = if is_control {
                                ChannelCommand::Control {
                                    command_id,
                                    point_id,
                                    value,
                                    timestamp: chrono::Utc::now().timestamp(),
                                }
                            } else {
                                ChannelCommand::Adjustment {
                                    command_id,
                                    point_id,
                                    value,
                                    timestamp: chrono::Utc::now().timestamp(),
                                }
                            };

                            // 发送到协议处理器
                            if let Err(e) = command_tx.send(channel_command).await {
                                error!("Failed to send command to protocol handler: {}", e);
                                // 如果通道关闭，退出循环
                                break;
                            } else {
                                info!(
                                    "✅ {} command forwarded to protocol handler: point={}, value={}",
                                    command_type, point_id, value
                                );
                            }
                        },
                        Err(e) => {
                            warn!("Failed to parse command data: {}, raw data: {}", e, data);
                        },
                    }
                },
                Ok(None) => {
                    // 超时，继续循环（这让我们可以检查停止信号）
                    continue;
                },
                Err(e) => {
                    error!("BLPOP error: {}", e);
                    // 短暂休眠后重试
                    tokio::time::sleep(Duration::from_secs(1)).await;
                },
            }
        }

        Ok(())
    }

    /// Process single message
    async fn process_message(
        command_tx: &mpsc::Sender<ChannelCommand>,
        channel_id: u16,
        msg: voltage_libs::redis::Msg,
    ) -> Result<()> {
        // Get message content
        let payload: String = msg.get_payload().map_err(|e| {
            crate::error::ComSrvError::InternalError(format!("Failed to get message payload: {e}"))
        })?;

        debug!(
            "Received command message on channel {}: {}",
            channel_id, payload
        );

        // Parse command
        let mut command: ControlCommand = serde_json::from_str(&payload).map_err(|e| {
            crate::error::ComSrvError::ParsingError(format!("Failed to parse command: {e}"))
        })?;

        // Infer channel_id if not provided (use the one from subscription)
        if command.channel_id.is_none() {
            command.channel_id = Some(channel_id);
        }

        // Ensure command is sent to correct channel
        let cmd_channel_id = command.channel_id.unwrap_or(channel_id);
        if cmd_channel_id != channel_id {
            warn!(
                "Received command for wrong channel: expected {}, got {}",
                channel_id, cmd_channel_id
            );
            return Ok(());
        }

        // Convert to ChannelCommand and send
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

        // Send command to protocol processor
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

        let command: ControlCommand =
            serde_json::from_str(json).expect("test JSON should be valid");
        assert_eq!(command.command_id, "test-123");
        assert_eq!(command.channel_id, Some(1));
        assert!(matches!(command.command_type, CommandType::Control));
        assert_eq!(command.point_id, 1001);
        assert!((command.value - 1.0).abs() < f64::EPSILON);
    }
}
