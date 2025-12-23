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
use tokio::sync::{mpsc, RwLock};
use tracing::debug;

use igw::core::data::{DataBatch, DataPoint, DataType, Value};
use igw::core::error::Result as IgwResult;
use igw::core::point::PointConfig;
use igw::core::traits::{DataEvent, DataEventReceiver, DataEventSender};

use common::FourRemote;
use voltage_model::KeySpaceConfig;
use voltage_model::PointType;
use voltage_routing::ChannelPointUpdate;
use voltage_rtdb::{RoutingCache, Rtdb, WriteBuffer, WriteBufferConfig};

use crate::core::channels::point_config::RuntimeConfigProvider;
use crate::core::channels::sync::TransformDirection;

/// Redis-backed data store for VoltageEMS.
///
/// This is the bridge between IGW protocols and the VoltageEMS Redis storage.
/// Called by IgwChannelWrapper after protocol.poll_once() to persist data.
///
/// It handles:
/// - Data transformation (scale/offset/reverse) via RuntimeConfigProvider
/// - Redis Hash writes via WriteBuffer (high-performance buffered writes)
/// - C2M routing to forward data to model instances
pub struct RedisDataStore {
    /// Redis connection
    rtdb: Arc<dyn Rtdb>,
    /// C2M/M2C routing cache
    routing_cache: Arc<RoutingCache>,
    /// Write buffer for aggregating Redis writes
    write_buffer: Arc<WriteBuffer>,
    /// Point configuration provider for transformations
    config_provider: Arc<RuntimeConfigProvider>,
    /// Point configurations cache (channel_id -> configs)
    point_configs: DashMap<u32, Vec<PointConfig>>,
    /// Event subscribers
    subscribers: Arc<RwLock<Vec<DataEventSender>>>,
    /// KeySpace configuration
    key_config: KeySpaceConfig,
}

impl RedisDataStore {
    /// Create a new RedisDataStore.
    ///
    /// # Arguments
    ///
    /// * `rtdb` - Redis connection
    /// * `routing_cache` - C2M/M2C routing cache
    /// * `config_provider` - Point configuration provider for transformations
    pub fn new(
        rtdb: Arc<dyn Rtdb>,
        routing_cache: Arc<RoutingCache>,
        config_provider: Arc<RuntimeConfigProvider>,
    ) -> Self {
        Self {
            rtdb,
            routing_cache,
            write_buffer: Arc::new(WriteBuffer::new(WriteBufferConfig::default())),
            config_provider,
            point_configs: DashMap::new(),
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

    /// Convert igw DataType to voltage PointType.
    fn to_point_type(data_type: DataType) -> PointType {
        match data_type {
            DataType::Telemetry => PointType::Telemetry,
            DataType::Signal => PointType::Signal,
            DataType::Control => PointType::Control,
            DataType::Adjustment => PointType::Adjustment,
        }
    }

    /// Convert igw DataType to FourRemote for transformation lookup.
    fn to_four_remote(data_type: DataType) -> FourRemote {
        match data_type {
            DataType::Telemetry => FourRemote::Telemetry,
            DataType::Signal => FourRemote::Signal,
            DataType::Control => FourRemote::Control,
            DataType::Adjustment => FourRemote::Adjustment,
        }
    }

    /// Convert igw Value to f64 for Redis storage.
    fn value_to_f64(value: &Value) -> f64 {
        value.as_f64().unwrap_or(0.0)
    }

    /// Transform a DataPoint using configured transformations.
    fn transform_point(&self, channel_id: u32, point: &DataPoint) -> (f64, f64) {
        let raw_value = Self::value_to_f64(&point.value);
        let four_remote = Self::to_four_remote(point.data_type);

        let transformer = self
            .config_provider
            .get_transformer(channel_id, &four_remote, point.id);

        let processed_value = transformer.transform(raw_value, TransformDirection::DeviceToSystem);

        debug!(
            "[{}] Point {}: raw={:.2} → value={:.2}",
            point.data_type.as_str(),
            point.id,
            raw_value,
            processed_value
        );

        (processed_value, raw_value)
    }

    /// Convert DataBatch to ChannelPointUpdates for voltage-routing.
    fn batch_to_updates(&self, channel_id: u32, batch: &DataBatch) -> Vec<ChannelPointUpdate> {
        let mut updates = Vec::with_capacity(batch.len());

        for point in batch.iter() {
            let (value, raw_value) = self.transform_point(channel_id, point);

            updates.push(ChannelPointUpdate {
                channel_id,
                point_type: Self::to_point_type(point.data_type),
                point_id: point.id,
                value,
                raw_value: Some(raw_value),
                cascade_depth: 0,
            });
        }

        updates
    }

    /// Notify all subscribers of a data event.
    async fn notify_subscribers(&self, event: DataEvent) {
        let subscribers = self.subscribers.read().await;
        for sender in subscribers.iter() {
            let _ = sender.try_send(event.clone());
        }
    }
}

// Data storage methods (no longer implementing igw::DataStore trait)
impl RedisDataStore {
    /// Write a batch of data points to Redis with transformations and routing.
    pub async fn write_batch(&self, channel_id: u32, batch: &DataBatch) -> IgwResult<()> {
        if batch.is_empty() {
            return Ok(());
        }

        // Convert to ChannelPointUpdates with transformations
        let updates = self.batch_to_updates(channel_id, batch);

        // Write via voltage-routing (handles Redis + C2M routing)
        let _stats = voltage_routing::write_channel_batch_buffered(
            &self.write_buffer,
            &self.routing_cache,
            updates,
        );

        // Notify subscribers
        self.notify_subscribers(DataEvent::DataUpdate(batch.clone()))
            .await;

        Ok(())
    }

    /// Read a single point from Redis.
    pub async fn read_point(&self, channel_id: u32, point_id: u32) -> IgwResult<Option<DataPoint>> {
        // Try to read from each point type
        for data_type in [
            DataType::Telemetry,
            DataType::Signal,
            DataType::Control,
            DataType::Adjustment,
        ] {
            let point_type = Self::to_point_type(data_type);
            let key = self.key_config.channel_key(channel_id, point_type);

            if let Ok(Some(value_bytes)) = self.rtdb.hash_get(&key, &point_id.to_string()).await {
                let value_str = String::from_utf8_lossy(&value_bytes);
                if let Ok(value) = value_str.parse::<f64>() {
                    return Ok(Some(DataPoint::new(point_id, data_type, value)));
                }
            }
        }

        Ok(None)
    }

    /// Read all points for a channel from Redis.
    pub async fn read_all(&self, channel_id: u32) -> IgwResult<DataBatch> {
        let mut batch = DataBatch::default();

        for data_type in [
            DataType::Telemetry,
            DataType::Signal,
            DataType::Control,
            DataType::Adjustment,
        ] {
            let point_type = Self::to_point_type(data_type);
            let key = self.key_config.channel_key(channel_id, point_type);

            if let Ok(values) = self.rtdb.hash_get_all(&key).await {
                for (point_id_str, value_bytes) in values {
                    let value_str = String::from_utf8_lossy(&value_bytes);
                    if let (Ok(point_id), Ok(value)) =
                        (point_id_str.parse::<u32>(), value_str.parse::<f64>())
                    {
                        batch.add(DataPoint::new(point_id, data_type, value));
                    }
                }
            }
        }

        Ok(batch)
    }

    /// Subscribe to data events.
    pub fn subscribe(&self) -> DataEventReceiver {
        let (tx, rx) = mpsc::channel(1024);

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
        let config_provider = Arc::new(RuntimeConfigProvider::new());

        let store = RedisDataStore::new(rtdb, routing_cache, config_provider);

        // Create a batch
        let mut batch = DataBatch::default();
        batch.add(DataPoint::telemetry(1, 25.5));
        batch.add(DataPoint::signal(2, true));

        // Write
        store.write_batch(9901, &batch).await.unwrap();

        // Note: In-memory test rtdb doesn't persist, so we can't verify reads
        // This test just ensures the code compiles and runs without panics
    }

    #[tokio::test]
    async fn test_point_configs() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());
        let config_provider = Arc::new(RuntimeConfigProvider::new());

        let store = RedisDataStore::new(rtdb, routing_cache, config_provider);

        // Set configs
        let configs = vec![PointConfig::new(
            1,
            igw::core::data::DataType::Telemetry,
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
}
