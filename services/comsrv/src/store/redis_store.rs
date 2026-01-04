//! Redis-backed data store for IGW integration
//!
//! This module provides Redis storage for protocol data, with C2M routing
//! support through voltage-routing.
//!
//! # Architecture
//!
//! The protocol layer (igw) is now separated from storage. Protocols return
//! `DataBatch` directly, and the service layer (comsrv) handles persistence:
//!
//! ```text
//! IgwChannelWrapper::poll_once()
//!         ├─ protocol.poll_once() → DataBatch
//!         └─ store.write_batch(channel_id, batch)
//!                   ↓
//!             RedisDataStore
//!                   ├─ Apply data transformations (scale/offset/reverse)
//!                   ├─ Write to Redis Hash (via WriteBuffer)
//!                   └─ C2M routing → inst:{id}:M (via RoutingCache)
//! ```

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{broadcast, Notify, RwLock};
use tracing::{debug, warn};

use igw::core::data::{DataBatch, DataPoint};
use igw::core::error::Result as IgwResult;
use igw::core::point::PointConfig;
use igw::core::traits::{DataEvent, DataEventReceiver, DataEventSender};

use voltage_model::{KeySpaceConfig, PointType};
use voltage_routing::ChannelPointUpdate;
use voltage_rtdb::{RoutingCache, Rtdb, WriteBuffer, WriteBufferConfig};

/// Redis-backed data store for VoltageEMS.
///
/// This is the bridge between IGW protocols and the VoltageEMS Redis storage.
/// Called by IgwChannelWrapper after protocol.poll_once() to persist data.
///
/// IGW handles all data transformations (scale/offset/reverse) in poll_once(),
/// so this store receives already-transformed data and writes it directly.
///
/// Point type is encoded in the internal_id using `PointType::to_internal_id()`,
/// and decoded using `PointType::from_internal_id()` when writing to Redis.
/// This avoids point_id collisions when different types share the same original ID.
///
/// It handles:
/// - Redis Hash writes via WriteBuffer (high-performance buffered writes)
/// - C2M routing to forward data to model instances
pub struct RedisDataStore<R: Rtdb> {
    /// Redis connection
    rtdb: Arc<R>,
    /// C2M/M2C routing cache
    routing_cache: Arc<RoutingCache>,
    /// Write buffer for aggregating Redis writes
    write_buffer: Arc<WriteBuffer>,
    /// Point configurations cache (channel_id -> configs)
    point_configs: DashMap<u32, Vec<PointConfig>>,
    /// Event subscribers
    subscribers: Arc<RwLock<Vec<DataEventSender>>>,
    /// KeySpace configuration
    key_config: KeySpaceConfig,
    /// Flush task handle for cleanup (uses RwLock for interior mutability)
    flush_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    /// Shutdown signal for flush task
    shutdown_notify: Arc<Notify>,
}

impl<R: Rtdb> RedisDataStore<R> {
    /// Create a new RedisDataStore.
    ///
    /// # Arguments
    ///
    /// * `rtdb` - Redis connection
    /// * `routing_cache` - C2M/M2C routing cache
    pub fn new(rtdb: Arc<R>, routing_cache: Arc<RoutingCache>) -> Self {
        Self {
            rtdb,
            routing_cache,
            write_buffer: Arc::new(WriteBuffer::new(WriteBufferConfig::default())),
            point_configs: DashMap::new(),
            subscribers: Arc::new(RwLock::new(Vec::new())),
            key_config: KeySpaceConfig::production(),
            flush_handle: RwLock::new(None),
            shutdown_notify: Arc::new(Notify::new()),
        }
    }

    /// Start the background flush task for the write buffer.
    ///
    /// The task runs until `shutdown()` is called or the store is dropped.
    /// Uses interior mutability so this can be called on Arc<RedisDataStore>.
    pub fn start_flush_task(&self) {
        let buffer = Arc::clone(&self.write_buffer);
        let rtdb = Arc::clone(&self.rtdb);
        let shutdown = Arc::clone(&self.shutdown_notify);

        let handle = tokio::spawn(async move {
            buffer.flush_loop_with_shutdown(&*rtdb, shutdown).await;
        });

        // Use blocking_write since we're not in async context here
        // and this is only called once during channel creation
        *self.flush_handle.blocking_write() = Some(handle);
    }

    /// Shutdown the flush task gracefully.
    ///
    /// Sends shutdown signal and waits for the task to complete (with timeout).
    pub async fn shutdown(&self) {
        // Signal shutdown
        self.shutdown_notify.notify_one();

        // Take the handle
        let handle = self.flush_handle.write().await.take();

        // Wait for task to finish
        if let Some(handle) = handle {
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => debug!("RedisDataStore flush task stopped gracefully"),
                Ok(Err(e)) => warn!("RedisDataStore flush task panicked: {}", e),
                Err(_) => {
                    warn!("RedisDataStore flush task did not stop in time, aborting");
                },
            }
        }
    }

    /// Convert DataBatch to ChannelPointUpdates for voltage-routing.
    ///
    /// The internal_id from IGW encodes both point type and original point_id.
    /// We decode it using `PointType::from_internal_id()` to get the correct
    /// Redis key (type) and field (original point_id).
    ///
    /// Note: IGW has already applied transformations (scale/offset/reverse) in poll_once(),
    /// so point.value is already the final transformed value.
    fn batch_to_updates(&self, channel_id: u32, batch: &DataBatch) -> Vec<ChannelPointUpdate> {
        let mut updates = Vec::with_capacity(batch.len());

        for point in batch.iter() {
            // Decode internal_id to get point_type and original point_id
            let (point_type, original_point_id) = PointType::from_internal_id(point.id);

            // IGW returns already-transformed values
            let value = point.value.as_f64().unwrap_or(0.0);

            debug!(
                "[{:?}] Point {} (internal_id={}): value={:.2}",
                point_type, original_point_id, point.id, value
            );

            updates.push(ChannelPointUpdate {
                channel_id,
                point_type,
                point_id: original_point_id, // Use original point_id for Redis field
                value,
                raw_value: None, // IGW doesn't expose pre-transform values
                cascade_depth: 0,
            });
        }

        updates
    }

    /// Notify all subscribers of a data event.
    ///
    /// Uses broadcast channel - all subscribers receive the event.
    async fn notify_subscribers(&self, event: DataEvent) {
        let subscribers = self.subscribers.read().await;
        if subscribers.is_empty() {
            return;
        }
        // Broadcast sends to all subscribers at once
        for sender in subscribers.iter() {
            let _ = sender.send(event.clone());
        }
    }
}

// Data storage methods
impl<R: Rtdb> RedisDataStore<R> {
    /// Write a batch of data points to Redis and route to model instances.
    ///
    /// Takes ownership of `batch` to avoid cloning when notifying subscribers.
    /// Note: Data is already transformed by IGW in poll_once().
    pub async fn write_batch(&self, channel_id: u32, batch: DataBatch) -> IgwResult<()> {
        if batch.is_empty() {
            return Ok(());
        }

        // Convert to ChannelPointUpdates (values already transformed by IGW)
        let updates = self.batch_to_updates(channel_id, &batch);

        // Write via voltage-routing (handles Redis + C2M routing)
        let _stats = voltage_routing::write_channel_batch_buffered(
            &self.write_buffer,
            &self.routing_cache,
            updates,
        );

        // Notify subscribers - move batch into event, no clone needed
        self.notify_subscribers(DataEvent::DataUpdate(batch)).await;

        Ok(())
    }

    /// Read a single point from Redis.
    ///
    /// Tries all point types until a value is found.
    pub async fn read_point(&self, channel_id: u32, point_id: u32) -> IgwResult<Option<DataPoint>> {
        // Try to read from each point type
        for point_type in [
            PointType::Telemetry,
            PointType::Signal,
            PointType::Control,
            PointType::Adjustment,
        ] {
            let key = self.key_config.channel_key(channel_id, point_type);

            if let Ok(Some(value_bytes)) = self.rtdb.hash_get(&key, &point_id.to_string()).await {
                let value_str = String::from_utf8_lossy(&value_bytes);
                if let Ok(value) = value_str.parse::<f64>() {
                    return Ok(Some(DataPoint::new(point_id, value)));
                }
            }
        }

        Ok(None)
    }

    /// Read all points for a channel from Redis.
    pub async fn read_all(&self, channel_id: u32) -> IgwResult<DataBatch> {
        let mut batch = DataBatch::default();

        for point_type in [
            PointType::Telemetry,
            PointType::Signal,
            PointType::Control,
            PointType::Adjustment,
        ] {
            let key = self.key_config.channel_key(channel_id, point_type);

            if let Ok(values) = self.rtdb.hash_get_all(&key).await {
                for (point_id_str, value_bytes) in values {
                    let value_str = String::from_utf8_lossy(&value_bytes);
                    if let (Ok(point_id), Ok(value)) =
                        (point_id_str.parse::<u32>(), value_str.parse::<f64>())
                    {
                        batch.add(DataPoint::new(point_id, value));
                    }
                }
            }
        }

        Ok(batch)
    }

    /// Subscribe to data events.
    ///
    /// Creates a new broadcast channel and registers the sender with the store.
    /// Returns the receiver for receiving events.
    pub fn subscribe(&self) -> DataEventReceiver {
        let (tx, rx) = broadcast::channel(1024);

        let subscribers = self.subscribers.clone();
        tokio::spawn(async move {
            let mut subs = subscribers.write().await;
            subs.push(tx);
        });

        rx
    }

    /// Get a specific point configuration.
    pub fn get_point_config(&self, channel_id: u32, point_id: u32) -> Option<PointConfig> {
        self.point_configs
            .get(&channel_id)
            .and_then(|configs| configs.value().iter().find(|c| c.id == point_id).cloned())
    }

    /// Set point configurations for a channel.
    pub fn set_point_configs(&self, channel_id: u32, configs: Vec<PointConfig>) {
        self.point_configs.insert(channel_id, configs);
    }

    /// Get all point configurations for a channel.
    pub fn get_all_point_configs(&self, channel_id: u32) -> Vec<PointConfig> {
        self.point_configs
            .get(&channel_id)
            .map(|c| c.value().clone())
            .unwrap_or_default()
    }

    /// Clear all data for a channel.
    pub async fn clear_channel(&self, channel_id: u32) -> IgwResult<()> {
        // Clear all point types for this channel
        for point_type in [
            PointType::Telemetry,
            PointType::Signal,
            PointType::Control,
            PointType::Adjustment,
        ] {
            let key = self.key_config.channel_key(channel_id, point_type);
            let _ = self.rtdb.del(&key).await;
        }

        // Clear configs
        self.point_configs.remove(&channel_id);

        Ok(())
    }
}

/// Drop implementation for defensive cleanup.
///
/// Ensures the flush task is aborted if the store is dropped without
/// explicit shutdown. This prevents orphaned tasks in edge cases.
///
/// Uses try_write() to avoid blocking, which is necessary when running
/// inside a tokio runtime (e.g., in tests).
impl<R: Rtdb> Drop for RedisDataStore<R> {
    fn drop(&mut self) {
        // Use try_write to avoid blocking in async context (e.g., tokio tests)
        // If we can't acquire the lock, the task will be cleaned up by tokio anyway
        if let Ok(mut guard) = self.flush_handle.try_write() {
            if let Some(handle) = guard.take() {
                if !handle.is_finished() {
                    warn!("RedisDataStore dropped without shutdown, aborting flush task");
                    // Signal shutdown first to allow graceful exit
                    self.shutdown_notify.notify_one();
                    handle.abort();
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use voltage_rtdb::helpers::create_test_rtdb;

    #[tokio::test]
    async fn test_redis_store_write_with_internal_id() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // Create a batch with internal_ids (simulating what igw returns)
        // Signal point_id=1 and Control point_id=1 should have different internal_ids
        let signal_internal = PointType::Signal.to_internal_id(1);
        let control_internal = PointType::Control.to_internal_id(1);

        let mut batch = DataBatch::default();
        batch.add(DataPoint::new(signal_internal, true)); // DI
        batch.add(DataPoint::new(control_internal, false)); // DO

        // Write - this should decode internal_ids correctly and write to different Redis keys
        store.write_batch(9901, batch).await.unwrap();

        // Note: In-memory test rtdb doesn't persist, so we can't verify reads
        // This test ensures the code compiles and runs without panics
    }

    #[tokio::test]
    async fn test_batch_to_updates_decodes_internal_id() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // Simulate GPIO data with Signal point_id=1 and Control point_id=1
        let signal_internal = PointType::Signal.to_internal_id(1);
        let control_internal = PointType::Control.to_internal_id(1);

        let mut batch = DataBatch::default();
        batch.add(DataPoint::new(signal_internal, 1.0)); // DI value
        batch.add(DataPoint::new(control_internal, 0.0)); // DO value

        let updates = store.batch_to_updates(5, &batch);

        assert_eq!(updates.len(), 2);

        // First update should be Signal with original point_id=1
        assert_eq!(updates[0].point_type, PointType::Signal);
        assert_eq!(updates[0].point_id, 1);
        assert_eq!(updates[0].value, 1.0);

        // Second update should be Control with original point_id=1
        assert_eq!(updates[1].point_type, PointType::Control);
        assert_eq!(updates[1].point_id, 1);
        assert_eq!(updates[1].value, 0.0);
    }

    #[tokio::test]
    async fn test_point_configs() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // Set configs using new API (with internal_id)
        let internal_id = PointType::Telemetry.to_internal_id(1);
        let configs = vec![PointConfig::new(
            internal_id,
            igw::core::point::ProtocolAddress::Generic("test".to_string()),
        )];
        store.set_point_configs(9902, configs);

        // Get config by internal_id
        let config = store.get_point_config(9902, internal_id);
        assert!(config.is_some());
        assert_eq!(config.unwrap().id, internal_id);

        // Get all configs
        let all = store.get_all_point_configs(9902);
        assert_eq!(all.len(), 1);
    }
}
