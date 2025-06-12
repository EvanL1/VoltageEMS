pub mod redis_config;
pub mod network_config;
pub mod cloud_config;

use crate::error::Result;
use config::{Config as ConfigLib, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

use self::redis_config::RedisConfig;
use self::network_config::NetworkConfig;
use self::cloud_config::CloudMqttConfig;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub file: String,
    pub console: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub redis: RedisConfig,
    pub logging: LoggingConfig,
    pub networks: Vec<NetworkConfig>,
    pub cloud_networks: Option<Vec<CloudMqttConfig>>,
}

impl Config {
    pub fn new(config_file: &str) -> Result<Self> {
        let config_path = Path::new(config_file);
        if !config_path.exists() {
            return Err(ConfigError::NotFound(config_file.to_string()).into());
        }

        let config = ConfigLib::builder()
            .add_source(File::with_name(config_file))
            .build()?;

        let config: Config = config.try_deserialize()?;
        Ok(config)
    }

    pub fn default() -> Self {
        Config {
            redis: RedisConfig::default(),
            logging: LoggingConfig {
                level: "info".to_string(),
                file: "/var/log/ems/netsrv.log".to_string(),
                console: true,
            },
            networks: vec![
                NetworkConfig::default_mqtt(),
                NetworkConfig::default_http(),
            ],
            cloud_networks: Some(vec![
                CloudMqttConfig::aws_iot_template(),
                CloudMqttConfig::aliyun_iot_template(),
                CloudMqttConfig::azure_iot_template(),
                CloudMqttConfig::tencent_iot_template(),
            ]),
        }
    }
} 