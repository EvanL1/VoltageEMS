//! Redis configuration types

use crate::utils::error::{ComSrvError, Result};
use serde::{Deserialize, Serialize};

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Whether Redis is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Redis URL (supports redis://, rediss://, unix://)
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Database number
    #[serde(default, alias = "database")]
    pub db: u8,

    /// Connection timeout in milliseconds
    #[serde(default = "default_redis_timeout")]
    pub timeout_ms: u64,

    /// Maximum connections in pool
    pub max_connections: Option<u32>,

    /// Connection retry attempts
    #[serde(default = "default_redis_retries")]
    pub max_retries: u32,

    /// Batch size for pub/sub publishing (default: 100)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Batch timeout in milliseconds (default: 50)
    #[serde(default = "default_batch_timeout")]
    pub batch_timeout_ms: u64,
}

/// Pub/Sub configuration (for internal use)
#[derive(Debug, Clone)]
pub struct PubSubConfig {
    /// Whether to enable pub/sub publishing (always true when Redis is enabled)
    pub enabled: bool,

    /// Batch size for bulk publishing
    pub batch_size: usize,

    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,

    /// Whether to publish on set operations (always true)
    pub publish_on_set: bool,

    /// Message format version
    pub message_version: String,
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            enabled: true, // 始终启用
            batch_size: 100,
            batch_timeout_ms: 50,
            publish_on_set: true,
            message_version: "1.0".to_string(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

fn default_redis_timeout() -> u64 {
    5000
}

fn default_redis_retries() -> u32 {
    3
}

fn default_false() -> bool {
    false
}

fn default_batch_size() -> usize {
    100
}

fn default_batch_timeout() -> u64 {
    50
}

fn default_message_version() -> String {
    "1.0".to_string()
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            url: default_redis_url(),
            db: 0,
            timeout_ms: default_redis_timeout(),
            max_connections: None,
            max_retries: default_redis_retries(),
            batch_size: default_batch_size(),
            batch_timeout_ms: default_batch_timeout(),
        }
    }
}

impl RedisConfig {
    /// Validate Redis configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.url.is_empty() {
            return Err(ComSrvError::ConfigError(
                "Redis URL cannot be empty when enabled".to_string(),
            ));
        }
        Ok(())
    }

    /// Convert to Redis URL format (for backward compatibility)
    pub fn to_redis_url(&self) -> String {
        self.url.clone()
    }

    /// Get connection type (for backward compatibility)
    pub fn connection_type(&self) -> String {
        if self.url.starts_with("rediss://") {
            "tls".to_string()
        } else if self.url.starts_with("unix://") {
            "unix".to_string()
        } else {
            "tcp".to_string()
        }
    }

    /// Convert to PubSubConfig (pub/sub always enabled when Redis is enabled)
    pub fn to_pubsub_config(&self) -> PubSubConfig {
        PubSubConfig {
            enabled: self.enabled, // 与Redis配置一致
            batch_size: self.batch_size,
            batch_timeout_ms: self.batch_timeout_ms,
            publish_on_set: true,
            message_version: "1.0".to_string(),
        }
    }
}
