//! PDU (Protocol Data Unit) processing tests
//! 
//! Tests for Modbus PDU building and parsing functionality.

#[cfg(test)]
mod tests {
    use crate::core::protocols::modbus::pdu::{ModbusFunctionCode, ModbusExceptionCode};
    
    #[test]
    fn test_function_code_conversion() {
        // Test function code to u8 conversion
        assert_eq!(u8::from(ModbusFunctionCode::ReadCoils), 0x01);
        assert_eq!(u8::from(ModbusFunctionCode::ReadHoldingRegisters), 0x03);
        assert_eq!(u8::from(ModbusFunctionCode::WriteSingleCoil), 0x05);
        assert_eq!(u8::from(ModbusFunctionCode::WriteSingleRegister), 0x06);
    }
    
    #[test]
    fn test_function_code_try_from() {
        // Test u8 to function code conversion
        assert!(ModbusFunctionCode::try_from(0x01).is_ok());
        assert!(ModbusFunctionCode::try_from(0x03).is_ok());
        assert!(ModbusFunctionCode::try_from(0xFF).is_err()); // Invalid code
    }
    
    // TODO: Add more PDU tests when PDU processor is implemented
}