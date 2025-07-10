//! Optimized Redis storage with separated config and realtime data
//!
//! This module implements the new storage pattern that separates static
//! configuration from dynamic real-time values for better performance.

use crate::core::framework::realtime_data::{
    ChannelConfig, PointConfig, RealtimeBatch, RealtimeValue,
};
use crate::utils::error::{ComSrvError, Result};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Pipeline};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Optimized Redis storage manager
pub struct OptimizedRedisStorage {
    /// Redis connection manager
    conn: ConnectionManager,
    /// Cached channel configurations
    config_cache: Arc<RwLock<HashMap<u16, ChannelConfig>>>,
    /// Configuration TTL in seconds
    config_ttl: u64,
}

impl OptimizedRedisStorage {
    /// Create new optimized storage
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| ComSrvError::Storage(format!("Failed to create Redis client: {}", e)))?;

        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self {
            conn,
            config_cache: Arc::new(RwLock::new(HashMap::new())),
            config_ttl: 3600, // 1 hour default
        })
    }

    /// Store channel configuration
    pub async fn store_channel_config(&mut self, config: &ChannelConfig) -> Result<()> {
        let key = format!("comsrv:config:channel:{}:points", config.channel_id);
        let fields = config
            .to_redis_fields()
            .map_err(|e| ComSrvError::Storage(format!("Failed to serialize config: {}", e)))?;

        // Store in Redis
        let mut pipe = Pipeline::new();
        for (field, value) in fields {
            pipe.hset(&key, field, value);
        }
        pipe.expire(&key, self.config_ttl as i64);

        pipe.query_async(&mut self.conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to store config: {}", e)))?;

        // Update cache
        let mut cache = self.config_cache.write().await;
        cache.insert(config.channel_id, config.clone());

        info!(
            "Stored configuration for channel {} with {} points",
            config.channel_id,
            config.points.len()
        );

        Ok(())
    }

    /// Get channel configuration (with caching)
    pub async fn get_channel_config(&mut self, channel_id: u16) -> Result<Option<ChannelConfig>> {
        // Check cache first
        {
            let cache = self.config_cache.read().await;
            if let Some(config) = cache.get(&channel_id) {
                return Ok(Some(config.clone()));
            }
        }

        // Load from Redis
        let key = format!("comsrv:config:channel:{}:points", channel_id);
        let data: HashMap<String, String> = self
            .conn
            .hgetall(&key)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get config: {}", e)))?;

        if data.is_empty() {
            return Ok(None);
        }

        // Parse configuration
        let mut config = ChannelConfig::new(channel_id, format!("Channel {}", channel_id));
        for (point_id, json) in data.into_iter() {
            match serde_json::from_str::<PointConfig>(&json) {
                Ok(point_config) => {
                    config.add_point(point_id, point_config);
                }
                Err(e) => {
                    warn!("Failed to parse point config: {}", e);
                }
            }
        }

        // Update cache
        let mut cache = self.config_cache.write().await;
        cache.insert(channel_id, config.clone());

        Ok(Some(config))
    }

    /// Store real-time values batch
    pub async fn store_realtime_batch(&mut self, batch: &RealtimeBatch) -> Result<()> {
        let key = format!("comsrv:realtime:channel:{}", batch.channel_id);
        let fields = batch
            .to_redis_fields()
            .map_err(|e| ComSrvError::Storage(format!("Failed to serialize batch: {}", e)))?;

        if fields.is_empty() {
            return Ok(());
        }

        // Use pipeline for atomic update
        let mut pipe = Pipeline::new();
        for (field, value) in fields {
            pipe.hset(&key, &field, &value);
        }

        let start = std::time::Instant::now();
        pipe.query_async(&mut self.conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to store realtime data: {}", e)))?;

        let elapsed = start.elapsed();
        debug!(
            "Stored {} realtime values for channel {} in {:?}",
            batch.len(),
            batch.channel_id,
            elapsed
        );

        Ok(())
    }

    /// Get real-time values for a channel
    pub async fn get_realtime_channel(
        &mut self,
        channel_id: u16,
    ) -> Result<HashMap<String, RealtimeValue>> {
        let key = format!("comsrv:realtime:channel:{}", channel_id);
        let data: HashMap<String, String> = self
            .conn
            .hgetall(&key)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get realtime data: {}", e)))?;

        let mut values = HashMap::new();
        for (point_id, json) in data.into_iter() {
            match RealtimeValue::from_json(&json) {
                Ok(value) => {
                    values.insert(point_id, value);
                }
                Err(e) => {
                    warn!("Failed to parse realtime value: {}", e);
                }
            }
        }

        Ok(values)
    }

    /// Get specific points from a channel
    pub async fn get_realtime_points(
        &mut self,
        channel_id: u16,
        point_ids: &[String],
    ) -> Result<HashMap<String, RealtimeValue>> {
        if point_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let key = format!("comsrv:realtime:channel:{}", channel_id);
        let mut cmd = redis::cmd("HMGET");
        cmd.arg(&key);
        for id in point_ids {
            cmd.arg(id);
        }
        let data: Vec<Option<String>> = cmd
            .query_async(&mut self.conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get points: {}", e)))?;

        let mut values = HashMap::new();
        for (i, maybe_json) in data.iter().enumerate() {
            if let Some(json) = maybe_json {
                match RealtimeValue::from_json(json) {
                    Ok(value) => {
                        values.insert(point_ids[i].clone(), value);
                    }
                    Err(e) => {
                        warn!("Failed to parse value for point {}: {}", point_ids[i], e);
                    }
                }
            }
        }

        Ok(values)
    }

    /// Get merged data (config + realtime) for API responses
    pub async fn get_merged_channel_data(
        &mut self,
        channel_id: u16,
    ) -> Result<Vec<serde_json::Value>> {
        // Get configuration
        let config = self.get_channel_config(channel_id).await?;
        if config.is_none() {
            return Ok(Vec::new());
        }
        let config = config.unwrap();

        // Get realtime values
        let values = self.get_realtime_channel(channel_id).await?;

        // Merge data
        let mut result = Vec::new();
        for (point_id, point_config) in &config.points {
            let mut data = serde_json::json!({
                "id": point_id,
                "name": point_config.name,
                "unit": point_config.unit,
                "type": point_config.telemetry_type,
                "description": point_config.description,
            });

            // Add realtime value if available
            if let Some(realtime) = values.get(point_id) {
                data["raw"] = serde_json::json!(realtime.raw);
                data["value"] = serde_json::json!(realtime.value);
                data["timestamp"] = serde_json::json!(realtime.timestamp());
            } else {
                data["value"] = serde_json::json!(null);
                data["raw"] = serde_json::json!(null);
                data["timestamp"] = serde_json::json!(null);
            }

            result.push(data);
        }

        Ok(result)
    }

    /// Clear configuration cache
    pub async fn clear_config_cache(&self) {
        let mut cache = self.config_cache.write().await;
        cache.clear();
        info!("Configuration cache cleared");
    }

    /// Get storage statistics
    pub async fn get_stats(&mut self) -> Result<StorageStats> {
        // Get number of channels
        let channel_pattern = "comsrv:realtime:channel:*";
        let channels: Vec<String> = self
            .conn
            .keys(channel_pattern)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get channels: {}", e)))?;

        // Get cache size
        let cache_size = self.config_cache.read().await.len();

        // Get memory usage (approximate)
        let info: String = redis::cmd("INFO")
            .arg("memory")
            .query_async(&mut self.conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get Redis info: {}", e)))?;

        let memory_used = info
            .lines()
            .find(|line| line.starts_with("used_memory:"))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|val| val.parse::<u64>().ok())
            .unwrap_or(0);

        Ok(StorageStats {
            active_channels: channels.len(),
            cached_configs: cache_size,
            memory_used_bytes: memory_used,
        })
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub active_channels: usize,
    pub cached_configs: usize,
    pub memory_used_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_storage() {
        // This would require a test Redis instance
        // Skipping for now
    }
}
