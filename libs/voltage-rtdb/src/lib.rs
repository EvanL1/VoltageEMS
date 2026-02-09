//! VoltageEMS Realtime Database Abstraction
//!
//! Provides a unified interface for realtime data storage,
//! supporting multiple backends (Redis, in-memory, etc.)
//!
//! # Key Components
//!
//! - **Rtdb trait**: Core trait for realtime database operations
//! - **KeySpaceConfig**: Redis key naming configuration
//! - **RoutingCache**: In-memory routing table cache

pub mod traits;

#[cfg(feature = "redis-backend")]
pub mod redis_impl;

pub mod memory_impl;

// SHM modules are now in voltage-rtdb-shm crate, re-exported here for backward compatibility
pub use voltage_rtdb_shm::instance_index;
pub use voltage_rtdb_shm::notification;
pub use voltage_rtdb_shm::notifier;
pub use voltage_rtdb_shm::ring_buffer;
pub use voltage_rtdb_shm::slot_bitmap;
pub use voltage_rtdb_shm::vec_impl;

pub mod shared_impl;

pub mod unified_shm;

pub mod snapshot;

pub mod error;

pub mod cleanup;

pub mod time;

pub mod write_buffer;

pub mod routing_cache;

pub mod numfmt;

pub mod channel_index;

// Re-exports
pub use bytes::Bytes;
pub use traits::Rtdb;

// KeySpace (canonical location: voltage_model) and Routing exports
pub use routing_cache::{C2CTarget, C2MTarget, M2CTarget, RoutingCache, RoutingCacheStats};
pub use voltage_model::KeySpaceConfig;

#[cfg(feature = "redis-backend")]
pub use redis_impl::RedisRtdb;

pub use memory_impl::{MemoryRtdb, MemoryStats};

// Shared memory public API (only types needed by external crates)
pub use shared_impl::{default_shm_path, is_shm_available, ChannelToSlotIndex, SharedConfig};

// Unified shared memory (simplified architecture: Header + PointSlots only)
pub use unified_shm::{
    allocate_layouts, calculate_file_size, ChannelLayout, UnifiedHeader, UnifiedReader,
    UnifiedWriter, UNIFIED_MAGIC, UNIFIED_VERSION,
};

// UDS event notification (M2C command notification via Unix Domain Socket)
pub use voltage_rtdb_shm::notification::ShmNotification;
pub use voltage_rtdb_shm::notifier::{NotifyResult, ShmNotifier, UdsHealth, DEFAULT_UDS_PATH};

// Snapshot management (periodic saves, graceful shutdown)
pub use snapshot::{snapshot_exists, snapshot_info, SnapshotConfig, SnapshotInfo, SnapshotManager};

pub use cleanup::{cleanup_invalid_keys, CleanupProvider};

// Ring buffer (high-frequency data recording)
pub use voltage_rtdb_shm::ring_buffer::{
    current_timestamp_us, DataPoint, HighFreqRingBuffer, RingBufferConfig, SharedRingBuffer,
};

pub use time::{FixedTimeProvider, SystemTimeProvider, TimeProvider};

// Slot bitmap for dynamic allocation (unified pool architecture)
pub use voltage_rtdb_shm::slot_bitmap::{
    BitmapStats, SlotAllocation, SlotBitmap, SlotBitmapHeader,
};

// Channel index for dynamic channel management
pub use channel_index::{ChannelIndex, DynamicChannelLayout};

// Instance index for dynamic instance management with shared slots
pub use voltage_rtdb_shm::instance_index::{DynamicInstanceLayout, InstanceIndex, SharedSlotRef};

pub use write_buffer::{
    WriteBuffer, WriteBufferConfig, WriteBufferStats, WriteBufferStatsSnapshot,
};

/// Helper functions for common operations
pub mod helpers {
    use super::numfmt::{f64_to_bytes, i64_to_bytes, precomputed};
    use super::{KeySpaceConfig, MemoryRtdb, Rtdb, WriteBuffer};
    use anyhow::{Context, Result};
    use bytes::Bytes;
    use std::sync::Arc;
    use voltage_model::PointType;

    // ==================== Test Support ====================

    /// Create an in-memory RTDB for unit testing
    ///
    /// This creates a MemoryRtdb that doesn't require any external services.
    /// Suitable for unit tests that should not depend on Redis.
    ///
    /// # Example
    /// ```
    /// use voltage_rtdb::helpers::create_test_rtdb;
    ///
    /// let rtdb = create_test_rtdb();
    /// // Use rtdb in tests...
    /// ```
    pub fn create_test_rtdb() -> Arc<MemoryRtdb> {
        Arc::new(MemoryRtdb::new())
    }

    // ==================== Production Helpers ====================

    /// Set channel point with automatic TODO queue trigger
    ///
    /// This function implements the Write-Triggers-Routing pattern:
    /// 1. Writes to comsrv:{channel_id}:{A|C} Hash (value/ts/raw)
    /// 2. Automatically triggers comsrv:{channel_id}:{A|C}:TODO queue
    ///
    /// **Design principle**: Hash writes and TODO triggers are always synchronized.
    /// No matter how the Hash is modified (routing/API/tools), TODO is automatically triggered.
    pub async fn set_channel_point_with_trigger<R>(
        rtdb: &R,
        config: &KeySpaceConfig,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
        value: f64,
        timestamp_ms: i64,
    ) -> Result<()>
    where
        R: Rtdb,
    {
        let channel_key = config.channel_key(channel_id, point_type);
        write_channel_points(
            rtdb,
            &channel_key,
            vec![(point_id, value, value)],
            timestamp_ms,
        )
        .await?;

        let todo_key = config.todo_queue_key(channel_id, point_type);

        let trigger = format!(
            r#"{{"point_id":{},"value":{},"timestamp":{}}}"#,
            point_id, value, timestamp_ms
        );

        rtdb.list_rpush(&todo_key, Bytes::from(trigger))
            .await
            .context("Failed to trigger TODO queue")?;

        Ok(())
    }

    // ==================== Batch Helpers ====================

    /// Batch write channel points to Redis
    ///
    /// Writes multiple points to three separate hashes:
    /// - `{channel_key}`     → engineering values
    /// - `{channel_key}:ts`  → timestamps
    /// - `{channel_key}:raw` → raw values
    pub async fn write_channel_points<R>(
        rtdb: &R,
        channel_key: &str,
        points: Vec<(u32, f64, f64)>,
        timestamp_ms: i64,
    ) -> Result<usize>
    where
        R: Rtdb,
    {
        if points.is_empty() {
            return Ok(0);
        }

        let count = points.len();
        let timestamp_bytes = i64_to_bytes(timestamp_ms);

        let mut values = Vec::with_capacity(count);
        let mut timestamps = Vec::with_capacity(count);
        let mut raw_values = Vec::with_capacity(count);

        for (point_id, value, raw_value) in points {
            let field: Arc<str> = precomputed::get_point_id_str_or_alloc(point_id);
            values.push((Arc::clone(&field), f64_to_bytes(value)));
            timestamps.push((Arc::clone(&field), timestamp_bytes.clone()));
            raw_values.push((field, f64_to_bytes(raw_value)));
        }

        let ts_key = format!("{}:ts", channel_key);
        let raw_key = format!("{}:raw", channel_key);

        rtdb.pipeline_hash_mset(vec![
            (channel_key.to_string(), values),
            (ts_key, timestamps),
            (raw_key, raw_values),
        ])
        .await
        .context("Failed to write channel points")?;

        Ok(count)
    }

    /// Buffer channel points for deferred write (via WriteBuffer)
    pub fn buffer_channel_points(
        write_buffer: &WriteBuffer,
        channel_key: &str,
        points: Vec<(u32, f64, f64)>,
        timestamp_ms: i64,
    ) -> usize {
        if points.is_empty() {
            return 0;
        }

        let count = points.len();
        let timestamp_bytes = i64_to_bytes(timestamp_ms);

        let mut values = Vec::with_capacity(count);
        let mut timestamps = Vec::with_capacity(count);
        let mut raw_values = Vec::with_capacity(count);

        for (point_id, value, raw_value) in points {
            let field: Arc<str> = precomputed::get_point_id_str_or_alloc(point_id);
            values.push((Arc::clone(&field), f64_to_bytes(value)));
            timestamps.push((Arc::clone(&field), timestamp_bytes.clone()));
            raw_values.push((field, f64_to_bytes(raw_value)));
        }

        let ts_key = format!("{}:ts", channel_key);
        let raw_key = format!("{}:raw", channel_key);

        write_buffer.buffer_hash_mset(channel_key, values);
        write_buffer.buffer_hash_mset(&ts_key, timestamps);
        write_buffer.buffer_hash_mset(&raw_key, raw_values);

        count
    }

    /// Write a single point with automatic TODO queue trigger based on point type
    pub async fn write_point_auto_trigger<R>(
        rtdb: &R,
        config: &KeySpaceConfig,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
        value: f64,
        timestamp_ms: Option<i64>,
    ) -> Result<i64>
    where
        R: Rtdb,
    {
        let timestamp_ms = timestamp_ms.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time before UNIX epoch")
                .as_millis() as i64
        });

        match point_type {
            PointType::Control | PointType::Adjustment => {
                set_channel_point_with_trigger(
                    rtdb,
                    config,
                    channel_id,
                    point_type,
                    point_id,
                    value,
                    timestamp_ms,
                )
                .await?;
            },
            PointType::Telemetry | PointType::Signal => {
                let channel_key = config.channel_key(channel_id, point_type);
                write_channel_points(
                    rtdb,
                    &channel_key,
                    vec![(point_id, value, value)],
                    timestamp_ms,
                )
                .await?;
            },
        }

        Ok(timestamp_ms)
    }

    /// Write channel point to Hash only (no TODO queue trigger)
    pub async fn write_channel_hash_only<R>(
        rtdb: &R,
        config: &KeySpaceConfig,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
        value: f64,
        timestamp_ms: i64,
    ) -> Result<()>
    where
        R: Rtdb,
    {
        let channel_key = config.channel_key(channel_id, point_type);

        write_channel_points(
            rtdb,
            &channel_key,
            vec![(point_id, value, value)],
            timestamp_ms,
        )
        .await?;

        Ok(())
    }
}
