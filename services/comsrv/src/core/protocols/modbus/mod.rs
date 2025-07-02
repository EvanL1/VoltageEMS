//! Modbus Protocol Implementation
//!
//! This module provides a comprehensive native Modbus implementation supporting both RTU and TCP modes.
//! It features a complete protocol stack with PDU processing, frame handling, and transport integration:
//!
//! - **PDU Module**: Protocol Data Unit processing for all Modbus function codes
//! - **Frame Module**: TCP MBAP and RTU CRC frame handling
//! - **Client Module**: Complete Modbus client implementation
//! - **Server Module**: Modbus server with device simulation
//! - **Common Module**: Shared types, configurations, and utilities
//!
//! # Architecture
//!
//! The implementation follows a layered approach:
//! 1. **PDU Layer**: Handles parsing and building of Modbus Protocol Data Units
//! 2. **Frame Layer**: Manages TCP MBAP headers and RTU CRC checksums
//! 3. **Transport Layer**: Uses UniversalTransportBridge for communication
//! 4. **Protocol Layer**: Provides high-level client/server APIs
//!
//! # Native Implementation
//!
//! This is a pure Rust implementation without external dependencies, providing:
//! - Complete control over protocol handling
//! - Support for all standard Modbus function codes
//! - Proper error handling and exception responses
//! - Integrated statistics and diagnostics

pub mod pdu;
pub mod frame;
pub mod client;
pub mod protocol_engine;
pub mod server;
pub mod common;
pub mod modbus_polling;

pub mod tests;

// Re-export main types for easier usage
pub use client::{ModbusClient, ModbusChannelConfig, ProtocolMappingTable, ConnectionState, ClientStatistics};
pub use protocol_engine::{ModbusProtocolEngine, ProtocolEngineConfig};
pub use server::{ModbusServer, ModbusDevice};
pub use pdu::{ModbusFunctionCode, ModbusPduProcessor, ModbusExceptionCode};
pub use frame::{ModbusFrameProcessor, ModbusMode};
pub use common::{ModbusConfig, ModbusPoint};

use crate::core::protocols::common::combase::{PacketParseResult, ProtocolPacketParser};
use std::collections::HashMap;
use chrono::Utc;

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

    /// Format hex data
    fn format_hex_data(&self, data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<String>>()
            .join(" ")
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
                fields.insert("coil_value".to_string(), coil_value.to_string());
                format!("Write coil at address {} to {}", address, coil_value)
            }
            0x06 => format!("Write register at address {} to {}", address, value),
            _ => format!("Write single at address {} to {}", address, value),
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
                fields.insert("coil_value".to_string(), coil_value.to_string());
                format!("Confirmed write coil at address {} to {}", address, coil_value)
            }
            0x06 => format!("Confirmed write register at address {} to {}", address, value),
            _ => format!("Confirmed write single at address {} to {}", address, value),
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
            0x07 => "Negative Acknowledge",
            0x08 => "Memory Parity Error",
            0x0A => "Gateway Path Unavailable",
            0x0B => "Gateway Target Device Failed to Respond",
            _ => "Unknown Exception",
        };

        let mut fields = HashMap::new();
        fields.insert("original_function".to_string(), original_function.to_string());
        fields.insert("exception_code".to_string(), exception_code.to_string());
        fields.insert("exception_name".to_string(), exception_name.to_string());

        let description = format!(
            "Error response to function {}: {} (0x{:02X})",
            Self::function_code_name(original_function),
            exception_name,
            exception_code
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
        let (transaction_id, _protocol_id, _length, unit_id, function_code) =
            match self.parse_tcp_header(data) {
                Ok(header) => header,
                Err(err) => {
                    return PacketParseResult {
                        success: false,
                        protocol: "Modbus".to_string(),
                        direction: direction.to_string(),
                        hex_data,
                        parsed_data: None,
                        error_message: Some(err.to_string()),
                        timestamp: Utc::now(),
                    };
                }
            };

        // Parse function-specific data
        let (func_description, _func_fields) = if function_code & 0x80 != 0 {
            // Error response
            self.parse_error_response(data, function_code)
        } else {
            match function_code {
                0x01..=0x04 => {
                    if direction == "request" {
                        self.parse_read_request(data, function_code)
                    } else {
                        self.parse_read_response(data, function_code)
                    }
                }
                0x05 | 0x06 => {
                    if direction == "request" {
                        self.parse_write_single_request(data, function_code)
                    } else {
                        self.parse_write_single_response(data, function_code)
                    }
                }
                _ => (
                    format!("Function {} (0x{:02X})", Self::function_code_name(function_code), function_code),
                    HashMap::new(),
                ),
            }
        };

        // Build complete description
        let description = format!(
            "TxID:0x{:04X} Unit:{} FC:0x{:02X}({}) - {}",
            transaction_id,
            unit_id,
            function_code,
            Self::function_code_name(function_code),
            func_description
        );

        PacketParseResult {
            success: true,
            protocol: "Modbus".to_string(),
            direction: direction.to_string(),
            hex_data,
            parsed_data: Some(description),
            error_message: None,
            timestamp: Utc::now(),
        }
    }
}
