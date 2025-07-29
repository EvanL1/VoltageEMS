use crate::error::{Result, RulesrvError};
use serde::{Deserialize, Serialize};
use std::path::Path;
// Remove voltage_common dependency

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

    /// API server port
    #[serde(default = "default_api_port")]
    pub api_port: u16,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Key prefix
    #[serde(default = "default_key_prefix")]
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
    pub port: u16,
    pub timeout_seconds: u64,
}

impl Config {
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| RulesrvError::ConfigError(format!("Failed to read config file: {}", e)))?;

        let mut config: Config = serde_yaml::from_str(&content)
            .map_err(|e| RulesrvError::ConfigError(format!("Failed to parse config: {}", e)))?;

        // Set the redis_url from redis.url
        config.redis_url = config.redis.url.clone();

        Ok(config)
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| default_redis_url());
        Ok(Config {
            service: ServiceConfig {
                name: std::env::var("SERVICE_NAME").unwrap_or_else(|_| default_service_name()),
                port: std::env::var("SERVICE_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(default_service_port),
                api_port: std::env::var("API_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(default_api_port),
            },
            redis: RedisConfig {
                url: redis_url.clone(),
                key_prefix: std::env::var("REDIS_KEY_PREFIX")
                    .unwrap_or_else(|_| default_key_prefix()),
                subscribe_patterns: std::env::var("REDIS_SUBSCRIBE_PATTERNS")
                    .ok()
                    .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
                    .unwrap_or_else(default_subscribe_patterns),
            },
            engine: EngineConfig {
                max_workers: std::env::var("ENGINE_MAX_WORKERS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(default_max_workers),
                evaluation_timeout_ms: std::env::var("ENGINE_TIMEOUT_MS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(default_evaluation_timeout_ms),
                rule_key_pattern: std::env::var("RULE_KEY_PATTERN")
                    .unwrap_or_else(|_| default_rule_key_pattern()),
            },
            api: ApiConfig {
                host: std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: std::env::var("API_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(8080),
                timeout_seconds: std::env::var("API_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30),
            },
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| default_log_level()),
            redis_url,
        })
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
                port: 8080,
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
            api_port: default_api_port(),
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
    8086
}

fn default_redis_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_key_prefix() -> String {
    "rulesrv:".to_string()
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

fn default_api_port() -> u16 {
    8083
}

fn default_log_level() -> String {
    "info".to_string()
}
