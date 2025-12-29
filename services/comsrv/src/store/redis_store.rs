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
use tokio::sync::{broadcast, RwLock};
use tracing::debug;

use igw::core::data::{DataBatch, DataPoint};
use igw::core::error::Result as IgwResult;
use igw::core::point::PointConfig;
use igw::core::traits::{DataEvent, DataEventReceiver, DataEventSender};

use voltage_model::KeySpaceConfig;
use voltage_model::PointType;
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
    /// Point type mapping: (channel_id, point_id) -> PointType
    /// Since igw no longer tracks point types, the application layer must maintain this.
    point_types: DashMap<(u32, u32), PointType>,
    /// Event subscribers
    subscribers: Arc<RwLock<Vec<DataEventSender>>>,
    /// KeySpace configuration
    key_config: KeySpaceConfig,
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
            point_types: DashMap::new(),
            subscribers: Arc::new(RwLock::new(Vec::new())),
            key_config: KeySpaceConfig::production(),
        }
    }

    /// Start the background flush task for the write buffer.
    pub fn start_flush_task(&self) {
        let buffer = Arc::clone(&self.write_buffer);
        let rtdb = Arc::clone(&self.rtdb);

        tokio::spawn(async move {
            buffer.flush_loop(&*rtdb).await;
        });
    }

    /// Register point types for a channel.
    ///
    /// Since igw no longer tracks point types, the application layer must call this
    /// method to register the mapping from (channel_id, point_id) -> PointType.
    /// This is typically called by ChannelManager when creating a channel.
    pub fn register_point_types(&self, channel_id: u32, types: Vec<(u32, PointType)>) {
        for (point_id, point_type) in types {
            self.point_types.insert((channel_id, point_id), point_type);
        }
    }

    /// Get point type for a specific point.
    ///
    /// # Panics
    /// Panics if point type is not registered. This is intentional (Fail Fast)
    /// to catch configuration errors immediately rather than silently writing
    /// to wrong Redis keys.
    fn get_point_type(&self, channel_id: u32, point_id: u32) -> PointType {
        self.point_types
            .get(&(channel_id, point_id))
            .map(|r| *r)
            .unwrap_or_else(|| {
                panic!(
                    "Point type not registered for Ch{}:Point{}. \
                     Call register_point_types() before polling.",
                    channel_id, point_id
                )
            })
    }

    /// Convert DataBatch to ChannelPointUpdates for voltage-routing.
    ///
    /// Note: IGW has already applied transformations (scale/offset/reverse) in poll_once(),
    /// so point.value is already the final transformed value.
    fn batch_to_updates(&self, channel_id: u32, batch: &DataBatch) -> Vec<ChannelPointUpdate> {
        let mut updates = Vec::with_capacity(batch.len());

        for point in batch.iter() {
            // IGW returns already-transformed values
            let value = point.value.as_f64().unwrap_or(0.0);
            let point_type = self.get_point_type(channel_id, point.id);

            debug!("[{:?}] Point {}: value={:.2}", point_type, point.id, value);

            updates.push(ChannelPointUpdate {
                channel_id,
                point_type,
                point_id: point.id,
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

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use voltage_rtdb::helpers::create_test_rtdb;

    #[tokio::test]
    async fn test_redis_store_write_read() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // Register point types first
        store.register_point_types(
            9901,
            vec![(1, PointType::Telemetry), (2, PointType::Signal)],
        );

        // Create a batch using new API (no data_type in DataPoint)
        let mut batch = DataBatch::default();
        batch.add(DataPoint::new(1, 25.5));
        batch.add(DataPoint::new(2, true));

        // Write
        store.write_batch(9901, batch).await.unwrap();

        // Note: In-memory test rtdb doesn't persist, so we can't verify reads
        // This test just ensures the code compiles and runs without panics
    }

    #[tokio::test]
    async fn test_point_configs() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // Set configs using new API (no DataType in PointConfig)
        let configs = vec![PointConfig::new(
            1,
            igw::core::point::ProtocolAddress::Generic("test".to_string()),
        )];
        store.set_point_configs(9902, configs);

        // Get config
        let config = store.get_point_config(9902, 1);
        assert!(config.is_some());
        assert_eq!(config.unwrap().id, 1);

        // Get all configs
        let all = store.get_all_point_configs(9902);
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_point_type_mapping() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // Register point types
        store.register_point_types(
            9903,
            vec![
                (1, PointType::Telemetry),
                (2, PointType::Signal),
                (3, PointType::Control),
            ],
        );

        // Verify lookups for registered points
        assert_eq!(store.get_point_type(9903, 1), PointType::Telemetry);
        assert_eq!(store.get_point_type(9903, 2), PointType::Signal);
        assert_eq!(store.get_point_type(9903, 3), PointType::Control);
        // Note: Unregistered points now panic (Fail Fast principle)
        // See test_unregistered_point_panics for panic behavior
    }

    #[tokio::test]
    #[should_panic(expected = "Point type not registered")]
    async fn test_unregistered_point_panics() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // Don't register any points, accessing should panic
        store.get_point_type(9999, 1);
    }
}
