//! Basic Data Types and Structures
//!
//! This module contains the fundamental data structures used throughout
//! the communication service.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Channel operational status and health information
#[derive(Debug, Clone)]
pub struct ChannelStatus {
    /// Channel identifier
    pub id: String,
    /// Connection status
    pub connected: bool,
    /// Last response time in milliseconds
    pub last_response_time: f64,
    /// Last error message
    pub last_error: String,
    /// Last status update time
    pub last_update_time: DateTime<Utc>,
}

impl ChannelStatus {
    /// Create a new channel status with default values
    pub fn new(channel_id: &str) -> Self {
        Self {
            id: channel_id.to_string(),
            connected: false,
            last_response_time: 0.0,
            last_error: String::new(),
            last_update_time: Utc::now(),
        }
    }

    /// Check if the channel has any error
    pub fn has_error(&self) -> bool {
        !self.last_error.is_empty()
    }

    /// Get error message by reference to avoid cloning
    pub fn error_ref(&self) -> &str {
        &self.last_error
    }

    /// Get channel ID by reference to avoid cloning
    pub fn id_ref(&self) -> &str {
        &self.id
    }

    /// Check connection status without borrowing the whole struct
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get last response time without borrowing the whole struct
    pub fn response_time(&self) -> f64 {
        self.last_response_time
    }

    /// Get last update timestamp
    pub fn last_update(&self) -> DateTime<Utc> {
        self.last_update_time
    }
}

/// Real-time data point representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    /// Point ID - kept as String for simplicity since it's not frequently shared
    pub id: String,
    /// Point name - kept as String for better readability
    pub name: String,
    /// Point value as string
    pub value: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Engineering unit - usually short strings like "°C", "kW"
    pub unit: String,
    /// Point description
    pub description: String,
}


/// Polling configuration parameters
#[derive(Debug, Clone)]
pub struct PollingConfig {
    /// Enable or disable polling for this channel
    pub enabled: bool,
    /// Polling interval in milliseconds
    pub interval_ms: u64,
    /// Maximum number of points to read per polling cycle
    pub max_points_per_cycle: u32,
    /// Timeout for each polling operation
    pub timeout_ms: u64,
    /// Number of retry attempts on failure
    pub max_retries: u32,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Enable batch reading optimization (protocol-specific)
    pub enable_batch_reading: bool,
    /// Minimum delay between individual point reads in milliseconds
    pub point_read_delay_ms: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000,
            max_points_per_cycle: 100,
            timeout_ms: 5000,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_batch_reading: true,
            point_read_delay_ms: 10,
        }
    }
}

/// Polling statistics information
#[derive(Debug, Clone)]
pub struct PollingStats {
    /// Total number of polling cycles executed
    pub total_cycles: u64,
    /// Number of successful polling cycles
    pub successful_cycles: u64,
    /// Number of failed polling cycles
    pub failed_cycles: u64,
    /// Total data points read successfully
    pub total_points_read: u64,
    /// Total data points that failed to read
    pub total_points_failed: u64,
    /// Average polling cycle time in milliseconds
    pub avg_cycle_time_ms: f64,
    /// Current polling rate (cycles per second)
    pub current_polling_rate: f64,
    /// Last successful polling timestamp
    pub last_successful_polling: Option<DateTime<Utc>>,
    /// Last polling error message
    pub last_polling_error: Option<String>,
}

impl Default for PollingStats {
    fn default() -> Self {
        Self {
            total_cycles: 0,
            successful_cycles: 0,
            failed_cycles: 0,
            total_points_read: 0,
            total_points_failed: 0,
            avg_cycle_time_ms: 0.0,
            current_polling_rate: 0.0,
            last_successful_polling: None,
            last_polling_error: None,
        }
    }
}

/// Connection state enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Channel is disconnected.
    Disconnected,
    /// Channel is attempting to establish a connection.
    Connecting,
    /// Channel is connected and operational.
    Connected,
    /// Channel encountered an error during connection.
    Error(String),
}

/// Point configuration for polling operations
#[derive(Debug, Clone)]
pub struct PollingPoint {
    /// Unique point identifier - kept as Arc for sharing across tasks
    pub id: Arc<str>,
    /// Human-readable point name - kept as Arc for frequent logging/display
    pub name: Arc<str>,
    /// Protocol-specific address (e.g., Modbus register address, IEC60870 IOA)
    pub address: u32,
    /// Data type for value interpretation - usually fixed values like "float", "bool"
    pub data_type: String,
    /// Four-telemetry type classification
    pub telemetry_type: super::telemetry::TelemetryType,
    /// Scaling factor applied to raw values
    pub scale: f64,
    /// Offset applied after scaling
    pub offset: f64,
    /// Engineering unit - usually short like "°C", "kW"
    pub unit: String,
    /// Point description
    pub description: String,
    /// Access mode (read, write, read-write) - fixed values
    pub access_mode: String,
    /// Point group for batch operations - kept as Arc for grouping efficiency
    pub group: Arc<str>,
    /// Protocol-specific parameters
    pub protocol_params: HashMap<String, serde_json::Value>,
}

/// Protocol response data parsing
pub trait ProtocolResponse {
    /// Parse response data as registers (u16 values)
    fn parse_registers(&self) -> crate::utils::Result<Vec<u16>>;

    /// Parse response data as bits (bool values)
    fn parse_bits(&self) -> crate::utils::Result<Vec<bool>>;
}

/// Raw protocol value representation
#[derive(Debug, Clone)]
pub enum RawProtocolValue {
    /// Register values (u16) - from parse_registers()
    Registers(Vec<u16>),
    /// Bit values (bool) - from parse_bits()  
    Bits(Vec<bool>),
    /// Single register value
    SingleRegister(u16),
    /// Single bit value
    SingleBit(bool),
}

impl RawProtocolValue {
    /// Create from protocol response registers
    pub fn from_registers(response: &dyn ProtocolResponse) -> crate::utils::Result<Self> {
        Ok(Self::Registers(response.parse_registers()?))
    }

    /// Create from protocol response bits
    pub fn from_bits(response: &dyn ProtocolResponse) -> crate::utils::Result<Self> {
        Ok(Self::Bits(response.parse_bits()?))
    }

    /// Get register value at index
    pub fn get_register(&self, index: usize) -> crate::utils::Result<u16> {
        use crate::utils::ComSrvError;
        match self {
            Self::Registers(regs) => {
                regs.get(index)
                    .copied()
                    .ok_or_else(|| ComSrvError::InvalidParameter(format!("Register index {} out of bounds", index)))
            }
            Self::SingleRegister(val) if index == 0 => Ok(*val),
            _ => Err(ComSrvError::InvalidParameter("Not a register value".to_string())),
        }
    }

    /// Get bit value at index
    pub fn get_bit(&self, index: usize) -> crate::utils::Result<bool> {
        use crate::utils::ComSrvError;
        match self {
            Self::Bits(bits) => {
                bits.get(index)
                    .copied()
                    .ok_or_else(|| ComSrvError::InvalidParameter(format!("Bit index {} out of bounds", index)))
            }
            Self::SingleBit(val) if index == 0 => Ok(*val),
            _ => Err(ComSrvError::InvalidParameter("Not a bit value".to_string())),
        }
    }

    /// Convert to f64 value
    pub fn to_f64(&self, index: usize) -> crate::utils::Result<f64> {
        match self {
            Self::Registers(_) | Self::SingleRegister(_) => {
                Ok(self.get_register(index)? as f64)
            }
            Self::Bits(_) | Self::SingleBit(_) => {
                Ok(if self.get_bit(index)? { 1.0 } else { 0.0 })
            }
        }
    }

    /// Convert to bool value
    pub fn to_bool(&self, index: usize) -> crate::utils::Result<bool> {
        match self {
            Self::Bits(_) | Self::SingleBit(_) => self.get_bit(index),
            Self::Registers(_) | Self::SingleRegister(_) => {
                Ok(self.get_register(index)? != 0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_status_new() {
        let status = ChannelStatus::new("test_channel");
        assert_eq!(status.id, "test_channel");
        assert!(!status.connected);
        assert_eq!(status.last_response_time, 0.0);
        assert!(status.last_error.is_empty());
        assert!(!status.has_error());
    }

    #[test]
    fn test_channel_status_has_error() {
        let mut status = ChannelStatus::new("test_channel");
        assert!(!status.has_error());

        status.last_error = "Connection failed".to_string();
        assert!(status.has_error());
    }

    #[test]
    fn test_polling_config_default() {
        let config = PollingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.max_points_per_cycle, 100);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
        assert!(config.enable_batch_reading);
        assert_eq!(config.point_read_delay_ms, 10);
    }

    #[test]
    fn test_polling_stats_default() {
        let stats = PollingStats::default();
        assert_eq!(stats.total_cycles, 0);
        assert_eq!(stats.successful_cycles, 0);
        assert_eq!(stats.failed_cycles, 0);
        assert_eq!(stats.total_points_read, 0);
        assert_eq!(stats.total_points_failed, 0);
        assert_eq!(stats.avg_cycle_time_ms, 0.0);
        assert_eq!(stats.current_polling_rate, 0.0);
        assert!(stats.last_successful_polling.is_none());
        assert!(stats.last_polling_error.is_none());
    }

    #[test]
    fn test_connection_state_equality() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Connecting, ConnectionState::Connecting);
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_eq!(
            ConnectionState::Error("test".to_string()),
            ConnectionState::Error("test".to_string())
        );
        assert_ne!(
            ConnectionState::Error("test1".to_string()),
            ConnectionState::Error("test2".to_string())
        );
    }
} 