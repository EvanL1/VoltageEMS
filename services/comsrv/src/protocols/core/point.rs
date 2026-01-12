//! Point configuration model.
//!
//! This module defines protocol-agnostic point configuration,
//! with protocol-specific address types for each supported protocol.

use serde::{Deserialize, Serialize};

use crate::protocols::core::error::GatewayError;

/// Protocol-agnostic point configuration.
///
/// This is a pure protocol-layer type. It does NOT contain SCADA-level
/// categorization (Telemetry/Signal/Control/Adjustment). The application
/// layer determines point type based on its own configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointConfig {
    /// Unique point identifier (numeric).
    pub id: u32,

    /// Human-readable name (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Protocol-specific address.
    pub address: ProtocolAddress,

    /// Data transformation configuration.
    #[serde(default)]
    pub transform: TransformConfig,

    /// Polling group (for batch optimization).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_group: Option<String>,

    /// Whether this point is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl PointConfig {
    /// Create a new point configuration.
    pub fn new(id: u32, address: ProtocolAddress) -> Self {
        Self {
            id,
            name: None,
            address,
            transform: TransformConfig::default(),
            poll_group: None,
            enabled: true,
        }
    }

    /// Set the point name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the transform configuration.
    #[must_use]
    pub fn with_transform(mut self, transform: TransformConfig) -> Self {
        self.transform = transform;
        self
    }

    /// Set the poll group.
    #[must_use]
    pub fn with_poll_group(mut self, group: impl Into<String>) -> Self {
        self.poll_group = Some(group.into());
        self
    }
}

/// Protocol-specific address configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "protocol", content = "params")]
pub enum ProtocolAddress {
    /// Modbus address.
    Modbus(ModbusAddress),

    /// IEC 60870-5-104 address.
    Iec104(Iec104Address),

    /// OPC UA address.
    OpcUa(OpcUaAddress),

    /// DNP3 address.
    Dnp3(Dnp3Address),

    /// Virtual channel address (no physical device).
    Virtual(VirtualAddress),

    /// GPIO address (for DI/DO hardware control).
    #[cfg(feature = "gpio")]
    Gpio(GpioAddress),

    /// Generic string address (for custom protocols).
    Generic(String),
}

/// Virtual channel address.
///
/// Used for points that don't connect to a physical device.
/// Virtual points serve as data aggregation/relay points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualAddress {
    /// Logical group name (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    /// Tag/identifier within the group.
    pub tag: String,
}

impl VirtualAddress {
    /// Create a simple virtual address.
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            group: None,
            tag: tag.into(),
        }
    }

    /// Create a grouped virtual address.
    pub fn grouped(group: impl Into<String>, tag: impl Into<String>) -> Self {
        Self {
            group: Some(group.into()),
            tag: tag.into(),
        }
    }
}

/// GPIO pin direction.
#[cfg(feature = "gpio")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpioDirection {
    /// Input pin (DI - Digital Input).
    Input,
    /// Output pin (DO - Digital Output).
    Output,
}

/// GPIO address for hardware DI/DO control.
#[cfg(feature = "gpio")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioAddress {
    /// GPIO chip name (e.g., "gpiochip0").
    pub chip: String,

    /// Pin number/offset.
    pub pin: u32,

    /// Pin direction.
    pub direction: GpioDirection,

    /// Active low (invert logic).
    #[serde(default)]
    pub active_low: bool,
}

#[cfg(feature = "gpio")]
impl GpioAddress {
    /// Create a digital input address.
    pub fn digital_input(chip: impl Into<String>, pin: u32) -> Self {
        Self {
            chip: chip.into(),
            pin,
            direction: GpioDirection::Input,
            active_low: false,
        }
    }

    /// Create a digital output address.
    pub fn digital_output(chip: impl Into<String>, pin: u32) -> Self {
        Self {
            chip: chip.into(),
            pin,
            direction: GpioDirection::Output,
            active_low: false,
        }
    }

    /// Set active low mode.
    #[must_use]
    pub fn with_active_low(mut self, active_low: bool) -> Self {
        self.active_low = active_low;
        self
    }
}

/// Modbus point address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusAddress {
    /// Slave/unit ID.
    pub slave_id: u8,

    /// Function code (1-4 for read, 5-16 for write).
    pub function_code: u8,

    /// Register address (0-based).
    pub register: u16,

    /// Data format.
    #[serde(default)]
    pub format: DataFormat,

    /// Byte order for multi-byte values.
    #[serde(default)]
    pub byte_order: ByteOrder,

    /// Bit position for boolean values (0-15).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_position: Option<u8>,
}

impl ModbusAddress {
    /// Create a holding register address (FC03).
    pub fn holding_register(slave_id: u8, register: u16, format: DataFormat) -> Self {
        Self {
            slave_id,
            function_code: 3,
            register,
            format,
            byte_order: ByteOrder::default(),
            bit_position: None,
        }
    }

    /// Create an input register address (FC04).
    pub fn input_register(slave_id: u8, register: u16, format: DataFormat) -> Self {
        Self {
            slave_id,
            function_code: 4,
            register,
            format,
            byte_order: ByteOrder::default(),
            bit_position: None,
        }
    }

    /// Create a coil address (FC01).
    pub fn coil(slave_id: u8, register: u16) -> Self {
        Self {
            slave_id,
            function_code: 1,
            register,
            format: DataFormat::Bool,
            byte_order: ByteOrder::default(),
            bit_position: None,
        }
    }

    /// Create a discrete input address (FC02).
    pub fn discrete_input(slave_id: u8, register: u16) -> Self {
        Self {
            slave_id,
            function_code: 2,
            register,
            format: DataFormat::Bool,
            byte_order: ByteOrder::default(),
            bit_position: None,
        }
    }

    /// Get the number of registers to read based on format.
    pub fn register_count(&self) -> u16 {
        self.format.register_count()
    }
}

/// IEC 60870-5-104 address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iec104Address {
    /// Information Object Address (IOA).
    pub ioa: u32,

    /// Type Identifier.
    pub type_id: u8,

    /// Common Address of ASDU.
    pub common_address: u16,
}

impl Iec104Address {
    /// Create a new IEC 104 address.
    pub fn new(ioa: u32, type_id: u8, common_address: u16) -> Self {
        Self {
            ioa,
            type_id,
            common_address,
        }
    }
}

/// OPC UA address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpcUaAddress {
    /// Node ID (string format).
    pub node_id: String,

    /// Namespace index.
    #[serde(default)]
    pub namespace_index: u16,
}

impl OpcUaAddress {
    /// Create a new OPC UA address.
    pub fn new(node_id: impl Into<String>, namespace_index: u16) -> Self {
        Self {
            node_id: node_id.into(),
            namespace_index,
        }
    }
}

/// DNP3 address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dnp3Address {
    /// Point type.
    pub point_type: Dnp3PointType,

    /// Point index.
    pub index: u16,
}

/// DNP3 point types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dnp3PointType {
    /// Binary Input.
    BinaryInput,
    /// Binary Output.
    BinaryOutput,
    /// Analog Input.
    AnalogInput,
    /// Analog Output.
    AnalogOutput,
    /// Counter.
    Counter,
}

/// Data format for protocol values.
///
/// Supports multiple serde aliases for flexibility in JSON configs:
/// - `uint16` / `u16`, `int16` / `i16`, etc.
/// - `float32` / `f32` / `float`, `float64` / `f64` / `double`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataFormat {
    /// Boolean.
    #[serde(alias = "boolean")]
    Bool,
    /// Unsigned 16-bit integer.
    #[default]
    #[serde(alias = "u16")]
    UInt16,
    /// Signed 16-bit integer.
    #[serde(alias = "i16")]
    Int16,
    /// Unsigned 32-bit integer.
    #[serde(alias = "u32")]
    UInt32,
    /// Signed 32-bit integer.
    #[serde(alias = "i32")]
    Int32,
    /// Unsigned 64-bit integer.
    #[serde(alias = "u64")]
    UInt64,
    /// Signed 64-bit integer.
    #[serde(alias = "i64")]
    Int64,
    /// 32-bit floating point.
    #[serde(alias = "f32", alias = "float")]
    Float32,
    /// 64-bit floating point.
    #[serde(alias = "f64", alias = "double")]
    Float64,
    /// String (fixed length).
    String,
}

impl DataFormat {
    /// Get the number of 16-bit registers needed for this format.
    pub fn register_count(&self) -> u16 {
        match self {
            Self::Bool | Self::UInt16 | Self::Int16 => 1,
            Self::UInt32 | Self::Int32 | Self::Float32 => 2,
            Self::UInt64 | Self::Int64 | Self::Float64 => 4,
            Self::String => 8, // Default 16 characters
        }
    }

    /// Get the byte size of this format.
    pub fn byte_size(&self) -> usize {
        match self {
            Self::Bool => 1,
            Self::UInt16 | Self::Int16 => 2,
            Self::UInt32 | Self::Int32 | Self::Float32 => 4,
            Self::UInt64 | Self::Int64 | Self::Float64 => 8,
            Self::String => 16, // Default
        }
    }
}

/// Byte order for multi-byte values.
///
/// Supports multiple serde aliases for flexibility in JSON configs:
/// - `ABCD` / `BIG_ENDIAN` / `BE` / `big_endian`
/// - `DCBA` / `LITTLE_ENDIAN` / `LE` / `little_endian`
/// - `BADC` / `WORD_SWAP`, `CDAB` / `BYTE_SWAP`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ByteOrder {
    /// Big-endian (ABCD) - network byte order.
    #[default]
    #[serde(alias = "big_endian", alias = "BIG_ENDIAN", alias = "BE")]
    Abcd,

    /// Little-endian (DCBA).
    #[serde(alias = "little_endian", alias = "LITTLE_ENDIAN", alias = "LE")]
    Dcba,

    /// Mid-big-endian (BADC) - word swap.
    #[serde(alias = "WORD_SWAP", alias = "word_swap")]
    Badc,

    /// Mid-little-endian (CDAB) - byte swap.
    #[serde(alias = "BYTE_SWAP", alias = "byte_swap")]
    Cdab,
}

impl ByteOrder {
    /// Get the byte order string for debugging.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Abcd => "ABCD",
            Self::Dcba => "DCBA",
            Self::Badc => "BADC",
            Self::Cdab => "CDAB",
        }
    }
}

/// Data transformation configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransformConfig {
    /// Scale factor: result = raw * scale + offset.
    #[serde(default = "default_scale")]
    pub scale: f64,

    /// Offset: result = raw * scale + offset.
    #[serde(default)]
    pub offset: f64,

    /// Reverse boolean value (for signals/controls).
    #[serde(default)]
    pub reverse: bool,

    /// Deadband for change detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadband: Option<f64>,

    /// Minimum valid value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_value: Option<f64>,

    /// Maximum valid value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_value: Option<f64>,
}

fn default_scale() -> f64 {
    1.0
}

impl TransformConfig {
    /// Create a simple linear transform.
    pub fn linear(scale: f64, offset: f64) -> Self {
        Self {
            scale,
            offset,
            ..Default::default()
        }
    }

    /// Apply the transform to a raw value.
    pub fn apply(&self, raw: f64) -> f64 {
        raw * self.scale + self.offset
    }

    /// Apply reverse transform to get raw value.
    ///
    /// Returns an error if `scale` is zero (division by zero).
    pub fn reverse_apply(&self, value: f64) -> Result<f64, GatewayError> {
        if self.scale == 0.0 {
            return Err(GatewayError::DataConversion(
                "Cannot reverse transform: scale is zero".into(),
            ));
        }
        Ok((value - self.offset) / self.scale)
    }

    /// Apply boolean reverse if configured.
    pub fn apply_bool(&self, raw: bool) -> bool {
        if self.reverse {
            !raw
        } else {
            raw
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // unwrap in tests
mod tests {
    use super::*;

    #[test]
    fn test_modbus_address() {
        let addr = ModbusAddress::holding_register(1, 100, DataFormat::Float32);
        assert_eq!(addr.slave_id, 1);
        assert_eq!(addr.function_code, 3);
        assert_eq!(addr.register, 100);
        assert_eq!(addr.register_count(), 2);
    }

    #[test]
    fn test_transform() {
        let t = TransformConfig::linear(0.1, 10.0);
        assert_eq!(t.apply(100.0), 20.0); // 100 * 0.1 + 10 = 20
        assert_eq!(t.reverse_apply(20.0).unwrap(), 100.0);
    }

    #[test]
    fn test_transform_zero_scale() {
        let t = TransformConfig::linear(0.0, 10.0);
        assert!(t.reverse_apply(20.0).is_err());
    }

    #[test]
    fn test_data_format_register_count() {
        assert_eq!(DataFormat::UInt16.register_count(), 1);
        assert_eq!(DataFormat::Float32.register_count(), 2);
        assert_eq!(DataFormat::Float64.register_count(), 4);
    }
}
