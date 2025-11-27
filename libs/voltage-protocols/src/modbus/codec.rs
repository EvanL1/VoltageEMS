//! Modbus codec implementation
//!
//! Handles encoding and decoding of Modbus data types and values

use super::constants;
use tracing::{trace, warn};
use voltage_comlink::bytes::{regs_to_bytes_4, regs_to_bytes_8, ByteOrder};
use voltage_comlink::error::{ComLinkError, Result};
use voltage_comlink::ProtocolValue;
use voltage_modbus::{ModbusPdu, PduBuilder};

/// Modbus codec for data encoding/decoding
pub struct ModbusCodec;

// ============================================================================
// Standalone decoding functions (migrated from comsrv/protocols/modbus/protocol.rs)
// ============================================================================

/// Clamp a value to the valid range for a given Modbus data type.
///
/// Prevents overflow when writing values that exceed the target register's
/// capacity (e.g., writing 70000 to a uint16 register).
///
/// # Arguments
/// * `value` - The value to clamp
/// * `data_type` - Target data type (e.g., "uint16", "int32", "float32")
///
/// # Returns
/// The clamped value, or the original value if the type is unknown/boolean
pub fn clamp_to_data_type(value: f64, data_type: &str) -> f64 {
    let (min, max): (f64, f64) = match data_type.to_lowercase().as_str() {
        "uint16" | "u16" => (0.0, 65535.0),
        "int16" | "i16" => (-32768.0, 32767.0),
        "uint32" | "u32" => (0.0, 4294967295.0),
        "int32" | "i32" => (-2147483648.0, 2147483647.0),
        "uint64" | "u64" => (0.0, u64::MAX as f64),
        "int64" | "i64" => (i64::MIN as f64, i64::MAX as f64),
        "float32" | "f32" => (f32::MIN as f64, f32::MAX as f64),
        "float64" | "f64" => (f64::MIN, f64::MAX),
        // Boolean types don't need range clamping
        "bool" | "boolean" | "coil" => return value,
        // Unknown type - return as-is
        _ => return value,
    };

    value.clamp(min, max)
}

/// Parse a Modbus response PDU and extract register data.
///
/// This function implements graceful degradation - it will attempt to parse
/// as much valid data as possible even when the response is incomplete or
/// has byte count mismatches.
///
/// # Arguments
/// * `pdu` - The Modbus PDU to parse
/// * `function_code` - Expected function code (1, 2, 3, or 4)
/// * `expected_count` - Expected number of coils (FC01/02) or registers (FC03/04)
///
/// # Returns
/// - For FC01/02: Vec of bytes (each stored as u16 for uniform processing)
/// - For FC03/04: Vec of 16-bit register values
///
/// # Graceful degradation strategy:
/// - Parse partial data when available instead of failing completely
/// - Log warnings for incomplete/mismatched data
/// - Return as many valid registers as possible
pub fn parse_modbus_pdu(
    pdu: &ModbusPdu,
    function_code: u8,
    expected_count: u16,
) -> Result<Vec<u16>> {
    let pdu_data = pdu.as_slice();

    // Minimum viable PDU check (allow partial data)
    if pdu_data.len() < 2 {
        warn!(
            "PDU too short ({} bytes), cannot extract byte_count field",
            pdu_data.len()
        );
        return Ok(Vec::new()); // Return empty instead of failing
    }

    let actual_fc = pdu.function_code().unwrap_or(0);
    if actual_fc != function_code {
        return Err(ComLinkError::Protocol(format!(
            "Function code mismatch: expected {}, got {}",
            function_code, actual_fc
        )));
    }

    let byte_count = pdu_data[1] as usize;
    let available_bytes = pdu_data.len().saturating_sub(2); // Actual data bytes available

    // Use the smaller of declared byte_count or available bytes
    let actual_byte_count = byte_count.min(available_bytes);

    if byte_count > available_bytes {
        warn!(
            "Incomplete PDU data: declared {} bytes, only {} available - parsing partial data",
            byte_count, available_bytes
        );
    }

    // Parse based on function code with graceful degradation
    match function_code {
        1 | 2 => {
            // FC 01/02: byte_count should be ceil(coil_count / 8)
            let expected_bytes = expected_count.div_ceil(8) as usize;
            if byte_count != expected_bytes {
                warn!(
                    "Byte count mismatch for FC{:02}: expected {} bytes for {} coils, got {} - parsing available data",
                    function_code, expected_bytes, expected_count, byte_count
                );
            }

            // Return bytes as-is (each byte stored in a u16 for uniform processing)
            let mut registers = Vec::new();
            for &byte in &pdu_data[2..2 + actual_byte_count] {
                registers.push(u16::from(byte));
            }
            Ok(registers)
        },
        3 | 4 => {
            // FC 03/04: byte_count should be register_count * 2
            let expected_bytes = (expected_count * 2) as usize;
            if byte_count != expected_bytes {
                warn!(
                    "Byte count mismatch for FC{:02}: expected {} bytes for {} registers, got {} - parsing available data",
                    function_code, expected_bytes, expected_count, byte_count
                );
            }

            // Parse 16-bit registers from available complete register pairs
            let mut registers = Vec::new();
            let complete_pairs = actual_byte_count / 2; // Only parse complete 16-bit pairs

            for i in 0..complete_pairs {
                let offset = 2 + i * 2;
                if offset + 1 < pdu_data.len() {
                    let value =
                        (u16::from(pdu_data[offset]) << 8) | u16::from(pdu_data[offset + 1]);
                    registers.push(value);
                }
            }

            if !actual_byte_count.is_multiple_of(2) {
                warn!(
                    "Odd byte count ({}) - last incomplete byte ignored",
                    actual_byte_count
                );
            }

            Ok(registers)
        },
        _ => Err(ComLinkError::Protocol(format!(
            "Unsupported function code in PDU parsing: {function_code}"
        ))),
    }
}

/// Decode Modbus register values to ProtocolValue based on data format.
///
/// Supports multiple data types with configurable byte ordering:
/// - `bool`: Single bit extraction from register (0-15 bit position)
/// - `uint16`, `int16`: Single 16-bit register
/// - `uint32`, `int32`, `float32`: Two 16-bit registers
/// - `uint64`, `int64`, `float64`: Four 16-bit registers
///
/// # Arguments
/// * `registers` - Raw register values from Modbus response
/// * `format` - Data type string (e.g., "uint16", "float32", "bool")
/// * `bit_position` - For bool type: which bit to extract (0-15, LSB=0)
/// * `byte_order` - Optional byte ordering (e.g., "ABCD", "DCBA", "BADC", "CDAB")
/// * `function_code` - Optional FC for context (1/2 = coils, 3/4 = registers)
///
/// # Returns
/// `Ok(ProtocolValue)` on success, `Err` with description on failure
pub fn decode_register_value(
    registers: &[u16],
    format: &str,
    bit_position: u8,
    byte_order: Option<&str>,
    function_code: Option<u8>,
) -> Result<ProtocolValue> {
    match format {
        "bool" => {
            if registers.is_empty() {
                return Err(ComLinkError::Protocol("No registers for bool".to_string()));
            }

            // Use 0-based bit numbering (programmer-friendly)
            let bit_pos = bit_position;

            // Determine if this is from coils/discrete inputs (FC 01/02) or registers (FC 03/04)
            let is_coil_response = matches!(function_code, Some(1) | Some(2));

            // Validate bit position - unified range for all types (0-15)
            if bit_pos > 15 {
                return Err(ComLinkError::Protocol(format!(
                    "Invalid bit position: {} (must be 0-15)",
                    bit_pos
                )));
            }

            // Unified bit extraction for both coils and registers (0-15)
            let value = registers[0];
            let bit_value = (value >> bit_pos) & 0x01;

            if is_coil_response {
                trace!(
                    "Coil bit extraction: value=0x{:04X}, bit_pos={}, bit_value={}",
                    value,
                    bit_pos,
                    bit_value
                );
            } else {
                trace!(
                    "Register bit extraction: value=0x{:04X}, bit_pos={}, bit_value={}",
                    value,
                    bit_pos,
                    bit_value
                );
            }

            Ok(ProtocolValue::Integer(i64::from(bit_value)))
        },
        "uint16" => {
            if registers.is_empty() {
                return Err(ComLinkError::Protocol(
                    "No registers for uint16".to_string(),
                ));
            }
            let value = i64::from(registers[0]);
            trace!(
                "Decoded uint16: register=0x{:04X}, value={}",
                registers[0],
                value
            );
            Ok(ProtocolValue::Integer(value))
        },
        "int16" => {
            if registers.is_empty() {
                return Err(ComLinkError::Protocol("No registers for int16".to_string()));
            }
            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            let value = if bytes.len() >= 2 {
                let v = i16::from_be_bytes([bytes[0], bytes[1]]);
                trace!(
                    "Decoded int16: register=0x{:04X}, byte_order={:?}, bytes={:02X?}, value={}",
                    registers[0],
                    byte_order,
                    &bytes[0..2],
                    v
                );
                i64::from(v)
            } else {
                let v = registers[0] as i16;
                trace!(
                    "Decoded int16: register=0x{:04X}, value={}",
                    registers[0],
                    v
                );
                i64::from(v)
            };
            Ok(ProtocolValue::Integer(value))
        },
        "uint32" | "uint32_be" => {
            if registers.len() < 2 {
                return Err(ComLinkError::Protocol(
                    "Not enough registers for uint32".to_string(),
                ));
            }
            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            let value = if bytes.len() >= 4 {
                let v = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                trace!(
                    "Decoded uint32: registers=[0x{:04X}, 0x{:04X}], byte_order={:?}, bytes={:02X?}, value={}",
                    registers[0], registers[1], byte_order, &bytes[0..4], v
                );
                i64::from(v)
            } else {
                // Fallback to old method if bytes conversion fails
                let v = (u32::from(registers[0]) << 16) | u32::from(registers[1]);
                trace!(
                    "Decoded uint32 (fallback): registers=[0x{:04X}, 0x{:04X}], value={}",
                    registers[0],
                    registers[1],
                    v
                );
                i64::from(v)
            };
            Ok(ProtocolValue::Integer(value))
        },
        "int32" | "int32_be" => {
            if registers.len() < 2 {
                return Err(ComLinkError::Protocol(
                    "Not enough registers for int32".to_string(),
                ));
            }
            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            let value = if bytes.len() >= 4 {
                let v = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                trace!(
                    "Decoded int32: registers=[0x{:04X}, 0x{:04X}], byte_order={:?}, bytes={:02X?}, value={}",
                    registers[0], registers[1], byte_order, &bytes[0..4], v
                );
                i64::from(v)
            } else {
                // Fallback to old method if bytes conversion fails
                let v = (i32::from(registers[0]) << 16) | i32::from(registers[1]);
                trace!(
                    "Decoded int32 (fallback): registers=[0x{:04X}, 0x{:04X}], value={}",
                    registers[0],
                    registers[1],
                    v
                );
                i64::from(v)
            };
            Ok(ProtocolValue::Integer(value))
        },
        "float32" | "float32_be" | "float" => {
            if registers.len() < 2 {
                return Err(ComLinkError::Protocol(
                    "Not enough registers for float32".to_string(),
                ));
            }

            // Special handling for DCBA - the simulator stores bytes in little-endian order directly
            let (bytes, value) = if byte_order == Some("DCBA") {
                // For DCBA, extract bytes directly from registers (they're already in little-endian order)
                let mut bytes = Vec::new();
                for &reg in &registers[0..2] {
                    bytes.push((reg >> 8) as u8); // High byte of register
                    bytes.push((reg & 0xFF) as u8); // Low byte of register
                }
                // Bytes are already in little-endian order, decode with from_le_bytes
                let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                (bytes, value)
            } else {
                // For other byte orders, use the standard conversion
                let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
                if bytes.len() >= 4 {
                    let value = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    (bytes[0..4].to_vec(), value)
                } else {
                    // Fallback to direct conversion if not enough bytes
                    let bytes = vec![
                        (registers[0] >> 8) as u8,
                        (registers[0] & 0xFF) as u8,
                        (registers[1] >> 8) as u8,
                        (registers[1] & 0xFF) as u8,
                    ];
                    let value = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    (bytes, value)
                }
            };

            trace!(
                "Float32 conversion: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                registers,
                byte_order,
                &bytes[0..4],
                value
            );
            Ok(ProtocolValue::Float(f64::from(value)))
        },
        "float64" | "float64_be" | "double" => {
            if registers.len() < 4 {
                return Err(ComLinkError::Protocol(
                    "Not enough registers for float64".to_string(),
                ));
            }

            // Special handling for DCBA - the simulator stores bytes in little-endian order directly
            let (bytes, value) = if byte_order == Some("DCBA") {
                // For DCBA, extract bytes directly from registers (they're already in little-endian order)
                let mut bytes = Vec::new();
                for &reg in &registers[0..4] {
                    bytes.push((reg >> 8) as u8); // High byte of register
                    bytes.push((reg & 0xFF) as u8); // Low byte of register
                }
                // Bytes are already in little-endian order, decode with from_le_bytes
                let value = f64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                (bytes, value)
            } else {
                // For other byte orders, use the standard conversion
                let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
                let value = f64::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                (bytes, value)
            };

            trace!(
                "Float64 conversion: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                registers,
                byte_order,
                &bytes[0..8],
                value
            );
            Ok(ProtocolValue::Float(value))
        },
        "uint64" | "uint64_be" | "u64" | "qword" => {
            if registers.len() < 4 {
                return Err(ComLinkError::Protocol(
                    "Not enough registers for uint64".to_string(),
                ));
            }

            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            if bytes.len() < 8 {
                return Err(ComLinkError::Protocol(
                    "Not enough bytes for uint64".to_string(),
                ));
            }

            let value = u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);

            trace!(
                "Decoded uint64: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                &registers[0..4],
                byte_order,
                &bytes[0..8],
                value
            );

            // Redis stores integers as i64, so u64 values must be converted
            // Values greater than i64::MAX will be truncated
            Ok(ProtocolValue::Integer(value as i64))
        },
        "int64" | "int64_be" | "i64" | "longlong" => {
            if registers.len() < 4 {
                return Err(ComLinkError::Protocol(
                    "Not enough registers for int64".to_string(),
                ));
            }

            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            if bytes.len() < 8 {
                return Err(ComLinkError::Protocol(
                    "Not enough bytes for int64".to_string(),
                ));
            }

            let value = i64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);

            trace!(
                "Decoded int64: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                &registers[0..4],
                byte_order,
                &bytes[0..8],
                value
            );

            Ok(ProtocolValue::Integer(value))
        },
        _ => Err(ComLinkError::Protocol(format!(
            "Unsupported data format: {format}"
        ))),
    }
}

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
            return Err(ComLinkError::Protocol(
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
            return Err(ComLinkError::Protocol(
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
            return Err(ComLinkError::Protocol("Empty response PDU".to_string()));
        }

        // Check for exception response
        if data[0] & 0x80 != 0 {
            let exception_code = if data.len() > 1 { data[1] } else { 0 };
            return Err(ComLinkError::Protocol(format!(
                "Modbus exception response: code {:02X}",
                exception_code
            )));
        }

        // Verify function code
        if data[0] != expected_fc {
            return Err(ComLinkError::Protocol(format!(
                "Function code mismatch: expected {:02X}, got {:02X}",
                expected_fc, data[0]
            )));
        }

        // For write operations, a matching function code indicates success
        Ok(true)
    }

    /// Encode value for Modbus transmission
    pub fn encode_value_for_modbus(
        value: &ProtocolValue,
        data_type: &str,
        byte_order: Option<&str>,
    ) -> Result<Vec<u16>> {
        match data_type {
            "bool" | "boolean" => {
                let bool_val = match value {
                    ProtocolValue::Integer(i) => *i != 0,
                    ProtocolValue::Float(f) => *f != 0.0,
                    ProtocolValue::String(s) => {
                        s.to_lowercase() == "true" || s == "1" || s.to_lowercase() == "on"
                    },
                    _ => false,
                };
                Ok(vec![if bool_val { 1 } else { 0 }])
            },
            "uint16" | "u16" | "word" => {
                let val = match value {
                    ProtocolValue::Integer(i) => *i as u16,
                    ProtocolValue::Float(f) => f.round() as u16,
                    ProtocolValue::String(s) => s.parse::<u16>().unwrap_or(0),
                    _ => 0,
                };
                Ok(vec![val])
            },
            "int16" | "i16" | "short" => {
                let val = match value {
                    ProtocolValue::Integer(i) => *i as i16,
                    ProtocolValue::Float(f) => f.round() as i16,
                    ProtocolValue::String(s) => s.parse::<i16>().unwrap_or(0),
                    _ => 0,
                };
                Ok(vec![val as u16])
            },
            "uint32" | "u32" | "dword" => {
                let val = match value {
                    ProtocolValue::Integer(i) => *i as u32,
                    ProtocolValue::Float(f) => f.round() as u32,
                    ProtocolValue::String(s) => s.parse::<u32>().unwrap_or(0),
                    _ => 0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "int32" | "i32" | "long" => {
                let val = match value {
                    ProtocolValue::Integer(i) => *i as i32,
                    ProtocolValue::Float(f) => f.round() as i32,
                    ProtocolValue::String(s) => s.parse::<i32>().unwrap_or(0),
                    _ => 0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "float32" | "f32" | "float" | "real" => {
                let val = match value {
                    ProtocolValue::Float(f) => *f as f32,
                    ProtocolValue::Integer(i) => *i as f32,
                    ProtocolValue::String(s) => s.parse::<f32>().unwrap_or(0.0),
                    _ => 0.0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "float64" | "f64" | "double" | "lreal" => {
                let val = match value {
                    ProtocolValue::Float(f) => *f,
                    ProtocolValue::Integer(i) => *i as f64,
                    ProtocolValue::String(s) => s.parse::<f64>().unwrap_or(0.0),
                    _ => 0.0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "uint64" | "u64" | "qword" => {
                let val = match value {
                    ProtocolValue::Integer(i) => *i as u64,
                    ProtocolValue::Float(f) => f.round() as u64,
                    ProtocolValue::String(s) => s.parse::<u64>().unwrap_or(0),
                    _ => 0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            "int64" | "i64" | "longlong" => {
                let val = match value {
                    ProtocolValue::Integer(i) => *i,
                    ProtocolValue::Float(f) => f.round() as i64,
                    ProtocolValue::String(s) => s.parse::<i64>().unwrap_or(0),
                    _ => 0,
                };
                let bytes = val.to_be_bytes();
                Self::convert_bytes_to_registers_with_order(&bytes, byte_order)
            },
            _ => Err(ComLinkError::Config(format!(
                "Unsupported data type for encoding: {}",
                data_type
            ))),
        }
    }

    /// Convert bytes to registers using ByteOrder enum (type-safe)
    ///
    /// Inverse operation of `registers_to_bytes_typed`. Converts byte array
    /// back to register array with specified byte ordering.
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
            _ => Err(ComLinkError::Protocol(format!(
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
    fn registers_to_bytes_typed(registers: &[u16], order: ByteOrder) -> Result<Vec<u8>> {
        match registers.len() {
            1 => {
                // Single register (16-bit) - no byte order applies
                Ok(registers[0].to_be_bytes().to_vec())
            },
            2 => {
                // Two registers (32-bit) - use voltage-comlink bytes
                let regs: [u16; 2] = [registers[0], registers[1]];
                Ok(regs_to_bytes_4(&regs, order).to_vec())
            },
            4 => {
                // Four registers (64-bit) - use voltage-comlink bytes
                let regs: [u16; 4] = [registers[0], registers[1], registers[2], registers[3]];
                Ok(regs_to_bytes_8(&regs, order).to_vec())
            },
            _ => Err(ComLinkError::Protocol(format!(
                "Unsupported register count for conversion: {} (must be 1, 2, or 4)",
                registers.len()
            ))),
        }
    }

    /// Convert registers to bytes with specified byte order (legacy string interface)
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

    #[test]
    fn test_build_fc05_write_true() {
        let pdu = ModbusCodec::build_write_fc05_single_coil_pdu(0x0100, true).unwrap();
        assert_eq!(pdu.as_slice(), &[0x05, 0x01, 0x00, 0xFF, 0x00]);
        assert_eq!(pdu.function_code(), Some(0x05));
    }

    #[test]
    fn test_build_fc05_write_false() {
        let pdu = ModbusCodec::build_write_fc05_single_coil_pdu(0x0200, false).unwrap();
        assert_eq!(pdu.as_slice(), &[0x05, 0x02, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_build_fc06_zero_value() {
        let pdu = ModbusCodec::build_write_fc06_single_register_pdu(0x0100, 0x0000).unwrap();
        assert_eq!(pdu.as_slice(), &[0x06, 0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_build_fc06_typical_value() {
        let pdu = ModbusCodec::build_write_fc06_single_register_pdu(0x0300, 0x1234).unwrap();
        assert_eq!(pdu.as_slice(), &[0x06, 0x03, 0x00, 0x12, 0x34]);
    }

    #[test]
    fn test_build_fc15_empty_array_error() {
        let result = ModbusCodec::build_write_fc15_multiple_coils_pdu(0x0100, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_fc15_single_coil() {
        let pdu = ModbusCodec::build_write_fc15_multiple_coils_pdu(0x0100, &[true]).unwrap();
        assert_eq!(pdu.as_slice(), &[0x0F, 0x01, 0x00, 0x00, 0x01, 0x01, 0x01]);
    }

    #[test]
    fn test_build_fc16_empty_array_error() {
        let result = ModbusCodec::build_write_fc16_multiple_registers_pdu(0x0100, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_fc16_single_register() {
        let pdu = ModbusCodec::build_write_fc16_multiple_registers_pdu(0x0100, &[0x1234]).unwrap();
        assert_eq!(
            pdu.as_slice(),
            &[0x10, 0x01, 0x00, 0x00, 0x01, 0x02, 0x12, 0x34]
        );
    }

    // ============================================================================
    // Phase 2: response parsing tests
    // ============================================================================

    #[test]
    fn test_parse_write_response_success() {
        let mut pdu = ModbusPdu::new();
        pdu.push(0x06).unwrap();
        pdu.push_u16(0x0100).unwrap();
        pdu.push_u16(0x1234).unwrap();

        let result = ModbusCodec::parse_modbus_write_response(&pdu, 0x06);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_parse_write_response_exception() {
        let mut pdu = ModbusPdu::new();
        pdu.push(0x86).unwrap();
        pdu.push(0x02).unwrap();

        let result = ModbusCodec::parse_modbus_write_response(&pdu, 0x06);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_write_response_empty_pdu() {
        let pdu = ModbusPdu::new();
        let result = ModbusCodec::parse_modbus_write_response(&pdu, 0x06);
        assert!(result.is_err());
    }

    // ============================================================================
    // Phase 3: data encoding tests
    // ============================================================================

    #[test]
    fn test_encode_bool_from_integer() {
        let val = ProtocolValue::Integer(0);
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![0]);

        let val = ProtocolValue::Integer(1);
        let result = ModbusCodec::encode_value_for_modbus(&val, "bool", None).unwrap();
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_encode_uint16_boundary_values() {
        let val = ProtocolValue::Integer(0);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint16", None).unwrap();
        assert_eq!(result, vec![0]);

        let val = ProtocolValue::Integer(65535);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint16", None).unwrap();
        assert_eq!(result, vec![65535]);
    }

    #[test]
    fn test_encode_uint32_with_byte_order_abcd() {
        let val = ProtocolValue::Integer(0x12345678);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
        assert_eq!(result, vec![0x1234, 0x5678]);
    }

    #[test]
    fn test_encode_uint32_with_byte_order_dcba() {
        let val = ProtocolValue::Integer(0x12345678);
        let result = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("DCBA")).unwrap();
        assert_eq!(result, vec![0x7856, 0x3412]);
    }

    #[test]
    fn test_encode_float32_typical_values() {
        let val = ProtocolValue::Float(123.456);
        let result = ModbusCodec::encode_value_for_modbus(&val, "float32", Some("ABCD")).unwrap();

        let bytes = ModbusCodec::convert_registers_with_byte_order(&result, Some("ABCD"));
        let reconstructed = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert!((reconstructed - 123.456).abs() < 0.001);
    }

    #[test]
    fn test_encode_unsupported_data_type() {
        let val = ProtocolValue::Integer(123);
        let result = ModbusCodec::encode_value_for_modbus(&val, "unsupported_type", None);
        assert!(result.is_err());
    }

    // ============================================================================
    // Phase 4: endianness conversion tests
    // ============================================================================

    #[test]
    fn test_bytes_to_registers_single_register() {
        let bytes = [0xAB, 0xCD];
        let result = ModbusCodec::convert_bytes_to_registers_with_order(&bytes, None).unwrap();
        assert_eq!(result, vec![0xABCD]);
    }

    #[test]
    fn test_bytes_to_registers_32bit_abcd() {
        let bytes = [0x12, 0x34, 0x56, 0x78];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("ABCD")).unwrap();
        assert_eq!(result, vec![0x1234, 0x5678]);
    }

    #[test]
    fn test_bytes_to_registers_32bit_dcba() {
        let bytes = [0x12, 0x34, 0x56, 0x78];
        let result =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("DCBA")).unwrap();
        assert_eq!(result, vec![0x7856, 0x3412]);
    }

    #[test]
    fn test_registers_to_bytes_single_register() {
        let registers = [0xABCD];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, None);
        assert_eq!(result, vec![0xAB, 0xCD]);
    }

    #[test]
    fn test_registers_to_bytes_32bit_abcd() {
        let registers = [0x1234, 0x5678];
        let result = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCD"));
        assert_eq!(result, vec![0x12, 0x34, 0x56, 0x78]);
    }

    // ============================================================================
    // Phase 5: roundtrip tests
    // ============================================================================

    #[test]
    fn test_roundtrip_uint32_abcd() {
        let original_value = 0x12345678i64;
        let val = ProtocolValue::Integer(original_value);

        let registers = ModbusCodec::encode_value_for_modbus(&val, "uint32", Some("ABCD")).unwrap();
        assert_eq!(registers, vec![0x1234, 0x5678]);

        let bytes = ModbusCodec::convert_registers_with_byte_order(&registers, Some("ABCD"));
        let reconstructed =
            ModbusCodec::convert_bytes_to_registers_with_order(&bytes, Some("ABCD")).unwrap();
        assert_eq!(reconstructed, registers);
    }

    #[test]
    fn test_byte_order_symmetry() {
        let test_data = [0xAA, 0xBB, 0xCC, 0xDD];
        let orders = vec!["ABCD", "DCBA", "BADC", "CDAB"];

        for order in orders {
            let registers =
                ModbusCodec::convert_bytes_to_registers_with_order(&test_data, Some(order))
                    .unwrap();
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

    // ============================================================================
    // Phase 6: decode_register_value tests (migrated from comsrv/protocol.rs)
    // ============================================================================

    #[test]
    fn test_decode_register_value_bool_bitwise() {
        // Testing bit extraction with 0-based numbering (programmer-friendly)

        // Test case 1: Register value 0xB5 = 181 = 10110101 in binary
        let register_value = 0xB5;
        let registers = vec![register_value];

        // For FC 03/04 (registers), use 0-15 bit numbering
        // Bit 0 (LSB) = 1
        let result = decode_register_value(&registers, "bool", 0, None, Some(3))
            .expect("decoding bit 0 should succeed");
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 1 = 0
        let result = decode_register_value(&registers, "bool", 1, None, Some(3))
            .expect("decoding bit 1 should succeed");
        assert_eq!(result, ProtocolValue::Integer(0));

        // Bit 2 = 1
        let result = decode_register_value(&registers, "bool", 2, None, Some(3))
            .expect("decoding bit 2 should succeed");
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 7 = 1
        let result = decode_register_value(&registers, "bool", 7, None, Some(3))
            .expect("decoding bit 7 should succeed");
        assert_eq!(result, ProtocolValue::Integer(1));

        // Test that full 16-bit range (0-15) is valid for registers
        let high_bit_register = 0x8000; // Bit 15 (MSB) set
        let high_registers = vec![high_bit_register];
        let result = decode_register_value(&high_registers, "bool", 15, None, Some(3))
            .expect("decoding bit 15 should succeed");
        assert_eq!(result, ProtocolValue::Integer(1));

        // Test FC 01/02 (coils) - uses 0-15 bit numbering
        let coil_byte = 0xB5; // Same value but treated as byte
        let coil_registers = vec![coil_byte];

        // Bit 0 (LSB) = 1
        let result = decode_register_value(&coil_registers, "bool", 0, None, Some(1))
            .expect("decoding coil bit 0 should succeed");
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 7 (MSB of low byte) = 1
        let result = decode_register_value(&coil_registers, "bool", 7, None, Some(1))
            .expect("decoding coil bit 7 should succeed");
        assert_eq!(result, ProtocolValue::Integer(1));
    }

    #[test]
    fn test_decode_register_value_bool_edge_cases() {
        let registers = vec![0x0000]; // All-zero register

        // Testing FC 01/02 (coils) - unified 0-15 bit numbering
        for bit_pos in 0..=15 {
            let result = decode_register_value(&registers, "bool", bit_pos, None, Some(1));
            if let Ok(value) = result {
                assert_eq!(
                    value,
                    ProtocolValue::Integer(0),
                    "Bit {} should be 0",
                    bit_pos
                );
            } else {
                panic!("Failed to decode bit {}", bit_pos);
            }
        }

        // Testing FC 03/04 (registers) - 0-15 bit numbering
        let registers_16bit = vec![0x0100]; // 0x0100 in binary: 0000 0001 0000 0000, only bit 8 is set
        for bit_pos in 0..=15 {
            let result = decode_register_value(&registers_16bit, "bool", bit_pos, None, Some(3));
            let expected = if bit_pos == 8 { 1 } else { 0 }; // Only bit 8 is set
            if let Ok(value) = result {
                assert_eq!(
                    value,
                    ProtocolValue::Integer(expected),
                    "Bit {} should be {}",
                    bit_pos,
                    expected
                );
            } else {
                panic!("Failed to decode bit {}", bit_pos);
            }
        }

        let registers_all_ones = vec![0xFFFF]; // All 1s register
        for bit_pos in 0..=15 {
            let result = decode_register_value(&registers_all_ones, "bool", bit_pos, None, Some(3));
            if let Ok(value) = result {
                assert_eq!(
                    value,
                    ProtocolValue::Integer(1),
                    "Bit {} should be 1",
                    bit_pos
                );
            } else {
                panic!("Failed to decode bit {}", bit_pos);
            }
        }

        // Testing error case: Bit 0 should be valid for registers (FC 03)
        let result = decode_register_value(&registers, "bool", 0, None, Some(3));
        assert!(
            result.is_ok(),
            "Bit position 0 should be valid for registers"
        );

        // Testing error case: bit position out of range for 16-bit mode
        let registers_16bit = vec![0x0100];
        let result = decode_register_value(&registers_16bit, "bool", 16, None, Some(3));
        assert!(
            result.is_err(),
            "Bit position 16 should be invalid (must be 0-15)"
        );

        // Testing error case: empty registers
        let empty_registers = vec![];
        let result = decode_register_value(&empty_registers, "bool", 0, None, Some(3));
        assert!(result.is_err());

        // Testing default bit_position (should be 0 - LSB)
        let registers = vec![0x0001]; // Only bit 0 (LSB) is set
        let result = decode_register_value(&registers, "bool", 0, None, Some(3))
            .expect("decoding bool with default bit position should succeed");
        assert_eq!(result, ProtocolValue::Integer(1)); // Default bit 0 = 1
    }

    #[test]
    fn test_decode_register_value_other_formats() {
        // Ensure other data formats still work normally
        let registers = vec![0x1234];

        // Testing uint16
        let result = decode_register_value(&registers, "uint16", 0, None, None)
            .expect("decoding uint16 should succeed");
        assert_eq!(result, ProtocolValue::Integer(0x1234));

        // Testing int16
        let result = decode_register_value(&registers, "int16", 0, None, None)
            .expect("decoding int16 should succeed");
        assert_eq!(result, ProtocolValue::Integer(i64::from(0x1234_i16)));

        // Testing float32 (needs 2 registers)
        let float_registers = vec![0x4000, 0x0000]; // 2.0 in IEEE 754
        let result = decode_register_value(&float_registers, "float32", 0, None, None)
            .expect("decoding float32 should succeed");
        if let ProtocolValue::Float(f) = result {
            assert!((f - 2.0).abs() < 0.0001);
        } else {
            panic!("Expected float value");
        }
    }

    #[test]
    fn test_decode_register_value_float64_abcd() {
        // Prepare a known f64 value and encode as big-endian bytes (ABCD)
        let v: f64 = 123.456789;
        let bytes = v.to_be_bytes();
        let registers = vec![
            u16::from_be_bytes([bytes[0], bytes[1]]),
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u16::from_be_bytes([bytes[4], bytes[5]]),
            u16::from_be_bytes([bytes[6], bytes[7]]),
        ];

        let result = decode_register_value(&registers, "float64", 0, Some("ABCD"), None)
            .expect("float64 ABCD decode should succeed");
        match result {
            ProtocolValue::Float(f) => assert!((f - v).abs() < 1e-9),
            _ => panic!("Expected float value"),
        }
    }

    #[test]
    fn test_decode_register_value_float64_dcba() {
        // Prepare a known f64 value and encode as little-endian bytes (DCBA path)
        let v: f64 = -9876.54321;
        let bytes = v.to_le_bytes();
        let registers = vec![
            u16::from_be_bytes([bytes[0], bytes[1]]),
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u16::from_be_bytes([bytes[4], bytes[5]]),
            u16::from_be_bytes([bytes[6], bytes[7]]),
        ];

        let result = decode_register_value(&registers, "float64", 0, Some("DCBA"), None)
            .expect("float64 DCBA decode should succeed");
        match result {
            ProtocolValue::Float(f) => assert!((f - v).abs() < 1e-9),
            _ => panic!("Expected float value"),
        }
    }

    #[test]
    fn test_decode_register_value_int32_abcd() {
        // Test int32 with ABCD byte order (big-endian)
        let value: i32 = -12345678;
        let bytes = value.to_be_bytes();
        let registers = vec![
            u16::from_be_bytes([bytes[0], bytes[1]]),
            u16::from_be_bytes([bytes[2], bytes[3]]),
        ];

        let result = decode_register_value(&registers, "int32", 0, Some("ABCD"), None)
            .expect("int32 ABCD decode should succeed");

        match result {
            ProtocolValue::Integer(i) => assert_eq!(i, value as i64),
            _ => panic!("Expected integer value"),
        }
    }

    #[test]
    fn test_decode_register_value_int32_cdab() {
        // Test int32 with CDAB byte order (word-swapped big-endian)
        let value: i32 = 987654321;
        let bytes = value.to_be_bytes();
        // CDAB: swap word order
        let registers = vec![
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u16::from_be_bytes([bytes[0], bytes[1]]),
        ];

        let result = decode_register_value(&registers, "int32", 0, Some("CDAB"), None)
            .expect("int32 CDAB decode should succeed");

        match result {
            ProtocolValue::Integer(i) => assert_eq!(i, value as i64),
            _ => panic!("Expected integer value"),
        }
    }

    #[test]
    fn test_decode_register_value_uint32() {
        let value: u32 = 0xDEADBEEF;
        let registers = vec![(value >> 16) as u16, (value & 0xFFFF) as u16];

        let result = decode_register_value(&registers, "uint32", 0, Some("ABCD"), None)
            .expect("uint32 decode should succeed");

        match result {
            ProtocolValue::Integer(i) => assert_eq!(i as u32, value),
            _ => panic!("Expected integer value"),
        }
    }

    #[test]
    fn test_decode_register_value_float32_cdab() {
        // Test float32 with CDAB byte order
        let value: f32 = std::f32::consts::PI;
        let bytes = value.to_be_bytes();
        // CDAB: swap word order
        let registers = vec![
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u16::from_be_bytes([bytes[0], bytes[1]]),
        ];

        let result = decode_register_value(&registers, "float32", 0, Some("CDAB"), None)
            .expect("float32 CDAB decode should succeed");

        match result {
            ProtocolValue::Float(f) => assert!((f as f32 - value).abs() < 0.0001),
            _ => panic!("Expected float value"),
        }
    }

    #[test]
    fn test_decode_register_value_insufficient_registers() {
        // float32 needs 2 registers but only 1 provided
        let registers = vec![0x1234];

        let result = decode_register_value(&registers, "float32", 0, None, None);

        assert!(result.is_err());
    }

    #[test]
    fn test_decode_register_value_unknown_type() {
        let registers = vec![0x1234];

        let result = decode_register_value(&registers, "unknown_type", 0, None, None);

        // Unknown types should return error
        assert!(result.is_err());
    }

    // ============================================================================
    // Phase 7: clamp_to_data_type tests (migrated from comsrv/protocol.rs)
    // ============================================================================

    #[test]
    fn test_clamp_to_data_type_uint16_overflow() {
        // Upper overflow
        let result = clamp_to_data_type(70000.0, "uint16");
        assert_eq!(result, 65535.0);

        // Lower overflow (negative)
        let result = clamp_to_data_type(-100.0, "uint16");
        assert_eq!(result, 0.0);

        // Valid value
        let result = clamp_to_data_type(1000.0, "uint16");
        assert_eq!(result, 1000.0);

        // Boundary value
        let result = clamp_to_data_type(65535.0, "uint16");
        assert_eq!(result, 65535.0);
    }

    #[test]
    fn test_clamp_to_data_type_int16() {
        // Upper overflow
        let result = clamp_to_data_type(40000.0, "int16");
        assert_eq!(result, 32767.0);

        // Lower overflow
        let result = clamp_to_data_type(-40000.0, "int16");
        assert_eq!(result, -32768.0);

        // Valid negative value
        let result = clamp_to_data_type(-1000.0, "int16");
        assert_eq!(result, -1000.0);
    }

    #[test]
    fn test_clamp_to_data_type_uint32() {
        // Upper overflow
        let result = clamp_to_data_type(5000000000.0, "uint32");
        assert_eq!(result, 4294967295.0);

        // Lower overflow
        let result = clamp_to_data_type(-1.0, "uint32");
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_clamp_to_data_type_bool_passthrough() {
        // Boolean types should pass through unchanged
        let result = clamp_to_data_type(100.0, "bool");
        assert_eq!(result, 100.0);

        let result = clamp_to_data_type(-100.0, "coil");
        assert_eq!(result, -100.0);
    }

    #[test]
    fn test_clamp_to_data_type_case_insensitive() {
        // Test case variations
        let result = clamp_to_data_type(70000.0, "UINT16");
        assert_eq!(result, 65535.0);

        let result = clamp_to_data_type(70000.0, "UInt16");
        assert_eq!(result, 65535.0);

        let result = clamp_to_data_type(70000.0, "u16");
        assert_eq!(result, 65535.0);
    }

    // ============================================================================
    // Phase 8: parse_modbus_pdu tests (migrated from comsrv/protocol.rs)
    // ============================================================================

    #[test]
    fn test_parse_modbus_pdu_fc03_basic() {
        // FC03 response: Function code + Byte count + Data
        // Reading 2 registers: returns 4 bytes of data
        let response_data = vec![
            0x03, // Function code
            0x04, // Byte count (2 registers * 2 bytes)
            0x00, 0x0A, // Register 0: 10
            0x01, 0x02, // Register 1: 258
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x03, 2).expect("FC03 parsing should succeed");

        assert_eq!(registers.len(), 2);
        assert_eq!(registers[0], 0x000A); // 10
        assert_eq!(registers[1], 0x0102); // 258
    }

    #[test]
    fn test_parse_modbus_pdu_fc04_basic() {
        // FC04 response similar to FC03
        let response_data = vec![
            0x04, // Function code
            0x02, // Byte count (1 register)
            0x12, 0x34, // Register 0: 0x1234
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x04, 1).expect("FC04 parsing should succeed");

        assert_eq!(registers.len(), 1);
        assert_eq!(registers[0], 0x1234);
    }

    #[test]
    fn test_parse_modbus_pdu_fc01_coils() {
        // FC01 response: coil status bytes
        // Reading 10 coils: returns ceil(10/8) = 2 bytes
        let response_data = vec![
            0x01, // Function code
            0x02, // Byte count
            0xCD, // Coils 0-7: 11001101
            0x01, // Coils 8-9: 00000001 (only bits 0-1 valid)
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x01, 10).expect("FC01 parsing should succeed");

        // FC01 returns bytes as u16 values for uniform processing
        assert_eq!(registers.len(), 2);
        assert_eq!(registers[0], 0xCD); // First byte
        assert_eq!(registers[1], 0x01); // Second byte
    }

    #[test]
    fn test_parse_modbus_pdu_fc02_discrete_inputs() {
        // FC02 response similar to FC01
        let response_data = vec![
            0x02, // Function code
            0x01, // Byte count
            0xAC, // Inputs 0-7: 10101100
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x02, 8).expect("FC02 parsing should succeed");

        assert_eq!(registers.len(), 1);
        assert_eq!(registers[0], 0xAC);
    }

    #[test]
    fn test_parse_modbus_pdu_function_code_mismatch() {
        let response_data = vec![0x03, 0x02, 0x00, 0x0A];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        // Request was for FC04 but response is FC03
        let result = parse_modbus_pdu(&pdu, 0x04, 1);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("mismatch"));
    }

    #[test]
    fn test_parse_modbus_pdu_unsupported_function_code() {
        let response_data = vec![0x10, 0x00, 0x01, 0x00, 0x02]; // FC16 Write Multiple Registers response
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let result = parse_modbus_pdu(&pdu, 0x10, 2);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported"));
    }

    #[test]
    fn test_parse_modbus_pdu_empty_returns_empty_vec() {
        // Very short PDU - graceful degradation
        let response_data = vec![0x03]; // Only function code, no byte count
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x03, 1).expect("Should degrade gracefully");

        // Empty result due to insufficient data
        assert!(registers.is_empty());
    }

    #[test]
    fn test_parse_modbus_pdu_fc03_partial_data() {
        // FC03 response with incomplete register data (graceful degradation)
        let response_data = vec![
            0x03, // Function code
            0x04, // Byte count says 4 bytes (2 registers)
            0x00, 0x0A, // Only 1 complete register
            0x01, // Incomplete second register (only 1 byte)
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x03, 2).expect("Should parse partial data");

        // Should return only complete registers
        assert_eq!(registers.len(), 1);
        assert_eq!(registers[0], 0x000A);
    }

    #[test]
    fn test_parse_modbus_pdu_fc03_multiple_registers() {
        // FC03 response with 5 registers
        let response_data = vec![
            0x03, // Function code
            0x0A, // Byte count (5 registers * 2 bytes = 10)
            0x00, 0x01, // Register 0: 1
            0x00, 0x02, // Register 1: 2
            0x00, 0x03, // Register 2: 3
            0x00, 0x04, // Register 3: 4
            0x00, 0x05, // Register 4: 5
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers =
            parse_modbus_pdu(&pdu, 0x03, 5).expect("FC03 multi-register should succeed");

        assert_eq!(registers.len(), 5);
        for (i, reg) in registers.iter().enumerate() {
            assert_eq!(*reg, (i + 1) as u16);
        }
    }
}
