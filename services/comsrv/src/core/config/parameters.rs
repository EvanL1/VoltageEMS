//! parameter相offtypedefinition
//!
//! 简化的parameterprocessing，移除了complex的 `ChannelParameters` enum

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// protocol特定parameter（仅用于强typevalidation）
// ============================================================================

/// Modbusprotocolparameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusParameters {
    pub host: String,
    pub port: u16,
    pub timeout_ms: u64,
    pub max_retries: u32,
    #[serde(default)]
    pub polling: ModbusPollingConfig,
}

/// Modbus轮询configuring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusPollingConfig {
    #[serde(default = "default_polling_interval")]
    pub default_interval_ms: u64,
    #[serde(default = "default_batch_reading")]
    pub enable_batch_reading: bool,
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: u16,
    #[serde(default = "default_read_timeout")]
    pub read_timeout_ms: u64,
    #[serde(default)]
    pub slave_configs: HashMap<u8, SlavePollingConfig>,
}

/// slave站轮询configuring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlavePollingConfig {
    pub interval_ms: Option<u64>,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_requests: usize,
    #[serde(default = "default_retry_count")]
    pub retry_count: u8,
}

/// CANprotocolparameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanParameters {
    pub interface: String,
    pub bitrate: u32,
    pub timeout_ms: Option<u64>,
}

/// IEC60870protocolparameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iec60870Parameters {
    pub host: String,
    pub port: u16,
    pub timeout_ms: Option<u64>,
    pub common_address: u16,
    pub link_address: u16,
}

// ============================================================================
// parameterconverting辅助function
// ============================================================================

/// `slave通用HashMapconverting为Modbusparameter`
pub fn parse_modbus_parameters(
    params: &HashMap<String, serde_yaml::Value>,
) -> Result<ModbusParameters, String> {
    let host = params
        .get("host")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'host' parameter")?
        .to_string();

    let port = params
        .get("port")
        .and_then(serde_yaml::Value::as_u64)
        .ok_or("Missing 'port' parameter")? as u16;

    let timeout_ms = params
        .get("timeout_ms")
        .and_then(serde_yaml::Value::as_u64)
        .unwrap_or(5000);

    let max_retries = params
        .get("max_retries")
        .and_then(serde_yaml::Value::as_u64)
        .unwrap_or(3) as u32;

    // parse轮询configuring
    let polling = if let Some(polling_value) = params.get("polling") {
        serde_yaml::from_value(polling_value.clone())
            .map_err(|e| format!("Failed to parse polling config: {e}"))?
    } else {
        ModbusPollingConfig::default()
    };

    Ok(ModbusParameters {
        host,
        port,
        timeout_ms,
        max_retries,
        polling,
    })
}

/// `slave通用HashMapconverting为CANparameter`
pub fn parse_can_parameters(
    params: &HashMap<String, serde_yaml::Value>,
) -> Result<CanParameters, String> {
    let interface = params
        .get("interface")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'interface' parameter")?
        .to_string();

    let bitrate = params
        .get("bitrate")
        .and_then(serde_yaml::Value::as_u64)
        .ok_or("Missing 'bitrate' parameter")? as u32;

    let timeout_ms = params.get("timeout_ms").and_then(serde_yaml::Value::as_u64);

    Ok(CanParameters {
        interface,
        bitrate,
        timeout_ms,
    })
}

/// `slave通用HashMapconverting为IEC60870parameter`
pub fn parse_iec60870_parameters(
    params: &HashMap<String, serde_yaml::Value>,
) -> Result<Iec60870Parameters, String> {
    let host = params
        .get("host")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'host' parameter")?
        .to_string();

    let port = params
        .get("port")
        .and_then(serde_yaml::Value::as_u64)
        .ok_or("Missing 'port' parameter")? as u16;

    let timeout_ms = params.get("timeout_ms").and_then(serde_yaml::Value::as_u64);

    let common_address = params
        .get("common_address")
        .and_then(serde_yaml::Value::as_u64)
        .unwrap_or(1) as u16;

    let link_address = params
        .get("link_address")
        .and_then(serde_yaml::Value::as_u64)
        .unwrap_or(1) as u16;

    Ok(Iec60870Parameters {
        host,
        port,
        timeout_ms,
        common_address,
        link_address,
    })
}

// ============================================================================
// defaultvaluefunction
// ============================================================================

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modbus_parameters() {
        let mut params = HashMap::new();
        params.insert(
            "host".to_string(),
            serde_yaml::Value::String("192.168.1.1".to_string()),
        );
        params.insert(
            "port".to_string(),
            serde_yaml::Value::Number(serde_yaml::Number::from(502)),
        );

        let result = parse_modbus_parameters(&params).unwrap();
        assert_eq!(result.host, "192.168.1.1");
        assert_eq!(result.port, 502);
        assert_eq!(result.timeout_ms, 5000);
        assert_eq!(result.max_retries, 3);
    }

    #[test]
    fn test_parse_can_parameters() {
        let mut params = HashMap::new();
        params.insert(
            "interface".to_string(),
            serde_yaml::Value::String("can0".to_string()),
        );
        params.insert(
            "bitrate".to_string(),
            serde_yaml::Value::Number(serde_yaml::Number::from(500_000)),
        );

        let result = parse_can_parameters(&params).unwrap();
        assert_eq!(result.interface, "can0");
        assert_eq!(result.bitrate, 500_000);
        assert!(result.timeout_ms.is_none());
    }

    #[test]
    fn test_modbus_polling_defaults() {
        let config = ModbusPollingConfig::default();
        assert_eq!(config.default_interval_ms, 1000);
        assert!(config.enable_batch_reading);
        assert_eq!(config.max_batch_size, 125);
        assert_eq!(config.read_timeout_ms, 5000);
        assert!(config.slave_configs.is_empty());
    }
}
