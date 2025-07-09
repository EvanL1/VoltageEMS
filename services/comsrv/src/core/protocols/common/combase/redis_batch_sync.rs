//! Redis Batch Synchronization Module
//!
//! Efficient batch operations for syncing point data to Redis

use redis::aio::MultiplexedConnection;
use redis::Pipeline;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, Duration};
use tracing::{error, info};

use super::data_types::PointData;
use super::optimized_point_manager::OptimizedPointManager;
use crate::utils::Result;

/// Configuration for Redis batch sync
#[derive(Clone, Debug)]
pub struct RedisBatchSyncConfig {
    /// Maximum number of points to sync in one batch
    pub batch_size: usize,
    /// Interval between sync operations
    pub sync_interval: Duration,
    /// Redis key prefix for points
    pub key_prefix: String,
    /// TTL for point data in Redis (None = no expiry)
    pub point_ttl: Option<Duration>,
    /// Enable pipeline mode for better performance
    pub use_pipeline: bool,
}

impl Default for RedisBatchSyncConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            sync_interval: Duration::from_millis(100),
            key_prefix: "comsrv:points".to_string(),
            point_ttl: None,
            use_pipeline: true,
        }
    }
}

/// Redis batch synchronizer for point data
pub struct RedisBatchSync {
    /// Redis connection manager
    redis_conn: Arc<Mutex<MultiplexedConnection>>,
    /// Configuration
    config: RedisBatchSyncConfig,
    /// Buffer for pending updates
    update_buffer: Arc<RwLock<HashMap<u32, PointData>>>,
    /// Statistics
    stats: Arc<RwLock<RedisSyncStats>>,
}

impl std::fmt::Debug for RedisBatchSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisBatchSync")
            .field("redis_conn", &"<MultiplexedConnection>")
            .field("config", &self.config)
            .field("update_buffer", &"<buffer>")
            .field("stats", &self.stats)
            .finish()
    }
}

#[derive(Default, Clone, Debug)]
pub struct RedisSyncStats {
    pub total_synced: u64,
    pub batch_count: u64,
    pub failed_syncs: u64,
    pub last_sync_time: Option<chrono::DateTime<chrono::Utc>>,
    pub average_batch_time_ms: f64,
}

impl RedisBatchSync {
    /// Create a new Redis batch synchronizer
    pub fn new(redis_conn: MultiplexedConnection, config: RedisBatchSyncConfig) -> Self {
        Self {
            redis_conn: Arc::new(Mutex::new(redis_conn)),
            config,
            update_buffer: Arc::new(RwLock::new(HashMap::with_capacity(10000))),
            stats: Arc::new(RwLock::new(RedisSyncStats::default())),
        }
    }

    /// Start the background sync task
    pub fn start_sync_task(self: Arc<Self>, point_manager: Arc<OptimizedPointManager>) {
        tokio::spawn(async move {
            let mut sync_interval = interval(self.config.sync_interval);

            loop {
                sync_interval.tick().await;

                if let Err(e) = self.sync_batch(&point_manager).await {
                    error!("Redis sync error: {e}");
                    self.stats.write().await.failed_syncs += 1;
                }
            }
        });
    }

    /// Add points to the update buffer
    pub async fn buffer_updates(&self, updates: Vec<(u32, PointData)>) {
        let mut buffer = self.update_buffer.write().await;
        for (id, data) in updates {
            buffer.insert(id, data);
        }
    }

    /// Sync a batch of points to Redis
    async fn sync_batch(&self, point_manager: &Arc<OptimizedPointManager>) -> Result<()> {
        let start_time = std::time::Instant::now();

        // Get all current point data
        let all_data = point_manager.get_all_point_data().await;

        // Take updates from buffer
        let mut buffer = self.update_buffer.write().await;
        let updates: Vec<(u32, PointData)> = buffer.drain().collect();
        drop(buffer);

        // Combine with current data
        let mut to_sync = HashMap::new();
        for data in all_data {
            if let Ok(id) = data.id.parse::<u32>() {
                to_sync.insert(id, data);
            }
        }
        for (id, data) in updates {
            to_sync.insert(id, data);
        }

        if to_sync.is_empty() {
            return Ok(());
        }

        // Sync to Redis
        let mut conn = self.redis_conn.lock().await;

        if self.config.use_pipeline {
            self.sync_with_pipeline(&mut conn, to_sync).await?;
        } else {
            self.sync_individually(&mut conn, to_sync).await?;
        }

        // Update stats
        let elapsed = start_time.elapsed();
        let mut stats = self.stats.write().await;
        stats.batch_count += 1;
        stats.last_sync_time = Some(chrono::Utc::now());
        stats.average_batch_time_ms = (stats.average_batch_time_ms
            * (stats.batch_count - 1) as f64
            + elapsed.as_millis() as f64)
            / stats.batch_count as f64;

        Ok(())
    }

    /// Sync using Redis pipeline for better performance
    async fn sync_with_pipeline(
        &self,
        conn: &mut MultiplexedConnection,
        points: HashMap<u32, PointData>,
    ) -> Result<()> {
        let mut pipe = Pipeline::new();
        let mut count = 0;

        for (id, data) in points.iter() {
            let key = format!("{}:{id}", self.config.key_prefix);
            let value = json!({
                "id": data.id,
                "name": data.name,
                "value": data.value,
                "unit": data.unit,
                "timestamp": data.timestamp.to_rfc3339(),
                "description": data.description,
            });

            pipe.set(&key, value.to_string());

            if let Some(ttl) = self.config.point_ttl {
                pipe.expire(&key, ttl.as_secs() as usize);
            }

            count += 1;

            // Execute pipeline in batches
            if count >= self.config.batch_size {
                pipe.query_async::<_, ()>(conn)
                    .await
                    .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

                self.stats.write().await.total_synced += count as u64;

                pipe = Pipeline::new();
                count = 0;
            }
        }

        // Execute remaining items
        if count > 0 {
            pipe.query_async::<_, ()>(conn)
                .await
                .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

            self.stats.write().await.total_synced += count as u64;
        }

        info!("Synced {} points to Redis using pipeline", points.len());

        Ok(())
    }

    /// Sync points individually (fallback method)
    async fn sync_individually(
        &self,
        conn: &mut MultiplexedConnection,
        points: HashMap<u32, PointData>,
    ) -> Result<()> {
        let mut count = 0;

        for (id, data) in points {
            let key = format!("{}:{id}", self.config.key_prefix);
            let value = json!({
                "id": data.id,
                "name": data.name,
                "value": data.value,
                "unit": data.unit,
                "timestamp": data.timestamp.to_rfc3339(),
                "description": data.description,
            });

            let _: () = redis::cmd("SET")
                .arg(&key)
                .arg(value.to_string())
                .query_async(conn)
                .await
                .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

            if let Some(ttl) = self.config.point_ttl {
                let _: () = redis::cmd("EXPIRE")
                    .arg(&key)
                    .arg(ttl.as_secs())
                    .query_async(conn)
                    .await
                    .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;
            }

            count += 1;
        }

        self.stats.write().await.total_synced += count;
        info!("Synced {} points to Redis individually", count);

        Ok(())
    }

    /// Get sync statistics
    pub async fn get_stats(&self) -> RedisSyncStats {
        self.stats.read().await.clone()
    }

    /// Create indices in Redis for fast lookups
    pub async fn create_redis_indices(
        &self,
        point_manager: &Arc<OptimizedPointManager>,
    ) -> Result<()> {
        let mut conn = self.redis_conn.lock().await;

        // Create sets for different point types
        let all_configs = point_manager.get_all_point_configs().await;
        let mut type_sets: HashMap<String, Vec<String>> = HashMap::new();

        for config in all_configs {
            let type_key = format!(
                "{}:type:{:?}",
                self.config.key_prefix, config.telemetry_type
            );
            type_sets
                .entry(type_key)
                .or_default()
                .push(config.point_id.to_string());
        }

        // Store type sets in Redis
        for (key, members) in type_sets {
            let _: () = redis::cmd("SADD")
                .arg(&key)
                .arg(members)
                .query_async(&mut *conn)
                .await
                .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;
        }

        info!("Created Redis indices for point lookups");
        Ok(())
    }

    /// Scan Redis for existing points (using SCAN instead of KEYS)
    pub async fn scan_existing_points(&self) -> Result<Vec<String>> {
        let mut conn = self.redis_conn.lock().await;
        let pattern = format!("{}:*", self.config.key_prefix);
        let mut cursor = 0;
        let mut all_keys = Vec::new();

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut *conn)
                .await
                .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

            all_keys.extend(keys);

            if new_cursor == 0 {
                break;
            }
            cursor = new_cursor;
        }

        Ok(all_keys)
    }

    /// Batch delete points from Redis
    pub async fn batch_delete(&self, point_ids: Vec<u32>) -> Result<()> {
        if point_ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.redis_conn.lock().await;
        let keys: Vec<String> = point_ids
            .iter()
            .map(|id| format!("{}:{id}", self.config.key_prefix))
            .collect();

        let _: () = redis::cmd("DEL")
            .arg(keys)
            .query_async(&mut *conn)
            .await
            .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

        Ok(())
    }
}

/// Lua script for atomic operations
pub const LUA_BATCH_UPDATE: &str = r#"
    local prefix = ARGV[1]
    local ttl = tonumber(ARGV[2])
    local count = 0
    
    for i = 3, #ARGV, 2 do
        local key = prefix .. ':' .. ARGV[i]
        local value = ARGV[i + 1]
        
        redis.call('SET', key, value)
        if ttl > 0 then
            redis.call('EXPIRE', key, ttl)
        end
        
        count = count + 1
    end
    
    return count
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::protocols::common::combase::optimized_point_manager::generate_test_points;

    #[tokio::test]
    async fn test_redis_batch_sync() {
        // This test requires Redis to be running
        if let Ok(client) = redis::Client::open("redis://127.0.0.1:6379") {
            if let Ok(conn) = client.get_multiplexed_async_connection().await {
                let config = RedisBatchSyncConfig {
                    batch_size: 100,
                    sync_interval: Duration::from_millis(50),
                    ..Default::default()
                };

                let sync = Arc::new(RedisBatchSync::new(conn, config));
                let manager = Arc::new(OptimizedPointManager::new("test".to_string()));

                // Load test points
                let points = generate_test_points(500);
                manager.load_points(points).await.unwrap();

                // Start sync task
                sync.clone().start_sync_task(manager.clone());

                // Wait for sync
                tokio::time::sleep(Duration::from_millis(200)).await;

                // Check stats
                let stats = sync.get_stats().await;
                assert!(stats.total_synced > 0);
                assert!(stats.batch_count > 0);
            }
        }
    }
}
