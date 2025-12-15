//! Shared protocol types for VoltageEMS services
//!
//! This module provides unified protocol-related types used across all services,
//! ensuring consistency in data representation and protocol handling.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// Re-export PointType from voltage-model for convenience
pub use voltage_model::PointType;

/// FourRemote is an alias for PointType for backward compatibility
///
/// Both represent the same concept: the four remote point types (T/S/C/A)
/// in industrial SCADA systems.
///
/// **Prefer using `PointType` for new code.**
pub type FourRemote = PointType;

// Re-export ByteOrder from bytes module for unified access
pub use crate::bytes::ByteOrder;

// ============================================================================
// Data Type Definitions
// ============================================================================

/// Universal signal data type for all protocols
///
/// This enum represents all possible data types that can be transmitted
/// through various industrial protocols (Modbus, CAN, IEC, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalDataType {
    // Unsigned integers
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    // Signed integers
    Int8,
    Int16,
    Int32,
    Int64,
    // Floating point
    Float32,
    Float64,
    // Special types
    Boolean,
    String,
    Bytes,
}

impl SignalDataType {
    /// Parse from string representation (case-insensitive)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "uint8" | "u8" => Some(Self::UInt8),
            "uint16" | "u16" => Some(Self::UInt16),
            "uint32" | "u32" => Some(Self::UInt32),
            "uint64" | "u64" => Some(Self::UInt64),
            "int8" | "i8" => Some(Self::Int8),
            "int16" | "i16" => Some(Self::Int16),
            "int32" | "i32" => Some(Self::Int32),
            "int64" | "i64" => Some(Self::Int64),
            "float32" | "f32" | "float" => Some(Self::Float32),
            "float64" | "f64" | "double" => Some(Self::Float64),
            "boolean" | "bool" => Some(Self::Boolean),
            "string" | "str" => Some(Self::String),
            "bytes" | "raw" => Some(Self::Bytes),
            _ => None,
        }
    }

    /// Get size in bytes for fixed-size types
    pub fn size_bytes(&self) -> Option<usize> {
        match self {
            Self::UInt8 | Self::Int8 => Some(1),
            Self::UInt16 | Self::Int16 => Some(2),
            Self::UInt32 | Self::Int32 | Self::Float32 => Some(4),
            Self::UInt64 | Self::Int64 | Self::Float64 => Some(8),
            Self::Boolean => Some(1),
            Self::String | Self::Bytes => None, // Variable size
        }
    }

    /// Check if this is a numeric type
    pub fn is_numeric(&self) -> bool {
        !matches!(self, Self::String | Self::Bytes)
    }

    /// Check if this is an integer type
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::UInt8
                | Self::UInt16
                | Self::UInt32
                | Self::UInt64
                | Self::Int8
                | Self::Int16
                | Self::Int32
                | Self::Int64
        )
    }

    /// Check if this is a floating-point type
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float32 | Self::Float64)
    }
}

impl fmt::Display for SignalDataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::UInt8 => "uint8",
            Self::UInt16 => "uint16",
            Self::UInt32 => "uint32",
            Self::UInt64 => "uint64",
            Self::Int8 => "int8",
            Self::Int16 => "int16",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
            Self::Boolean => "boolean",
            Self::String => "string",
            Self::Bytes => "bytes",
        };
        write!(f, "{}", s)
    }
}

// ============================================================================
// Protocol Parameter Types
// ============================================================================

/// Protocol configuration parameter type
///
/// Used for defining and validating protocol-specific configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParameterType {
    String {
        #[serde(default)]
        min_length: Option<usize>,
        #[serde(default)]
        max_length: Option<usize>,
        #[serde(default)]
        pattern: Option<String>,
    },
    Integer {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
    },
    Float {
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
    },
    Boolean,
    Enum {
        values: Vec<EnumValue>,
    },
    Array {
        item_type: Box<ParameterType>,
        #[serde(default)]
        min_items: Option<usize>,
        #[serde(default)]
        max_items: Option<usize>,
    },
    Object {
        properties: HashMap<String, ParameterType>,
    },
    Duration {
        #[serde(default)]
        unit: DurationUnit,
    },
    IpAddress {
        #[serde(default)]
        version: Option<IpVersion>,
    },
    Port,
    FilePath {
        #[serde(default)]
        must_exist: bool,
    },
    Secret,
}

/// Enum value for parameter type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Duration unit specification
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DurationUnit {
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

impl Default for DurationUnit {
    fn default() -> Self {
        Self::Seconds
    }
}

/// IP version specification
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IpVersion {
    #[serde(rename = "v4")]
    V4,
    #[serde(rename = "v6")]
    V6,
    #[serde(rename = "any")]
    Any,
}

// ============================================================================
// Protocol Types
// ============================================================================

/// Supported protocol types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    // Modbus variants
    ModbusTcp,
    ModbusRtu,
    ModbusAscii,
    // IEC protocols
    Iec60870_5_104,
    Iec61850,
    // Other protocols
    Mqtt,
    Opcua,
    Bacnet,
    Dnp3,
    Virtual,
    Grpc,
}

impl ProtocolType {
    /// Parse from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "modbus_tcp" => Some(Self::ModbusTcp),
            "modbus_rtu" => Some(Self::ModbusRtu),
            "modbus_ascii" => Some(Self::ModbusAscii),
            "iec60870_5_104" | "iec104" => Some(Self::Iec60870_5_104),
            "iec61850" => Some(Self::Iec61850),
            "mqtt" => Some(Self::Mqtt),
            "opcua" | "opc_ua" => Some(Self::Opcua),
            "bacnet" => Some(Self::Bacnet),
            "dnp3" => Some(Self::Dnp3),
            "virtual" => Some(Self::Virtual),
            "grpc" => Some(Self::Grpc),
            _ => None,
        }
    }

    /// Check if this is a Modbus protocol
    pub fn is_modbus(&self) -> bool {
        matches!(self, Self::ModbusTcp | Self::ModbusRtu | Self::ModbusAscii)
    }

    /// Check if this is an IEC protocol
    pub fn is_iec(&self) -> bool {
        matches!(self, Self::Iec60870_5_104 | Self::Iec61850)
    }

    /// Check if this protocol supports server mode
    pub fn supports_server(&self) -> bool {
        matches!(
            self,
            Self::ModbusTcp | Self::Grpc | Self::Mqtt | Self::Opcua
        )
    }

    /// Check if this protocol supports client mode
    pub fn supports_client(&self) -> bool {
        !matches!(self, Self::Virtual) // Virtual is special case
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ModbusTcp => "modbus_tcp",
            Self::ModbusRtu => "modbus_rtu",
            Self::ModbusAscii => "modbus_ascii",
            Self::Iec60870_5_104 => "iec60870_5_104",
            Self::Iec61850 => "iec61850",
            Self::Mqtt => "mqtt",
            Self::Opcua => "opcua",
            Self::Bacnet => "bacnet",
            Self::Dnp3 => "dnp3",
            Self::Virtual => "virtual",
            Self::Grpc => "grpc",
        }
    }
}

impl fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ProtocolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown protocol type: {}", s))
    }
}

// ============================================================================
// Protocol Communication Modes
// ============================================================================

/// Communication mode for protocol channels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommunicationMode {
    /// Act as client/master
    Client,
    /// Act as server/slave
    Server,
    /// Bidirectional peer-to-peer
    Peer,
}

impl Default for CommunicationMode {
    fn default() -> Self {
        Self::Client
    }
}

// ============================================================================
// Protocol Quality Codes
// ============================================================================

/// Data quality codes for protocol values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityCode {
    Good,
    Bad,
    Uncertain,
    Invalid,
    NotConnected,
    DeviceFailure,
    SensorFailure,
    LastKnownValue,
    CommFailure,
    OutOfService,
    WaitingForInitialData,
}

impl QualityCode {
    /// Check if the quality is acceptable for normal operations
    pub fn is_good(&self) -> bool {
        matches!(self, Self::Good)
    }

    /// Check if the quality indicates a problem
    pub fn is_bad(&self) -> bool {
        matches!(
            self,
            Self::Bad
                | Self::Invalid
                | Self::NotConnected
                | Self::DeviceFailure
                | Self::SensorFailure
                | Self::CommFailure
                | Self::OutOfService
        )
    }

    /// Convert to numeric code (for some protocols)
    pub fn to_code(&self) -> u8 {
        match self {
            Self::Good => 0,
            Self::Uncertain => 64,
            Self::Bad => 128,
            Self::Invalid => 129,
            Self::NotConnected => 130,
            Self::DeviceFailure => 131,
            Self::SensorFailure => 132,
            Self::LastKnownValue => 133,
            Self::CommFailure => 134,
            Self::OutOfService => 135,
            Self::WaitingForInitialData => 136,
        }
    }
}

impl Default for QualityCode {
    fn default() -> Self {
        Self::WaitingForInitialData
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_signal_data_type_parsing() {
        assert_eq!(
            SignalDataType::parse("uint32"),
            Some(SignalDataType::UInt32)
        );
        assert_eq!(SignalDataType::parse("U32"), Some(SignalDataType::UInt32));
        assert_eq!(
            SignalDataType::parse("float"),
            Some(SignalDataType::Float32)
        );
        assert_eq!(SignalDataType::parse("invalid"), None);
    }

    #[test]
    fn test_protocol_type_categorization() {
        assert!(ProtocolType::ModbusTcp.is_modbus());
        assert!(ProtocolType::Iec61850.is_iec());
        assert!(ProtocolType::ModbusTcp.supports_server());
    }

    #[test]
    fn test_quality_code_checks() {
        assert!(QualityCode::Good.is_good());
        assert!(!QualityCode::Good.is_bad());
        assert!(QualityCode::DeviceFailure.is_bad());
        assert!(!QualityCode::Uncertain.is_good());
    }

    #[test]
    fn test_point_type_parsing() {
        assert_eq!(PointType::from_str("T"), Some(PointType::Telemetry));
        assert_eq!(PointType::from_str("S"), Some(PointType::Signal));
        assert_eq!(PointType::from_str("C"), Some(PointType::Control));
        assert_eq!(PointType::from_str("A"), Some(PointType::Adjustment));
        assert_eq!(PointType::from_str("X"), None);
    }

    #[test]
    fn test_point_type_as_str() {
        assert_eq!(PointType::Telemetry.as_str(), "T");
        assert_eq!(PointType::Signal.as_str(), "S");
        assert_eq!(PointType::Control.as_str(), "C");
        assert_eq!(PointType::Adjustment.as_str(), "A");
    }

    #[test]
    fn test_point_type_categorization() {
        // Measurement types
        assert!(PointType::Telemetry.is_measurement());
        assert!(PointType::Signal.is_measurement());
        assert!(!PointType::Control.is_measurement());
        assert!(!PointType::Adjustment.is_measurement());

        // Action types
        assert!(!PointType::Telemetry.is_action());
        assert!(!PointType::Signal.is_action());
        assert!(PointType::Control.is_action());
        assert!(PointType::Adjustment.is_action());

        // Analog types
        assert!(PointType::Telemetry.is_analog());
        assert!(!PointType::Signal.is_analog());
        assert!(!PointType::Control.is_analog());
        assert!(PointType::Adjustment.is_analog());

        // Digital types
        assert!(!PointType::Telemetry.is_digital());
        assert!(PointType::Signal.is_digital());
        assert!(PointType::Control.is_digital());
        assert!(!PointType::Adjustment.is_digital());
    }

    #[test]
    fn test_point_type_display() {
        assert_eq!(format!("{}", PointType::Telemetry), "T");
        assert_eq!(format!("{}", PointType::Signal), "S");
        assert_eq!(format!("{}", PointType::Control), "C");
        assert_eq!(format!("{}", PointType::Adjustment), "A");
    }

    #[test]
    fn test_byte_order_serde() {
        // Test deserialization with different aliases
        let json_abcd = r#""ABCD""#;
        let bo: ByteOrder = serde_json::from_str(json_abcd).unwrap();
        assert_eq!(bo, ByteOrder::BigEndian);

        let json_dcba = r#""DCBA""#;
        let bo: ByteOrder = serde_json::from_str(json_dcba).unwrap();
        assert_eq!(bo, ByteOrder::LittleEndian);

        let json_cdab = r#""CDAB""#;
        let bo: ByteOrder = serde_json::from_str(json_cdab).unwrap();
        assert_eq!(bo, ByteOrder::BigEndianSwap);
    }
}
