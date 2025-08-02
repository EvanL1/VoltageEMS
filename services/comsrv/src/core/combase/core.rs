//! Framework core module
//!
//! Integrates basic trait definitions, type definitions and default implementations

use crate::core::config::{ChannelConfig, TelemetryType};
use crate::plugins::core::PluginStorage;
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
// ============================================================================
// Redis value type definitions
// ============================================================================

/// Redis value type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedisValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
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

// ============================================================================
// Basic type definitions (from types.rs)
// ============================================================================

/// Channel operation status and health information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelStatus {
    pub is_connected: bool,
    pub last_error: Option<String>,
    pub last_update: u64,
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
    pub timestamp: u64,
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

/// Main communication service trait
///
/// This trait defines the core interface that all communication protocol implementations must provide
#[async_trait]
pub trait ComBase: Send + Sync {
    /// Get implementation name
    fn name(&self) -> &str;

    /// Get protocol type
    fn protocol_type(&self) -> &str;

    /// Check connection status
    fn is_connected(&self) -> bool;

    /// Get channel status
    async fn get_status(&self) -> ChannelStatus;

    /// Initialize channel
    async fn initialize(&mut self, channel_config: &ChannelConfig) -> Result<()>;

    /// Connect to target system
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect
    async fn disconnect(&mut self) -> Result<()>;

    /// Read four-telemetry data
    async fn read_four_telemetry(&self, telemetry_type: &str) -> Result<PointDataMap>;

    /// Execute control command
    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>>;

    /// Execute adjustment command
    async fn adjustment(&mut self, adjustments: Vec<(u32, RedisValue)>)
        -> Result<Vec<(u32, bool)>>;

    // Under the four-telemetry separated architecture, update_points method is no longer needed, point configuration is directly loaded during initialize phase

    /// Start periodic tasks
    async fn start_periodic_tasks(&self) -> Result<()> {
        Ok(())
    }

    /// Stop periodic tasks
    async fn stop_periodic_tasks(&self) -> Result<()> {
        Ok(())
    }

    /// Get diagnostic information
    async fn get_diagnostics(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "name": self.name(),
            "protocol": self.protocol_type(),
            "connected": self.is_connected()
        }))
    }
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
    channel_config: Option<ChannelConfig>,
    // Under the four-telemetry separated architecture, unified point_mappings is no longer needed
    storage: Option<Arc<Mutex<Box<dyn PluginStorage>>>>,
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
            storage: None,
        }
    }

    /// Set storage backend
    #[must_use]
    pub fn with_storage(mut self, storage: Box<dyn PluginStorage>) -> Self {
        self.storage = Some(Arc::new(Mutex::new(storage)));
        self
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
            .unwrap()
            .as_secs();
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

    fn is_connected(&self) -> bool {
        // Use try_read to avoid blocking in async environment
        self.is_connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn get_status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }

    async fn initialize(&mut self, channel_config: &ChannelConfig) -> Result<()> {
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

    async fn read_four_telemetry(&self, _telemetry_type: &str) -> Result<PointDataMap> {
        if !<Self as ComBase>::is_connected(self) {
            return Err(ComSrvError::NotConnected);
        }

        // Under the four-telemetry separated architecture, DefaultProtocol only provides basic implementation
        // Actual protocols should override this method to provide real data
        Ok(HashMap::new())
    }

    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        if !<Self as ComBase>::is_connected(self) {
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
        if !<Self as ComBase>::is_connected(self) {
            return Err(ComSrvError::NotConnected);
        }

        // Simulate adjustment execution
        let results = adjustments
            .into_iter()
            .map(|(point_id, _value)| (point_id, true))
            .collect();

        Ok(results)
    }

    // Under the four-telemetry separated architecture, update_points method has been removed
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
        self.read_four_telemetry("m").await
    }

    async fn read_yx(&self) -> Result<PointDataMap> {
        self.read_four_telemetry("s").await
    }

    async fn execute_yk(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        self.control(commands).await
    }

    async fn execute_yt(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        self.adjustment(adjustments).await
    }
}

#[async_trait]
impl ConnectionManager for DefaultProtocol {
    async fn connect(&mut self) -> Result<()> {
        ComBase::connect(self).await
    }

    async fn disconnect(&mut self) -> Result<()> {
        ComBase::disconnect(self).await
    }

    async fn reconnect(&mut self) -> Result<()> {
        <Self as ConnectionManager>::disconnect(self).await?;
        <Self as ConnectionManager>::connect(self).await
    }

    fn is_connected(&self) -> bool {
        ComBase::is_connected(self)
    }

    async fn check_connection(&self) -> Result<bool> {
        Ok(<Self as ComBase>::is_connected(self))
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
        assert!(!ComBase::is_connected(&protocol));

        // Test connection
        ComBase::connect(&mut protocol).await.unwrap();
        assert!(ComBase::is_connected(&protocol));

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
