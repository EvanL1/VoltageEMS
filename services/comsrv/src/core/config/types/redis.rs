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

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            url: default_redis_url(),
            db: 0,
            timeout_ms: default_redis_timeout(),
            max_connections: None,
            max_retries: default_redis_retries(),
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
}
