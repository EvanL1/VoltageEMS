//! Control command subscriber
//!
//! Responsible for subscribing to control commands from Redis and distributing them to corresponding channels for processing

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{debug, error, info, warn};
use voltage_libs::redis::RedisClient;

use super::traits::ChannelCommand;
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
    /// Listen to Redis pub/sub channel (old method) (监听Redis pub/sub频道，旧方式)
    PubSub,
    /// Listen to Redis List, use BLPOP blocking wait (recommended) (监听Redis List，使用BLPOP阻塞等待，推荐)
    ListQueue,
}

impl Default for TriggerMode {
    fn default() -> Self {
        Self::ListQueue // Use List queue by default (默认使用List队列)
    }
}

/// Command trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandTriggerConfig {
    pub channel_id: u16,
    pub redis_url: String,
    /// Trigger mode (触发模式)
    #[serde(default)]
    pub mode: TriggerMode,
    /// BLPOP timeout in seconds, 0 means block forever (BLPOP的超时时间，秒，0表示永久阻塞)
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    30 // Use 30 second timeout to reduce idle loops (select! ensures timely response) (使用30秒超时，减少空转，有select!保证及时响应)
}

/// Command trigger - listen to Redis commands and trigger protocol execution
pub struct CommandTrigger {
    config: CommandTriggerConfig,
    command_tx: mpsc::Sender<ChannelCommand>,
    shutdown_tx: tokio::sync::watch::Sender<bool>, // false = running, true = shutdown
    _shutdown_rx_keepalive: tokio::sync::watch::Receiver<bool>, // Keep receiver alive (保持接收端活跃)
    task_handle: Option<JoinHandle<()>>,
}

impl CommandTrigger {
    /// Create new command trigger
    pub async fn new(
        config: CommandTriggerConfig,
        command_tx: mpsc::Sender<ChannelCommand>,
    ) -> Result<Self> {
        // Create watch channel, initial value false = not stopped (创建 watch channel，初始值 false = 未停止)
        let (tx, rx) = tokio::sync::watch::channel(false);

        Ok(Self {
            config,
            command_tx,
            shutdown_tx: tx,
            _shutdown_rx_keepalive: rx,
            task_handle: None,
        })
    }

    /// Start subscription
    pub async fn start(&mut self) -> Result<()> {
        // Use task_handle to check if already running (使用 task_handle 判断是否已经在运行)
        if self.task_handle.is_some() {
            warn!(
                "Command trigger already running for channel {}",
                self.config.channel_id
            );
            return Ok(());
        }

        let channel_id = self.config.channel_id;
        let mode = self.config.mode.clone();

        info!(
            "Starting command trigger for channel {} in {:?} mode",
            channel_id, mode
        );

        // Clone necessary objects for task
        let command_tx = self.command_tx.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();
        let task_redis_url = config.redis_url.clone();

        // Start appropriate subscription task based on mode
        let task_handle = tokio::spawn(async move {
            // Create independent Redis connection within task (在任务内创建独立的 Redis 连接)
            let redis_client = match RedisClient::new(&task_redis_url).await {
                Ok(client) => client,
                Err(e) => {
                    error!("Failed to create Redis client for trigger: {}", e);
                    return;
                },
            };

            let result = match mode {
                TriggerMode::PubSub => {
                    let control_channel = format!("cmd:{}:control", channel_id);
                    let adjustment_channel = format!("cmd:{}:adjustment", channel_id);
                    Self::pubsub_loop(
                        redis_client,
                        command_tx,
                        shutdown_rx,
                        channel_id,
                        vec![control_channel, adjustment_channel],
                    )
                    .await
                },
                TriggerMode::ListQueue => {
                    Self::list_queue_loop(redis_client, command_tx, shutdown_rx, config).await
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
        // Send stop signal (true = shutdown) (发送停止信号)
        let _ = self.shutdown_tx.send(true);

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

    /// PubSub subscription loop (old method) (旧方式)
    async fn pubsub_loop(
        mut redis_client: RedisClient,
        command_tx: mpsc::Sender<ChannelCommand>,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        channel_id: u16,
        channels: Vec<String>,
    ) -> Result<()> {
        // Create subscription - use independent connection, no lock needed (使用独立连接，无需锁)
        let channel_refs: Vec<&str> = channels.iter().map(std::string::String::as_str).collect();
        let mut pubsub = redis_client.subscribe(&channel_refs).await.map_err(|e| {
            crate::utils::error::ComSrvError::InternalError(format!(
                "Failed to create subscription: {e}"
            ))
        })?;

        info!(
            "Command subscription established for channel {}",
            channel_id
        );

        let mut message_stream = pubsub.on_message();

        loop {
            tokio::select! {
                // Listen for stop signal (监听停止信号)
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Stopping command subscription for channel {}", channel_id);
                        break;
                    }
                }
                // Receive message (接收消息)
                msg = message_stream.next() => {
                    match msg {
                        Some(msg) => {
                            // Process message
                            if let Err(e) = Self::process_message(&command_tx, channel_id, msg).await {
                                error!("Failed to process command message: {}", e);
                            }
                        },
                        None => {
                            warn!("Subscription closed for channel {}", channel_id);
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// List queue loop (new method - use BLPOP blocking wait) with reconnection (新方式 - 使用BLPOP阻塞等待)
    async fn list_queue_loop(
        redis_client: RedisClient,
        command_tx: mpsc::Sender<ChannelCommand>,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        config: CommandTriggerConfig,
    ) -> Result<()> {
        let channel_id = config.channel_id;
        let timeout = config.timeout_seconds;
        let redis_url = config.redis_url.clone();

        // Define queues to listen to (定义要监听的队列)
        let control_queue = format!("comsrv:trigger:{}:C", channel_id);
        let adjustment_queue = format!("comsrv:trigger:{}:A", channel_id);

        info!(
            "Command trigger listening on queues: {} and {} (BLPOP with {}s timeout)",
            control_queue, adjustment_queue, timeout
        );

        // Reconnection loop (重连循环)
        let mut redis_client = redis_client;
        let mut reconnect_delay = Duration::from_secs(1);
        let max_reconnect_delay = Duration::from_secs(30);

        loop {
            // Check stop signal (检查停止信号)
            if *shutdown_rx.borrow() {
                info!("Stopping command trigger for channel {}", channel_id);
                break;
            }

            // Use BLPOP to block and wait for commands (使用BLPOP阻塞等待命令)
            let queues = vec![control_queue.as_str(), adjustment_queue.as_str()];

            let inner_loop_result: Result<()> = async {
                loop {
                    tokio::select! {
                        // Listen for stop signal (监听停止信号)
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                return Ok(());
                            }
                        }
                        // BLPOP wait for commands (BLPOP 等待命令)
                        result = redis_client.blpop(&queues, timeout as usize) => {
                            match result {
                                Ok(Some((queue, data))) => {
                                    // Determine command type (判断命令类型)
                                    let is_control = queue.ends_with(":C");
                                    let command_type = if is_control { "Control" } else { "Adjustment" };

                                    // Deserialize to ControlCommand uniformly, add source info (统一反序列化为 ControlCommand，添加来源信息)
                                    match Self::parse_command_data(&data, Some(channel_id)) {
                                        Ok(mut command) => {
                                            // Add source to metadata (添加来源到 metadata)
                                            if let serde_json::Value::Object(ref mut map) = command.metadata {
                                                map.insert("trigger_source".to_string(), serde_json::Value::String("list_queue".to_string()));
                                            }
                                            let point_id = command.point_id;
                                            let value = command.value;
                                            let command_id = command.command_id.clone();

                                            info!(
                                                "🎯 [ListQueue] {} command: channel={}, point={}, value={}, cmd_id={}",
                                                command_type, channel_id, point_id, value, command_id
                                            );

                                            // Convert to ChannelCommand (转换为 ChannelCommand)
                                            let channel_command = Self::to_channel_command(command);

                                            // Send to protocol handler (发送到协议处理器)
                                            if let Err(e) = command_tx.send(channel_command).await {
                                                error!("Failed to send command to protocol handler: {}", e);
                                                // If channel closed, exit completely (如果通道关闭，完全退出)
                                                return Err(crate::utils::error::ComSrvError::InternalError(
                                                    "Command channel closed".to_string()
                                                ));
                                            }
                                        },
                                        Err(e) => {
                                            warn!("Failed to parse command data: {}, raw data: {}", e, data);
                                        },
                                    }
                                },
                                Ok(None) => {
                                    // Timeout, continue loop (超时，继续循环)
                                    continue;
                                },
                                Err(e) => {
                                    // Redis error, trigger reconnection (Redis 错误，触发重连)
                                    error!("BLPOP error, will reconnect: {}", e);
                                    return Err(crate::utils::error::ComSrvError::InternalError(
                                        format!("Redis error: {}", e)
                                    ));
                                },
                            }
                        }
                    }
                }
            }.await;

            // Process inner loop result (处理内层循环结果)
            match inner_loop_result {
                Ok(()) => {
                    // Normal exit (received stop signal) (正常退出，收到停止信号)
                    break;
                },
                Err(e) => {
                    // Error, attempt reconnection (错误，尝试重连)
                    warn!(
                        "List queue loop error for channel {}: {}, attempting reconnect",
                        channel_id, e
                    );

                    // Wait before reconnecting (等待一段时间后重连)
                    tokio::select! {
                        _ = tokio::time::sleep(reconnect_delay) => {
                            // Attempt to recreate Redis connection (尝试重新创建 Redis 连接)
                            match RedisClient::new(&redis_url).await {
                                Ok(new_client) => {
                                    redis_client = new_client;
                                    info!("Reconnected to Redis for channel {}", channel_id);
                                    // Reset delay (重置延迟)
                                    reconnect_delay = Duration::from_secs(1);
                                },
                                Err(e) => {
                                    error!("Failed to reconnect to Redis: {}", e);
                                    // Exponential backoff (指数退避)
                                    reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay);
                                }
                            }
                        }
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                info!("Stopping during reconnect for channel {}", channel_id);
                                break;
                            }
                        }
                    }
                },
            }
        }

        Ok(())
    }

    /// Parse command data (unified deserialization logic) (解析命令数据，统一的反序列化逻辑)
    fn parse_command_data(data: &str, channel_id: Option<u16>) -> Result<ControlCommand> {
        let mut command: ControlCommand = serde_json::from_str(data).map_err(|e| {
            crate::utils::error::ComSrvError::ParsingError(format!("Failed to parse command: {e}"))
        })?;

        // If channel_id not provided, use the passed default value (如果没有提供 channel_id，使用传入的默认值)
        if command.channel_id.is_none() {
            command.channel_id = channel_id;
        }

        Ok(command)
    }

    /// Convert to ChannelCommand (转换为 ChannelCommand)
    fn to_channel_command(command: ControlCommand) -> ChannelCommand {
        match command.command_type {
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
        }
    }

    /// Process single message
    async fn process_message(
        command_tx: &mpsc::Sender<ChannelCommand>,
        channel_id: u16,
        msg: voltage_libs::redis::Msg,
    ) -> Result<()> {
        // Get message content
        let payload: String = msg.get_payload().map_err(|e| {
            crate::utils::error::ComSrvError::InternalError(format!(
                "Failed to get message payload: {e}"
            ))
        })?;

        debug!(
            "[PubSub] Received command message on channel {}: {}",
            channel_id, payload
        );

        // Use unified parsing logic (使用统一的解析逻辑)
        let mut command = Self::parse_command_data(&payload, Some(channel_id))?;

        // Add source to metadata (添加来源到 metadata)
        if let serde_json::Value::Object(ref mut map) = command.metadata {
            map.insert(
                "trigger_source".to_string(),
                serde_json::Value::String("pubsub".to_string()),
            );
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

        // Use unified conversion logic (使用统一的转换逻辑)
        let channel_command = Self::to_channel_command(command);

        // Send command to protocol processor
        if let Err(e) = command_tx.send(channel_command).await {
            error!("Failed to send command to protocol handler: {}", e);
            return Err(crate::utils::error::ComSrvError::InternalError(
                "Command channel closed".to_string(),
            ));
        }

        debug!("Command forwarded to protocol handler");
        Ok(())
    }
}

impl Drop for CommandTrigger {
    fn drop(&mut self) {
        // Send stop signal (发送停止信号)
        let _ = self.shutdown_tx.send(true);

        // If task still running, abort it (fallback) (如果任务还在运行，abort 它，兜底)
        if let Some(handle) = self.task_handle.take() {
            if !handle.is_finished() {
                warn!(
                    "CommandTrigger for channel {} dropped with running task, aborting",
                    self.config.channel_id
                );
                handle.abort();
            }
        }
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
