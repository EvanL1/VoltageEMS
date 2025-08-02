use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use voltage_libs::config::utils::{get_global_log_level, get_global_redis_url};
use voltage_libs::config::ConfigLoader;

/// Network service configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Service configuration
    pub service: ServiceConfig,

    /// Redis configuration
    pub redis: RedisConfig,

    /// Network configurations
    #[serde(default)]
    pub networks: Vec<NetworkConfig>,

    /// Data processing configuration
    pub data: DataConfig,
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    #[serde(default = "default_service_name")]
    pub name: String,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Log file path
    pub log_file: Option<PathBuf>,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
}

/// Data processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    /// Redis data key pattern
    #[serde(default = "default_data_key_pattern")]
    pub redis_data_key: String,

    /// Redis polling interval in seconds
    #[serde(default = "default_polling_interval")]
    pub redis_polling_interval_secs: u64,

    /// Enable data buffering
    #[serde(default = "default_true")]
    pub enable_buffering: bool,

    /// Buffer size
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkConfig {
    /// MQTT configuration
    Mqtt(MqttConfig),

    /// HTTP configuration
    Http(HttpConfig),
}

/// MQTT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    /// Network name
    pub name: String,

    /// Broker address
    pub broker: String,

    /// Client ID
    pub client_id: String,

    /// Username
    pub username: Option<String>,

    /// Password
    pub password: Option<String>,

    /// Topic prefix
    pub topic_prefix: String,

    /// Data format
    #[serde(default)]
    pub format_type: FormatType,

    /// QoS level
    #[serde(default = "default_qos")]
    pub qos: u8,
}

/// HTTP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Network name
    pub name: String,

    /// Base URL
    pub url: String,

    /// HTTP method
    #[serde(default = "default_http_method")]
    pub method: String,

    /// Headers
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

/// Data format type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormatType {
    #[default]
    Json,
    Ascii,
    Binary,
}

impl Config {
    /// Load configuration from file and environment
    pub fn load() -> Result<Config> {
        // 尝试多个配置文件路径
        let config_paths = [
            "config/netsrv/netsrv.yaml",
            "config/netsrv.yaml",
            "netsrv.yaml",
        ];

        let mut yaml_path = None;
        for path in &config_paths {
            if Path::new(path).exists() {
                yaml_path = Some(path.to_string());
                break;
            }
        }

        // 使用新的 ConfigLoader
        let loader = ConfigLoader::new()
            .with_defaults(Config::default())
            .with_env_prefix("NETSRV");

        let config = if let Some(path) = yaml_path {
            loader.with_yaml_file(&path).build()
        } else {
            loader.build()
        }?;

        Ok(config)
    }
}

/// Load configuration from file and environment (backward compatibility)
pub fn load_config() -> Result<Config> {
    Config::load()
}

// Default functions
fn default_service_name() -> String {
    "netsrv".to_string()
}

fn default_log_level() -> String {
    get_global_log_level("NETSRV")
}

fn default_redis_url() -> String {
    get_global_redis_url("NETSRV")
}

fn default_pool_size() -> u32 {
    10
}

fn default_data_key_pattern() -> String {
    "comsrv:*:T".to_string() // 使用大写T表示遥测
}

fn default_polling_interval() -> u64 {
    5
}

fn default_true() -> bool {
    true
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: default_service_name(),
            log_level: default_log_level(),
            log_file: None,
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            pool_size: default_pool_size(),
        }
    }
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            redis_data_key: default_data_key_pattern(),
            redis_polling_interval_secs: default_polling_interval(),
            enable_buffering: default_true(),
            buffer_size: default_buffer_size(),
        }
    }
}

fn default_buffer_size() -> usize {
    1000
}

fn default_qos() -> u8 {
    1
}

fn default_http_method() -> String {
    "POST".to_string()
}

fn default_timeout() -> u64 {
    30
}
