use crate::error::Result;
use config::{Config as ConfigLib, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub socket: String,
    pub prefix: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub file: String,
    pub console: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelConfig {
    pub update_interval_ms: u64,
    pub config_key_pattern: String,
    pub data_key_pattern: String,
    pub output_key_pattern: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControlConfig {
    pub operation_key_pattern: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub redis: RedisConfig,
    pub logging: LoggingConfig,
    pub model: ModelConfig,
    pub control: ControlConfig,
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
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                password: "".to_string(),
                socket: "".to_string(),
                prefix: "ems:".to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: "/var/log/ems/modelsrv.log".to_string(),
                console: true,
            },
            model: ModelConfig {
                update_interval_ms: 1000,
                config_key_pattern: "ems:model:config:*".to_string(),
                data_key_pattern: "ems:data:*".to_string(),
                output_key_pattern: "ems:model:output:*".to_string(),
            },
            control: ControlConfig {
                operation_key_pattern: "ems:control:operation:*".to_string(),
                enabled: true,
            },
        }
    }
} 