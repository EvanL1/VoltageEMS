//! Redis-backed data store for protocol integration
//!
//! This module provides Redis storage for protocol data, with C2M routing
//! support through voltage-routing.
//!
//! # Architecture
//!
//! The protocol layer is separated from storage. Protocols return
//! `DataBatch` directly, and the service layer (comsrv) handles persistence:
//!
//! ```text
//! run_polling_task()
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
use tokio::sync::{Notify, RwLock};
use tracing::{debug, trace, warn};

use crate::protocols::core::data::DataBatch;
use crate::protocols::core::error::Result as ProtocolResult;
use crate::protocols::core::point::PointConfig;
use crate::protocols::core::traits::{DataEvent, DataEventReceiver, DataEventSender};

use voltage_model::KeySpaceConfig;
use voltage_routing::{ChannelPointUpdate, RoutingCache};
use voltage_rtdb::{Bytes, Rtdb, WriteBuffer, WriteBufferConfig};
use voltage_rtdb_shm::ShmHandle;

/// Redis-backed data store for VoltageEMS.
///
/// This is the bridge between protocol adapters and the VoltageEMS Redis storage.
/// Called by run_polling_task after protocol.poll_once() to persist data.
///
/// Protocol adapters handle all data transformations (scale/offset/reverse) in poll_once(),
/// so this store receives already-transformed data and writes it directly.
///
/// Each DataPoint carries an explicit `point_type` field to identify its SCADA category
/// (Telemetry/Signal/Control/Adjustment), used for routing to the correct Redis key.
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
    /// Runtime-swappable shared memory handle (writer + index, rebuilt on routing reload)
    shm_handle: Option<Arc<ShmHandle>>,
    /// Point configurations cache (channel_id -> `Arc<configs>` for O(1) clone)
    point_configs: DashMap<u32, Arc<Vec<PointConfig>>>,
    /// Single broadcast sender for all subscribers (avoids clone * N)
    event_sender: DataEventSender,
    /// KeySpace configuration (cached static ref to avoid repeated allocation)
    key_config: &'static KeySpaceConfig,
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
        // Create single broadcast channel - all subscribers share this sender
        let (event_sender, _) = tokio::sync::broadcast::channel(1024);
        // 24h TTL on written keys: prevents stale data after service crash/restart.
        // TTL is refreshed every 5 minutes by WriteBuffer's throttled expire logic.
        let wb_config = WriteBufferConfig {
            key_ttl_seconds: Some(86400),
            ..WriteBufferConfig::default()
        };
        Self {
            rtdb,
            routing_cache,
            write_buffer: Arc::new(WriteBuffer::new(wb_config)),
            shm_handle: None,
            point_configs: DashMap::new(),
            event_sender,
            key_config: KeySpaceConfig::production_cached(),
            flush_handle: RwLock::new(None),
            shutdown_notify: Arc::new(Notify::new()),
        }
    }

    /// Set shared memory handle for high-performance writes.
    ///
    /// When enabled, writes go directly to shared memory using pre-computed
    /// channel → slot mappings, bypassing the C2M routing lookup at runtime.
    /// This provides ~89% latency reduction for batch writes.
    ///
    /// The ShmHandle is runtime-swappable via ArcSwap, so routing reloads
    /// automatically rebuild the writer and index without restarting channels.
    pub fn with_shm_handle(mut self, handle: Arc<ShmHandle>) -> Self {
        self.shm_handle = Some(handle);
        self
    }

    /// Start the background flush task for the write buffer.
    ///
    /// The task runs until `shutdown()` is called or the store is dropped.
    /// Uses interior mutability so this can be called on `Arc<RedisDataStore>`.
    /// Idempotent: if a task is already running, this is a no-op.
    pub async fn start_flush_task(&self) {
        // Check if task already running (prevent concurrent start race)
        {
            let guard = self.flush_handle.read().await;
            if let Some(ref handle) = *guard {
                if !handle.is_finished() {
                    debug!("Flush task already running, skipping start");
                    return;
                }
            }
        }

        let buffer = Arc::clone(&self.write_buffer);
        let rtdb = Arc::clone(&self.rtdb);
        let shutdown = Arc::clone(&self.shutdown_notify);

        let handle = tokio::spawn(async move {
            buffer.flush_loop_with_shutdown(&*rtdb, shutdown).await;
        });

        *self.flush_handle.write().await = Some(handle);
    }

    /// Shutdown the flush task gracefully.
    ///
    /// Sends shutdown signal and waits for the task to complete (with timeout).
    /// If timeout occurs, performs a final flush before aborting to prevent data loss.
    pub async fn shutdown(&self) {
        // Signal shutdown
        self.shutdown_notify.notify_one();

        // Take the handle
        let handle = self.flush_handle.write().await.take();

        // Wait for task to finish
        if let Some(handle) = handle {
            // Get abort handle before timeout consumes the JoinHandle
            let abort_handle = handle.abort_handle();
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => debug!("RedisDataStore flush task stopped gracefully"),
                Ok(Err(e)) => warn!("RedisDataStore flush task panicked: {}", e),
                Err(_) => {
                    warn!("RedisDataStore flush task did not stop in time, performing final flush before abort");
                    // Flush any remaining buffered data before aborting to prevent data loss
                    match self.write_buffer.flush_now(&*self.rtdb).await {
                        Ok(count) => {
                            if count > 0 {
                                debug!("Final flush saved {} keys before abort", count);
                            }
                        },
                        Err(e) => warn!("Final flush failed: {}", e),
                    }
                    abort_handle.abort();
                },
            }
        }
    }

    /// Convert DataBatch to ChannelPointUpdates for voltage-routing.
    ///
    /// Each DataPoint carries explicit `id` and `point_type` fields.
    ///
    /// Note: Protocol adapters have already applied transformations (scale/offset/reverse),
    /// so point.value is already the final transformed value.
    fn batch_to_updates(&self, channel_id: u32, batch: &DataBatch) -> Vec<ChannelPointUpdate> {
        let mut updates = Vec::with_capacity(batch.len());

        for point in batch.iter() {
            // Use explicit point_type and id (no decoding needed)
            let value = match point.value.as_f64() {
                Some(v) => v,
                None => {
                    trace!(
                        "Ch{} [{:?}] Point {}: non-numeric value {:?}, defaulting to 0.0",
                        channel_id,
                        point.point_type,
                        point.id,
                        point.value
                    );
                    0.0
                },
            };

            // Use trace! to avoid flooding logs - only visible with RUST_LOG=trace
            trace!(
                "[{:?}] Point {}: value={:.2}",
                point.point_type,
                point.id,
                value
            );

            updates.push(ChannelPointUpdate {
                channel_id,
                point_type: point.point_type,
                point_id: point.id,
                value,
                raw_value: None, // Protocol adapters don't expose pre-transform values
                cascade_depth: 0,
            });
        }

        updates
    }

    /// Notify all subscribers of a data event.
    ///
    /// Uses single broadcast channel - send once, all receivers get the event.
    /// No clone needed - broadcast internally uses Arc for zero-copy sharing.
    fn notify_subscribers(&self, event: DataEvent) {
        // Single send - broadcast channel handles distribution to all receivers
        // Ignore SendError (no active receivers) - this is expected when no subscribers
        let _ = self.event_sender.send(event);
    }
}

// Data storage methods
impl<R: Rtdb> RedisDataStore<R> {
    /// Write a batch of data points to Redis and route to model instances.
    ///
    /// Takes ownership of `batch` to avoid cloning when notifying subscribers.
    /// Note: Data is already transformed by the protocol layer in poll_once().
    ///
    /// # Write Path Selection
    ///
    /// 1. **Shared Memory Path** (fastest): If `shared_writer` and `channel_index` are configured,
    ///    uses `write_channel_batch_direct()` for O(1) direct slot writes (~10ns per point).
    /// 2. **Buffered Path** (fallback): Uses `write_channel_batch_buffered()` for Redis writes.
    pub async fn write_batch(&self, channel_id: u32, batch: DataBatch) -> ProtocolResult<()> {
        if batch.is_empty() {
            return Ok(());
        }

        // Convert to ChannelPointUpdates (values already transformed by protocol layer)
        let updates = self.batch_to_updates(channel_id, &batch);

        // Select write path: prefer shared memory direct write for best performance
        let _stats = if let Some(handle) = &self.shm_handle {
            if let (Some(writer_guard), Some(index_guard)) = (handle.writer(), handle.index()) {
                // Fast path: direct shared memory write with pre-computed channel → slot mapping
                let writer = writer_guard.as_ref().expect("writer guard non-empty");
                let index = index_guard.as_ref().expect("index guard non-empty");
                voltage_rtdb_shm::write_channel_batch_direct(
                    writer,
                    index,
                    &self.routing_cache,
                    updates,
                )
            } else {
                // SHM handle exists but writer/index not yet initialized
                voltage_routing::write_channel_batch_buffered(
                    &self.write_buffer,
                    &self.routing_cache,
                    updates,
                )
            }
        } else {
            // Fallback path: buffered write to Redis only
            voltage_routing::write_channel_batch_buffered(
                &self.write_buffer,
                &self.routing_cache,
                updates,
            )
        };

        // Notify subscribers - wrap batch in Arc for 0.2.18 API
        self.notify_subscribers(DataEvent::DataUpdate(Arc::new(batch)));

        Ok(())
    }

    /// Subscribe to data events.
    ///
    /// Returns a new receiver from the shared broadcast channel.
    /// Multiple subscribers share the same sender - no clone * N overhead.
    /// Receivers are automatically cleaned up when dropped.
    pub fn subscribe(&self) -> DataEventReceiver {
        self.event_sender.subscribe()
    }

    /// Set point configurations for a channel.
    pub fn set_point_configs(&self, channel_id: u32, configs: Vec<PointConfig>) {
        self.point_configs.insert(channel_id, Arc::new(configs));
    }

    /// Publish channel online status to Redis hash.
    /// Only call when status actually changes to minimize Redis writes.
    pub async fn publish_channel_online(&self, channel_id: u32, online: bool) {
        let key = self.key_config.channel_online_key();
        let value = if online { "1" } else { "0" };
        if let Err(e) = self
            .rtdb
            .hash_set(&key, &channel_id.to_string(), Bytes::from(value))
            .await
        {
            warn!("Ch{} failed to publish online status: {}", channel_id, e);
        }
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
    use crate::protocols::core::data::DataPoint;
    use voltage_model::PointType;
    use voltage_rtdb::helpers::create_test_rtdb;

    #[tokio::test]
    async fn test_redis_store_write_with_point_type() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // Create a batch with explicit point types
        // Signal point_id=1 and Control point_id=1 are different points
        let mut batch = DataBatch::default();
        batch.add(DataPoint::signal(1, true)); // DI
        batch.add(DataPoint::control(1, false)); // DO

        // Write - uses point.point_type to route to different Redis keys
        store.write_batch(9901, batch).await.unwrap();

        // Note: In-memory test rtdb doesn't persist, so we can't verify reads
        // This test ensures the code compiles and runs without panics
    }

    #[tokio::test]
    async fn test_batch_to_updates_uses_point_type() {
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(RoutingCache::new());

        let store = RedisDataStore::new(rtdb, routing_cache);

        // GPIO data with explicit point types - same point_id, different types
        let mut batch = DataBatch::default();
        batch.add(DataPoint::signal(1, 1.0)); // DI value
        batch.add(DataPoint::control(1, 0.0)); // DO value

        let updates = store.batch_to_updates(5, &batch);

        assert_eq!(updates.len(), 2);

        // First update should be Signal with point_id=1
        assert_eq!(updates[0].point_type, PointType::Signal);
        assert_eq!(updates[0].point_id, 1);
        assert_eq!(updates[0].value, 1.0);

        // Second update should be Control with point_id=1
        assert_eq!(updates[1].point_type, PointType::Control);
        assert_eq!(updates[1].point_id, 1);
        assert_eq!(updates[1].value, 0.0);
    }
}
