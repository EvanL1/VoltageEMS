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

impl std::fmt::Display for ModbusFunctionCode {
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

/// Precomputed CRC16 lookup table for Modbus (Polynomial 0xA001)
const CRC16_TABLE: [u16; 256] = [
    0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241,
    0xC601, 0x06C0, 0x0780, 0xC741, 0x0500, 0xC5C1, 0xC481, 0x0440,
    0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1, 0xCE81, 0x0E40,
    0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841,
    0xD801, 0x18C0, 0x1980, 0xD941, 0x1B00, 0xDBC1, 0xDA81, 0x1A40,
    0x1E00, 0xDEC1, 0xDF81, 0x1F40, 0xDD01, 0x1DC0, 0x1C80, 0xDC41,
    0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680, 0xD641,
    0xD201, 0x12C0, 0x1380, 0xD341, 0x1100, 0xD1C1, 0xD081, 0x1040,
    0xF001, 0x30C0, 0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240,
    0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501, 0x35C0, 0x3480, 0xF441,
    0x3C00, 0xFCC1, 0xFD81, 0x3D40, 0xFF01, 0x3FC0, 0x3E80, 0xFE41,
    0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840,
    0x2800, 0xE8C1, 0xE981, 0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41,
    0xEE01, 0x2EC0, 0x2F80, 0xEF41, 0x2D00, 0xEDC1, 0xEC81, 0x2C40,
    0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640,
    0x2200, 0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041,
    0xA001, 0x60C0, 0x6180, 0xA141, 0x6300, 0xA3C1, 0xA281, 0x6240,
    0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480, 0xA441,
    0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41,
    0xAA01, 0x6AC0, 0x6B80, 0xAB41, 0x6900, 0xA9C1, 0xA881, 0x6840,
    0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01, 0x7BC0, 0x7A80, 0xBA41,
    0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40,
    0xB401, 0x74C0, 0x7580, 0xB541, 0x7700, 0xB7C1, 0xB681, 0x7640,
    0x7200, 0xB2C1, 0xB381, 0x7340, 0xB101, 0x71C0, 0x7080, 0xB041,
    0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0, 0x5280, 0x9241,
    0x9601, 0x56C0, 0x5780, 0x9741, 0x5500, 0x95C1, 0x9481, 0x5440,
    0x9C01, 0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40,
    0x5A00, 0x9AC1, 0x9B81, 0x5B40, 0x9901, 0x59C0, 0x5880, 0x9841,
    0x8801, 0x48C0, 0x4980, 0x8941, 0x4B00, 0x8BC1, 0x8A81, 0x4A40,
    0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41,
    0x4400, 0x84C1, 0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641,
    0x8201, 0x42C0, 0x4380, 0x8341, 0x4100, 0x81C1, 0x8081, 0x4040
];

/// Calculate Modbus RTU CRC16 checksum using a lookup table.
/// 
/// # Arguments
/// * `data` - Data bytes to calculate CRC for (typically Slave ID + PDU)
/// 
/// # Returns
/// Calculated CRC16 value (does not handle byte order for appending).
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