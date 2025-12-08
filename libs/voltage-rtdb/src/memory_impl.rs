//! High-performance in-memory RTDB implementation
//!
//! Uses DashMap for lock-free concurrent access with excellent performance.
//! Perfect for testing and embedded scenarios.

use crate::traits::*;
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use dashmap::{DashMap, DashSet};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use voltage_config::RoutingCache;

/// In-memory RTDB implementation with concurrent access support
pub struct MemoryRtdb {
    kv_store: Arc<DashMap<String, Bytes>>,
    hash_store: Arc<DashMap<String, DashMap<String, Bytes>>>,
    list_store: Arc<DashMap<String, RwLock<Vec<Bytes>>>>,
    set_store: Arc<DashMap<String, DashSet<String>>>,
    routing_cache: Option<Arc<RoutingCache>>,
}

impl MemoryRtdb {
    /// Create new in-memory RTDB instance (without routing)
    pub fn new() -> Self {
        Self {
            kv_store: Arc::new(DashMap::new()),
            hash_store: Arc::new(DashMap::new()),
            list_store: Arc::new(DashMap::new()),
            set_store: Arc::new(DashMap::new()),
            routing_cache: None,
        }
    }

    /// Create in-memory RTDB with routing cache support
    ///
    /// This enables automatic routing trigger in `write_point_runtime()` method.
    pub fn with_routing(routing_cache: Arc<RoutingCache>) -> Self {
        Self {
            kv_store: Arc::new(DashMap::new()),
            hash_store: Arc::new(DashMap::new()),
            list_store: Arc::new(DashMap::new()),
            set_store: Arc::new(DashMap::new()),
            routing_cache: Some(routing_cache),
        }
    }

    /// Set routing cache (can be called after construction)
    ///
    /// Useful for injecting routing cache after RTDB creation.
    pub async fn set_routing_cache(&mut self, routing_cache: Arc<RoutingCache>) {
        self.routing_cache = Some(routing_cache);
    }

    /// Clear all data (useful for testing)
    pub fn clear(&self) {
        self.kv_store.clear();
        self.hash_store.clear();
        self.list_store.clear();
        self.set_store.clear();
    }

    /// Get statistics about stored data
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            kv_count: self.kv_store.len(),
            hash_count: self.hash_store.len(),
            list_count: self.list_store.len(),
            set_count: self.set_store.len(),
        }
    }
}

impl Default for MemoryRtdb {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about memory RTDB usage
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub kv_count: usize,
    pub hash_count: usize,
    pub list_count: usize,
    pub set_count: usize,
}

#[async_trait]
impl Rtdb for MemoryRtdb {
    async fn get(&self, key: &str) -> Result<Option<Bytes>> {
        Ok(self.kv_store.get(key).map(|v| v.clone()))
    }

    async fn set(&self, key: &str, value: Bytes) -> Result<()> {
        self.kv_store.insert(key.to_string(), value);
        Ok(())
    }

    async fn del(&self, key: &str) -> Result<bool> {
        Ok(self.kv_store.remove(key).is_some())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.kv_store.contains_key(key))
    }

    async fn incrbyfloat(&self, key: &str, increment: f64) -> Result<f64> {
        // Get current value or default to 0.0
        let current_value = if let Some(bytes) = self.kv_store.get(key) {
            let s = String::from_utf8(bytes.to_vec())?;
            s.parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };

        // Calculate new value
        let new_value = current_value + increment;

        // Store new value
        self.kv_store
            .insert(key.to_string(), Bytes::from(new_value.to_string()));

        Ok(new_value)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn hash_set(&self, key: &str, field: &str, value: Bytes) -> Result<()> {
        self.hash_store
            .entry(key.to_string())
            .or_default()
            .insert(field.to_string(), value);
        Ok(())
    }

    async fn hash_get(&self, key: &str, field: &str) -> Result<Option<Bytes>> {
        Ok(self
            .hash_store
            .get(key)
            .and_then(|hash| hash.get(field).map(|v| v.clone())))
    }

    async fn hash_mget(&self, key: &str, fields: &[&str]) -> Result<Vec<Option<Bytes>>> {
        if let Some(hash) = self.hash_store.get(key) {
            Ok(fields
                .iter()
                .map(|field| hash.get(*field).map(|v| v.clone()))
                .collect())
        } else {
            // Key doesn't exist, return None for all fields
            Ok(vec![None; fields.len()])
        }
    }

    async fn hash_mset(&self, key: &str, fields: Vec<(String, Bytes)>) -> Result<()> {
        let hash = self.hash_store.entry(key.to_string()).or_default();

        for (field, value) in fields {
            hash.insert(field, value);
        }
        Ok(())
    }

    async fn hash_get_all(&self, key: &str) -> Result<HashMap<String, Bytes>> {
        if let Some(hash) = self.hash_store.get(key) {
            Ok(hash
                .iter()
                .map(|entry| (entry.key().clone(), entry.value().clone()))
                .collect())
        } else {
            Ok(HashMap::new())
        }
    }

    async fn hash_del(&self, key: &str, field: &str) -> Result<bool> {
        if let Some(hash) = self.hash_store.get(key) {
            Ok(hash.remove(field).is_some())
        } else {
            Ok(false)
        }
    }

    async fn hash_del_many(&self, key: &str, fields: &[String]) -> Result<usize> {
        if let Some(hash) = self.hash_store.get(key) {
            let mut removed = 0;
            for field in fields {
                if hash.remove(field).is_some() {
                    removed += 1;
                }
            }
            Ok(removed)
        } else {
            Ok(0)
        }
    }

    async fn list_lpush(&self, key: &str, value: Bytes) -> Result<()> {
        self.list_store
            .entry(key.to_string())
            .or_insert_with(|| RwLock::new(Vec::new()))
            .write()
            .insert(0, value);
        Ok(())
    }

    async fn list_rpush(&self, key: &str, value: Bytes) -> Result<()> {
        self.list_store
            .entry(key.to_string())
            .or_insert_with(|| RwLock::new(Vec::new()))
            .write()
            .push(value);
        Ok(())
    }

    async fn list_lpop(&self, key: &str) -> Result<Option<Bytes>> {
        Ok(self.list_store.get(key).and_then(|list| {
            let mut list = list.write();
            if list.is_empty() {
                None
            } else {
                Some(list.remove(0))
            }
        }))
    }

    async fn list_rpop(&self, key: &str) -> Result<Option<Bytes>> {
        Ok(self.list_store.get(key).and_then(|list| {
            let mut list = list.write();
            // list.pop() returns Option<Bytes>; returns Some when not empty
            list.pop()
        }))
    }

    async fn list_blpop(
        &self,
        keys: &[&str],
        timeout_seconds: u64,
    ) -> Result<Option<(String, Bytes)>> {
        use tokio::time::{sleep, Duration, Instant};

        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_seconds);

        // Poll keys until timeout or data found
        loop {
            // Try to pop from each key in order
            for key in keys {
                if let Some(list) = self.list_store.get(*key) {
                    let mut list = list.write();
                    if !list.is_empty() {
                        let value = list.remove(0);
                        return Ok(Some((key.to_string(), value)));
                    }
                }
            }

            // Check timeout
            if start.elapsed() >= timeout {
                return Ok(None);
            }

            // Sleep briefly before next poll (10ms)
            sleep(Duration::from_millis(10)).await;
        }
    }

    async fn list_range(&self, key: &str, start: isize, stop: isize) -> Result<Vec<Bytes>> {
        if let Some(list) = self.list_store.get(key) {
            let list = list.read();
            let len = list.len() as isize;

            // Handle negative indices
            let start_idx = if start < 0 {
                (len + start).max(0) as usize
            } else {
                start.min(len) as usize
            };

            let stop_idx = if stop < 0 {
                (len + stop + 1).max(0) as usize
            } else {
                (stop + 1).min(len) as usize
            };

            if start_idx < stop_idx {
                Ok(list[start_idx..stop_idx].to_vec())
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }

    async fn list_trim(&self, key: &str, start: isize, stop: isize) -> Result<()> {
        if let Some(list) = self.list_store.get(key) {
            let mut list = list.write();
            let len = list.len() as isize;

            let start_idx = if start < 0 {
                (len + start).max(0) as usize
            } else {
                start.min(len) as usize
            };

            let stop_idx = if stop < 0 {
                (len + stop + 1).max(0) as usize
            } else {
                (stop + 1).min(len) as usize
            };

            if start_idx < stop_idx && stop_idx <= list.len() {
                *list = list[start_idx..stop_idx].to_vec();
            } else {
                list.clear();
            }
        }
        Ok(())
    }

    async fn publish(&self, channel: &str, message: &str) -> Result<u32> {
        // Test stub: log the publish operation but don't actually send
        tracing::trace!(
            "MemoryRtdb: PUBLISH to channel '{}' with message: {}",
            channel,
            message
        );
        Ok(0) // Return 0 subscribers in test mode
    }

    async fn fcall(&self, function: &str, keys: &[&str], args: &[&str]) -> Result<String> {
        // Test stub: log the function call but don't execute
        tracing::trace!(
            "MemoryRtdb: FCALL function '{}' with keys={:?} args={:?}",
            function,
            keys,
            args
        );
        Ok("OK".to_string()) // Return placeholder success result
    }

    async fn scan_match(&self, pattern: &str) -> Result<Vec<String>> {
        // Test stub: simple pattern matching on in-memory keys
        tracing::trace!("MemoryRtdb: SCAN MATCH pattern '{}'", pattern);

        // Convert glob pattern to regex (simple implementation for testing)
        let regex_pattern = pattern.replace("*", ".*").replace("?", ".");
        let re = regex::Regex::new(&format!("^{}$", regex_pattern))?;

        let mut matches = Vec::new();

        // Scan KV store
        for entry in self.kv_store.iter() {
            if re.is_match(entry.key()) {
                matches.push(entry.key().clone());
            }
        }

        // Scan hash store
        for entry in self.hash_store.iter() {
            if re.is_match(entry.key()) {
                matches.push(entry.key().clone());
            }
        }

        // Scan list store
        for entry in self.list_store.iter() {
            if re.is_match(entry.key()) {
                matches.push(entry.key().clone());
            }
        }

        matches.sort();
        matches.dedup();
        Ok(matches)
    }

    async fn sadd(&self, key: &str, member: &str) -> Result<bool> {
        let set = self.set_store.entry(key.to_string()).or_default();
        Ok(set.insert(member.to_string()))
    }

    async fn srem(&self, key: &str, member: &str) -> Result<bool> {
        if let Some(set) = self.set_store.get(key) {
            Ok(set.remove(member).is_some())
        } else {
            Ok(false)
        }
    }

    async fn smembers(&self, key: &str) -> Result<Vec<String>> {
        if let Some(set) = self.set_store.get(key) {
            let members: Vec<String> = set.iter().map(|entry| entry.key().clone()).collect();
            Ok(members)
        } else {
            Ok(Vec::new())
        }
    }

    async fn hincrby(&self, key: &str, field: &str, increment: i64) -> Result<i64> {
        let hash = self.hash_store.entry(key.to_string()).or_default();

        // Get current value or default to 0
        let current_value = if let Some(bytes) = hash.get(field) {
            let s = String::from_utf8(bytes.to_vec())?;
            s.parse::<i64>().unwrap_or(0)
        } else {
            0
        };

        // Calculate new value
        let new_value = current_value + increment;

        // Store new value
        hash.insert(field.to_string(), Bytes::from(new_value.to_string()));

        Ok(new_value)
    }

    async fn time_millis(&self) -> Result<i64> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow::anyhow!("System time error: {}", e))?;
        Ok(duration.as_millis() as i64)
    }

    async fn pipeline_hash_mset(
        &self,
        operations: Vec<(String, Vec<(String, Bytes)>)>,
    ) -> Result<()> {
        // For in-memory implementation, just execute each HSET sequentially
        // This is efficient since it's all in-memory with no network overhead
        for (key, fields) in operations {
            if !fields.is_empty() {
                let hash = self.hash_store.entry(key).or_default();
                for (field, value) in fields {
                    hash.insert(field, value);
                }
            }
        }
        Ok(())
    }

    // ========== Override Domain-Specific Methods with Routing Logic ==========

    async fn write_point_runtime(&self, key: &str, point_id: u32, value: f64) -> Result<()> {
        // 1. Write value and timestamp to Hash
        let field = point_id.to_string();
        let ts_field = format!("ts:{}", point_id);
        let value_bytes = Bytes::from(value.to_string());
        let ts = self.time_millis().await?;
        let ts_bytes = Bytes::from(ts.to_string());

        self.hash_set(key, &field, value_bytes.clone()).await?;
        self.hash_set(key, &ts_field, ts_bytes).await?;

        // 2. Trigger routing if routing_cache exists
        if let Some(ref routing_cache) = self.routing_cache {
            if let Some(todo_key) = self.resolve_todo_queue(key, point_id, routing_cache) {
                // Build trigger message: {"point_id": X, "value": Y, "timestamp": Z}
                let trigger_msg = format!(
                    r#"{{"point_id":{},"value":{},"timestamp":{}}}"#,
                    point_id, value, ts
                );
                self.list_rpush(&todo_key, Bytes::from(trigger_msg)).await?;
            }
        }

        Ok(())
    }
}

impl MemoryRtdb {
    /// Resolve TODO queue key based on point key and routing cache
    ///
    /// # Logic
    /// - "inst:X:A:point" → lookup M2C routing → "comsrv:Y:{A|C}:TODO"
    /// - "comsrv:X:A" or "comsrv:X:C" → "comsrv:X:{A|C}:TODO"
    /// - Other keys → None (no trigger)
    fn resolve_todo_queue(
        &self,
        key: &str,
        point_id: u32,
        routing_cache: &voltage_config::RoutingCache,
    ) -> Option<String> {
        let parts: Vec<&str> = key.split(':').collect();

        // Case 1: Instance action point "inst:X:A"
        if parts.len() >= 3 && parts[0] == "inst" && parts[2] == "A" {
            let instance_id = parts[1];
            let routing_key = format!("{}:A:{}", instance_id, point_id);

            if let Some(target) = routing_cache.lookup_m2c(&routing_key) {
                // target format: "channel_id:A:point_id" or "channel_id:C:point_id"
                let target_parts: Vec<&str> = target.split(':').collect();
                if target_parts.len() >= 2 {
                    let channel_id = target_parts[0];
                    let point_type = target_parts[1]; // "A" or "C"
                    return Some(format!("comsrv:{}:{}:TODO", channel_id, point_type));
                }
            }
        }

        // Case 2: Channel action/control point "comsrv:X:A" or "comsrv:X:C"
        if parts.len() >= 3 && parts[0] == "comsrv" && (parts[2] == "A" || parts[2] == "C") {
            let channel_id = parts[1];
            let point_type = parts[2];
            return Some(format!("comsrv:{}:{}:TODO", channel_id, point_type));
        }

        // Case 3: Other keys - no trigger
        None
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_rtdb_kv_operations() {
        let rtdb = MemoryRtdb::new();

        // Test set and get
        rtdb.set("test:key", Bytes::from("value")).await.unwrap();
        let value = rtdb.get("test:key").await.unwrap();
        assert_eq!(value, Some(Bytes::from("value")));

        // Test exists
        assert!(rtdb.exists("test:key").await.unwrap());
        assert!(!rtdb.exists("nonexistent").await.unwrap());

        // Test delete
        assert!(rtdb.del("test:key").await.unwrap());
        assert!(!rtdb.exists("test:key").await.unwrap());
        assert!(!rtdb.del("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_rtdb_hash_operations() {
        let rtdb = MemoryRtdb::new();

        // Test hash set/get
        rtdb.hash_set("test:hash", "field1", Bytes::from("value1"))
            .await
            .unwrap();
        rtdb.hash_set("test:hash", "field2", Bytes::from("value2"))
            .await
            .unwrap();

        let value = rtdb.hash_get("test:hash", "field1").await.unwrap();
        assert_eq!(value, Some(Bytes::from("value1")));

        // Test hash_get_all
        let all = rtdb.hash_get_all("test:hash").await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("field1"), Some(&Bytes::from("value1")));

        // Test hash_mset
        rtdb.hash_mset(
            "test:hash2",
            vec![
                ("f1".to_string(), Bytes::from("v1")),
                ("f2".to_string(), Bytes::from("v2")),
            ],
        )
        .await
        .unwrap();

        let all2 = rtdb.hash_get_all("test:hash2").await.unwrap();
        assert_eq!(all2.len(), 2);

        // Test hash_del
        assert!(rtdb.hash_del("test:hash", "field1").await.unwrap());
        assert!(!rtdb.hash_del("test:hash", "nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_rtdb_list_operations() {
        let rtdb = MemoryRtdb::new();

        // Test lpush and lpop
        rtdb.list_lpush("test:list", Bytes::from("value1"))
            .await
            .unwrap();
        rtdb.list_lpush("test:list", Bytes::from("value2"))
            .await
            .unwrap();

        let value = rtdb.list_lpop("test:list").await.unwrap();
        assert_eq!(value, Some(Bytes::from("value2")));

        // Test rpush
        rtdb.list_rpush("test:list", Bytes::from("value3"))
            .await
            .unwrap();

        // Test list_range
        let range = rtdb.list_range("test:list", 0, -1).await.unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0], Bytes::from("value1"));
        assert_eq!(range[1], Bytes::from("value3"));

        // Test list_trim
        rtdb.list_trim("test:list", 0, 0).await.unwrap();
        let range = rtdb.list_range("test:list", 0, -1).await.unwrap();
        assert_eq!(range.len(), 1);
    }

    #[tokio::test]
    async fn test_memory_rtdb_point_operations() {
        let rtdb = MemoryRtdb::new();

        // Test write_point and read_point
        rtdb.write_point("instance1", "M", 1, 100.5).await.unwrap();
        rtdb.write_point("instance1", "M", 2, 200.3).await.unwrap();

        let value = rtdb.read_point("instance1", "M", 1).await.unwrap();
        assert_eq!(value, Some(100.5));

        // Test write_points_batch
        rtdb.write_points_batch("instance2", "A", vec![(1, 50.0), (2, 75.5)])
            .await
            .unwrap();

        let points = rtdb.get_instance_points("instance2", "A").await.unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points.get(&1), Some(&50.0));
        assert_eq!(points.get(&2), Some(&75.5));
    }

    #[tokio::test]
    async fn test_memory_rtdb_mapping_operations() {
        let rtdb = MemoryRtdb::new();

        // Test write_mapping and read_mapping
        rtdb.write_mapping("channel:1:T:1", "modsrv:inst1:M:1")
            .await
            .unwrap();

        let mapping = rtdb.read_mapping("channel:1:T:1").await.unwrap();
        assert_eq!(mapping, Some("modsrv:inst1:M:1".to_string()));

        let missing = rtdb.read_mapping("nonexistent").await.unwrap();
        assert_eq!(missing, None);
    }

    #[tokio::test]
    async fn test_memory_rtdb_todo_queues() {
        use voltage_config::comsrv::ChannelRedisKeys;

        let rtdb = MemoryRtdb::new();

        // Enqueue control/adjustment into per-channel queues
        rtdb.enqueue_control(1001, r#"{"cmd":"c1"}"#).await.unwrap();
        rtdb.enqueue_adjustment(1001, r#"{"cmd":"a1"}"#)
            .await
            .unwrap();

        // Verify queue contents by popping from keys
        let c_key = ChannelRedisKeys::control_todo(1001);
        let a_key = ChannelRedisKeys::adjustment_todo(1001);

        let c1 = rtdb.list_lpop(&c_key).await.unwrap();
        assert_eq!(c1, Some(Bytes::from(r#"{"cmd":"c1"}"#)));

        let a1 = rtdb.list_lpop(&a_key).await.unwrap();
        assert_eq!(a1, Some(Bytes::from(r#"{"cmd":"a1"}"#)));

        // Now empty
        let empty = rtdb.list_lpop(&a_key).await.unwrap();
        assert_eq!(empty, None);
    }

    #[tokio::test]
    async fn test_memory_rtdb_stats() {
        let rtdb = MemoryRtdb::new();

        rtdb.set("key1", Bytes::from("value")).await.unwrap();
        rtdb.hash_set("hash1", "field", Bytes::from("value"))
            .await
            .unwrap();
        rtdb.list_lpush("list1", Bytes::from("value"))
            .await
            .unwrap();

        let stats = rtdb.stats();
        assert_eq!(stats.kv_count, 1);
        assert_eq!(stats.hash_count, 1);
        assert_eq!(stats.list_count, 1);

        rtdb.clear();
        let stats = rtdb.stats();
        assert_eq!(stats.kv_count, 0);
        assert_eq!(stats.hash_count, 0);
        assert_eq!(stats.list_count, 0);
    }

    // ========== Boundary Case Tests ==========

    #[tokio::test]
    async fn test_empty_key_operations() {
        let rtdb = MemoryRtdb::new();

        // Empty key should work
        rtdb.set("", Bytes::from("value")).await.unwrap();
        let value = rtdb.get("").await.unwrap();
        assert_eq!(value, Some(Bytes::from("value")));

        // Empty hash key
        rtdb.hash_set("", "field", Bytes::from("value"))
            .await
            .unwrap();
        let value = rtdb.hash_get("", "field").await.unwrap();
        assert_eq!(value, Some(Bytes::from("value")));
    }

    #[tokio::test]
    async fn test_empty_value_operations() {
        let rtdb = MemoryRtdb::new();

        // Empty value
        rtdb.set("key", Bytes::from("")).await.unwrap();
        let value = rtdb.get("key").await.unwrap();
        assert_eq!(value, Some(Bytes::from("")));

        // Empty hash field value
        rtdb.hash_set("hash", "field", Bytes::from(""))
            .await
            .unwrap();
        let value = rtdb.hash_get("hash", "field").await.unwrap();
        assert_eq!(value, Some(Bytes::from("")));
    }

    #[tokio::test]
    async fn test_nonexistent_key_operations() {
        let rtdb = MemoryRtdb::new();

        // Get nonexistent key
        let value = rtdb.get("nonexistent").await.unwrap();
        assert_eq!(value, None);

        // Delete nonexistent key
        assert!(!rtdb.del("nonexistent").await.unwrap());

        // Hash operations on nonexistent key
        let value = rtdb.hash_get("nonexistent", "field").await.unwrap();
        assert_eq!(value, None);

        let all = rtdb.hash_get_all("nonexistent").await.unwrap();
        assert_eq!(all.len(), 0);

        // List operations on nonexistent key
        let value = rtdb.list_lpop("nonexistent").await.unwrap();
        assert_eq!(value, None);

        let range = rtdb.list_range("nonexistent", 0, -1).await.unwrap();
        assert_eq!(range.len(), 0);
    }

    #[tokio::test]
    async fn test_invalid_utf8_handling() {
        let rtdb = MemoryRtdb::new();

        // Store invalid UTF-8 bytes
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        rtdb.set("invalid", Bytes::from(invalid_utf8.clone()))
            .await
            .unwrap();

        let value = rtdb.get("invalid").await.unwrap().unwrap();
        assert_eq!(value.to_vec(), invalid_utf8);
    }

    #[tokio::test]
    async fn test_point_parse_failure() {
        let rtdb = MemoryRtdb::new();

        // Write invalid float string
        rtdb.hash_set("modsrv:inst1:M", "1", Bytes::from("not_a_number"))
            .await
            .unwrap();

        // read_point should fail to parse
        let result = rtdb.read_point("inst1", "M", 1).await;
        assert!(result.is_err(), "Should fail to parse invalid float");
    }

    #[tokio::test]
    async fn test_list_range_boundaries() {
        let rtdb = MemoryRtdb::new();

        // Populate list
        for i in 0..5 {
            rtdb.list_rpush("list", Bytes::from(format!("value{}", i)))
                .await
                .unwrap();
        }

        // Negative indices
        let range = rtdb.list_range("list", -2, -1).await.unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0], Bytes::from("value3"));
        assert_eq!(range[1], Bytes::from("value4"));

        // Out of bounds (should return empty or partial)
        let range = rtdb.list_range("list", 10, 20).await.unwrap();
        assert_eq!(range.len(), 0);

        // Reverse range (start > stop)
        let range = rtdb.list_range("list", 3, 1).await.unwrap();
        assert_eq!(range.len(), 0);
    }

    #[tokio::test]
    async fn test_list_trim_boundaries() {
        let rtdb = MemoryRtdb::new();

        // Populate list
        for i in 0..5 {
            rtdb.list_rpush("list", Bytes::from(format!("value{}", i)))
                .await
                .unwrap();
        }

        // Trim to negative range
        rtdb.list_trim("list", -3, -1).await.unwrap();
        let range = rtdb.list_range("list", 0, -1).await.unwrap();
        assert_eq!(range.len(), 3);

        // Trim out of bounds (should clear)
        rtdb.list_trim("list", 10, 20).await.unwrap();
        let range = rtdb.list_range("list", 0, -1).await.unwrap();
        assert_eq!(range.len(), 0);
    }

    #[tokio::test]
    async fn test_special_float_values() {
        let rtdb = MemoryRtdb::new();

        // Test NaN
        rtdb.write_point("inst1", "M", 1, f64::NAN).await.unwrap();
        let value = rtdb.read_point("inst1", "M", 1).await.unwrap().unwrap();
        assert!(value.is_nan());

        // Test Infinity
        rtdb.write_point("inst1", "M", 2, f64::INFINITY)
            .await
            .unwrap();
        let value = rtdb.read_point("inst1", "M", 2).await.unwrap().unwrap();
        assert_eq!(value, f64::INFINITY);

        // Test negative infinity
        rtdb.write_point("inst1", "M", 3, f64::NEG_INFINITY)
            .await
            .unwrap();
        let value = rtdb.read_point("inst1", "M", 3).await.unwrap().unwrap();
        assert_eq!(value, f64::NEG_INFINITY);

        // Test zero
        rtdb.write_point("inst1", "M", 4, 0.0).await.unwrap();
        let value = rtdb.read_point("inst1", "M", 4).await.unwrap().unwrap();
        assert_eq!(value, 0.0);

        // Test negative zero
        rtdb.write_point("inst1", "M", 5, -0.0).await.unwrap();
        let value = rtdb.read_point("inst1", "M", 5).await.unwrap().unwrap();
        assert_eq!(value, -0.0);
    }

    #[tokio::test]
    async fn test_empty_batch_operations() {
        let rtdb = MemoryRtdb::new();

        // Empty batch write
        rtdb.write_points_batch("inst1", "M", vec![]).await.unwrap();

        let points = rtdb.get_instance_points("inst1", "M").await.unwrap();
        assert_eq!(points.len(), 0);

        // Empty hash_mset
        rtdb.hash_mset("hash", vec![]).await.unwrap();
        let all = rtdb.hash_get_all("hash").await.unwrap();
        assert_eq!(all.len(), 0);
    }

    #[tokio::test]
    async fn test_large_field_names() {
        let rtdb = MemoryRtdb::new();

        // Very long key
        let long_key = "a".repeat(1000);
        rtdb.set(&long_key, Bytes::from("value")).await.unwrap();
        let value = rtdb.get(&long_key).await.unwrap();
        assert_eq!(value, Some(Bytes::from("value")));

        // Very long hash field
        let long_field = "f".repeat(1000);
        rtdb.hash_set("hash", &long_field, Bytes::from("value"))
            .await
            .unwrap();
        let value = rtdb.hash_get("hash", &long_field).await.unwrap();
        assert_eq!(value, Some(Bytes::from("value")));
    }

    // ========== Concurrent Access Tests ==========

    #[tokio::test]
    async fn test_concurrent_kv_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let rtdb = Arc::new(MemoryRtdb::new());
        let mut tasks = JoinSet::new();

        // Spawn 100 concurrent writers
        for i in 0..100 {
            let rtdb_clone = rtdb.clone();
            tasks.spawn(async move {
                rtdb_clone
                    .set(&format!("key{}", i), Bytes::from(format!("value{}", i)))
                    .await
                    .unwrap();
            });
        }

        // Wait for all writes
        while tasks.join_next().await.is_some() {}

        // Verify all writes succeeded
        for i in 0..100 {
            let value = rtdb.get(&format!("key{}", i)).await.unwrap();
            assert_eq!(value, Some(Bytes::from(format!("value{}", i))));
        }
    }

    // ========== New Method Tests ==========

    #[tokio::test]
    async fn test_set_operations() {
        let rtdb = MemoryRtdb::new();

        // Test sadd - adding new member
        assert!(rtdb.sadd("test:set", "member1").await.unwrap());
        assert!(rtdb.sadd("test:set", "member2").await.unwrap());
        assert!(rtdb.sadd("test:set", "member3").await.unwrap());

        // Test sadd - adding duplicate member (should return false)
        assert!(!rtdb.sadd("test:set", "member1").await.unwrap());

        // Test smembers
        let members = rtdb.smembers("test:set").await.unwrap();
        assert_eq!(members.len(), 3);
        assert!(members.contains(&"member1".to_string()));
        assert!(members.contains(&"member2".to_string()));
        assert!(members.contains(&"member3".to_string()));

        // Test srem - removing existing member
        assert!(rtdb.srem("test:set", "member2").await.unwrap());
        let members = rtdb.smembers("test:set").await.unwrap();
        assert_eq!(members.len(), 2);
        assert!(!members.contains(&"member2".to_string()));

        // Test srem - removing non-existent member
        assert!(!rtdb.srem("test:set", "nonexistent").await.unwrap());

        // Test smembers on non-existent set
        let empty = rtdb.smembers("nonexistent:set").await.unwrap();
        assert_eq!(empty.len(), 0);
    }

    #[tokio::test]
    async fn test_hincrby_operation() {
        let rtdb = MemoryRtdb::new();

        // Test hincrby on non-existent field (should initialize to increment)
        let result = rtdb.hincrby("test:hash", "counter", 5).await.unwrap();
        assert_eq!(result, 5);

        // Test hincrby on existing field
        let result = rtdb.hincrby("test:hash", "counter", 10).await.unwrap();
        assert_eq!(result, 15);

        // Test negative increment
        let result = rtdb.hincrby("test:hash", "counter", -3).await.unwrap();
        assert_eq!(result, 12);

        // Test increment by zero
        let result = rtdb.hincrby("test:hash", "counter", 0).await.unwrap();
        assert_eq!(result, 12);

        // Test different field in same hash
        let result = rtdb
            .hincrby("test:hash", "other_counter", 100)
            .await
            .unwrap();
        assert_eq!(result, 100);
    }

    #[tokio::test]
    async fn test_time_millis_operation() {
        let rtdb = MemoryRtdb::new();

        // Test time_millis returns a reasonable value
        let time1 = rtdb.time_millis().await.unwrap();
        assert!(time1 > 0);

        // Test time progresses (sleep for a bit)
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let time2 = rtdb.time_millis().await.unwrap();
        assert!(time2 > time1);

        // Verify time is in milliseconds since UNIX_EPOCH
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let time_secs = time2 / 1000;
        assert!(time_secs >= now_secs - 1 && time_secs <= now_secs + 1);
    }

    #[tokio::test]
    async fn test_stats_includes_set_count() {
        let rtdb = MemoryRtdb::new();

        // Initially all counts should be 0
        let stats = rtdb.stats();
        assert_eq!(stats.set_count, 0);

        // Add some data to different stores
        rtdb.set("key1", Bytes::from("value")).await.unwrap();
        rtdb.hash_set("hash1", "field", Bytes::from("value"))
            .await
            .unwrap();
        rtdb.list_lpush("list1", Bytes::from("value"))
            .await
            .unwrap();
        rtdb.sadd("set1", "member1").await.unwrap();

        let stats = rtdb.stats();
        assert_eq!(stats.kv_count, 1);
        assert_eq!(stats.hash_count, 1);
        assert_eq!(stats.list_count, 1);
        assert_eq!(stats.set_count, 1);

        // Clear and verify all counts reset
        rtdb.clear();
        let stats = rtdb.stats();
        assert_eq!(stats.kv_count, 0);
        assert_eq!(stats.hash_count, 0);
        assert_eq!(stats.list_count, 0);
        assert_eq!(stats.set_count, 0);
    }

    #[tokio::test]
    async fn test_concurrent_set_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let rtdb = Arc::new(MemoryRtdb::new());
        let mut tasks = JoinSet::new();

        // Spawn 50 concurrent sadd operations
        for i in 0..50 {
            let rtdb_clone = rtdb.clone();
            tasks.spawn(async move {
                rtdb_clone
                    .sadd("shared_set", &format!("member{}", i))
                    .await
                    .unwrap();
            });
        }

        while tasks.join_next().await.is_some() {}

        // Verify all members exist
        let members = rtdb.smembers("shared_set").await.unwrap();
        assert_eq!(members.len(), 50);
    }

    #[tokio::test]
    async fn test_concurrent_hincrby_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let rtdb = Arc::new(MemoryRtdb::new());
        let mut tasks = JoinSet::new();

        // Spawn 100 concurrent increments
        for _ in 0..100 {
            let rtdb_clone = rtdb.clone();
            tasks.spawn(async move {
                rtdb_clone.hincrby("test:hash", "counter", 1).await.unwrap();
            });
        }

        while tasks.join_next().await.is_some() {}

        // Final value should be 100
        let result = rtdb.hincrby("test:hash", "counter", 0).await.unwrap();
        assert_eq!(result, 100);
    }

    #[tokio::test]
    async fn test_concurrent_hash_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let rtdb = Arc::new(MemoryRtdb::new());
        let mut tasks = JoinSet::new();

        // Spawn 50 concurrent hash writers to same key
        for i in 0..50 {
            let rtdb_clone = rtdb.clone();
            tasks.spawn(async move {
                rtdb_clone
                    .hash_set(
                        "shared_hash",
                        &format!("field{}", i),
                        Bytes::from(format!("value{}", i)),
                    )
                    .await
                    .unwrap();
            });
        }

        while tasks.join_next().await.is_some() {}

        // Verify all fields exist
        let all = rtdb.hash_get_all("shared_hash").await.unwrap();
        assert_eq!(all.len(), 50);
    }

    #[tokio::test]
    async fn test_concurrent_list_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let rtdb = Arc::new(MemoryRtdb::new());
        let mut tasks = JoinSet::new();

        // Spawn 100 concurrent list pushers
        for i in 0..100 {
            let rtdb_clone = rtdb.clone();
            tasks.spawn(async move {
                rtdb_clone
                    .list_lpush("shared_list", Bytes::from(format!("value{}", i)))
                    .await
                    .unwrap();
            });
        }

        while tasks.join_next().await.is_some() {}

        // Verify list has 100 elements
        let range = rtdb.list_range("shared_list", 0, -1).await.unwrap();
        assert_eq!(range.len(), 100);
    }

    #[tokio::test]
    async fn test_concurrent_mixed_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let rtdb = Arc::new(MemoryRtdb::new());
        let mut tasks = JoinSet::new();

        // Mix of reads and writes
        for i in 0..50 {
            let rtdb_clone = rtdb.clone();
            tasks.spawn(async move {
                // Write
                rtdb_clone
                    .set(&format!("key{}", i), Bytes::from(format!("value{}", i)))
                    .await
                    .unwrap();
                // Read
                let _ = rtdb_clone.get(&format!("key{}", i)).await;
                // Delete
                rtdb_clone.del(&format!("key{}", i)).await.unwrap();
            });
        }

        while tasks.join_next().await.is_some() {}

        // All keys should be deleted
        for i in 0..50 {
            assert!(!rtdb.exists(&format!("key{}", i)).await.unwrap());
        }
    }

    // ========== Dual-Mode Write Tests (Init vs Runtime) ==========

    #[tokio::test]
    async fn test_write_point_init_mode() {
        let rtdb = MemoryRtdb::new();

        // Write Instance Action point (Init mode)
        rtdb.write_point_init("inst:1:A", 10, 100.0).await.unwrap();

        // Verify: value is written (100.0 may be stored as "100" or "100.0")
        let value = rtdb.hash_get("inst:1:A", "10").await.unwrap();
        let value_str = String::from_utf8(value.unwrap().to_vec()).unwrap();
        let value_f64: f64 = value_str.parse().unwrap();
        assert_eq!(value_f64, 100.0);

        // Verify: timestamp is 0
        let ts = rtdb.hash_get("inst:1:A", "ts:10").await.unwrap();
        assert_eq!(ts, Some(Bytes::from("0")));

        // Verify: no TODO queue written (any possible TODO key should not exist)
        let todo_keys = ["comsrv:1001:A:TODO", "comsrv:1001:C:TODO"];
        for key in &todo_keys {
            let data = rtdb.list_range(key, 0, -1).await.unwrap();
            assert_eq!(
                data.len(),
                0,
                "No TODO queue should be triggered in init mode"
            );
        }
    }

    #[tokio::test]
    async fn test_write_point_runtime_without_routing() {
        // Runtime mode without routing_cache (using new())
        let rtdb = MemoryRtdb::new();

        // Write Instance Action point (Runtime mode, no cache)
        rtdb.write_point_runtime("inst:1:A", 10, 100.0)
            .await
            .unwrap();

        // Verify: value is written (100.0 may be stored as "100" or "100.0")
        let value = rtdb.hash_get("inst:1:A", "10").await.unwrap();
        let value_str = String::from_utf8(value.unwrap().to_vec()).unwrap();
        let value_f64: f64 = value_str.parse().unwrap();
        assert_eq!(value_f64, 100.0);

        // Verify: timestamp is non-zero (current time)
        let ts_bytes = rtdb.hash_get("inst:1:A", "ts:10").await.unwrap().unwrap();
        let ts_str = String::from_utf8(ts_bytes.to_vec()).unwrap();
        let ts: i64 = ts_str.parse().unwrap();
        assert!(ts > 0, "Timestamp should be current time in runtime mode");

        // Verify: no TODO queue triggered (no routing_cache)
        let scan_result = rtdb.scan_match("*:TODO").await.unwrap();
        assert_eq!(scan_result.len(), 0, "No TODO queues without routing_cache");
    }

    #[tokio::test]
    async fn test_write_point_runtime_instance_action_with_routing() {
        use std::collections::HashMap;

        // Build routing cache: inst 1's A point 10 → channel 1001's A point 5
        let mut m2c_data = HashMap::new();
        m2c_data.insert("1:A:10".to_string(), "1001:A:5".to_string());
        let cache = Arc::new(voltage_config::RoutingCache::from_maps(
            HashMap::new(), // C2M routing
            m2c_data,
            HashMap::new(), // C2C routing (not used in this test)
        ));

        let rtdb = MemoryRtdb::with_routing(cache);

        // Write Instance Action point (Runtime mode with routing)
        rtdb.write_point_runtime("inst:1:A", 10, 100.0)
            .await
            .unwrap();

        // Verify: value is written (100.0 may be stored as "100" or "100.0")
        let value = rtdb.hash_get("inst:1:A", "10").await.unwrap();
        let value_str = String::from_utf8(value.unwrap().to_vec()).unwrap();
        let value_f64: f64 = value_str.parse().unwrap();
        assert_eq!(value_f64, 100.0);

        // Verify: timestamp is non-zero
        let ts_bytes = rtdb.hash_get("inst:1:A", "ts:10").await.unwrap().unwrap();
        let ts_str = String::from_utf8(ts_bytes.to_vec()).unwrap();
        let ts: i64 = ts_str.parse().unwrap();
        assert!(ts > 0, "Timestamp should be current time");

        // Verify: TODO queue has trigger message
        let todo_key = "comsrv:1001:A:TODO";
        let todo_msgs = rtdb.list_range(todo_key, 0, -1).await.unwrap();
        assert_eq!(
            todo_msgs.len(),
            1,
            "Should have 1 trigger message in TODO queue"
        );

        // Verify: TODO message format (JSON with point_id, value, timestamp)
        let msg = String::from_utf8(todo_msgs[0].to_vec()).unwrap();
        assert!(
            msg.contains(r#""point_id":10"#),
            "Message should contain point_id"
        );
        assert!(
            msg.contains(r#""value":100"#),
            "Message should contain value"
        );
        assert!(
            msg.contains(r#""timestamp":"#),
            "Message should contain timestamp"
        );
    }

    #[tokio::test]
    async fn test_write_point_runtime_channel_action_direct_trigger() {
        // Channel A/C points don't need routing lookup, directly write to TODO queue
        let rtdb = MemoryRtdb::with_routing(Arc::new(voltage_config::RoutingCache::new()));

        // Test Channel Adjustment point
        rtdb.write_point_runtime("comsrv:1001:A", 5, 12.3)
            .await
            .unwrap();

        // Verify: value is written
        let value = rtdb.hash_get("comsrv:1001:A", "5").await.unwrap();
        assert_eq!(value, Some(Bytes::from("12.3")));

        // Verify: TODO queue is written
        let todo_msgs = rtdb.list_range("comsrv:1001:A:TODO", 0, -1).await.unwrap();
        assert_eq!(todo_msgs.len(), 1, "Channel A should trigger TODO queue");

        // Test Channel Control point
        rtdb.write_point_runtime("comsrv:1002:C", 3, 1.0)
            .await
            .unwrap();

        // Verify: TODO queue is written
        let todo_msgs = rtdb.list_range("comsrv:1002:C:TODO", 0, -1).await.unwrap();
        assert_eq!(todo_msgs.len(), 1, "Channel C should trigger TODO queue");
    }

    #[tokio::test]
    async fn test_write_point_runtime_no_trigger_cases() {
        let rtdb = MemoryRtdb::with_routing(Arc::new(voltage_config::RoutingCache::new()));

        // Case 1: Instance Measurement point (M doesn't trigger)
        rtdb.write_point_runtime("inst:1:M", 10, 230.5)
            .await
            .unwrap();

        // Case 2: Channel Telemetry point (T doesn't trigger)
        rtdb.write_point_runtime("comsrv:1001:T", 5, 50.0)
            .await
            .unwrap();

        // Case 3: Channel Signal point (S doesn't trigger)
        rtdb.write_point_runtime("comsrv:1001:S", 3, 1.0)
            .await
            .unwrap();

        // Case 4: Other key formats
        rtdb.write_point_runtime("other:key:format", 1, 42.0)
            .await
            .unwrap();

        // Verify: all values are written (check first one as representative)
        let value = rtdb.hash_get("inst:1:M", "10").await.unwrap();
        assert!(value.is_some(), "Value should be written for M points");

        // Verify: no TODO queues created
        let scan_result = rtdb.scan_match("*:TODO").await.unwrap();
        assert_eq!(
            scan_result.len(),
            0,
            "No TODO queues should be created for M/T/S points"
        );
    }

    #[tokio::test]
    async fn test_routing_boundary_cases() {
        use std::collections::HashMap;

        // Case 1: Routing doesn't exist
        let mut m2c_data = HashMap::new();
        m2c_data.insert("1:A:10".to_string(), "1001:A:5".to_string());
        let cache = Arc::new(voltage_config::RoutingCache::from_maps(
            HashMap::new(),
            m2c_data,
            HashMap::new(), // C2C routing (not used in this test)
        ));
        let rtdb = MemoryRtdb::with_routing(cache);

        // Write Instance A point without configured routing
        rtdb.write_point_runtime("inst:999:A", 10, 100.0)
            .await
            .unwrap();

        // Verify: value is written but no TODO triggered
        let value = rtdb.hash_get("inst:999:A", "10").await.unwrap();
        assert!(
            value.is_some(),
            "Value should be written even without routing"
        );
        let scan_result = rtdb.scan_match("*:TODO").await.unwrap();
        assert_eq!(scan_result.len(), 0, "No TODO queue when routing not found");

        // Case 2: No routing_cache (using new())
        let rtdb_no_cache = MemoryRtdb::new();
        rtdb_no_cache
            .write_point_runtime("inst:1:A", 10, 100.0)
            .await
            .unwrap();

        // Verify: value is written but no TODO triggered
        let value = rtdb_no_cache.hash_get("inst:1:A", "10").await.unwrap();
        assert!(value.is_some(), "Value should be written without cache");
        let scan_result = rtdb_no_cache.scan_match("*:TODO").await.unwrap();
        assert_eq!(scan_result.len(), 0, "No TODO queue without cache");

        // Case 3: M2C routing target format incomplete
        let mut m2c_invalid = HashMap::new();
        m2c_invalid.insert("2:A:10".to_string(), "1001".to_string()); // Missing ":A" part
        let cache_invalid = Arc::new(voltage_config::RoutingCache::from_maps(
            HashMap::new(),
            m2c_invalid,
            HashMap::new(), // C2C routing (not used in this test)
        ));
        let rtdb_invalid = MemoryRtdb::with_routing(cache_invalid);

        rtdb_invalid
            .write_point_runtime("inst:2:A", 10, 100.0)
            .await
            .unwrap();

        // Verify: value is written but no TODO triggered (invalid format ignored)
        let value = rtdb_invalid.hash_get("inst:2:A", "10").await.unwrap();
        assert!(value.is_some(), "Value should be written");
        let scan_result = rtdb_invalid.scan_match("*:TODO").await.unwrap();
        assert_eq!(
            scan_result.len(),
            0,
            "No TODO queue when route format is invalid"
        );
    }

    #[tokio::test]
    async fn test_action_to_control_routing() {
        use std::collections::HashMap;

        // Build M2C routing: Instance A point → Channel C point (Action maps to Control)
        let mut m2c_data = HashMap::new();
        m2c_data.insert("1:A:10".to_string(), "1001:C:5".to_string());
        let cache = Arc::new(voltage_config::RoutingCache::from_maps(
            HashMap::new(), // C2M routing
            m2c_data,
            HashMap::new(), // C2C routing (not used in this test)
        ));

        let rtdb = MemoryRtdb::with_routing(cache);

        // Write Instance A point
        rtdb.write_point_runtime("inst:1:A", 10, 100.0)
            .await
            .unwrap();

        // Verify: TODO queue written to Control type
        let todo_key = "comsrv:1001:C:TODO";
        let todo_msgs = rtdb.list_range(todo_key, 0, -1).await.unwrap();
        assert_eq!(todo_msgs.len(), 1, "Should trigger Control TODO queue");

        // Verify: Adjustment TODO queue doesn't exist
        let adj_todo = rtdb.list_range("comsrv:1001:A:TODO", 0, -1).await.unwrap();
        assert_eq!(
            adj_todo.len(),
            0,
            "Should not trigger Adjustment TODO queue"
        );
    }
}
