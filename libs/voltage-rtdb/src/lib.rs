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

pub mod error;

pub mod cleanup;

pub mod time;

pub mod write_buffer;

pub mod routing_cache;

// Re-exports
pub use bytes::Bytes;
pub use traits::Rtdb;

// KeySpace (canonical location: voltage_model) and Routing exports
pub use routing_cache::{C2CTarget, C2MTarget, M2CTarget, RoutingCache, RoutingCacheStats};
pub use voltage_model::KeySpaceConfig;

#[cfg(feature = "redis-backend")]
pub use redis_impl::RedisRtdb;

pub use memory_impl::{MemoryRtdb, MemoryStats};

pub use cleanup::{cleanup_invalid_keys, CleanupProvider};

pub use time::{FixedTimeProvider, SystemTimeProvider, TimeProvider};

pub use write_buffer::{
    WriteBuffer, WriteBufferConfig, WriteBufferStats, WriteBufferStatsSnapshot,
};

/// Helper functions for common operations
pub mod helpers {
    use super::{KeySpaceConfig, MemoryRtdb, Rtdb};
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
    pub fn create_test_rtdb() -> Arc<dyn Rtdb> {
        Arc::new(MemoryRtdb::new())
    }

    /// Create a concrete MemoryRtdb for unit testing
    ///
    /// Use this when you need direct access to MemoryRtdb methods
    /// (e.g., for inspecting internal state in tests).
    pub fn create_test_memory_rtdb() -> Arc<MemoryRtdb> {
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
        R: Rtdb + ?Sized,
    {
        // Step 1: Write to comsrv:{channel_id}:{A|C} Hash (three fields: value/ts/raw)
        let channel_key = config.channel_key(channel_id, point_type);

        // Field 1: {point_id} = value
        rtdb.hash_set(
            &channel_key,
            &point_id.to_string(),
            Bytes::from(value.to_string()),
        )
        .await
        .context("Failed to write channel point value")?;

        // Field 2: {point_id}:ts = timestamp
        rtdb.hash_set(
            &channel_key,
            &format!("{}:ts", point_id),
            Bytes::from(timestamp_ms.to_string()),
        )
        .await
        .context("Failed to write channel point timestamp")?;

        // Field 3: {point_id}:raw = value (same as value for now)
        rtdb.hash_set(
            &channel_key,
            &format!("{}:raw", point_id),
            Bytes::from(value.to_string()),
        )
        .await
        .context("Failed to write channel point raw value")?;

        // Step 2: Auto-trigger TODO queue (Write-Triggers-Routing pattern)
        let todo_key = config.todo_queue_key(channel_id, point_type);

        // Compact trigger message format (point_id, value, timestamp)
        #[allow(clippy::disallowed_methods)]
        let trigger = serde_json::json!({
            "point_id": point_id,
            "value": value,
            "timestamp": timestamp_ms
        });

        rtdb.list_rpush(&todo_key, Bytes::from(trigger.to_string()))
            .await
            .context("Failed to trigger TODO queue")?;

        Ok(())
    }

    // ==================== Batch Helpers ====================

    /// Batch write channel points with 3-layer architecture (value + timestamp + raw)
    ///
    /// This function efficiently writes multiple points to a channel hash using
    /// the 3-layer data architecture:
    /// - Layer 1: Engineering values (comsrv:{channel_id}:{type})
    /// - Layer 2: Timestamps (comsrv:{channel_id}:{type}:ts)
    /// - Layer 3: Raw values (comsrv:{channel_id}:{type}:raw)
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
    pub async fn set_channel_points_3layer<R>(
        rtdb: &R,
        channel_key: &str,
        points: Vec<(u32, f64, f64)>, // (point_id, value, raw_value)
        timestamp_ms: i64,
    ) -> Result<usize>
    where
        R: Rtdb + ?Sized,
    {
        if points.is_empty() {
            return Ok(0);
        }

        let count = points.len();

        // Pre-convert timestamp to Bytes once (clone is O(1) for Bytes)
        let timestamp_bytes = Bytes::from(timestamp_ms.to_string());

        // Prepare 3-layer data
        let mut values = Vec::with_capacity(count);
        let mut timestamps = Vec::with_capacity(count);
        let mut raw_values = Vec::with_capacity(count);

        for (point_id, value, raw_value) in points {
            let point_id_str = point_id.to_string();

            // Layer 1: Engineering values
            values.push((point_id_str.clone(), Bytes::from(value.to_string())));

            // Layer 2: Timestamps
            timestamps.push((point_id_str.clone(), timestamp_bytes.clone()));

            // Layer 3: Raw values
            raw_values.push((point_id_str, Bytes::from(raw_value.to_string())));
        }

        // Write all 3 layers in a single pipeline
        let ts_key = format!("{}:ts", channel_key);
        let raw_key = format!("{}:raw", channel_key);

        rtdb.pipeline_hash_mset(vec![
            (channel_key.to_string(), values),
            (ts_key, timestamps),
            (raw_key, raw_values),
        ])
        .await
        .context("Failed to write channel 3-layer data")?;

        Ok(count)
    }

    /// Buffer channel points with 3-layer architecture (for WriteBuffer)
    ///
    /// This is a synchronous version that buffers writes instead of sending them
    /// directly to Redis. Used with WriteBuffer for high-frequency updates.
    ///
    /// # Arguments
    /// * `write_buffer` - WriteBuffer for aggregating writes
    /// * `channel_key` - Base channel key (e.g. "comsrv:1001:T")
    /// * `points` - Vector of (point_id, value, raw_value) tuples
    /// * `timestamp_ms` - Timestamp in milliseconds
    ///
    /// # Returns
    /// Number of points buffered
    pub fn buffer_channel_points_3layer(
        write_buffer: &super::WriteBuffer,
        channel_key: &str,
        points: Vec<(u32, f64, f64)>, // (point_id, value, raw_value)
        timestamp_ms: i64,
    ) -> usize {
        if points.is_empty() {
            return 0;
        }

        let count = points.len();

        // Pre-convert timestamp to Bytes once
        let timestamp_bytes = Bytes::from(timestamp_ms.to_string());

        // Prepare 3-layer data
        let mut values = Vec::with_capacity(count);
        let mut timestamps = Vec::with_capacity(count);
        let mut raw_values = Vec::with_capacity(count);

        for (point_id, value, raw_value) in points {
            let point_id_str = point_id.to_string();

            values.push((point_id_str.clone(), Bytes::from(value.to_string())));
            timestamps.push((point_id_str.clone(), timestamp_bytes.clone()));
            raw_values.push((point_id_str, Bytes::from(raw_value.to_string())));
        }

        // Buffer all 3 layers
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
    /// - **Control/Adjustment types**: Write 3-layer data + trigger TODO queue
    /// - **Telemetry/Signal types**: Write 3-layer data only (no TODO trigger)
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
        R: Rtdb + ?Sized,
    {
        // Get current timestamp (milliseconds since epoch)
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("Failed to get system time")?
            .as_millis() as i64;

        match point_type {
            PointType::Control | PointType::Adjustment => {
                // Write 3-layer data + trigger TODO queue (Write-Triggers-Routing pattern)
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
                // Write 3-layer data only (no TODO trigger for uplink data)
                let channel_key = config.channel_key(channel_id, point_type);
                set_channel_points_3layer(
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
}
