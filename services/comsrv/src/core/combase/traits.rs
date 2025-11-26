//! Framework core module
//!
//! This module contains ComBase and ComClient trait definitions that use ComSrvError.
//! Data types (ConnectionState, RedisValue, PointData, etc.) are imported from voltage_comlink.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::core::config::types::RuntimeChannelConfig;
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use std::sync::Arc;

// Re-export data types from voltage_comlink for backward compatibility
pub use voltage_comlink::{
    ChannelCommand, ChannelLogger, ChannelStatus, ConnectionState, ExtendedPointData, PointData,
    PointDataMap, ProtocolValue, RedisValue, TelemetryBatch, TestChannelParams,
};

// Import FourRemote from voltage_config (via local config module)
use voltage_config::FourRemote;

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
