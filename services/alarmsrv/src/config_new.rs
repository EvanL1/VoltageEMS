use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::any::Any;
use voltage_config::prelude::*;

/// Alarm service configuration using the unified config framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmServiceConfig {
    /// Base service configuration (flattened)
    #[serde(flatten)]
    pub base: BaseServiceConfig,
    
    /// API server configuration
    pub api: ApiConfig,
    
    /// Alarm-specific configuration
    pub alarm: AlarmConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Listen address
    #[serde(default = "default_api_host")]
    pub host: String,
    /// Listen port
    #[serde(default = "default_api_port")]
    pub port: u16,
    /// Worker threads
    #[serde(default = "default_workers")]
    pub workers: u16,
}

/// Alarm-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmConfig {
    /// Classification thresholds
    pub classification: ClassificationConfig,
    
    /// Processing configuration
    pub processing: ProcessingConfig,
    
    /// Notification configuration
    pub notification: NotificationConfig,
}

/// Classification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationConfig {
    /// Critical alarm threshold
    #[serde(default = "default_critical_threshold")]
    pub critical_threshold: f64,
    /// Warning alarm threshold
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold: f64,
    /// Info alarm threshold
    #[serde(default = "default_info_threshold")]
    pub info_threshold: f64,
}

/// Processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Enable deduplication
    #[serde(default = "default_true")]
    pub enable_deduplication: bool,
    /// Deduplication window in seconds
    #[serde(default = "default_dedup_window")]
    pub deduplication_window_secs: u64,
    /// Maximum alarms per device
    #[serde(default = "default_max_alarms")]
    pub max_alarms_per_device: u32,
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Enable notifications
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Notification channels
    #[serde(default)]
    pub channels: Vec<String>,
    /// Rate limit per hour
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_hour: u32,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Retention period for resolved alarms (in days)
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// Enable automatic cleanup
    #[serde(default = "default_true")]
    pub auto_cleanup: bool,
    /// Cleanup interval (in hours)
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_hours: u32,
    /// Batch size for operations
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
}

// Default value functions
fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_api_port() -> u16 {
    8094
}

fn default_workers() -> u16 {
    4
}

fn default_critical_threshold() -> f64 {
    0.9
}

fn default_warning_threshold() -> f64 {
    0.7
}

fn default_info_threshold() -> f64 {
    0.5
}

fn default_true() -> bool {
    true
}

fn default_dedup_window() -> u64 {
    300 // 5 minutes
}

fn default_max_alarms() -> u32 {
    1000
}

fn default_rate_limit() -> u32 {
    100
}

fn default_retention_days() -> u32 {
    30
}

fn default_cleanup_interval() -> u32 {
    24
}

fn default_batch_size() -> u32 {
    100
}

impl Configurable for AlarmServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        // Validate API configuration
        if self.api.port == 0 {
            return Err(ConfigError::Validation("API port cannot be 0".into()));
        }
        
        // Validate classification thresholds
        if self.alarm.classification.critical_threshold <= self.alarm.classification.warning_threshold {
            return Err(ConfigError::Validation(
                "Critical threshold must be higher than warning threshold".into()
            ));
        }
        
        if self.alarm.classification.warning_threshold <= self.alarm.classification.info_threshold {
            return Err(ConfigError::Validation(
                "Warning threshold must be higher than info threshold".into()
            ));
        }
        
        // Validate storage configuration
        if self.storage.retention_days == 0 {
            return Err(ConfigError::Validation(
                "Retention days must be greater than 0".into()
            ));
        }
        
        if self.storage.batch_size == 0 {
            return Err(ConfigError::Validation(
                "Batch size must be greater than 0".into()
            ));
        }
        
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ServiceConfig for AlarmServiceConfig {
    fn base(&self) -> &BaseServiceConfig {
        &self.base
    }
    
    fn base_mut(&mut self) -> &mut BaseServiceConfig {
        &mut self.base
    }
}

impl AlarmServiceConfig {
    /// Load configuration using the unified framework
    pub async fn load() -> Result<Self> {
        let loader = ConfigLoaderBuilder::new()
            .base_path("config")
            .add_file("alarmsrv.yml")
            .environment(Environment::from_env())
            .env_prefix("ALARM")
            .defaults(serde_json::json!({
                "service": {
                    "name": "alarmsrv",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "Intelligent Alarm Management Service"
                },
                "redis": {
                    "url": "redis://localhost:6379",
                    "prefix": "voltage:alarm:",
                    "pool_size": 20
                },
                "logging": {
                    "level": "info",
                    "console": true
                },
                "monitoring": {
                    "metrics_enabled": true,
                    "metrics_port": 9094,
                    "health_check_enabled": true,
                    "health_check_port": 8095
                },
                "api": {
                    "host": "0.0.0.0",
                    "port": 8094,
                    "workers": 4
                },
                "alarm": {
                    "classification": {
                        "critical_threshold": 0.9,
                        "warning_threshold": 0.7,
                        "info_threshold": 0.5
                    },
                    "processing": {
                        "enable_deduplication": true,
                        "deduplication_window_secs": 300,
                        "max_alarms_per_device": 1000
                    },
                    "notification": {
                        "enabled": true,
                        "channels": ["email", "sms", "webhook"],
                        "rate_limit_per_hour": 100
                    }
                },
                "storage": {
                    "retention_days": 30,
                    "auto_cleanup": true,
                    "cleanup_interval_hours": 24,
                    "batch_size": 100
                }
            }))?
            .build()?;
        
        let config: AlarmServiceConfig = loader.load_async().await
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;
        
        // Validate complete configuration
        config.validate_all()
            .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;
        
        Ok(config)
    }
    
    /// Generate default configuration file
    pub fn generate_default_config() -> String {
        let config = AlarmServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "alarmsrv".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Intelligent Alarm Management Service".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "voltage:alarm:".to_string(),
                    pool_size: 20,
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
                    metrics_port: 9094,
                    health_check_enabled: true,
                    health_check_port: 8095,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                port: 8094,
                workers: 4,
            },
            alarm: AlarmConfig {
                classification: ClassificationConfig {
                    critical_threshold: 0.9,
                    warning_threshold: 0.7,
                    info_threshold: 0.5,
                },
                processing: ProcessingConfig {
                    enable_deduplication: true,
                    deduplication_window_secs: 300,
                    max_alarms_per_device: 1000,
                },
                notification: NotificationConfig {
                    enabled: true,
                    channels: vec!["email".to_string(), "sms".to_string(), "webhook".to_string()],
                    rate_limit_per_hour: 100,
                },
            },
            storage: StorageConfig {
                retention_days: 30,
                auto_cleanup: true,
                cleanup_interval_hours: 24,
                batch_size: 100,
            },
        };
        
        serde_yaml::to_string(&config).unwrap_or_else(|_| "# Failed to generate config file".to_string())
    }
    
    /// Migrate from old configuration format
    pub fn from_old_config(old_config: super::config::AlarmConfig) -> Self {
        AlarmServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "alarmsrv".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Intelligent Alarm Management Service".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: old_config.redis.get_connection_url(),
                    prefix: "voltage:alarm:".to_string(),
                    pool_size: 20,
                    database: old_config.redis.database as u32,
                    password: old_config.redis.password,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: None,
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9094,
                    health_check_enabled: true,
                    health_check_port: 8095,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                host: old_config.api.host,
                port: old_config.api.port,
                workers: 4,
            },
            alarm: AlarmConfig {
                classification: ClassificationConfig {
                    critical_threshold: 0.9,
                    warning_threshold: 0.7,
                    info_threshold: 0.5,
                },
                processing: ProcessingConfig {
                    enable_deduplication: true,
                    deduplication_window_secs: 300,
                    max_alarms_per_device: 1000,
                },
                notification: NotificationConfig {
                    enabled: true,
                    channels: vec!["email".to_string(), "sms".to_string(), "webhook".to_string()],
                    rate_limit_per_hour: 100,
                },
            },
            storage: StorageConfig {
                retention_days: old_config.storage.retention_days,
                auto_cleanup: old_config.storage.auto_cleanup,
                cleanup_interval_hours: old_config.storage.cleanup_interval_hours,
                batch_size: 100,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_validation() {
        let mut config = AlarmServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "alarmsrv".to_string(),
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
                host: "0.0.0.0".to_string(),
                port: 8094,
                workers: 4,
            },
            alarm: AlarmConfig {
                classification: ClassificationConfig {
                    critical_threshold: 0.9,
                    warning_threshold: 0.7,
                    info_threshold: 0.5,
                },
                processing: ProcessingConfig {
                    enable_deduplication: true,
                    deduplication_window_secs: 300,
                    max_alarms_per_device: 1000,
                },
                notification: NotificationConfig {
                    enabled: true,
                    channels: vec!["email".to_string()],
                    rate_limit_per_hour: 100,
                },
            },
            storage: StorageConfig {
                retention_days: 30,
                auto_cleanup: true,
                cleanup_interval_hours: 24,
                batch_size: 100,
            },
        };
        
        // Valid configuration should pass
        assert!(config.validate_all().is_ok());
        
        // Invalid threshold order should fail
        config.alarm.classification.warning_threshold = 0.95;
        assert!(config.validate_all().is_err());
        config.alarm.classification.warning_threshold = 0.7;
        
        // Invalid storage configuration should fail
        config.storage.retention_days = 0;
        assert!(config.validate_all().is_err());
    }
    
    #[test]
    fn test_generate_default_config() {
        let yaml = AlarmServiceConfig::generate_default_config();
        assert!(yaml.contains("alarmsrv"));
        assert!(yaml.contains("redis"));
        assert!(yaml.contains("classification"));
        assert!(yaml.contains("storage"));
    }
}