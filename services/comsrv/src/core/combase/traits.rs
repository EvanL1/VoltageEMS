//! Framework core module
//!
//! Integrates basic trait definitions, type definitions and default implementations

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::core::config::types::{FourRemote, RuntimeChannelConfig};
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Connection State and Channel Logger
// ============================================================================

/// Connection state for unified logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not initialized yet
    Uninitialized,
    /// Initializing connection
    Initializing,
    /// Attempting to connect
    Connecting,
    /// Successfully connected
    Connected,
    /// Connection failed, will retry
    Disconnected,
    /// In retry process
    Retrying,
    /// Connection closed normally
    Closed,
    /// Fatal error, won't retry
    Failed,
}

impl ConnectionState {
    /// Check if state represents an active connection
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }

    /// Check if state allows retry
    pub fn can_retry(&self) -> bool {
        matches!(
            self,
            ConnectionState::Disconnected | ConnectionState::Retrying
        )
    }
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Uninitialized => write!(f, "UNINITIALIZED"),
            ConnectionState::Initializing => write!(f, "INITIALIZING"),
            ConnectionState::Connecting => write!(f, "CONNECTING"),
            ConnectionState::Connected => write!(f, "CONNECTED"),
            ConnectionState::Disconnected => write!(f, "DISCONNECTED"),
            ConnectionState::Retrying => write!(f, "RETRYING"),
            ConnectionState::Closed => write!(f, "CLOSED"),
            ConnectionState::Failed => write!(f, "FAILED"),
        }
    }
}

/// Lightweight logger for channel-specific logging in static contexts
#[derive(Debug, Clone)]
pub struct ChannelLogger {
    pub channel_id: u32,
    pub channel_name: String,
}

impl ChannelLogger {
    /// Create new channel logger
    pub fn new(channel_id: u32, channel_name: String) -> Self {
        Self {
            channel_id,
            channel_name,
        }
    }

    // ========== Internal Helper ==========

    /// Internal helper for dual logging (channel + service)
    fn log_dual(&self, level: tracing::Level, message: String) {
        common::log_to_channel!(self.channel_id, &self.channel_name, level, "{}", message);
        common::log_to_service!(self.channel_id, level, "{}", message);
    }

    /// Internal helper for channel-only logging
    #[allow(unused_variables)] // False positive: level is used in macro expansion
    fn log_channel_only(&self, level: tracing::Level, message: String) {
        common::log_to_channel!(self.channel_id, &self.channel_name, level, "{}", message);
    }

    // ========== Connection State Logging ==========

    /// Log initialization step (dual output)
    pub fn log_init(&self, protocol: &str, message: &str) {
        self.log_dual(
            tracing::Level::INFO,
            format!("[INIT] {} - {}", protocol, message),
        );
    }

    /// Log connection attempt (dual output)
    pub fn log_connect(&self, protocol: &str, target: &str, details: &str) {
        self.log_dual(
            tracing::Level::INFO,
            format!("[CONNECT] {} to {} - {}", protocol, target, details),
        );
    }

    /// Log connection status change (dual output)
    pub fn log_status(&self, old_state: ConnectionState, new_state: ConnectionState, reason: &str) {
        self.log_dual(
            tracing::Level::INFO,
            format!("[STATUS] {} -> {} - {}", old_state, new_state, reason),
        );
    }

    /// Log retry attempt (dual output)
    pub fn log_retry(&self, attempt: u32, max_attempts: u32, delay_ms: u64, reason: &str) {
        self.log_dual(
            tracing::Level::WARN,
            format!(
                "[RETRY] Attempt {}/{}, delay {}ms - {}",
                attempt, max_attempts, delay_ms, reason
            ),
        );
    }

    /// Log configuration details (channel file only)
    pub fn log_config(&self, protocol: &str, key: &str, value: &str) {
        self.log_channel_only(
            tracing::Level::DEBUG,
            format!("[CONFIG] {} {} = {}", protocol, key, value),
        );
    }

    // ========== Protocol-Specific Logging ==========

    /// Log protocol message (channel file only)
    pub fn log_protocol_message(&self, direction: &str, data: &[u8], message: &str) {
        // Format bytes as hex with space separator: [02 BD 00 00]
        let hex_str = data
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");

        self.log_channel_only(
            tracing::Level::DEBUG,
            format!(
                "[{}] {} bytes: [{}] - {}",
                direction,
                data.len(),
                hex_str,
                message
            ),
        );
    }

    /// Log parsed data (channel file only)
    pub fn log_parsed_data(
        &self,
        telemetry_type: &str,
        point_id: &str,
        value: &str,
        raw_decimal: u64,
        raw_data: &[u8],
    ) {
        // Format raw bytes as hex without commas: [00 1A 24]
        let regs_str = raw_data
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");

        self.log_channel_only(
            tracing::Level::DEBUG,
            format!(
                "[{}] Parsed point {}: regs=[{}] raw={} value={}",
                telemetry_type, point_id, regs_str, raw_decimal, value
            ),
        );
    }

    /// Log raw Modbus message (channel file only)
    ///
    /// Supports both Modbus TCP (with transaction_id) and RTU (without transaction_id).
    pub fn log_raw_message(
        &self,
        direction: &str,
        transaction_id: Option<u16>,
        slave_id: u8,
        function_code: u8,
        raw_frame: &[u8],
    ) {
        // Format complete frame as hex: [01 03 00 00 00 04 44 09]
        let hex_str = raw_frame
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");

        let message = if let Some(tid) = transaction_id {
            // Modbus TCP - include transaction ID
            format!(
                "[{}] TID={:04X} Slave={} FC={:02X} Frame:[{}]",
                direction, tid, slave_id, function_code, hex_str
            )
        } else {
            // Modbus RTU - no transaction ID
            format!(
                "[{}] Slave={} FC={:02X} Frame:[{}]",
                direction, slave_id, function_code, hex_str
            )
        };

        self.log_channel_only(tracing::Level::INFO, message);
    }
}
// ============================================================================
// Redis value type definitions
// ============================================================================

/// Redis value type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedisValue {
    String(Cow<'static, str>),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
}

// Convenience From implementations
impl From<f64> for RedisValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<i64> for RedisValue {
    fn from(v: i64) -> Self {
        Self::Integer(v)
    }
}

impl From<i32> for RedisValue {
    fn from(v: i32) -> Self {
        Self::Integer(v as i64)
    }
}

impl From<&str> for RedisValue {
    fn from(v: &str) -> Self {
        Self::String(Cow::Owned(v.to_string()))
    }
}

impl From<String> for RedisValue {
    fn from(v: String) -> Self {
        Self::String(Cow::Owned(v))
    }
}

impl From<bool> for RedisValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<u16> for RedisValue {
    fn from(v: u16) -> Self {
        Self::Integer(v as i64)
    }
}

impl From<u32> for RedisValue {
    fn from(v: u32) -> Self {
        Self::Integer(v as i64)
    }
}

impl From<u8> for RedisValue {
    fn from(v: u8) -> Self {
        Self::Integer(v as i64)
    }
}

// Unified numeric interface methods
impl RedisValue {
    /// Try to convert to f64
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Integer(i) => Some(*i as f64),
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Self::String(s) => s.parse().ok(),
            Self::Null => None,
        }
    }

    /// Try to convert to i64
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            Self::Float(f) => Some(f.round() as i64), // 四舍五入，避免精度损失 (round to nearest to avoid precision loss)
            Self::Bool(b) => Some(if *b { 1 } else { 0 }),
            Self::String(s) => s.parse().ok(),
            Self::Null => None,
        }
    }

    /// Try to convert to bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::Integer(i) => Some(*i != 0),
            Self::Float(f) => Some(*f != 0.0),
            Self::String(s) => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            },
            Self::Null => None,
        }
    }

    /// Try to convert to String
    pub fn as_string(&self) -> String {
        match self {
            Self::String(s) => s.to_string(),
            Self::Integer(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Bool(b) => b.to_string(),
            Self::Null => String::new(),
        }
    }

    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Convert to u16 with bounds checking
    pub fn as_u16(&self) -> Option<u16> {
        self.as_i64().and_then(|i| {
            if i >= 0 && i <= u16::MAX as i64 {
                Some(i as u16)
            } else {
                None
            }
        })
    }

    /// Convert to u32 with bounds checking
    pub fn as_u32(&self) -> Option<u32> {
        self.as_i64().and_then(|i| {
            if i >= 0 && i <= u32::MAX as i64 {
                Some(i as u32)
            } else {
                None
            }
        })
    }

    /// Get value or default
    pub fn unwrap_or<T>(&self, default: T) -> T
    where
        T: From<RedisValue> + Clone,
    {
        if self.is_null() {
            default
        } else {
            T::from(self.clone())
        }
    }
}

/// Channel command enumeration
#[derive(Debug, Clone)]
pub enum ChannelCommand {
    /// Control command (YK)
    Control {
        command_id: String,
        point_id: u32,
        value: f64,
        timestamp: i64,
    },
    /// Adjustment command (YT)
    Adjustment {
        command_id: String,
        point_id: u32,
        value: f64,
        timestamp: i64,
    },
}

/// Batch telemetry data for channel transmission
#[derive(Debug, Clone)]
pub struct TelemetryBatch {
    pub channel_id: u16,
    pub telemetry: Vec<(u32, f64, i64)>, // (point_id, raw_value, timestamp)
    pub signal: Vec<(u32, f64, i64)>,    // (point_id, raw_value, timestamp)
}

// ============================================================================
// Basic type definitions (from types.rs)
// ============================================================================

/// Channel operation status - minimal and essential fields only
///
/// All fields are Copy types, making clone() essentially free (memcpy).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ChannelStatus {
    pub is_connected: bool,
    pub last_update: i64, // Unix timestamp in seconds
}

/// Point data structure - using combase wrapper type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    pub value: RedisValue,
    pub timestamp: i64, // Unix timestamp in seconds
}

/// Extended point data (for API and display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedPointData {
    pub id: String,
    pub name: String,
    pub value: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub unit: String,
    pub description: String,
    pub telemetry_type: Option<FourRemote>,
    pub channel_id: Option<u16>,
}

impl Default for PointData {
    fn default() -> Self {
        Self {
            value: RedisValue::Float(0.0),
            timestamp: 0,
        }
    }
}

/// Point mapping table
pub type PointDataMap = HashMap<u32, PointData>;

/// Test channel parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestChannelParams {
    pub initial_value: f64,
    pub increment: f64,
    pub interval_ms: u64,
}

impl Default for TestChannelParams {
    fn default() -> Self {
        Self {
            initial_value: 0.0,
            increment: 1.0,
            interval_ms: 1000,
        }
    }
}

// ============================================================================
// Core trait definitions (from traits.rs)
// ============================================================================

/// Base communication trait - defines common four-telemetry data model
///
/// This trait defines the core data interface shared by both clients and servers
///
/// @trait ComBase
/// @purpose Define core four-telemetry data model interface
/// @implementors All protocol plugins (Modbus, Virtual, gRPC, CAN, IEC)
/// @philosophy Four-telemetry separation (T/S/C/A) for clean data flow
#[async_trait]
pub trait ComBase: Send + Sync {
    /// Get implementation name
    fn name(&self) -> &str;

    /// Get channel ID
    fn get_channel_id(&self) -> u16;

    /// Get channel status
    async fn get_status(&self) -> ChannelStatus;

    /// Initialize channel (load point configuration)
    ///
    /// @input runtime_config: Arc<RuntimeChannelConfig> - Point definitions and mappings
    /// @output Result<()> - Success or initialization error
    /// @side-effects Loads protocol mappings into memory
    /// @lifecycle Called once during channel creation
    async fn initialize(&mut self, runtime_config: Arc<RuntimeChannelConfig>) -> Result<()>;

    /// Read four-telemetry data (from cache or Redis)
    /// Each telemetry type should be handled independently with its own configuration
    ///
    /// @input telemetry_type: FourRemote - T|S|C|A type to read
    /// @output Result<PointDataMap> - Point ID to value mapping
    /// @redis-read comsrv:{channel}:{type} - Cached telemetry data
    /// @philosophy Four-telemetry isolation for clean data management
    async fn read_four_telemetry(&self, telemetry_type: FourRemote) -> Result<PointDataMap>;

    /// Get diagnostic information
    async fn get_diagnostics(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "name": self.name(),
        }))
    }
}

/// Client communication trait - for active data collection
///
/// This trait extends ComBase with client-specific functionality
///
/// @trait ComClient
/// @extends ComBase
/// @purpose Define active client protocol behavior
/// @implementors ModbusClient, VirtualClient, GrpcClient, CanClient, IecClient
/// @lifecycle connect → read telemetry → write control/adjustment → disconnect
/// @philosophy Active polling and command execution
#[async_trait]
pub trait ComClient: ComBase {
    /// Check connection status
    fn is_connected(&self) -> bool;

    /// Connect to target system
    ///
    /// @output Result<()> - Connection success or error
    /// @side-effects Establishes TCP/Serial/gRPC connection
    /// @retry Automatic reconnection on failure
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect
    async fn disconnect(&mut self) -> Result<()>;

    /// Execute control command (actively send)
    ///
    /// @input commands: Vec<(u32, RedisValue)> - Point ID and value pairs
    /// @output Result<Vec<(u32, bool)>> - Execution results per point
    /// @protocol-write Send YK control commands to device
    /// @redis-write comsrv:{channel}:C - Control status
    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>>;

    /// Execute adjustment command (actively send)
    ///
    /// @input adjustments: Vec<(u32, RedisValue)> - Point ID and value pairs
    /// @output Result<Vec<(u32, bool)>> - Execution results per point
    /// @protocol-write Send YT adjustment commands to device
    /// @redis-write comsrv:{channel}:A - Adjustment status
    async fn adjustment(&mut self, adjustments: Vec<(u32, RedisValue)>)
        -> Result<Vec<(u32, bool)>>;

    /// Start periodic tasks (polling, etc.)
    async fn start_periodic_tasks(&self) -> Result<()> {
        Ok(())
    }

    /// Stop periodic tasks
    async fn stop_periodic_tasks(&self) -> Result<()> {
        Ok(())
    }

    /// Set data channel for sending telemetry data
    /// Protocols will send data through this channel instead of caching
    fn set_data_channel(&mut self, _tx: tokio::sync::mpsc::Sender<TelemetryBatch>) {
        // Default implementation does nothing
        // Protocols that support channel-based data flow should override this
    }

    /// Set command receiver for receiving control commands
    /// Protocols that support command processing should override this
    fn set_command_receiver(&mut self, _rx: tokio::sync::mpsc::Receiver<ChannelCommand>) {
        // Default implementation does nothing
        // Protocols that support command processing should override this
    }

    /// Try to reconnect when connection is lost
    /// Default implementation: disconnect and reconnect with delay
    ///
    /// @output Result<()> - Reconnection success or error
    /// @side-effects Drops old connection and creates new one
    /// @delay 1000ms between disconnect and reconnect
    async fn try_reconnect(&mut self) -> Result<()> {
        use tokio::time::{sleep, Duration};

        // First try to disconnect cleanly
        let _ = self.disconnect().await;

        // Wait a bit before reconnecting
        sleep(Duration::from_millis(1000)).await;

        // Attempt to reconnect
        self.connect().await
    }

    /// Check if the error indicates a connection problem that needs reconnection
    fn needs_reconnect(&self, error: &ComSrvError) -> bool {
        match error {
            ComSrvError::IoError(msg) => {
                msg.contains("Broken pipe")
                    || msg.contains("Connection reset")
                    || msg.contains("Connection refused")
                    || msg.contains("Connection aborted")
                    || msg.contains("Network is unreachable")
            },
            ComSrvError::ConnectionError(_) => true,
            ComSrvError::NotConnected => true,
            _ => false,
        }
    }
}

// ============================================================================
// Test module
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_point_data_default() {
        let point = PointData::default();
        assert_eq!(point.timestamp, 0);
        match point.value {
            RedisValue::Float(v) => assert!(v.abs() < f64::EPSILON),
            _ => panic!("Expected float value"),
        }
    }
}
