//! Protocol mapping types - Unified and simplified

use crate::utils::error::{ComSrvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified point mapping structure for all protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPointMapping {
    /// Point ID (must match four telemetry table)
    pub point_id: u32,

    /// Signal name
    pub signal_name: String,

    /// Protocol type
    pub protocol_type: ProtocolType,

    /// Protocol-specific address
    pub address: ProtocolAddress,

    /// Data type
    pub data_type: DataType,

    /// Scaling configuration
    pub scaling: Option<ScalingConfig>,

    /// Validation configuration
    pub validation: Option<ValidationConfig>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

// FourTelemetryPoint definition moved to channel.rs to avoid conflicts

/// Protocol type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProtocolType {
    ModbusTcp,
    ModbusRtu,
    Can,
    Iec104,
    Virtual,
    Dio,
    Iec61850,
}

/// Telemetry type enumeration (四遥类型)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TelemetryType {
    /// YC - 遥测 (Telemetry)
    Telemetry,
    /// YX - 遥信 (Signal)
    Signal,
    /// YT - 遥调 (Adjustment)
    Adjustment,
    /// YK - 遥控 (Control)
    Control,
}

/// Protocol address enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProtocolAddress {
    Modbus {
        slave_id: u8,
        function_code: u8,
        register: u16,
        bit: Option<u8>,
    },
    Can {
        can_id: u32,
        start_byte: u8,
        length: u8,
        bit: Option<u8>,
    },
    Iec104 {
        ioa: u32,
        ca: u16,
        type_id: u8,
    },
    Virtual {
        address: String,
    },
}

/// Data type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataType {
    Bool,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Float32,
    Float64,
    String,
}

/// Scaling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingConfig {
    /// Scale factor
    pub scale: f64,

    /// Offset
    pub offset: f64,

    /// Engineering unit
    pub unit: Option<String>,
}

/// Validation configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationConfig {
    /// Minimum value
    pub min: Option<f64>,

    /// Maximum value
    pub max: Option<f64>,

    /// Valid range check
    pub range_check: bool,
}

/// Data format for Modbus protocol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataFormat {
    Float32,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Bool,
}

/// Byte order for multi-byte data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ByteOrder {
    /// Big endian (ABCD)
    ABCD,
    /// Little endian (DCBA)
    DCBA,
    /// Big endian word swap (BADC)
    BADC,
    /// Little endian word swap (CDAB)
    CDAB,

    AB,

    BA,

    ABCDEFGH,

    HGFEDCBA,
}

// CombinedPoint definition moved to channel.rs to avoid conflicts

// Implementation of utility methods
impl UnifiedPointMapping {
    /// Validate the point mapping
    pub fn validate(&self) -> Result<()> {
        if self.signal_name.is_empty() {
            return Err(ComSrvError::ConfigError(
                "Signal name cannot be empty".to_string(),
            ));
        }

        self.address.validate()?;

        Ok(())
    }

    /// Convert raw value to engineering value
    pub fn convert_to_engineering(&self, raw_value: f64) -> f64 {
        if let Some(scaling) = &self.scaling {
            raw_value * scaling.scale + scaling.offset
        } else {
            raw_value
        }
    }

    /// Convert engineering value to raw value
    pub fn convert_from_engineering(&self, engineering_value: f64) -> f64 {
        if let Some(scaling) = &self.scaling {
            (engineering_value - scaling.offset) / scaling.scale
        } else {
            engineering_value
        }
    }
}

impl ProtocolAddress {
    /// Validate the protocol address
    pub fn validate(&self) -> Result<()> {
        match self {
            ProtocolAddress::Modbus { function_code, .. } => {
                if ![1, 2, 3, 4, 5, 6, 15, 16].contains(function_code) {
                    return Err(ComSrvError::ConfigError(format!(
                        "Invalid Modbus function code: {function_code}"
                    )));
                }
            }
            ProtocolAddress::Can { can_id, .. } => {
                if *can_id > 0x1FFFFFFF {
                    return Err(ComSrvError::ConfigError(format!(
                        "Invalid CAN ID: 0x{:08X}",
                        can_id
                    )));
                }
            }
            ProtocolAddress::Iec104 { .. } => {
                // Add IEC104 specific validation if needed
            }
            ProtocolAddress::Virtual { address } => {
                if address.is_empty() {
                    return Err(ComSrvError::ConfigError(
                        "Virtual address cannot be empty".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Get address as string for display
    pub fn address_string(&self) -> String {
        match self {
            ProtocolAddress::Modbus {
                slave_id,
                function_code,
                register,
                bit,
            } => {
                if let Some(bit) = bit {
                    format!("{}:{}:{}:{bit}", slave_id, function_code, register)
                } else {
                    format!("{}:{}:{register}", slave_id, function_code)
                }
            }
            ProtocolAddress::Can {
                can_id,
                start_byte,
                length,
                ..
            } => {
                format!("0x{:08X}:{}:{length}", can_id, start_byte)
            }
            ProtocolAddress::Iec104 { ioa, ca, type_id } => {
                format!("{}:{}:{type_id}", ca, ioa)
            }
            ProtocolAddress::Virtual { address } => address.clone(),
        }
    }
}

// FourTelemetryPoint impl moved to channel.rs

impl Default for ScalingConfig {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset: 0.0,
            unit: None,
        }
    }
}

impl ProtocolType {
    /// Convert to string representation for protocol identification

    /// Parse from string
    pub fn parse_protocol_type(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "modbus_tcp" | "modbustcp" => Ok(ProtocolType::ModbusTcp),
            "modbus_rtu" | "modbusrtu" => Ok(ProtocolType::ModbusRtu),
            "can" => Ok(ProtocolType::Can),
            "iec104" => Ok(ProtocolType::Iec104),
            "virtual" => Ok(ProtocolType::Virtual),
            "dio" => Ok(ProtocolType::Dio),
            "iec61850" => Ok(ProtocolType::Iec61850),
            _ => Err(ComSrvError::ConfigError(format!(
                "Unknown protocol type: {s}"
            ))),
        }
    }

    /// Get string representation (borrowed)
    pub fn as_str(&self) -> &str {
        match self {
            ProtocolType::ModbusTcp => "modbus_tcp",
            ProtocolType::ModbusRtu => "modbus_rtu",
            ProtocolType::Can => "can",
            ProtocolType::Iec104 => "iec104",
            ProtocolType::Virtual => "virtual",
            ProtocolType::Dio => "dio",
            ProtocolType::Iec61850 => "iec61850",
        }
    }
}

impl std::fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
