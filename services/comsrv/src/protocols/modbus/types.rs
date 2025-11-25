//! Modbus protocol data types and configuration
//!
//! Contains simplified Modbus point definitions, polling configuration and batch processing configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use voltage_config::common::timeouts;

/// Simplified Modbus point mapping
/// Contains protocol-related fields and transformation parameters for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusPoint {
    /// Unique point identifier (matches four-telemetry table)
    pub point_id: String,
    /// Modbus slave ID
    pub slave_id: u8,
    /// Read function code
    pub function_code: u8,
    /// Register address
    pub register_address: u16,
    /// Data format (e.g., "float32", "uint16", "bool")
    pub data_type: String,
    /// Number of registers to read (e.g., 2 for float32)
    pub register_count: u16,
    /// Byte order for multi-register values (e.g., "ABCD", "CDAB")
    pub byte_order: Option<String>,
    /// Bit position within 16-bit register (0-15, default: 0 = LSB)
    #[serde(default)]
    pub bit_position: u8,

    // Transformation parameters (for logging and validation purposes)
    /// Scale factor for linear transformation (processed = raw * scale + offset)
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// Offset for linear transformation
    #[serde(default)]
    pub offset: f64,
    /// Boolean reversal flag (for signal/control points)
    #[serde(default)]
    pub reverse: bool,
}

fn default_scale() -> f64 {
    1.0
}

/// Modbus polling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusPollingConfig {
    /// Whether polling is enabled
    pub enabled: bool,
    /// Global default polling interval (milliseconds)
    pub default_interval_ms: u64,
    /// Connection timeout (milliseconds)
    pub connection_timeout_ms: u64,
    /// Read timeout (milliseconds)
    pub read_timeout_ms: u64,
    /// Maximum retry count
    pub max_retries: u32,
    /// Retry interval after error (milliseconds)
    pub retry_interval_ms: u64,
    /// Batch processing configuration
    pub batch_config: ModbusBatchConfig,
    /// Slave-specific configuration
    pub slaves: HashMap<u8, SlavePollingConfig>,

    // Reconnection configuration (normally not needed in config file)
    /// Enable automatic reconnection (default: true)
    #[serde(skip_serializing_if = "is_default_reconnect_enabled")]
    #[serde(default = "default_reconnect_enabled")]
    pub reconnect_enabled: bool,
    /// Max consecutive reconnect attempts before waiting longer (default: 5)
    /// After this many failures, wait reconnect_cooldown_ms before trying again
    #[serde(skip_serializing_if = "is_default_reconnect_retries")]
    #[serde(default = "default_reconnect_retries")]
    pub reconnect_max_consecutive: u32,
    /// Cooldown period after max consecutive failures (default: 60000ms = 1 minute)
    #[serde(skip_serializing_if = "is_default_reconnect_cooldown")]
    #[serde(default = "default_reconnect_cooldown_ms")]
    pub reconnect_cooldown_ms: u64,
}

// Default value functions for serde
fn default_reconnect_enabled() -> bool {
    true
}
fn default_reconnect_retries() -> u32 {
    5 // After 5 consecutive failures, wait cooldown period before trying again
}
fn default_reconnect_cooldown_ms() -> u64 {
    timeouts::RECONNECT_COOLDOWN_MS
}

// Helper functions for skip_serializing_if
fn is_default_reconnect_enabled(v: &bool) -> bool {
    *v == default_reconnect_enabled()
}
fn is_default_reconnect_retries(v: &u32) -> bool {
    *v == default_reconnect_retries()
}
fn is_default_reconnect_cooldown(v: &u64) -> bool {
    *v == default_reconnect_cooldown_ms()
}

/// Slave polling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlavePollingConfig {
    /// Slave ID
    pub slave_id: u8,
    /// Polling interval (milliseconds)
    pub interval_ms: u64,
    /// Whether this slave is enabled
    pub enabled: bool,
    /// Slave-specific timeout
    pub timeout_ms: Option<u64>,
    /// Slave description
    pub description: Option<String>,
}

/// Modbus batch read configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusBatchConfig {
    /// Whether batch reading is enabled
    pub enabled: bool,
    /// Maximum batch size (number of registers)
    pub max_batch_size: u16,
    /// Address gap threshold
    pub max_gap: u16,
    /// Device-specific limits
    pub device_limits: HashMap<u8, DeviceLimit>,
}

/// Device-specific limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLimit {
    /// Maximum registers per read
    pub max_registers_per_read: u16,
    /// Device description
    pub description: Option<String>,
}

impl Default for ModbusPollingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_interval_ms: 1000,
            connection_timeout_ms: timeouts::DEFAULT_CONNECT_TIMEOUT_MS,
            read_timeout_ms: timeouts::DEFAULT_READ_TIMEOUT_MS,
            max_retries: 3,
            retry_interval_ms: 1000,
            batch_config: ModbusBatchConfig::default(),
            slaves: HashMap::new(),
            reconnect_enabled: default_reconnect_enabled(),
            reconnect_max_consecutive: default_reconnect_retries(),
            reconnect_cooldown_ms: timeouts::RECONNECT_COOLDOWN_MS,
        }
    }
}

impl Default for ModbusBatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_batch_size: 100,
            max_gap: 5,
            device_limits: HashMap::new(),
        }
    }
}

impl Default for SlavePollingConfig {
    fn default() -> Self {
        Self {
            slave_id: 1,
            interval_ms: 1000,
            enabled: true,
            timeout_ms: None,
            description: None,
        }
    }
}

impl Default for DeviceLimit {
    fn default() -> Self {
        Self {
            max_registers_per_read: 100,
            description: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use voltage_config::common::timeouts;

    // ========== ModbusPollingConfig Default tests ==========

    #[test]
    fn test_modbus_polling_config_default_values() {
        let config = ModbusPollingConfig::default();

        assert!(config.enabled);
        assert_eq!(config.default_interval_ms, 1000);
        assert_eq!(
            config.connection_timeout_ms,
            timeouts::DEFAULT_CONNECT_TIMEOUT_MS
        );
        assert_eq!(config.read_timeout_ms, timeouts::DEFAULT_READ_TIMEOUT_MS);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_interval_ms, 1000);
        assert!(config.slaves.is_empty());

        // Reconnection defaults
        assert!(config.reconnect_enabled);
        assert_eq!(config.reconnect_max_consecutive, 5);
        assert_eq!(
            config.reconnect_cooldown_ms,
            timeouts::RECONNECT_COOLDOWN_MS
        );
    }

    #[test]
    fn test_modbus_polling_config_batch_config_included() {
        let config = ModbusPollingConfig::default();

        // Should have default batch config
        assert!(config.batch_config.enabled);
        assert_eq!(config.batch_config.max_batch_size, 100);
    }

    // ========== ModbusBatchConfig Default tests ==========

    #[test]
    fn test_modbus_batch_config_default_values() {
        let config = ModbusBatchConfig::default();

        assert!(config.enabled);
        assert_eq!(config.max_batch_size, 100);
        assert_eq!(config.max_gap, 5);
        assert!(config.device_limits.is_empty());
    }

    // ========== SlavePollingConfig Default tests ==========

    #[test]
    fn test_slave_polling_config_default_values() {
        let config = SlavePollingConfig::default();

        assert_eq!(config.slave_id, 1);
        assert_eq!(config.interval_ms, 1000);
        assert!(config.enabled);
        assert!(config.timeout_ms.is_none());
        assert!(config.description.is_none());
    }

    // ========== DeviceLimit Default tests ==========

    #[test]
    fn test_device_limit_default_values() {
        let limit = DeviceLimit::default();

        assert_eq!(limit.max_registers_per_read, 100);
        assert!(limit.description.is_none());
    }

    // ========== ModbusPoint serialization tests ==========

    #[test]
    fn test_modbus_point_deserialization_minimal() {
        let json = r#"{
            "point_id": "T001",
            "slave_id": 1,
            "function_code": 3,
            "register_address": 100,
            "data_type": "float32",
            "register_count": 2
        }"#;

        let point: ModbusPoint = serde_json::from_str(json).unwrap();

        assert_eq!(point.point_id, "T001");
        assert_eq!(point.slave_id, 1);
        assert_eq!(point.function_code, 3);
        assert_eq!(point.register_address, 100);
        assert_eq!(point.data_type, "float32");
        assert_eq!(point.register_count, 2);

        // Check defaults
        assert!(point.byte_order.is_none());
        assert_eq!(point.bit_position, 0);
        assert_eq!(point.scale, 1.0);
        assert_eq!(point.offset, 0.0);
        assert!(!point.reverse);
    }

    #[test]
    fn test_modbus_point_deserialization_full() {
        let json = r#"{
            "point_id": "T002",
            "slave_id": 2,
            "function_code": 4,
            "register_address": 200,
            "data_type": "uint16",
            "register_count": 1,
            "byte_order": "ABCD",
            "bit_position": 5,
            "scale": 0.1,
            "offset": -10.0,
            "reverse": true
        }"#;

        let point: ModbusPoint = serde_json::from_str(json).unwrap();

        assert_eq!(point.point_id, "T002");
        assert_eq!(point.byte_order, Some("ABCD".to_string()));
        assert_eq!(point.bit_position, 5);
        assert_eq!(point.scale, 0.1);
        assert_eq!(point.offset, -10.0);
        assert!(point.reverse);
    }

    #[test]
    fn test_modbus_point_serialization_roundtrip() {
        let original = ModbusPoint {
            point_id: "T003".to_string(),
            slave_id: 3,
            function_code: 3,
            register_address: 300,
            data_type: "int32".to_string(),
            register_count: 2,
            byte_order: Some("CDAB".to_string()),
            bit_position: 0,
            scale: 2.5,
            offset: 100.0,
            reverse: false,
        };

        let json = serde_json::to_string(&original).unwrap();
        let restored: ModbusPoint = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.point_id, original.point_id);
        assert_eq!(restored.slave_id, original.slave_id);
        assert_eq!(restored.scale, original.scale);
        assert_eq!(restored.byte_order, original.byte_order);
    }

    // ========== ModbusPollingConfig serialization tests ==========

    #[test]
    fn test_polling_config_skip_serializing_defaults() {
        let config = ModbusPollingConfig::default();
        let json = serde_json::to_string(&config).unwrap();

        // Default reconnect values should be skipped (not in JSON)
        assert!(!json.contains("reconnect_enabled"));
        assert!(!json.contains("reconnect_max_consecutive"));
        assert!(!json.contains("reconnect_cooldown_ms"));
    }

    #[test]
    fn test_polling_config_serializes_non_default_reconnect() {
        let config = ModbusPollingConfig {
            reconnect_enabled: false,
            reconnect_max_consecutive: 10,
            reconnect_cooldown_ms: 120000,
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();

        // Non-default values should be serialized
        assert!(json.contains("reconnect_enabled"));
        assert!(json.contains("reconnect_max_consecutive"));
        assert!(json.contains("reconnect_cooldown_ms"));
    }

    // ========== SlavePollingConfig tests ==========

    #[test]
    fn test_slave_config_with_optional_fields() {
        let json = r#"{
            "slave_id": 5,
            "interval_ms": 500,
            "enabled": true,
            "timeout_ms": 2000,
            "description": "Main controller"
        }"#;

        let config: SlavePollingConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.slave_id, 5);
        assert_eq!(config.interval_ms, 500);
        assert!(config.enabled);
        assert_eq!(config.timeout_ms, Some(2000));
        assert_eq!(config.description, Some("Main controller".to_string()));
    }

    // ========== DeviceLimit tests ==========

    #[test]
    fn test_device_limit_with_description() {
        let limit = DeviceLimit {
            max_registers_per_read: 50,
            description: Some("Legacy device".to_string()),
        };

        assert_eq!(limit.max_registers_per_read, 50);
        assert_eq!(limit.description, Some("Legacy device".to_string()));
    }

    // ========== Edge cases ==========

    #[test]
    fn test_modbus_point_bool_data_type() {
        let json = r#"{
            "point_id": "S001",
            "slave_id": 1,
            "function_code": 1,
            "register_address": 0,
            "data_type": "bool",
            "register_count": 1,
            "bit_position": 7,
            "reverse": true
        }"#;

        let point: ModbusPoint = serde_json::from_str(json).unwrap();

        assert_eq!(point.data_type, "bool");
        assert_eq!(point.bit_position, 7);
        assert!(point.reverse);
    }

    #[test]
    fn test_batch_config_with_device_limits() {
        let mut config = ModbusBatchConfig::default();
        config.device_limits.insert(
            1,
            DeviceLimit {
                max_registers_per_read: 25,
                description: Some("Small PLC".to_string()),
            },
        );
        config.device_limits.insert(
            2,
            DeviceLimit {
                max_registers_per_read: 125,
                description: None,
            },
        );

        assert_eq!(config.device_limits.len(), 2);
        assert_eq!(
            config.device_limits.get(&1).unwrap().max_registers_per_read,
            25
        );
        assert_eq!(
            config.device_limits.get(&2).unwrap().max_registers_per_read,
            125
        );
    }
}
