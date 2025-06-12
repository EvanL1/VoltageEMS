use serde::{Deserialize, Serialize};

pub mod redis_config;
pub mod network;

use redis_config::RedisConfig;
use network::NetworkConfig;

/// Logging configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub console: bool,
    pub file: Option<String>,
}

/// Main application configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub redis: RedisConfig,
    pub networks: Vec<NetworkConfig>,
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            redis: RedisConfig::default(),
            networks: vec![
                NetworkConfig::default_mqtt(),
                NetworkConfig::default_http(),
            ],
            logging: LoggingConfig {
                level: "info".to_string(),
                console: true,
                file: None,
            },
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn new(config_path: &str) -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(config_path))
            .build()?;

        settings.try_deserialize()
    }


} 