use serde::{Deserialize, Serialize};
use std::any::Any;
use crate::{Configurable, Result, ConfigError};

/// Base configuration shared by all VoltageEMS services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseServiceConfig {
    pub service: ServiceInfo,
    pub redis: RedisConfig,
    pub logging: LoggingConfig,
    pub monitoring: MonitoringConfig,
}

/// Service identification and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub instance_id: String,
}

/// Redis connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    #[serde(default = "default_redis_prefix")]
    pub prefix: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "default_database")]
    pub database: u32,
    #[serde(default)]
    pub password: Option<String>,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_console")]
    pub console: bool,
    #[serde(default)]
    pub file: Option<LogFileConfig>,
    #[serde(default)]
    pub json_format: bool,
}

/// Log file configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileConfig {
    pub path: String,
    #[serde(default = "default_log_rotation")]
    pub rotation: String,
    #[serde(default = "default_max_size")]
    pub max_size: String,
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

/// Monitoring and metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
    #[serde(default = "default_health_check_enabled")]
    pub health_check_enabled: bool,
    #[serde(default = "default_health_check_port")]
    pub health_check_port: u16,
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval: u64,
}

// Default value functions
fn default_redis_prefix() -> String {
    "voltage:".to_string()
}

fn default_pool_size() -> u32 {
    10
}

fn default_database() -> u32 {
    0
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_console() -> bool {
    true
}

fn default_log_rotation() -> String {
    "daily".to_string()
}

fn default_max_size() -> String {
    "100MB".to_string()
}

fn default_max_files() -> u32 {
    7
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_health_check_enabled() -> bool {
    true
}

fn default_health_check_port() -> u16 {
    8080
}

fn default_health_check_interval() -> u64 {
    30
}

impl Configurable for BaseServiceConfig {
    fn validate(&self) -> Result<()> {
        // Validate service info
        if self.service.name.is_empty() {
            return Err(ConfigError::Validation("Service name cannot be empty".into()));
        }
        
        if self.service.version.is_empty() {
            return Err(ConfigError::Validation("Service version cannot be empty".into()));
        }
        
        // Validate Redis config
        if !self.redis.url.starts_with("redis://") {
            return Err(ConfigError::Validation("Redis URL must start with redis://".into()));
        }
        
        if self.redis.pool_size == 0 {
            return Err(ConfigError::Validation("Redis pool size must be greater than 0".into()));
        }
        
        // Validate logging config
        match self.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(ConfigError::Validation(format!(
                "Invalid log level: {}. Must be one of: trace, debug, info, warn, error",
                self.logging.level
            ))),
        }
        
        // Validate monitoring config
        if self.monitoring.metrics_port == 0 {
            return Err(ConfigError::Validation("Metrics port cannot be 0".into()));
        }
        
        if self.monitoring.health_check_port == 0 {
            return Err(ConfigError::Validation("Health check port cannot be 0".into()));
        }
        
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Trait for service-specific configurations that extend BaseServiceConfig
pub trait ServiceConfig: Configurable {
    /// Get the base service configuration
    fn base(&self) -> &BaseServiceConfig;
    
    /// Get mutable reference to base service configuration
    fn base_mut(&mut self) -> &mut BaseServiceConfig;
    
    /// Validate the complete configuration including base and service-specific parts
    fn validate_all(&self) -> Result<()> {
        // First validate base configuration
        self.base().validate()?;
        
        // Then validate service-specific configuration
        self.validate()?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_base_config_validation() {
        let mut config = BaseServiceConfig {
            service: ServiceInfo {
                name: "test-service".to_string(),
                version: "1.0.0".to_string(),
                description: "Test service".to_string(),
                instance_id: "test-1".to_string(),
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
        };
        
        assert!(config.validate().is_ok());
        
        // Test invalid service name
        config.service.name = "".to_string();
        assert!(config.validate().is_err());
        config.service.name = "test-service".to_string();
        
        // Test invalid Redis URL
        config.redis.url = "invalid-url".to_string();
        assert!(config.validate().is_err());
        config.redis.url = "redis://localhost:6379".to_string();
        
        // Test invalid log level
        config.logging.level = "invalid".to_string();
        assert!(config.validate().is_err());
    }
}