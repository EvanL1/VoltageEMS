//! Application configuration types

use super::{ChannelConfig, LoggingConfig, RedisConfig};
use serde::{Deserialize, Serialize};

/// Application configuration using Figment (matching figment_demo structure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration version
    #[serde(default = "default_version")]
    pub version: String,

    /// Service configuration
    #[serde(default)]
    pub service: ServiceConfig,

    /// Communication channels
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,

    /// Default path configuration
    #[serde(default)]
    pub defaults: DefaultPathConfig,
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    #[serde(default = "default_service_name")]
    pub name: String,

    /// Service description
    pub description: Option<String>,

    /// API configuration
    #[serde(default)]
    pub api: ApiConfig,

    /// Redis configuration
    #[serde(default)]
    pub redis: RedisConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Whether API is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Bind address
    #[serde(default = "default_api_bind")]
    pub bind_address: String,

    /// API version
    #[serde(default = "default_api_version")]
    pub version: String,
}

/// Default path configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultPathConfig {
    /// Channels root directory
    #[serde(default = "default_channels_root")]
    pub channels_root: String,

    /// ComBase directory name
    #[serde(default = "default_combase_dir")]
    pub combase_dir: String,

    /// Protocol directory name
    #[serde(default = "default_protocol_dir")]
    pub protocol_dir: String,
}

// Default value functions
fn default_version() -> String {
    "1.0".to_string()
}

fn default_service_name() -> String {
    "comsrv".to_string()
}

fn default_true() -> bool {
    true
}

fn default_api_bind() -> String {
    "127.0.0.1:3000".to_string()
}

fn default_api_version() -> String {
    "v1".to_string()
}

fn default_channels_root() -> String {
    "channels".to_string()
}

fn default_combase_dir() -> String {
    "combase".to_string()
}

fn default_protocol_dir() -> String {
    "protocol".to_string()
}

// Default implementations
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: "2.0".to_string(),
            service: ServiceConfig::default(),
            channels: Vec::new(),
            defaults: DefaultPathConfig::default(),
        }
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: default_service_name(),
            description: Some("Communication Service".to_string()),
            api: ApiConfig::default(),
            redis: RedisConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            bind_address: default_api_bind(),
            version: default_api_version(),
        }
    }
}

impl Default for DefaultPathConfig {
    fn default() -> Self {
        Self {
            channels_root: default_channels_root(),
            combase_dir: default_combase_dir(),
            protocol_dir: default_protocol_dir(),
        }
    }
}
