//! PDU (Protocol Data Unit) processing tests
//!
//! Tests for Modbus PDU building and parsing functionality.

#[cfg(test)]
mod tests {
    use crate::plugins::protocols::modbus::common::ModbusFunctionCode;
    use crate::plugins::protocols::modbus::pdu::ModbusExceptionCode;

    #[test]
    fn test_function_code_conversion() {
        // Test function code to u8 conversion
        assert_eq!(u8::from(ModbusFunctionCode::Read01), 0x01);
        assert_eq!(u8::from(ModbusFunctionCode::Read03), 0x03);
        assert_eq!(u8::from(ModbusFunctionCode::Write05), 0x05);
        assert_eq!(u8::from(ModbusFunctionCode::Write06), 0x06);
    }

    #[test]
    fn test_function_code_from() {
        // Test u8 to function code conversion
        assert_eq!(ModbusFunctionCode::from(0x01), ModbusFunctionCode::Read01);
        assert_eq!(ModbusFunctionCode::from(0x03), ModbusFunctionCode::Read03);
        assert_eq!(
            ModbusFunctionCode::from(0xFF),
            ModbusFunctionCode::Custom(0xFF)
        ); // Custom code
    }

    // Additional PDU tests will be added with PDU processor implementation
}
