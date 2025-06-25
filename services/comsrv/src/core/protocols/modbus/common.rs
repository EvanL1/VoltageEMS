//! Modbus Protocol Common Definitions
//!
//! This module contains common data structures, enumerations, and utilities
//! used across different Modbus protocol implementations. It provides the
//! foundational types for Modbus communication, including function codes,
//! register types, data types, and mapping configurations.
//!
//! # Features
//!
//! - Modbus function code definitions
//! - Register type classifications (coil, discrete input, holding register, input register)
//! - Data type definitions with proper register count calculations
//! - Register mapping configurations for point definitions
//! - Byte order handling for multi-register values
//! - CRC16 calculation for Modbus RTU communication
//!
//! # Examples
//!
//! ```rust
//! use comsrv::core::protocols::modbus::common::*;
//!
//! // Create a register mapping
//! let mapping = ModbusRegisterMapping {
//!     name: "temperature".to_string(),
//!     register_type: ModbusRegisterType::HoldingRegister,
//!     address: 100,
//!     data_type: ModbusDataType::Float32,
//!     scale: 0.1,
//!     offset: 0.0,
//!     ..Default::default()
//! };
//!
//! // Calculate CRC for Modbus RTU
//! let data = &[0x01, 0x03, 0x00, 0x00, 0x00, 0x02];
//! let crc = crc16_modbus(data);
//! ```

use serde::{Deserialize, Serialize};

/// Modbus function code enumeration
///
/// Represents the standard Modbus function codes defined in the Modbus specification.
/// Function codes determine the type of operation to be performed on Modbus registers.
///
/// # Standard Function Codes
///
/// - **0x01**: Read Coils - Read 1-2000 contiguous coils
/// - **0x02**: Read Discrete Inputs - Read 1-2000 contiguous discrete inputs  
/// - **0x03**: Read Holding Registers - Read 1-125 contiguous holding registers
/// - **0x04**: Read Input Registers - Read 1-125 contiguous input registers
/// - **0x05**: Write Single Coil - Write a single coil
/// - **0x06**: Write Single Register - Write a single holding register
/// - **0x0F**: Write Multiple Coils - Write multiple coils
/// - **0x10**: Write Multiple Registers - Write multiple holding registers
///
/// # Custom Function Codes
///
/// The `Custom(u8)` variant allows for vendor-specific or non-standard function codes.
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::ModbusFunctionCode;
///
/// let read_holding = ModbusFunctionCode::ReadHoldingRegisters;
/// let code: u8 = read_holding.into();
/// assert_eq!(code, 0x03);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModbusFunctionCode {
    /// Read coils (0x01) - Read 1-2000 contiguous coils
    ReadCoils = 0x01,
    /// Read discrete inputs (0x02) - Read 1-2000 contiguous discrete inputs
    ReadDiscreteInputs = 0x02,
    /// Read holding registers (0x03) - Read 1-125 contiguous holding registers
    ReadHoldingRegisters = 0x03,
    /// Read input registers (0x04) - Read 1-125 contiguous input registers
    ReadInputRegisters = 0x04,
    /// Write single coil (0x05) - Write a single coil
    WriteSingleCoil = 0x05,
    /// Write single register (0x06) - Write a single holding register
    WriteSingleRegister = 0x06,
    /// Write multiple coils (0x0F) - Write multiple coils
    WriteMultipleCoils = 0x0F,
    /// Write multiple registers (0x10) - Write multiple holding registers
    WriteMultipleRegisters = 0x10,
    /// Custom function code for vendor-specific operations
    Custom(u8),
}

impl From<u8> for ModbusFunctionCode {
    /// Convert a raw byte value to a ModbusFunctionCode
    ///
    /// # Arguments
    ///
    /// * `code` - Raw function code byte
    ///
    /// # Returns
    ///
    /// Corresponding `ModbusFunctionCode`, or `Custom(code)` for non-standard codes
    fn from(code: u8) -> Self {
        match code {
            0x01 => ModbusFunctionCode::ReadCoils,
            0x02 => ModbusFunctionCode::ReadDiscreteInputs,
            0x03 => ModbusFunctionCode::ReadHoldingRegisters,
            0x04 => ModbusFunctionCode::ReadInputRegisters,
            0x05 => ModbusFunctionCode::WriteSingleCoil,
            0x06 => ModbusFunctionCode::WriteSingleRegister,
            0x0F => ModbusFunctionCode::WriteMultipleCoils,
            0x10 => ModbusFunctionCode::WriteMultipleRegisters,
            other => ModbusFunctionCode::Custom(other),
        }
    }
}

impl From<ModbusFunctionCode> for u8 {
    /// Convert a ModbusFunctionCode to its raw byte value
    ///
    /// # Returns
    ///
    /// Raw function code byte suitable for transmission over Modbus protocol
    fn from(code: ModbusFunctionCode) -> Self {
        match code {
            ModbusFunctionCode::ReadCoils => 0x01,
            ModbusFunctionCode::ReadDiscreteInputs => 0x02,
            ModbusFunctionCode::ReadHoldingRegisters => 0x03,
            ModbusFunctionCode::ReadInputRegisters => 0x04,
            ModbusFunctionCode::WriteSingleCoil => 0x05,
            ModbusFunctionCode::WriteSingleRegister => 0x06,
            ModbusFunctionCode::WriteMultipleCoils => 0x0F,
            ModbusFunctionCode::WriteMultipleRegisters => 0x10,
            ModbusFunctionCode::Custom(code) => code,
        }
    }
}

impl std::fmt::Display for ModbusFunctionCode {
    /// Format the function code for human-readable display
    ///
    /// # Returns
    ///
    /// Human-readable string representation including the hex code
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let name = match self {
            ModbusFunctionCode::ReadCoils => "Read Coils (0x01)",
            ModbusFunctionCode::ReadDiscreteInputs => "Read Discrete Inputs (0x02)",
            ModbusFunctionCode::ReadHoldingRegisters => "Read Holding Registers (0x03)",
            ModbusFunctionCode::ReadInputRegisters => "Read Input Registers (0x04)",
            ModbusFunctionCode::WriteSingleCoil => "Write Single Coil (0x05)",
            ModbusFunctionCode::WriteSingleRegister => "Write Single Register (0x06)",
            ModbusFunctionCode::WriteMultipleCoils => "Write Multiple Coils (0x0F)",
            ModbusFunctionCode::WriteMultipleRegisters => "Write Multiple Registers (0x10)",
            ModbusFunctionCode::Custom(code) => return write!(f, "Custom (0x{:02X})", code),
        };
        write!(f, "{}", name)
    }
}

/// Modbus register type classification
///
/// Modbus defines four types of registers, each with different characteristics
/// and addressing schemes. This enumeration provides a type-safe way to
/// distinguish between them.
///
/// # Register Types
///
/// - **Coil (0x series)**: Single-bit read/write registers for discrete outputs
/// - **Discrete Input (1x series)**: Single-bit read-only registers for discrete inputs  
/// - **Input Register (3x series)**: 16-bit read-only registers for analog inputs
/// - **Holding Register (4x series)**: 16-bit read/write registers for analog outputs
///
/// # Address Ranges
///
/// Traditional Modbus addressing uses different ranges for each register type:
/// - Coils: 00001-09999 (displayed as 0x0000-0x9999 in protocol)
/// - Discrete Inputs: 10001-19999 (displayed as 0x0000-0x9999 in protocol)
/// - Input Registers: 30001-39999 (displayed as 0x0000-0x9999 in protocol)
/// - Holding Registers: 40001-49999 (displayed as 0x0000-0x9999 in protocol)
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::{ModbusRegisterType, ModbusFunctionCode};
///
/// let reg_type = ModbusRegisterType::HoldingRegister;
/// assert!(reg_type.is_writable());
/// assert_eq!(reg_type.read_function_code(), ModbusFunctionCode::ReadHoldingRegisters);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ModbusRegisterType {
    /// Coil (0x series) - Single-bit read/write discrete output
    Coil,
    /// Discrete Input (1x series) - Single-bit read-only discrete input
    DiscreteInput,
    /// Input Register (3x series) - 16-bit read-only analog input
    InputRegister,
    /// Holding Register (4x series) - 16-bit read/write analog output/storage
    HoldingRegister,
}

impl ModbusRegisterType {
    /// Get the appropriate function code for reading this register type
    ///
    /// # Returns
    ///
    /// The Modbus function code used to read registers of this type
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::{ModbusRegisterType, ModbusFunctionCode};
    ///
    /// let holding_reg = ModbusRegisterType::HoldingRegister;
    /// assert_eq!(holding_reg.read_function_code(), ModbusFunctionCode::ReadHoldingRegisters);
    /// ```
    pub fn read_function_code(&self) -> ModbusFunctionCode {
        match self {
            ModbusRegisterType::Coil => ModbusFunctionCode::ReadCoils,
            ModbusRegisterType::DiscreteInput => ModbusFunctionCode::ReadDiscreteInputs,
            ModbusRegisterType::InputRegister => ModbusFunctionCode::ReadInputRegisters,
            ModbusRegisterType::HoldingRegister => ModbusFunctionCode::ReadHoldingRegisters,
        }
    }

    /// Get the appropriate function code for writing this register type
    ///
    /// # Arguments
    ///
    /// * `multiple` - Whether to write multiple registers (true) or single register (false)
    ///
    /// # Returns
    ///
    /// `Some(function_code)` if the register type is writable, `None` for read-only types
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::{ModbusRegisterType, ModbusFunctionCode};
    ///
    /// let holding_reg = ModbusRegisterType::HoldingRegister;
    /// assert_eq!(holding_reg.write_function_code(false), Some(ModbusFunctionCode::WriteSingleRegister));
    /// assert_eq!(holding_reg.write_function_code(true), Some(ModbusFunctionCode::WriteMultipleRegisters));
    ///
    /// let input_reg = ModbusRegisterType::InputRegister;
    /// assert_eq!(input_reg.write_function_code(false), None); // Read-only
    /// ```
    pub fn write_function_code(&self, multiple: bool) -> Option<ModbusFunctionCode> {
        match self {
            ModbusRegisterType::Coil => {
                if multiple {
                    Some(ModbusFunctionCode::WriteMultipleCoils)
                } else {
                    Some(ModbusFunctionCode::WriteSingleCoil)
                }
            }
            ModbusRegisterType::HoldingRegister => {
                if multiple {
                    Some(ModbusFunctionCode::WriteMultipleRegisters)
                } else {
                    Some(ModbusFunctionCode::WriteSingleRegister)
                }
            }
            // Discrete inputs and input registers are typically read-only
            ModbusRegisterType::DiscreteInput | ModbusRegisterType::InputRegister => None,
        }
    }

    /// Check if this register type supports write operations
    ///
    /// # Returns
    ///
    /// `true` if the register type can be written to, `false` for read-only types
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusRegisterType;
    ///
    /// assert!(ModbusRegisterType::Coil.is_writable());
    /// assert!(ModbusRegisterType::HoldingRegister.is_writable());
    /// assert!(!ModbusRegisterType::DiscreteInput.is_writable());
    /// assert!(!ModbusRegisterType::InputRegister.is_writable());
    /// ```
    pub fn is_writable(&self) -> bool {
        matches!(
            self,
            ModbusRegisterType::Coil | ModbusRegisterType::HoldingRegister
        )
    }

    /// Get the string representation of the register type
    ///
    /// # Returns
    ///
    /// String name suitable for configuration files and APIs
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusRegisterType;
    ///
    /// assert_eq!(ModbusRegisterType::HoldingRegister.as_str(), "holding_register");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            ModbusRegisterType::Coil => "coil",
            ModbusRegisterType::DiscreteInput => "discrete_input",
            ModbusRegisterType::InputRegister => "input_register",
            ModbusRegisterType::HoldingRegister => "holding_register",
        }
    }
}

/// Modbus data type definitions
///
/// Defines the data types that can be stored in Modbus registers, along with
/// their register count requirements and conversion characteristics.
///
/// # Supported Data Types
///
/// - **Bool**: Single bit value (1 register)
/// - **Int16/UInt16**: 16-bit integers (1 register)
/// - **Int32/UInt32**: 32-bit integers (2 registers)
/// - **Int64/UInt64**: 64-bit integers (4 registers)
/// - **Float32**: IEEE 754 single precision (2 registers)
/// - **Float64**: IEEE 754 double precision (4 registers)
/// - **String(n)**: Variable length string (⌈n/2⌉ registers)
///
/// # Register Usage
///
/// Each Modbus register is 16 bits. Multi-register data types require
/// consecutive registers and may be affected by byte order configuration.
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::ModbusDataType;
///
/// assert_eq!(ModbusDataType::Float32.register_count(), 2);
/// assert_eq!(ModbusDataType::String(10).register_count(), 5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusDataType {
    /// Boolean value (single bit, stored in 1 register)
    Bool,
    /// 16-bit signed integer (-32,768 to 32,767)
    Int16,
    /// 16-bit unsigned integer (0 to 65,535)
    UInt16,
    /// 32-bit signed integer (-2,147,483,648 to 2,147,483,647)
    Int32,
    /// 32-bit unsigned integer (0 to 4,294,967,295)
    UInt32,
    /// 64-bit signed integer
    Int64,
    /// 64-bit unsigned integer
    UInt64,
    /// 32-bit IEEE 754 floating point
    Float32,
    /// 64-bit IEEE 754 floating point
    Float64,
    /// Variable-length string with specified maximum byte count
    String(usize),
}

impl ModbusDataType {
    /// Get the number of Modbus registers required for this data type
    ///
    /// # Returns
    ///
    /// Number of 16-bit registers needed to store this data type
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusDataType;
    ///
    /// assert_eq!(ModbusDataType::UInt16.register_count(), 1);
    /// assert_eq!(ModbusDataType::Float32.register_count(), 2);
    /// assert_eq!(ModbusDataType::Int64.register_count(), 4);
    /// ```
    pub fn register_count(&self) -> u16 {
        match self {
            ModbusDataType::Bool | ModbusDataType::Int16 | ModbusDataType::UInt16 => 1,
            ModbusDataType::Int32 | ModbusDataType::UInt32 | ModbusDataType::Float32 => 2,
            ModbusDataType::Int64 | ModbusDataType::UInt64 | ModbusDataType::Float64 => 4,
            ModbusDataType::String(length) => (*length as u16 + 1) / 2, // 2 characters per register
        }
    }

    /// Get the string representation of the data type
    ///
    /// # Returns
    ///
    /// Human-readable string name of the data type
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusDataType;
    ///
    /// assert_eq!(ModbusDataType::Float32.as_str(), "float32");
    /// assert_eq!(ModbusDataType::String(20).as_str(), "string(20)");
    /// ```
    pub fn as_str(&self) -> String {
        match self {
            ModbusDataType::Bool => "bool".to_string(),
            ModbusDataType::Int16 => "int16".to_string(),
            ModbusDataType::UInt16 => "uint16".to_string(),
            ModbusDataType::Int32 => "int32".to_string(),
            ModbusDataType::UInt32 => "uint32".to_string(),
            ModbusDataType::Int64 => "int64".to_string(),
            ModbusDataType::UInt64 => "uint64".to_string(),
            ModbusDataType::Float32 => "float32".to_string(),
            ModbusDataType::Float64 => "float64".to_string(),
            ModbusDataType::String(length) => format!("string({})", length),
        }
    }

    /// Parse a data type from string representation
    ///
    /// # Arguments
    ///
    /// * `s` - String representation of the data type
    ///
    /// # Returns
    ///
    /// `Ok(ModbusDataType)` if parsing succeeds, `Err(String)` with error message if it fails
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusDataType;
    ///
    /// assert_eq!(ModbusDataType::from_str("float32").unwrap(), ModbusDataType::Float32);
    /// assert_eq!(ModbusDataType::from_str("string(20)").unwrap(), ModbusDataType::String(20));
    /// ```
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "bool" => Ok(ModbusDataType::Bool),
            "int16" => Ok(ModbusDataType::Int16),
            "uint16" => Ok(ModbusDataType::UInt16),
            "int32" => Ok(ModbusDataType::Int32),
            "uint32" => Ok(ModbusDataType::UInt32),
            "int64" => Ok(ModbusDataType::Int64),
            "uint64" => Ok(ModbusDataType::UInt64),
            "float32" => Ok(ModbusDataType::Float32),
            "float64" => Ok(ModbusDataType::Float64),
            s if s.starts_with("string(") && s.ends_with(')') => {
                let length_str = &s[7..s.len() - 1];
                match length_str.parse::<usize>() {
                    Ok(length) => Ok(ModbusDataType::String(length)),
                    Err(_) => Err(format!("Invalid string length: {}", length_str)),
                }
            }
            _ => Err(format!("Unknown data type: {}", s)),
        }
    }

    /// Check if this data type is a numeric type
    ///
    /// # Returns
    ///
    /// `true` if the data type represents a numeric value
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusDataType;
    ///
    /// assert!(ModbusDataType::Float32.is_numeric());
    /// assert!(ModbusDataType::Int16.is_numeric());
    /// assert!(!ModbusDataType::String(10).is_numeric());
    /// ```
    pub fn is_numeric(&self) -> bool {
        !matches!(self, ModbusDataType::String(_))
    }

    /// Check if this data type is a floating point type
    ///
    /// # Returns
    ///
    /// `true` if the data type represents a floating point value
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusDataType;
    ///
    /// assert!(ModbusDataType::Float32.is_float());
    /// assert!(ModbusDataType::Float64.is_float());
    /// assert!(!ModbusDataType::Int16.is_float());
    /// ```
    pub fn is_float(&self) -> bool {
        matches!(self, ModbusDataType::Float32 | ModbusDataType::Float64)
    }

    /// Check if this data type is an integer type
    ///
    /// # Returns
    ///
    /// `true` if the data type represents an integer value
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusDataType;
    ///
    /// assert!(ModbusDataType::Int32.is_integer());
    /// assert!(ModbusDataType::UInt16.is_integer());
    /// assert!(!ModbusDataType::Float32.is_integer());
    /// ```
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            ModbusDataType::Int16
                | ModbusDataType::UInt16
                | ModbusDataType::Int32
                | ModbusDataType::UInt32
                | ModbusDataType::Int64
                | ModbusDataType::UInt64
        )
    }

    /// Check if this data type is signed
    ///
    /// # Returns
    ///
    /// `true` if the data type can represent negative values
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusDataType;
    ///
    /// assert!(ModbusDataType::Int32.is_signed());
    /// assert!(ModbusDataType::Float32.is_signed());
    /// assert!(!ModbusDataType::UInt16.is_signed());
    /// ```
    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            ModbusDataType::Int16
                | ModbusDataType::Int32
                | ModbusDataType::Int64
                | ModbusDataType::Float32
                | ModbusDataType::Float64
        )
    }
}

/// Enhanced Modbus register address mapping configuration
///
/// Defines a complete mapping between a logical point and a physical Modbus register,
/// including data conversion, scaling, and metadata. This structure is typically
/// loaded from configuration files or point tables.
///
/// # Purpose
///
/// - Maps logical point names to physical register addresses
/// - Defines data type and conversion parameters
/// - Provides scaling and offset for engineering unit conversion
/// - Specifies access permissions and grouping information
///
/// # Configuration Fields
///
/// - **Identity**: `name`, `display_name`, `description`
/// - **Register**: `register_type`, `address`, `data_type`, `byte_order`
/// - **Conversion**: `scale`, `offset`, `unit`
/// - **Access**: `access_mode` (read, write, read_write)
/// - **Organization**: `group` for logical grouping
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::*;
///
/// let mapping = ModbusRegisterMapping {
///     name: "tank_temperature".to_string(),
///     display_name: Some("Tank Temperature".to_string()),
///     register_type: ModbusRegisterType::HoldingRegister,
///     address: 1000,
///     data_type: ModbusDataType::Int16,
///     scale: 0.1,        // Convert raw value to engineering units
///     offset: -40.0,     // Apply offset after scaling  
///     unit: Some("°C".to_string()),
///     access_mode: "read".to_string(),
///     ..Default::default()
/// };
///
/// // The raw register value 650 would convert to: (650 * 0.1) + (-40.0) = 25.0°C
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusRegisterMapping {
    /// Unique point identifier (primary key)
    pub name: String,
    /// Human-readable display name for user interfaces
    pub display_name: Option<String>,
    /// Register type classification (coil, discrete_input, input_register, holding_register)
    pub register_type: ModbusRegisterType,
    /// Physical register address (0-based protocol address)
    pub address: u16,
    /// Data type stored in the register(s)
    pub data_type: ModbusDataType,
    /// Scale factor applied to raw register value (multiplier)
    pub scale: f64,
    /// Offset added after scaling (engineering unit conversion)
    pub offset: f64,
    /// Engineering unit of measurement (e.g., "°C", "Pa", "RPM")
    pub unit: Option<String>,
    /// Detailed description of the point
    pub description: Option<String>,
    /// Access permissions: "read", "write", or "read_write"
    pub access_mode: String,
    /// Logical grouping category (e.g., "temperature", "pressure", "controls")
    pub group: Option<String>,
    /// Byte order for multi-register values
    pub byte_order: ByteOrder,
}

impl Default for ModbusRegisterMapping {
    /// Create a register mapping with default values
    ///
    /// # Returns
    ///
    /// Default mapping configured as:
    /// - Input register at address 0
    /// - UInt16 data type
    /// - 1:1 scaling (scale=1.0, offset=0.0)
    /// - Read-only access
    /// - Big-endian byte order
    fn default() -> Self {
        Self {
            name: String::new(),
            display_name: None,
            register_type: ModbusRegisterType::InputRegister,
            address: 0,
            data_type: ModbusDataType::UInt16,
            scale: 1.0,
            offset: 0.0,
            unit: None,
            description: None,
            access_mode: "read".to_string(),
            group: None,
            byte_order: ByteOrder::BigEndian,
        }
    }
}

impl ModbusRegisterMapping {
    /// Create a new register mapping with specified parameters
    ///
    /// # Arguments
    ///
    /// * `address` - Register address
    /// * `data_type` - Data type stored in the register
    /// * `name` - Unique identifier for the point
    ///
    /// # Returns
    ///
    /// New register mapping with default values for other fields
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mapping = ModbusRegisterMapping::new(100, ModbusDataType::UInt16, "temperature".to_string());
    /// assert_eq!(mapping.address, 100);
    /// assert_eq!(mapping.name, "temperature");
    /// ```
    pub fn new(address: u16, data_type: ModbusDataType, name: String) -> Self {
        Self {
            name,
            address,
            data_type,
            register_type: ModbusRegisterType::HoldingRegister,
            ..Default::default()
        }
    }

    /// Check if this mapping supports read operations
    ///
    /// # Returns
    ///
    /// `true` if the mapping can be read from
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::ModbusRegisterMapping;
    ///
    /// let mut mapping = ModbusRegisterMapping::default();
    /// mapping.access_mode = "read_write".to_string();
    /// assert!(mapping.is_readable());
    /// ```
    pub fn is_readable(&self) -> bool {
        matches!(self.access_mode.as_str(), "read" | "read_write")
    }

    /// Check if this mapping supports write operations
    ///
    /// # Returns
    ///
    /// `true` if the mapping can be written to (both access mode and register type must allow writes)
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mut mapping = ModbusRegisterMapping::default();
    /// mapping.register_type = ModbusRegisterType::HoldingRegister;
    /// mapping.access_mode = "read_write".to_string();
    /// assert!(mapping.is_writable());
    /// ```
    pub fn is_writable(&self) -> bool {
        matches!(self.access_mode.as_str(), "write" | "read_write")
            && self.register_type.is_writable()
    }

    /// Get the number of registers this mapping requires
    ///
    /// # Returns
    ///
    /// Number of consecutive registers needed for this data type
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mut mapping = ModbusRegisterMapping::default();
    /// mapping.data_type = ModbusDataType::Float32;
    /// assert_eq!(mapping.register_count(), 2);
    /// ```
    pub fn register_count(&self) -> u16 {
        self.data_type.register_count()
    }

    /// Get the ending address (inclusive) for this mapping
    ///
    /// # Returns
    ///
    /// The last register address used by this mapping
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mut mapping = ModbusRegisterMapping::default();
    /// mapping.address = 100;
    /// mapping.data_type = ModbusDataType::Float32; // Uses 2 registers
    /// assert_eq!(mapping.end_address(), 101); // Addresses 100 and 101
    /// ```
    pub fn end_address(&self) -> u16 {
        self.address + self.register_count() - 1
    }

    /// Check if this mapping overlaps with another mapping
    ///
    /// # Arguments
    ///
    /// * `other` - Another register mapping to check against
    ///
    /// # Returns
    ///
    /// `true` if the mappings overlap in address space
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mapping1 = ModbusRegisterMapping::new(100, ModbusDataType::Float32, "temp1".to_string());
    /// let mapping2 = ModbusRegisterMapping::new(101, ModbusDataType::UInt16, "temp2".to_string());
    /// assert!(mapping1.overlaps_with(&mapping2)); // Float32 uses 2 registers (100-101)
    /// ```
    pub fn overlaps_with(&self, other: &ModbusRegisterMapping) -> bool {
        if self.register_type != other.register_type {
            return false; // Different register types can't overlap
        }

        let self_start = self.address;
        let self_end = self.end_address();
        let other_start = other.address;
        let other_end = other.end_address();

        !(self_end < other_start || other_end < self_start)
    }

    /// Validate the mapping configuration
    ///
    /// # Returns
    ///
    /// `Ok(())` if the mapping is valid, `Err(String)` with error message if invalid
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mut mapping = ModbusRegisterMapping::default();
    /// mapping.name = "test".to_string();
    /// assert!(mapping.validate().is_ok());
    ///
    /// let mut invalid_mapping = ModbusRegisterMapping::default();
    /// invalid_mapping.name = "".to_string(); // Empty name
    /// assert!(invalid_mapping.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        // Check required fields
        if self.name.is_empty() {
            return Err("Point name cannot be empty".to_string());
        }

        // Check access mode
        if !matches!(self.access_mode.as_str(), "read" | "write" | "read_write") {
            return Err(format!("Invalid access mode: {}", self.access_mode));
        }

        // Check write permission consistency
        if self.access_mode.contains("write") && !self.register_type.is_writable() {
            return Err(format!(
                "Register type {:?} does not support write operations",
                self.register_type
            ));
        }

        // Check address range (16-bit)
        let end_addr = self.end_address();
        if end_addr < self.address {
            // Overflow check
            return Err("Address range overflow".to_string());
        }

        // Check string length
        if let ModbusDataType::String(len) = self.data_type {
            if len == 0 {
                return Err("String length cannot be zero".to_string());
            }
            if len > 1000 {
                // Reasonable limit
                return Err("String length too large (max 1000)".to_string());
            }
        }

        Ok(())
    }

    /// Convert raw register value to engineering units
    ///
    /// # Arguments
    ///
    /// * `raw_value` - Raw value from the register
    ///
    /// # Returns
    ///
    /// Scaled and offset-adjusted value in engineering units
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mut mapping = ModbusRegisterMapping::default();
    /// mapping.scale = 0.1;
    /// mapping.offset = -40.0;
    ///
    /// // Raw value 650 -> (650 * 0.1) + (-40.0) = 25.0
    /// assert_eq!(mapping.convert_to_engineering_units(650.0), 25.0);
    /// ```
    pub fn convert_to_engineering_units(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }

    /// Convert engineering units value to raw register value
    ///
    /// # Arguments
    ///
    /// * `engineering_value` - Value in engineering units
    ///
    /// # Returns
    ///
    /// Raw register value after inverse scaling and offset
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mut mapping = ModbusRegisterMapping::default();
    /// mapping.scale = 0.1;
    /// mapping.offset = -40.0;
    ///
    /// // Engineering value 25.0 -> (25.0 + 40.0) / 0.1 = 650.0
    /// assert_eq!(mapping.convert_from_engineering_units(25.0), 650.0);
    /// ```
    pub fn convert_from_engineering_units(&self, engineering_value: f64) -> f64 {
        if self.scale != 0.0 {
            (engineering_value - self.offset) / self.scale
        } else {
            engineering_value // No scaling if scale is zero
        }
    }

    /// Get the address range used by this mapping
    ///
    /// # Returns
    ///
    /// Tuple of (start_address, end_address_inclusive)
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mapping = ModbusRegisterMapping::new(100, ModbusDataType::Float32, "test".to_string());
    /// assert_eq!(mapping.address_range(), (100, 101));
    /// ```
    pub fn address_range(&self) -> (u16, u16) {
        (self.address, self.end_address())
    }

    /// Create a builder for constructing register mappings
    ///
    /// # Arguments
    ///
    /// * `name` - Unique point identifier
    ///
    /// # Returns
    ///
    /// Builder instance for fluent construction
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::*;
    ///
    /// let mapping = ModbusRegisterMapping::builder("temperature")
    ///     .address(100)
    ///     .data_type(ModbusDataType::Float32)
    ///     .scale(0.1)
    ///     .offset(-40.0)
    ///     .unit("°C")
    ///     .build();
    ///
    /// assert_eq!(mapping.name, "temperature");
    /// assert_eq!(mapping.address, 100);
    /// ```
    pub fn builder(name: &str) -> ModbusRegisterMappingBuilder {
        ModbusRegisterMappingBuilder::new(name)
    }
}

/// Builder for constructing ModbusRegisterMapping instances
///
/// Provides a fluent interface for building register mappings with optional parameters.
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::*;
///
/// let mapping = ModbusRegisterMapping::builder("flow_rate")
///     .address(200)
///     .register_type(ModbusRegisterType::HoldingRegister)
///     .data_type(ModbusDataType::Float32)
///     .scale(0.01)
///     .unit("m³/h")
///     .description("Main flow rate sensor")
///     .access_mode("read_write")
///     .build();
/// ```
#[derive(Debug)]
pub struct ModbusRegisterMappingBuilder {
    mapping: ModbusRegisterMapping,
}

impl ModbusRegisterMappingBuilder {
    /// Create a new builder with the specified name
    ///
    /// # Arguments
    ///
    /// * `name` - Unique point identifier
    ///
    /// # Returns
    ///
    /// New builder instance
    pub fn new(name: &str) -> Self {
        Self {
            mapping: ModbusRegisterMapping {
                name: name.to_string(),
                ..Default::default()
            },
        }
    }

    /// Set the register address
    pub fn address(mut self, address: u16) -> Self {
        self.mapping.address = address;
        self
    }

    /// Set the register type
    pub fn register_type(mut self, register_type: ModbusRegisterType) -> Self {
        self.mapping.register_type = register_type;
        self
    }

    /// Set the data type
    pub fn data_type(mut self, data_type: ModbusDataType) -> Self {
        self.mapping.data_type = data_type;
        self
    }

    /// Set the scale factor
    pub fn scale(mut self, scale: f64) -> Self {
        self.mapping.scale = scale;
        self
    }

    /// Set the offset
    pub fn offset(mut self, offset: f64) -> Self {
        self.mapping.offset = offset;
        self
    }

    /// Set the engineering unit
    pub fn unit(mut self, unit: &str) -> Self {
        self.mapping.unit = Some(unit.to_string());
        self
    }

    /// Set the description
    pub fn description(mut self, description: &str) -> Self {
        self.mapping.description = Some(description.to_string());
        self
    }

    /// Set the display name
    pub fn display_name(mut self, display_name: &str) -> Self {
        self.mapping.display_name = Some(display_name.to_string());
        self
    }

    /// Set the access mode
    pub fn access_mode(mut self, access_mode: &str) -> Self {
        self.mapping.access_mode = access_mode.to_string();
        self
    }

    /// Set the group
    pub fn group(mut self, group: &str) -> Self {
        self.mapping.group = Some(group.to_string());
        self
    }

    /// Set the byte order
    pub fn byte_order(mut self, byte_order: ByteOrder) -> Self {
        self.mapping.byte_order = byte_order;
        self
    }

    /// Build the final mapping
    ///
    /// # Returns
    ///
    /// Constructed ModbusRegisterMapping instance
    pub fn build(self) -> ModbusRegisterMapping {
        self.mapping
    }
}

/// Byte order enumeration for multi-register values
///
/// Defines how multi-register data types (32-bit, 64-bit) are arranged
/// in memory and transmitted over the Modbus protocol. Different devices
/// may use different byte ordering conventions.
///
/// # Byte Order Variants
///
/// For a 32-bit value `0x12345678` stored in registers:
///
/// - **BigEndian (ABCD)**: Register 0 = 0x1234, Register 1 = 0x5678
/// - **LittleEndian (DCBA)**: Register 0 = 0x5678, Register 1 = 0x1234  
/// - **BigEndianWordSwapped (BADC)**: Register 0 = 0x3412, Register 1 = 0x7856
/// - **LittleEndianWordSwapped (CDAB)**: Register 0 = 0x7856, Register 1 = 0x3412
///
/// # Default
///
/// Most Modbus devices use big-endian byte order (ABCD), which is the default.
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::ByteOrder;
///
/// let order = ByteOrder::BigEndian;
/// assert_eq!(order, ByteOrder::default());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByteOrder {
    /// Big endian byte order (ABCD) - most significant byte first
    BigEndian,
    /// Little endian byte order (DCBA) - least significant byte first
    LittleEndian,
    /// Big endian with word swap (BADC) - bytes swapped within words
    BigEndianWordSwapped,
    /// Little endian with word swap (CDAB) - bytes swapped within words
    LittleEndianWordSwapped,
}

impl Default for ByteOrder {
    /// Default byte order is big-endian (most common for Modbus devices)
    fn default() -> Self {
        ByteOrder::BigEndian
    }
}

/// Precomputed CRC16 lookup table for Modbus RTU (Polynomial 0xA001)
///
/// This table accelerates CRC16 calculation by providing precomputed values
/// for all possible byte inputs. The CRC16 algorithm used here follows the
/// Modbus RTU specification with polynomial 0xA001.
const CRC16_TABLE: [u16; 256] = [
    0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241, 0xC601, 0x06C0, 0x0780, 0xC741,
    0x0500, 0xC5C1, 0xC481, 0x0440, 0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1, 0xCE81, 0x0E40,
    0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841, 0xD801, 0x18C0, 0x1980, 0xD941,
    0x1B00, 0xDBC1, 0xDA81, 0x1A40, 0x1E00, 0xDEC1, 0xDF81, 0x1F40, 0xDD01, 0x1DC0, 0x1C80, 0xDC41,
    0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680, 0xD641, 0xD201, 0x12C0, 0x1380, 0xD341,
    0x1100, 0xD1C1, 0xD081, 0x1040, 0xF001, 0x30C0, 0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240,
    0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501, 0x35C0, 0x3480, 0xF441, 0x3C00, 0xFCC1, 0xFD81, 0x3D40,
    0xFF01, 0x3FC0, 0x3E80, 0xFE41, 0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840,
    0x2800, 0xE8C1, 0xE981, 0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41, 0xEE01, 0x2EC0, 0x2F80, 0xEF41,
    0x2D00, 0xEDC1, 0xEC81, 0x2C40, 0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640,
    0x2200, 0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041, 0xA001, 0x60C0, 0x6180, 0xA141,
    0x6300, 0xA3C1, 0xA281, 0x6240, 0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480, 0xA441,
    0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41, 0xAA01, 0x6AC0, 0x6B80, 0xAB41,
    0x6900, 0xA9C1, 0xA881, 0x6840, 0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01, 0x7BC0, 0x7A80, 0xBA41,
    0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40, 0xB401, 0x74C0, 0x7580, 0xB541,
    0x7700, 0xB7C1, 0xB681, 0x7640, 0x7200, 0xB2C1, 0xB381, 0x7340, 0xB101, 0x71C0, 0x7080, 0xB041,
    0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0, 0x5280, 0x9241, 0x9601, 0x56C0, 0x5780, 0x9741,
    0x5500, 0x95C1, 0x9481, 0x5440, 0x9C01, 0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40,
    0x5A00, 0x9AC1, 0x9B81, 0x5B40, 0x9901, 0x59C0, 0x5880, 0x9841, 0x8801, 0x48C0, 0x4980, 0x8941,
    0x4B00, 0x8BC1, 0x8A81, 0x4A40, 0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41,
    0x4400, 0x84C1, 0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641, 0x8201, 0x42C0, 0x4380, 0x8341,
    0x4100, 0x81C1, 0x8081, 0x4040,
];

/// Calculate Modbus RTU CRC16 checksum using lookup table optimization
///
/// Computes the CRC16 checksum for Modbus RTU communication using the standard
/// polynomial 0xA001. This implementation uses a precomputed lookup table for
/// optimal performance.
///
/// # Algorithm
///
/// The CRC16 calculation follows the Modbus RTU specification:
/// 1. Initialize CRC to 0xFFFF
/// 2. For each byte in the data:
///    - XOR the byte with the low byte of the CRC
///    - Use the result as an index into the lookup table
///    - Shift CRC right by 8 bits and XOR with the table value
/// 3. Return the final CRC value
///
/// # Arguments
///
/// * `data` - Byte slice containing the data to calculate CRC for
///           (typically Slave ID + PDU for Modbus RTU)
///
/// # Returns
///
/// 16-bit CRC checksum value in host byte order
///
/// # Note
///
/// The returned CRC value is in host byte order and may need to be converted
/// to little-endian format before appending to the Modbus RTU frame.
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::crc16_modbus;
///
/// // Calculate CRC for a Modbus RTU read request
/// let data = &[0x01, 0x03, 0x00, 0x00, 0x00, 0x02]; // Slave 1, Read Holding, Addr 0, Count 2
/// let crc = crc16_modbus(data);
///
/// // Convert to little-endian bytes for transmission
/// let crc_bytes = crc.to_le_bytes();
/// println!("CRC: 0x{:04X} -> [{:02X}, {:02X}]", crc, crc_bytes[0], crc_bytes[1]);
/// ```
///
/// # Performance
///
/// This lookup table implementation is significantly faster than bit-by-bit
/// calculation, especially for larger data blocks. The 256-entry table requires
/// 512 bytes of memory but provides O(n) performance where n is the data length.
pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        // XOR byte into least significant byte of crc, then use result as index into table
        let index = (crc ^ (byte as u16)) & 0x00FF;
        // Shift crc right by 8 and XOR with the value from the table
        crc = (crc >> 8) ^ CRC16_TABLE[index as usize];
    }
    crc // CRC is returned directly, need to handle byte order when appending
}

/// Batch configuration for super-scale testing operations
///
/// Optimizes batch processing for large-scale testing scenarios with
/// hundreds of thousands of data points. Provides configuration for
/// batch sizes, timeouts, and memory management.
///
/// # Purpose
///
/// - Configure optimal batch sizes for different operation types
/// - Set appropriate timeouts for large batch operations
/// - Manage memory usage during super-scale testing
/// - Control concurrency levels for batch processing
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::BatchConfig;
///
/// let config = BatchConfig::super_scale();
/// assert_eq!(config.batch_size, 1000);
/// assert_eq!(config.timeout_ms, 5000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Number of operations to process in a single batch
    pub batch_size: usize,
    /// Timeout for batch operations in milliseconds
    pub timeout_ms: u64,
    /// Maximum memory usage per batch in bytes
    pub max_memory_bytes: usize,
    /// Maximum number of concurrent batches
    pub max_concurrent_batches: usize,
    /// Enable batch optimization features
    pub enable_optimization: bool,
}

impl Default for BatchConfig {
    /// Create a batch configuration with standard values
    ///
    /// # Returns
    ///
    /// Default batch configuration suitable for normal operations
    fn default() -> Self {
        Self {
            batch_size: 100,
            timeout_ms: 1000,
            max_memory_bytes: 10 * 1024 * 1024, // 10MB
            max_concurrent_batches: 5,
            enable_optimization: true,
        }
    }
}

impl BatchConfig {
    /// Create a batch configuration optimized for super-scale testing
    ///
    /// Provides configuration values optimized for handling 250,000+ data points
    /// with high throughput and efficient memory usage.
    ///
    /// # Returns
    ///
    /// Batch configuration optimized for super-scale operations
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::BatchConfig;
    ///
    /// let config = BatchConfig::super_scale();
    /// // Optimized for 250K+ points
    /// assert!(config.batch_size >= 1000);
    /// assert!(config.max_memory_bytes >= 100 * 1024 * 1024);
    /// ```
    pub fn super_scale() -> Self {
        Self {
            batch_size: 1000,                    // Large batches for efficiency
            timeout_ms: 5000,                    // Extended timeout for large batches
            max_memory_bytes: 100 * 1024 * 1024, // 100MB memory limit
            max_concurrent_batches: 10,          // Higher concurrency
            enable_optimization: true,
        }
    }

    /// Calculate the number of batches needed for a given operation count
    ///
    /// # Arguments
    ///
    /// * `total_operations` - Total number of operations to process
    ///
    /// # Returns
    ///
    /// Number of batches required to process all operations
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::BatchConfig;
    ///
    /// let config = BatchConfig::super_scale();
    /// let batches = config.calculate_batches(250000);
    /// assert_eq!(batches, 250); // 250,000 / 1,000 = 250 batches
    /// ```
    pub fn calculate_batches(&self, total_operations: usize) -> usize {
        (total_operations + self.batch_size - 1) / self.batch_size
    }

    /// Estimate memory usage for a given number of operations
    ///
    /// # Arguments
    ///
    /// * `operations` - Number of operations to estimate memory for
    ///
    /// # Returns
    ///
    /// Estimated memory usage in bytes
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::BatchConfig;
    ///
    /// let config = BatchConfig::default();
    /// let memory = config.estimate_memory_usage(1000);
    /// assert!(memory > 0);
    /// ```
    pub fn estimate_memory_usage(&self, operations: usize) -> usize {
        // Rough estimate: 100 bytes per operation (register data + overhead)
        operations * 100
    }
}

/// Performance metrics for super-scale testing operations
///
/// Tracks comprehensive performance metrics during large-scale testing
/// scenarios, including throughput, latency, error rates, and resource usage.
///
/// # Metrics Categories
///
/// - **Throughput**: Operations per second, data transfer rates
/// - **Latency**: Response times, processing delays
/// - **Reliability**: Success rates, error counts
/// - **Resources**: Memory usage, CPU utilization
///
/// # Example
///
/// ```rust
/// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
///
/// let mut metrics = PerformanceMetrics::new();
/// metrics.record_operation(true, 50); // Success, 50ms latency
/// metrics.record_operation(false, 0); // Failure
///
/// assert_eq!(metrics.success_rate(), 0.5);
/// assert_eq!(metrics.average_latency_ms(), 25.0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Total number of operations attempted
    pub total_operations: u64,
    /// Number of successful operations
    pub successful_operations: u64,
    /// Number of failed operations
    pub failed_operations: u64,
    /// Total latency in milliseconds for all operations
    pub total_latency_ms: u64,
    /// Minimum observed latency in milliseconds
    pub min_latency_ms: u64,
    /// Maximum observed latency in milliseconds
    pub max_latency_ms: u64,
    /// Total bytes transferred
    pub total_bytes_transferred: u64,
    /// Peak memory usage in bytes
    pub peak_memory_bytes: usize,
    /// Test start timestamp
    pub start_time: std::time::SystemTime,
    /// Test end timestamp (None if still running)
    pub end_time: Option<std::time::SystemTime>,
}

impl Default for PerformanceMetrics {
    /// Create new performance metrics with zero values
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMetrics {
    /// Create a new performance metrics instance
    ///
    /// # Returns
    ///
    /// New metrics instance with zero values and current timestamp
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            total_latency_ms: 0,
            min_latency_ms: u64::MAX,
            max_latency_ms: 0,
            total_bytes_transferred: 0,
            peak_memory_bytes: 0,
            start_time: std::time::SystemTime::now(),
            end_time: None,
        }
    }

    /// Record the result of a single operation
    ///
    /// # Arguments
    ///
    /// * `success` - Whether the operation was successful
    /// * `latency_ms` - Operation latency in milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
    ///
    /// let mut metrics = PerformanceMetrics::new();
    /// metrics.record_operation(true, 100);
    /// assert_eq!(metrics.total_operations, 1);
    /// assert_eq!(metrics.successful_operations, 1);
    /// ```
    pub fn record_operation(&mut self, success: bool, latency_ms: u64) {
        self.total_operations += 1;

        if success {
            self.successful_operations += 1;
        } else {
            self.failed_operations += 1;
        }

        if latency_ms > 0 {
            self.total_latency_ms += latency_ms;
            self.min_latency_ms = self.min_latency_ms.min(latency_ms);
            self.max_latency_ms = self.max_latency_ms.max(latency_ms);
        }
    }

    /// Record data transfer for throughput calculation
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes transferred
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
    ///
    /// let mut metrics = PerformanceMetrics::new();
    /// metrics.record_data_transfer(1024);
    /// assert_eq!(metrics.total_bytes_transferred, 1024);
    /// ```
    pub fn record_data_transfer(&mut self, bytes: u64) {
        self.total_bytes_transferred += bytes;
    }

    /// Update peak memory usage
    ///
    /// # Arguments
    ///
    /// * `memory_bytes` - Current memory usage in bytes
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
    ///
    /// let mut metrics = PerformanceMetrics::new();
    /// metrics.update_memory_usage(1024 * 1024); // 1MB
    /// assert_eq!(metrics.peak_memory_bytes, 1024 * 1024);
    /// ```
    pub fn update_memory_usage(&mut self, memory_bytes: usize) {
        self.peak_memory_bytes = self.peak_memory_bytes.max(memory_bytes);
    }

    /// Mark the end of the test period
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
    ///
    /// let mut metrics = PerformanceMetrics::new();
    /// metrics.finish();
    /// assert!(metrics.end_time.is_some());
    /// ```
    pub fn finish(&mut self) {
        self.end_time = Some(std::time::SystemTime::now());
    }

    /// Calculate success rate as a percentage
    ///
    /// # Returns
    ///
    /// Success rate between 0.0 and 1.0
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
    ///
    /// let mut metrics = PerformanceMetrics::new();
    /// metrics.record_operation(true, 100);
    /// metrics.record_operation(false, 0);
    /// assert_eq!(metrics.success_rate(), 0.5);
    /// ```
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            self.successful_operations as f64 / self.total_operations as f64
        }
    }

    /// Calculate average latency in milliseconds
    ///
    /// # Returns
    ///
    /// Average latency for successful operations
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
    ///
    /// let mut metrics = PerformanceMetrics::new();
    /// metrics.record_operation(true, 100);
    /// metrics.record_operation(true, 200);
    /// assert_eq!(metrics.average_latency_ms(), 150.0);
    /// ```
    pub fn average_latency_ms(&self) -> f64 {
        if self.successful_operations == 0 {
            0.0
        } else {
            self.total_latency_ms as f64 / self.successful_operations as f64
        }
    }

    /// Calculate operations per second
    ///
    /// # Returns
    ///
    /// Operations per second based on elapsed time
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
    /// use std::time::Duration;
    ///
    /// let mut metrics = PerformanceMetrics::new();
    /// // Simulate some operations over time
    /// for _ in 0..1000 {
    ///     metrics.record_operation(true, 10);
    /// }
    /// let ops_per_sec = metrics.operations_per_second();
    /// assert!(ops_per_sec > 0.0);
    /// ```
    pub fn operations_per_second(&self) -> f64 {
        let end_time = self.end_time.unwrap_or_else(std::time::SystemTime::now);

        if let Ok(duration) = end_time.duration_since(self.start_time) {
            let seconds = duration.as_secs_f64();
            if seconds > 0.0 {
                self.total_operations as f64 / seconds
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Calculate data throughput in bytes per second
    ///
    /// # Returns
    ///
    /// Bytes per second based on elapsed time
    ///
    /// # Example
    ///
    /// ```rust
    /// use comsrv::core::protocols::modbus::common::PerformanceMetrics;
    ///
    /// let mut metrics = PerformanceMetrics::new();
    /// metrics.record_data_transfer(1024 * 1024); // 1MB
    /// let throughput = metrics.bytes_per_second();
    /// assert!(throughput > 0.0);
    /// ```
    pub fn bytes_per_second(&self) -> f64 {
        let end_time = self.end_time.unwrap_or_else(std::time::SystemTime::now);

        if let Ok(duration) = end_time.duration_since(self.start_time) {
            let seconds = duration.as_secs_f64();
            if seconds > 0.0 {
                self.total_bytes_transferred as f64 / seconds
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.timeout_ms, 1000);
        assert!(config.enable_optimization);
    }

    #[test]
    fn test_batch_config_super_scale() {
        let config = BatchConfig::super_scale();
        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_memory_bytes, 100 * 1024 * 1024);
        assert_eq!(config.max_concurrent_batches, 10);
    }

    #[test]
    fn test_batch_config_calculate_batches() {
        let config = BatchConfig::super_scale();
        assert_eq!(config.calculate_batches(250000), 250);
        assert_eq!(config.calculate_batches(999), 1);
        assert_eq!(config.calculate_batches(1001), 2);
    }

    #[test]
    fn test_batch_config_estimate_memory() {
        let config = BatchConfig::default();
        let memory = config.estimate_memory_usage(1000);
        assert_eq!(memory, 100000); // 1000 * 100 bytes
    }

    #[test]
    fn test_performance_metrics_new() {
        let metrics = PerformanceMetrics::new();
        assert_eq!(metrics.total_operations, 0);
        assert_eq!(metrics.successful_operations, 0);
        assert_eq!(metrics.failed_operations, 0);
        assert!(metrics.end_time.is_none());
    }

    #[test]
    fn test_performance_metrics_record_operation() {
        let mut metrics = PerformanceMetrics::new();

        metrics.record_operation(true, 100);
        assert_eq!(metrics.total_operations, 1);
        assert_eq!(metrics.successful_operations, 1);
        assert_eq!(metrics.failed_operations, 0);
        assert_eq!(metrics.min_latency_ms, 100);
        assert_eq!(metrics.max_latency_ms, 100);

        metrics.record_operation(false, 0);
        assert_eq!(metrics.total_operations, 2);
        assert_eq!(metrics.successful_operations, 1);
        assert_eq!(metrics.failed_operations, 1);
    }

    #[test]
    fn test_performance_metrics_success_rate() {
        let mut metrics = PerformanceMetrics::new();

        // No operations
        assert_eq!(metrics.success_rate(), 0.0);

        // 50% success rate
        metrics.record_operation(true, 100);
        metrics.record_operation(false, 0);
        assert_eq!(metrics.success_rate(), 0.5);

        // 100% success rate
        metrics.record_operation(true, 200);
        assert!((metrics.success_rate() - 0.6666666666666666).abs() < f64::EPSILON);
    }

    #[test]
    fn test_performance_metrics_average_latency() {
        let mut metrics = PerformanceMetrics::new();

        // No successful operations
        assert_eq!(metrics.average_latency_ms(), 0.0);

        // Average of 100 and 200
        metrics.record_operation(true, 100);
        metrics.record_operation(true, 200);
        assert_eq!(metrics.average_latency_ms(), 150.0);

        // Failed operation shouldn't affect average
        metrics.record_operation(false, 0);
        assert_eq!(metrics.average_latency_ms(), 150.0);
    }

    #[test]
    fn test_performance_metrics_data_transfer() {
        let mut metrics = PerformanceMetrics::new();

        metrics.record_data_transfer(1024);
        metrics.record_data_transfer(2048);
        assert_eq!(metrics.total_bytes_transferred, 3072);
    }

    #[test]
    fn test_performance_metrics_memory_usage() {
        let mut metrics = PerformanceMetrics::new();

        metrics.update_memory_usage(1024);
        assert_eq!(metrics.peak_memory_bytes, 1024);

        metrics.update_memory_usage(512); // Should not decrease peak
        assert_eq!(metrics.peak_memory_bytes, 1024);

        metrics.update_memory_usage(2048); // Should increase peak
        assert_eq!(metrics.peak_memory_bytes, 2048);
    }

    #[test]
    fn test_crc16_modbus() {
        // Test with known Modbus RTU frame
        let data = &[0x01, 0x03, 0x00, 0x00, 0x00, 0x02];
        let crc = crc16_modbus(data);

        // CRC should be calculated correctly
        assert_ne!(crc, 0);

        // Test with empty data
        let empty_crc = crc16_modbus(&[]);
        assert_eq!(empty_crc, 0xFFFF); // Initial CRC value
    }

    #[test]
    fn test_modbus_function_code_conversion() {
        let code = ModbusFunctionCode::ReadHoldingRegisters;
        let byte_val: u8 = code.into();
        assert_eq!(byte_val, 0x03);

        let converted_back = ModbusFunctionCode::from(byte_val);
        assert_eq!(converted_back, ModbusFunctionCode::ReadHoldingRegisters);
    }

    #[test]
    fn test_modbus_register_type_functions() {
        let holding_reg = ModbusRegisterType::HoldingRegister;
        assert!(holding_reg.is_writable());
        assert_eq!(
            holding_reg.read_function_code(),
            ModbusFunctionCode::ReadHoldingRegisters
        );
        assert_eq!(
            holding_reg.write_function_code(false),
            Some(ModbusFunctionCode::WriteSingleRegister)
        );
        assert_eq!(
            holding_reg.write_function_code(true),
            Some(ModbusFunctionCode::WriteMultipleRegisters)
        );

        let input_reg = ModbusRegisterType::InputRegister;
        assert!(!input_reg.is_writable());
        assert_eq!(input_reg.write_function_code(false), None);
    }

    #[test]
    fn test_modbus_data_type_register_count() {
        assert_eq!(ModbusDataType::Bool.register_count(), 1);
        assert_eq!(ModbusDataType::UInt16.register_count(), 1);
        assert_eq!(ModbusDataType::Float32.register_count(), 2);
        assert_eq!(ModbusDataType::Float64.register_count(), 4);
        assert_eq!(ModbusDataType::String(10).register_count(), 5);
    }

    #[test]
    fn test_modbus_register_mapping() {
        let mapping = ModbusRegisterMapping::new(100, ModbusDataType::Float32, "test".to_string());
        assert_eq!(mapping.address, 100);
        assert_eq!(mapping.name, "test");
        assert_eq!(mapping.register_count(), 2);
        assert_eq!(mapping.end_address(), 101);

        // Test default mapping
        let default_mapping = ModbusRegisterMapping::default();
        assert!(default_mapping.is_readable());
        assert!(!default_mapping.is_writable()); // Input register is read-only
    }

    #[test]
    fn test_modbus_data_type_additional_methods() {
        // Test from_str
        assert_eq!(
            ModbusDataType::from_str("float32").unwrap(),
            ModbusDataType::Float32
        );
        assert_eq!(
            ModbusDataType::from_str("string(20)").unwrap(),
            ModbusDataType::String(20)
        );
        assert!(ModbusDataType::from_str("invalid").is_err());

        // Test type checking methods
        assert!(ModbusDataType::Float32.is_numeric());
        assert!(ModbusDataType::Float32.is_float());
        assert!(!ModbusDataType::Float32.is_integer());
        assert!(ModbusDataType::Float32.is_signed());

        assert!(ModbusDataType::UInt16.is_numeric());
        assert!(!ModbusDataType::UInt16.is_float());
        assert!(ModbusDataType::UInt16.is_integer());
        assert!(!ModbusDataType::UInt16.is_signed());

        assert!(!ModbusDataType::String(10).is_numeric());
        assert!(!ModbusDataType::String(10).is_float());
        assert!(!ModbusDataType::String(10).is_integer());
        assert!(!ModbusDataType::String(10).is_signed());
    }

    #[test]
    fn test_modbus_register_mapping_validation() {
        let mut mapping = ModbusRegisterMapping::default();
        mapping.name = "test".to_string();
        assert!(mapping.validate().is_ok());

        // Test empty name
        let mut invalid_mapping = ModbusRegisterMapping::default();
        invalid_mapping.name = "".to_string();
        assert!(invalid_mapping.validate().is_err());

        // Test invalid access mode
        let mut invalid_access = ModbusRegisterMapping::default();
        invalid_access.name = "test".to_string();
        invalid_access.access_mode = "invalid".to_string();
        assert!(invalid_access.validate().is_err());

        // Test write on read-only register type
        let mut invalid_write = ModbusRegisterMapping::default();
        invalid_write.name = "test".to_string();
        invalid_write.register_type = ModbusRegisterType::InputRegister;
        invalid_write.access_mode = "write".to_string();
        assert!(invalid_write.validate().is_err());
    }

    #[test]
    fn test_modbus_register_mapping_overlap() {
        let mapping1 =
            ModbusRegisterMapping::new(100, ModbusDataType::Float32, "temp1".to_string());
        let mapping2 = ModbusRegisterMapping::new(101, ModbusDataType::UInt16, "temp2".to_string());
        let mapping3 = ModbusRegisterMapping::new(102, ModbusDataType::UInt16, "temp3".to_string());

        // mapping1 uses addresses 100-101 (Float32 = 2 registers)
        // mapping2 uses address 101 (UInt16 = 1 register)
        // mapping3 uses address 102 (UInt16 = 1 register)

        assert!(mapping1.overlaps_with(&mapping2)); // 100-101 overlaps with 101
        assert!(!mapping1.overlaps_with(&mapping3)); // 100-101 doesn't overlap with 102
        assert!(!mapping2.overlaps_with(&mapping3)); // 101 doesn't overlap with 102

        // Different register types don't overlap
        let mut coil_mapping = mapping1.clone();
        coil_mapping.register_type = ModbusRegisterType::Coil;
        assert!(!mapping1.overlaps_with(&coil_mapping));
    }

    #[test]
    fn test_modbus_register_mapping_conversion() {
        let mut mapping = ModbusRegisterMapping::default();
        mapping.scale = 0.1;
        mapping.offset = -40.0;

        // Test engineering units conversion
        assert_eq!(mapping.convert_to_engineering_units(650.0), 25.0); // (650 * 0.1) + (-40.0)
        assert_eq!(mapping.convert_from_engineering_units(25.0), 650.0); // (25.0 - (-40.0)) / 0.1

        // Test with zero scale
        mapping.scale = 0.0;
        assert_eq!(mapping.convert_from_engineering_units(25.0), 25.0); // No scaling
    }

    #[test]
    fn test_modbus_register_mapping_builder() {
        let mapping = ModbusRegisterMapping::builder("temperature")
            .address(100)
            .register_type(ModbusRegisterType::HoldingRegister)
            .data_type(ModbusDataType::Float32)
            .scale(0.1)
            .offset(-40.0)
            .unit("°C")
            .description("Temperature sensor")
            .access_mode("read_write")
            .build();

        assert_eq!(mapping.name, "temperature");
        assert_eq!(mapping.address, 100);
        assert_eq!(mapping.register_type, ModbusRegisterType::HoldingRegister);
        assert_eq!(mapping.data_type, ModbusDataType::Float32);
        assert_eq!(mapping.scale, 0.1);
        assert_eq!(mapping.offset, -40.0);
        assert_eq!(mapping.unit, Some("°C".to_string()));
        assert_eq!(mapping.description, Some("Temperature sensor".to_string()));
        assert_eq!(mapping.access_mode, "read_write");
        assert!(mapping.is_writable());
    }

    #[test]
    fn test_modbus_register_mapping_address_range() {
        let mapping = ModbusRegisterMapping::new(100, ModbusDataType::Float32, "test".to_string());
        assert_eq!(mapping.address_range(), (100, 101));

        let single_reg_mapping =
            ModbusRegisterMapping::new(200, ModbusDataType::UInt16, "test2".to_string());
        assert_eq!(single_reg_mapping.address_range(), (200, 200));
    }

    /// Integration test for real configuration loading
    #[tokio::test]
    async fn test_real_config_loading_to_memory_and_redis() {
        use std::fs::{self, File};
        use std::io::Write;
        use tempfile::TempDir;

        // Create temporary directory for test files
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        // 1. Create real Modbus client configuration YAML
        let modbus_config_content = r#"
# Modbus TCP Client Configuration
slave_id: 1
timeout_ms: 5000
max_retries: 3
poll_interval_ms: 1000
host: "192.168.1.100"
port: 502
mode: "tcp"

# Point table configuration
point_tables:
  main_device: "device_points.csv"
  backup_device: "backup_points.csv"
"#;

        let config_path = config_dir.join("modbus_config.yaml");
        let mut config_file = File::create(&config_path).unwrap();
        config_file
            .write_all(modbus_config_content.as_bytes())
            .unwrap();

        // 2. Create real CSV point table
        let csv_content = r#"id,name,address,type_,data_type,unit,scale,offset,writable,description,byte_order
PT001,Tank_Temperature,1000,input_register,float32,°C,0.1,-40.0,false,Main tank temperature sensor,big_endian
PT002,Tank_Pressure,1002,input_register,uint16,Pa,1.0,0.0,false,Tank pressure sensor,big_endian
PT003,Flow_Rate,1004,input_register,float32,L/min,0.01,0.0,false,Flow rate meter,big_endian
CT001,Pump_Control,2000,holding_register,uint16,,1.0,0.0,true,Pump speed control,big_endian
CT002,Valve_Position,2001,holding_register,uint16,%,0.1,0.0,true,Valve position setpoint,big_endian
ST001,Pump_Status,3000,coil,bool,,1.0,0.0,true,Pump on/off status,big_endian
ST002,Alarm_Status,3001,discrete_input,bool,,1.0,0.0,false,General alarm status,big_endian
AT001,System_Mode,4000,holding_register,uint16,,1.0,0.0,true,System operation mode,big_endian
AT002,Error_Code,4001,input_register,uint16,,1.0,0.0,false,Last error code,big_endian
FT001,Total_Volume,5000,input_register,uint32,L,1.0,0.0,false,Cumulative volume,big_endian"#;

        let csv_path = config_dir.join("device_points.csv");
        let mut csv_file = File::create(&csv_path).unwrap();
        csv_file.write_all(csv_content.as_bytes()).unwrap();

        // 3. Parse configuration and load point mappings
        println!(
            "📂 Loading Modbus configuration from: {}",
            config_path.display()
        );

        // Simulate loading YAML config (simplified)
        let config_content = fs::read_to_string(&config_path).unwrap();
        println!(
            "✅ Config file loaded, size: {} bytes",
            config_content.len()
        );

        // Parse CSV using our point table functionality
        use csv::ReaderBuilder;
        use std::collections::HashMap as StdHashMap;

        #[derive(serde::Deserialize)]
        struct CsvPointRecord {
            id: String,
            name: String,
            address: u16,
            #[serde(rename = "type_")]
            type_: String,
            data_type: String,
            unit: String,
            scale: f64,
            offset: f64,
            writable: bool,
            description: String,
            byte_order: String,
        }

        println!("📊 Loading CSV point table from: {}", csv_path.display());

        let file = File::open(&csv_path).unwrap();
        let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);

        let mut mappings = Vec::new();
        let mut point_count_by_type = StdHashMap::new();

        for result in reader.deserialize() {
            let record: CsvPointRecord = result.unwrap();

            // Parse data type
            let data_type = match record.data_type.to_lowercase().as_str() {
                "bool" | "boolean" => ModbusDataType::Bool,
                "uint16" | "u16" => ModbusDataType::UInt16,
                "uint32" | "u32" => ModbusDataType::UInt32,
                "float32" | "f32" => ModbusDataType::Float32,
                _ => ModbusDataType::UInt16, // default
            };

            // Parse register type
            let register_type = match record.type_.to_lowercase().as_str() {
                "coil" => ModbusRegisterType::Coil,
                "discrete_input" => ModbusRegisterType::DiscreteInput,
                "input_register" => ModbusRegisterType::InputRegister,
                "holding_register" => ModbusRegisterType::HoldingRegister,
                _ => ModbusRegisterType::InputRegister, // default
            };

            // Parse byte order
            let byte_order = match record.byte_order.to_lowercase().as_str() {
                "little_endian" => ByteOrder::LittleEndian,
                "big_endian_word_swapped" => ByteOrder::BigEndianWordSwapped,
                "little_endian_word_swapped" => ByteOrder::LittleEndianWordSwapped,
                _ => ByteOrder::BigEndian, // default
            };

            // Create mapping using our enhanced constructor
            let mapping = ModbusRegisterMapping::builder(&record.id)
                .address(record.address)
                .register_type(register_type)
                .data_type(data_type)
                .scale(record.scale)
                .offset(record.offset)
                .unit(&record.unit)
                .description(&record.description)
                .display_name(&record.name)
                .access_mode(if record.writable {
                    "read_write"
                } else {
                    "read"
                })
                .byte_order(byte_order)
                .build();

            // Validate the mapping
            mapping.validate().unwrap();

            // Count by type for statistics
            *point_count_by_type
                .entry(format!("{:?}", register_type))
                .or_insert(0) += 1;

            mappings.push(mapping);
        }

        println!("✅ Loaded {} point mappings from CSV", mappings.len());
        println!("📈 Point distribution:");
        for (type_name, count) in &point_count_by_type {
            println!("   - {}: {} points", type_name, count);
        }

        // 4. Test memory storage and access
        println!("\n🧠 Testing in-memory storage and access...");

        // Test address range calculations
        let mut total_registers = 0u32;
        let mut address_ranges = Vec::new();

        for mapping in &mappings {
            let range = mapping.address_range();
            address_ranges.push((mapping.name.clone(), range));
            total_registers += mapping.register_count() as u32;

            // Test engineering units conversion
            let raw_value = 1000.0;
            let eng_value = mapping.convert_to_engineering_units(raw_value);
            let back_to_raw = mapping.convert_from_engineering_units(eng_value);

            println!(
                "   📏 {}: addr={}-{}, raw={} -> eng={} -> raw={:.1}",
                mapping.name, range.0, range.1, raw_value, eng_value, back_to_raw
            );
        }

        println!("✅ Total registers required: {}", total_registers);

        // Test overlap detection
        println!("\n🔍 Testing address overlap detection...");
        let mut overlaps_found = 0;
        for i in 0..mappings.len() {
            for j in (i + 1)..mappings.len() {
                if mappings[i].overlaps_with(&mappings[j]) {
                    overlaps_found += 1;
                    println!(
                        "   ⚠️  Overlap detected: {} <-> {}",
                        mappings[i].name, mappings[j].name
                    );
                }
            }
        }

        if overlaps_found == 0 {
            println!("✅ No address overlaps detected");
        }

        // 5. Test Redis storage simulation
        println!("\n🔴 Testing Redis storage simulation...");

        // Simulate Redis storage structure
        let mut redis_data = StdHashMap::new();

        // Store configuration
        let config_key = "modbus:config:channel_1";
        let config_json = serde_json::json!({
            "slave_id": 1,
            "host": "192.168.1.100",
            "port": 502,
            "timeout_ms": 5000,
            "loaded_at": chrono::Utc::now().to_rfc3339(),
            "point_count": mappings.len()
        });
        redis_data.insert(config_key.to_string(), config_json.to_string());

        // Store point mappings
        for (_index, mapping) in mappings.iter().enumerate() {
            let point_key = format!("modbus:points:channel_1:{}", mapping.name);
            let point_json = serde_json::json!({
                "id": mapping.name,
                "display_name": mapping.display_name,
                "address": mapping.address,
                "register_type": format!("{:?}", mapping.register_type),
                "data_type": format!("{:?}", mapping.data_type),
                "scale": mapping.scale,
                "offset": mapping.offset,
                "unit": mapping.unit,
                "access_mode": mapping.access_mode,
                "byte_order": format!("{:?}", mapping.byte_order),
                "register_count": mapping.register_count(),
                "address_range": mapping.address_range(),
                "stored_at": chrono::Utc::now().to_rfc3339()
            });
            redis_data.insert(point_key, point_json.to_string());
        }

        // Store statistics
        let stats_key = "modbus:stats:channel_1";
        let stats_json = serde_json::json!({
            "total_points": mappings.len(),
            "total_registers": total_registers,
            "point_types": point_count_by_type,
            "overlaps_detected": overlaps_found,
            "last_updated": chrono::Utc::now().to_rfc3339()
        });
        redis_data.insert(stats_key.to_string(), stats_json.to_string());

        println!("✅ Stored {} entries in Redis simulation", redis_data.len());

        // 6. Test retrieval and validation
        println!("\n🔍 Testing data retrieval and validation...");

        // Retrieve and validate configuration
        let retrieved_config = redis_data.get(config_key).unwrap();
        let config_obj: serde_json::Value = serde_json::from_str(retrieved_config).unwrap();
        assert_eq!(config_obj["slave_id"], 1);
        assert_eq!(config_obj["point_count"], mappings.len());
        println!("✅ Configuration retrieved and validated");

        // Retrieve and validate sample point
        let sample_point_key = format!("modbus:points:channel_1:{}", mappings[0].name);
        let retrieved_point = redis_data.get(&sample_point_key).unwrap();
        let point_obj: serde_json::Value = serde_json::from_str(retrieved_point).unwrap();
        assert_eq!(point_obj["id"], mappings[0].name);
        assert_eq!(point_obj["address"], mappings[0].address);
        println!("✅ Point data retrieved and validated");

        // Retrieve and validate statistics
        let retrieved_stats = redis_data.get(stats_key).unwrap();
        let stats_obj: serde_json::Value = serde_json::from_str(retrieved_stats).unwrap();
        assert_eq!(stats_obj["total_points"], mappings.len());
        println!("✅ Statistics retrieved and validated");

        // 7. Performance metrics test
        println!("\n⚡ Testing performance metrics...");

        let start_time = std::time::Instant::now();

        // Simulate batch operations
        let batch_config = BatchConfig::super_scale();
        let batch_count = batch_config.calculate_batches(mappings.len());
        let estimated_memory = batch_config.estimate_memory_usage(mappings.len());

        let mut metrics = PerformanceMetrics::new();

        // Simulate processing each mapping
        for mapping in &mappings {
            let processing_time = 10 + (mapping.register_count() as u64 * 2); // Simulate processing time
            metrics.record_operation(true, processing_time);
            metrics.record_data_transfer(mapping.register_count() as u64 * 2); // 2 bytes per register
        }

        metrics.finish();

        let elapsed = start_time.elapsed();

        println!("✅ Processed {} mappings in {:?}", mappings.len(), elapsed);
        println!("   📊 Success rate: {:.2}%", metrics.success_rate() * 100.0);
        println!(
            "   ⏱️  Average latency: {:.2}ms",
            metrics.average_latency_ms()
        );
        println!(
            "   🚀 Operations/sec: {:.2}",
            metrics.operations_per_second()
        );
        println!("   📦 Batches required: {}", batch_count);
        println!(
            "   💾 Estimated memory: {:.2}KB",
            estimated_memory as f64 / 1024.0
        );

        // 8. Final assertions
        assert_eq!(mappings.len(), 10, "Should load exactly 10 points from CSV");
        assert!(
            overlaps_found == 0,
            "No address overlaps should be detected"
        );
        assert!(
            redis_data.len() > mappings.len(),
            "Redis should contain config + points + stats"
        );
        assert!(
            metrics.success_rate() == 1.0,
            "All operations should succeed"
        );
        assert!(estimated_memory > 0, "Memory estimation should be positive");

        println!("\n🎉 Integration test completed successfully!");
        println!("   ✅ Configuration loading: PASSED");
        println!("   ✅ CSV parsing: PASSED");
        println!("   ✅ Memory storage: PASSED");
        println!("   ✅ Redis simulation: PASSED");
        println!("   ✅ Data validation: PASSED");
        println!("   ✅ Performance metrics: PASSED");
    }
}
