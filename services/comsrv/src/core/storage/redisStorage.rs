use std::sync::Arc;
use tokio::sync::Mutex;
use redis::{AsyncCommands, Client};
use redis::aio::Connection;
use serde::{Serialize, Deserialize};
use crate::utils::error::{ComSrvError, Result};
use crate::core::config::config_manager::RedisConfig;

/// realtime value structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeValue {
    pub raw: f64,
    pub processed: f64,
    pub timestamp: String, // ISO 8601 format
}

/// Redis storage structure
#[derive(Clone)]
pub struct RedisStore {
    conn: Arc<Mutex<Connection>>,  // Redis connection
}

impl RedisStore {
    /// create Redis connection, support TCP and Unix Socket
    pub async fn from_config(config: &RedisConfig) -> Result<Option<Self>> {
        if !config.enabled {
            tracing::info!("Redis disabled in config");
            return Ok(None);
        }

        let url = if config.address.starts_with("unix://") {
            config.address.to_string()
        } else if config.address.starts_with("tcp://") {
            config.address.replacen("tcp://", "redis://", 1)
        } else if config.address.starts_with("redis://") {
            config.address.to_string()
        } else {
            return Err(ComSrvError::RedisError(format!("Unsupported Redis address: {}", config.address)));
        };

        let client = Client::open(url)
            .map_err(|e| ComSrvError::RedisError(format!("Invalid Redis URL: {}", e)))?;

        let conn = client.get_async_connection().await
            .map_err(|e| ComSrvError::RedisError(format!("Failed to connect Redis: {}", e)))?;

        let mut conn = conn;
        
        if let Some(db_index) = config.db {
            redis::cmd("SELECT").arg(db_index)
                .query_async(&mut conn).await
                .map_err(|e| ComSrvError::RedisError(format!("SELECT DB error: {}", e)))?;
        }

        Ok(Some(RedisStore {
            conn: Arc::new(Mutex::new(conn)),
        }))
    }


    /// write realtime value to Redis
    pub async fn set_realtime_value(&self, key: &str, value: &RealtimeValue) -> Result<()> {
        let val_str = serde_json::to_string(value)
            .map_err(|e| ComSrvError::RedisError(format!("Serialize RealtimeValue error: {}", e)))?;

        let mut guard = self.conn.lock().await;
        guard.set::<&str, String, ()>(key, val_str).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set error: {}", e)))?;

        Ok(())
    }

    /// write realtime value with expire time (seconds)
    pub async fn set_realtime_value_with_expire(&self, key: &str, value: &RealtimeValue, expire_secs: usize) -> Result<()> {
        let val_str = serde_json::to_string(value)
            .map_err(|e| ComSrvError::RedisError(format!("Serialize RealtimeValue error: {}", e)))?;

        let mut guard = self.conn.lock().await;
        guard.set_ex::<&str, String, ()>(key, val_str, expire_secs).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set_ex error: {}", e)))?;

        Ok(())
    }

    /// read realtime value
    pub async fn get_realtime_value(&self, key: &str) -> Result<Option<RealtimeValue>> {
        let mut guard = self.conn.lock().await;
        let val: Option<String> = guard.get(key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str)
                .map_err(|e| ComSrvError::RedisError(format!("Deserialize RealtimeValue error: {}", e)))?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }
}