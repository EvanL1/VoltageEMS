//! Trait definitions for RTDB abstraction

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::any::Any;
use std::collections::HashMap;
use voltage_config::common::RedisRoutingKeys;
use voltage_config::comsrv::RedisKeys;

/// Unified RTDB Storage Trait
///
/// Provides complete storage interface for VoltageEMS, combining:
/// - Basic key-value operations
/// - Structured data (Hash, List, Set)
/// - Domain-specific operations (points, mappings, commands)
///
/// Implementations:
/// - `RedisRtdb`: Production Redis backend
/// - `MemoryRtdb`: In-memory backend for testing
#[async_trait]
pub trait Rtdb: Send + Sync + 'static {
    // ========== Introspection ==========

    /// Allow downcasting to concrete types
    ///
    /// This enables runtime type checking and conversion to specific implementations
    /// like RedisRtdb or MemoryRtdb when needed.
    fn as_any(&self) -> &dyn Any;

    // ========== Basic Key-Value Operations ==========

    /// Get value by key
    async fn get(&self, key: &str) -> Result<Option<Bytes>>;

    /// Set value for key
    async fn set(&self, key: &str, value: Bytes) -> Result<()>;

    /// Delete key
    async fn del(&self, key: &str) -> Result<bool>;

    /// Check if key exists
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Increment key by float value (Redis INCRBYFLOAT)
    ///
    /// Returns the new value after incrementing.
    async fn incrbyfloat(&self, key: &str, increment: f64) -> Result<f64>;

    // ========== Hash Operations ==========

    /// Set hash field
    async fn hash_set(&self, key: &str, field: &str, value: Bytes) -> Result<()>;

    /// Get hash field
    async fn hash_get(&self, key: &str, field: &str) -> Result<Option<Bytes>>;

    /// Get multiple hash fields (Redis HMGET)
    ///
    /// Returns a vector of values corresponding to the requested fields.
    /// Non-existent fields are returned as None.
    async fn hash_mget(&self, key: &str, fields: &[&str]) -> Result<Vec<Option<Bytes>>>;

    /// Set multiple hash fields
    async fn hash_mset(&self, key: &str, fields: Vec<(String, Bytes)>) -> Result<()>;

    /// Get all hash fields
    async fn hash_get_all(&self, key: &str) -> Result<HashMap<String, Bytes>>;

    /// Delete hash field
    async fn hash_del(&self, key: &str, field: &str) -> Result<bool>;

    /// Increment hash field by value (Redis HINCRBY)
    ///
    /// Returns the new value after incrementing.
    async fn hincrby(&self, key: &str, field: &str, increment: i64) -> Result<i64>;

    // ========== List Operations ==========

    /// Push value to left of list
    async fn list_lpush(&self, key: &str, value: Bytes) -> Result<()>;

    /// Push value to right of list
    async fn list_rpush(&self, key: &str, value: Bytes) -> Result<()>;

    /// Pop value from left of list
    async fn list_lpop(&self, key: &str) -> Result<Option<Bytes>>;

    /// Pop value from right of list (Redis RPOP)
    ///
    /// Returns the popped value if the list is not empty, None otherwise.
    async fn list_rpop(&self, key: &str) -> Result<Option<Bytes>>;

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
    async fn list_blpop(
        &self,
        keys: &[&str],
        timeout_seconds: u64,
    ) -> Result<Option<(String, Bytes)>>;

    /// Get list range
    async fn list_range(&self, key: &str, start: isize, stop: isize) -> Result<Vec<Bytes>>;

    /// Trim list to range
    async fn list_trim(&self, key: &str, start: isize, stop: isize) -> Result<()>;

    // ========== Set Operations ==========

    /// Add member to set (Redis SADD)
    ///
    /// Returns true if the member was added, false if it already existed.
    async fn sadd(&self, key: &str, member: &str) -> Result<bool>;

    /// Remove member from set (Redis SREM)
    ///
    /// Returns true if the member was removed, false if it didn't exist.
    async fn srem(&self, key: &str, member: &str) -> Result<bool>;

    /// Get all members of a set (Redis SMEMBERS)
    ///
    /// Returns a vector of all members in the set.
    async fn smembers(&self, key: &str) -> Result<Vec<String>>;

    // ========== Messaging Operations ==========

    /// Publish message to channel (Redis Pub/Sub)
    ///
    /// Returns the number of subscribers that received the message.
    /// In test implementations (MemoryRtdb), this may return 0.
    async fn publish(&self, channel: &str, message: &str) -> Result<u32>;

    // ========== Function Operations ==========

    /// Call Redis function (FCALL)
    ///
    /// Executes a Lua function loaded in Redis with the given keys and arguments.
    /// Returns the function result as a String.
    /// In test implementations (MemoryRtdb), this may return a placeholder result.
    async fn fcall(&self, function: &str, keys: &[&str], args: &[&str]) -> Result<String>;

    // ========== Key Scanning Operations ==========

    /// Scan keys matching a pattern (Redis SCAN with MATCH)
    ///
    /// Returns a list of keys matching the glob pattern.
    /// In test implementations (MemoryRtdb), this searches in-memory keys.
    async fn scan_match(&self, pattern: &str) -> Result<Vec<String>>;

    // ========== Time Operations ==========

    /// Get current Redis server time in milliseconds (Redis TIME)
    ///
    /// Returns Unix timestamp in milliseconds.
    /// In test implementations (MemoryRtdb), this returns system time.
    async fn time_millis(&self) -> Result<i64>;

    // ========== Domain-Specific Operations (with default implementations) ==========

    /// Write point data to instance
    ///
    /// Stores data in format: `modsrv:{instance_name}:{point_role}`
    /// where point_role is "M" (measurement) or "A" (action)
    async fn write_point(
        &self,
        instance: &str,
        point_role: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let key = format!("modsrv:{}:{}", instance, point_role);
        let field = point_id.to_string();
        let value_bytes = Bytes::from(value.to_string());
        self.hash_set(&key, &field, value_bytes).await
    }

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
    async fn write_point_init(&self, key: &str, point_id: u32, value: f64) -> Result<()> {
        // Default implementation: write value + ts=0, no routing
        let field = point_id.to_string();
        let ts_field = format!("ts:{}", point_id);
        let value_bytes = Bytes::from(value.to_string());
        let ts_bytes = Bytes::from("0");

        self.hash_set(key, &field, value_bytes).await?;
        self.hash_set(key, &ts_field, ts_bytes).await?;
        Ok(())
    }

    /// Write point data in runtime mode (with routing trigger)
    ///
    /// This method writes point value with current timestamp, AND automatically triggers
    /// routing via TODO queue based on the routing_cache.
    ///
    /// # Operations
    /// 1. HSET {key}:{point_id} → {value}
    /// 2. HSET {key}:ts:{point_id} → {current_time_millis}
    /// 3. RPUSH {todo_queue} → {trigger_message} (if routing exists)
    ///
    /// # Routing Logic
    /// - "inst:X:A" → lookup M2C routing → write to "comsrv:Y:{A|C}:TODO"
    /// - "comsrv:X:A" or "comsrv:X:C" → direct write to "comsrv:X:{A|C}:TODO"
    /// - Other keys → no trigger
    ///
    /// # Arguments
    /// * `key` - Full point key (e.g., "inst:1:A" or "comsrv:1001:A")
    /// * `point_id` - Point identifier
    /// * `value` - Point value
    ///
    /// # Examples
    /// ```ignore
    /// // Write instance action point with trigger
    /// rtdb.write_point_runtime("inst:1:A", 10, 100.0).await?;
    /// // → Automatically writes to comsrv TODO queue if routing exists
    ///
    /// // Write channel adjustment point with trigger
    /// rtdb.write_point_runtime("comsrv:1001:A", 5, 12.3).await?;
    /// // → Automatically writes to comsrv:1001:A:TODO
    /// ```
    async fn write_point_runtime(&self, key: &str, point_id: u32, value: f64) -> Result<()> {
        // Default implementation: write value + current timestamp, no routing
        // Concrete implementations (RedisRtdb, MemoryRtdb) should override this
        // to add routing logic based on self.routing_cache
        let field = point_id.to_string();
        let ts_field = format!("ts:{}", point_id);
        let value_bytes = Bytes::from(value.to_string());
        let ts = self.time_millis().await?;
        let ts_bytes = Bytes::from(ts.to_string());

        self.hash_set(key, &field, value_bytes).await?;
        self.hash_set(key, &ts_field, ts_bytes).await?;
        Ok(())
    }

    /// Read point data from instance
    async fn read_point(
        &self,
        instance: &str,
        point_role: &str,
        point_id: u32,
    ) -> Result<Option<f64>> {
        let key = format!("modsrv:{}:{}", instance, point_role);
        let field = point_id.to_string();

        if let Some(bytes) = self.hash_get(&key, &field).await? {
            let s = String::from_utf8(bytes.to_vec())?;
            Ok(Some(s.parse()?))
        } else {
            Ok(None)
        }
    }

    /// Write multiple points in batch
    async fn write_points_batch(
        &self,
        instance: &str,
        point_role: &str,
        points: Vec<(u32, f64)>,
    ) -> Result<()> {
        let key = format!("modsrv:{}:{}", instance, point_role);
        let fields: Vec<(String, Bytes)> = points
            .into_iter()
            .map(|(id, val)| (id.to_string(), Bytes::from(val.to_string())))
            .collect();
        self.hash_mset(&key, fields).await
    }

    /// Write channel-to-model mapping
    async fn write_mapping(&self, from_key: &str, to_key: &str) -> Result<()> {
        self.hash_set(
            RedisRoutingKeys::CHANNEL_TO_MODEL,
            from_key,
            Bytes::from(to_key.to_string()),
        )
        .await
    }

    /// Read channel-to-model mapping
    async fn read_mapping(&self, from_key: &str) -> Result<Option<String>> {
        if let Some(bytes) = self
            .hash_get(RedisRoutingKeys::CHANNEL_TO_MODEL, from_key)
            .await?
        {
            Ok(Some(String::from_utf8(bytes.to_vec())?))
        } else {
            Ok(None)
        }
    }

    /// Enqueue control command to per-channel TODO queue: comsrv:{channel}:C:TODO
    async fn enqueue_control(&self, channel_id: u16, payload_json: &str) -> Result<()> {
        let key = RedisKeys::control_todo(channel_id);
        self.list_rpush(&key, Bytes::from(payload_json.to_string()))
            .await
    }

    /// Enqueue adjustment command to per-channel TODO queue: comsrv:{channel}:A:TODO
    async fn enqueue_adjustment(&self, channel_id: u16, payload_json: &str) -> Result<()> {
        let key = RedisKeys::adjustment_todo(channel_id);
        self.list_rpush(&key, Bytes::from(payload_json.to_string()))
            .await
    }

    /// Get all points for an instance
    async fn get_instance_points(
        &self,
        instance: &str,
        point_role: &str,
    ) -> Result<HashMap<u32, f64>> {
        let key = format!("modsrv:{}:{}", instance, point_role);
        let data = self.hash_get_all(&key).await?;

        let mut points = HashMap::new();
        for (field, value_bytes) in data {
            if let Ok(point_id) = field.parse::<u32>() {
                if let Ok(s) = String::from_utf8(value_bytes.to_vec()) {
                    if let Ok(value) = s.parse::<f64>() {
                        points.insert(point_id, value);
                    }
                }
            }
        }

        Ok(points)
    }
}
