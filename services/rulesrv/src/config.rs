use crate::error::{Result, RulesrvError};
use serde::{Deserialize, Serialize};
use std::path::Path;
use voltage_libs::config::utils::{get_global_log_level, get_global_redis_url};
use voltage_libs::config::ConfigLoader;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Service configuration
    pub service: ServiceConfig,

    /// Redis configuration
    pub redis: RedisConfig,

    /// Engine configuration
    pub engine: EngineConfig,

    /// API configuration
    pub api: ApiConfig,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Redis URL (convenience accessor)
    #[serde(skip)]
    pub redis_url: String,
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    #[serde(default = "default_service_name")]
    pub name: String,

    /// Service port
    #[serde(default = "default_service_port")]
    pub port: u16,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Key prefix (hardcoded, not configurable)
    #[serde(skip_serializing, skip_deserializing)]
    pub key_prefix: String,

    /// Subscribe patterns
    #[serde(default = "default_subscribe_patterns")]
    pub subscribe_patterns: Vec<String>,
}

/// Engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Maximum number of workers
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,

    /// Evaluation timeout in milliseconds
    #[serde(default = "default_evaluation_timeout_ms")]
    pub evaluation_timeout_ms: u64,

    /// Rule configuration key pattern
    #[serde(default = "default_rule_key_pattern")]
    pub rule_key_pattern: String,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub timeout_seconds: u64,
}

impl Config {
    /// Load configuration
    pub fn load() -> Result<Self> {
        // 尝试多个配置文件路径
        let config_paths = [
            "config/rulesrv/rulesrv.yaml",
            "config/rulesrv.yaml",
            "rulesrv.yaml",
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
            .with_defaults(Config::default())
            .with_env_prefix("RULESRV");

        let mut config = if let Some(path) = yaml_path {
            loader.with_yaml_file(&path).build()
        } else {
            loader.build()
        }
        .map_err(|e| RulesrvError::ConfigError(format!("Failed to load config: {}", e)))?;

        // 确保硬编码值
        config.redis.key_prefix = "rulesrv:".to_string();
        // 强制硬编码端口，不可配置
        config.service.port = 6003;
        config.redis_url = config.redis.url.clone();

        Ok(config)
    }

    /// Load configuration from file (backward compatibility)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            let loader = ConfigLoader::new()
                .with_defaults(Config::default())
                .with_env_prefix("RULESRV")
                .with_yaml_file(&path.to_string_lossy());

            let mut config = loader
                .build()
                .map_err(|e| RulesrvError::ConfigError(format!("Failed to load config: {}", e)))?;

            // 确保硬编码值
            config.redis.key_prefix = "rulesrv:".to_string();
            // 强制硬编码端口，不可配置
            config.service.port = 6003;
            config.redis_url = config.redis.url.clone();

            Ok(config)
        } else {
            Self::load()
        }
    }

    /// Load configuration from environment variables (backward compatibility)
    pub fn from_env() -> Result<Self> {
        Self::load()
    }
}

impl ApiConfig {
    /// Build a path with API prefix
    pub fn build_path(&self, path: &str) -> String {
        format!("/api/v1/{}", path.trim_start_matches('/'))
    }
}

impl Default for Config {
    fn default() -> Self {
        let redis_config = RedisConfig::default();
        let redis_url = redis_config.url.clone();
        Config {
            service: ServiceConfig::default(),
            redis: redis_config,
            engine: EngineConfig::default(),
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                timeout_seconds: 30,
            },
            log_level: default_log_level(),
            redis_url,
        }
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        ServiceConfig {
            name: default_service_name(),
            port: default_service_port(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        RedisConfig {
            url: default_redis_url(),
            key_prefix: default_key_prefix(),
            subscribe_patterns: default_subscribe_patterns(),
        }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            max_workers: default_max_workers(),
            evaluation_timeout_ms: default_evaluation_timeout_ms(),
            rule_key_pattern: default_rule_key_pattern(),
        }
    }
}

// Default value functions
fn default_service_name() -> String {
    "rulesrv".to_string()
}

fn default_service_port() -> u16 {
    6003 // 默认端口
}

fn default_redis_url() -> String {
    get_global_redis_url("RULESRV")
}

fn default_key_prefix() -> String {
    "rulesrv:".to_string() // 硬编码
}

fn default_subscribe_patterns() -> Vec<String> {
    vec!["modsrv:model:output:*".to_string()]
}

fn default_max_workers() -> usize {
    10
}

fn default_evaluation_timeout_ms() -> u64 {
    5000
}

fn default_rule_key_pattern() -> String {
    "rulesrv:rule:config:*".to_string()
}

fn default_log_level() -> String {
    get_global_log_level("RULESRV")
}
