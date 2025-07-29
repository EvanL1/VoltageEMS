use anyhow::Result;
use figment::{
    providers::{Env, Format, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Network service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatType {
    Json,
    Ascii,
    Binary,
}

impl Default for FormatType {
    fn default() -> Self {
        FormatType::Json
    }
}

/// Load configuration from file and environment
pub fn load_config() -> Result<Config> {
    let figment = Figment::new()
        .merge(Yaml::file("config/netsrv.yml"))
        .merge(Yaml::file("config/netsrv.yaml"))
        .merge(Env::prefixed("NETSRV_").split("_"));

    let config: Config = figment.extract()?;
    Ok(config)
}

// Default functions
fn default_service_name() -> String {
    "netsrv".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_redis_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_pool_size() -> u32 {
    10
}

fn default_data_key_pattern() -> String {
    "comsrv:*:m".to_string()
}

fn default_polling_interval() -> u64 {
    5
}

fn default_true() -> bool {
    true
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
