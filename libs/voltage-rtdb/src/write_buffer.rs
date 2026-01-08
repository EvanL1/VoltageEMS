//! Write Buffer for batching Redis hash operations
//!
//! This module provides a write buffer that aggregates multiple hash_set/hash_mset
//! calls in memory and periodically flushes them to Redis in batches.
//!
//! # Benefits
//! - Reduces Redis round-trips by aggregating writes
//! - Non-blocking writes for callers (fire-and-forget)
//! - Configurable flush interval and capacity limits
//!
//! # Usage
//! ```ignore
//! let config = WriteBufferConfig::default();
//! let buffer = WriteBuffer::new(config);
//!
//! // Buffer writes (returns immediately)
//! buffer.buffer_hash_set("comsrv:1001:T", "1", Bytes::from("100.5"));
//! buffer.buffer_hash_set("comsrv:1001:T", "2", Bytes::from("200.3"));
//!
//! // Start background flush task
//! let rtdb = Arc::new(redis_rtdb);
//! tokio::spawn(buffer.flush_loop(rtdb));
//! ```

use bytes::Bytes;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

use crate::Rtdb;

/// Write buffer configuration
#[derive(Clone, Debug)]
pub struct WriteBufferConfig {
    /// Flush interval in milliseconds (default: 20ms)
    pub flush_interval_ms: u64,
    /// Maximum fields per key before forcing a flush (default: 1000)
    pub max_fields_per_key: usize,
}

impl Default for WriteBufferConfig {
    fn default() -> Self {
        Self {
            flush_interval_ms: 20,
            max_fields_per_key: 1000,
        }
    }
}

impl WriteBufferConfig {
    /// Create config optimized for low latency
    pub fn low_latency() -> Self {
        Self {
            flush_interval_ms: 10,
            max_fields_per_key: 500,
        }
    }

    /// Create config optimized for high throughput
    pub fn high_throughput() -> Self {
        Self {
            flush_interval_ms: 50,
            max_fields_per_key: 2000,
        }
    }
}

/// Statistics for monitoring write buffer performance
#[derive(Debug, Default)]
pub struct WriteBufferStats {
    /// Total number of buffered writes
    pub buffered_writes: AtomicU64,
    /// Total number of flush operations
    pub flush_count: AtomicU64,
    /// Total number of fields flushed
    pub fields_flushed: AtomicU64,
    /// Number of forced flushes (due to capacity)
    pub forced_flushes: AtomicU64,
    /// Number of flush errors
    pub flush_errors: AtomicU64,
}

impl WriteBufferStats {
    /// Get a snapshot of current stats
    pub fn snapshot(&self) -> WriteBufferStatsSnapshot {
        WriteBufferStatsSnapshot {
            buffered_writes: self.buffered_writes.load(Ordering::Relaxed),
            flush_count: self.flush_count.load(Ordering::Relaxed),
            fields_flushed: self.fields_flushed.load(Ordering::Relaxed),
            forced_flushes: self.forced_flushes.load(Ordering::Relaxed),
            flush_errors: self.flush_errors.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of write buffer statistics
#[derive(Debug, Clone)]
pub struct WriteBufferStatsSnapshot {
    pub buffered_writes: u64,
    pub flush_count: u64,
    pub fields_flushed: u64,
    pub forced_flushes: u64,
    pub flush_errors: u64,
}

/// Hash write buffer for aggregating Redis hash operations
///
/// Buffers hash_set and hash_mset calls in memory, then flushes them
/// to Redis periodically using pipeline_hash_mset for efficiency.
///
/// # Performance Optimization
/// Field names use `Arc<str>` internally to avoid string clones in
/// 3-layer writes (value/timestamp/raw). `Arc::clone()` is O(1).
pub struct WriteBuffer {
    /// Pending data: key -> {field -> value}
    /// Field names use Arc<str> for O(1) cloning in multi-layer writes
    pending: DashMap<String, DashMap<Arc<str>, Bytes>>,
    /// Notification for forced flush
    flush_notify: Arc<Notify>,
    /// Configuration
    config: WriteBufferConfig,
    /// Statistics
    stats: WriteBufferStats,
}

impl WriteBuffer {
    /// Create a new write buffer with the given configuration
    pub fn new(config: WriteBufferConfig) -> Self {
        Self {
            pending: DashMap::new(),
            flush_notify: Arc::new(Notify::new()),
            config,
            stats: WriteBufferStats::default(),
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &WriteBufferConfig {
        &self.config
    }

    /// Get statistics
    pub fn stats(&self) -> &WriteBufferStats {
        &self.stats
    }

    /// Buffer a single hash field write (returns immediately)
    ///
    /// The write will be flushed to Redis on the next flush cycle.
    ///
    /// # Arguments
    /// * `key` - Redis hash key
    /// * `field` - Field name as `Arc<str>` for O(1) cloning in 3-layer writes
    /// * `value` - Field value
    pub fn buffer_hash_set(&self, key: &str, field: Arc<str>, value: Bytes) {
        // Two-phase check: get_mut first to avoid allocation on hot path
        let len = if let Some(entry) = self.pending.get_mut(key) {
            entry.insert(field, value);
            entry.len()
        } else {
            // Slow path: key doesn't exist, need to allocate
            let entry = self.pending.entry(key.to_string()).or_default();
            entry.insert(field, value);
            entry.len()
        };

        self.stats.buffered_writes.fetch_add(1, Ordering::Relaxed);

        // Check if we need to force a flush
        if len >= self.config.max_fields_per_key {
            self.stats.forced_flushes.fetch_add(1, Ordering::Relaxed);
            self.flush_notify.notify_one();
        }
    }

    /// Buffer multiple hash field writes (returns immediately)
    ///
    /// More efficient than multiple buffer_hash_set calls.
    ///
    /// # Arguments
    /// * `key` - Redis hash key
    /// * `fields` - Field-value pairs with `Arc<str>` field names for O(1) cloning
    pub fn buffer_hash_mset(&self, key: &str, fields: Vec<(Arc<str>, Bytes)>) {
        if fields.is_empty() {
            return;
        }

        let count = fields.len() as u64;

        // Two-phase check: get_mut first to avoid allocation on hot path
        let len = if let Some(entry) = self.pending.get_mut(key) {
            for (field, value) in fields {
                entry.insert(field, value);
            }
            entry.len()
        } else {
            // Slow path: key doesn't exist, need to allocate
            let entry = self.pending.entry(key.to_string()).or_default();
            for (field, value) in fields {
                entry.insert(field, value);
            }
            entry.len()
        };

        self.stats
            .buffered_writes
            .fetch_add(count, Ordering::Relaxed);

        // Check if we need to force a flush
        if len >= self.config.max_fields_per_key {
            self.stats.forced_flushes.fetch_add(1, Ordering::Relaxed);
            self.flush_notify.notify_one();
        }
    }

    /// Collect and clear all pending data
    ///
    /// Optimized to avoid double iteration and unnecessary clones.
    /// Converts `Arc<str>` field names to `String` for the Rtdb trait.
    /// This conversion happens at flush time (batched), not per-write.
    fn drain_pending(&self) -> Vec<(String, Vec<(String, Bytes)>)> {
        // Pre-allocate with estimated capacity
        let estimated_len = self.pending.len();
        let mut operations = Vec::with_capacity(estimated_len);

        // Use retain to iterate and remove in one pass
        // DashMap::retain provides mutable access and removes entries returning false
        self.pending.retain(|key, fields_map| {
            if !fields_map.is_empty() {
                // Convert Arc<str> to String at flush time
                // Drain the inner map to avoid cloning
                let fields: Vec<_> = fields_map
                    .iter()
                    .map(|entry| (entry.key().to_string(), entry.value().clone()))
                    .collect();

                // Clear the inner map instead of removing outer entry
                // This allows potential reuse of the DashMap allocation
                fields_map.clear();

                operations.push((key.clone(), fields));
            }
            false // Remove all entries after processing
        });

        operations
    }

    /// Get the number of pending keys
    pub fn pending_keys(&self) -> usize {
        self.pending.len()
    }

    /// Get the total number of pending fields across all keys
    pub fn pending_fields(&self) -> usize {
        self.pending.iter().map(|e| e.value().len()).sum()
    }

    /// Background flush loop - runs until cancelled
    ///
    /// This should be spawned as a tokio task:
    /// ```ignore
    /// let buffer = Arc::new(WriteBuffer::new(config));
    /// let rtdb = Arc::new(redis_rtdb);
    /// tokio::spawn({
    ///     let buffer = buffer.clone();
    ///     let rtdb = rtdb.clone();
    ///     async move { buffer.flush_loop(&*rtdb).await }
    /// });
    /// ```
    pub async fn flush_loop<R>(&self, rtdb: &R)
    where
        R: Rtdb,
    {
        let interval = Duration::from_millis(self.config.flush_interval_ms);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {}
                _ = self.flush_notify.notified() => {}
            }

            if let Err(e) = self.flush(rtdb).await {
                tracing::warn!(error = %e, "WriteBuffer flush failed");
                self.stats.flush_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Background flush loop with shutdown support - runs until shutdown signal
    ///
    /// Like `flush_loop`, but accepts a shutdown notify to gracefully stop.
    /// Performs a final flush before exiting to ensure no data is lost.
    ///
    /// # Arguments
    /// * `rtdb` - Redis connection
    /// * `shutdown` - Notify to signal shutdown
    pub async fn flush_loop_with_shutdown<R>(&self, rtdb: &R, shutdown: Arc<Notify>)
    where
        R: Rtdb,
    {
        let interval = Duration::from_millis(self.config.flush_interval_ms);

        loop {
            tokio::select! {
                biased;  // Check shutdown first

                _ = shutdown.notified() => {
                    tracing::debug!("WriteBuffer received shutdown signal");
                    // Final flush to ensure no data loss
                    if let Err(e) = self.flush(rtdb).await {
                        tracing::warn!(error = %e, "WriteBuffer final flush failed");
                    }
                    break;
                }
                _ = tokio::time::sleep(interval) => {}
                _ = self.flush_notify.notified() => {}
            }

            if let Err(e) = self.flush(rtdb).await {
                tracing::warn!(error = %e, "WriteBuffer flush failed");
                self.stats.flush_errors.fetch_add(1, Ordering::Relaxed);
            }
        }

        tracing::debug!("WriteBuffer flush loop stopped");
    }

    /// Flush all pending data to Redis
    ///
    /// Returns the number of fields flushed.
    pub async fn flush<R>(&self, rtdb: &R) -> anyhow::Result<usize>
    where
        R: Rtdb,
    {
        let operations = self.drain_pending();

        if operations.is_empty() {
            return Ok(0);
        }

        let field_count: usize = operations.iter().map(|(_, fields)| fields.len()).sum();

        rtdb.pipeline_hash_mset(operations).await?;

        self.stats.flush_count.fetch_add(1, Ordering::Relaxed);
        self.stats
            .fields_flushed
            .fetch_add(field_count as u64, Ordering::Relaxed);

        tracing::trace!(fields = field_count, "WriteBuffer flushed");

        Ok(field_count)
    }

    /// Force flush all pending data (for graceful shutdown)
    ///
    /// Unlike flush_loop, this is a one-shot operation.
    pub async fn flush_now<R>(&self, rtdb: &R) -> anyhow::Result<usize>
    where
        R: Rtdb,
    {
        self.flush(rtdb).await
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::MemoryRtdb;

    #[test]
    fn test_config_default() {
        let config = WriteBufferConfig::default();
        assert_eq!(config.flush_interval_ms, 20);
        assert_eq!(config.max_fields_per_key, 1000);
    }

    #[test]
    fn test_config_presets() {
        let low_latency = WriteBufferConfig::low_latency();
        assert_eq!(low_latency.flush_interval_ms, 10);

        let high_throughput = WriteBufferConfig::high_throughput();
        assert_eq!(high_throughput.flush_interval_ms, 50);
    }

    #[test]
    fn test_buffer_hash_set() {
        let buffer = WriteBuffer::new(WriteBufferConfig::default());

        buffer.buffer_hash_set("key1", Arc::from("field1"), Bytes::from("value1"));
        buffer.buffer_hash_set("key1", Arc::from("field2"), Bytes::from("value2"));
        buffer.buffer_hash_set("key2", Arc::from("field1"), Bytes::from("value3"));

        assert_eq!(buffer.pending_keys(), 2);
        assert_eq!(buffer.pending_fields(), 3);
        assert_eq!(buffer.stats.buffered_writes.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_buffer_hash_mset() {
        let buffer = WriteBuffer::new(WriteBufferConfig::default());

        buffer.buffer_hash_mset(
            "key1",
            vec![
                (Arc::from("field1"), Bytes::from("value1")),
                (Arc::from("field2"), Bytes::from("value2")),
            ],
        );

        assert_eq!(buffer.pending_keys(), 1);
        assert_eq!(buffer.pending_fields(), 2);
        assert_eq!(buffer.stats.buffered_writes.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_buffer_overwrites_same_field() {
        let buffer = WriteBuffer::new(WriteBufferConfig::default());

        buffer.buffer_hash_set("key1", Arc::from("field1"), Bytes::from("value1"));
        buffer.buffer_hash_set("key1", Arc::from("field1"), Bytes::from("value2"));

        // Should only have 1 field (overwritten)
        assert_eq!(buffer.pending_fields(), 1);
        // But stats count both writes
        assert_eq!(buffer.stats.buffered_writes.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_drain_pending() {
        let buffer = WriteBuffer::new(WriteBufferConfig::default());

        buffer.buffer_hash_set("key1", Arc::from("field1"), Bytes::from("value1"));
        buffer.buffer_hash_set("key2", Arc::from("field1"), Bytes::from("value2"));

        let operations = buffer.drain_pending();

        assert_eq!(operations.len(), 2);
        assert_eq!(buffer.pending_keys(), 0);
        assert_eq!(buffer.pending_fields(), 0);
    }

    #[tokio::test]
    async fn test_flush() {
        let buffer = WriteBuffer::new(WriteBufferConfig::default());
        let rtdb = MemoryRtdb::new();

        buffer.buffer_hash_set("test:key", Arc::from("field1"), Bytes::from("100"));
        buffer.buffer_hash_set("test:key", Arc::from("field2"), Bytes::from("200"));

        let flushed = buffer.flush(&rtdb).await.unwrap();
        assert_eq!(flushed, 2);

        // Verify data in RTDB
        let value1 = rtdb.hash_get("test:key", "field1").await.unwrap();
        assert_eq!(value1, Some(Bytes::from("100")));

        let value2 = rtdb.hash_get("test:key", "field2").await.unwrap();
        assert_eq!(value2, Some(Bytes::from("200")));

        // Stats updated
        assert_eq!(buffer.stats.flush_count.load(Ordering::Relaxed), 1);
        assert_eq!(buffer.stats.fields_flushed.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn test_flush_empty() {
        let buffer = WriteBuffer::new(WriteBufferConfig::default());
        let rtdb = MemoryRtdb::new();

        let flushed = buffer.flush(&rtdb).await.unwrap();
        assert_eq!(flushed, 0);
        assert_eq!(buffer.stats.flush_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_forced_flush_trigger() {
        let config = WriteBufferConfig {
            flush_interval_ms: 20,
            max_fields_per_key: 3, // Low threshold for testing
        };
        let buffer = WriteBuffer::new(config);

        buffer.buffer_hash_set("key1", Arc::from("field1"), Bytes::from("v1"));
        buffer.buffer_hash_set("key1", Arc::from("field2"), Bytes::from("v2"));
        assert_eq!(buffer.stats.forced_flushes.load(Ordering::Relaxed), 0);

        // Third write should trigger forced flush notification
        buffer.buffer_hash_set("key1", Arc::from("field3"), Bytes::from("v3"));
        assert_eq!(buffer.stats.forced_flushes.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_multiple_keys_flush() {
        let buffer = WriteBuffer::new(WriteBufferConfig::default());
        let rtdb = MemoryRtdb::new();

        // Buffer data for multiple keys (simulating T and S data)
        buffer.buffer_hash_set("comsrv:1001:T", Arc::from("1"), Bytes::from("100.5"));
        buffer.buffer_hash_set("comsrv:1001:T", Arc::from("2"), Bytes::from("200.3"));
        buffer.buffer_hash_set("comsrv:1001:S", Arc::from("1"), Bytes::from("1"));
        buffer.buffer_hash_set("comsrv:1001:S", Arc::from("2"), Bytes::from("0"));
        buffer.buffer_hash_set(
            "comsrv:1001:T:ts",
            Arc::from("1"),
            Bytes::from("1234567890"),
        );
        buffer.buffer_hash_set(
            "comsrv:1001:T:ts",
            Arc::from("2"),
            Bytes::from("1234567890"),
        );

        let flushed = buffer.flush(&rtdb).await.unwrap();
        assert_eq!(flushed, 6);

        // Verify all data
        let t1 = rtdb.hash_get("comsrv:1001:T", "1").await.unwrap();
        assert_eq!(t1, Some(Bytes::from("100.5")));

        let s1 = rtdb.hash_get("comsrv:1001:S", "1").await.unwrap();
        assert_eq!(s1, Some(Bytes::from("1")));

        let ts1 = rtdb.hash_get("comsrv:1001:T:ts", "1").await.unwrap();
        assert_eq!(ts1, Some(Bytes::from("1234567890")));
    }

    #[tokio::test]
    async fn test_stats_snapshot() {
        let buffer = WriteBuffer::new(WriteBufferConfig::default());
        let rtdb = MemoryRtdb::new();

        buffer.buffer_hash_set("key", Arc::from("f1"), Bytes::from("v1"));
        buffer.buffer_hash_set("key", Arc::from("f2"), Bytes::from("v2"));
        buffer.flush(&rtdb).await.unwrap();

        let snapshot = buffer.stats.snapshot();
        assert_eq!(snapshot.buffered_writes, 2);
        assert_eq!(snapshot.flush_count, 1);
        assert_eq!(snapshot.fields_flushed, 2);
        assert_eq!(snapshot.forced_flushes, 0);
        assert_eq!(snapshot.flush_errors, 0);
    }
}
