//! Framework core module
//!
//! Integrates basic trait definitions, type definitions and default implementations

use crate::core::config::types::TelemetryType;
use crate::core::config::ChannelConfig;
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
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
            Self::Float(f) => Some(*f as i64),
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

/// Channel operation status and health information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelStatus {
    pub is_connected: bool,
    pub last_error: Option<String>,
    pub last_update: i64, // Unix timestamp in seconds
    pub success_count: u64,
    pub error_count: u64,
    pub reconnect_count: u64,
    pub points_count: usize,
    pub last_read_duration_ms: Option<u64>,
    pub average_read_duration_ms: Option<f64>,
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
    pub telemetry_type: Option<TelemetryType>,
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
#[async_trait]
pub trait ComBase: Send + Sync {
    /// Get implementation name
    fn name(&self) -> &str;

    /// Get protocol type
    fn protocol_type(&self) -> &str;

    /// Get channel status
    async fn get_status(&self) -> ChannelStatus;

    /// Initialize channel (load point configuration)
    async fn initialize(&mut self, channel_config: Arc<ChannelConfig>) -> Result<()>;

    /// Read four-telemetry data (from cache or Redis)
    async fn read_four_telemetry(&self, telemetry_type: TelemetryType) -> Result<PointDataMap>;

    /// Get diagnostic information
    async fn get_diagnostics(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "name": self.name(),
            "protocol": self.protocol_type(),
        }))
    }
}

/// Client communication trait - for active data collection
///
/// This trait extends ComBase with client-specific functionality
#[async_trait]
pub trait ComClient: ComBase {
    /// Check connection status
    fn is_connected(&self) -> bool;

    /// Connect to target system
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect
    async fn disconnect(&mut self) -> Result<()>;

    /// Execute control command (actively send)
    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>>;

    /// Execute adjustment command (actively send)
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
}

/// Server communication trait - for passive response to requests
///
/// This trait extends ComBase with server-specific functionality
#[async_trait]
pub trait ComServer: ComBase {
    /// Check if server is running
    fn is_running(&self) -> bool;

    /// Start listening
    async fn start(&mut self) -> Result<()>;

    /// Stop server
    async fn stop(&mut self) -> Result<()>;

    /// Verify if client is allowed to connect (e.g., IP whitelist)
    fn verify_client(&self, client_addr: std::net::SocketAddr) -> bool;

    /// Handle read request (passive response)
    /// Read from own channel_id in Redis
    async fn handle_read_request(
        &self,
        address: u16,
        count: u16,
        telemetry_type: TelemetryType,
    ) -> Result<Vec<RedisValue>>;

    /// Handle write request (passive receive)
    /// Write to own channel_id in Redis
    async fn handle_write_request(
        &mut self,
        address: u16,
        value: RedisValue,
        telemetry_type: TelemetryType,
    ) -> Result<bool>;

    /// Get connected client count
    async fn client_count(&self) -> usize;

    /// Get connected client information
    async fn get_connected_clients(&self) -> Vec<ClientInfo>;
}

/// Client connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub addr: String,      // Stored as string for serialization
    pub connected_at: i64, // Unix timestamp in seconds
    pub last_request: i64, // Unix timestamp in seconds
    pub request_count: u64,
}

/// Four-telemetry operations trait
#[async_trait]
pub trait FourTelemetryOperations: Send + Sync {
    async fn read_yc(&self) -> Result<PointDataMap>;
    async fn read_yx(&self) -> Result<PointDataMap>;
    async fn execute_yk(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>>;
    async fn execute_yt(&mut self, adjustments: Vec<(u32, RedisValue)>)
        -> Result<Vec<(u32, bool)>>;
}

/// Connection management trait
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn reconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
    async fn check_connection(&self) -> Result<bool>;
}

/// Configuration validation trait
pub trait ConfigValidator {
    fn validate_config(config: &serde_json::Value) -> Result<()>;
}

/// Protocol packet parser trait
#[async_trait]
pub trait ProtocolPacketParser: Send + Sync {
    fn protocol_name(&self) -> &'static str {
        "Unknown"
    }
    async fn parse_packet(&self, data: &[u8]) -> Result<PacketParseResult>;
    async fn build_packet(&self, data: &PointDataMap) -> Result<Vec<u8>>;
}

/// Packet parse result
#[derive(Debug, Clone)]
pub enum PacketParseResult {
    TelemetryData(PointDataMap),
    ControlResponse(Vec<(u32, bool)>),
    Error(String),
}

// ============================================================================
// Default implementation (from base.rs)
// ============================================================================

/// Default protocol implementation
///
/// Provides reference implementation of ComBase trait
pub struct DefaultProtocol {
    name: String,
    protocol_type: String,
    status: Arc<RwLock<ChannelStatus>>,
    is_connected: Arc<RwLock<bool>>,
    channel_config: Option<Arc<ChannelConfig>>,
    // Under the four-telemetry separated architecture, unified point_mappings is no longer needed
}

impl DefaultProtocol {
    /// Create new instance
    pub fn new(name: String, protocol_type: String) -> Self {
        Self {
            name,
            protocol_type,
            status: Arc::new(RwLock::new(ChannelStatus::default())),
            is_connected: Arc::new(RwLock::new(false)),
            channel_config: None,
            // Under the four-telemetry separated architecture, unified point_mappings is no longer needed
        }
    }

    /// Update status information
    async fn update_status<F>(&self, updater: F)
    where
        F: FnOnce(&mut ChannelStatus),
    {
        let mut status = self.status.write().await;
        updater(&mut status);
        status.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs() as i64;
    }

    // Under the four-telemetry separated architecture, get_mappings method is no longer needed
}

#[async_trait]
impl ComBase for DefaultProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol_type(&self) -> &str {
        &self.protocol_type
    }

    async fn get_status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }

    async fn initialize(&mut self, channel_config: Arc<ChannelConfig>) -> Result<()> {
        self.channel_config = Some(channel_config.clone());

        let point_count = channel_config
            .parameters
            .get("point_count")
            .and_then(serde_yaml::Value::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(usize::MAX);

        self.update_status(|status| {
            status.points_count = point_count;
        })
        .await;

        Ok(())
    }

    async fn read_four_telemetry(&self, _telemetry_type: TelemetryType) -> Result<PointDataMap> {
        // Under the four-telemetry separated architecture, DefaultProtocol only provides basic implementation
        // Actual protocols should override this method to provide real data
        Ok(HashMap::new())
    }
}

#[async_trait]
impl ComClient for DefaultProtocol {
    fn is_connected(&self) -> bool {
        // Use try_read to avoid blocking in async environment
        self.is_connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn connect(&mut self) -> Result<()> {
        *self.is_connected.write().await = true;

        self.update_status(|status| {
            status.is_connected = true;
            status.last_error = None;
        })
        .await;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        *self.is_connected.write().await = false;

        self.update_status(|status| {
            status.is_connected = false;
        })
        .await;

        Ok(())
    }

    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        if !ComClient::is_connected(self) {
            return Err(ComSrvError::NotConnected);
        }

        // Simulate control execution
        let results = commands
            .into_iter()
            .map(|(point_id, _value)| (point_id, true))
            .collect();

        Ok(results)
    }

    async fn adjustment(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        if !ComClient::is_connected(self) {
            return Err(ComSrvError::NotConnected);
        }

        // Simulate adjustment execution
        let results = adjustments
            .into_iter()
            .map(|(point_id, _value)| (point_id, true))
            .collect();

        Ok(results)
    }
}

impl std::fmt::Debug for DefaultProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultProtocol")
            .field("name", &self.name)
            .field("protocol_type", &self.protocol_type)
            .field("is_connected", &self.is_connected)
            .field("channel_config", &self.channel_config)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl FourTelemetryOperations for DefaultProtocol {
    async fn read_yc(&self) -> Result<PointDataMap> {
        self.read_four_telemetry(TelemetryType::Telemetry).await
    }

    async fn read_yx(&self) -> Result<PointDataMap> {
        self.read_four_telemetry(TelemetryType::Signal).await
    }

    async fn execute_yk(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        <Self as ComClient>::control(self, commands).await
    }

    async fn execute_yt(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        <Self as ComClient>::adjustment(self, adjustments).await
    }
}

#[async_trait]
impl ConnectionManager for DefaultProtocol {
    async fn connect(&mut self) -> Result<()> {
        <Self as ComClient>::connect(self).await
    }

    async fn disconnect(&mut self) -> Result<()> {
        <Self as ComClient>::disconnect(self).await
    }

    async fn reconnect(&mut self) -> Result<()> {
        <Self as ConnectionManager>::disconnect(self).await?;
        <Self as ConnectionManager>::connect(self).await
    }

    fn is_connected(&self) -> bool {
        <Self as ComClient>::is_connected(self)
    }

    async fn check_connection(&self) -> Result<bool> {
        Ok(<Self as ComClient>::is_connected(self))
    }
}

// ============================================================================
// Test module
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_protocol() {
        let mut protocol = DefaultProtocol::new("test".to_string(), "default".to_string());

        assert_eq!(protocol.name(), "test");
        assert_eq!(protocol.protocol_type(), "default");
        assert!(!ComClient::is_connected(&protocol));

        // Test connection
        ComClient::connect(&mut protocol).await.unwrap();
        assert!(ComClient::is_connected(&protocol));

        // Test status
        let status = protocol.get_status().await;
        assert!(status.is_connected);
        assert_eq!(status.error_count, 0);
    }

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
