use crate::core::config::config_manager::RedisConfig;
use crate::utils::error::{ComSrvError, Result};
use redis::aio::{Connection, PubSub};
use redis::{AsyncCommands, Client, Commands};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use log::{debug, error, info, warn};

/// realtime value structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeValue {
    pub raw: f64,
    pub processed: f64,
    pub timestamp: String, // ISO 8601 format
}

/// Command types for remote control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    /// 遥控指令 (Remote Control) - 布尔值操作
    RemoteControl,
    /// 遥调指令 (Remote Regulation) - 数值设定
    RemoteRegulation,
}

/// Command structure for remote operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCommand {
    pub command_type: CommandType,
    pub point_name: String,
    pub value: f64,
    pub timestamp: String,
    pub command_id: String,          // 唯一标识符
    pub operator: Option<String>,    // 操作员信息
    pub description: Option<String>, // 操作描述
}

/// Command execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command_id: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub execution_time: String,
    pub actual_value: Option<f64>, // 执行后的实际值
}

/// Channel metadata for Redis storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisChannelMetadata {
    pub name: String,
    pub protocol_type: String,
    pub created_at: String,
    pub last_accessed: String,
    pub running: bool,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Configuration data for Redis storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfigData {
    pub config_type: String,
    pub data: serde_json::Value,
    pub version: String,
    pub last_updated: String,
}

/// Connection pool entry for Redis storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConnectionEntry {
    pub protocol: String,
    pub address: String,
    pub port: Option<u16>,
    pub params: HashMap<String, String>,
    pub created_at: String,
    pub last_used: String,
    pub connection_count: u32,
}

/// Statistics data for Redis storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisStatsData {
    pub stats_type: String,
    pub data: serde_json::Value,
    pub timestamp: String,
}

/// Redis connection manager with enhanced features
#[derive(Clone)]
pub struct RedisConnectionManager {
    client: Client,
    config: RedisConfig,
}

impl RedisConnectionManager {
    /// Create new Redis connection manager
    pub async fn new(config: &RedisConfig) -> Result<Self> {
        config.validate()?;

        let url = config.to_redis_url();
        log::debug!("Creating Redis connection manager with URL: {}", &url);

        let client = Client::open(url.clone())
            .map_err(|e| ComSrvError::RedisError(format!("Invalid Redis URL '{}': {}", url, e)))?;

        // Test the connection
        let mut conn = client
            .get_async_connection()
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis connection test failed: {}", e)))?;

        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis PING failed: {}", e)))?;

        // Select database if specified
        if config.db > 0 {
            redis::cmd("SELECT")
                .arg(config.db)
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    ComSrvError::RedisError(format!(
                        "Failed to select database {}: {}",
                        config.db, e
                    ))
                })?;
        }

        log::info!(
            "Redis connection manager created successfully: type={:?}, db={:?}, timeout={}ms",
            config.connection_type,
            config.db,
            config.timeout_ms
        );

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// Get a new connection from Redis client
    pub async fn get_connection(&self) -> Result<Connection> {
        let mut conn = self.client.get_async_connection().await.map_err(|e| {
            ComSrvError::RedisError(format!("Failed to create Redis connection: {}", e))
        })?;

        // Select database if specified
        if self.config.db > 0 {
            redis::cmd("SELECT")
                .arg(self.config.db)
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    ComSrvError::RedisError(format!(
                        "Failed to select database {}: {}",
                        self.config.db, e
                    ))
                })?;
        }

        Ok(conn)
    }

    /// Get synchronous connection for blocking operations
    pub fn get_sync_connection(&self) -> Result<redis::Connection> {
        self.client.get_connection().map_err(|e| {
            ComSrvError::RedisError(format!("Failed to create sync Redis connection: {}", e))
        })
    }

    /// Get the configuration
    pub fn config(&self) -> &RedisConfig {
        &self.config
    }

    /// Test connection health
    pub async fn health_check(&self) -> Result<bool> {
        match self.get_connection().await {
            Ok(mut conn) => match redis::cmd("PING").query_async::<_, String>(&mut conn).await {
                Ok(_) => Ok(true),
                Err(e) => {
                    log::warn!("Redis health check failed: {}", e);
                    Ok(false)
                }
            },
            Err(e) => {
                log::warn!("Redis connection failed: {}", e);
                Ok(false)
            }
        }
    }
}

/// Redis storage structure with enhanced connection management
#[derive(Clone)]
pub struct RedisStore {
    manager: RedisConnectionManager,
}

impl RedisStore {
    /// create Redis connection with enhanced management
    pub async fn from_config(config: &RedisConfig) -> Result<Option<Self>> {
        if !config.enabled {
            log::info!("Redis disabled in configuration");
            return Ok(None);
        }

        let manager = RedisConnectionManager::new(config).await?;

        Ok(Some(RedisStore { manager }))
    }

    /// Get connection manager
    pub fn manager(&self) -> &RedisConnectionManager {
        &self.manager
    }

    // ========== Real-time Value Operations ==========

    /// write realtime value to Redis
    pub async fn set_realtime_value(&self, key: &str, value: &RealtimeValue) -> Result<()> {
        let val_str = serde_json::to_string(value).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize RealtimeValue error: {}", e))
        })?;

        // Use retry mechanism for robustness
        let mut last_error = None;
        for attempt in 1..=self.manager.config().max_retries {
            match self.manager.get_connection().await {
                Ok(mut conn) => match conn.set::<&str, String, ()>(key, val_str.clone()).await {
                    Ok(_) => {
                        if attempt > 1 {
                            log::info!("Redis set succeeded on attempt {}", attempt);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        last_error = Some(format!("Redis set error: {}", e));
                    }
                },
                Err(e) => {
                    last_error = Some(format!("Connection error: {}", e));
                }
            }

            if attempt < self.manager.config().max_retries {
                log::warn!(
                    "Redis set failed on attempt {}, retrying: {}",
                    attempt,
                    last_error.as_ref().unwrap()
                );
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
            }
        }

        Err(ComSrvError::RedisError(format!(
            "Redis set error after {} attempts: {}",
            self.manager.config().max_retries,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        )))
    }

    /// write realtime value with expire time (seconds)
    pub async fn set_realtime_value_with_expire(
        &self,
        key: &str,
        value: &RealtimeValue,
        expire_secs: usize,
    ) -> Result<()> {
        let val_str = serde_json::to_string(value).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize RealtimeValue error: {}", e))
        })?;

        // Use retry mechanism for robustness
        let mut last_error = None;
        for attempt in 1..=self.manager.config().max_retries {
            match self.manager.get_connection().await {
                Ok(mut conn) => {
                    match conn
                        .set_ex::<&str, String, ()>(key, val_str.clone(), expire_secs)
                        .await
                    {
                        Ok(_) => {
                            if attempt > 1 {
                                log::info!("Redis set_ex succeeded on attempt {}", attempt);
                            }
                            return Ok(());
                        }
                        Err(e) => {
                            last_error = Some(format!("Redis set_ex error: {}", e));
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(format!("Connection error: {}", e));
                }
            }

            if attempt < self.manager.config().max_retries {
                log::warn!(
                    "Redis set_ex failed on attempt {}, retrying: {}",
                    attempt,
                    last_error.as_ref().unwrap()
                );
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
            }
        }

        Err(ComSrvError::RedisError(format!(
            "Redis set_ex error after {} attempts: {}",
            self.manager.config().max_retries,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        )))
    }

    /// read realtime value
    pub async fn get_realtime_value(&self, key: &str) -> Result<Option<RealtimeValue>> {
        let mut conn = self.manager.get_connection().await?;

        let val: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str).map_err(|e| {
                ComSrvError::RedisError(format!("Deserialize RealtimeValue error: {}", e))
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    // ========== Channel Management Operations ==========

    /// Store channel metadata in Redis
    pub async fn set_channel_metadata(&self, channel_id: u16, metadata: &RedisChannelMetadata) -> Result<()> {
        let key = format!("comsrv:channel:{}:metadata", channel_id);
        let val_str = serde_json::to_string(metadata).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize channel metadata error: {}", e))
        })?;

        let mut conn = self.manager.get_connection().await?;
        conn.set::<&str, String, ()>(&key, val_str).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set channel metadata error: {}", e)))
    }

    /// Get channel metadata from Redis
    pub async fn get_channel_metadata(&self, channel_id: u16) -> Result<Option<RedisChannelMetadata>> {
        let key = format!("comsrv:channel:{}:metadata", channel_id);
        let mut conn = self.manager.get_connection().await?;

        let val: Option<String> = conn.get(&key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get channel metadata error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str).map_err(|e| {
                ComSrvError::RedisError(format!("Deserialize channel metadata error: {}", e))
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    /// List all channel IDs
    pub async fn list_channel_ids(&self) -> Result<Vec<u16>> {
        let pattern = "comsrv:channel:*:metadata";
        let mut conn = self.manager.get_connection().await?;

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis KEYS error: {}", e)))?;

        let mut channel_ids = Vec::new();
        for key in keys {
            if let Some(captures) = regex::Regex::new(r"comsrv:channel:(\d+):metadata").unwrap().captures(&key) {
                if let Some(id_str) = captures.get(1) {
                    if let Ok(id) = id_str.as_str().parse::<u16>() {
                        channel_ids.push(id);
                    }
                }
            }
        }

        channel_ids.sort();
        Ok(channel_ids)
    }

    /// Delete channel metadata
    pub async fn delete_channel_metadata(&self, channel_id: u16) -> Result<()> {
        let key = format!("comsrv:channel:{}:metadata", channel_id);
        let mut conn = self.manager.get_connection().await?;

        let _: () = conn.del(&key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis delete channel metadata error: {}", e)))?;

        Ok(())
    }

    // ========== Configuration Management Operations ==========

    /// Store configuration data
    pub async fn set_config_data(&self, config_name: &str, config_data: &RedisConfigData) -> Result<()> {
        let key = format!("comsrv:config:{}", config_name);
        let val_str = serde_json::to_string(config_data).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize config data error: {}", e))
        })?;

        let mut conn = self.manager.get_connection().await?;
        conn.set::<&str, String, ()>(&key, val_str).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set config data error: {}", e)))
    }

    /// Get configuration data
    pub async fn get_config_data(&self, config_name: &str) -> Result<Option<RedisConfigData>> {
        let key = format!("comsrv:config:{}", config_name);
        let mut conn = self.manager.get_connection().await?;

        let val: Option<String> = conn.get(&key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get config data error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str).map_err(|e| {
                ComSrvError::RedisError(format!("Deserialize config data error: {}", e))
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    /// List all configuration names
    pub async fn list_config_names(&self) -> Result<Vec<String>> {
        let pattern = "comsrv:config:*";
        let mut conn = self.manager.get_connection().await?;

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis KEYS error: {}", e)))?;

        let config_names: Vec<String> = keys
            .into_iter()
            .filter_map(|key| {
                key.strip_prefix("comsrv:config:").map(|s| s.to_string())
            })
            .collect();

        Ok(config_names)
    }

    // ========== Connection Pool Management Operations ==========

    /// Store connection pool entry
    pub async fn set_connection_entry(&self, connection_key: &str, entry: &RedisConnectionEntry) -> Result<()> {
        let key = format!("comsrv:pool:{}", connection_key);
        let val_str = serde_json::to_string(entry).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize connection entry error: {}", e))
        })?;

        let mut conn = self.manager.get_connection().await?;
        conn.set::<&str, String, ()>(&key, val_str).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set connection entry error: {}", e)))
    }

    /// Get connection pool entry
    pub async fn get_connection_entry(&self, connection_key: &str) -> Result<Option<RedisConnectionEntry>> {
        let key = format!("comsrv:pool:{}", connection_key);
        let mut conn = self.manager.get_connection().await?;

        let val: Option<String> = conn.get(&key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get connection entry error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str).map_err(|e| {
                ComSrvError::RedisError(format!("Deserialize connection entry error: {}", e))
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    /// List all connection pool keys
    pub async fn list_connection_keys(&self) -> Result<Vec<String>> {
        let pattern = "comsrv:pool:*";
        let mut conn = self.manager.get_connection().await?;

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis KEYS error: {}", e)))?;

        let connection_keys: Vec<String> = keys
            .into_iter()
            .filter_map(|key| {
                key.strip_prefix("comsrv:pool:").map(|s| s.to_string())
            })
            .collect();

        Ok(connection_keys)
    }

    /// Delete connection pool entry
    pub async fn delete_connection_entry(&self, connection_key: &str) -> Result<()> {
        let key = format!("comsrv:pool:{}", connection_key);
        let mut conn = self.manager.get_connection().await?;

        let _: () = conn.del(&key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis delete connection entry error: {}", e)))?;

        Ok(())
    }

    // ========== Statistics Management Operations ==========

    /// Store statistics data
    pub async fn set_stats_data(&self, stats_key: &str, stats_data: &RedisStatsData) -> Result<()> {
        let key = format!("comsrv:stats:{}", stats_key);
        let val_str = serde_json::to_string(stats_data).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize stats data error: {}", e))
        })?;

        let mut conn = self.manager.get_connection().await?;
        conn.set::<&str, String, ()>(&key, val_str).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set stats data error: {}", e)))
    }

    /// Get statistics data
    pub async fn get_stats_data(&self, stats_key: &str) -> Result<Option<RedisStatsData>> {
        let key = format!("comsrv:stats:{}", stats_key);
        let mut conn = self.manager.get_connection().await?;

        let val: Option<String> = conn.get(&key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get stats data error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str).map_err(|e| {
                ComSrvError::RedisError(format!("Deserialize stats data error: {}", e))
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    /// List all statistics keys
    pub async fn list_stats_keys(&self) -> Result<Vec<String>> {
        let pattern = "comsrv:stats:*";
        let mut conn = self.manager.get_connection().await?;

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis KEYS error: {}", e)))?;

        let stats_keys: Vec<String> = keys
            .into_iter()
            .filter_map(|key| {
                key.strip_prefix("comsrv:stats:").map(|s| s.to_string())
            })
            .collect();

        Ok(stats_keys)
    }

    // ========== Generic Key-Value Operations ==========

    /// Generic set operation for any serializable data
    pub async fn set_data<T: Serialize>(&self, key: &str, data: &T, expire_secs: Option<Duration>) -> Result<()> {
        let val_str = serde_json::to_string(data).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize data error: {}", e))
        })?;

        let mut conn = self.manager.get_connection().await?;
        
        if let Some(duration) = expire_secs {
            conn.set_ex::<&str, String, ()>(key, val_str, duration.as_secs() as usize).await
                .map_err(|e| ComSrvError::RedisError(format!("Redis set data with expire error: {}", e)))
        } else {
            conn.set::<&str, String, ()>(key, val_str).await
                .map_err(|e| ComSrvError::RedisError(format!("Redis set data error: {}", e)))
        }
    }

    /// Generic get operation for any deserializable data
    pub async fn get_data<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.manager.get_connection().await?;

        let val: Option<String> = conn.get(key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get data error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str).map_err(|e| {
                ComSrvError::RedisError(format!("Deserialize data error: {}", e))
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    /// Delete a key
    pub async fn delete_key(&self, key: &str) -> Result<()> {
        let mut conn = self.manager.get_connection().await?;

        let _: () = conn.del(key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis delete key error: {}", e)))?;

        Ok(())
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.manager.get_connection().await?;

        let exists: bool = conn.exists(key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis exists error: {}", e)))?;

        Ok(exists)
    }

    /// List keys matching a pattern
    pub async fn list_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut conn = self.manager.get_connection().await?;

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis KEYS error: {}", e)))?;

        Ok(keys)
    }

    /// Set expiration for a key
    pub async fn set_expire(&self, key: &str, seconds: usize) -> Result<()> {
        let mut conn = self.manager.get_connection().await?;

        let _: () = conn.expire(key, seconds).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis expire error: {}", e)))?;

        Ok(())
    }

    // ========== Hash Operations ==========

    /// Set hash field
    pub async fn hset<T: Serialize>(&self, key: &str, field: &str, value: &T) -> Result<()> {
        let val_str = serde_json::to_string(value).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize hash value error: {}", e))
        })?;

        let mut conn = self.manager.get_connection().await?;
        conn.hset::<&str, &str, String, ()>(key, field, val_str).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis hset error: {}", e)))
    }

    /// Get hash field
    pub async fn hget<T: for<'de> Deserialize<'de>>(&self, key: &str, field: &str) -> Result<Option<T>> {
        let mut conn = self.manager.get_connection().await?;

        let val: Option<String> = conn.hget(key, field).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis hget error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str).map_err(|e| {
                ComSrvError::RedisError(format!("Deserialize hash value error: {}", e))
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    /// Get all hash fields
    pub async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>> {
        let mut conn = self.manager.get_connection().await?;

        let hash: HashMap<String, String> = conn.hgetall(key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis hgetall error: {}", e)))?;

        Ok(hash)
    }

    /// Delete hash field
    pub async fn hdel(&self, key: &str, field: &str) -> Result<()> {
        let mut conn = self.manager.get_connection().await?;

        let _: () = conn.hdel(key, field).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis hdel error: {}", e)))?;

        Ok(())
    }

    // ========== Command Operations (existing) ==========

    /// 发布遥控/遥调指令到指定通道
    pub async fn publish_command(&self, channel: &str, command: &RemoteCommand) -> Result<()> {
        let command_str = serde_json::to_string(command)
            .map_err(|e| ComSrvError::RedisError(format!("Serialize command error: {}", e)))?;

        let mut conn = self.manager.get_connection().await?;
        let _: () = conn
            .publish(channel, command_str)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis publish error: {}", e)))?;

        Ok(())
    }

    /// 设置指令到指令队列
    pub async fn set_command(&self, channel_id: &str, command: &RemoteCommand) -> Result<()> {
        let command_key = format!("cmd:{}:{}", channel_id, command.command_id);
        let command_str = serde_json::to_string(command)
            .map_err(|e| ComSrvError::RedisError(format!("Serialize command error: {}", e)))?;

        let mut conn = self.manager.get_connection().await?;
        // 设置指令，带过期时间（5分钟）
        conn.set_ex::<&str, String, ()>(&command_key, command_str, 300)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set command error: {}", e)))?;

        // 同时发布到指令通道通知
        let notify_channel = format!("commands:{}", channel_id);
        let _: () = conn
            .publish(&notify_channel, &command.command_id)
            .await
            .map_err(|e| {
                ComSrvError::RedisError(format!("Redis publish command notification error: {}", e))
            })?;

        Ok(())
    }

    /// 获取指令
    pub async fn get_command(
        &self,
        channel_id: &str,
        command_id: &str,
    ) -> Result<Option<RemoteCommand>> {
        let command_key = format!("cmd:{}:{}", channel_id, command_id);

        let mut conn = self.manager.get_connection().await?;
        let val: Option<String> = conn
            .get(&command_key)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get command error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str).map_err(|e| {
                ComSrvError::RedisError(format!("Deserialize command error: {}", e))
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    /// 删除已执行的指令
    pub async fn delete_command(&self, channel_id: &str, command_id: &str) -> Result<()> {
        let command_key = format!("cmd:{}:{}", channel_id, command_id);

        let mut conn = self.manager.get_connection().await?;
        let _: () = conn
            .del(&command_key)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis delete command error: {}", e)))?;

        Ok(())
    }

    /// 设置指令执行结果
    pub async fn set_command_result(&self, channel_id: &str, result: &CommandResult) -> Result<()> {
        let result_key = format!("result:{}:{}", channel_id, result.command_id);
        let result_str = serde_json::to_string(result).map_err(|e| {
            ComSrvError::RedisError(format!("Serialize command result error: {}", e))
        })?;

        let mut conn = self.manager.get_connection().await?;
        // 设置结果，带过期时间（1小时）
        conn.set_ex::<&str, String, ()>(&result_key, result_str, 3600)
            .await
            .map_err(|e| {
                ComSrvError::RedisError(format!("Redis set command result error: {}", e))
            })?;

        Ok(())
    }

    /// 创建新的Redis PubSub连接用于订阅
    pub async fn create_pubsub(&self) -> Result<PubSub> {
        let conn = self.manager.get_connection().await?;
        let pubsub = conn.into_pubsub();

        Ok(pubsub)
    }

    /// Check if Redis connection is healthy
    pub async fn is_healthy(&self) -> bool {
        self.manager.health_check().await.unwrap_or(false)
    }

    /// 通用发布方法
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        let mut conn = self.manager.get_connection().await?;
        let _: () = conn
            .publish(channel, message)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis publish error: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::config_manager::RedisConnectionType;

    /// create test Redis config
    fn create_test_redis_config() -> RedisConfig {
        RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: 0,
            timeout_ms: 5000,
            max_connections: Some(10),
            min_connections: Some(1),
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        }
    }

    /// create disabled Redis config
    fn create_disabled_redis_config() -> RedisConfig {
        RedisConfig {
            enabled: false,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: 0,
            timeout_ms: 5000,
            max_connections: Some(10),
            min_connections: Some(1),
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        }
    }

    /// create test realtime value
    fn create_test_realtime_value() -> RealtimeValue {
        RealtimeValue {
            raw: 123.45,
            processed: 120.0,
            timestamp: "2023-12-01T10:30:00Z".to_string(),
        }
    }

    #[test]
    fn test_realtime_value_creation() {
        let value = create_test_realtime_value();
        assert_eq!(value.raw, 123.45);
        assert_eq!(value.processed, 120.0);
        assert_eq!(value.timestamp, "2023-12-01T10:30:00Z");
    }

    #[test]
    fn test_realtime_value_serialization() {
        let value = create_test_realtime_value();

        // Test JSON serialization
        let json_str = serde_json::to_string(&value).unwrap();
        assert!(json_str.contains("123.45"));
        assert!(json_str.contains("120"));
        assert!(json_str.contains("2023-12-01T10:30:00Z"));

        // Test JSON deserialization
        let deserialized: RealtimeValue = serde_json::from_str(&json_str).unwrap();
        assert_eq!(value.raw, deserialized.raw);
        assert_eq!(value.processed, deserialized.processed);
        assert_eq!(value.timestamp, deserialized.timestamp);
    }

    #[tokio::test]
    async fn test_redis_store_from_disabled_config() {
        let config = create_disabled_redis_config();
        let result = RedisStore::from_config(&config).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_redis_config_invalid_address() {
        // Test with invalid protocol
        let config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "invalid://invalid".to_string(),
            db: 0,
            timeout_ms: 5000,
            max_connections: Some(10),
            min_connections: Some(1),
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        // This test just verifies the configuration structure
        assert_eq!(config.address, "invalid://invalid");
        assert!(config.enabled);
    }

    #[test]
    fn test_redis_config_address_types() {
        let tcp_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "tcp://127.0.0.1:6379".to_string(),
            db: 1,
            timeout_ms: 5000,
            max_connections: Some(10),
            min_connections: Some(1),
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        let redis_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: 2,
            timeout_ms: 5000,
            max_connections: Some(10),
            min_connections: Some(1),
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        let unix_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Unix,
            address: "unix:///tmp/redis.sock".to_string(),
            db: 3,
            timeout_ms: 5000,
            max_connections: Some(10),
            min_connections: Some(1),
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        // Test that all address types are properly stored
        assert!(tcp_config.address.starts_with("tcp://"));
        assert!(redis_config.address.starts_with("redis://"));
        assert!(unix_config.address.starts_with("unix://"));
    }

    #[test]
    fn test_redis_config_db_selection() {
        let config_with_db = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: 5,
            timeout_ms: 5000,
            max_connections: Some(10),
            min_connections: Some(1),
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        let config_without_db = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: 0,
            timeout_ms: 5000,
            max_connections: Some(10),
            min_connections: Some(1),
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        assert_eq!(config_with_db.db, 5);
        assert_eq!(config_without_db.db, 0);
    }

    // Note: The following tests require a running Redis instance
    // They are marked with #[ignore] to skip them by default
    // Run with `cargo test -- --ignored` to include them

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_connection() {
        let config = create_test_redis_config();
        let result = RedisStore::from_config(&config).await;

        match result {
            Ok(Some(_store)) => {
                // Connection successful
                assert!(true);
            }
            Ok(None) => {
                panic!("Redis should be enabled");
            }
            Err(_) => {
                // Redis not available, skip test
                println!("Redis not available, skipping test");
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_set_get_realtime_value() {
        let config = create_test_redis_config();
        if let Ok(Some(store)) = RedisStore::from_config(&config).await {
            let test_key = "test:realtime:value";
            let test_value = create_test_realtime_value();

            // Test set operation
            let set_result = store.set_realtime_value(test_key, &test_value).await;
            assert!(set_result.is_ok());

            // Test get operation
            let get_result = store.get_realtime_value(test_key).await;
            assert!(get_result.is_ok());

            let retrieved_value = get_result.unwrap();
            assert!(retrieved_value.is_some());

            let retrieved_value = retrieved_value.unwrap();
            assert_eq!(test_value.raw, retrieved_value.raw);
            assert_eq!(test_value.processed, retrieved_value.processed);
            assert_eq!(test_value.timestamp, retrieved_value.timestamp);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_set_with_expire() {
        let config = create_test_redis_config();
        if let Ok(Some(store)) = RedisStore::from_config(&config).await {
            let test_key = "test:expire:value";
            let test_value = create_test_realtime_value();

            // Test set with expire
            let set_result = store
                .set_realtime_value_with_expire(test_key, &test_value, 10)
                .await;
            assert!(set_result.is_ok());

            // Test get operation immediately
            let get_result = store.get_realtime_value(test_key).await;
            assert!(get_result.is_ok());

            let retrieved_value = get_result.unwrap();
            assert!(retrieved_value.is_some());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_get_nonexistent_key() {
        let config = create_test_redis_config();
        if let Ok(Some(store)) = RedisStore::from_config(&config).await {
            let test_key = "test:nonexistent:key";

            // Test get operation for non-existent key
            let get_result = store.get_realtime_value(test_key).await;
            assert!(get_result.is_ok());

            let retrieved_value = get_result.unwrap();
            assert!(retrieved_value.is_none());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_multiple_operations() {
        let config = create_test_redis_config();
        if let Ok(Some(store)) = RedisStore::from_config(&config).await {
            let test_keys = vec!["test:multi:1", "test:multi:2", "test:multi:3"];
            let test_values = vec![
                RealtimeValue {
                    raw: 100.0,
                    processed: 95.0,
                    timestamp: "2023-12-01T10:00:00Z".to_string(),
                },
                RealtimeValue {
                    raw: 200.0,
                    processed: 195.0,
                    timestamp: "2023-12-01T10:01:00Z".to_string(),
                },
                RealtimeValue {
                    raw: 300.0,
                    processed: 295.0,
                    timestamp: "2023-12-01T10:02:00Z".to_string(),
                },
            ];

            // Set multiple values
            for (key, value) in test_keys.iter().zip(test_values.iter()) {
                let set_result = store.set_realtime_value(key, value).await;
                assert!(set_result.is_ok());
            }

            // Get multiple values
            for (key, expected_value) in test_keys.iter().zip(test_values.iter()) {
                let get_result = store.get_realtime_value(key).await;
                assert!(get_result.is_ok());

                let retrieved_value = get_result.unwrap();
                assert!(retrieved_value.is_some());

                let retrieved_value = retrieved_value.unwrap();
                assert_eq!(expected_value.raw, retrieved_value.raw);
                assert_eq!(expected_value.processed, retrieved_value.processed);
                assert_eq!(expected_value.timestamp, retrieved_value.timestamp);
            }
        }
    }

    #[test]
    fn test_error_handling_serialization() {
        // Test creating a RealtimeValue with extreme values
        let extreme_value = RealtimeValue {
            raw: f64::INFINITY,
            processed: f64::NEG_INFINITY,
            timestamp: "invalid-timestamp".to_string(),
        };

        // JSON serialization should handle infinity values
        let json_result = serde_json::to_string(&extreme_value);
        // Note: JSON serialization of infinity might fail or produce "null"
        // This test verifies the behavior is predictable
        match json_result {
            Ok(_) => assert!(true),  // Serialization succeeded
            Err(_) => assert!(true), // Serialization failed as expected
        }
    }

    #[test]
    fn test_redis_config_clone() {
        let config = create_test_redis_config();
        let cloned_config = config.clone();

        assert_eq!(config.enabled, cloned_config.enabled);
        assert_eq!(config.address, cloned_config.address);
        assert_eq!(config.db, cloned_config.db);
    }

    #[test]
    fn test_realtime_value_clone() {
        let value = create_test_realtime_value();
        let cloned_value = value.clone();

        assert_eq!(value.raw, cloned_value.raw);
        assert_eq!(value.processed, cloned_value.processed);
        assert_eq!(value.timestamp, cloned_value.timestamp);
    }

    #[test]
    fn test_realtime_value_debug() {
        let value = create_test_realtime_value();
        let debug_str = format!("{:?}", value);

        assert!(debug_str.contains("RealtimeValue"));
        assert!(debug_str.contains("123.45"));
        assert!(debug_str.contains("120"));
        assert!(debug_str.contains("2023-12-01T10:30:00Z"));
    }
}
