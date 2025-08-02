//! ModSrv configuration management
//!
//! Provides unified configuration loading and management

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info};

use crate::error::{ModelSrvError, Result};
use crate::model::ModelConfig;
use voltage_libs::config::utils::{get_global_log_level, get_global_redis_url};
use voltage_libs::config::ConfigLoader;

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub key_prefix: String, // Hard-coded, not loaded from config
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_ms: u64,
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: usize,
}

fn default_redis_url() -> String {
    get_global_redis_url("MODSRV")
}

fn default_connection_timeout() -> u64 {
    5000
}

fn default_retry_attempts() -> usize {
    3
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: get_global_redis_url("MODSRV"),
            key_prefix: "modsrv:".to_string(), // Hard-coded
            connection_timeout_ms: 5000,
            retry_attempts: 3,
        }
    }
}

/// API service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_api_host")]
    pub host: String,
    #[serde(default = "default_modsrv_port")]
    pub port: u16,
    #[serde(default = "default_api_timeout")]
    pub timeout_seconds: u64,
}

fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_modsrv_port() -> u16 {
    8082
}

fn default_api_timeout() -> u64 {
    30
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_modsrv_port(),
            timeout_seconds: default_api_timeout(),
        }
    }
}

/// Log configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file_path: Option<String>,
}

fn default_log_level() -> String {
    get_global_log_level("MODSRV")
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file_path: None,
        }
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_service_name")]
    pub service_name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub models: Vec<ModelConfig>,
    #[serde(default = "default_update_interval")]
    pub update_interval_ms: u64,
}

fn default_service_name() -> String {
    "modsrv".to_string()
}

fn default_version() -> String {
    "0.0.1".to_string()
}

fn default_update_interval() -> u64 {
    1000
}

// Default implementation is automatically derived

impl Config {
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            ModelSrvError::ConfigError(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        let config: Config = if path.extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.extension().and_then(|s| s.to_str()) == Some("yml")
        {
            serde_yaml::from_str(&content).map_err(|e| {
                ModelSrvError::ConfigError(format!("Failed to parse YAML config: {}", e))
            })?
        } else {
            serde_json::from_str(&content).map_err(|e| {
                ModelSrvError::ConfigError(format!("Failed to parse JSON config: {}", e))
            })?
        };

        info!("Config loaded successfully: {}", path.display());
        debug!("Config content: {:?}", config);
        Ok(config)
    }

    /// Load configuration from environment variables
    #[allow(dead_code)]
    pub fn from_env() -> Result<Self> {
        let mut config = Config::default();

        // Redis config (using global variables)
        config.redis.url = get_global_redis_url("MODSRV");
        // key_prefix is hard-coded, not loaded from env vars
        config.redis.key_prefix = "modsrv:".to_string();

        // API config
        if let Ok(api_host) = std::env::var("MODSRV_API_HOST") {
            config.api.host = api_host;
        }
        // Port is fixed, not loaded from env vars
        config.api.port = 8082;

        // Log config (using global variables)
        config.log.level = get_global_log_level("MODSRV");
        if let Ok(log_file) = std::env::var("MODSRV_LOG_FILE") {
            config.log.file_path = Some(log_file);
        }

        // Update interval
        if let Ok(interval) = std::env::var("MODSRV_UPDATE_INTERVAL_MS") {
            config.update_interval_ms = interval.parse().map_err(|e| {
                ModelSrvError::ConfigError(format!("Invalid update interval: {}", e))
            })?;
        }

        info!("Config loaded from environment variables");
        Ok(config)
    }

    /// Auto-load config (YAML highest priority > env vars > defaults)
    pub fn load() -> Result<Self> {
        // Try multiple config file paths
        let config_paths = [
            "config/modsrv/modsrv.yaml",
            "config/modsrv.yaml",
            "modsrv.yaml",
        ];

        let mut yaml_path = None;
        for path in &config_paths {
            if Path::new(path).exists() {
                yaml_path = Some(path.to_string());
                break;
            }
        }

        // Use new ConfigLoader
        let loader = ConfigLoader::new()
            .with_defaults(Config::default())
            .with_env_prefix("MODSRV");

        let mut config = if let Some(path) = yaml_path {
            loader.with_yaml_file(&path).build()
        } else {
            loader.build()
        }
        .map_err(|e| ModelSrvError::ConfigError(format!("Failed to load config: {}", e)))?;

        // Hard-code some fields, not allowed to override from config
        config.redis.key_prefix = "modsrv:".to_string();
        config.api.port = 8082;

        // Validate config
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate Redis URL
        if self.redis.url.is_empty() {
            return Err(ModelSrvError::ConfigError(
                "Redis URL cannot be empty".to_string(),
            ));
        }

        // Validate API port
        if self.api.port == 0 {
            return Err(ModelSrvError::ConfigError(
                "API port cannot be 0".to_string(),
            ));
        }

        // Validate model config
        for model in &self.models {
            if model.id.is_empty() {
                return Err(ModelSrvError::ConfigError(
                    "Model ID cannot be empty".to_string(),
                ));
            }
            if model.monitoring.is_empty() && model.control.is_empty() {
                return Err(ModelSrvError::ConfigError(format!(
                    "Model {} must contain monitoring or control points",
                    model.id
                )));
            }
        }

        info!("Config validation passed");
        Ok(())
    }

    /// Save configuration to file
    #[allow(dead_code)]
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let content = if path.extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.extension().and_then(|s| s.to_str()) == Some("yml")
        {
            serde_yaml::to_string(self).map_err(|e| {
                ModelSrvError::ConfigError(format!("Failed to serialize YAML: {}", e))
            })?
        } else {
            serde_json::to_string_pretty(self).map_err(|e| {
                ModelSrvError::ConfigError(format!("Failed to serialize JSON: {}", e))
            })?
        };

        std::fs::write(path, content).map_err(|e| {
            ModelSrvError::ConfigError(format!("Failed to write config file: {}", e))
        })?;

        info!("Config saved to: {}", path.display());
        Ok(())
    }

    /// Add model configuration
    #[allow(dead_code)]
    pub fn add_model(&mut self, model: ModelConfig) {
        self.models.push(model);
        info!("Added model config: {}", self.models.last().unwrap().id);
    }

    /// Remove model configuration
    #[allow(dead_code)]
    pub fn remove_model(&mut self, model_id: &str) -> bool {
        let original_len = self.models.len();
        self.models.retain(|m| m.id != model_id);
        let removed = self.models.len() < original_len;
        if removed {
            info!("Removed model config: {}", model_id);
        }
        removed
    }

    /// Get enabled model configurations
    pub fn enabled_models(&self) -> Vec<&ModelConfig> {
        self.models.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.service_name, "modsrv");
        assert_eq!(config.redis.url, get_global_redis_url("MODSRV"));
        assert_eq!(config.api.port, 8082);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        // Test invalid config
        config.redis.url = "".to_string();
        assert!(config.validate().is_err());

        config.redis.url = "redis://localhost:6379".to_string();
        config.api.port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_file_operations() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_config.yaml");

        let config = Config::default();
        assert!(config.save_to_file(&file_path).is_ok());
        assert!(file_path.exists());

        let loaded_config = Config::from_file(&file_path).unwrap();
        assert_eq!(config.service_name, loaded_config.service_name);
        assert_eq!(config.api.port, loaded_config.api.port);
    }
}
