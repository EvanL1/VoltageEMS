//! VoltageEMS Realtime Database Abstraction
//!
//! Provides a unified interface for realtime data storage,
//! supporting multiple backends (Redis, in-memory, etc.)
//!
//! # Key Components
//!
//! - **Rtdb trait**: Core trait for realtime database operations
//! - **KeySpaceConfig**: Redis key naming configuration
//! - **WriteBuffer**: Deferred batch write buffer

pub mod traits;

#[cfg(feature = "redis-backend")]
pub mod redis_impl;

pub mod memory_impl;

pub mod error;

pub mod cleanup;

pub mod time;

pub mod write_buffer;

pub mod numfmt;

// Re-exports
pub use bytes::Bytes;
pub use traits::Rtdb;

// KeySpace (canonical location: voltage_model)
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
    use super::numfmt::{f64_to_bytes, i64_to_bytes, precomputed};
    use super::{KeySpaceConfig, MemoryRtdb, Rtdb, WriteBuffer};
    use anyhow::{Context, Result};
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

    /// Write a single point to channel Hash (all point types unified)
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

        write_channel_hash_only(
            rtdb,
            config,
            channel_id,
            point_type,
            point_id,
            value,
            timestamp_ms,
        )
        .await?;

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
