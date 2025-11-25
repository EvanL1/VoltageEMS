//! Unified storage module for ComSrv
//!
//! Combines Redis operations with statistics and data synchronization

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::utils::error::{ComSrvError, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, warn};
use voltage_config::comsrv::RedisKeys;
use voltage_config::FourRemote;
use voltage_rtdb::Rtdb;

/// Plugin point update for batch operations
///
/// Represents a single point update that will be written to Redis.
/// Used by storage manager for batch updates.
#[derive(Debug, Clone)]
pub struct PluginPointUpdate {
    /// Type of telemetry point (T/S/C/A)
    pub telemetry_type: FourRemote,
    /// Point identifier
    pub point_id: u32,
    /// Transformed value (after scale/offset/reverse)
    pub value: f64,
    /// Original raw value before transformation (optional)
    pub raw_value: Option<f64>,
}

/// Point update data
#[derive(Debug, Clone)]
pub struct PointUpdate {
    pub channel_id: u16,
    pub point_type: String,
    pub point_id: u32,
    pub value: f64,
    pub raw_value: Option<f64>,
    pub cascade_depth: u8, // Track C2C cascade depth to prevent infinite loops
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

/// Unified storage manager with statistics
pub struct StorageManager {
    rtdb: Arc<dyn Rtdb>,
    routing_cache: Arc<voltage_config::RoutingCache>,
    stats: Arc<StorageStats>,
}

impl StorageManager {
    /// Create new storage manager
    pub async fn new(
        redis_url: &str,
        routing_cache: Arc<voltage_config::RoutingCache>,
    ) -> Result<Self> {
        let rtdb = create_rtdb(redis_url).await?;
        Ok(Self::from_rtdb(rtdb, routing_cache))
    }

    /// Create storage manager from injected RTDB and routing cache
    pub fn from_rtdb(
        rtdb: Arc<dyn Rtdb>,
        routing_cache: Arc<voltage_config::RoutingCache>,
    ) -> Self {
        Self {
            rtdb,
            routing_cache,
            stats: Arc::new(StorageStats::default()),
        }
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
                point_type: u.telemetry_type.as_str().to_string(),
                point_id: u.point_id,
                value: u.value,
                raw_value: u.raw_value,
                cascade_depth: 0, // Initial depth for direct writes
            })
            .collect();

        // Batch write to Redis with routing cache
        if let Err(e) = write_batch(self.rtdb.as_ref(), &self.routing_cache, storage_updates).await
        {
            warn!("Failed to batch write to Redis: {}", e);
            self.stats.increment_errors();
            return Err(e);
        }

        // Note: Removed pub/sub publishing and individual sync calls for performance
        // Synchronization will be handled by Redis Functions if configured

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

        // Write to Redis
        if let Err(e) = write_point(&*self.rtdb, channel_id, telemetry_type, point_id, value).await
        {
            warn!("Failed to write point: {}", e);
            self.stats.increment_errors();
            return Err(e);
        }

        // Note: Removed pub/sub publishing and sync calls for performance
        // Synchronization will be handled by Redis Functions if configured

        Ok(())
    }
}

/// Write a single point to Redis (legacy version, without timestamp)
pub async fn write_point(
    rtdb: &dyn Rtdb,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    value: f64,
) -> Result<()> {
    let hash_key = RedisKeys::channel_data(channel_id, point_type);
    let field = point_id.to_string();
    let value_bytes = bytes::Bytes::from(format!("{:.6}", value));

    rtdb.hash_set(&hash_key, &field, value_bytes)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to write point: {e}")))?;

    Ok(())
}

/// Write a single point with timestamp and TODO queue trigger
///
/// This function implements:
/// 1. Atomically writes value + timestamp to Redis Hash
/// 2. For Control/Adjustment types, writes trigger signal to TODO queue
///
/// # Arguments
/// * `rtdb` - RTDB trait object
/// * `channel_id` - Channel ID
/// * `point_type` - Point type ("T"|"S"|"C"|"A")
/// * `point_id` - Point ID
/// * `value` - Point value
///
/// # Returns
/// * `Ok(i64)` - Timestamp in milliseconds
/// * `Err(ComSrvError)` - Storage error
pub async fn write_point_with_trigger(
    rtdb: &dyn Rtdb,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    value: f64,
) -> Result<i64> {
    use voltage_config::protocols::PointType;

    // Parse point type
    let point_type_enum = PointType::from_str(point_type)
        .ok_or_else(|| ComSrvError::Storage(format!("Invalid point type: {}", point_type)))?;

    let config = voltage_config::KeySpaceConfig::production();
    let channel_key = config.channel_key(channel_id, point_type_enum);

    // Get current timestamp (milliseconds since epoch)
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| ComSrvError::Storage(format!("Failed to get timestamp: {e}")))?
        .as_millis() as i64;

    // Step 1: Atomically write value + timestamp to Hash
    let ts_field = format!("{}:ts", point_id);
    let value_str = value.to_string();
    let ts_str = timestamp_ms.to_string();

    // Use hash_mset for atomic multi-field write (equivalent to single HSET with multiple fields)
    rtdb.hash_mset(
        &channel_key,
        vec![
            (point_id.to_string(), bytes::Bytes::from(value_str)),
            (ts_field, bytes::Bytes::from(ts_str)),
        ],
    )
    .await
    .map_err(|e| ComSrvError::Storage(format!("Failed to write point with timestamp: {e}")))?;

    // Also update the channel timestamp hash so read endpoints see fresh timestamps
    let channel_ts_key = format!("{}:ts", channel_key);
    rtdb.hash_set(
        &channel_ts_key,
        &point_id.to_string(),
        bytes::Bytes::from(timestamp_ms.to_string()),
    )
    .await
    .map_err(|e| ComSrvError::Storage(format!("Failed to update channel timestamp hash: {e}")))?;

    // Step 2: Conditionally write TODO queue trigger (only for Control/Adjustment types)
    if matches!(point_type_enum, PointType::Control | PointType::Adjustment) {
        let todo_key = config.todo_queue_key(channel_id, point_type_enum);

        // Compact trigger message (core fields only: point_id, value, timestamp)
        let trigger = serde_json::json!({
            "point_id": point_id,
            "value": value,
            "timestamp": timestamp_ms
        });

        rtdb.list_rpush(&todo_key, bytes::Bytes::from(trigger.to_string()))
            .await
            .map_err(|e| {
                ComSrvError::Storage(format!("Failed to write TODO queue trigger: {e}"))
            })?;
    }

    Ok(timestamp_ms)
}

/// Batch write points to Redis with routing cache support
///
/// This function implements:
/// 1. Writes engineering values, timestamps, and raw values to channel Hashes
/// 2. Looks up C2M routing in cache and writes to instance Hashes
/// 3. Looks up C2C routing in cache and forwards to target channels
///
/// # Arguments
/// * `rtdb` - RTDB trait object
/// * `routing_cache` - C2M/C2C routing cache
/// * `updates` - Vector of point updates
///
/// # Returns
/// * `Ok(())` - Success
/// * `Err(ComSrvError)` - Storage error
pub async fn write_batch(
    rtdb: &dyn Rtdb,
    routing_cache: &voltage_config::RoutingCache,
    updates: Vec<PointUpdate>,
) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    // Group updates by channel_id and point_type for efficient batch processing
    let mut grouped: HashMap<(u16, String), Vec<PointUpdate>> = HashMap::new();

    for update in updates {
        let key = (update.channel_id, update.point_type.clone());
        grouped.entry(key).or_default().push(update);
    }

    // Get current timestamp (milliseconds since epoch)
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| ComSrvError::Storage(format!("Failed to get timestamp: {e}")))?
        .as_millis() as i64;

    let config = voltage_config::KeySpaceConfig::production();

    for ((channel_id, point_type), updates) in grouped {
        // Parse point type
        use voltage_config::protocols::PointType;
        let point_type_enum = PointType::from_str(&point_type)
            .ok_or_else(|| ComSrvError::Storage(format!("Invalid point type: {}", point_type)))?;

        // Prepare data for channel Hash writes (3-layer architecture)
        let mut channel_values = Vec::new(); // Layer 1: Engineering values
        let mut channel_ts = Vec::new(); // Layer 2: Timestamps
        let mut channel_raw = Vec::new(); // Layer 3: Raw values

        // Prepare data for instance Hash writes (grouped by instance_id)
        let mut instance_writes: HashMap<u16, Vec<(String, bytes::Bytes)>> = HashMap::new();

        // Prepare data for C2C forwarding (grouped by target channel)
        let mut c2c_forwards: Vec<PointUpdate> = Vec::new();

        for update in updates {
            let point_id_str = update.point_id.to_string();

            // Layer 1: Engineering values
            channel_values.push((
                point_id_str.clone(),
                bytes::Bytes::from(update.value.to_string()),
            ));

            // Layer 2: Timestamps
            channel_ts.push((
                point_id_str.clone(),
                bytes::Bytes::from(timestamp_ms.to_string()),
            ));

            // Layer 3: Raw values
            let raw_value = update.raw_value.unwrap_or(update.value);
            channel_raw.push((
                point_id_str.clone(),
                bytes::Bytes::from(raw_value.to_string()),
            ));

            // Check for C2M routing (Channel to Model)
            let route_key = format!("{}:{}:{}", channel_id, point_type, update.point_id);
            if let Some(target) = routing_cache.lookup_c2m(&route_key) {
                // Parse target: "23:M:1" -> instance_id=23, point_id=1
                let parts: Vec<&str> = target.split(':').collect();
                if parts.len() == 3 {
                    if let (Ok(instance_id), Ok(target_point_id)) =
                        (parts[0].parse::<u16>(), parts[2].parse::<u32>())
                    {
                        instance_writes.entry(instance_id).or_default().push((
                            target_point_id.to_string(),
                            bytes::Bytes::from(update.value.to_string()),
                        ));
                    }
                }
            }

            // Check for C2C routing (Channel to Channel)
            // Only process if cascade_depth < MAX_C2C_DEPTH to prevent infinite loops
            const MAX_C2C_DEPTH: u8 = 2;
            if update.cascade_depth < MAX_C2C_DEPTH {
                if let Some(target) = routing_cache.lookup_c2c(&route_key) {
                    // Parse target: "1002:T:5" -> channel_id=1002, type=T, point_id=5
                    let parts: Vec<&str> = target.split(':').collect();
                    if parts.len() == 3 {
                        if let (Ok(target_channel_id), Ok(target_point_id)) =
                            (parts[0].parse::<u16>(), parts[2].parse::<u32>())
                        {
                            // Create a forwarded update with incremented cascade depth
                            c2c_forwards.push(PointUpdate {
                                channel_id: target_channel_id,
                                point_type: parts[1].to_string(),
                                point_id: target_point_id,
                                value: update.value,
                                raw_value: update.raw_value,
                                cascade_depth: update.cascade_depth + 1,
                            });
                        }
                    }
                }
            }
        }

        // Write channel data (3-layer architecture)
        let channel_key = config.channel_key(channel_id, point_type_enum);
        let channel_ts_key = format!("{}:ts", channel_key);
        let channel_raw_key = format!("{}:raw", channel_key);

        rtdb.hash_mset(&channel_key, channel_values)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to write channel values: {e}")))?;

        rtdb.hash_mset(&channel_ts_key, channel_ts)
            .await
            .map_err(|e| {
                ComSrvError::Storage(format!("Failed to write channel timestamps: {e}"))
            })?;

        rtdb.hash_mset(&channel_raw_key, channel_raw)
            .await
            .map_err(|e| {
                ComSrvError::Storage(format!("Failed to write channel raw values: {e}"))
            })?;

        // Write instance data (C2M routing results) - parallelized for better performance
        // Each instance write is independent, so we can run them concurrently
        if !instance_writes.is_empty() {
            let futures: Vec<_> = instance_writes
                .into_iter()
                .map(|(instance_id, values)| {
                    let instance_key = config.instance_measurement_key(instance_id.into());
                    async move {
                        rtdb.hash_mset(&instance_key, values).await.map_err(|e| {
                            ComSrvError::Storage(format!("Failed to write instance values: {e}"))
                        })
                    }
                })
                .collect();

            futures::future::try_join_all(futures).await?;
        }

        // Process C2C forwards (recursive call with incremented cascade depth)
        if !c2c_forwards.is_empty() {
            debug!(
                "Processing {} C2C forwards for channel {}",
                c2c_forwards.len(),
                channel_id
            );
            // Recursive call to write_batch for C2C forwarding
            Box::pin(write_batch(rtdb, routing_cache, c2c_forwards)).await?;
        }
    }

    Ok(())
}

/// Read a single point
pub async fn read_point(
    rtdb: &dyn Rtdb,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
) -> Result<Option<f64>> {
    let hash_key = RedisKeys::channel_data(channel_id, point_type);
    let field = point_id.to_string();

    let value_bytes = rtdb
        .hash_get(&hash_key, &field)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to read point: {e}")))?;

    Ok(value_bytes.and_then(|bytes| {
        String::from_utf8(bytes.to_vec())
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
    }))
}

/// Read multiple points
pub async fn read_points(
    rtdb: &dyn Rtdb,
    channel_id: u16,
    point_type: &str,
    point_ids: &[u32],
) -> Result<Vec<Option<f64>>> {
    if point_ids.is_empty() {
        return Ok(vec![]);
    }

    // Read points individually (StructuredStore doesn't have hmget)
    let mut results = Vec::with_capacity(point_ids.len());
    for &point_id in point_ids {
        let value = read_point(rtdb, channel_id, point_type, point_id).await?;
        results.push(value);
    }

    Ok(results)
}

/// Get all points for a channel
pub async fn get_channel_points(
    rtdb: &dyn Rtdb,
    channel_id: u16,
    point_type: &str,
) -> Result<HashMap<u32, f64>> {
    let hash_key = RedisKeys::channel_data(channel_id, point_type);

    let all = rtdb
        .hash_get_all(&hash_key)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to get all points: {e}")))?;

    let mut result = HashMap::new();
    for (field, value_bytes) in all {
        if let Ok(point_id) = field.parse::<u32>() {
            if let Ok(value_str) = String::from_utf8(value_bytes.to_vec()) {
                if let Ok(val) = value_str.parse::<f64>() {
                    result.insert(point_id, val);
                }
            }
        }
    }

    Ok(result)
}

// publish_update function removed - not used

/// Create RTDB instance
pub async fn create_rtdb(redis_url: &str) -> Result<Arc<dyn Rtdb>> {
    let rtdb = voltage_rtdb::RedisRtdb::new(redis_url)
        .await
        .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {e}")))?;

    Ok(Arc::new(rtdb))
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_storage_stats_creation() {
        let stats = StorageStats::default();
        assert_eq!(stats.total_updates.load(Ordering::Relaxed), 0);
        assert_eq!(stats.batch_updates.load(Ordering::Relaxed), 0);
        assert_eq!(stats.single_updates.load(Ordering::Relaxed), 0);
        assert_eq!(stats.publish_success.load(Ordering::Relaxed), 0);
        assert_eq!(stats.publish_failed.load(Ordering::Relaxed), 0);
        assert_eq!(stats.storage_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_storage_stats_increment_total() {
        let stats = StorageStats::default();
        stats.increment_total(5);
        stats.increment_total(3);
        assert_eq!(stats.total_updates.load(Ordering::Relaxed), 8);
    }

    #[test]
    fn test_storage_stats_increment_batch() {
        let stats = StorageStats::default();
        stats.increment_batch();
        stats.increment_batch();
        assert_eq!(stats.batch_updates.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_storage_stats_increment_single() {
        let stats = StorageStats::default();
        stats.increment_single();
        stats.increment_single();
        stats.increment_single();
        assert_eq!(stats.single_updates.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_storage_stats_increment_publish_success() {
        let stats = StorageStats::default();
        stats.increment_publish_success();
        assert_eq!(stats.publish_success.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_storage_stats_increment_publish_failed() {
        let stats = StorageStats::default();
        stats.increment_publish_failed();
        assert_eq!(stats.publish_failed.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_storage_stats_increment_errors() {
        let stats = StorageStats::default();
        stats.increment_errors();
        stats.increment_errors();
        assert_eq!(stats.storage_errors.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_point_update_structure() {
        let update = PointUpdate {
            channel_id: 1001,
            point_type: "T".to_string(),
            point_id: 1,
            value: 123.45,
            raw_value: Some(100.0),
            cascade_depth: 0,
        };

        assert_eq!(update.channel_id, 1001);
        assert_eq!(update.point_type, "T");
        assert_eq!(update.point_id, 1);
        assert_eq!(update.value, 123.45);
        assert_eq!(update.raw_value, Some(100.0));
    }

    #[test]
    fn test_storage_stats_concurrent_updates() {
        use std::thread;

        let stats = Arc::new(StorageStats::default());

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let stats_clone = Arc::clone(&stats);
                thread::spawn(move || {
                    for _ in 0..100 {
                        stats_clone.increment_total(1);
                        stats_clone.increment_batch();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // 10 threads * 100 iterations = 1000 updates
        assert_eq!(stats.total_updates.load(Ordering::Relaxed), 1000);
        assert_eq!(stats.batch_updates.load(Ordering::Relaxed), 1000);
    }

    #[test]
    fn test_point_update_clone() {
        let update1 = PointUpdate {
            channel_id: 1001,
            point_type: "T".to_string(),
            point_id: 1,
            value: 123.45,
            raw_value: Some(100.0),
            cascade_depth: 0,
        };

        let update2 = update1.clone();
        assert_eq!(update2.channel_id, update1.channel_id);
        assert_eq!(update2.point_type, update1.point_type);
        assert_eq!(update2.point_id, update1.point_id);
        assert_eq!(update2.value, update1.value);
        assert_eq!(update2.raw_value, update1.raw_value);
        assert_eq!(update2.cascade_depth, update1.cascade_depth);
    }

    #[test]
    fn test_write_batch_grouping_logic() {
        // Test grouping logic without Redis
        let updates = vec![
            PointUpdate {
                channel_id: 1001,
                point_type: "T".to_string(),
                point_id: 1,
                value: 10.0,
                raw_value: None,
                cascade_depth: 0,
            },
            PointUpdate {
                channel_id: 1001,
                point_type: "T".to_string(),
                point_id: 2,
                value: 20.0,
                raw_value: None,
                cascade_depth: 0,
            },
            PointUpdate {
                channel_id: 1002,
                point_type: "S".to_string(),
                point_id: 1,
                value: 1.0,
                raw_value: None,
                cascade_depth: 0,
            },
        ];

        // Group updates by channel_id and point_type
        let mut grouped: HashMap<(u16, String), Vec<PointUpdate>> = HashMap::new();
        for update in updates {
            let key = (update.channel_id, update.point_type.clone());
            grouped.entry(key).or_default().push(update);
        }

        // Should have 2 groups: (1001, "T") and (1002, "S")
        assert_eq!(grouped.len(), 2);
        assert!(grouped.contains_key(&(1001, "T".to_string())));
        assert!(grouped.contains_key(&(1002, "S".to_string())));

        // First group should have 2 updates
        assert_eq!(grouped.get(&(1001, "T".to_string())).unwrap().len(), 2);
        // Second group should have 1 update
        assert_eq!(grouped.get(&(1002, "S".to_string())).unwrap().len(), 1);
    }
}
