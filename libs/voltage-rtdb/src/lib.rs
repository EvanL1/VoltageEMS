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

pub mod vec_impl;

pub mod shared_impl;

pub mod error;

pub mod cleanup;

pub mod time;

pub mod write_buffer;

pub mod routing_cache;

pub mod numfmt;

// Re-exports
pub use bytes::Bytes;
pub use traits::Rtdb;

// KeySpace (canonical location: voltage_model) and Routing exports
pub use routing_cache::{C2CTarget, C2MTarget, M2CTarget, RoutingCache, RoutingCacheStats};
pub use voltage_model::KeySpaceConfig;

#[cfg(feature = "redis-backend")]
pub use redis_impl::RedisRtdb;

pub use memory_impl::{MemoryRtdb, MemoryStats};

// VecRtdb removed from public API - using SharedMemory + Redis two-tier architecture
// PointSlot and ChannelVecStore are still used internally by SharedMemory
pub use vec_impl::{instance_point_type, ChannelVecStore, PointSlot};

// Shared memory exports (123, 145)
pub use shared_impl::{
    default_shm_path, is_shm_available, try_open_reader, ChannelIndex, ChannelToSlotIndex,
    SharedConfig, SharedHeader, SharedReaderStats, SharedVecRtdbReader, SharedVecRtdbWriter,
    SharedWriterStats, SHARED_MAGIC,
};

pub use cleanup::{cleanup_invalid_keys, CleanupProvider};

pub use time::{FixedTimeProvider, SystemTimeProvider, TimeProvider};

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
    ///
    /// # Arguments
    /// * `rtdb` - RTDB trait object
    /// * `config` - KeySpace configuration
    /// * `channel_id` - Channel ID
    /// * `point_type` - Point type (Control or Adjustment)
    /// * `point_id` - Point ID
    /// * `value` - Point value
    /// * `timestamp_ms` - Timestamp in milliseconds
    ///
    /// # Returns
    /// * `Ok(())` - Success
    /// * `Err(anyhow::Error)` - Write error
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
        // Step 1: Write to separate hashes (value, ts, raw)
        // - comsrv:{channel_id}:{type}      -> values
        // - comsrv:{channel_id}:{type}:ts   -> timestamps
        // - comsrv:{channel_id}:{type}:raw  -> raw values
        let channel_key = config.channel_key(channel_id, point_type);
        write_channel_points(
            rtdb,
            &channel_key,
            vec![(point_id, value, value)], // (point_id, value, raw_value)
            timestamp_ms,
        )
        .await?;

        // Step 2: Auto-trigger TODO queue (Write-Triggers-Routing pattern)
        let todo_key = config.todo_queue_key(channel_id, point_type);

        // Compact trigger message format - direct string construction (avoids json! double serialization)
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
    ///
    /// # Arguments
    /// * `rtdb` - RTDB trait object
    /// * `channel_key` - Base channel key (e.g. "comsrv:1001:T")
    /// * `points` - Vector of (point_id, value, raw_value) tuples
    /// * `timestamp_ms` - Timestamp in milliseconds (shared by all points)
    ///
    /// # Returns
    /// * `Ok(usize)` - Number of points written
    /// * `Err(anyhow::Error)` - Write error
    ///
    /// # Optimization
    /// - Uses zero-allocation number formatting (itoa/ryu)
    /// - Uses Arc<str> for O(1) clone across 3 layers, converts to String only at final push
    pub async fn write_channel_points<R>(
        rtdb: &R,
        channel_key: &str,
        points: Vec<(u32, f64, f64)>, // (point_id, value, raw_value)
        timestamp_ms: i64,
    ) -> Result<usize>
    where
        R: Rtdb,
    {
        if points.is_empty() {
            return Ok(0);
        }

        let count = points.len();

        // Pre-convert timestamp to Bytes once using itoa (zero heap during format)
        let timestamp_bytes = i64_to_bytes(timestamp_ms);

        // Prepare data for three hashes using Arc<str> for O(1) sharing
        let mut values = Vec::with_capacity(count);
        let mut timestamps = Vec::with_capacity(count);
        let mut raw_values = Vec::with_capacity(count);

        for (point_id, value, raw_value) in points {
            // Use precomputed pool (0-255) or itoa - returns Arc<str>
            let field: Arc<str> = precomputed::get_point_id_str_or_alloc(point_id);

            // Arc::clone is O(1), convert to String only when pushing to final Vec
            // This reduces 3 String clones to 3 Arc::clone + 3 Arc->String conversions
            values.push((field.to_string(), f64_to_bytes(value)));
            timestamps.push((field.to_string(), timestamp_bytes.clone()));
            raw_values.push((field.to_string(), f64_to_bytes(raw_value)));
        }

        // Write all hashes in a single pipeline
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
    ///
    /// Synchronous version that buffers writes for later flush to Redis.
    /// Used with WriteBuffer for high-frequency updates.
    ///
    /// Uses precomputed point ID pool and itoa/ryu for zero-allocation formatting.
    ///
    /// # Arguments
    /// * `write_buffer` - WriteBuffer for aggregating writes
    /// * `channel_key` - Base channel key (e.g. "comsrv:1001:T")
    /// * `points` - Vector of (point_id, value, raw_value) tuples
    /// * `timestamp_ms` - Timestamp in milliseconds
    ///
    /// # Returns
    /// Number of points buffered
    pub fn buffer_channel_points(
        write_buffer: &WriteBuffer,
        channel_key: &str,
        points: Vec<(u32, f64, f64)>, // (point_id, value, raw_value)
        timestamp_ms: i64,
    ) -> usize {
        if points.is_empty() {
            return 0;
        }

        let count = points.len();

        // Pre-convert timestamp to Bytes once using itoa (zero heap during format)
        let timestamp_bytes = i64_to_bytes(timestamp_ms);

        // Prepare data with Arc<str> for O(1) field name sharing
        let mut values = Vec::with_capacity(count);
        let mut timestamps = Vec::with_capacity(count);
        let mut raw_values = Vec::with_capacity(count);

        for (point_id, value, raw_value) in points {
            // Use precomputed pool (0-255) or itoa for larger IDs
            // Arc<str> allows O(1) clone across 3 layers
            let field: Arc<str> = precomputed::get_point_id_str_or_alloc(point_id);

            // Arc::clone is O(1) - just atomic counter increment
            // f64_to_bytes uses ryu for fast formatting
            values.push((Arc::clone(&field), f64_to_bytes(value)));
            timestamps.push((Arc::clone(&field), timestamp_bytes.clone()));
            raw_values.push((field, f64_to_bytes(raw_value)));
        }

        // Buffer all hashes
        let ts_key = format!("{}:ts", channel_key);
        let raw_key = format!("{}:raw", channel_key);

        write_buffer.buffer_hash_mset(channel_key, values);
        write_buffer.buffer_hash_mset(&ts_key, timestamps);
        write_buffer.buffer_hash_mset(&raw_key, raw_values);

        count
    }

    /// Write a single point with automatic TODO queue trigger based on point type
    ///
    /// This function automatically determines whether to trigger the TODO queue:
    /// - **Control/Adjustment types**: Write data + trigger TODO queue
    /// - **Telemetry/Signal types**: Write data only (no TODO trigger)
    ///
    /// This implements the Write-Triggers-Routing pattern for downlink commands
    /// while avoiding unnecessary TODO triggers for uplink (read-only) data.
    ///
    /// # Arguments
    /// * `rtdb` - RTDB trait object
    /// * `config` - KeySpace configuration
    /// * `channel_id` - Channel ID
    /// * `point_type` - Point type (determines TODO trigger behavior)
    /// * `point_id` - Point ID
    /// * `value` - Point value
    ///
    /// # Returns
    /// * `Ok(i64)` - Timestamp in milliseconds
    /// * `Err(anyhow::Error)` - Write error
    pub async fn write_point_auto_trigger<R>(
        rtdb: &R,
        config: &KeySpaceConfig,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
        value: f64,
    ) -> Result<i64>
    where
        R: Rtdb,
    {
        // Get current timestamp (milliseconds since epoch)
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("Failed to get system time")?
            .as_millis() as i64;

        match point_type {
            PointType::Control | PointType::Adjustment => {
                // Write data + trigger TODO queue (Write-Triggers-Routing pattern)
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
                // Write data only (no TODO trigger for uplink data)
                let channel_key = config.channel_key(channel_id, point_type);
                write_channel_points(
                    rtdb,
                    &channel_key,
                    vec![(point_id, value, value)], // (point_id, value, raw_value)
                    timestamp_ms,
                )
                .await?;
            },
        }

        Ok(timestamp_ms)
    }

    /// Write channel point to Hash only (no TODO queue trigger)
    ///
    /// # Optimization
    /// When direct mpsc trigger succeeds, use this function to persist data
    /// to Redis Hash without triggering the TODO queue (already triggered via mpsc).
    ///
    /// This reduces Redis operations by 50%:
    /// - Before: HSET + RPUSH (TODO queue)
    /// - After: HSET only
    ///
    /// # Arguments
    /// * `rtdb` - RTDB trait object
    /// * `config` - KeySpace configuration
    /// * `channel_id` - Channel ID
    /// * `point_type` - Point type (Control or Adjustment)
    /// * `point_id` - Point ID
    /// * `value` - Point value
    /// * `timestamp_ms` - Timestamp in milliseconds
    ///
    /// # Returns
    /// * `Ok(())` - Success
    /// * `Err(anyhow::Error)` - Write error
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

        // Write to three-layer Hash (value/ts/raw) - NO TODO queue trigger
        write_channel_points(
            rtdb,
            &channel_key,
            vec![(point_id, value, value)], // (point_id, value, raw_value)
            timestamp_ms,
        )
        .await?;

        Ok(())
    }
}
