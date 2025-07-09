//! Protocol mapping trait and implementations
//!
//! This module defines a common interface for protocol mappings
//! and provides implementations for different protocols.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Common trait for all protocol mappings
pub trait ProtocolMapping: Send + Sync {
    /// Get the point ID
    fn point_id(&self) -> u32;

    /// Get the signal name
    fn signal_name(&self) -> &str;

    /// Convert to protocol-specific address parameters
    fn to_protocol_params(&self) -> HashMap<String, String>;

    /// Get the data format
    fn data_format(&self) -> &str;

    /// Get the number of bytes/registers
    fn data_size(&self) -> u8;
}

/// Modbus protocol mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_format: String,
    pub number_of_bytes: Option<u8>, // Made optional
    pub bit_position: Option<u8>,
    pub byte_order: Option<String>,
    pub register_count: Option<u16>, // Primary field for size
    pub description: Option<String>,
}

impl ProtocolMapping for ModbusMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn to_protocol_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("slave_id".to_string(), self.slave_id.to_string());
        params.insert("function_code".to_string(), self.function_code.to_string());
        params.insert(
            "register_address".to_string(),
            self.register_address.to_string(),
        );
        params.insert("data_format".to_string(), self.data_format.clone());

        if let Some(bit) = self.bit_position {
            params.insert("bit_position".to_string(), bit.to_string());
        }

        if let Some(ref order) = self.byte_order {
            params.insert("byte_order".to_string(), order.clone());
        }

        if let Some(count) = self.register_count {
            params.insert("register_count".to_string(), count.to_string());
        }

        params
    }

    fn data_format(&self) -> &str {
        &self.data_format
    }

    fn data_size(&self) -> u8 {
        // Use register_count if available, otherwise calculate from data_format
        if let Some(count) = self.register_count {
            (count * 2) as u8 // Each register is 2 bytes
        } else if let Some(bytes) = self.number_of_bytes {
            bytes // Use number_of_bytes if provided
        } else {
            // Default based on data format
            match self.data_format.as_str() {
                "bool" | "uint8" | "int8" => 1,
                "uint16" | "int16" => 2,
                "uint32" | "int32" | "float32" | "float32_be" | "float32_le" => 4,
                "uint64" | "int64" | "float64" | "float64_be" | "float64_le" => 8,
                _ => 2, // Default to 2 bytes (1 register)
            }
        }
    }
}

/// CAN protocol mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub can_id: u32,
    pub start_byte: u8,
    pub bit_length: u8,
    pub byte_order: String,
    pub sign_type: String,
    pub scale: f64,
    pub offset: f64,
    pub description: Option<String>,
}

impl ProtocolMapping for CanMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn to_protocol_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("can_id".to_string(), format!("0x{:X}", self.can_id));
        params.insert("start_byte".to_string(), self.start_byte.to_string());
        params.insert("bit_length".to_string(), self.bit_length.to_string());
        params.insert("byte_order".to_string(), self.byte_order.clone());
        params.insert("sign_type".to_string(), self.sign_type.clone());
        params.insert("scale".to_string(), self.scale.to_string());
        params.insert("offset".to_string(), self.offset.to_string());
        params
    }

    fn data_format(&self) -> &str {
        &self.sign_type
    }

    fn data_size(&self) -> u8 {
        (self.bit_length + 7) / 8 // Convert bits to bytes
    }
}

/// IEC 60870-5-104 protocol mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iec104Mapping {
    pub point_id: u32,
    pub signal_name: String,
    pub ioa: u32,    // Information Object Address
    pub type_id: u8, // ASDU Type ID
    pub ca: u16,     // Common Address
    pub cot: u8,     // Cause of Transmission
    pub description: Option<String>,
}

impl ProtocolMapping for Iec104Mapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn to_protocol_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("ioa".to_string(), self.ioa.to_string());
        params.insert("type_id".to_string(), self.type_id.to_string());
        params.insert("ca".to_string(), self.ca.to_string());
        params.insert("cot".to_string(), self.cot.to_string());
        params
    }

    fn data_format(&self) -> &str {
        match self.type_id {
            1..=14 => "BOOL",   // Single/double point information
            15..=40 => "FLOAT", // Measured values
            45..=64 => "BOOL",  // Commands
            _ => "UNKNOWN",
        }
    }

    fn data_size(&self) -> u8 {
        match self.type_id {
            1..=14 => 1,  // Boolean types
            15..=20 => 2, // Normalized values
            21..=40 => 4, // Float values
            _ => 1,
        }
    }
}

/// DIO (Digital I/O) protocol mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DioMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub channel: u8,
    pub pin: u8,
    pub direction: String, // "input" or "output"
    pub active_low: bool,
    pub debounce_ms: Option<u32>,
    pub description: Option<String>,
}

impl ProtocolMapping for DioMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn to_protocol_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.to_string());
        params.insert("pin".to_string(), self.pin.to_string());
        params.insert("direction".to_string(), self.direction.clone());
        params.insert("active_low".to_string(), self.active_low.to_string());

        if let Some(debounce) = self.debounce_ms {
            params.insert("debounce_ms".to_string(), debounce.to_string());
        }

        params
    }

    fn data_format(&self) -> &str {
        "BOOL"
    }

    fn data_size(&self) -> u8 {
        1
    }
}
