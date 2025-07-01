//! Communication Base Traits
//!
//! This module contains all the trait definitions for the communication service,
//! including the main ComBase trait and specialized operation traits.

use async_trait::async_trait;
use std::collections::HashMap;

use super::data_types::{ChannelStatus, PointData, ConnectionState};
use super::telemetry::{TelemetryType, PointValueType, RemoteOperationRequest, RemoteOperationResponse};
use super::point_manager::UniversalPointManager;
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

    /// Get the universal point manager if available
    /// 
    /// This method allows access to the unified point management system.
    /// Protocols that use UniversalPointManager should return it here.
    /// Protocols with custom point management can return None.
    async fn get_point_manager(&self) -> Option<UniversalPointManager> {
        None
    }

    /// Get points by telemetry type using unified point manager
    /// 
    /// This provides a default implementation that uses UniversalPointManager
    /// if available, otherwise returns empty list. Protocols can override
    /// this method to provide custom implementations.
    async fn get_points_by_telemetry_type(&self, telemetry_type: &TelemetryType) -> Vec<PointData> {
        if let Some(point_manager) = self.get_point_manager().await {
            point_manager.get_point_data_by_type(telemetry_type).await
        } else {
            // Fallback to protocol-specific implementation
            let all_points = self.get_all_points().await;
            // Filter points by telemetry type if needed (requires protocol-specific logic)
            all_points
        }
    }

    /// Get all point configurations using unified point manager
    /// 
    /// This provides a default implementation that uses UniversalPointManager
    /// if available. Protocols can override for custom implementations.
    async fn get_all_point_configs(&self) -> Vec<super::point_manager::UniversalPointConfig> {
        if let Some(point_manager) = self.get_point_manager().await {
            point_manager.get_all_point_configs().await
        } else {
            Vec::new()
        }
    }

    /// Get enabled points by telemetry type using unified point manager
    async fn get_enabled_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<String> {
        if let Some(point_manager) = self.get_point_manager().await {
            point_manager.get_enabled_points_by_type(telemetry_type).await
        } else {
            Vec::new()
        }
    }
}

/// Four telemetry operations trait
///
/// This trait defines the standard four telemetry operations used in
/// industrial automation: measurement, signaling, control, and regulation.
#[async_trait]
pub trait FourTelemetryOperations: Send + Sync {
    /// Remote Measurement (遥测) - Read analog measurement values
    async fn remote_measurement(
        &self,
        point_names: &[String],
    ) -> Result<Vec<(String, PointValueType)>>;

    /// Remote Signaling (遥信) - Read digital status values
    async fn remote_signaling(
        &self,
        point_names: &[String],
    ) -> Result<Vec<(String, PointValueType)>>;

    /// Remote Control (遥控) - Execute digital control operations
    async fn remote_control(
        &self,
        request: RemoteOperationRequest,
    ) -> Result<RemoteOperationResponse>;

    /// Remote Regulation (遥调) - Execute analog regulation operations
    async fn remote_regulation(
        &self,
        request: RemoteOperationRequest,
    ) -> Result<RemoteOperationResponse>;

    /// Get all available remote control points
    async fn get_control_points(&self) -> Vec<String>;

    /// Get all available remote regulation points
    async fn get_regulation_points(&self) -> Vec<String>;

    /// Get all available measurement points
    async fn get_measurement_points(&self) -> Vec<String>;

    /// Get all available signaling points
    async fn get_signaling_points(&self) -> Vec<String>;

    /// Get points by telemetry type
    async fn get_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<String> {
        match *telemetry_type {
            TelemetryType::Telemetry => self.get_measurement_points().await,
            TelemetryType::Signaling => self.get_signaling_points().await,
            TelemetryType::Control => self.get_control_points().await,
            TelemetryType::Setpoint => self.get_regulation_points().await,
        }
    }

    /// Batch read points by telemetry type
    async fn batch_read_by_type(
        &self,
        telemetry_type: &TelemetryType,
        point_names: Option<&[String]>,
    ) -> Result<Vec<(String, PointValueType)>> {
        let points_to_read = if let Some(names) = point_names {
            names.to_vec()
        } else {
            self.get_points_by_type(&telemetry_type).await
        };

        match *telemetry_type {
            TelemetryType::Telemetry => self.remote_measurement(&points_to_read).await,
            TelemetryType::Signaling => self.remote_signaling(&points_to_read).await,
            _ => Err(crate::utils::ComSrvError::InvalidOperation(
                "Batch read not supported for control/regulation points".to_string(),
            )),
        }
    }
}

/// Connection management trait
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    /// Connect to the remote endpoint
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the remote endpoint
    async fn disconnect(&mut self) -> Result<()>;

    /// Attempt to reconnect using protocol specific strategy
    async fn reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    /// Retrieve the current connection state
    async fn connection_state(&self) -> ConnectionState;
}

/// Configuration validation trait
#[async_trait]
pub trait ConfigValidator: Send + Sync {
    /// Validate configuration parameters
    async fn validate_config(&self) -> Result<()> {
        Ok(())
    }
}

/// Protocol statistics trait
pub trait ProtocolStats: Send + Sync {
    /// Reset all statistic counters
    fn reset(&mut self);
}

/// Protocol packet parsing trait
pub trait ProtocolPacketParser: Send + Sync {
    /// Get the protocol name
    fn protocol_name(&self) -> &str;

    /// Parse a packet and return human-readable interpretation
    fn parse_packet(&self, data: &[u8], direction: &str) -> super::default_protocol::PacketParseResult;

    /// Convert bytes to hexadecimal string
    fn format_hex_data(&self, data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Protocol logging trait
#[async_trait]
pub trait ProtocolLogger: Send + Sync {
    /// Log a data point read operation
    async fn log_point_read(&self, channel_id: &str, point_id: &str, value: &str, success: bool);

    /// Log a data point write operation
    async fn log_point_write(&self, channel_id: &str, point_id: &str, value: &str, success: bool);

    /// Log a connection event
    async fn log_connection_event(&self, channel_id: &str, event: &str, success: bool);

    /// Log an error event
    async fn log_error(&self, channel_id: &str, error: &str);

    /// Log a protocol-specific message
    async fn log_protocol_message(&self, channel_id: &str, direction: &str, data: &[u8]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::protocols::common::combase::telemetry::*;

    // Mock implementation for testing
    #[derive(Debug)]
    struct MockComBase {
        name: String,
        running: bool,
    }

    #[async_trait]
    impl ComBase for MockComBase {

        fn name(&self) -> &str {
            &self.name
        }

        fn channel_id(&self) -> String {
            "test_channel".to_string()
        }

        fn protocol_type(&self) -> &str {
            "test_protocol"
        }

        fn get_parameters(&self) -> HashMap<String, String> {
            HashMap::new()
        }

        async fn is_running(&self) -> bool {
            self.running
        }

        async fn start(&mut self) -> Result<()> {
            self.running = true;
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            self.running = false;
            Ok(())
        }

        async fn status(&self) -> ChannelStatus {
            ChannelStatus::new("test_channel")
        }

        async fn update_status(&mut self, _status: ChannelStatus) -> Result<()> {
            Ok(())
        }

        async fn get_all_points(&self) -> Vec<PointData> {
            Vec::new()
        }

        async fn read_point(&self, _point_id: &str) -> Result<PointData> {
            Ok(PointData {
                id: "test_point".to_string(),
                name: "Test Point".to_string(),
                value: "123.45".to_string(),
                timestamp: chrono::Utc::now(),
                unit: "°C".to_string(),
                description: "Test point".to_string(),
            })
        }

        async fn write_point(&mut self, _point_id: &str, _value: &str) -> Result<()> {
            Ok(())
        }

        async fn get_diagnostics(&self) -> HashMap<String, String> {
            HashMap::new()
        }
    }

    #[tokio::test]
    async fn test_combase_trait() {
        let mut mock = MockComBase {
            name: "Test Service".to_string(),
            running: false,
        };

        assert_eq!(mock.name(), "Test Service");
        assert_eq!(mock.channel_id(), "test_channel");
        assert_eq!(mock.protocol_type(), "test_protocol");
        assert!(!mock.is_running().await);

        mock.start().await.unwrap();
        assert!(mock.is_running().await);

        mock.stop().await.unwrap();
        assert!(!mock.is_running().await);
    }
} 