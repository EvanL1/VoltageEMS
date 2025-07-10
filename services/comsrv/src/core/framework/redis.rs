//! Redis Integration Module
//!
//! Efficient batch operations for syncing point data to Redis.
//! Consolidated from redis_batch_sync.rs

use redis::aio::MultiplexedConnection;
use redis::Pipeline;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, Duration};
use tracing::{error, info};

use super::manager::OptimizedPointManager;
use super::types::PointData;
use crate::utils::Result;

/// Configuration for Redis batch sync
#[derive(Debug, Clone)]
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
    /// Channel ID for key generation
    pub channel_id: Option<u16>,
}

impl Default for RedisBatchSyncConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            sync_interval: Duration::from_millis(100),
            key_prefix: "comsrv:points".to_string(),
            point_ttl: None,
            use_pipeline: true,
            channel_id: None,
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
            .field("config", &self.config)
            .field("update_buffer", &"<buffer>")
            .field("stats", &self.stats)
            .finish()
    }
}

#[derive(Debug, Default, Clone)]
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
    async fn sync_batch(&self, point_manager: &OptimizedPointManager) -> Result<()> {
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
        // 按channel_id分组，以便使用Hash结构
        let mut channel_groups: HashMap<u16, HashMap<String, String>> = HashMap::new();

        for (_id, data) in points.iter() {
            if let (Some(channel_id), Some(telemetry_type)) = (
                data.channel_id.or(self.config.channel_id),
                &data.telemetry_type,
            ) {
                let field = format!("{}:{}", telemetry_type, data.id);
                let value = json!({
                    "id": data.id,
                    "name": data.name,
                    "value": data.value,
                    "unit": data.unit,
                    "timestamp": data.timestamp.to_rfc3339(),
                    "description": data.description,
                    "telemetry_type": telemetry_type,
                    "channel_id": channel_id,
                });

                channel_groups
                    .entry(channel_id)
                    .or_insert_with(HashMap::new)
                    .insert(field, value.to_string());
            }
        }

        // 使用Pipeline批量更新每个通道的Hash
        let mut pipe = Pipeline::new();
        let mut total_count = 0;

        for (channel_id, fields) in channel_groups {
            let hash_key = format!("comsrv:realtime:channel:{}", channel_id);

            // 使用HMSET批量设置字段
            for (field, value) in fields {
                pipe.hset(&hash_key, &field, value);
                total_count += 1;
            }

            // 设置Hash的过期时间
            if let Some(ttl) = self.config.point_ttl {
                pipe.expire(&hash_key, ttl.as_secs() as i64);
            }

            // 当累积了足够的命令时执行
            if total_count >= self.config.batch_size {
                pipe.query_async(conn)
                    .await
                    .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

                self.stats.write().await.total_synced += total_count as u64;

                pipe = Pipeline::new();
                total_count = 0;
            }
        }

        // 执行剩余的命令
        if total_count > 0 {
            pipe.query_async(conn)
                .await
                .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

            self.stats.write().await.total_synced += total_count as u64;
        }

        info!("Synced {} points to Redis using pipeline", points.len());

        Ok(())
    }

    /// Sync points individually using Hash (fallback method)
    async fn sync_individually(
        &self,
        conn: &mut MultiplexedConnection,
        points: HashMap<u32, PointData>,
    ) -> Result<()> {
        let mut count = 0;

        for (_id, data) in points {
            if let (Some(channel_id), Some(telemetry_type)) = (
                data.channel_id.or(self.config.channel_id),
                &data.telemetry_type,
            ) {
                let hash_key = format!("comsrv:realtime:channel:{}", channel_id);
                let field = format!("{}:{}", telemetry_type, data.id);

                let value = json!({
                    "id": data.id,
                    "name": data.name,
                    "value": data.value,
                    "unit": data.unit,
                    "timestamp": data.timestamp.to_rfc3339(),
                    "description": data.description,
                    "telemetry_type": telemetry_type,
                    "channel_id": channel_id,
                });

                // 使用HSET设置单个字段
                let _: () = redis::cmd("HSET")
                    .arg(&hash_key)
                    .arg(&field)
                    .arg(value.to_string())
                    .query_async(conn)
                    .await
                    .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

                // 刷新Hash的过期时间
                if let Some(ttl) = self.config.point_ttl {
                    let _: () = redis::cmd("EXPIRE")
                        .arg(&hash_key)
                        .arg(ttl.as_secs())
                        .query_async(conn)
                        .await
                        .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;
                }

                count += 1;
            }
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
                .push(config.address.to_string());
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

    /// Update a single point value in Redis using Hash structure
    pub async fn update_value(&self, point_data: PointData) -> Result<()> {
        info!("RedisBatchSync::update_value called - Using Hash structure for point {} with channel_id={:?}, telemetry_type={:?}", 
            point_data.id, point_data.channel_id.or(self.config.channel_id), point_data.telemetry_type);

        if let (Some(channel_id), Some(telemetry_type)) = (
            point_data.channel_id.or(self.config.channel_id),
            &point_data.telemetry_type,
        ) {
            let mut conn = self.redis_conn.lock().await;
            let hash_key = format!("comsrv:realtime:channel:{}", channel_id);
            let field = format!("{}:{}", telemetry_type, point_data.id);

            info!(
                "RedisBatchSync::update_value - Writing to Hash: key={}, field={}, value={}",
                hash_key, field, point_data.value
            );

            let value = json!({
                "id": point_data.id,
                "name": point_data.name,
                "value": point_data.value,
                "unit": point_data.unit,
                "timestamp": point_data.timestamp.to_rfc3339(),
                "description": point_data.description,
                "telemetry_type": telemetry_type,
                "channel_id": channel_id,
            });

            // Use HSET to update single field in Hash
            let _: () = redis::cmd("HSET")
                .arg(&hash_key)
                .arg(&field)
                .arg(value.to_string())
                .query_async(&mut *conn)
                .await
                .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;

            // Refresh Hash expiration if configured
            if let Some(ttl) = self.config.point_ttl {
                let _: () = redis::cmd("EXPIRE")
                    .arg(&hash_key)
                    .arg(ttl.as_secs())
                    .query_async(&mut *conn)
                    .await
                    .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Batch update multiple point values in Redis using Hash structure
    pub async fn batch_update_values(&self, points: Vec<PointData>) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        info!("RedisBatchSync::batch_update_values called - Batch updating {} points using Hash structure", points.len());

        // Group points by channel_id for Hash storage
        let mut channel_groups: HashMap<u16, HashMap<String, String>> = HashMap::new();

        for point_data in points {
            if let (Some(channel_id), Some(telemetry_type)) = (
                point_data.channel_id.or(self.config.channel_id),
                &point_data.telemetry_type,
            ) {
                let field = format!("{}:{}", telemetry_type, point_data.id);
                let value = json!({
                    "id": point_data.id,
                    "name": point_data.name,
                    "value": point_data.value,
                    "unit": point_data.unit,
                    "timestamp": point_data.timestamp.to_rfc3339(),
                    "description": point_data.description,
                    "telemetry_type": telemetry_type,
                    "channel_id": channel_id,
                });

                channel_groups
                    .entry(channel_id)
                    .or_insert_with(HashMap::new)
                    .insert(field, value.to_string());
            }
        }

        let mut conn = self.redis_conn.lock().await;

        if self.config.use_pipeline {
            // Use pipeline for better performance
            let mut pipe = Pipeline::new();

            for (channel_id, fields) in channel_groups {
                let hash_key = format!("comsrv:realtime:channel:{}", channel_id);

                // Use HMSET to set multiple fields at once
                for (field, value) in fields {
                    pipe.hset(&hash_key, &field, value);
                }

                // Set Hash expiration if configured
                if let Some(ttl) = self.config.point_ttl {
                    pipe.expire(&hash_key, ttl.as_secs() as i64);
                }
            }

            pipe.query_async(&mut *conn)
                .await
                .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;
        } else {
            // Update individually using HSET
            for (channel_id, fields) in channel_groups {
                let hash_key = format!("comsrv:realtime:channel:{}", channel_id);

                for (field, value) in fields {
                    let _: () = redis::cmd("HSET")
                        .arg(&hash_key)
                        .arg(&field)
                        .arg(value)
                        .query_async(&mut *conn)
                        .await
                        .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;
                }

                if let Some(ttl) = self.config.point_ttl {
                    let _: () = redis::cmd("EXPIRE")
                        .arg(&hash_key)
                        .arg(ttl.as_secs())
                        .query_async(&mut *conn)
                        .await
                        .map_err(|e| crate::utils::ComSrvError::RedisError(e.to_string()))?;
                }
            }
        }

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
    use crate::core::framework::manager::generate_test_points;
    use crate::core::framework::TelemetryType;
    use chrono::Utc;

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

                // Generate test data updates for the points
                let updates: Vec<(u32, PointData)> = (0..100)
                    .map(|i| {
                        let point_id = 1000 + i as u32;
                        let data = PointData {
                            id: point_id.to_string(),
                            name: format!("Point_{:04}", point_id),
                            value: (i as f64 * 1.5).to_string(),
                            timestamp: Utc::now(),
                            unit: "V".to_string(),
                            description: format!("Test point {}", i),
                            telemetry_type: Some(TelemetryType::Telemetry),
                            channel_id: None,
                        };
                        (point_id, data)
                    })
                    .collect();

                // Update point values in the manager
                manager.batch_update_point_data(updates).await.unwrap();

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
