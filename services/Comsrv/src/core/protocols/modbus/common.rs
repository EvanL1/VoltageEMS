use serde::{Deserialize, Serialize};

/// Modbus Function Code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModbusFunctionCode {
    /// read coils
    ReadCoils = 0x01,
    /// read discrete inputs
    ReadDiscreteInputs = 0x02,
    /// read holding registers
    ReadHoldingRegisters = 0x03,
    /// read input registers
    ReadInputRegisters = 0x04,
    /// write single coil
    WriteSingleCoil = 0x05,
    /// write single register
    WriteSingleRegister = 0x06,
    /// write multiple coils
    WriteMultipleCoils = 0x0F,
    /// write multiple registers
    WriteMultipleRegisters = 0x10,
    /// custom function code
    Custom(u8),
}

impl From<u8> for ModbusFunctionCode {
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

/// Modbus data type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusDataType {
    /// boolean value
    Bool,
    /// 16-bit integer (1 register)
    Int16,
    /// 16-bit unsigned integer (1 register)
    UInt16,
    /// 32-bit integer (2 registers)
    Int32,
    /// 32-bit unsigned integer (2 registers)
    UInt32,
    /// 32-bit float (2 registers)
    Float32,
    /// 64-bit float (4 registers)
    Float64,
    /// string
    String(usize),
}

/// Modbus register address mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusRegisterMapping {
    /// point ID
    pub point_id: String,
    /// register address
    pub address: u16,
    /// register quantity
    pub quantity: u16,
    /// data type
    pub data_type: ModbusDataType,
    /// writable
    pub writable: bool,
    /// byte order (default big endian)
    pub byte_order: ByteOrder,
    /// scale factor (default 1.0)
    pub scale_factor: Option<f64>,
    /// offset (default 0.0)
    pub offset: Option<f64>,
}

/// byte order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByteOrder {
    /// big endian (ABCD)
    BigEndian,
    /// little endian (DCBA)
    LittleEndian,
    /// big endian word, little endian byte (BADC)
    BigEndianWordSwapped,
    /// little endian word, big endian byte (CDAB)
    LittleEndianWordSwapped,
} 