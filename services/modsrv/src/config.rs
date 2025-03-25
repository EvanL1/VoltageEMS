use crate::error::Result;
use config::{Config as ConfigLib, ConfigError, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use crate::storage::SyncMode;
use crate::error::{ModelSrvError};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub database: u8,
    pub key_prefix: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub file: String,
    pub console: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelConfig {
    pub update_interval_ms: u64,
    pub config_key_pattern: String,
    pub data_key_pattern: String,
    pub output_key_pattern: String,
    #[serde(default = "default_templates_dir")]
    pub templates_dir: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiConfig {
    /// API server host
    #[serde(default = "default_api_host")]
    pub host: String,
    
    /// API server port
    #[serde(default = "default_api_port")]
    pub port: u16,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_api_port(),
        }
    }
}

fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_api_port() -> u16 {
    8000
}

fn default_templates_dir() -> String {
    "templates".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ControlConfig {
    pub operation_key_pattern: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub notification_threshold_ms: Option<u128>,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            notification_threshold_ms: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub redis: RedisConfig,
    pub logging: LoggingConfig,
    pub model: ModelConfig,
    pub control: ControlConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    pub templates_dir: String,
    pub log_level: String,
    
    // 存储相关配置
    #[serde(default = "default_use_redis")]
    pub use_redis: bool,
    
    #[serde(default = "default_storage_mode")]
    pub storage_mode: String,
    
    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,
}

fn default_use_redis() -> bool {
    true
}

fn default_storage_mode() -> String {
    "hybrid".to_string()
}

fn default_sync_interval_secs() -> u64 {
    60
}

impl Config {
    pub fn new(config_file: &str) -> Result<Self> {
        let config_path = Path::new(config_file);
        if !config_path.exists() {
            return Err(ConfigError::NotFound(config_file.to_string()).into());
        }

        // Determine format based on file extension
        let format = match config_path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => FileFormat::Yaml,
            Some("toml") => FileFormat::Toml,
            _ => FileFormat::Yaml, // Default to YAML
        };

        let config = ConfigLib::builder()
            .add_source(File::with_name(config_file).format(format))
            .build()?;

        let mut config: Config = config.try_deserialize()?;
        config.templates_dir = config.model.templates_dir.clone();
        Ok(config)
    }
    
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        
        // Determine format based on file extension
        let format = match path_ref.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => FileFormat::Yaml,
            Some("toml") => FileFormat::Toml,
            _ => FileFormat::Yaml, // Default to YAML
        };
        
        let config = ConfigLib::builder()
            .add_source(File::with_name(path_ref.to_str().unwrap()).format(format))
            .build()
            .map_err(|e| ModelSrvError::ConfigError(e.to_string()))?;
            
        config.try_deserialize()
            .map_err(|e| ModelSrvError::ConfigError(e.to_string()))
    }

    pub fn default() -> Self {
        Config {
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                database: 0,
                key_prefix: "ems:".to_string(),
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
                templates_dir: "/opt/voltageems/modsrv/templates".to_string(),
            },
            control: ControlConfig {
                operation_key_pattern: "ems:control:operation:*".to_string(),
                enabled: true,
            },
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                port: 8000,
            },
            monitoring: MonitoringConfig {
                enabled: false,
                notification_threshold_ms: None,
            },
            templates_dir: "/opt/voltageems/modsrv/templates".to_string(),
            log_level: "info".to_string(),
            use_redis: true,
            storage_mode: "hybrid".to_string(),
            sync_interval_secs: 60,
        }
    }

    pub fn get_sync_mode(&self) -> SyncMode {
        match self.storage_mode.as_str() {
            "write_through" => SyncMode::WriteThrough,
            "write_back" => SyncMode::WriteBack(Duration::from_secs(self.sync_interval_secs)),
            "on_demand" => SyncMode::OnDemand,
            _ => SyncMode::WriteThrough, // Default to use WriteThrough
        }
    }
} 