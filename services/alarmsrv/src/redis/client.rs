use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use voltage_common::redis::RedisClient;

use crate::config::{AlarmConfig, RedisConnectionType};

/// Redis client wrapper for alarm service
pub struct AlarmRedisClient {
    client: Arc<Mutex<Option<RedisClient>>>,
    #[allow(dead_code)]
    config: Arc<AlarmConfig>,
}

impl AlarmRedisClient {
    /// Create new Redis client instance
    pub async fn new(config: Arc<AlarmConfig>) -> Result<Self> {
        let redis_url = config.redis.get_connection_url();
        info!(
            "Connecting to Redis using URL: {}",
            redis_url.replace(&config.redis.password.clone().unwrap_or_default(), "***")
        );

        let client = RedisClient::new(&redis_url).await?;

        // Test connection with PING
        let ping_result = client.ping().await?;
        if ping_result != "PONG" {
            return Err(anyhow::anyhow!("Redis connection test failed"));
        }

        // Log connection success
        match config.redis.connection_type {
            RedisConnectionType::Tcp => {
                info!(
                    "Successfully connected to Redis via TCP at {}:{}",
                    config.redis.host, config.redis.port
                );
            }
            RedisConnectionType::Unix => {
                if let Some(ref socket_path) = config.redis.socket_path {
                    info!(
                        "Successfully connected to Redis via Unix socket at {}",
                        socket_path
                    );
                } else {
                    info!("Successfully connected to Redis via TCP (fallback from Unix socket)");
                }
            }
        }

        Ok(Self {
            client: Arc::new(Mutex::new(Some(client))),
            config,
        })
    }

    /// Check if Redis connection is active
    pub async fn is_connected(&self) -> bool {
        let client = self.client.lock().await;
        client.is_some()
    }

    /// Get a mutable reference to the Redis client
    pub async fn get_client(&self) -> Result<tokio::sync::MutexGuard<'_, Option<RedisClient>>> {
        Ok(self.client.lock().await)
    }

    /// Scan keys matching a pattern
    pub async fn scan_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut client_guard = self.client.lock().await;
        if let Some(conn) = client_guard.as_mut() {
            // Use keys command for simplicity - in production should use SCAN
            let keys: Vec<String> = conn.keys(pattern).await?;
            Ok(keys)
        } else {
            Err(anyhow::anyhow!("Redis connection not available"))
        }
    }

    /// Get multiple values at once
    pub async fn mget(&self, keys: &[String]) -> Result<Vec<Option<String>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut client_guard = self.client.lock().await;
        if let Some(conn) = client_guard.as_mut() {
            let mut values = Vec::new();
            // Get values one by one - in production should use MGET
            for key in keys {
                let value: Option<String> = conn.get(key).await?;
                values.push(value);
            }
            Ok(values)
        } else {
            Err(anyhow::anyhow!("Redis connection not available"))
        }
    }
}
