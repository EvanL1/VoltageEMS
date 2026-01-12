//! CAN protocol configuration types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CAN client configuration.
#[derive(Debug, Clone)]
pub struct CanConfig {
    /// CAN interface name (e.g., "can0").
    pub can_interface: String,

    /// CAN bitrate (bits per second).
    pub bitrate: u32,

    /// RX polling interval in milliseconds.
    pub rx_poll_interval_ms: u64,

    /// Data reading interval in milliseconds.
    pub data_read_interval_ms: u64,
}

impl Default for CanConfig {
    fn default() -> Self {
        Self {
            can_interface: "can0".to_string(),
            bitrate: 250000,
            rx_poll_interval_ms: 50,
            data_read_interval_ms: 1000,
        }
    }
}

/// CAN data type enumeration.
///
/// Using an enum instead of String eliminates heap allocation per point
/// and enables fast integer-based matching in the decoder hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CanDataType {
    /// Unsigned 8-bit integer
    UInt8,
    /// Unsigned 16-bit integer (default)
    #[default]
    UInt16,
    /// Signed 16-bit integer
    Int16,
    /// Unsigned 32-bit integer
    UInt32,
    /// Signed 32-bit integer
    Int32,
    /// 32-bit floating point
    Float32,
    /// ASCII string
    Ascii,
}

/// CAN point mapping structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanPoint {
    /// Unique point identifier (numeric)
    pub point_id: u32,
    /// CAN-ID (e.g., 0x351)
    pub can_id: u32,
    /// Byte offset in CAN data field (0-7)
    pub byte_offset: u8,
    /// Bit starting position within byte (0-7, LSB=0)
    pub bit_position: u8,
    /// Bit length (2/8/16/32/64)
    pub bit_length: u8,
    /// Data type for interpretation
    pub data_type: CanDataType,
    /// Scale factor for linear transformation (value = raw * scale + offset)
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// Offset for linear transformation
    #[serde(default)]
    pub offset: f64,
}

fn default_scale() -> f64 {
    1.0
}

// ============================================================================
// Strongly-typed mapping configs for JSON deserialization
// ============================================================================

/// CAN point mapping configuration (deserialized from protocol_mappings JSON).
///
/// # Required Fields
/// - `can_id`: The CAN frame ID. This field is **required** and
///   deserialization will fail if missing.
///
/// # Optional Fields
/// - `byte_offset`: Byte offset in CAN data field (default: 0)
/// - `bit_position`: Bit position within byte (default: 0)
/// - `bit_length`: Number of bits to read (default: 16)
/// - `data_type`: Data type interpretation (default: "uint16")
///
/// # Example JSON
/// ```json
/// {
///     "can_id": 849,
///     "byte_offset": 0,
///     "bit_position": 0,
///     "bit_length": 16,
///     "data_type": "uint16"
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct CanMappingConfig {
    /// CAN frame ID (e.g., 0x351 or 849). **Required field**.
    pub can_id: u32,

    /// Byte offset in CAN data field (0-7).
    #[serde(default)]
    pub byte_offset: u8,

    /// Bit starting position within byte (0-7, LSB=0).
    #[serde(default)]
    pub bit_position: u8,

    /// Bit length (1/2/8/16/32/64).
    #[serde(default = "default_bit_length")]
    pub bit_length: u8,

    /// Data type for interpretation.
    #[serde(default)]
    pub data_type: CanDataType,
}

fn default_bit_length() -> u8 {
    16
}

impl CanMappingConfig {
    /// Convert to CanPoint.
    pub fn to_can_point(&self, point_id: u32, scale: f64, offset: f64) -> CanPoint {
        CanPoint {
            point_id,
            can_id: self.can_id,
            byte_offset: self.byte_offset,
            bit_position: self.bit_position,
            bit_length: self.bit_length,
            data_type: self.data_type, // Copy, no clone needed
            scale,
            offset,
        }
    }
}

/// CAN channel parameters configuration (deserialized from parameters_json).
///
/// # Example JSON
/// ```json
/// {
///     "interface": "can0",
///     "bitrate": 250000,
///     "rx_poll_interval_ms": 50,
///     "data_read_interval_ms": 1000
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct CanChannelParamsConfig {
    /// CAN interface name (e.g., "can0").
    #[serde(default = "default_can_interface")]
    pub interface: String,

    /// CAN bitrate in bits per second.
    #[serde(default = "default_bitrate")]
    pub bitrate: u32,

    /// RX polling interval in milliseconds.
    #[serde(default = "default_rx_poll_interval")]
    pub rx_poll_interval_ms: u64,

    /// Data reading interval in milliseconds.
    #[serde(default = "default_data_read_interval")]
    pub data_read_interval_ms: u64,
}

fn default_can_interface() -> String {
    "can0".to_string()
}

fn default_bitrate() -> u32 {
    250000
}

fn default_rx_poll_interval() -> u64 {
    50
}

fn default_data_read_interval() -> u64 {
    1000
}

impl CanChannelParamsConfig {
    /// Convert to CanConfig.
    pub fn to_config(&self) -> CanConfig {
        CanConfig {
            can_interface: self.interface.clone(),
            bitrate: self.bitrate,
            rx_poll_interval_ms: self.rx_poll_interval_ms,
            data_read_interval_ms: self.data_read_interval_ms,
        }
    }
}

/// CAN frame data - stack-allocated fixed buffer for up to 8 bytes
#[derive(Debug, Clone, Copy, Default)]
pub struct CanFrameData {
    data: [u8; 8],
    len: u8,
}

impl CanFrameData {
    /// Create from a byte slice (copies up to 8 bytes)
    pub fn from_slice(bytes: &[u8]) -> Self {
        let mut data = [0u8; 8];
        let len = bytes.len().min(8) as u8;
        data[..len as usize].copy_from_slice(&bytes[..len as usize]);
        Self { data, len }
    }

    /// Get the data as a slice
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }

    /// Get the length
    pub fn len(&self) -> usize {
        self.len as usize
    }
}

/// CAN frame cache - stores the latest received frame for each CAN-ID
/// Uses fixed-size arrays instead of Vec to avoid heap allocation per frame
#[derive(Debug, Clone, Default)]
pub struct CanFrameCache {
    /// Map from CAN-ID to frame data (fixed 8-byte buffer + length)
    frames: HashMap<u32, CanFrameData>,
}

impl CanFrameCache {
    /// Create a new empty frame cache
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
        }
    }

    /// Update cache with a new frame (no heap allocation for the data)
    pub fn update(&mut self, can_id: u32, data: &[u8]) {
        self.frames.insert(can_id, CanFrameData::from_slice(data));
    }

    /// Get the latest frame data for a CAN-ID
    pub fn get(&self, can_id: u32) -> Option<&[u8]> {
        self.frames.get(&can_id).map(|f| f.as_slice())
    }

    /// Get number of cached CAN-IDs
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Get all frames (for debugging)
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &CanFrameData)> {
        self.frames.iter()
    }
}

/// LYNK Serial CAN protocol CAN-IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum LynkCanId {
    /// Battery Limits (1s period)
    BatteryLimits = 0x351,
    /// Battery Capacity Information (1s period)
    BatteryCapacity = 0x354,
    /// Battery Status (SOC/SOH) (1s period)
    BatteryStatus = 0x355,
    /// Battery Measurements (voltage/current/temp) (1s period)
    BatteryMeasurements = 0x356,
    /// Battery Alarms & Warnings (1s period)
    BatteryAlarms = 0x35A,
    /// Manufacturer Name ASCII (10s period)
    ManufacturerName = 0x35E,
    /// Model Name Upper ASCII (10s period)
    ModelNameUpper = 0x370,
    /// Model Name Lower ASCII (10s period)
    ModelNameLower = 0x371,
    /// Firmware Version (10s period)
    FirmwareVersion = 0x372,
    /// Protocol Version (10s period)
    ProtocolVersion = 0x373,
}

impl LynkCanId {
    /// Convert to u32
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    /// Try to create from u32
    pub fn from_u32(id: u32) -> Option<Self> {
        match id {
            0x351 => Some(Self::BatteryLimits),
            0x354 => Some(Self::BatteryCapacity),
            0x355 => Some(Self::BatteryStatus),
            0x356 => Some(Self::BatteryMeasurements),
            0x35A => Some(Self::BatteryAlarms),
            0x35E => Some(Self::ManufacturerName),
            0x370 => Some(Self::ModelNameUpper),
            0x371 => Some(Self::ModelNameLower),
            0x372 => Some(Self::FirmwareVersion),
            0x373 => Some(Self::ProtocolVersion),
            _ => None,
        }
    }

    /// Check if this is a LYNK protocol CAN-ID
    pub fn is_lynk_id(id: u32) -> bool {
        Self::from_u32(id).is_some()
    }

    /// Get description
    pub fn description(self) -> &'static str {
        match self {
            Self::BatteryLimits => "Battery Limits",
            Self::BatteryCapacity => "Battery Capacity Information",
            Self::BatteryStatus => "Battery Status",
            Self::BatteryMeasurements => "Battery Measurements",
            Self::BatteryAlarms => "Battery Alarms & Warnings",
            Self::ManufacturerName => "Manufacturer Name",
            Self::ModelNameUpper => "Model Name Upper",
            Self::ModelNameLower => "Model Name Lower",
            Self::FirmwareVersion => "Firmware Version",
            Self::ProtocolVersion => "Protocol Version",
        }
    }
}
