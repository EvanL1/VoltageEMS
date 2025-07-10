//! Redis configuration types

use serde::{Deserialize, Serialize};

/// Redis client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis server hostname
    #[serde(default = "default_host")]
    pub host: String,

    /// Redis server port
    #[serde(default = "default_port")]
    pub port: u16,

    /// Redis password (optional)
    pub password: Option<String>,

    /// Unix socket path (optional, takes precedence over host/port)
    pub socket: Option<String>,

    /// Database number
    #[serde(default)]
    pub database: u8,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub connection_timeout: u64,

    /// Maximum number of retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            password: None,
            socket: None,
            database: 0,
            connection_timeout: default_timeout(),
            max_retries: default_max_retries(),
        }
    }
}

impl RedisConfig {
    /// Convert configuration to Redis URL
    pub fn to_url(&self) -> String {
        if let Some(password) = &self.password {
            format!(
                "redis://:{}@{}:{}/{}",
                password, self.host, self.port, self.database
            )
        } else {
            format!("redis://{}:{}/{}", self.host, self.port, self.database)
        }
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("REDIS_HOST").unwrap_or_else(|_| default_host()),
            port: std::env::var("REDIS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or_else(default_port),
            password: std::env::var("REDIS_PASSWORD").ok(),
            socket: std::env::var("REDIS_SOCKET").ok(),
            database: std::env::var("REDIS_DATABASE")
                .ok()
                .and_then(|d| d.parse().ok())
                .unwrap_or(0),
            connection_timeout: std::env::var("REDIS_TIMEOUT")
                .ok()
                .and_then(|t| t.parse().ok())
                .unwrap_or_else(default_timeout),
            max_retries: std::env::var("REDIS_MAX_RETRIES")
                .ok()
                .and_then(|r| r.parse().ok())
                .unwrap_or_else(default_max_retries),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    6379
}

fn default_timeout() -> u64 {
    10
}

fn default_max_retries() -> u32 {
    3
}
