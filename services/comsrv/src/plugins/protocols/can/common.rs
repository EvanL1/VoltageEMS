//! CAN Protocol Common Types and Definitions
//!
//! This module contains common data structures, enums, and utilities
//! used across the CAN protocol implementation.

use crate::utils::error::ComSrvError;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// CAN message ID type
pub type CanId = u32;

/// CAN data payload (0-8 bytes for CAN 2.0, 0-64 bytes for CAN FD)
pub type CanData = Vec<u8>;

/// CAN message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanPriority {
    /// Highest priority (0)
    Highest = 0,
    /// High priority (1-3)
    High = 1,
    /// Medium priority (4-5)
    Medium = 4,
    /// Low priority (6-7)
    Low = 6,
}

/// CAN frame format types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanFrameFormat {
    /// Standard CAN 2.0A (11-bit identifier)
    Standard,
    /// Extended CAN 2.0B (29-bit identifier)
    Extended,
}

/// CAN interface types supported by the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanInterfaceType {
    /// Linux SocketCAN interface
    SocketCan(String),
    /// Peak CAN interface
    PeakCan(String),
    /// Virtual CAN for testing
    Virtual(String),
    /// USB CAN adapter
    UsbCan(String),
}

/// CAN bit rate configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanBitRate {
    /// 10 kbit/s
    Kbps10 = 10000,
    /// 20 kbit/s
    Kbps20 = 20000,
    /// 50 kbit/s
    Kbps50 = 50000,
    /// 100 kbit/s
    Kbps100 = 100000,
    /// 125 kbit/s
    Kbps125 = 125000,
    /// 250 kbit/s
    Kbps250 = 250000,
    /// 500 kbit/s
    Kbps500 = 500000,
    /// 800 kbit/s
    Kbps800 = 800000,
    /// 1 Mbit/s
    Mbps1 = 1000000,
}

/// CAN message mapping for point table integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanMessageMapping {
    /// Unique point name/identifier
    pub name: String,
    /// Human-readable display name
    pub display_name: Option<String>,
    /// CAN message ID
    pub can_id: CanId,
    /// Frame format (standard/extended)
    pub frame_format: CanFrameFormat,
    /// Data extraction configuration
    pub data_config: CanDataConfig,
    /// Scaling factor for numeric values
    pub scale: f64,
    /// Offset for numeric values
    pub offset: f64,
    /// Unit of measurement
    pub unit: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Access mode (read/write/read_write)
    pub access_mode: String,
    /// Message transmission rate (Hz, 0 = on-demand)
    pub transmission_rate: f64,
}

/// CAN data extraction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanDataConfig {
    /// Data type for extraction
    pub data_type: CanDataType,
    /// Start byte position (0-based)
    pub start_byte: u8,
    /// Bit offset within start byte (0-7)
    pub bit_offset: u8,
    /// Length in bits for bit-field extraction
    pub bit_length: u8,
    /// Byte order (big endian/little endian)
    pub byte_order: CanByteOrder,
}

/// CAN data types for message payload interpretation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanDataType {
    /// Boolean (1 bit)
    Bool,
    /// 8-bit unsigned integer
    UInt8,
    /// 8-bit signed integer
    Int8,
    /// 16-bit unsigned integer
    UInt16,
    /// 16-bit signed integer
    Int16,
    /// 32-bit unsigned integer
    UInt32,
    /// 32-bit signed integer
    Int32,
    /// 64-bit unsigned integer
    UInt64,
    /// 64-bit signed integer
    Int64,
    /// 32-bit floating point
    Float32,
    /// 64-bit floating point
    Float64,
    /// Raw byte array
    Raw,
    /// ASCII string
    String(usize),
}

impl CanDataType {
    /// Get the size in bytes for this data type
    pub fn size_bytes(&self) -> usize {
        match self {
            CanDataType::Bool => 1,
            CanDataType::UInt8 | CanDataType::Int8 => 1,
            CanDataType::UInt16 | CanDataType::Int16 => 2,
            CanDataType::UInt32 | CanDataType::Int32 | CanDataType::Float32 => 4,
            CanDataType::UInt64 | CanDataType::Int64 | CanDataType::Float64 => 8,
            CanDataType::Raw => 8, // Max CAN frame size
            CanDataType::String(len) => *len,
        }
    }
}

/// Byte order for multi-byte data types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanByteOrder {
    /// Big endian (network byte order)
    BigEndian,
    /// Little endian (Intel byte order)
    LittleEndian,
}

/// CAN-specific error type that wraps the base communication error
pub type CanError = ComSrvError;

/// CAN bus statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct CanStatistics {
    /// Total messages sent
    #[serde(skip)]
    pub messages_sent: AtomicU64,
    /// Total messages received
    #[serde(skip)]
    pub messages_received: AtomicU64,
    /// Error message count
    #[serde(skip)]
    pub error_messages: AtomicU64,
    /// Bus utilization percentage
    #[serde(skip)]
    pub bus_utilization: AtomicU64,
    /// Last error time
    pub last_error_time: Option<std::time::SystemTime>,
}

impl Clone for CanStatistics {
    fn clone(&self) -> Self {
        Self {
            messages_sent: AtomicU64::new(self.messages_sent.load(Ordering::Relaxed)),
            messages_received: AtomicU64::new(self.messages_received.load(Ordering::Relaxed)),
            error_messages: AtomicU64::new(self.error_messages.load(Ordering::Relaxed)),
            bus_utilization: AtomicU64::new(self.bus_utilization.load(Ordering::Relaxed)),
            last_error_time: self.last_error_time,
        }
    }
}

impl CanStatistics {
    /// Create new CAN statistics
    pub fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            error_messages: AtomicU64::new(0),
            bus_utilization: AtomicU64::new(0),
            last_error_time: None,
        }
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        self.messages_sent.store(0, Ordering::Relaxed);
        self.messages_received.store(0, Ordering::Relaxed);
        self.error_messages.store(0, Ordering::Relaxed);
        self.bus_utilization.store(0, Ordering::Relaxed);
        self.last_error_time = None;
    }

    /// Record a sent message
    pub fn record_message_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a received message
    pub fn record_message_received(&self) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an error message
    pub fn record_error_message(&mut self, _error_type: &str) {
        self.error_messages.fetch_add(1, Ordering::Relaxed);
        self.last_error_time = Some(std::time::SystemTime::now());
    }

    /// Update bus utilization
    pub fn update_bus_utilization(&self, utilization: f64) {
        self.bus_utilization
            .store((utilization * 100.0) as u64, Ordering::Relaxed);
    }

    /// Get total messages sent
    pub fn get_messages_sent(&self) -> u64 {
        self.messages_sent.load(Ordering::Relaxed)
    }

    /// Get total messages received
    pub fn get_messages_received(&self) -> u64 {
        self.messages_received.load(Ordering::Relaxed)
    }

    /// Get error message count
    pub fn get_error_messages(&self) -> u64 {
        self.error_messages.load(Ordering::Relaxed)
    }

    /// Get bus utilization percentage
    pub fn get_bus_utilization(&self) -> f64 {
        self.bus_utilization.load(Ordering::Relaxed) as f64 / 100.0
    }

    /// Get last error time
    pub fn get_last_error_time(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.last_error_time.and_then(|st| {
            chrono::DateTime::from_timestamp(
                st.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                0,
            )
        })
    }
}

impl Default for CanStatistics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_data_type_sizes() {
        assert_eq!(CanDataType::Bool.size_bytes(), 1);
        assert_eq!(CanDataType::UInt8.size_bytes(), 1);
        assert_eq!(CanDataType::UInt16.size_bytes(), 2);
        assert_eq!(CanDataType::UInt32.size_bytes(), 4);
        assert_eq!(CanDataType::UInt64.size_bytes(), 8);
        assert_eq!(CanDataType::Float32.size_bytes(), 4);
        assert_eq!(CanDataType::Float64.size_bytes(), 8);
        assert_eq!(CanDataType::String(20).size_bytes(), 20);
    }

    #[test]
    fn test_can_bit_rate_values() {
        assert_eq!(CanBitRate::Kbps125 as u32, 125000);
        assert_eq!(CanBitRate::Kbps250 as u32, 250000);
        assert_eq!(CanBitRate::Kbps500 as u32, 500000);
        assert_eq!(CanBitRate::Mbps1 as u32, 1000000);
    }

    #[test]
    fn test_can_error_display() {
        let error = CanError::InvalidParameter(format!("Invalid CAN ID: 0x{:X}", 0x123));
        assert_eq!(
            error.to_string(),
            "Invalid parameter: Invalid CAN ID: 0x123"
        );

        let error = CanError::ProtocolError(format!("CAN interface not available: {}", "can0"));
        assert_eq!(
            error.to_string(),
            "Protocol error: CAN interface not available: can0"
        );
    }

    #[test]
    fn test_can_message_mapping_serialization() {
        let mapping = CanMessageMapping {
            name: "engine_rpm".to_string(),
            display_name: Some("Engine RPM".to_string()),
            can_id: 0x123,
            frame_format: CanFrameFormat::Standard,
            data_config: CanDataConfig {
                data_type: CanDataType::UInt16,
                start_byte: 0,
                bit_offset: 0,
                bit_length: 16,
                byte_order: CanByteOrder::BigEndian,
            },
            scale: 0.25,
            offset: 0.0,
            unit: Some("RPM".to_string()),
            description: Some("Engine speed in RPM".to_string()),
            access_mode: "read".to_string(),
            transmission_rate: 10.0,
        };

        let json = serde_json::to_string(&mapping).unwrap();
        let deserialized: CanMessageMapping = serde_json::from_str(&json).unwrap();

        assert_eq!(mapping.name, deserialized.name);
        assert_eq!(mapping.can_id, deserialized.can_id);
        assert_eq!(mapping.scale, deserialized.scale);
    }
}
