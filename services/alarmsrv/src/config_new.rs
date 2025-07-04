use serde::{Deserialize, Serialize};
use voltage_config::prelude::*;

/// Alarm service configuration using the unified config framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmServiceConfig {
    /// Base service configuration (flattened)
    #[serde(flatten)]
    pub base: BaseServiceConfig,
    
    /// API configuration
    pub api: ApiConfig,
    
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
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Retention period for resolved alarms (in days)
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// Enable automatic cleanup
    #[serde(default = "default_auto_cleanup")]
    pub auto_cleanup: bool,
    /// Cleanup interval (in hours)
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_hours: u32,
}

// Default value functions
fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_api_port() -> u16 {
    8094
}

fn default_retention_days() -> u32 {
    30
}

fn default_auto_cleanup() -> bool {
    true
}

fn default_cleanup_interval() -> u32 {
    24
}

impl Default for AlarmServiceConfig {
    fn default() -> Self {
        Self {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "alarmsrv".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Alarm Management Service".to_string(),
                },
                redis: RedisConfig::default(),
                logging: LoggingConfig::default(),
                monitoring: MonitoringConfig::default(),
            },
            api: ApiConfig {
                host: default_api_host(),
                port: default_api_port(),
            },
            storage: StorageConfig {
                retention_days: default_retention_days(),
                auto_cleanup: default_auto_cleanup(),
                cleanup_interval_hours: default_cleanup_interval(),
            },
        }
    }
}

impl Configurable for AlarmServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        // Validate API configuration
        if self.api.port == 0 {
            return Err(ConfigError::Validation("API port cannot be 0".into()));
        }
        
        // Validate storage configuration
        if self.storage.retention_days == 0 {
            return Err(ConfigError::Validation("Retention days must be greater than 0".into()));
        }
        
        if self.storage.cleanup_interval_hours == 0 {
            return Err(ConfigError::Validation("Cleanup interval must be greater than 0".into()));
        }
        
        // Validate base configuration
        self.base.validate()?;
        
        Ok(())
    }
}

/// Helper function to load configuration
pub async fn load_config() -> anyhow::Result<AlarmServiceConfig> {
    let config = ConfigLoaderBuilder::new()
        .add_file("config/alarmsrv.yml")
        .add_file("config/alarmsrv.yaml")
        .add_sqlite("sqlite:data/config.db", "alarmsrv")
        .add_env_prefix("ALARMSRV_")
        .build()?
        .load::<AlarmServiceConfig>()?;
    
    Ok(config)
}

/// Helper function to generate default configuration file
pub fn generate_default_config() -> String {
    let config = AlarmServiceConfig::default();
    serde_yaml::to_string(&config).unwrap_or_else(|_| "# Failed to generate config file".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AlarmServiceConfig::default();
        assert_eq!(config.base.service.name, "alarmsrv");
        assert_eq!(config.api.host, "0.0.0.0");
        assert_eq!(config.api.port, 8094);
        assert_eq!(config.storage.retention_days, 30);
        assert!(config.storage.auto_cleanup);
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = AlarmServiceConfig::default();
        
        // Valid config should pass
        assert!(config.validate().is_ok());
        
        // Invalid API port
        config.api.port = 0;
        assert!(config.validate().is_err());
        config.api.port = 8094;
        
        // Invalid retention days
        config.storage.retention_days = 0;
        assert!(config.validate().is_err());
        config.storage.retention_days = 30;
        
        // Invalid cleanup interval
        config.storage.cleanup_interval_hours = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_generate_default_config() {
        let yaml = generate_default_config();
        assert!(yaml.contains("service:"));
        assert!(yaml.contains("name: alarmsrv"));
        assert!(yaml.contains("api:"));
        assert!(yaml.contains("storage:"));
        assert!(yaml.contains("redis:"));
        assert!(yaml.contains("logging:"));
    }
}