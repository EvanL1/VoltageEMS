//! Trait definitions for RTDB abstraction

use anyhow::Result;
use bytes::Bytes;
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;

/// Unified RTDB Storage Trait
///
/// Provides complete storage interface for VoltageEMS, combining:
/// - Basic key-value operations
/// - Structured data (Hash, List, Set)
/// - Convenience operations (point initialization, command queuing)
///
/// Implementations:
/// - `RedisRtdb`: Production Redis backend
/// - `MemoryRtdb`: In-memory backend for testing
///
/// Note: All async methods return `impl Future + Send` to ensure compatibility
/// with `tokio::spawn` and other multi-threaded contexts.
pub trait Rtdb: Send + Sync + 'static {
    // ========== Introspection ==========

    /// Allow downcasting to concrete types
    ///
    /// This enables runtime type checking and conversion to specific implementations
    /// like RedisRtdb or MemoryRtdb when needed.
    fn as_any(&self) -> &dyn Any;

    // ========== Basic Key-Value Operations ==========

    /// Get value by key
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Bytes>>> + Send;

    /// Set value for key
    fn set(&self, key: &str, value: Bytes) -> impl Future<Output = Result<()>> + Send;

    /// Delete key
    fn del(&self, key: &str) -> impl Future<Output = Result<bool>> + Send;

    /// Check if key exists
    fn exists(&self, key: &str) -> impl Future<Output = Result<bool>> + Send;

    /// Increment key by float value (Redis INCRBYFLOAT)
    ///
    /// Returns the new value after incrementing.
    ///
    /// # Behavior
    ///
    /// - If the key does not exist, it is initialized to 0.0 before incrementing.
    /// - If the current value cannot be parsed as f64, it is treated as 0.0.
    ///
    /// # Implementation Notes
    ///
    /// - **RedisRtdb**: Delegates to Redis INCRBYFLOAT, which returns an error if the value is not a valid float.
    /// - **MemoryRtdb**: Silently defaults to 0.0 on parse failure (logs at trace level).
    ///
    /// For test consistency, ensure stored values are always valid numeric strings.
    fn incrbyfloat(&self, key: &str, increment: f64) -> impl Future<Output = Result<f64>> + Send;

    // ========== Hash Operations ==========

    /// Set hash field
    fn hash_set(
        &self,
        key: &str,
        field: &str,
        value: Bytes,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Get hash field
    fn hash_get(
        &self,
        key: &str,
        field: &str,
    ) -> impl Future<Output = Result<Option<Bytes>>> + Send;

    /// Get multiple hash fields (Redis HMGET)
    ///
    /// Returns a vector of values corresponding to the requested fields.
    /// Non-existent fields are returned as None.
    fn hash_mget(
        &self,
        key: &str,
        fields: &[&str],
    ) -> impl Future<Output = Result<Vec<Option<Bytes>>>> + Send;

    /// Set multiple hash fields
    fn hash_mset(
        &self,
        key: &str,
        fields: Vec<(String, Bytes)>,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Get all hash fields
    fn hash_get_all(
        &self,
        key: &str,
    ) -> impl Future<Output = Result<HashMap<String, Bytes>>> + Send;

    /// Delete hash field
    fn hash_del(&self, key: &str, field: &str) -> impl Future<Output = Result<bool>> + Send;

    /// Delete multiple hash fields at once (Redis HDEL with multiple fields)
    ///
    /// This is more efficient than multiple individual hash_del calls as it uses
    /// a single Redis command to delete all specified fields.
    ///
    /// Returns the number of fields that were removed.
    fn hash_del_many(
        &self,
        key: &str,
        fields: &[String],
    ) -> impl Future<Output = Result<usize>> + Send;

    /// Delete multiple hash fields using string slices (convenience wrapper)
    ///
    /// This is a convenience method that avoids the need to convert `&[&str]` to `Vec<String>`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// rtdb.hash_del_many_str("my_hash", &["field1", "field2", "field3"]).await?;
    /// ```
    fn hash_del_many_str(
        &self,
        key: &str,
        fields: &[&str],
    ) -> impl Future<Output = Result<usize>> + Send {
        let key = key.to_string();
        let fields: Vec<String> = fields.iter().copied().map(String::from).collect();
        async move { self.hash_del_many(&key, &fields).await }
    }

    /// Increment hash field by value (Redis HINCRBY)
    ///
    /// Returns the new value after incrementing.
    ///
    /// # Behavior
    ///
    /// - If the hash or field does not exist, it is initialized to 0 before incrementing.
    /// - If the current value cannot be parsed as i64, it is treated as 0.
    ///
    /// # Implementation Notes
    ///
    /// - **RedisRtdb**: Delegates to Redis HINCRBY, which returns an error if the value is not a valid integer.
    /// - **MemoryRtdb**: Silently defaults to 0 on parse failure (logs at trace level).
    ///
    /// For test consistency, ensure stored values are always valid numeric strings.
    fn hincrby(
        &self,
        key: &str,
        field: &str,
        increment: i64,
    ) -> impl Future<Output = Result<i64>> + Send;

    // ========== List Operations ==========

    /// Push value to left of list
    fn list_lpush(&self, key: &str, value: Bytes) -> impl Future<Output = Result<()>> + Send;

    /// Push value to right of list
    fn list_rpush(&self, key: &str, value: Bytes) -> impl Future<Output = Result<()>> + Send;

    /// Pop value from left of list
    fn list_lpop(&self, key: &str) -> impl Future<Output = Result<Option<Bytes>>> + Send;

    /// Pop value from right of list (Redis RPOP)
    ///
    /// Returns the popped value if the list is not empty, None otherwise.
    fn list_rpop(&self, key: &str) -> impl Future<Output = Result<Option<Bytes>>> + Send;

    /// Block and pop value from multiple lists (Redis BLPOP)
    ///
    /// Blocks until a value is available in one of the specified lists,
    /// or until the timeout expires.
    ///
    /// # Arguments
    /// * `keys` - List of keys to wait on
    /// * `timeout_seconds` - Timeout in seconds (0 = block indefinitely)
    ///
    /// # Returns
    /// * `Some((key, value))` - The key that had data and the popped value
    /// * `None` - Timeout expired without data
    fn list_blpop(
        &self,
        keys: &[&str],
        timeout_seconds: u64,
    ) -> impl Future<Output = Result<Option<(String, Bytes)>>> + Send;

    /// Get list range
    fn list_range(
        &self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> impl Future<Output = Result<Vec<Bytes>>> + Send;

    /// Trim list to range
    fn list_trim(
        &self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> impl Future<Output = Result<()>> + Send;

    // ========== Set Operations ==========

    /// Add member to set (Redis SADD)
    ///
    /// Returns true if the member was added, false if it already existed.
    fn sadd(&self, key: &str, member: &str) -> impl Future<Output = Result<bool>> + Send;

    /// Remove member from set (Redis SREM)
    ///
    /// Returns true if the member was removed, false if it didn't exist.
    fn srem(&self, key: &str, member: &str) -> impl Future<Output = Result<bool>> + Send;

    /// Get all members of a set (Redis SMEMBERS)
    ///
    /// Returns a vector of all members in the set.
    fn smembers(&self, key: &str) -> impl Future<Output = Result<Vec<String>>> + Send;

    // ========== Key Scanning Operations ==========

    /// Scan keys matching a pattern (Redis SCAN with MATCH)
    ///
    /// Returns a list of keys matching the glob pattern.
    /// In test implementations (MemoryRtdb), this searches in-memory keys.
    fn scan_match(&self, pattern: &str) -> impl Future<Output = Result<Vec<String>>> + Send;

    // ========== Time Operations ==========

    /// Get current Redis server time in milliseconds (Redis TIME)
    ///
    /// Returns Unix timestamp in milliseconds.
    /// In test implementations (MemoryRtdb), this returns system time.
    ///
    /// # Deprecation
    /// This method mixes time acquisition with storage operations.
    /// Use `voltage_rtdb::TimeProvider` trait instead for better separation of concerns.
    #[deprecated(
        since = "0.2.0",
        note = "Use voltage_rtdb::TimeProvider trait instead for better separation of concerns"
    )]
    fn time_millis(&self) -> impl Future<Output = Result<i64>> + Send;

    // ========== Pipeline Operations ==========

    /// Execute multiple HMSET operations in a single pipeline (pure Redis, no Lua)
    ///
    /// This batches multiple hash write operations into a single network round-trip,
    /// significantly reducing latency for bulk writes.
    ///
    /// # Arguments
    /// * `operations` - Vector of (key, fields) tuples, where fields is Vec<(field, value)>
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err` if any operation fails
    fn pipeline_hash_mset(
        &self,
        operations: Vec<(String, Vec<(String, Bytes)>)>,
    ) -> impl Future<Output = Result<()>> + Send;

    // ========== Convenience Operations (with default implementations) ==========

    /// Write point data in initialization mode (no routing trigger)
    ///
    /// This method writes point value with timestamp=0, WITHOUT triggering routing.
    /// Use during system initialization and configuration loading.
    ///
    /// # Operations
    /// 1. HSET {key}:{point_id} → {value}
    /// 2. HSET {key}:ts:{point_id} → 0
    ///
    /// # Arguments
    /// * `key` - Full point key (e.g., "inst:1:A" or "comsrv:1001:T")
    /// * `point_id` - Point identifier
    /// * `value` - Point value
    ///
    /// # Examples
    /// ```ignore
    /// // Initialize instance action point
    /// rtdb.write_point_init("inst:1:A", 10, 100.0).await?;
    ///
    /// // Initialize channel telemetry point
    /// rtdb.write_point_init("comsrv:1001:T", 5, 230.5).await?;
    /// ```
    fn write_point_init(
        &self,
        key: &str,
        point_id: u32,
        value: f64,
    ) -> impl Future<Output = Result<()>> + Send {
        // Capture parameters for the async block
        let key = key.to_string();
        async move {
            // Default implementation: write value + ts=0, no routing
            let field = point_id.to_string();
            let ts_field = format!("ts:{}", point_id);
            let value_bytes = Bytes::from(value.to_string());
            let ts_bytes = Bytes::from("0");

            self.hash_set(&key, &field, value_bytes).await?;
            self.hash_set(&key, &ts_field, ts_bytes).await?;
            Ok(())
        }
    }

    /// Enqueue control command to per-channel TODO queue: comsrv:{channel}:C:TODO
    fn enqueue_control(
        &self,
        channel_id: u32,
        payload_json: &str,
    ) -> impl Future<Output = Result<()>> + Send {
        let key = format!("comsrv:{}:C:TODO", channel_id);
        let payload = payload_json.to_string();
        async move { self.list_rpush(&key, Bytes::from(payload)).await }
    }

    /// Enqueue adjustment command to per-channel TODO queue: comsrv:{channel}:A:TODO
    fn enqueue_adjustment(
        &self,
        channel_id: u32,
        payload_json: &str,
    ) -> impl Future<Output = Result<()>> + Send {
        let key = format!("comsrv:{}:A:TODO", channel_id);
        let payload = payload_json.to_string();
        async move { self.list_rpush(&key, Bytes::from(payload)).await }
    }
}
