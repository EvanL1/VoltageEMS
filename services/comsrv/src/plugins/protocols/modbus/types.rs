//! Modbus protocol data types and configuration
//!
//! Contains simplified Modbus point definitions, polling configuration and batch processing configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simplified Modbus point mapping
/// Contains only protocol-related fields
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
    /// Bit position for bool types (0-15 for all types)
    pub bit_position: Option<u8>,
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
    5 // After 5 consecutive failures, wait 1 minute before trying again
}
fn default_reconnect_cooldown_ms() -> u64 {
    60000 // 1 minute cooldown after max consecutive failures
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
            connection_timeout_ms: 5000,
            read_timeout_ms: 3000,
            max_retries: 3,
            retry_interval_ms: 1000,
            batch_config: ModbusBatchConfig::default(),
            slaves: HashMap::new(),
            reconnect_enabled: default_reconnect_enabled(),
            reconnect_max_consecutive: default_reconnect_retries(),
            reconnect_cooldown_ms: default_reconnect_cooldown_ms(),
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
