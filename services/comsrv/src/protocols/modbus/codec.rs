//! Modbus codec implementation
//!
//! Handles encoding and decoding of Modbus data types and values

use super::constants;
use super::pdu::{ModbusPdu, PduBuilder};
use crate::core::combase::RedisValue;
use crate::utils::bytes::{regs_to_bytes_4, regs_to_bytes_8, ByteOrder};
use crate::utils::error::{ComSrvError, Result};
use tracing::trace;

/// Modbus codec for data encoding/decoding
pub struct ModbusCodec;

impl ModbusCodec {
    /// Build write PDU for FC05 (Write Single Coil)
    pub fn build_write_fc05_single_coil_pdu(address: u16, value: bool) -> Result<ModbusPdu> {
        // FC05 request value is 0xFF00 for ON, 0x0000 for OFF
        Ok(PduBuilder::new()
            .function_code(0x05)?
            .address(address)?
            .byte(if value { 0xFF } else { 0x00 })?
            .byte(0x00)?
            .build())
    }

    /// Build write PDU for FC06 (Write Single Register)
    pub fn build_write_fc06_single_register_pdu(address: u16, value: u16) -> Result<ModbusPdu> {
        // Use quantity() to push a u16 value
        Ok(PduBuilder::new()
            .function_code(0x06)?
            .address(address)?
            .quantity(value)?
            .build())
    }

    /// Build write PDU for FC15 (Write Multiple Coils)
    pub fn build_write_fc15_multiple_coils_pdu(
        start_address: u16,
        values: &[bool],
    ) -> Result<ModbusPdu> {
        if values.is_empty() || values.len() > constants::MODBUS_MAX_WRITE_COILS {
            return Err(ComSrvError::ProtocolError(
                "Invalid coil count for FC15".to_string(),
            ));
        }

        let mut pdu = ModbusPdu::new();

        // Function code
        pdu.push(0x0F)?;

        // Starting address
        pdu.push_u16(start_address)?;

        // Quantity of coils
        let quantity = values.len() as u16;
        pdu.push_u16(quantity)?;

        // Byte count
        let byte_count = values.len().div_ceil(8) as u8;
        pdu.push(byte_count)?;

        // Coil values (packed as bits)
        let mut current_byte = 0u8;
        let mut bit_index = 0;

        for &value in values {
            if value {
                current_byte |= 1 << bit_index;
            }
            bit_index += 1;

            if bit_index == 8 {
                pdu.push(current_byte)?;
                current_byte = 0;
                bit_index = 0;
            }
        }

        // Push last byte if needed
        if bit_index > 0 {
            pdu.push(current_byte)?;
        }

        Ok(pdu)
    }

    /// Build write PDU for FC16 (Write Multiple Registers)
    pub fn build_write_fc16_multiple_registers_pdu(
        start_address: u16,
        values: &[u16],
    ) -> Result<ModbusPdu> {
        if values.is_empty() || values.len() > constants::MODBUS_MAX_WRITE_REGISTERS {
            return Err(ComSrvError::ProtocolError(
                "Invalid register count for FC16".to_string(),
            ));
        }

        let mut pdu = ModbusPdu::new();

        // Function code
        pdu.push(0x10)?;

        // Starting address
        pdu.push_u16(start_address)?;

        // Quantity of registers
        let quantity = values.len() as u16;
        pdu.push_u16(quantity)?;

        // Byte count
        let byte_count = (values.len() * 2) as u8;
        pdu.push(byte_count)?;

        // Register values
        for &value in values {
            pdu.push_u16(value)?;
        }

        Ok(pdu)
    }

    /// Parse write response PDU
    pub fn parse_modbus_write_response(pdu: &ModbusPdu, expected_fc: u8) -> Result<bool> {
        let data = pdu.as_slice();

        if data.is_empty() {
            return Err(ComSrvError::ProtocolError("Empty response PDU".to_string()));
        }

        // Check for exception response
        if data[0] & 0x80 != 0 {
            let exception_code = if data.len() > 1 { data[1] } else { 0 };
            return Err(ComSrvError::ProtocolError(format!(
                "Modbus exception response: code {:02X}",
                exception_code
            )));
        }

        // Verify function code
        if data[0] != expected_fc {
            return Err(ComSrvError::ProtocolError(format!(
                "Function code mismatch: expected {:02X}, got {:02X}",
                expected_fc, data[0]
            )));
        }

        // For write operations, a matching function code indicates success
        Ok(true)
    }

    /// Encode value for Modbus transmission
    pub fn encode_value_for_modbus(
        value: &RedisValue,
        data_type: &str,
        byte_order: Option<&str>,
    ) -> Result<Vec<u16>> {
        match data_type {
            "bool" | "boolean" => {
                let bool_val = match value {
                    RedisValue::Integer(i) => *i != 0,
                    RedisValue::Float(f) => *f != 0.0,
                    RedisValue::String(s) => {
                        s.to_lowercase() == "true" || s == "1" || s.to_lowercase() == "on"
                    },
                    _ => false,
                };
                Ok(vec![if bool_val { 1 } else { 0 }])
            },
            "uint16" | "u16" | "word" => {
                let val = match value {
                    RedisValue::Integer(i) => *i as u16,
                    RedisValue::Float(f) => f.round() as u16, // 四舍五入，避免精度损失 (round to nearest to avoid precision loss)
                    RedisValue::String(s) => s.parse::<u16>().unwrap_or(0),
                    _ => 0,
                };
                Ok(vec![val])
            },
            "int16" | "i16" | "short" => {
                let val = match value {
                    RedisValue::Integer(i) => *i as i16,
                    RedisValue::Float(f) => f.round() as i16, // 四舍五入 (round to nearest)
                    RedisValue::String(s) => s.parse::<i16>().unwrap_or(0),
                    _ => 0,
                };
                Ok(vec![val as u16])
            },
            "uint32" | "u32" | "dword" => {
                let val = match value {
                    RedisValue::Integer(i) => *i as u32,
                    RedisValue::Float(f) => f.round() as u32, // 四舍五入 (round to nearest)
                    RedisValue::String(s) => s.parse::<u32>().unwrap_or(0),
                    _ => 0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "int32" | "i32" | "long" => {
                let val = match value {
                    RedisValue::Integer(i) => *i as i32,
                    RedisValue::Float(f) => f.round() as i32, // 四舍五入 (round to nearest)
                    RedisValue::String(s) => s.parse::<i32>().unwrap_or(0),
                    _ => 0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "float32" | "f32" | "float" | "real" => {
                let val = match value {
                    RedisValue::Float(f) => *f as f32,
                    RedisValue::Integer(i) => *i as f32,
                    RedisValue::String(s) => s.parse::<f32>().unwrap_or(0.0),
                    _ => 0.0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "float64" | "f64" | "double" | "lreal" => {
                let val = match value {
                    RedisValue::Float(f) => *f,
                    RedisValue::Integer(i) => *i as f64,
                    RedisValue::String(s) => s.parse::<f64>().unwrap_or(0.0),
                    _ => 0.0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "uint64" | "u64" | "qword" => {
                let val = match value {
                    RedisValue::Integer(i) => *i as u64,
                    RedisValue::Float(f) => f.round() as u64, // 四舍五入 (round to nearest)
                    RedisValue::String(s) => s.parse::<u64>().unwrap_or(0),
                    _ => 0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "int64" | "i64" | "longlong" => {
                let val = match value {
                    RedisValue::Integer(i) => *i,
                    RedisValue::Float(f) => f.round() as i64, // 四舍五入 (round to nearest)
                    RedisValue::String(s) => s.parse::<i64>().unwrap_or(0),
                    _ => 0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            _ => Err(ComSrvError::ConfigError(format!(
                "Unsupported data type for encoding: {}",
                data_type
            ))),
        }
    }

    /// Convert bytes to registers using ByteOrder enum (type-safe)
    ///
    /// Inverse operation of `registers_to_bytes_typed`. Converts byte array
    /// back to register array with specified byte ordering.
    ///
    /// # Arguments
    /// * `bytes` - Input byte array (2, 4, or 8 bytes)
    /// * `order` - Byte order specification (enum)
    ///
    /// # Returns
    /// Register vector with values reconstructed according to byte order
    ///
    /// # Errors
    /// Returns error if byte length is not 2, 4, or 8
    fn bytes_to_registers_typed(bytes: &[u8], order: ByteOrder) -> Result<Vec<u16>> {
        match bytes.len() {
            2 => {
                // Single register (16-bit) - no byte order applies
                let register = u16::from_be_bytes([bytes[0], bytes[1]]);
                Ok(vec![register])
            },
            4 => {
                // Two registers (32-bit) - reconstruct based on byte order
                let registers = match order {
                    ByteOrder::BigEndian | ByteOrder::BigEndian16 => vec![
                        u16::from_be_bytes([bytes[0], bytes[1]]),
                        u16::from_be_bytes([bytes[2], bytes[3]]),
                    ],
                    ByteOrder::LittleEndian | ByteOrder::LittleEndian16 => vec![
                        u16::from_be_bytes([bytes[3], bytes[2]]),
                        u16::from_be_bytes([bytes[1], bytes[0]]),
                    ],
                    ByteOrder::LittleEndianSwap => vec![
                        u16::from_be_bytes([bytes[1], bytes[0]]),
                        u16::from_be_bytes([bytes[3], bytes[2]]),
                    ],
                    ByteOrder::BigEndianSwap => vec![
                        u16::from_be_bytes([bytes[2], bytes[3]]),
                        u16::from_be_bytes([bytes[0], bytes[1]]),
                    ],
                };
                Ok(registers)
            },
            8 => {
                // Four registers (64-bit)
                let registers = match order {
                    ByteOrder::BigEndian | ByteOrder::BigEndian16 => vec![
                        u16::from_be_bytes([bytes[0], bytes[1]]),
                        u16::from_be_bytes([bytes[2], bytes[3]]),
                        u16::from_be_bytes([bytes[4], bytes[5]]),
                        u16::from_be_bytes([bytes[6], bytes[7]]),
                    ],
                    ByteOrder::LittleEndian | ByteOrder::LittleEndian16 => vec![
                        u16::from_be_bytes([bytes[7], bytes[6]]),
                        u16::from_be_bytes([bytes[5], bytes[4]]),
                        u16::from_be_bytes([bytes[3], bytes[2]]),
                        u16::from_be_bytes([bytes[1], bytes[0]]),
                    ],
                    _ => {
                        // Other byte orders not supported for 64-bit, default to BigEndian
                        vec![
                            u16::from_be_bytes([bytes[0], bytes[1]]),
                            u16::from_be_bytes([bytes[2], bytes[3]]),
                            u16::from_be_bytes([bytes[4], bytes[5]]),
                            u16::from_be_bytes([bytes[6], bytes[7]]),
                        ]
                    },
                };
                Ok(registers)
            },
            _ => Err(ComSrvError::ProtocolError(format!(
                "Unsupported byte length for register conversion: {} (must be 2, 4, or 8)",
                bytes.len()
            ))),
        }
    }

    /// Convert bytes to registers with specified byte order
    pub fn convert_bytes_to_registers_with_order(
        bytes: &[u8],
        byte_order: Option<&str>,
    ) -> Result<Vec<u16>> {
        let order = byte_order
            .and_then(ByteOrder::from_str)
            .unwrap_or(ByteOrder::BigEndian);

        Self::bytes_to_registers_typed(bytes, order)
    }

    /// Convert registers to bytes using ByteOrder enum (type-safe)
    ///
    /// This is the new type-safe implementation that uses the ByteOrder enum
    /// instead of string-based byte order specification.
    ///
    /// # Arguments
    /// * `registers` - Input register array
    /// * `order` - Byte order specification (enum)
    ///
    /// # Returns
    /// Byte vector with specified byte ordering
    ///
    /// # Errors
    /// Returns error if register length is not supported (must be 1, 2, or 4)
    fn registers_to_bytes_typed(registers: &[u16], order: ByteOrder) -> Result<Vec<u8>> {
        match registers.len() {
            1 => {
                // Single register (16-bit) - no byte order applies
                Ok(registers[0].to_be_bytes().to_vec())
            },
            2 => {
                // Two registers (32-bit) - use utils::bytes
                let regs: [u16; 2] = [registers[0], registers[1]];
                Ok(regs_to_bytes_4(&regs, order).to_vec())
            },
            4 => {
                // Four registers (64-bit) - use utils::bytes
                let regs: [u16; 4] = [registers[0], registers[1], registers[2], registers[3]];
                Ok(regs_to_bytes_8(&regs, order).to_vec())
            },
            _ => Err(ComSrvError::ProtocolError(format!(
                "Unsupported register count for conversion: {} (must be 1, 2, or 4)",
                registers.len()
            ))),
        }
    }

    /// Convert registers to bytes with specified byte order (legacy string interface)
    ///
    /// This function maintains backward compatibility by accepting string-based
    /// byte order specification. Internally, it converts the string to ByteOrder
    /// enum and uses the type-safe implementation.
    ///
    /// # Arguments
    /// * `registers` - Input register array
    /// * `byte_order` - Optional byte order string ("ABCD", "DCBA", "CDAB", "BADC", etc.)
    ///
    /// # Returns
    /// Byte vector with specified byte ordering
    ///
    /// # Note
    /// If byte_order is invalid, defaults to BigEndian (ABCD)
    pub fn convert_registers_with_byte_order(
        registers: &[u16],
        byte_order: Option<&str>,
    ) -> Vec<u8> {
        // Convert string to ByteOrder enum
        let order = byte_order
            .and_then(ByteOrder::from_str)
            .unwrap_or(ByteOrder::BigEndian);

        // Use type-safe implementation
        Self::registers_to_bytes_typed(registers, order).unwrap_or_else(|err| {
            trace!(
                "Error converting registers: {}, falling back to big-endian",
                err
            );
            // Fallback: concatenate registers as big-endian
            registers.iter().flat_map(|r| r.to_be_bytes()).collect()
        })
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    // ============================================================================
    // Phase 1: Write PDU construction tests
    // ============================================================================

    // ---------- FC05 single-coil write tests ----------

    #[test]
    fn test_build_fc05_write_true() {
        let pdu = ModbusCodec::build_write_fc05_single_coil_pdu(0x0100, true).unwrap();

        // FC05 format: [FC, Address_Hi, Address_Lo, Value_Hi, Value_Lo]
        // true = 0xFF00
        assert_eq!(pdu.as_slice(), &[0x05, 0x01, 0x00, 0xFF, 0x00]);
        assert_eq!(pdu.function_code(), Some(0x05));
    }

    #[test]
    fn test_build_fc05_write_false() {
        let pdu = ModbusCodec::build_write_fc05_single_coil_pdu(0x0200, false).unwrap();

        // false = 0x0000
        assert_eq!(pdu.as_slice(), &[0x05, 0x02, 0x00, 0x00, 0x00]);
        assert_eq!(pdu.function_code(), Some(0x05));
    }

    #[test]
    fn test_build_fc05_different_addresses() {
        // Test minimum address
        let pdu_min = ModbusCodec::build_write_fc05_single_coil_pdu(0x0000, true).unwrap();
        assert_eq!(pdu_min.as_slice()[1..3], [0x00, 0x00]); // Address bytes

        // Test maximum address
        let pdu_max = ModbusCodec::build_write_fc05_single_coil_pdu(0xFFFF, false).unwrap();
        assert_eq!(pdu_max.as_slice()[1..3], [0xFF, 0xFF]); // Address bytes
    }

    // ---------- FC06 single-register write tests ----------

    #[test]
    fn test_build_fc06_zero_value() {
        let pdu = ModbusCodec::build_write_fc06_single_register_pdu(0x0100, 0x0000).unwrap();

        // FC06 format: [FC, Address_Hi, Address_Lo, Value_Hi, Value_Lo]
        assert_eq!(pdu.as_slice(), &[0x06, 0x01, 0x00, 0x00, 0x00]);
        assert_eq!(pdu.function_code(), Some(0x06));
    }

    #[test]
    fn test_build_fc06_max_value() {
        let pdu = ModbusCodec::build_write_fc06_single_register_pdu(0x0200, 0xFFFF).unwrap();

        assert_eq!(pdu.as_slice(), &[0x06, 0x02, 0x00, 0xFF, 0xFF]);
    }

    #[test]
    fn test_build_fc06_typical_value() {
        let pdu = ModbusCodec::build_write_fc06_single_register_pdu(0x0300, 0x1234).unwrap();

        assert_eq!(pdu.as_slice(), &[0x06, 0x03, 0x00, 0x12, 0x34]);
    }

    // ---------- FC15 multi-coil write tests ----------

    #[test]
    fn test_build_fc15_empty_array_error() {
        let result = ModbusCodec::build_write_fc15_multiple_coils_pdu(0x0100, &[]);

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Invalid coil count"));
        }
    }

    #[test]
    fn test_build_fc15_single_coil() {
        let pdu = ModbusCodec::build_write_fc15_multiple_coils_pdu(0x0100, &[true]).unwrap();

        // FC15 format: [FC, Address_Hi, Address_Lo, Quantity_Hi, Quantity_Lo, Byte_Count, Data...]
        assert_eq!(
            pdu.as_slice(),
            &[
                0x0F, // Function code
                0x01, 0x00, // Start address
                0x00, 0x01, // Quantity = 1
                0x01, // Byte count = 1
                0x01, // Data: bit 0 = 1
            ]
        );
    }

    #[test]
    fn test_build_fc15_multiple_coils_within_byte() {
        // Test 5 coils: [true, false, true, true, false]
        // Binary: 0b00001101 = 0x0D (LSB first)
        let pdu = ModbusCodec::build_write_fc15_multiple_coils_pdu(
            0x0200,
            &[true, false, true, true, false],
        )
        .unwrap();

        assert_eq!(
            pdu.as_slice(),
            &[
                0x0F, // Function code
                0x02, 0x00, // Start address
                0x00, 0x05, // Quantity = 5
                0x01, // Byte count = 1
                0x0D, // Data: bits = 00001101
            ]
        );
    }

    #[test]
    fn test_build_fc15_multiple_coils_cross_byte() {
        // Test 10 coils across 2 bytes
        // First 8: [T,F,T,T,F,F,F,T] = 0b10001101 = 0x8D
        // Next 2:  [T,T] = 0b00000011 = 0x03
        let coils = vec![
            true, false, true, true, false, false, false, true, // Byte 1
            true, true, // Byte 2
        ];

        let pdu = ModbusCodec::build_write_fc15_multiple_coils_pdu(0x0300, &coils).unwrap();

        assert_eq!(
            pdu.as_slice(),
            &[
                0x0F, // Function code
                0x03, 0x00, // Start address
                0x00, 0x0A, // Quantity = 10
                0x02, // Byte count = 2
                0x8D, // First byte
                0x03, // Second byte
            ]
        );
    }

    #[test]
    fn test_build_fc15_bit_packing_verification() {
        // Verify bit packing: bit 0 is LSB
        let coils = vec![
            true,  // bit 0
            true,  // bit 1
            false, // bit 2
            false, // bit 3
            true,  // bit 4
            false, // bit 5
            false, // bit 6
            false, // bit 7
        ];

        // Expected: 0b00010011 = 0x13
        let pdu = ModbusCodec::build_write_fc15_multiple_coils_pdu(0x0100, &coils).unwrap();

        let data_byte = pdu.as_slice()[6]; // Data starts at index 6
        assert_eq!(data_byte, 0x13, "Bit packing incorrect");
    }

    #[test]
    fn test_build_fc15_exceed_max_coils_error() {
        // Modbus spec: max 2000 coils for FC15
        let coils = vec![false; 2001];
        let result = ModbusCodec::build_write_fc15_multiple_coils_pdu(0x0100, &coils);

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Invalid coil count"));
        }
    }

    // ---------- FC16 multi-register write tests ----------

    #[test]
    fn test_build_fc16_empty_array_error() {
        let result = ModbusCodec::build_write_fc16_multiple_registers_pdu(0x0100, &[]);

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Invalid register count"));
        }
    }

    #[test]
    fn test_build_fc16_single_register() {
        let pdu = ModbusCodec::build_write_fc16_multiple_registers_pdu(0x0100, &[0x1234]).unwrap();

        // FC16 format: [FC, Address_Hi, Address_Lo, Quantity_Hi, Quantity_Lo, Byte_Count, Data...]
        assert_eq!(
            pdu.as_slice(),
            &[
                0x10, // Function code
                0x01, 0x00, // Start address
                0x00, 0x01, // Quantity = 1
                0x02, // Byte count = 2
                0x12, 0x34, // Register value
            ]
        );
    }

    #[test]
    fn test_build_fc16_multiple_registers() {
        let registers = vec![0xABCD, 0x1234, 0x5678];
        let pdu = ModbusCodec::build_write_fc16_multiple_registers_pdu(0x0200, &registers).unwrap();

        assert_eq!(
            pdu.as_slice(),
            &[
                0x10, // Function code
                0x02, 0x00, // Start address
                0x00, 0x03, // Quantity = 3
                0x06, // Byte count = 6
                0xAB, 0xCD, // Register 1
                0x12, 0x34, // Register 2
                0x56, 0x78, // Register 3
            ]
        );
    }

    #[test]
    fn test_build_fc16_exceed_max_registers_error() {
        // Modbus spec: max 123 registers for FC16
        let registers = vec![0u16; 124];
        let result = ModbusCodec::build_write_fc16_multiple_registers_pdu(0x0100, &registers);

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Invalid register count"));
        }
    }

    // ============================================================================
    // Phase 2: response parsing tests
    // ============================================================================

    #[test]
    fn test_parse_write_response_success() {
        // Create a valid write response PDU for FC06
        let mut pdu = ModbusPdu::new();
        pdu.push(0x06).unwrap(); // Function code
        pdu.push_u16(0x0100).unwrap(); // Address
        pdu.push_u16(0x1234).unwrap(); // Value

        let result = ModbusCodec::parse_modbus_write_response(&pdu, 0x06);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_parse_write_response_exception() {
        // Create an exception response: FC with 0x80 bit set
        let mut pdu = ModbusPdu::new();
        pdu.push(0x86).unwrap(); // 0x06 | 0x80 = exception
        pdu.push(0x02).unwrap(); // Exception code: Illegal Data Address

        let result = ModbusCodec::parse_modbus_write_response(&pdu, 0x06);
        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("exception"));
            assert!(error_msg.contains("02")); // Exception code
        }
    }

    #[test]
    fn test_parse_write_response_function_code_mismatch() {
        // Response with wrong function code
        let mut pdu = ModbusPdu::new();
        pdu.push(0x10).unwrap(); // FC16, but we expect FC06
        pdu.push_u16(0x0100).unwrap();

        let result = ModbusCodec::parse_modbus_write_response(&pdu, 0x06);
        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("mismatch"));
            assert!(error_msg.contains("10")); // Got FC
            assert!(error_msg.contains("06")); // Expected FC
        }
    }

    #[test]
    fn test_parse_write_response_empty_pdu() {
        let pdu = ModbusPdu::new();

        let result = ModbusCodec::parse_modbus_write_response(&pdu, 0x06);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Empty response"));
        }
    }

    #[test]
    fn test_parse_write_response_various_function_codes() {
        // Test all write function codes
        let function_codes = vec![0x05, 0x06, 0x0F, 0x10];

        for fc in function_codes {
            let mut pdu = ModbusPdu::new();
            pdu.push(fc).unwrap();
            pdu.push_u16(0x0100).unwrap(); // Address
            pdu.push_u16(0x0001).unwrap(); // Value/Quantity

            let result = ModbusCodec::parse_modbus_write_response(&pdu, fc);
            assert!(result.is_ok(), "FC {:02X} should succeed", fc);
        }
    }

    // ============================================================================
    // Phase 3: data encoding tests
    // ============================================================================

    // ---------- bool/boolean type tests ----------

    #[test]
    fn test_encode_bool_from_integer() {
        // Integer 0 = false
        let val = RedisValue::Integer(0);
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![0]);

        // Integer 1 = true
        let val = RedisValue::Integer(1);
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![1]);

        // Any non-zero integer = true
        let val = RedisValue::Integer(5);
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_encode_bool_from_float() {
        // Float 0.0 = false
        let val = RedisValue::Float(0.0);
        let result = ModbusCodec::encode_value_for_modbus(&val, "boolean", None).unwrap();
        assert_eq!(result, vec![0]);

        // Any non-zero float = true
        let val = RedisValue::Float(1.5);
        let result = ModbusCodec::encode_value_for_modbus(&val, "boolean", None).unwrap();
        assert_eq!(result, vec![1]);

        let val = RedisValue::Float(-0.1);
        let result = ModbusCodec::encode_value_for_modbus(&val, "boolean", None).unwrap();
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_encode_bool_from_string() {
        use std::borrow::Cow;

        // "true" = true
        let val = RedisValue::String(Cow::Borrowed("true"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![1]);

        // "false" = false
        let val = RedisValue::String(Cow::Borrowed("false"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![0]);

        // "1" = true
        let val = RedisValue::String(Cow::Borrowed("1"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![1]);

        // "0" = false
        let val = RedisValue::String(Cow::Borrowed("0"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![0]);

        // "on" = true
        let val = RedisValue::String(Cow::Borrowed("on"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![1]);

        // "ON" = true (case insensitive)
        let val = RedisValue::String(Cow::Borrowed("ON"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_encode_bool_from_null() {
        let val = RedisValue::Null;
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![0]); // Null = false
    }

    // ---------- uint16/u16/word type tests ----------

    #[test]
    fn test_encode_uint16_boundary_values() {
        // Zero
        let val = RedisValue::Integer(0);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint16", None).unwrap();
        assert_eq!(result, vec![0]);

        // Maximum u16
        let val = RedisValue::Integer(65535);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint16", None).unwrap();
        assert_eq!(result, vec![65535]);

        // Typical value
        let val = RedisValue::Integer(1234);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint16", None).unwrap();
        assert_eq!(result, vec![1234]);
    }

    #[test]
    fn test_encode_uint16_from_various_types() {
        use std::borrow::Cow;

        // From Float
        let val = RedisValue::Float(123.7);
        let result = ModbusCodec::encode_value_for_modbus(&val, "u16", None).unwrap();
        assert_eq!(result, vec![124]); // Rounded (123.7 → 124)

        // From String
        let val = RedisValue::String(Cow::Borrowed("456"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "word", None).unwrap();
        assert_eq!(result, vec![456]);

        // From invalid String (should default to 0)
        let val = RedisValue::String(Cow::Borrowed("invalid"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint16", None).unwrap();
        assert_eq!(result, vec![0]);
    }

    // ---------- int16/i16/short type tests ----------

    #[test]
    fn test_encode_int16_positive_negative() {
        // Positive value
        let val = RedisValue::Integer(1234);
        let result = ModbusCodec::encode_value_for_modbus(&val, "int16", None).unwrap();
        assert_eq!(result, vec![1234]);

        // Negative value
        let val = RedisValue::Integer(-100);
        let result = ModbusCodec::encode_value_for_modbus(&val, "int16", None).unwrap();
        // -100 as i16, then cast to u16
        let expected = (-100i16) as u16;
        assert_eq!(result, vec![expected]);
    }

    #[test]
    fn test_encode_int16_boundary_values() {
        // Maximum positive i16
        let val = RedisValue::Integer(32767);
        let result = ModbusCodec::encode_value_for_modbus(&val, "i16", None).unwrap();
        assert_eq!(result, vec![32767]);

        // Minimum negative i16
        let val = RedisValue::Integer(-32768);
        let result = ModbusCodec::encode_value_for_modbus(&val, "short", None).unwrap();
        let expected = (-32768i16) as u16;
        assert_eq!(result, vec![expected]);

        // Zero
        let val = RedisValue::Integer(0);
        let result = ModbusCodec::encode_value_for_modbus(&val, "int16", None).unwrap();
        assert_eq!(result, vec![0]);
    }

    // ---------- uint32/u32/dword type tests ----------

    #[test]
    fn test_encode_uint32_with_byte_order_abcd() {
        // Value: 0x12345678
        // ABCD (big-endian): [0x1234, 0x5678]
        let val = RedisValue::Integer(0x12345678);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
        assert_eq!(result, vec![0x1234, 0x5678]);
    }

    #[test]
    fn test_encode_uint32_with_byte_order_dcba() {
        // Value: 0x12345678
        // DCBA (little-endian): [0x7856, 0x3412]
        let val = RedisValue::Integer(0x12345678);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("DCBA")).unwrap();
        assert_eq!(result, vec![0x7856, 0x3412]);
    }

    #[test]
    fn test_encode_uint32_with_byte_order_badc() {
        // Value: 0x12345678
        // BADC (middle-endian): [0x3412, 0x7856]
        let val = RedisValue::Integer(0x12345678);
        let result = ModbusCodec::encode_value_for_modbus(&val, "dword", Some("BADC")).unwrap();
        assert_eq!(result, vec![0x3412, 0x7856]);
    }

    #[test]
    fn test_encode_uint32_with_byte_order_cdab() {
        // Value: 0x12345678
        // CDAB (swapped word order): [0x5678, 0x1234]
        let val = RedisValue::Integer(0x12345678);
        let result = ModbusCodec::encode_value_for_modbus(&val, "u32", Some("CDAB")).unwrap();
        assert_eq!(result, vec![0x5678, 0x1234]);
    }

    #[test]
    fn test_encode_uint32_boundary_values() {
        // Zero
        let val = RedisValue::Integer(0);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint32", None).unwrap();
        assert_eq!(result, vec![0x0000, 0x0000]);

        // Maximum u32
        let val = RedisValue::Integer(0xFFFFFFFF);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint32", None).unwrap();
        assert_eq!(result, vec![0xFFFF, 0xFFFF]);
    }

    // ---------- int32/i32/long type tests ----------

    #[test]
    fn test_encode_int32_positive_with_byte_order() {
        // Positive value: 305419896 = 0x12345678
        let val = RedisValue::Integer(305419896);
        let result = ModbusCodec::encode_value_for_modbus(&val, "int32", Some("ABCD")).unwrap();
        assert_eq!(result, vec![0x1234, 0x5678]);
    }

    #[test]
    fn test_encode_int32_negative_values() {
        // Negative value: -100
        let val = RedisValue::Integer(-100);
        let result = ModbusCodec::encode_value_for_modbus(&val, "int32", Some("ABCD")).unwrap();

        // -100 as i32 = 0xFFFFFF9C
        // ABCD: [0xFFFF, 0xFF9C]
        assert_eq!(result, vec![0xFFFF, 0xFF9C]);
    }

    #[test]
    fn test_encode_int32_boundary_values() {
        use std::borrow::Cow;

        // From String
        let val = RedisValue::String(Cow::Borrowed("2147483647"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "long", None).unwrap();
        // 2147483647 = 0x7FFFFFFF
        assert_eq!(result, vec![0x7FFF, 0xFFFF]);
    }

    // ---------- float32/f32/float/real type tests ----------

    #[test]
    fn test_encode_float32_typical_values() {
        // Positive value
        let val = RedisValue::Float(123.456);
        let result = ModbusCodec::encode_value_for_modbus(&val, "float32", Some("ABCD")).unwrap();

        // Convert back to verify
        let bytes = ModbusCodec::convert_registers_with_byte_order(&result, Some("ABCD"));
        let reconstructed = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert!((reconstructed - 123.456).abs() < 0.001);
    }

    #[test]
    fn test_encode_float32_with_byte_orders() {
        let val = RedisValue::Float(25.5);

        // Test ABCD
        let result_abcd =
            ModbusCodec::encode_value_for_modbus(&val, "float", Some("ABCD")).unwrap();
        assert_eq!(result_abcd.len(), 2);

        // Test DCBA
        let result_dcba =
            ModbusCodec::encode_value_for_modbus(&val, "float", Some("DCBA")).unwrap();
        assert_eq!(result_dcba.len(), 2);

        // They should be different
        assert_ne!(result_abcd, result_dcba);
    }

    #[test]
    fn test_encode_float32_special_values() {
        // Zero
        let val = RedisValue::Float(0.0);
        let result = ModbusCodec::encode_value_for_modbus(&val, "real", None).unwrap();
        assert_eq!(result, vec![0x0000, 0x0000]);

        // Negative value
        let val = RedisValue::Float(-10.5);
        let result = ModbusCodec::encode_value_for_modbus(&val, "f32", None).unwrap();
        assert_eq!(result.len(), 2);
    }

    // ---------- float64/f64/double/lreal type tests ----------

    #[test]
    fn test_encode_float64_with_byte_order() {
        let val = RedisValue::Float(123.456789);

        // ABCD (big-endian)
        let result = ModbusCodec::encode_value_for_modbus(&val, "float64", Some("ABCD")).unwrap();
        assert_eq!(result.len(), 4); // 64 bits = 4 registers

        // Convert back to verify
        let bytes = ModbusCodec::convert_registers_with_byte_order(&result, Some("ABCD"));
        let reconstructed = f64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        assert!((reconstructed - 123.456789).abs() < 0.000001);
    }

    #[test]
    fn test_encode_float64_precision() {
        use std::borrow::Cow;

        // High precision value from String
        let val = RedisValue::String(Cow::Borrowed("3.141592653589793"));
        let result = ModbusCodec::encode_value_for_modbus(&val, "double", None).unwrap();
        assert_eq!(result.len(), 4);

        // Verify precision is maintained
        let bytes = ModbusCodec::convert_registers_with_byte_order(&result, None);
        let reconstructed = f64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        assert!((reconstructed - std::f64::consts::PI).abs() < 0.000000000000001);
    }

    #[test]
    fn test_encode_float64_byte_order_dcba() {
        let val = RedisValue::Float(999.888);

        // DCBA encoding
        let result = ModbusCodec::encode_value_for_modbus(&val, "lreal", Some("DCBA")).unwrap();
        assert_eq!(result.len(), 4);

        // Verify roundtrip consistency (encode → decode should give same registers)
        let bytes = ModbusCodec::convert_registers_with_byte_order(&result, Some("DCBA"));
        let reconstructed =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("DCBA")).unwrap();
        assert_eq!(reconstructed, result, "DCBA roundtrip should be consistent");
    }

    // ---------- Error scenario tests ----------

    #[test]
    fn test_encode_unsupported_data_type() {
        let val = RedisValue::Integer(123);
        let result = ModbusCodec::encode_value_for_modbus(&val, "unsupported_type", None);

        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Unsupported data type"));
            assert!(error_msg.contains("unsupported_type"));
        }
    }

    // ============================================================================
    // Phase 4: endianness conversion tests (critical section)
    // ============================================================================

    // ---------- Encoding direction: bytes -> registers ----------

    #[test]
    fn test_bytes_to_registers_single_register() {
        // 2 bytes → 1 register
        let bytes = [0xAB, 0xCD];
        let result = ModbusCodec::convert_bytes_to_registers_with_order(&bytes, None).unwrap();
        assert_eq!(result, vec![0xABCD]);
    }

    #[test]
    fn test_bytes_to_registers_32bit_abcd() {
        // ABCD: Big-endian (normal)
        // Bytes: [0x12, 0x34, 0x56, 0x78]
        // Registers: [0x1234, 0x5678]
        let bytes = [0x12, 0x34, 0x56, 0x78];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("ABCD")).unwrap();
        assert_eq!(result, vec![0x1234, 0x5678]);
    }

    #[test]
    fn test_bytes_to_registers_32bit_dcba() {
        // DCBA: Little-endian (completely reversed)
        // Bytes: [0x12, 0x34, 0x56, 0x78]
        // Registers: [0x7856, 0x3412]
        let bytes = [0x12, 0x34, 0x56, 0x78];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("DCBA")).unwrap();
        assert_eq!(result, vec![0x7856, 0x3412]);
    }

    #[test]
    fn test_bytes_to_registers_32bit_badc() {
        // BADC: Middle-endian (bytes swapped within words)
        // Bytes: [0x12, 0x34, 0x56, 0x78]
        // Registers: [0x3412, 0x7856]
        let bytes = [0x12, 0x34, 0x56, 0x78];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("BADC")).unwrap();
        assert_eq!(result, vec![0x3412, 0x7856]);
    }

    #[test]
    fn test_bytes_to_registers_32bit_cdab() {
        // CDAB: Word order swapped
        // Bytes: [0x12, 0x34, 0x56, 0x78]
        // Registers: [0x5678, 0x1234]
        let bytes = [0x12, 0x34, 0x56, 0x78];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("CDAB")).unwrap();
        assert_eq!(result, vec![0x5678, 0x1234]);
    }

    #[test]
    fn test_bytes_to_registers_32bit_unknown_order() {
        // Unknown order should default to ABCD
        let bytes = [0x12, 0x34, 0x56, 0x78];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("UNKNOWN")).unwrap();
        assert_eq!(result, vec![0x1234, 0x5678]); // Default to ABCD
    }

    #[test]
    fn test_bytes_to_registers_32bit_with_ab_ba_shorthand() {
        // Test AB shorthand (same as ABCD)
        let bytes = [0x12, 0x34, 0x56, 0x78];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("AB")).unwrap();
        assert_eq!(result, vec![0x1234, 0x5678]);

        // Test BA shorthand (same as DCBA)
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("BA")).unwrap();
        assert_eq!(result, vec![0x7856, 0x3412]);
    }

    #[test]
    fn test_bytes_to_registers_64bit_abcdefgh() {
        // ABCDEFGH: Big-endian
        let bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("ABCDEFGH")).unwrap();
        assert_eq!(result, vec![0x0102, 0x0304, 0x0506, 0x0708]);
    }

    #[test]
    fn test_bytes_to_registers_64bit_hgfedcba() {
        // HGFEDCBA: Little-endian
        let bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("HGFEDCBA")).unwrap();
        assert_eq!(result, vec![0x0807, 0x0605, 0x0403, 0x0201]);
    }

    #[test]
    fn test_bytes_to_registers_64bit_unknown_order() {
        // Unknown order should default to ABCDEFGH
        let bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("UNKNOWN64")).unwrap();
        assert_eq!(result, vec![0x0102, 0x0304, 0x0506, 0x0708]);
    }

    #[test]
    fn test_bytes_to_registers_64bit_with_abcd_dcba_shorthand() {
        // Test ABCD shorthand for 64-bit (same as ABCDEFGH)
        let bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("ABCD")).unwrap();
        assert_eq!(result, vec![0x0102, 0x0304, 0x0506, 0x0708]);

        // Test DCBA shorthand for 64-bit (same as HGFEDCBA)
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("DCBA")).unwrap();
        assert_eq!(result, vec![0x0807, 0x0605, 0x0403, 0x0201]);
    }

    #[test]
    fn test_bytes_to_registers_unsupported_length() {
        // 3 bytes - not 2, 4, or 8
        let bytes = [0x01, 0x02, 0x03];
        let result = ModbusCodec::convert_bytes_to_registers_with_order(&bytes, None);
        assert!(result.is_err());

        // 5 bytes
        let bytes = [0x01, 0x02, 0x03, 0x04, 0x05];
        let result = ModbusCodec::convert_bytes_to_registers_with_order(&bytes, None);
        assert!(result.is_err());

        // 6 bytes
        let bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let result = ModbusCodec::convert_bytes_to_registers_with_order(&bytes, None);
        assert!(result.is_err());
    }

    // ---------- Decoding direction: registers -> bytes ----------

    #[test]
    fn test_registers_to_bytes_single_register() {
        // 1 register → 2 bytes
        let registers = [0xABCD];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, None);
        assert_eq!(result, vec![0xAB, 0xCD]);
    }

    #[test]
    fn test_registers_to_bytes_32bit_abcd() {
        // ABCD: Big-endian
        let registers = [0x1234, 0x5678];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCD"));
        assert_eq!(result, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_registers_to_bytes_32bit_dcba() {
        // DCBA: Little-endian
        let registers = [0x1234, 0x5678];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("DCBA"));
        assert_eq!(result, vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_registers_to_bytes_32bit_badc() {
        // BADC: Middle-endian
        let registers = [0x1234, 0x5678];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("BADC"));
        assert_eq!(result, vec![0x34, 0x12, 0x78, 0x56]);
    }

    #[test]
    fn test_registers_to_bytes_32bit_cdab() {
        // CDAB: Word order swapped
        let registers = [0x1234, 0x5678];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("CDAB"));
        assert_eq!(result, vec![0x56, 0x78, 0x12, 0x34]);
    }

    #[test]
    fn test_registers_to_bytes_32bit_unknown_order() {
        // Unknown order should default to ABCD
        let registers = [0x1234, 0x5678];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("UNKNOWN"));
        assert_eq!(result, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_registers_to_bytes_64bit_abcdefgh() {
        // ABCDEFGH: Big-endian
        let registers = [0x0102, 0x0304, 0x0506, 0x0708];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCDEFGH"));
        assert_eq!(result, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_registers_to_bytes_64bit_hgfedcba() {
        // HGFEDCBA: Little-endian
        let registers = [0x0102, 0x0304, 0x0506, 0x0708];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("HGFEDCBA"));
        assert_eq!(result, vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_registers_to_bytes_64bit_unknown_order() {
        // Unknown order should default to ABCDEFGH
        let registers = [0x0102, 0x0304, 0x0506, 0x0708];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("UNKNOWN64"));
        assert_eq!(result, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_registers_to_bytes_64bit_with_abcd_dcba_shorthand() {
        // Test ABCD shorthand for 64-bit
        let registers = [0x0102, 0x0304, 0x0506, 0x0708];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCD"));
        assert_eq!(result, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);

        // Test DCBA shorthand for 64-bit
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("DCBA"));
        assert_eq!(result, vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_registers_to_bytes_default_behavior_for_other_counts() {
        // 3 registers: should just concatenate as big-endian
        let registers = [0x0102, 0x0304, 0x0506];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, None);
        assert_eq!(result, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        // 5 registers: should just concatenate
        let registers = [0x01, 0x02, 0x03, 0x04, 0x05];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, None);
        assert_eq!(result.len(), 10); // 5 * 2 = 10 bytes
    }

    // ============================================================================
    // Phase 5: end-to-end integration tests
    // ============================================================================

    #[test]
    fn test_roundtrip_uint32_abcd() {
        // Encode uint32 → registers → bytes → registers (verify identical)
        let original_value = 0x12345678i64;
        let val = RedisValue::Integer(original_value);

        // Encode to registers
        let registers = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
        assert_eq!(registers, vec![0x1234, 0x5678]);

        // Convert to bytes
        let bytes = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCD"));

        // Convert back to registers
        let reconstructed =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("ABCD")).unwrap();
        assert_eq!(reconstructed, registers, "Roundtrip should preserve data");
    }

    #[test]
    fn test_roundtrip_uint32_dcba() {
        let original_value = 0xABCDEF01i64;
        let val = RedisValue::Integer(original_value);

        // Encode with DCBA
        let registers = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("DCBA")).unwrap();

        // Convert to bytes
        let bytes = ModbusCodec::convert_registers_with_byte_order(&registers, Some("DCBA"));

        // Convert back
        let reconstructed =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("DCBA")).unwrap();
        assert_eq!(reconstructed, registers);
    }

    #[test]
    fn test_roundtrip_float32_all_byte_orders() {
        let orders = vec!["ABCD", "DCBA", "BADC", "CDAB"];

        for order in orders {
            let val = RedisValue::Float(123.456);

            // Encode
            let registers =
                ModbusCodec::encode_value_for_modbus(&val, "float32", Some(order)).unwrap();

            // To bytes
            let bytes = ModbusCodec::convert_registers_with_byte_order(&registers, Some(order));

            // Back to registers
            let reconstructed =
                ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some(order)).unwrap();

            assert_eq!(
                reconstructed, registers,
                "Roundtrip failed for order {}",
                order
            );
        }
    }

    #[test]
    fn test_roundtrip_int32_negative_values() {
        let val = RedisValue::Integer(-12345);

        // Encode
        let registers = ModbusCodec::encode_value_for_modbus(&val, "int32", Some("ABCD")).unwrap();

        // To bytes
        let bytes = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCD"));

        // Verify we can reconstruct the value
        let bytes_array: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
        let reconstructed = i32::from_be_bytes(bytes_array);
        assert_eq!(reconstructed, -12345);
    }

    #[test]
    fn test_roundtrip_float64_precision() {
        use std::borrow::Cow;

        let val = RedisValue::String(Cow::Borrowed("3.14159265358979"));

        // Encode
        let registers =
            ModbusCodec::encode_value_for_modbus(&val, "float64", Some("ABCD")).unwrap();
        assert_eq!(registers.len(), 4);

        // To bytes
        let bytes = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCD"));

        // Back to registers
        let reconstructed =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("ABCD")).unwrap();
        assert_eq!(reconstructed, registers);

        // Verify precision
        let bytes_array: [u8; 8] = [
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ];
        let value = f64::from_be_bytes(bytes_array);
        assert!((value - std::f64::consts::PI).abs() < 1e-14);
    }

    #[test]
    fn test_encode_decode_consistency_all_types() {
        // Test that all data types can be encoded and maintain their byte representation

        // uint16
        let val = RedisValue::Integer(0x1234);
        let regs = ModbusCodec::encode_value_for_modbus(&val, "uint16", None).unwrap();
        assert_eq!(regs, vec![0x1234]);

        // uint32 ABCD
        let val = RedisValue::Integer(0x12345678);
        let regs = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
        let bytes = ModbusCodec::convert_registers_with_byte_order(&regs, Some("ABCD"));
        assert_eq!(bytes, vec![0x12, 0x34, 0x56, 0x78]);

        // float32
        let val = RedisValue::Float(25.5);
        let regs = ModbusCodec::encode_value_for_modbus(&val, "float32", Some("ABCD")).unwrap();
        let bytes = ModbusCodec::convert_registers_with_byte_order(&regs, Some("ABCD"));
        let reconstructed = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert!((reconstructed - 25.5).abs() < 0.001);
    }

    #[test]
    fn test_pdu_write_and_encode_integration() {
        // Test building a write PDU with encoded values

        // FC06: Write single register with uint16
        let val = RedisValue::Integer(0xABCD);
        let encoded = ModbusCodec::encode_value_for_modbus(&val, "uint16", None).unwrap();
        assert_eq!(encoded.len(), 1);

        let pdu = ModbusCodec::build_write_fc06_single_register_pdu(0x0100, encoded[0]).unwrap();
        assert_eq!(pdu.as_slice(), &[0x06, 0x01, 0x00, 0xAB, 0xCD]);

        // FC16: Write multiple registers with uint32
        let val = RedisValue::Integer(0x12345678);
        let encoded = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
        assert_eq!(encoded.len(), 2);

        let pdu = ModbusCodec::build_write_fc16_multiple_registers_pdu(0x0200, &encoded).unwrap();
        assert_eq!(pdu.as_slice()[0], 0x10); // FC16
        assert_eq!(pdu.as_slice()[6..10], [0x12, 0x34, 0x56, 0x78]); // Data
    }

    #[test]
    fn test_byte_order_symmetry() {
        // Verify that encode and decode are symmetric for all byte orders
        let test_data = [0xAA, 0xBB, 0xCC, 0xDD];
        let orders = vec!["ABCD", "DCBA", "BADC", "CDAB"];

        for order in orders {
            // bytes → registers
            let registers =
                ModbusCodec::convert_bytes_to_registers_with_order(&test_data, Some(order))
                    .unwrap();

            // registers → bytes
            let reconstructed_bytes =
                ModbusCodec::convert_registers_with_byte_order(&registers, Some(order));

            assert_eq!(
                reconstructed_bytes,
                test_data.to_vec(),
                "Byte order {} is not symmetric",
                order
            );
        }
    }

    #[test]
    fn test_complex_data_flow_simulation() {
        // Simulate a complete data flow:
        // Redis value → encode → registers → PDU → response → decode

        // Step 1: Start with a temperature value from Redis
        let temperature = RedisValue::Float(23.5); // 23.5°C

        // Step 2: Encode for Modbus (float32, ABCD)
        let registers =
            ModbusCodec::encode_value_for_modbus(&temperature, "float32", Some("ABCD")).unwrap();
        assert_eq!(registers.len(), 2);

        // Step 3: Build write PDU
        let pdu = ModbusCodec::build_write_fc16_multiple_registers_pdu(0x1000, &registers).unwrap();
        assert_eq!(pdu.function_code(), Some(0x10));

        // Step 4: Simulate successful response
        let mut response_pdu = ModbusPdu::new();
        response_pdu.push(0x10).unwrap(); // FC16
        response_pdu.push_u16(0x1000).unwrap(); // Address
        response_pdu.push_u16(0x0002).unwrap(); // Quantity = 2 registers

        let result = ModbusCodec::parse_modbus_write_response(&response_pdu, 0x10);
        assert!(result.is_ok());

        // Step 5: Decode back to verify
        let bytes = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCD"));
        let decoded_temp = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert!((decoded_temp - 23.5).abs() < 0.001);
    }

    #[test]
    fn test_edge_cases_all_zeros_all_ones() {
        // Test with all zeros
        let val = RedisValue::Integer(0);
        let regs = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
        assert_eq!(regs, vec![0x0000, 0x0000]);

        // Test with all ones (maximum values)
        let val = RedisValue::Integer(0xFFFFFFFF);
        let regs = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
        assert_eq!(regs, vec![0xFFFF, 0xFFFF]);

        // Test roundtrip for both
        for &test_val in &[0i64, 0xFFFFFFFF] {
            let val = RedisValue::Integer(test_val);
            let regs = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
            let bytes = ModbusCodec::convert_registers_with_byte_order(&regs, Some("ABCD"));
            let reconstructed =
                ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("ABCD")).unwrap();
            assert_eq!(reconstructed, regs);
        }
    }
}
