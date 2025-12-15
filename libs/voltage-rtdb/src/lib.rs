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

pub mod keyspace;

pub mod routing_cache;

// Re-exports
pub use bytes::Bytes;
pub use traits::Rtdb;

// KeySpace and Routing exports
pub use keyspace::KeySpaceConfig;
pub use routing_cache::{RoutingCache, RoutingCacheStats};

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
}
