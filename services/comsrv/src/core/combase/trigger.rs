//! Control command subscriber
//!
//! Responsible for subscribing to control commands from Redis and distributing them to corresponding channels for processing

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
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

/// Command trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandTriggerConfig {
    pub channel_id: u16,
    pub redis_url: String,
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

        // Subscribe to control and adjustment command channels
        let control_channel = format!("cmd:{}:control", self.config.channel_id);
        let adjustment_channel = format!("cmd:{}:adjustment", self.config.channel_id);

        info!(
            "Starting command trigger for channel {}, subscribing to: {} and {}",
            self.config.channel_id, control_channel, adjustment_channel
        );

        // Clone necessary objects for task
        let redis_client = self.redis_client.clone();
        let command_tx = self.command_tx.clone();
        let is_running = self.is_running.clone();
        let channel_id = self.config.channel_id;

        // Start subscription task
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

    /// Subscription loop
    async fn subscription_loop(
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
