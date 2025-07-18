//! Redis client utilities for `VoltageEMS` services
//!
//! This module provides both synchronous and asynchronous Redis client implementations
//! with support for connection pooling, Unix sockets, and comprehensive error handling.

use crate::Result;

#[cfg(feature = "async")]
pub mod async_client;
pub mod config;
#[cfg(feature = "sync")]
pub mod sync_client;
pub mod types;

// Re-export commonly used items
#[cfg(feature = "async")]
pub use async_client::{RedisClient, RedisClientBuilder};
pub use config::RedisConfig;
#[cfg(feature = "sync")]
pub use sync_client::{RedisSyncClient, RedisSyncClientBuilder};
pub use types::*;

/// Create an async Redis client from configuration
#[cfg(feature = "async")]
pub async fn create_async_client(config: &RedisConfig) -> Result<RedisClient> {
    #[cfg(feature = "unix-socket")]
    if let Some(socket) = &config.socket {
        return RedisClient::new_with_socket(socket).await;
    }

    let url = config.to_url();
    RedisClient::new(&url).await
}

/// Create a sync Redis client from configuration
#[cfg(feature = "sync")]
pub fn create_sync_client(config: &RedisConfig) -> Result<RedisSyncClient> {
    #[cfg(feature = "unix-socket")]
    if let Some(socket) = &config.socket {
        return RedisSyncClient::new_with_socket(socket);
    }

    let url = config.to_url();
    RedisSyncClient::new(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_config() {
        let config = RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: Some("password".to_string()),
            socket: None,
            database: 0,
            connection_timeout: 10,
            max_retries: 3,
        };

        let url = config.to_url();
        assert_eq!(url, "redis://:password@localhost:6379/0");
    }

    #[test]
    fn test_redis_config_no_password() {
        let config = RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            socket: None,
            database: 0,
            connection_timeout: 10,
            max_retries: 3,
        };

        let url = config.to_url();
        assert_eq!(url, "redis://localhost:6379/0");
    }
}
