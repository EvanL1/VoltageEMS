//! Modbus Protocol Common Definitions
//!
//! This module contains basic Modbus protocol definitions including function codes,
//! register types, data types, and utility functions.

use serde::{Deserialize, Serialize};

/// Modbus function codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModbusFunctionCode {
    Read01 = 0x01,
    Read02 = 0x02,
    Read03 = 0x03,
    Read04 = 0x04,
    Write05 = 0x05,
    Write06 = 0x06,
    Write0F = 0x0F,
    Write10 = 0x10,
    Custom(u8),
}

impl From<u8> for ModbusFunctionCode {
    fn from(code: u8) -> Self {
        match code {
            0x01 => ModbusFunctionCode::Read01,
            0x02 => ModbusFunctionCode::Read02,
            0x03 => ModbusFunctionCode::Read03,
            0x04 => ModbusFunctionCode::Read04,
            0x05 => ModbusFunctionCode::Write05,
            0x06 => ModbusFunctionCode::Write06,
            0x0F => ModbusFunctionCode::Write0F,
            0x10 => ModbusFunctionCode::Write10,
            other => ModbusFunctionCode::Custom(other),
        }
    }
}

impl From<ModbusFunctionCode> for u8 {
    fn from(code: ModbusFunctionCode) -> Self {
        match code {
            ModbusFunctionCode::Read01 => 0x01,
            ModbusFunctionCode::Read02 => 0x02,
            ModbusFunctionCode::Read03 => 0x03,
            ModbusFunctionCode::Read04 => 0x04,
            ModbusFunctionCode::Write05 => 0x05,
            ModbusFunctionCode::Write06 => 0x06,
            ModbusFunctionCode::Write0F => 0x0F,
            ModbusFunctionCode::Write10 => 0x10,
            ModbusFunctionCode::Custom(code) => code,
        }
    }
}

impl ModbusFunctionCode {
    /// Get the register type associated with this function code
    pub fn register_type(&self) -> ModbusRegisterType {
        match self {
            ModbusFunctionCode::Read01 | ModbusFunctionCode::Write05 | ModbusFunctionCode::Write0F => {
                ModbusRegisterType::Coil
            }
            ModbusFunctionCode::Read02 => ModbusRegisterType::DiscreteInput,
            ModbusFunctionCode::Read03 | ModbusFunctionCode::Write06 | ModbusFunctionCode::Write10 => {
                ModbusRegisterType::HoldingRegister
            }
            ModbusFunctionCode::Read04 => ModbusRegisterType::InputRegister,
            ModbusFunctionCode::Custom(_) => ModbusRegisterType::HoldingRegister, // Default for custom
        }
    }
}

/// Modbus register types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ModbusRegisterType {
    Coil,
    DiscreteInput,
    InputRegister,
    HoldingRegister,
}

/// Modbus data types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusDataType {
    Bool,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Float32,
    Float64,
    String(usize),
}

impl ModbusDataType {
    /// Get number of registers needed for this data type
    pub fn register_count(&self) -> u16 {
        match self {
            ModbusDataType::Bool | ModbusDataType::Int16 | ModbusDataType::UInt16 => 1,
            ModbusDataType::Int32 | ModbusDataType::UInt32 | ModbusDataType::Float32 => 2,
            ModbusDataType::Int64 | ModbusDataType::UInt64 | ModbusDataType::Float64 => 4,
            ModbusDataType::String(length) => (*length as u16 + 1) / 2,
        }
    }
}

/// Byte order for multi-register values
/// 
/// Different data types support different byte ordering options:
/// - 16-bit (1 register): AB, BA
/// - 32-bit (2 registers): ABCD, DCBA, BADC, CDAB  
/// - 64-bit (4 registers): All combinations + additional patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByteOrder {
    // 16-bit patterns (1 register)
    /// AB - Big Endian for 16-bit values
    AB,
    /// BA - Little Endian for 16-bit values  
    BA,
    
    // 32-bit patterns (2 registers)
    /// ABCD - Big Endian for 32-bit values
    ABCD,
    /// DCBA - Little Endian for 32-bit values
    DCBA,
    /// BADC - Big Endian Word Swapped for 32-bit values
    BADC,
    /// CDAB - Little Endian Word Swapped for 32-bit values
    CDAB,
    
    // 64-bit patterns (4 registers) 
    /// ABCDEFGH - Big Endian for 64-bit values
    ABCDEFGH,
    /// HGFEDCBA - Little Endian for 64-bit values
    HGFEDCBA,
    /// BADCFEHG - Word Swapped for 64-bit values
    BADCFEHG,
    /// GHEFCDAB - Double Word Swapped for 64-bit values
    GHEFCDAB,
}

impl Default for ByteOrder {
    fn default() -> Self {
        ByteOrder::AB
    }
}

impl ByteOrder {
    /// Get valid byte orders for a specific data type
    pub fn valid_for_data_type(data_type: &ModbusDataType) -> Vec<ByteOrder> {
        match data_type {
            ModbusDataType::Bool | ModbusDataType::Int16 | ModbusDataType::UInt16 => {
                vec![ByteOrder::AB, ByteOrder::BA]
            }
            ModbusDataType::Int32 | ModbusDataType::UInt32 | ModbusDataType::Float32 => {
                vec![
                    ByteOrder::ABCD,
                    ByteOrder::DCBA,
                    ByteOrder::BADC,
                    ByteOrder::CDAB,
                ]
            }
            ModbusDataType::Int64 | ModbusDataType::UInt64 | ModbusDataType::Float64 => {
                vec![
                    ByteOrder::ABCDEFGH,
                    ByteOrder::HGFEDCBA,
                    ByteOrder::BADCFEHG,
                    ByteOrder::GHEFCDAB,
                ]
            }
            ModbusDataType::String(_) => {
                vec![ByteOrder::AB, ByteOrder::BA] // String uses 16-bit chunks
            }
        }
    }
    
    /// Check if this byte order is valid for the given data type
    pub fn is_valid_for(&self, data_type: &ModbusDataType) -> bool {
        Self::valid_for_data_type(data_type).contains(self)
    }
    
    /// Get default byte order for a data type
    pub fn default_for_data_type(data_type: &ModbusDataType) -> ByteOrder {
        match data_type {
            ModbusDataType::Bool | ModbusDataType::Int16 | ModbusDataType::UInt16 => ByteOrder::AB,
            ModbusDataType::Int32 | ModbusDataType::UInt32 | ModbusDataType::Float32 => ByteOrder::ABCD,
            ModbusDataType::Int64 | ModbusDataType::UInt64 | ModbusDataType::Float64 => ByteOrder::ABCDEFGH,
            ModbusDataType::String(_) => ByteOrder::AB,
        }
    }
}

/// Point mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusRegisterMapping {
    pub name: String,
    pub slave_id: u8,
    pub function_code: ModbusFunctionCode,
    pub register_type: ModbusRegisterType,
    pub address: u16,
    pub data_type: ModbusDataType,
    pub byte_order: ByteOrder,
    pub description: Option<String>,
}

impl Default for ModbusRegisterMapping {
    fn default() -> Self {
        let data_type = ModbusDataType::UInt16;
        let function_code = ModbusFunctionCode::Read03;
        Self {
            name: String::new(),
            slave_id: 1,
            function_code,
            register_type: function_code.register_type(),
            address: 0,
            data_type,
            byte_order: ByteOrder::default_for_data_type(&data_type),
            description: None,
        }
    }
}

impl ModbusRegisterMapping {
    /// Create a new mapping with basic validation (automatically derives register_type from function_code)
    pub fn new(address: u16, data_type: ModbusDataType, name: String) -> Self {
        let function_code = ModbusFunctionCode::Read03;
        let mapping = Self {
            name,
            slave_id: 1,
            function_code,
            register_type: function_code.register_type(),
            address,
            data_type,
            byte_order: ByteOrder::default_for_data_type(&data_type),
            description: None,
        };
        mapping
    }

    /// Get the number of registers this mapping occupies
    pub fn register_count(&self) -> u16 {
        self.data_type.register_count()
    }

    /// Get the last address this mapping occupies
    pub fn end_address(&self) -> u16 {
        self.address + self.register_count() - 1
    }

    /// Validate the mapping configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate slave ID range
        if self.slave_id == 0 || self.slave_id > 247 {
            return Err(format!("Invalid slave_id: {}. Must be between 1 and 247", self.slave_id));
        }

        // Validate address range
        if self.address > 65535 {
            return Err(format!("Invalid address: {}. Must be <= 65535", self.address));
        }

        // Validate address doesn't overflow
        let end_addr = self.address as u32 + self.register_count() as u32 - 1;
        if end_addr > 65535 {
            return Err(format!(
                "Address range overflow: {} to {} exceeds maximum address 65535",
                self.address, end_addr
            ));
        }

        // Validate byte order matches data type
        if !self.byte_order.is_valid_for(&self.data_type) {
            return Err(format!(
                "Invalid byte order {:?} for data type {:?}. Valid options: {:?}",
                self.byte_order,
                self.data_type,
                ByteOrder::valid_for_data_type(&self.data_type)
            ));
        }

        // Validate function code compatibility with data type
        match (&self.function_code, &self.data_type) {
            // Coil functions (01, 05, 0F) should use Bool
            (ModbusFunctionCode::Read01 | ModbusFunctionCode::Write05 | ModbusFunctionCode::Write0F, dt) => {
                if !matches!(dt, ModbusDataType::Bool) {
                    return Err(format!("Function code {:?} requires Bool data type, got {:?}", self.function_code, dt));
                }
            }
            // Discrete input (02) should use Bool
            (ModbusFunctionCode::Read02, dt) => {
                if !matches!(dt, ModbusDataType::Bool) {
                    return Err(format!("Function code {:?} requires Bool data type, got {:?}", self.function_code, dt));
                }
            }
            // Register functions (03, 04, 06, 10) should not use Bool
            (ModbusFunctionCode::Read03 | ModbusFunctionCode::Read04 | ModbusFunctionCode::Write06 | ModbusFunctionCode::Write10, dt) => {
                if matches!(dt, ModbusDataType::Bool) {
                    return Err(format!("Function code {:?} cannot use Bool data type", self.function_code));
                }
            }
            _ => {} // Custom function codes are not validated
        }

        // Validate register_type matches function_code
        let expected_register_type = self.function_code.register_type();
        if self.register_type != expected_register_type {
            return Err(format!(
                "Register type {:?} does not match function code {:?}. Expected {:?}",
                self.register_type, self.function_code, expected_register_type
            ));
        }

        Ok(())
    }

    /// Create a new mapping with full validation (automatically derives register_type from function_code)
    pub fn new_with_validation(
        name: String,
        slave_id: u8,
        function_code: ModbusFunctionCode,
        address: u16,
        data_type: ModbusDataType,
    ) -> Result<Self, String> {
        let mapping = Self {
            name,
            slave_id,
            function_code,
            register_type: function_code.register_type(),
            address,
            data_type,
            byte_order: ByteOrder::default_for_data_type(&data_type),
            description: None,
        };
        
        mapping.validate()?;
        Ok(mapping)
    }

    /// Set byte order with validation
    pub fn with_byte_order(mut self, byte_order: ByteOrder) -> Result<Self, String> {
        if !byte_order.is_valid_for(&self.data_type) {
            return Err(format!(
                "Invalid byte order {:?} for data type {:?}. Valid options: {:?}",
                byte_order,
                self.data_type,
                ByteOrder::valid_for_data_type(&self.data_type)
            ));
        }
        self.byte_order = byte_order;
        Ok(self)
    }

    /// Set function code and automatically update register_type
    pub fn with_function_code(mut self, function_code: ModbusFunctionCode) -> Self {
        self.function_code = function_code;
        self.register_type = function_code.register_type();
        self
    }

    /// Check if this mapping is writable based on function code
    pub fn is_writable(&self) -> bool {
        matches!(
            self.function_code,
            ModbusFunctionCode::Write05 | ModbusFunctionCode::Write06 
            | ModbusFunctionCode::Write0F | ModbusFunctionCode::Write10
        )
    }

    /// Get access mode string for backward compatibility
    pub fn access_mode(&self) -> String {
        if self.is_writable() {
            "read_write".to_string()
        } else {
            "read".to_string()
        }
    }

    /// Get display name (same as name for simplicity)
    pub fn display_name(&self) -> String {
        self.name.clone()
    }

    /// Get unit (empty string as default)
    pub fn unit(&self) -> String {
        String::new()
    }

    /// Get scale factor (1.0 as default for no scaling)
    pub fn scale(&self) -> f64 {
        1.0
    }

    /// Get offset (0.0 as default for no offset)
    pub fn offset(&self) -> f64 {
        0.0
    }
}

/// Calculate CRC16 for Modbus RTU
/// 
/// # Arguments
/// * `data` - The data bytes to calculate CRC for
/// 
/// # Returns
/// * `u16` - The calculated CRC16 value
pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= *byte as u16;
        for _ in 0..8 {
            if crc & 0x0001 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_code_conversion() {
        assert_eq!(u8::from(ModbusFunctionCode::Read01), 0x01);
        assert_eq!(ModbusFunctionCode::from(0x03), ModbusFunctionCode::Read03);
        assert_eq!(u8::from(ModbusFunctionCode::Write10), 0x10);
        
        // Test custom function code
        let custom = ModbusFunctionCode::Custom(0x50);
        assert_eq!(u8::from(custom), 0x50);
    }

    #[test]
    fn test_function_code_register_type() {
        assert_eq!(ModbusFunctionCode::Read01.register_type(), ModbusRegisterType::Coil);
        assert_eq!(ModbusFunctionCode::Read02.register_type(), ModbusRegisterType::DiscreteInput);
        assert_eq!(ModbusFunctionCode::Read03.register_type(), ModbusRegisterType::HoldingRegister);
        assert_eq!(ModbusFunctionCode::Read04.register_type(), ModbusRegisterType::InputRegister);
        assert_eq!(ModbusFunctionCode::Write05.register_type(), ModbusRegisterType::Coil);
        assert_eq!(ModbusFunctionCode::Write06.register_type(), ModbusRegisterType::HoldingRegister);
        assert_eq!(ModbusFunctionCode::Write0F.register_type(), ModbusRegisterType::Coil);
        assert_eq!(ModbusFunctionCode::Write10.register_type(), ModbusRegisterType::HoldingRegister);
    }

    #[test]
    fn test_data_type_register_count() {
        assert_eq!(ModbusDataType::Bool.register_count(), 1);
        assert_eq!(ModbusDataType::UInt16.register_count(), 1);
        assert_eq!(ModbusDataType::Float32.register_count(), 2);
        assert_eq!(ModbusDataType::Float64.register_count(), 4);
        assert_eq!(ModbusDataType::String(10).register_count(), 5);
    }

    #[test]
    fn test_register_mapping() {
        let mapping = ModbusRegisterMapping::new(1000, ModbusDataType::Float32, "test".to_string());
        assert_eq!(mapping.address, 1000);
        assert_eq!(mapping.register_count(), 2);
        assert_eq!(mapping.end_address(), 1001);
        // Check auto-derived register_type
        assert_eq!(mapping.register_type, ModbusRegisterType::HoldingRegister);
    }

    #[test]
    fn test_crc16_modbus() {
        let data = [0x01, 0x03, 0x00, 0x00, 0x00, 0x02];
        let crc = crc16_modbus(&data);
        assert_eq!(crc, 0x40C0); // Expected CRC for this data
    }

    #[test]
    fn test_byte_order_validation() {
        // 16-bit data types
        assert!(ByteOrder::AB.is_valid_for(&ModbusDataType::UInt16));
        assert!(ByteOrder::BA.is_valid_for(&ModbusDataType::UInt16));
        assert!(!ByteOrder::ABCD.is_valid_for(&ModbusDataType::UInt16));

        // 32-bit data types
        assert!(ByteOrder::ABCD.is_valid_for(&ModbusDataType::Float32));
        assert!(ByteOrder::CDAB.is_valid_for(&ModbusDataType::Float32));
        assert!(!ByteOrder::AB.is_valid_for(&ModbusDataType::Float32));

        // 64-bit data types
        assert!(ByteOrder::ABCDEFGH.is_valid_for(&ModbusDataType::Float64));
        assert!(ByteOrder::HGFEDCBA.is_valid_for(&ModbusDataType::Float64));
        assert!(!ByteOrder::ABCD.is_valid_for(&ModbusDataType::Float64));

        // String data type
        assert!(ByteOrder::AB.is_valid_for(&ModbusDataType::String(10)));
        assert!(ByteOrder::BA.is_valid_for(&ModbusDataType::String(10)));
        assert!(!ByteOrder::ABCD.is_valid_for(&ModbusDataType::String(10)));
    }

    #[test]
    fn test_default_byte_order_for_data_type() {
        assert_eq!(ByteOrder::default_for_data_type(&ModbusDataType::UInt16), ByteOrder::AB);
        assert_eq!(ByteOrder::default_for_data_type(&ModbusDataType::Float32), ByteOrder::ABCD);
        assert_eq!(ByteOrder::default_for_data_type(&ModbusDataType::Float64), ByteOrder::ABCDEFGH);
    }

    #[test]
    fn test_register_mapping_validation() {
        // Valid mapping
        let mapping = ModbusRegisterMapping::new_with_validation(
            "test".to_string(),
            1,
            ModbusFunctionCode::Read03,
            1000,
            ModbusDataType::Float32,
        );
        assert!(mapping.is_ok());

        // Invalid slave ID
        let mapping = ModbusRegisterMapping::new_with_validation(
            "test".to_string(),
            0,
            ModbusFunctionCode::Read03,
            1000,
            ModbusDataType::Float32,
        );
        assert!(mapping.is_err());

        // Invalid function code for Bool type
        let mapping = ModbusRegisterMapping::new_with_validation(
            "test".to_string(),
            1,
            ModbusFunctionCode::Read03,
            1000,
            ModbusDataType::Bool,
        );
        assert!(mapping.is_err());

        // Valid coil mapping
        let mapping = ModbusRegisterMapping::new_with_validation(
            "test".to_string(),
            1,
            ModbusFunctionCode::Read01,
            1000,
            ModbusDataType::Bool,
        );
        assert!(mapping.is_ok());

        // Address overflow
        let mapping = ModbusRegisterMapping::new_with_validation(
            "test".to_string(),
            1,
            ModbusFunctionCode::Read03,
            65534,
            ModbusDataType::Float64, // Needs 4 registers
        );
        assert!(mapping.is_err());
    }

    #[test]
    fn test_byte_order_validation_in_mapping() {
        let mut mapping = ModbusRegisterMapping::new(1000, ModbusDataType::Float32, "test".to_string());
        
        // Valid byte order for Float32
        let result = mapping.clone().with_byte_order(ByteOrder::ABCD);
        assert!(result.is_ok());

        // Invalid byte order for Float32
        let result = mapping.with_byte_order(ByteOrder::AB);
        assert!(result.is_err());
    }

    #[test]
    fn test_auto_register_type_derivation() {
        // Test new_with_validation automatically derives register_type
        let mapping = ModbusRegisterMapping::new_with_validation(
            "test".to_string(),
            1,
            ModbusFunctionCode::Read04,
            1000,
            ModbusDataType::Float32,
        ).unwrap();
        assert_eq!(mapping.register_type, ModbusRegisterType::InputRegister);

        // Test with_function_code updates register_type
        let mapping = ModbusRegisterMapping::new(1000, ModbusDataType::Bool, "test".to_string())
            .with_function_code(ModbusFunctionCode::Read01);
        assert_eq!(mapping.register_type, ModbusRegisterType::Coil);
    }

    #[test]
    fn test_register_type_function_code_validation() {
        // Create mapping with mismatched register_type and function_code
        let mut mapping = ModbusRegisterMapping::new_with_validation(
            "test".to_string(),
            1,
            ModbusFunctionCode::Read03,
            1000,
            ModbusDataType::Float32,
        ).unwrap();
        
        // Manually set wrong register_type
        mapping.register_type = ModbusRegisterType::Coil;
        
        // Should fail validation
        assert!(mapping.validate().is_err());
    }
} 