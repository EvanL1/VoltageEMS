//! Utility Functions and Common Components
//!
//! This module provides essential utilities and shared
//! components used throughout the communication service library.
//!
//! # Modules
//!
//! - [`error`] - Error handling and result types
//! - [`bytes`] - Binary data processing utilities (byte order, bit operations, conversions)

pub mod bytes;
pub mod error;

use std::str::FromStr;
use voltage_config::common::ProtocolType;

// Re-export error types for convenience
pub use error::{ComSrvError, ErrorExt, Result};

// Re-export bytes utilities for convenience
pub use bytes::ByteOrder;

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

        // CAN variations
        "can" | "canbus" | "can_bus" => "can".to_string(),

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
    use std::io;

    #[test]
    fn test_module_exports() {
        // Test that we can use the re-exported types
        let error = ComSrvError::ConfigError("test error".to_string());
        assert!(error.to_string().contains("test error"));

        // Test Result type alias
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.ok(), Some(42));

        let error_result: Result<i32> = Err(ComSrvError::IoError("test io error".to_string()));
        assert!(error_result.is_err());
    }

    #[test]
    fn test_error_types() {
        // Test basic error creation and handling
        let config_error = ComSrvError::ConfigError("test config error".to_string());
        assert!(config_error.to_string().contains("config"));

        let io_error = ComSrvError::IoError("test io error".to_string());
        assert!(io_error.to_string().contains("io"));
    }

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

        // Test CAN variations
        assert_eq!(normalize_protocol_name("can"), "can");
        assert_eq!(normalize_protocol_name("CAN"), "can");
        assert_eq!(normalize_protocol_name("canbus"), "can");
        assert_eq!(normalize_protocol_name("can_bus"), "can");
        assert_eq!(normalize_protocol_name("can-bus"), "can");

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
    fn test_error_conversion_integration() {
        // Test that we can convert between error types
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let comsrv_error: ComSrvError = io_error.into();
        assert!(matches!(comsrv_error, ComSrvError::IoError(_)));
    }

    #[test]
    fn test_error_context_integration() {
        use crate::utils::error::ErrorExt;

        // Test that ErrorExt trait is available through the module
        let io_result: std::result::Result<String, io::Error> = Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "access denied",
        ));

        let contextualized = io_result.context("Failed to access resource");
        assert!(contextualized.is_err());

        let error = contextualized.unwrap_err();
        assert!(error.to_string().contains("Failed to access resource"));
        assert!(error.to_string().contains("access denied"));
    }

    #[test]
    fn test_module_structure() {
        // Verify that the expected modules are accessible

        // Test error module availability
        let _error_type = ComSrvError::ConfigError("test".to_string());

        // We can't directly test logger and pool modules here without importing them,
        // but we can verify the module exists by checking compilation

        // This test mainly serves as a compilation check for module structure
        // Module structure is valid if this compiles
    }

    #[test]
    fn test_comprehensive_error_types() {
        // Test various error types that should be available
        let errors = vec![
            ComSrvError::ConfigError("config".to_string()),
            ComSrvError::IoError("io".to_string()),
            ComSrvError::ProtocolError("protocol".to_string()),
            ComSrvError::ConnectionError("connection".to_string()),
            ComSrvError::TimeoutError("timeout".to_string()),
            ComSrvError::InternalError("internal".to_string()),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn test_error_types_comprehensive() {
        // Test various ComSrvError types
        let errors = vec![
            ComSrvError::ConfigError("config error".to_string()),
            ComSrvError::ConnectionError("connection error".to_string()),
            ComSrvError::TimeoutError("timeout error".to_string()),
            ComSrvError::ProtocolError("protocol error".to_string()),
            ComSrvError::InternalError("internal error".to_string()),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[tokio::test]
    async fn test_async_error_handling() {
        // Test error handling in async context
        fn failing_async_operation() -> Result<String> {
            Err(ComSrvError::TimeoutError("async timeout".to_string()))
        }

        let result = failing_async_operation();
        assert!(result.is_err());

        if let Err(error) = result {
            assert!(matches!(error, ComSrvError::TimeoutError(_)));
        }
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

        assert_eq!(parse_protocol_type("can"), Some(ProtocolType::Can));
        assert_eq!(parse_protocol_type("CAN"), Some(ProtocolType::Can));
        assert_eq!(parse_protocol_type("canbus"), Some(ProtocolType::Can));
        assert_eq!(parse_protocol_type("can-bus"), Some(ProtocolType::Can));

        assert_eq!(parse_protocol_type("virtual"), Some(ProtocolType::Virtual));
        assert_eq!(parse_protocol_type("virt"), Some(ProtocolType::Virtual));
        assert_eq!(parse_protocol_type("VIRTUAL"), Some(ProtocolType::Virtual));

        // Test invalid protocol types
        assert_eq!(parse_protocol_type("unknown"), None);
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
        assert_eq!(protocol_type_to_string(ProtocolType::Can), "can");
        assert_eq!(protocol_type_to_string(ProtocolType::Virtual), "virtual");
    }
}
