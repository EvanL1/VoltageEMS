//! Simplified AlarmSrv Configuration
//!
//! This module provides a streamlined configuration system without complex abstractions.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use voltage_libs::config::utils::get_global_redis_url;
use voltage_libs::config::ConfigLoader;

/// Simplified alarm service configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlarmConfig {
    /// Redis configuration
    pub redis: RedisConfig,
    /// API configuration
    pub api: ApiConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Simple monitoring configuration
    #[serde(default)]
    pub monitoring: MonitoringConfig,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,
    /// Key prefix (hardcoded, not configurable)
    #[serde(skip_serializing, skip_deserializing)]
    pub key_prefix: String,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Listen address
    pub host: String,
    /// Listen port
    pub port: u16,
}

impl ApiConfig {
    /// Build a path with API prefix
    pub fn build_path(&self, path: &str) -> String {
        format!("/api/v1/{}", path.trim_start_matches('/'))
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Retention period for resolved alarms (in days)
    pub retention_days: u32,
    /// Enable automatic cleanup
    pub auto_cleanup: bool,
    /// Cleanup interval (in hours)
    pub cleanup_interval_hours: u32,
}

/// Simple monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable monitoring
    #[serde(default = "default_false")]
    pub enabled: bool,
    /// Scan interval in seconds
    #[serde(default = "default_scan_interval")]
    pub scan_interval: u64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scan_interval: default_scan_interval(),
        }
    }
}

fn default_false() -> bool {
    false
}

fn default_scan_interval() -> u64 {
    10
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: get_global_redis_url("ALARMSRV"),
            key_prefix: "alarmsrv:".to_string(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8083,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            auto_cleanup: true,
            cleanup_interval_hours: 24,
        }
    }
}

impl AlarmConfig {
    /// Load configuration
    pub async fn load() -> Result<Self> {
        // 尝试多个配置文件路径
        let config_paths = [
            "config/alarmsrv/alarmsrv.yaml",
            "config/alarmsrv.yaml",
            "alarmsrv.yaml",
        ];

        let mut yaml_path = None;
        for path in &config_paths {
            if Path::new(path).exists() {
                yaml_path = Some(path.to_string());
                break;
            }
        }

        // 使用新的 ConfigLoader
        let loader = ConfigLoader::new()
            .with_defaults(AlarmConfig::default())
            .with_env_prefix("ALARMSRV");

        let mut config = if let Some(path) = yaml_path {
            loader.with_yaml_file(&path).build()
        } else {
            loader.build()
        }?;

        // 确保硬编码值
        config.redis.key_prefix = "alarmsrv:".to_string();
        config.api.port = 8083;

        // Override specific environment variables if set
        if let Ok(retention_days) = std::env::var("ALARMSRV_STORAGE_RETENTION_DAYS") {
            if let Ok(days) = retention_days.parse() {
                config.storage.retention_days = days;
            }
        }

        // Disable monitoring by default for simplified implementation
        config.monitoring.enabled = false;

        Ok(config)
    }

    /// Generate default configuration file
    pub fn generate_default_config() -> String {
        let config = Self::default();
        serde_yaml::to_string(&config)
            .unwrap_or_else(|_| "# Failed to generate config file".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AlarmConfig::default();
        assert!(config.redis.url.contains("redis://"));
        assert_eq!(config.redis.key_prefix, "alarmsrv:");
        assert_eq!(config.api.host, "0.0.0.0");
        assert_eq!(config.api.port, 8083);
        assert_eq!(config.storage.retention_days, 30);
        assert!(config.storage.auto_cleanup);
    }

    #[tokio::test]
    async fn test_config_load() {
        let config = AlarmConfig::load().await.unwrap();
        assert!(!config.redis.url.is_empty());
        assert_eq!(config.redis.key_prefix, "alarmsrv:");
        assert_eq!(config.api.port, 8083);
        assert!(config.storage.retention_days > 0);
    }

    #[test]
    fn test_generate_default_config() {
        let yaml = AlarmConfig::generate_default_config();
        assert!(yaml.contains("redis"));
        assert!(yaml.contains("api"));
        assert!(yaml.contains("storage"));
    }
}
