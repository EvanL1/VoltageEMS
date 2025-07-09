//! Channel parameters types
//!
//! Provides compatibility with existing protocol code

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Channel parameters enum for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelParameters {
    /// Generic parameters as a HashMap
    Generic(HashMap<String, serde_yaml::Value>),

    /// Modbus-specific parameters
    #[allow(dead_code)]
    Modbus(ModbusParameters),

    /// CAN-specific parameters  
    #[allow(dead_code)]
    Can(CanParameters),

    /// IEC 60870-5-104 specific parameters
    #[allow(dead_code)]
    Iec104(Iec104Parameters),
}

/// Modbus protocol parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusParameters {
    pub host: String,
    pub port: u16,
    pub timeout_ms: u64,
    pub max_retries: u32,
    /// Polling configuration (Modbus-specific feature)
    #[serde(default)]
    pub polling: ModbusPollingConfig,
}

/// Modbus-specific polling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusPollingConfig {
    /// Default polling interval in milliseconds
    #[serde(default = "default_polling_interval")]
    pub default_interval_ms: u64,
    /// Enable batch reading optimization
    #[serde(default = "default_batch_reading")]
    pub enable_batch_reading: bool,
    /// Maximum registers per batch read
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: u16,
    /// Timeout for each read operation
    #[serde(default = "default_read_timeout")]
    pub read_timeout_ms: u64,
    /// Slave-specific configurations
    #[serde(default)]
    pub slave_configs: HashMap<u8, SlavePollingConfig>,
}

/// Per-slave polling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlavePollingConfig {
    /// Polling interval for this slave (overrides default)
    pub interval_ms: Option<u64>,
    /// Maximum concurrent requests
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_requests: usize,
    /// Retry count on failure
    #[serde(default = "default_retry_count")]
    pub retry_count: u8,
}

// Default value functions
fn default_polling_interval() -> u64 {
    1000
}
fn default_batch_reading() -> bool {
    true
}
fn default_max_batch_size() -> u16 {
    125
}
fn default_read_timeout() -> u64 {
    5000
}
fn default_max_concurrent() -> usize {
    1
}
fn default_retry_count() -> u8 {
    3
}

impl Default for ModbusPollingConfig {
    fn default() -> Self {
        Self {
            default_interval_ms: default_polling_interval(),
            enable_batch_reading: default_batch_reading(),
            max_batch_size: default_max_batch_size(),
            read_timeout_ms: default_read_timeout(),
            slave_configs: HashMap::new(),
        }
    }
}

/// CAN protocol parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanParameters {
    pub interface: String,
    pub bitrate: u32,
    pub timeout_ms: Option<u64>,
}

/// IEC 60870-5-104 protocol parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iec104Parameters {
    pub host: String,
    pub port: u16,
    pub timeout_ms: Option<u64>,
}

impl Default for ChannelParameters {
    fn default() -> Self {
        ChannelParameters::Generic(HashMap::new())
    }
}

impl From<HashMap<String, serde_yaml::Value>> for ChannelParameters {
    fn from(map: HashMap<String, serde_yaml::Value>) -> Self {
        ChannelParameters::Generic(map)
    }
}

impl ChannelParameters {
    /// Get as generic map
    pub fn as_generic(&self) -> Option<&HashMap<String, serde_yaml::Value>> {
        match self {
            ChannelParameters::Generic(map) => Some(map),
            _ => None,
        }
    }

    /// Get a parameter value by key
    pub fn get(&self, key: &str) -> Option<&serde_yaml::Value> {
        match self {
            ChannelParameters::Generic(map) => map.get(key),
            _ => None,
        }
    }
}
