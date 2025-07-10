use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::any::Any;
use voltage_config::prelude::*;

/// Model service configuration using the unified config framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModServiceConfig {
    /// Base service configuration (flattened)
    #[serde(flatten)]
    pub base: BaseServiceConfig,
    
    /// API configuration
    pub api: ApiConfig,
    
    /// Model execution configuration
    pub model: ModelConfig,
    
    /// Control operations configuration
    pub control: ControlConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Enable API
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// API host
    #[serde(default = "default_api_host")]
    pub host: String,
    /// API port
    #[serde(default = "default_api_port")]
    pub port: u16,
}

/// Model execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model execution interval in milliseconds
    #[serde(default = "default_update_interval")]
    pub update_interval_ms: u64,
    /// Redis key pattern for model configurations
    #[serde(default = "default_config_key_pattern")]
    pub config_key_pattern: String,
    /// Redis key pattern for input data
    #[serde(default = "default_data_key_pattern")]
    pub data_key_pattern: String,
    /// Redis key pattern for model outputs
    #[serde(default = "default_output_key_pattern")]
    pub output_key_pattern: String,
    /// Directory containing model templates
    #[serde(default = "default_templates_dir")]
    pub templates_dir: String,
    /// Enable model caching
    #[serde(default = "default_true")]
    pub enable_caching: bool,
    /// Maximum number of concurrent model executions
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_executions: u32,
}

/// Control operations configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlConfig {
    /// Redis key pattern for control operations
    #[serde(default = "default_operation_key_pattern")]
    pub operation_key_pattern: String,
    /// Whether control operations are enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Control execution timeout in seconds
    #[serde(default = "default_control_timeout")]
    pub timeout_secs: u64,
    /// Maximum retries for failed control operations
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage mode
    #[serde(default = "default_storage_mode")]
    pub mode: StorageMode,
    /// Use Redis for storage
    #[serde(default = "default_true")]
    pub use_redis: bool,
    /// Synchronization interval in seconds
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    /// Enable persistence
    #[serde(default = "default_true")]
    pub enable_persistence: bool,
    /// Persistence directory
    #[serde(default = "default_persistence_dir")]
    pub persistence_dir: String,
}

/// Storage mode enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Memory,
    Redis,
}

impl Default for StorageMode {
    fn default() -> Self {
        StorageMode::Redis
    }
}

// Default value functions
fn default_true() -> bool {
    true
}

fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_api_port() -> u16 {
    8093
}

fn default_update_interval() -> u64 {
    1000
}

fn default_config_key_pattern() -> String {
    "voltage:model:config:*".to_string()
}

fn default_data_key_pattern() -> String {
    "voltage:data:*".to_string()
}

fn default_output_key_pattern() -> String {
    "voltage:model:output:*".to_string()
}

fn default_templates_dir() -> String {
    "templates".to_string()
}

fn default_max_concurrent() -> u32 {
    10
}

fn default_operation_key_pattern() -> String {
    "voltage:control:operation:*".to_string()
}

fn default_control_timeout() -> u64 {
    30
}

fn default_max_retries() -> u32 {
    3
}

fn default_storage_mode() -> StorageMode {
    StorageMode::Redis
}

fn default_sync_interval() -> u64 {
    10
}

fn default_persistence_dir() -> String {
    "data/persistence".to_string()
}

impl Configurable for ModServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        // Validate API configuration
        if self.api.port == 0 {
            return Err(ConfigError::Validation("API port cannot be 0".into()));
        }
        
        // Validate model configuration
        if self.model.update_interval_ms == 0 {
            return Err(ConfigError::Validation(
                "Model update interval cannot be 0".into()
            ));
        }
        
        if self.model.max_concurrent_executions == 0 {
            return Err(ConfigError::Validation(
                "Maximum concurrent executions cannot be 0".into()
            ));
        }
        
        // Validate control configuration
        if self.control.enabled && self.control.timeout_secs == 0 {
            return Err(ConfigError::Validation(
                "Control timeout cannot be 0 when control is enabled".into()
            ));
        }
        
        // Validate storage configuration
        if self.storage.sync_interval_secs == 0 {
            return Err(ConfigError::Validation(
                "Storage sync interval cannot be 0".into()
            ));
        }
        
        // Validate storage mode combinations
        match self.storage.mode {
            StorageMode::Redis => {
                if !self.storage.use_redis {
                    return Err(ConfigError::Validation(
                        "Redis storage mode requires use_redis to be true".into()
                    ));
                }
            }
            StorageMode::Memory => {
                if self.storage.use_redis {
                    return Err(ConfigError::Validation(
                        "Memory storage mode should have use_redis set to false".into()
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ServiceConfig for ModServiceConfig {
    fn base(&self) -> &BaseServiceConfig {
        &self.base
    }
    
    fn base_mut(&mut self) -> &mut BaseServiceConfig {
        &mut self.base
    }
}

impl ModServiceConfig {
    /// Load configuration using the unified framework
    pub async fn load() -> Result<Self> {
        let loader = ConfigLoaderBuilder::new()
            .base_path("config")
            .add_file("modsrv.yml")
            .environment(Environment::from_env())
            .env_prefix("MOD")
            .defaults(serde_json::json!({
                "service": {
                    "name": "modsrv",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "Model Calculation Service"
                },
                "redis": {
                    "url": "redis://localhost:6379",
                    "prefix": "voltage:mod:",
                    "pool_size": 30
                },
                "logging": {
                    "level": "info",
                    "console": true,
                    "file": {
                        "path": "logs/modsrv.log",
                        "rotation": "daily",
                        "max_size": "100MB",
                        "max_files": 7
                    }
                },
                "monitoring": {
                    "metrics_enabled": true,
                    "metrics_port": 9093,
                    "health_check_enabled": true,
                    "health_check_port": 8094
                },
                "api": {
                    "enabled": true,
                    "host": "0.0.0.0",
                    "port": 8093
                },
                "model": {
                    "update_interval_ms": 1000,
                    "config_key_pattern": "voltage:model:config:*",
                    "data_key_pattern": "voltage:data:*",
                    "output_key_pattern": "voltage:model:output:*",
                    "templates_dir": "templates",
                    "enable_caching": true,
                    "max_concurrent_executions": 10
                },
                "control": {
                    "operation_key_pattern": "voltage:control:operation:*",
                    "enabled": true,
                    "timeout_secs": 30,
                    "max_retries": 3
                },
                "storage": {
                    "mode": "redis",
                    "use_redis": true,
                    "sync_interval_secs": 10,
                    "enable_persistence": true,
                    "persistence_dir": "data/persistence"
                }
            }))?
            .build()?;
        
        let config: ModServiceConfig = loader.load_async().await
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;
        
        // Validate complete configuration
        config.validate_all()
            .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;
        
        Ok(config)
    }
    
    /// Generate default configuration file
    pub fn generate_default_config() -> String {
        let config = ModServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "modsrv".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Model Calculation Service".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "voltage:mod:".to_string(),
                    pool_size: 30,
                    database: 0,
                    password: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: Some(voltage_config::base::LogFileConfig {
                        path: "logs/modsrv.log".to_string(),
                        rotation: "daily".to_string(),
                        max_size: "100MB".to_string(),
                        max_files: 7,
                    }),
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9093,
                    health_check_enabled: true,
                    health_check_port: 8094,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8093,
            },
            model: ModelConfig {
                update_interval_ms: 1000,
                config_key_pattern: "voltage:model:config:*".to_string(),
                data_key_pattern: "voltage:data:*".to_string(),
                output_key_pattern: "voltage:model:output:*".to_string(),
                templates_dir: "templates".to_string(),
                enable_caching: true,
                max_concurrent_executions: 10,
            },
            control: ControlConfig {
                operation_key_pattern: "voltage:control:operation:*".to_string(),
                enabled: true,
                timeout_secs: 30,
                max_retries: 3,
            },
            storage: StorageConfig {
                mode: StorageMode::Redis,
                use_redis: true,
                sync_interval_secs: 10,
                enable_persistence: true,
                persistence_dir: "data/persistence".to_string(),
            },
        };
        
        serde_yaml::to_string(&config).unwrap_or_else(|_| "# Failed to generate config file".to_string())
    }
    
    /// Migrate from old configuration format
    pub fn from_old_config(old_config: super::config::Config) -> Self {
        ModServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "modsrv".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Model Calculation Service".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: format!("redis://{}:{}", old_config.redis.host, old_config.redis.port),
                    prefix: old_config.redis.key_prefix,
                    pool_size: 30,
                    database: old_config.redis.database as u32,
                    password: if old_config.redis.password.is_empty() { 
                        None 
                    } else { 
                        Some(old_config.redis.password) 
                    },
                },
                logging: LoggingConfig {
                    level: old_config.logging.level,
                    console: old_config.logging.console,
                    file: if old_config.logging.file.is_empty() {
                        None
                    } else {
                        Some(voltage_config::base::LogFileConfig {
                            path: old_config.logging.file,
                            rotation: "daily".to_string(),
                            max_size: "100MB".to_string(),
                            max_files: 10,
                        })
                    },
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: old_config.monitoring.enabled,
                    metrics_port: 9093,
                    health_check_enabled: true,
                    health_check_port: 8094,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                enabled: true,
                host: old_config.api.host,
                port: old_config.api.port,
            },
            model: ModelConfig {
                update_interval_ms: old_config.model.update_interval_ms,
                config_key_pattern: old_config.model.config_key_pattern,
                data_key_pattern: old_config.model.data_key_pattern,
                output_key_pattern: old_config.model.output_key_pattern,
                templates_dir: old_config.model.templates_dir,
                enable_caching: true,
                max_concurrent_executions: 10,
            },
            control: ControlConfig {
                operation_key_pattern: old_config.control.operation_key_pattern,
                enabled: old_config.control.enabled,
                timeout_secs: 30,
                max_retries: 3,
            },
            storage: StorageConfig {
                mode: match old_config.storage_mode.as_str() {
                    "memory" => StorageMode::Memory,
                    "redis" => StorageMode::Redis,
                    _ => StorageMode::Redis,
                },
                use_redis: old_config.use_redis,
                sync_interval_secs: old_config.sync_interval_secs,
                enable_persistence: true,
                persistence_dir: "data/persistence".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_validation() {
        let mut config = ModServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "modsrv".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Test".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "test:".to_string(),
                    pool_size: 10,
                    database: 0,
                    password: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: None,
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9090,
                    health_check_enabled: true,
                    health_check_port: 8080,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8093,
            },
            model: ModelConfig {
                update_interval_ms: 1000,
                config_key_pattern: "model:*".to_string(),
                data_key_pattern: "data:*".to_string(),
                output_key_pattern: "output:*".to_string(),
                templates_dir: "templates".to_string(),
                enable_caching: true,
                max_concurrent_executions: 10,
            },
            control: ControlConfig {
                operation_key_pattern: "control:*".to_string(),
                enabled: true,
                timeout_secs: 30,
                max_retries: 3,
            },
            storage: StorageConfig {
                mode: StorageMode::Redis,
                use_redis: true,
                sync_interval_secs: 10,
                enable_persistence: true,
                persistence_dir: "data".to_string(),
            },
        };
        
        // Valid configuration should pass
        assert!(config.validate_all().is_ok());
        
        // Invalid update interval should fail
        config.model.update_interval_ms = 0;
        assert!(config.validate_all().is_err());
        config.model.update_interval_ms = 1000;
        
        // Storage mode mismatch should fail
        config.storage.mode = StorageMode::Redis;
        config.storage.use_redis = false;
        assert!(config.validate_all().is_err());
    }
    
    #[test]
    fn test_storage_mode_validation() {
        let mut config = ModServiceConfig {
            base: Default::default(),
            api: Default::default(),
            model: ModelConfig {
                update_interval_ms: 1000,
                config_key_pattern: "model:*".to_string(),
                data_key_pattern: "data:*".to_string(),
                output_key_pattern: "output:*".to_string(),
                templates_dir: "templates".to_string(),
                enable_caching: true,
                max_concurrent_executions: 10,
            },
            control: ControlConfig {
                operation_key_pattern: "control:*".to_string(),
                enabled: true,
                timeout_secs: 30,
                max_retries: 3,
            },
            storage: StorageConfig {
                mode: StorageMode::Memory,
                use_redis: false,
                sync_interval_secs: 10,
                enable_persistence: true,
                persistence_dir: "data".to_string(),
            },
        };
        
        // Memory mode with use_redis=false should be valid
        assert!(config.validate().is_ok());
        
        // Memory mode with use_redis=true should be invalid
        config.storage.use_redis = true;
        assert!(config.validate().is_err());
        
        // Redis mode requires use_redis to be true
        config.storage.mode = StorageMode::Redis;
        config.storage.use_redis = true;
        assert!(config.validate().is_ok());
    }
}