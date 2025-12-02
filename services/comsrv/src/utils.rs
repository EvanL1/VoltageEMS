//! Utility Functions and Common Components
//!
//! This module provides essential utilities and shared
//! components used throughout the communication service library.
//!
//! # Features
//!
//! - Protocol name normalization
//! - Bytes utilities (re-exported from voltage_comlink)

use std::str::FromStr;
use voltage_config::common::ProtocolType;

// Re-export bytes utilities from voltage_comlink
pub use voltage_comlink::bytes::*;

/// Normalize protocol name to standard format (lowercase underscore)
/// This ensures consistency across configuration files, plugins, and database
pub fn normalize_protocol_name(name: &str) -> String {
    // Clean input: trim whitespace and convert to lowercase
    let cleaned = name.trim().to_lowercase();

    // Replace common separators with underscores for matching
    let normalized = cleaned.replace(['-', ' ', '.'], "_");

    // Map various protocol name variations to standard names
    match normalized.as_str() {
        // Modbus variations
        "modbus_tcp" | "modbustcp" | "modbus tcp" => "modbus_tcp".to_string(),
        "modbus_rtu" | "modbusrtu" | "modbus rtu" => "modbus_rtu".to_string(),
        "modbus_ascii" | "modbusascii" | "modbus ascii" => "modbus_ascii".to_string(),

        // Virtual protocol variations
        "virtual" | "virt" | "virtual_protocol" => "virtual".to_string(),

        // IEC variations
        "iec104" | "iec_104" | "iec60870" | "iec_60870" | "iec60870_5_104" | "iec_60870_5_104" => {
            "iec104".to_string()
        },

        // gRPC variations
        "grpc" | "g_rpc" => "grpc".to_string(),

        // MQTT variations
        "mqtt" | "mqtt_protocol" => "mqtt".to_string(),

        // OPC UA variations
        "opcua" | "opc_ua" | "opc ua" => "opcua".to_string(),

        // Default: return cleaned name with underscores
        _ => normalized,
    }
}

/// Parse protocol name string to ProtocolType enum
/// Returns None if the protocol type is not recognized
pub fn parse_protocol_type(name: &str) -> Option<ProtocolType> {
    // Use normalize_protocol_name to handle variations
    let normalized = normalize_protocol_name(name);

    // Try to parse using FromStr implementation
    ProtocolType::from_str(&normalized).ok()
}

/// Get protocol type string from enum
pub fn protocol_type_to_string(protocol: ProtocolType) -> String {
    protocol.as_str().to_string()
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_normalize_protocol_name() {
        // Test Modbus variations
        assert_eq!(normalize_protocol_name("modbus_tcp"), "modbus_tcp");
        assert_eq!(normalize_protocol_name("modbustcp"), "modbus_tcp");
        assert_eq!(normalize_protocol_name("MODBUSTCP"), "modbus_tcp");
        assert_eq!(normalize_protocol_name("modbus-tcp"), "modbus_tcp");
        assert_eq!(normalize_protocol_name("modbus tcp"), "modbus_tcp");
        assert_eq!(normalize_protocol_name(" Modbus_TCP "), "modbus_tcp");

        assert_eq!(normalize_protocol_name("modbus_rtu"), "modbus_rtu");
        assert_eq!(normalize_protocol_name("modbusrtu"), "modbus_rtu");
        assert_eq!(normalize_protocol_name("MODBUS-RTU"), "modbus_rtu");

        // Test Virtual variations
        assert_eq!(normalize_protocol_name("virtual"), "virtual");
        assert_eq!(normalize_protocol_name("virt"), "virtual");
        assert_eq!(normalize_protocol_name("VIRTUAL"), "virtual");
        assert_eq!(normalize_protocol_name("virtual_protocol"), "virtual");

        // Test IEC variations
        assert_eq!(normalize_protocol_name("iec104"), "iec104");
        assert_eq!(normalize_protocol_name("iec_104"), "iec104");
        assert_eq!(normalize_protocol_name("iec60870"), "iec104");
        assert_eq!(normalize_protocol_name("IEC-60870-5-104"), "iec104");

        // Test other protocols
        assert_eq!(normalize_protocol_name("grpc"), "grpc");
        assert_eq!(normalize_protocol_name("GRPC"), "grpc");
        assert_eq!(normalize_protocol_name("mqtt"), "mqtt");
        assert_eq!(normalize_protocol_name("opcua"), "opcua");
        assert_eq!(normalize_protocol_name("OPC-UA"), "opcua");

        // Test unknown protocols (should return cleaned version)
        assert_eq!(
            normalize_protocol_name("custom-protocol"),
            "custom_protocol"
        );
        assert_eq!(normalize_protocol_name("NEW.PROTOCOL"), "new_protocol");
    }

    #[test]
    fn test_parse_protocol_type() {
        // Test valid protocol types
        assert_eq!(
            parse_protocol_type("modbus_tcp"),
            Some(ProtocolType::ModbusTcp)
        );
        assert_eq!(
            parse_protocol_type("modbustcp"),
            Some(ProtocolType::ModbusTcp)
        );
        assert_eq!(
            parse_protocol_type("MODBUS-TCP"),
            Some(ProtocolType::ModbusTcp)
        );
        assert_eq!(
            parse_protocol_type(" Modbus TCP "),
            Some(ProtocolType::ModbusTcp)
        );

        assert_eq!(
            parse_protocol_type("modbus_rtu"),
            Some(ProtocolType::ModbusRtu)
        );
        assert_eq!(
            parse_protocol_type("modbusrtu"),
            Some(ProtocolType::ModbusRtu)
        );
        assert_eq!(
            parse_protocol_type("MODBUS-RTU"),
            Some(ProtocolType::ModbusRtu)
        );

        assert_eq!(parse_protocol_type("virtual"), Some(ProtocolType::Virtual));
        assert_eq!(parse_protocol_type("virt"), Some(ProtocolType::Virtual));
        assert_eq!(parse_protocol_type("VIRTUAL"), Some(ProtocolType::Virtual));

        // Test invalid protocol types
        assert_eq!(parse_protocol_type("unknown"), None);
        assert_eq!(parse_protocol_type("can"), None); // CAN protocol removed
        assert_eq!(parse_protocol_type("iec104"), None); // Not in ProtocolType enum
        assert_eq!(parse_protocol_type("mqtt"), None); // Not in ProtocolType enum
    }

    #[test]
    fn test_protocol_type_to_string() {
        assert_eq!(
            protocol_type_to_string(ProtocolType::ModbusTcp),
            "modbus_tcp"
        );
        assert_eq!(
            protocol_type_to_string(ProtocolType::ModbusRtu),
            "modbus_rtu"
        );
        assert_eq!(protocol_type_to_string(ProtocolType::Virtual), "virtual");
    }
}
