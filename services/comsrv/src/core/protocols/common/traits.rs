//! Communication Base Traits
//!
//! This module contains all the trait definitions for the communication service,
//! including the main ComBase trait and specialized operation traits.
//! Consolidated from combase module.

use async_trait::async_trait;
use std::collections::HashMap;

use super::data_types::{ChannelStatus, PointData, TelemetryType};
use crate::utils::Result;

/// Main communication service trait
///
/// This trait defines the core interface that all communication protocol
/// implementations must provide.
#[async_trait]
pub trait ComBase: Send + Sync + std::fmt::Debug {
    /// Get the human-readable name of the communication service
    fn name(&self) -> &str;

    /// Get the unique channel identifier
    fn channel_id(&self) -> String;

    /// Get the protocol type identifier
    fn protocol_type(&self) -> &str;

    /// Get protocol-specific parameters and configuration
    fn get_parameters(&self) -> HashMap<String, String>;

    /// Check if the communication service is currently running
    async fn is_running(&self) -> bool;

    /// Start the communication service
    async fn start(&mut self) -> Result<()>;

    /// Stop the communication service gracefully
    async fn stop(&mut self) -> Result<()>;

    /// Get the current status of the communication channel
    async fn status(&self) -> ChannelStatus;

    /// Update the channel status
    async fn update_status(&mut self, status: ChannelStatus) -> Result<()>;

    /// Get all available data points
    async fn get_all_points(&self) -> Vec<PointData>;

    /// Read a specific data point by ID
    async fn read_point(&self, point_id: &str) -> Result<PointData>;

    /// Write a value to a specific data point
    async fn write_point(&mut self, point_id: &str, value: &str) -> Result<()>;

    /// Get diagnostic information
    async fn get_diagnostics(&self) -> HashMap<String, String>;

    /// Get the optimized point manager if available
    ///
    /// This method allows access to the unified point management system.
    /// Protocols that use OptimizedPointManager should return it here.
    /// Protocols with custom point management can return None.
    async fn get_point_manager(
        &self,
    ) -> Option<std::sync::Arc<super::manager::OptimizedPointManager>> {
        None
    }

    /// Get points by telemetry type using unified point manager
    ///
    /// This provides a default implementation that uses OptimizedPointManager
    /// if available, otherwise returns empty list. Protocols can override
    /// this method to provide custom implementations.
    async fn get_points_by_telemetry_type(&self, telemetry_type: &TelemetryType) -> Vec<PointData> {
        if let Some(point_manager) = self.get_point_manager().await {
            point_manager.get_point_data_by_type(telemetry_type).await
        } else {
            // Fallback to protocol-specific implementation

            // Filter points by telemetry type if needed (requires protocol-specific logic)
            self.get_all_points().await
        }
    }

    // Point configuration methods have been removed.
    // Each protocol manages its own point configurations.

    /// Get enabled points by telemetry type using unified point manager
    async fn get_enabled_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<String> {
        if let Some(point_manager) = self.get_point_manager().await {
            point_manager
                .get_enabled_points_by_type(telemetry_type)
                .await
        } else {
            Vec::new()
        }
    }
}

/// Four telemetry operations trait
///
/// This trait provides the standard four telemetry operations used
/// in industrial communication protocols.
#[async_trait]
pub trait FourTelemetryOperations: Send + Sync {
    /// 遥测 (YC) - Read telemetry/analog values
    async fn read_telemetry(&self, point_ids: &[String]) -> Result<Vec<PointData>>;

    /// 遥信 (YX) - Read digital signal states
    async fn read_signals(&self, point_ids: &[String]) -> Result<Vec<PointData>>;

    /// 遥控 (YK) - Execute control commands
    async fn execute_control(&mut self, point_id: &str, command: bool) -> Result<bool>;

    /// 遥调 (YT) - Set adjustment values
    async fn set_adjustment(&mut self, point_id: &str, value: f64) -> Result<f64>;
}

/// Connection management trait
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    /// Establish connection to the remote device
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the remote device
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if currently connected
    async fn is_connected(&self) -> bool;

    /// Test connection health
    async fn test_connection(&self) -> Result<bool>;

    /// Get connection statistics
    async fn get_connection_stats(&self) -> HashMap<String, u64>;
}

/// Configuration validation trait
pub trait ConfigValidator {
    /// Validate protocol-specific configuration
    fn validate_config(&self, config: &HashMap<String, String>) -> Result<()>;

    /// Get required configuration parameters
    fn required_params(&self) -> Vec<String>;

    /// Get optional configuration parameters with defaults
    fn optional_params(&self) -> HashMap<String, String>;
}

/// Protocol packet parser trait
pub trait ProtocolPacketParser {
    /// Parse raw bytes into protocol-specific data
    fn parse_packet(&self, data: &[u8]) -> Result<HashMap<String, String>>;

    /// Build protocol packet from data
    fn build_packet(&self, data: &HashMap<String, String>) -> Result<Vec<u8>>;

    /// Get packet type identifier
    fn packet_type(&self, data: &[u8]) -> Result<String>;
}

// PointReader trait has been removed from common layer.
// Each protocol implements its own data reading mechanism.
