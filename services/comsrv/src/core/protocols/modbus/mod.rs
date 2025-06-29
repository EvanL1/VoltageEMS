//! Modbus Protocol Implementation
//!
//! This module provides a comprehensive Modbus implementation supporting both RTU and TCP modes.
//! It integrates with the voltage_modbus library and provides enhanced features like:
//!
//! - **Client Module**: Unified client supporting RTU and TCP communication
//! - **Server Module**: Unified server with device simulation capabilities  
//! - **Common Module**: Shared types, utilities, and protocol definitions
//!
//! # Architecture
//!
//! The design follows a unified approach where communication mode (RTU vs TCP) is determined
//! by configuration rather than separate modules. This reduces code duplication and provides
//! a consistent API across different transport layers.
//!
//! # Integration with voltage_modbus
//!
//! This implementation leverages the voltage_modbus library for core protocol handling while
//! adding enhanced features like connection management, statistics collection, and point
//! value caching.

pub mod bit_operations;
pub mod client;
pub mod common;
pub mod server;

// Re-enabled comprehensive tests with updated structure
// #[cfg(test)]
// pub mod comprehensive_tests;

// Re-export main types for easier usage
pub use client::{ModbusClient, ModbusClientConfig, ModbusCommunicationMode};

use crate::core::protocols::common::combase::{PacketParseResult, ProtocolPacketParser};
use std::collections::HashMap;

/// Modbus protocol packet parser
///
/// Implements protocol-specific parsing for Modbus TCP packets,
/// providing human-readable interpretation of packet structure and data.
pub struct ModbusPacketParser;

impl ModbusPacketParser {
    /// Create a new Modbus packet parser
    pub fn new() -> Self {
        Self
    }

    /// Parse Modbus function code to human-readable name
    fn function_code_name(code: u8) -> &'static str {
        match code {
            0x01 => "Read Coils",
            0x02 => "Read Discrete Inputs",
            0x03 => "Read Holding Registers",
            0x04 => "Read Input Registers",
            0x05 => "Write Single Coil",
            0x06 => "Write Single Register",
            0x0F => "Write Multiple Coils",
            0x10 => "Write Multiple Registers",
            0x16 => "Mask Write Register",
            0x17 => "Read/Write Multiple Registers",
            _ => {
                if code & 0x80 != 0 {
                    "Error Response"
                } else {
                    "Unknown Function"
                }
            }
        }
    }

    /// Parse Modbus TCP header
    fn parse_tcp_header(&self, data: &[u8]) -> Result<(u16, u16, u16, u8, u8), &'static str> {
        if data.len() < 8 {
            return Err("Packet too short for Modbus TCP header");
        }

        let transaction_id = u16::from_be_bytes([data[0], data[1]]);
        let protocol_id = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let unit_id = data[6];
        let function_code = data[7];

        Ok((transaction_id, protocol_id, length, unit_id, function_code))
    }

    /// Parse read request (functions 0x01, 0x02, 0x03, 0x04)
    fn parse_read_request(
        &self,
        data: &[u8],
        function_code: u8,
    ) -> (String, HashMap<String, String>) {
        if data.len() < 12 {
            return ("Invalid read request".to_string(), HashMap::new());
        }

        let start_address = u16::from_be_bytes([data[8], data[9]]);
        let quantity = u16::from_be_bytes([data[10], data[11]]);

        let register_type = match function_code {
            0x01 => "coils",
            0x02 => "discrete inputs",
            0x03 => "holding registers",
            0x04 => "input registers",
            _ => "unknown registers",
        };

        let description = format!(
            "Read {} {} from address {}",
            quantity, register_type, start_address
        );

        let mut fields = HashMap::new();
        fields.insert("start_address".to_string(), start_address.to_string());
        fields.insert("quantity".to_string(), quantity.to_string());
        fields.insert("register_type".to_string(), register_type.to_string());

        (description, fields)
    }

    /// Parse read response (functions 0x01, 0x02, 0x03, 0x04)
    fn parse_read_response(
        &self,
        data: &[u8],
        function_code: u8,
    ) -> (String, HashMap<String, String>) {
        if data.len() < 9 {
            return ("Invalid read response".to_string(), HashMap::new());
        }

        let byte_count = data[8];
        let data_bytes = &data[9..];

        let mut fields = HashMap::new();
        fields.insert("byte_count".to_string(), byte_count.to_string());

        let description = match function_code {
            0x01 | 0x02 => {
                // Coils/discrete inputs - bit values
                let mut bit_values = Vec::new();
                for (i, &byte) in data_bytes.iter().enumerate() {
                    for bit in 0..8 {
                        if i * 8 + bit < (byte_count * 8) as usize {
                            bit_values.push(((byte >> bit) & 1).to_string());
                        }
                    }
                }
                fields.insert(
                    "bit_values".to_string(),
                    format!("[{}]", bit_values.join(", ")),
                );
                format!("Response: {} bytes, {} bits", byte_count, bit_values.len())
            }
            0x03 | 0x04 => {
                // Holding/input registers - word values
                let mut register_values = Vec::new();
                for chunk in data_bytes.chunks(2) {
                    if chunk.len() == 2 {
                        let value = u16::from_be_bytes([chunk[0], chunk[1]]);
                        register_values.push(value.to_string());
                    }
                }
                fields.insert(
                    "register_values".to_string(),
                    format!("[{}]", register_values.join(", ")),
                );
                format!(
                    "Response: {} bytes, values: [{}]",
                    byte_count,
                    register_values.join(", ")
                )
            }
            _ => format!("Response: {} bytes", byte_count),
        };

        (description, fields)
    }

    /// Parse write single request (functions 0x05, 0x06)
    fn parse_write_single_request(
        &self,
        data: &[u8],
        function_code: u8,
    ) -> (String, HashMap<String, String>) {
        if data.len() < 12 {
            return ("Invalid write single request".to_string(), HashMap::new());
        }

        let address = u16::from_be_bytes([data[8], data[9]]);
        let value = u16::from_be_bytes([data[10], data[11]]);

        let mut fields = HashMap::new();
        fields.insert("address".to_string(), address.to_string());
        fields.insert("value".to_string(), value.to_string());

        let description = match function_code {
            0x05 => {
                let coil_value = if value == 0xFF00 { "ON" } else { "OFF" };
                fields.insert("coil_state".to_string(), coil_value.to_string());
                format!("Write coil {} to {}", address, coil_value)
            }
            0x06 => {
                format!("Write value {} to register {}", value, address)
            }
            _ => format!("Write value {} to address {}", value, address),
        };

        (description, fields)
    }

    /// Parse write single response (functions 0x05, 0x06)
    fn parse_write_single_response(
        &self,
        data: &[u8],
        function_code: u8,
    ) -> (String, HashMap<String, String>) {
        if data.len() < 12 {
            return ("Invalid write single response".to_string(), HashMap::new());
        }

        let address = u16::from_be_bytes([data[8], data[9]]);
        let value = u16::from_be_bytes([data[10], data[11]]);

        let mut fields = HashMap::new();
        fields.insert("address".to_string(), address.to_string());
        fields.insert("value".to_string(), value.to_string());

        let description = match function_code {
            0x05 => {
                let coil_value = if value == 0xFF00 { "ON" } else { "OFF" };
                fields.insert("coil_state".to_string(), coil_value.to_string());
                format!("Confirmed: wrote coil {} to {}", address, coil_value)
            }
            0x06 => {
                format!("Confirmed: wrote value {} to register {}", value, address)
            }
            _ => format!("Confirmed: wrote value {} to address {}", value, address),
        };

        (description, fields)
    }

    /// Parse error response
    fn parse_error_response(
        &self,
        data: &[u8],
        function_code: u8,
    ) -> (String, HashMap<String, String>) {
        if data.len() < 9 {
            return ("Invalid error response".to_string(), HashMap::new());
        }

        let exception_code = data[8];
        let original_function = function_code & 0x7F;

        let exception_name = match exception_code {
            0x01 => "Illegal Function",
            0x02 => "Illegal Data Address",
            0x03 => "Illegal Data Value",
            0x04 => "Slave Device Failure",
            0x05 => "Acknowledge",
            0x06 => "Slave Device Busy",
            0x08 => "Memory Parity Error",
            0x0A => "Gateway Path Unavailable",
            0x0B => "Gateway Target Failed",
            _ => "Unknown Exception",
        };

        let mut fields = HashMap::new();
        fields.insert(
            "original_function".to_string(),
            format!("0x{:02x}", original_function),
        );
        fields.insert(
            "exception_code".to_string(),
            format!("0x{:02x}", exception_code),
        );
        fields.insert("exception_name".to_string(), exception_name.to_string());

        let description = format!(
            "Error: {} (0x{:02x}) for function 0x{:02x}",
            exception_name, exception_code, original_function
        );

        (description, fields)
    }
}

impl ProtocolPacketParser for ModbusPacketParser {
    fn protocol_name(&self) -> &str {
        "Modbus"
    }

    fn parse_packet(&self, data: &[u8], direction: &str) -> PacketParseResult {
        let hex_data = self.format_hex_data(data);

        // Parse TCP header
        let (transaction_id, protocol_id, length, unit_id, function_code) =
            match self.parse_tcp_header(data) {
                Ok(header) => header,
                Err(e) => {
                    return PacketParseResult::failure("Modbus", direction, &hex_data, e);
                }
            };

        let mut fields = HashMap::new();
        fields.insert(
            "transaction_id".to_string(),
            format!("0x{:04x}", transaction_id),
        );
        fields.insert("protocol_id".to_string(), protocol_id.to_string());
        fields.insert("length".to_string(), length.to_string());
        fields.insert("unit_id".to_string(), unit_id.to_string());
        fields.insert(
            "function_code".to_string(),
            format!("0x{:02x}", function_code),
        );
        fields.insert(
            "function_name".to_string(),
            Self::function_code_name(function_code).to_string(),
        );

        // Base description with header info
        let mut description = format!(
            "TxID:0x{:04x} ProtoID:{} Len:{} Unit:{} FC:0x{:02x}({})",
            transaction_id,
            protocol_id,
            length,
            unit_id,
            function_code,
            Self::function_code_name(function_code)
        );

        // Parse function-specific data
        let (func_description, func_fields) = if function_code & 0x80 != 0 {
            // Error response
            self.parse_error_response(data, function_code)
        } else {
            match function_code {
                0x01 | 0x02 | 0x03 | 0x04 => {
                    if direction == "send" {
                        self.parse_read_request(data, function_code)
                    } else {
                        self.parse_read_response(data, function_code)
                    }
                }
                0x05 | 0x06 => {
                    if direction == "send" {
                        self.parse_write_single_request(data, function_code)
                    } else {
                        self.parse_write_single_response(data, function_code)
                    }
                }
                _ => ("Unsupported function".to_string(), HashMap::new()),
            }
        };

        // Combine descriptions
        if !func_description.is_empty() {
            description = format!("{} - {}", description, func_description);
        }

        // Merge fields
        fields.extend(func_fields);

        PacketParseResult::success("Modbus", direction, &hex_data, &description)
    }
}
