//! Unified storage module for ComSrv
//!
//! Combines Redis operations with statistics and data synchronization

use crate::core::sync::DataSync;
use crate::plugins::registry::{telemetry_type_to_redis, PluginPointUpdate};
use crate::utils::error::{ComSrvError, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};
use voltage_libs::redis::RedisClient;

/// Point update data
#[derive(Debug, Clone)]
pub struct PointUpdate {
    pub channel_id: u16,
    pub point_type: String,
    pub point_id: u32,
    pub value: f64,
}

/// Storage statistics using atomic operations for better performance
#[derive(Debug, Default)]
pub struct StorageStats {
    pub total_updates: AtomicU64,
    pub batch_updates: AtomicU64,
    pub single_updates: AtomicU64,
    pub publish_success: AtomicU64,
    pub publish_failed: AtomicU64,
    pub storage_errors: AtomicU64,
}

impl StorageStats {
    pub fn increment_total(&self, count: u64) {
        self.total_updates.fetch_add(count, Ordering::Relaxed);
    }

    pub fn increment_batch(&self) {
        self.batch_updates.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_single(&self) {
        self.single_updates.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_publish_success(&self) {
        self.publish_success.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_publish_failed(&self) {
        self.publish_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_errors(&self) {
        self.storage_errors.fetch_add(1, Ordering::Relaxed);
    }
}

/// Unified storage manager with statistics and data sync support
pub struct StorageManager {
    redis_client: Arc<Mutex<RedisClient>>,
    stats: Arc<StorageStats>,
    data_sync: Option<Arc<dyn DataSync>>,
}

impl StorageManager {
    /// Create new storage manager
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = create_redis_client(redis_url).await?;

        Ok(Self {
            redis_client: Arc::new(Mutex::new(client)),
            stats: Arc::new(StorageStats::default()),
            data_sync: None,
        })
    }

    /// Set data synchronizer
    pub fn set_data_sync(&mut self, data_sync: Arc<dyn DataSync>) {
        self.data_sync = Some(data_sync);
    }

    /// Get statistics
    pub fn get_stats(&self) -> Arc<StorageStats> {
        Arc::clone(&self.stats)
    }

    /// Batch update with statistics and sync
    pub async fn batch_update_and_publish(
        &self,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        self.stats.increment_batch();
        self.stats.increment_total(updates.len() as u64);

        // Convert to storage format
        let storage_updates: Vec<PointUpdate> = updates
            .iter()
            .map(|u| PointUpdate {
                channel_id,
                point_type: telemetry_type_to_redis(&u.telemetry_type).to_string(),
                point_id: u.point_id,
                value: u.value,
            })
            .collect();

        // Batch write to Redis
        let mut client = self.redis_client.lock().await;
        if let Err(e) = write_batch(&mut client, storage_updates).await {
            warn!("Failed to batch write to Redis: {}", e);
            self.stats.increment_errors();
            return Err(e);
        }

        // Publish updates
        for update in &updates {
            let point_type = telemetry_type_to_redis(&update.telemetry_type);
            if let Err(e) = publish_update(
                &mut client,
                channel_id,
                point_type,
                update.point_id,
                update.value,
            )
            .await
            {
                warn!("Failed to publish update: {}", e);
                self.stats.increment_publish_failed();
            } else {
                self.stats.increment_publish_success();
            }
        }

        // Trigger data sync
        if let Some(ref data_sync) = self.data_sync {
            for update in &updates {
                let point_type = telemetry_type_to_redis(&update.telemetry_type);

                if let Err(e) = data_sync
                    .sync_telemetry(channel_id, point_type, update.point_id, update.value)
                    .await
                {
                    debug!("Sync failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Single point update with statistics and sync
    pub async fn update_and_publish(
        &self,
        channel_id: u16,
        point_id: u32,
        value: f64,
        telemetry_type: &str,
    ) -> Result<()> {
        self.stats.increment_single();
        self.stats.increment_total(1);

        let mut client = self.redis_client.lock().await;

        // Write to Redis
        if let Err(e) = write_point(&mut client, channel_id, telemetry_type, point_id, value).await
        {
            warn!("Failed to write point: {}", e);
            self.stats.increment_errors();
            return Err(e);
        }

        // Publish update
        if let Err(e) =
            publish_update(&mut client, channel_id, telemetry_type, point_id, value).await
        {
            warn!("Failed to publish: {}", e);
            self.stats.increment_publish_failed();
        } else {
            self.stats.increment_publish_success();
        }

        // Trigger data sync
        if let Some(ref data_sync) = self.data_sync {
            if let Err(e) = data_sync
                .sync_telemetry(channel_id, telemetry_type, point_id, value)
                .await
            {
                debug!("Sync failed: {}", e);
            }
        }

        Ok(())
    }
}

/// Write a single point to Redis
pub async fn write_point(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    value: f64,
) -> Result<()> {
    let hash_key = format!("comsrv:{channel_id}:{point_type}");
    let field = point_id.to_string();
    let value_str = format!("{:.6}", value);

    client
        .hset(&hash_key, &field, value_str)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to write point: {e}")))?;

    Ok(())
}

/// Batch write points to Redis
pub async fn write_batch(client: &mut RedisClient, updates: Vec<PointUpdate>) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    // Group by hash key
    let mut grouped: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for update in updates {
        let hash_key = format!("comsrv:{}:{}", update.channel_id, update.point_type);
        let field = update.point_id.to_string();
        let value = format!("{:.6}", update.value);

        grouped.entry(hash_key).or_default().push((field, value));
    }

    // Batch write to each hash
    for (hash_key, fields) in grouped {
        for (field, value) in fields {
            client
                .hset(&hash_key, &field, value)
                .await
                .map_err(|e| ComSrvError::Storage(format!("Batch write failed: {e}")))?;
        }
    }

    Ok(())
}

/// Read a single point
pub async fn read_point(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
) -> Result<Option<f64>> {
    let hash_key = format!("comsrv:{channel_id}:{point_type}");
    let field = point_id.to_string();

    let value: Option<String> = client
        .hget(&hash_key, &field)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to read point: {e}")))?;

    Ok(value.and_then(|v| v.parse::<f64>().ok()))
}

/// Read multiple points
pub async fn read_points(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
    point_ids: &[u32],
) -> Result<Vec<Option<f64>>> {
    if point_ids.is_empty() {
        return Ok(vec![]);
    }

    let hash_key = format!("comsrv:{channel_id}:{point_type}");
    let fields: Vec<String> = point_ids.iter().map(|id| id.to_string()).collect();
    let field_refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();

    let values: Vec<Option<String>> = client
        .hmget(&hash_key, &field_refs)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to read points: {e}")))?;

    Ok(values
        .into_iter()
        .map(|opt| opt.and_then(|v| v.parse::<f64>().ok()))
        .collect())
}

/// Get all points for a channel
pub async fn get_channel_points(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
) -> Result<HashMap<u32, f64>> {
    let hash_key = format!("comsrv:{channel_id}:{point_type}");

    let all: HashMap<String, String> = client
        .hgetall(&hash_key)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to get all points: {e}")))?;

    let mut result = HashMap::new();
    for (field, value) in all {
        if let (Ok(point_id), Ok(val)) = (field.parse::<u32>(), value.parse::<f64>()) {
            result.insert(point_id, val);
        }
    }

    Ok(result)
}

/// Publish point update to Redis Pub/Sub
pub async fn publish_update(
    client: &mut RedisClient,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    value: f64,
) -> Result<()> {
    let topic = format!("comsrv:{}:{}", channel_id, point_type);
    let message = serde_json::json!({
        "point_id": point_id,
        "value": value,
        "timestamp": chrono::Utc::now().timestamp()
    });

    client
        .publish(&topic, &message.to_string())
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to publish: {e}")))?;

    Ok(())
}

/// Create Redis client
pub async fn create_redis_client(redis_url: &str) -> Result<RedisClient> {
    RedisClient::new(redis_url)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {e}")))
}
