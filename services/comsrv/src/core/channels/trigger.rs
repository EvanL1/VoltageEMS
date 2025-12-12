//! Control command subscriber
//!
//! Responsible for subscribing to control commands from Redis and distributing them to corresponding channels for processing

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use voltage_config::common::timeouts;
use voltage_config::comsrv::ChannelRedisKeys;
use voltage_rtdb::Rtdb;

use super::traits::ChannelCommand;
use crate::error::Result;

/// Control command type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    #[serde(rename = "control", alias = "C")]
    Control,
    #[serde(rename = "adjustment", alias = "A")]
    Adjustment,
}

/// Compact trigger message (minimal format for TODO queue)
/// Contains only the core fields needed for command execution
#[derive(Debug, Clone, Deserialize)]
struct CompactTrigger {
    /// Point ID
    point_id: u32,
    /// Command value
    value: f64,
    /// Timestamp in milliseconds
    timestamp: i64,
}

/// Control command message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    /// Command ID (auto-generated if not provided)
    #[serde(default = "generate_command_id")]
    pub command_id: String,
    /// Channel ID (will be inferred from topic if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<u32>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTriggerConfig {
    pub channel_id: u32,
    /// BLPOP timeout in seconds; 0 means block forever.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

impl Default for CommandTriggerConfig {
    fn default() -> Self {
        Self {
            channel_id: 0,
            timeout_seconds: default_timeout(), // Use the same default function
        }
    }
}

fn default_timeout() -> u64 {
    1 // Use a 1 second timeout to reduce connection pool contention; select! ensures timely response.
}

/// Command trigger - listen to RTDB commands and trigger protocol execution
pub struct CommandTrigger {
    config: CommandTriggerConfig,
    command_tx: mpsc::Sender<ChannelCommand>,
    shutdown_tx: tokio::sync::watch::Sender<bool>, // false = running, true = shutdown
    _shutdown_rx_keepalive: tokio::sync::watch::Receiver<bool>, // Keep receiver alive.
    task_handle: Option<JoinHandle<()>>,
    rtdb: Arc<dyn Rtdb>,
    /// Timestamp deduplication: tracks last executed timestamp for each point
    last_ts_map: Arc<DashMap<u32, i64>>,
}

impl CommandTrigger {
    /// Create new command trigger
    pub async fn new(
        config: CommandTriggerConfig,
        command_tx: mpsc::Sender<ChannelCommand>,
        rtdb: Arc<dyn Rtdb>,
    ) -> Result<Self> {
        // Create watch channel; the initial value false means not stopped.
        let (tx, rx) = tokio::sync::watch::channel(false);

        Ok(Self {
            config,
            command_tx,
            shutdown_tx: tx,
            _shutdown_rx_keepalive: rx,
            task_handle: None,
            rtdb,
            last_ts_map: Arc::new(DashMap::new()),
        })
    }

    /// Start subscription
    pub async fn start(&mut self) -> Result<()> {
        // Use task_handle to check if it is already running.
        if self.task_handle.is_some() {
            warn!("Ch{} trigger running", self.config.channel_id);
            return Ok(());
        }

        let channel_id = self.config.channel_id;

        debug!("Ch{} trigger starting", channel_id);

        // Clone necessary objects for task
        let command_tx = self.command_tx.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();
        let rtdb = self.rtdb.clone();
        let last_ts_map = self.last_ts_map.clone();

        // Start ListQueue subscription task
        let task_handle = tokio::spawn(async move {
            let result =
                Self::list_queue_loop(rtdb.clone(), command_tx, shutdown_rx, config, last_ts_map)
                    .await;

            if let Err(e) = result {
                error!("Ch{} trigger err: {}", channel_id, e);
            }
        });

        self.task_handle = Some(task_handle);
        Ok(())
    }

    /// Stop subscription
    pub async fn stop(&mut self) -> Result<()> {
        // Send stop signal (true = shutdown).
        let _ = self.shutdown_tx.send(true);

        // Wait for task to finish
        if let Some(handle) = self.task_handle.take() {
            // Give the task some time to exit gracefully
            match tokio::time::timeout(timeouts::SHUTDOWN_TIMEOUT, handle).await {
                Ok(Ok(())) => debug!("Ch{} trigger stopped", self.config.channel_id),
                Ok(Err(e)) => warn!("Trigger task err: {}", e),
                Err(_) => warn!("Trigger timeout, force stop"),
            }
        }

        Ok(())
    }

    /// List queue loop using a BLPOP blocking wait with reconnection logic.
    async fn list_queue_loop(
        rtdb: Arc<dyn Rtdb>,
        command_tx: mpsc::Sender<ChannelCommand>,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        config: CommandTriggerConfig,
        last_ts_map: Arc<DashMap<u32, i64>>,
    ) -> Result<()> {
        let channel_id = config.channel_id;
        let timeout = config.timeout_seconds;

        // Define the queues to listen to.
        let control_queue = ChannelRedisKeys::control_todo(channel_id);
        let adjustment_queue = ChannelRedisKeys::adjustment_todo(channel_id);

        info!("Ch{} queues: C/A todo ({}s)", channel_id, timeout);

        // Reconnection loop with failure tracking.
        let mut reconnect_delay = timeouts::MIN_RECONNECT_DELAY;
        let max_reconnect_delay = timeouts::MAX_RECONNECT_DELAY;
        let mut consecutive_failures = 0u32;

        loop {
            // Check the stop signal.
            if *shutdown_rx.borrow() {
                debug!("Ch{} trigger stopping", channel_id);
                break;
            }

            // Use BLPOP to block and wait for commands.
            let queues = vec![control_queue.as_str(), adjustment_queue.as_str()];

            let inner_loop_result: Result<()> = async {
                loop {
                    tokio::select! {
                        // Listen for the stop signal.
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                return Ok(());
                            }
                        }
                        // Use BLPOP to wait for commands.
                        result = rtdb.list_blpop(&queues, timeout) => {
                            match result {
                                Ok(Some((queue, data_bytes))) => {
                                    let data = String::from_utf8(data_bytes.to_vec()).map_err(|e| {
                                        crate::error::ComSrvError::ParsingError(
                                            format!("Failed to parse UTF-8: {}", e)
                                        )
                                    })?;
                                    // Determine the command type.
                                    let is_control = queue.contains(":C:");
                                    let point_type_str = if is_control { "C" } else { "A" };

                                    // ★ Try parsing as compact trigger (new format: point_id, value, timestamp)
                                    let compact_trigger = serde_json::from_str::<CompactTrigger>(&data);

                                    let (point_id, value, current_ts) = match compact_trigger {
                                        Ok(trigger) => {
                                            // New compact format parsed successfully
                                            (trigger.point_id, trigger.value, trigger.timestamp)
                                        }
                                        Err(compact_err) => {
                                            // Fallback: try parsing legacy format (only point_id)
                                            debug!("Compact parse fail, trying legacy: {}", compact_err);

                                            let legacy_trigger: serde_json::Value = match serde_json::from_str(&data) {
                                                Ok(v) => v,
                                                Err(e) => {
                                                    error!("Parse err q={}: {}", queue, e);
                                                    continue;
                                                }
                                            };

                                            let point_id: u32 = match legacy_trigger.get("point_id").and_then(|v| v.as_u64()) {
                                                Some(id) => id as u32,
                                                None => {
                                                    error!("No point_id q={}", queue);
                                                    continue;
                                                }
                                            };

                                            // For legacy format, read value and timestamp from Hash
                                            let channel_key = format!("comsrv:{}:{}", channel_id, point_type_str);
                                            let ts_field = format!("{}:ts", point_id);

                                            // Read timestamp from Hash
                                            let current_ts: i64 = match rtdb.hash_get(&channel_key, &ts_field).await {
                                                Ok(Some(ts_bytes)) => {
                                                    let ts_str = String::from_utf8(ts_bytes.to_vec()).map_err(|e| {
                                                        crate::error::ComSrvError::ParsingError(
                                                            format!("Failed to parse UTF-8 timestamp: {}", e)
                                                        )
                                                    })?;
                                                    match ts_str.parse() {
                                                        Ok(ts) => ts,
                                                        Err(e) => {
                                                            error!("Ch{} pt{} ts parse err: {}", channel_id, point_id, e);
                                                            continue;
                                                        }
                                                    }
                                                }
                                                Ok(None) => {
                                                    0  // Treat as new command if timestamp missing
                                                }
                                                Err(e) => {
                                                    error!("Ch{} pt{} ts read err: {}", channel_id, point_id, e);
                                                    continue;
                                                }
                                            };

                                            // Read value from Hash (legacy format compatibility)
                                            let value: f64 = match rtdb.hash_get(&channel_key, &point_id.to_string()).await {
                                                Ok(Some(value_bytes)) => {
                                                    let value_str = String::from_utf8(value_bytes.to_vec()).map_err(|e| {
                                                        crate::error::ComSrvError::ParsingError(
                                                            format!("Failed to parse UTF-8 value: {}", e)
                                                        )
                                                    })?;
                                                    match value_str.parse() {
                                                        Ok(v) => v,
                                                        Err(e) => {
                                                            error!("Ch{} pt{} val parse err: {}", channel_id, point_id, e);
                                                            continue;
                                                        }
                                                    }
                                                }
                                                Ok(None) => {
                                                    error!("Ch{} pt{} no value", channel_id, point_id);
                                                    continue;
                                                }
                                                Err(e) => {
                                                    error!("Ch{} pt{} val read err: {}", channel_id, point_id, e);
                                                    continue;
                                                }
                                            };

                                            debug!("Legacy trigger Ch{} pt{}", channel_id, point_id);

                                            (point_id, value, current_ts)
                                        }
                                    };

                                    // ★ Timestamp deduplication check
                                    let last_ts = last_ts_map.get(&point_id).map(|v| *v).unwrap_or(0);
                                    if current_ts <= last_ts {
                                        debug!("Skip pt{}: ts={} (same)", point_id, current_ts);
                                        continue;
                                    }

                                    // ★ Timestamp changed - execute command
                                    debug!("Exec pt{}: val={} ts={}", point_id, value, current_ts);

                                    // Update last_ts
                                    last_ts_map.insert(point_id, current_ts);

                                    // Build metadata without json! macro to avoid clippy warnings
                                    let mut metadata = serde_json::Map::new();
                                    metadata.insert("trigger_source".to_string(), serde_json::Value::String("list_queue".to_string()));
                                    metadata.insert("timestamp_ms".to_string(), serde_json::Value::Number(current_ts.into()));

                                    // Build ControlCommand
                                    let command = ControlCommand {
                                        command_id: format!("trigger_{}_{}", channel_id, current_ts),
                                        channel_id: Some(channel_id),
                                        command_type: if is_control { CommandType::Control } else { CommandType::Adjustment },
                                        point_id,
                                        value,
                                        timestamp: current_ts / 1000,  // Convert ms to seconds
                                        metadata: serde_json::Value::Object(metadata),
                                    };

                                    // Convert to ChannelCommand and send
                                    let channel_command = Self::to_channel_command(command);
                                    if let Err(e) = command_tx.send(channel_command).await {
                                        error!("Cmd send err: {}", e);
                                        return Err(crate::error::ComSrvError::InternalError(
                                            "Command channel closed".to_string()
                                        ));
                                    }
                                },
                                Ok(None) => {
                                    // Timeout; continue the loop.
                                    continue;
                                },
                                Err(e) => {
                                    // Redis error; classify and trigger reconnection.
                                    let error_type = classify_redis_error(&e);
                                    error!("Ch{} BLPOP err: {} ({:?})", channel_id, e, error_type);
                                    return Err(crate::error::ComSrvError::InternalError(
                                        format!("Redis {:?} Ch{}: {}", error_type, channel_id, e)
                                    ));
                                },
                            }
                        }
                    }
                }
            }.await;

            // Process the inner loop result.
            match inner_loop_result {
                Ok(()) => {
                    // Normal exit after receiving the stop signal.
                    break;
                },
                Err(e) => {
                    // Error; attempt reconnection with failure tracking.
                    consecutive_failures += 1;
                    warn!(
                        "Ch{} err #{}: {}, retry {:?}",
                        channel_id, consecutive_failures, e, reconnect_delay
                    );

                    // Log critical if too many consecutive failures
                    if consecutive_failures >= 10 {
                        error!(
                            "CRITICAL: Ch{} {}x failures",
                            channel_id, consecutive_failures
                        );
                    }

                    // Wait before retrying with exponential backoff.
                    tokio::select! {
                        _ = tokio::time::sleep(reconnect_delay) => {
                            reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay);
                        }
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                debug!("Ch{} backoff stop", channel_id);
                                break;
                            }
                        }
                    }
                },
            }
        }

        Ok(())
    }

    /// Parse command data with unified deserialization logic.
    /// If `command_type` missing, use `fallback_type` inferred from queue.
    #[allow(dead_code)] // Used in tests, reserved for future queue format migration
    fn parse_command_data(
        data: &str,
        channel_id: Option<u32>,
        fallback_type: Option<CommandType>,
    ) -> Result<ControlCommand> {
        // First, try strict deserialization
        if let Ok(mut command) = serde_json::from_str::<ControlCommand>(data) {
            if command.channel_id.is_none() {
                command.channel_id = channel_id;
            }
            return Ok(command);
        }

        // Fallback: attempt to augment JSON with missing fields
        let mut value: serde_json::Value = serde_json::from_str(data).map_err(|e| {
            crate::error::ComSrvError::ParsingError(format!("Failed to parse command JSON: {e}"))
        })?;

        if let serde_json::Value::Object(ref mut map) = value {
            // Fill channel_id if absent
            if !map.contains_key("channel_id") {
                if let Some(cid) = channel_id {
                    map.insert(
                        "channel_id".to_string(),
                        serde_json::Value::Number(cid.into()),
                    );
                }
            }

            // Inject command_type from fallback if absent
            if !map.contains_key("command_type") {
                if let Some(fb) = &fallback_type {
                    let ct = match fb {
                        CommandType::Control => serde_json::Value::String("C".to_string()),
                        CommandType::Adjustment => serde_json::Value::String("A".to_string()),
                    };
                    map.insert("command_type".to_string(), ct);
                }
            }

            // Try final deserialization
            let mut command: ControlCommand = serde_json::from_value(value).map_err(|e| {
                crate::error::ComSrvError::ParsingError(format!(
                    "Failed to parse command after fallback: {e}"
                ))
            })?;

            if command.channel_id.is_none() {
                command.channel_id = channel_id;
            }
            return Ok(command);
        }

        Err(crate::error::ComSrvError::ParsingError(
            "Command JSON is not an object".to_string(),
        ))
    }

    /// Convert to ChannelCommand.
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

    // process_message removed - only using ListQueue mode
}

/// Classify Redis error types for better debugging
fn classify_redis_error(error: &dyn std::fmt::Display) -> &'static str {
    let error_str = error.to_string().to_lowercase();

    if error_str.contains("timeout") {
        "timeout"
    } else if error_str.contains("connection") || error_str.contains("refused") {
        "connection"
    } else if error_str.contains("network") || error_str.contains("broken pipe") {
        "network"
    } else if error_str.contains("parse") || error_str.contains("deserialize") {
        "serialization"
    } else if error_str.contains("auth") || error_str.contains("permission") {
        "authentication"
    } else {
        "unknown"
    }
}

impl Drop for CommandTrigger {
    fn drop(&mut self) {
        // Send stop signal.
        let _ = self.shutdown_tx.send(true);

        // If the task is still running, abort it as a fallback.
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
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
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

    // ========================================================================
    // CommandType Tests
    // ========================================================================

    #[test]
    fn test_command_type_control() {
        let cmd_type = CommandType::Control;
        assert!(matches!(cmd_type, CommandType::Control));
    }

    #[test]
    fn test_command_type_adjustment() {
        let cmd_type = CommandType::Adjustment;
        assert!(matches!(cmd_type, CommandType::Adjustment));
    }

    #[test]
    fn test_command_type_serialization() {
        // Control serializes to "control"
        let control_json = serde_json::to_string(&CommandType::Control).unwrap();
        assert_eq!(control_json, "\"control\"");

        // Adjustment serializes to "adjustment"
        let adjustment_json = serde_json::to_string(&CommandType::Adjustment).unwrap();
        assert_eq!(adjustment_json, "\"adjustment\"");
    }

    #[test]
    fn test_command_type_deserialization_full_names() {
        // Deserialize from full names
        let control: CommandType = serde_json::from_str("\"control\"").unwrap();
        assert!(matches!(control, CommandType::Control));

        let adjustment: CommandType = serde_json::from_str("\"adjustment\"").unwrap();
        assert!(matches!(adjustment, CommandType::Adjustment));
    }

    #[test]
    fn test_command_type_deserialization_aliases() {
        // Deserialize from aliases
        let control: CommandType = serde_json::from_str("\"C\"").unwrap();
        assert!(matches!(control, CommandType::Control));

        let adjustment: CommandType = serde_json::from_str("\"A\"").unwrap();
        assert!(matches!(adjustment, CommandType::Adjustment));
    }

    // ========================================================================
    // ControlCommand Tests
    // ========================================================================

    #[test]
    fn test_control_command_default_command_id() {
        let json = r#"{
            "command_type": "control",
            "point_id": 100,
            "value": 1.5
        }"#;

        let command: ControlCommand = serde_json::from_str(json).unwrap();

        // command_id should be auto-generated
        assert!(command.command_id.starts_with("cmd_"));
        assert_eq!(command.point_id, 100);
        assert!((command.value - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_control_command_default_timestamp() {
        let json = r#"{
            "command_type": "control",
            "point_id": 200,
            "value": 2.5
        }"#;

        let command: ControlCommand = serde_json::from_str(json).unwrap();

        // timestamp should be auto-generated
        let now = chrono::Utc::now().timestamp();
        assert!(command.timestamp > 0);
        assert!((command.timestamp - now).abs() < 5); // Within 5 seconds
    }

    #[test]
    fn test_control_command_without_channel_id() {
        let json = r#"{
            "command_type": "adjustment",
            "point_id": 300,
            "value": 3.14
        }"#;

        let command: ControlCommand = serde_json::from_str(json).unwrap();

        // channel_id is optional
        assert!(command.channel_id.is_none());
        assert!(matches!(command.command_type, CommandType::Adjustment));
    }

    #[test]
    fn test_control_command_with_metadata() {
        let json = r#"{
            "command_type": "control",
            "point_id": 400,
            "value": 4.2,
            "metadata": {
                "source": "test",
                "priority": "high"
            }
        }"#;

        let command: ControlCommand = serde_json::from_str(json).unwrap();

        assert!(!command.metadata.is_null());
        assert_eq!(command.metadata["source"], "test");
        assert_eq!(command.metadata["priority"], "high");
    }

    #[test]
    fn test_control_command_metadata_defaults_to_null() {
        let json = r#"{
            "command_type": "control",
            "point_id": 500,
            "value": 5.5
        }"#;

        let command: ControlCommand = serde_json::from_str(json).unwrap();

        // metadata should default to null/empty
        assert!(command.metadata.is_null() || command.metadata.is_object());
    }

    // ========================================================================
    // Helper Function Tests
    // ========================================================================

    #[test]
    fn test_generate_command_id_format() {
        let id1 = generate_command_id();

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(2));

        let id2 = generate_command_id();

        // Should start with "cmd_"
        assert!(id1.starts_with("cmd_"));
        assert!(id2.starts_with("cmd_"));

        // Should be unique (different timestamps)
        assert_ne!(id1, id2);

        // Should contain timestamp after prefix
        let timestamp_str = &id1[4..];
        assert!(timestamp_str.parse::<i64>().is_ok());
    }

    #[test]
    fn test_current_timestamp_returns_valid_time() {
        let ts = current_timestamp();
        let now = chrono::Utc::now().timestamp();

        // Should be recent timestamp
        assert!(ts > 0);
        assert!((ts - now).abs() < 2); // Within 2 seconds
    }

    #[test]
    fn test_default_timeout_value() {
        // Default BLPOP timeout reduced to 1s to avoid long-held connections
        assert_eq!(default_timeout(), 1);
    }

    #[test]
    fn test_classify_redis_error_connection_refused() {
        // classify_redis_error accepts &dyn Display, so we can use simple strings
        let err = "connection refused";
        let classification = classify_redis_error(&err);
        assert_eq!(classification, "connection");
    }

    #[test]
    fn test_classify_redis_error_timeout() {
        let err = "operation timeout";
        let classification = classify_redis_error(&err);
        assert_eq!(classification, "timeout");
    }

    #[test]
    fn test_classify_redis_error_network() {
        let err = "network error";
        let classification = classify_redis_error(&err);
        assert_eq!(classification, "network");
    }

    #[test]
    fn test_classify_redis_error_parse() {
        let err = "parse error";
        let classification = classify_redis_error(&err);
        assert_eq!(classification, "serialization");
    }

    #[test]
    fn test_classify_redis_error_unknown() {
        let err = "some error";
        let classification = classify_redis_error(&err);
        assert_eq!(classification, "unknown");
    }

    // ========================================================================
    // CommandTrigger Tests
    // ========================================================================

    #[test]
    fn test_parse_command_data_valid_json() {
        let data = r#"{"command_type":"control","point_id":1,"value":10.5}"#;
        let result = CommandTrigger::parse_command_data(data, Some(1001), None);

        assert!(result.is_ok());
        let command = result.unwrap();
        assert_eq!(command.channel_id, Some(1001));
        assert_eq!(command.point_id, 1);
        assert!((command.value - 10.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_command_data_invalid_json() {
        let data = "invalid json {";
        let result = CommandTrigger::parse_command_data(data, Some(1001), None);

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_command_data_channel_id_inference() {
        let data = r#"{"command_type":"adjustment","point_id":2,"value":20.0}"#;
        let result = CommandTrigger::parse_command_data(data, Some(2001), None);

        assert!(result.is_ok());
        let command = result.unwrap();
        // channel_id should be inferred from function parameter
        assert_eq!(command.channel_id, Some(2001));
    }

    #[test]
    fn test_to_channel_command_control_type() {
        let ctrl_cmd = ControlCommand {
            command_id: "test-control".to_string(),
            channel_id: Some(1001),
            command_type: CommandType::Control,
            point_id: 10,
            value: 1.0,
            timestamp: 1234567890,
            metadata: serde_json::Value::Null,
        };

        let channel_cmd = CommandTrigger::to_channel_command(ctrl_cmd.clone());

        // ChannelCommand is an enum, use pattern matching
        match channel_cmd {
            ChannelCommand::Control {
                point_id, value, ..
            } => {
                assert_eq!(point_id, 10);
                assert!((value - 1.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Control variant"),
        }
    }

    #[test]
    fn test_to_channel_command_adjustment_type() {
        let adj_cmd = ControlCommand {
            command_id: "test-adjustment".to_string(),
            channel_id: Some(2001),
            command_type: CommandType::Adjustment,
            point_id: 20,
            value: 50.5,
            timestamp: 1234567890,
            metadata: serde_json::Value::Null,
        };

        let channel_cmd = CommandTrigger::to_channel_command(adj_cmd.clone());

        // ChannelCommand is an enum, use pattern matching
        match channel_cmd {
            ChannelCommand::Adjustment {
                point_id, value, ..
            } => {
                assert_eq!(point_id, 20);
                assert!((value - 50.5).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Adjustment variant"),
        }
    }

    // ========================================================================
    // CommandStatus Tests
    // ========================================================================

    #[test]
    fn test_command_status_creation() {
        let status = CommandStatus {
            command_id: "cmd_123".to_string(),
            status: "pending".to_string(),
            result: None,
            error: None,
            timestamp: 1234567890,
        };

        assert_eq!(status.command_id, "cmd_123");
        assert_eq!(status.status, "pending");
        assert!(status.result.is_none());
        assert!(status.error.is_none());
    }

    #[test]
    fn test_command_status_serialization() {
        let status = CommandStatus {
            command_id: "cmd_456".to_string(),
            status: "success".to_string(),
            result: Some(serde_json::json!({"data": "test"})),
            error: None,
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("cmd_456"));
        assert!(json.contains("success"));
    }

    // ========================================================================
    // CommandTriggerConfig Tests
    // ========================================================================

    #[test]
    fn test_command_trigger_config_default() {
        let config = CommandTriggerConfig::default();

        assert_eq!(config.channel_id, 0);
        // Default trait now uses default_timeout() function for consistency
        assert_eq!(config.timeout_seconds, 1);
    }

    #[test]
    fn test_command_trigger_config_creation() {
        let config = CommandTriggerConfig {
            channel_id: 3001,
            timeout_seconds: 60,
        };

        assert_eq!(config.channel_id, 3001);
        assert_eq!(config.timeout_seconds, 60);
    }
}
