//! Redis implementation of RTDB traits

use crate::traits::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use common::redis::RedisClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use voltage_config::RoutingCache;

/// Redis-backed RTDB implementation
pub struct RedisRtdb {
    client: Arc<RedisClient>,
    routing_cache: Arc<RwLock<Option<Arc<RoutingCache>>>>,
}

impl RedisRtdb {
    /// Create new Redis RTDB from URL (without routing)
    pub async fn new(url: &str) -> Result<Self> {
        Ok(Self {
            client: Arc::new(RedisClient::new(url).await?),
            routing_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Create from existing RedisClient (without routing)
    pub fn from_client(client: Arc<RedisClient>) -> Self {
        Self {
            client,
            routing_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Create Redis RTDB with routing cache support
    ///
    /// This enables automatic routing trigger in `write_point_runtime()` method.
    pub fn with_routing(client: Arc<RedisClient>, routing_cache: Arc<RoutingCache>) -> Self {
        Self {
            client,
            routing_cache: Arc::new(RwLock::new(Some(routing_cache))),
        }
    }

    /// Set routing cache (can be called after construction)
    ///
    /// Useful for injecting routing cache after RTDB creation.
    pub async fn set_routing_cache(&mut self, routing_cache: Arc<RoutingCache>) {
        *self.routing_cache.write().await = Some(routing_cache);
    }

    /// Update routing cache (hot reload support)
    ///
    /// This method supports hot reload by atomically updating the routing cache.
    /// Can be called on Arc-wrapped instances without requiring &mut self.
    ///
    /// # Example
    /// ```ignore
    /// let rtdb = Arc::new(RedisRtdb::from_client(client));
    /// // ... later for hot reload
    /// rtdb.update_routing_cache(new_routing_cache).await;
    /// ```
    pub async fn update_routing_cache(&self, routing_cache: Arc<RoutingCache>) {
        *self.routing_cache.write().await = Some(routing_cache);
    }

    /// Get reference to underlying Redis client
    ///
    /// This is useful for calling Redis commands directly
    /// that are not part of the Rtdb trait.
    pub fn client(&self) -> &Arc<RedisClient> {
        &self.client
    }
}

#[async_trait]
impl Rtdb for RedisRtdb {
    async fn get(&self, key: &str) -> Result<Option<Bytes>> {
        let value: Option<String> = self.client.get(key).await.map_err(|e| anyhow::anyhow!(e))?;
        Ok(value.map(Bytes::from))
    }

    async fn set(&self, key: &str, value: Bytes) -> Result<()> {
        let s = String::from_utf8(value.to_vec())?;
        self.client
            .set(key, s)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn del(&self, key: &str) -> Result<bool> {
        let count = self
            .client
            .del(&[key])
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(count > 0)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        self.client
            .exists(key)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn incrbyfloat(&self, key: &str, increment: f64) -> Result<f64> {
        self.client
            .incrbyfloat(key, increment)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn hash_set(&self, key: &str, field: &str, value: Bytes) -> Result<()> {
        let s = String::from_utf8(value.to_vec()).context("UTF-8 conversion failed")?;
        self.client
            .hset(key, field, s)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn hash_get(&self, key: &str, field: &str) -> Result<Option<Bytes>> {
        let value: Option<String> = self
            .client
            .hget(key, field)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(value.map(Bytes::from))
    }

    async fn hash_mget(&self, key: &str, fields: &[&str]) -> Result<Vec<Option<Bytes>>> {
        let values: Vec<Option<String>> = self
            .client
            .hmget(key, fields)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(values.into_iter().map(|v| v.map(Bytes::from)).collect())
    }

    async fn hash_mset(&self, key: &str, fields: Vec<(String, Bytes)>) -> Result<()> {
        let string_fields: Result<Vec<(String, String)>> = fields
            .into_iter()
            .map(|(k, v)| {
                let s = String::from_utf8(v.to_vec()).context("UTF-8 conversion failed")?;
                Ok((k, s))
            })
            .collect();
        self.client
            .hmset(key, &string_fields?)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn hash_get_all(&self, key: &str) -> Result<HashMap<String, Bytes>> {
        let data: HashMap<String, String> = self
            .client
            .hgetall(key)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(data.into_iter().map(|(k, v)| (k, Bytes::from(v))).collect())
    }

    async fn hash_del(&self, key: &str, field: &str) -> Result<bool> {
        self.client
            .hdel(key, field)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn list_lpush(&self, key: &str, value: Bytes) -> Result<()> {
        let s = String::from_utf8(value.to_vec()).context("UTF-8 conversion failed")?;
        self.client
            .lpush(key, &s)
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn list_rpush(&self, key: &str, value: Bytes) -> Result<()> {
        let s = String::from_utf8(value.to_vec()).context("UTF-8 conversion failed")?;
        self.client
            .rpush(key, &s)
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn list_lpop(&self, key: &str) -> Result<Option<Bytes>> {
        let value: Option<String> = self
            .client
            .lpop(key)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(value.map(Bytes::from))
    }

    async fn list_rpop(&self, key: &str) -> Result<Option<Bytes>> {
        let value: Option<String> = self
            .client
            .rpop(key)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(value.map(Bytes::from))
    }

    async fn list_blpop(
        &self,
        keys: &[&str],
        timeout_seconds: u64,
    ) -> Result<Option<(String, Bytes)>> {
        let result: Option<(String, String)> = self
            .client
            .blpop(keys, timeout_seconds as usize)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(result.map(|(k, v)| (k, Bytes::from(v))))
    }

    async fn list_range(&self, key: &str, start: isize, stop: isize) -> Result<Vec<Bytes>> {
        let values: Vec<String> = self
            .client
            .lrange(key, start, stop)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(values.into_iter().map(Bytes::from).collect())
    }

    async fn list_trim(&self, key: &str, start: isize, stop: isize) -> Result<()> {
        self.client
            .ltrim(key, start, stop)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn publish(&self, channel: &str, message: &str) -> Result<u32> {
        self.client
            .publish(channel, message)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn fcall(&self, function: &str, keys: &[&str], args: &[&str]) -> Result<String> {
        self.client
            .fcall(function, keys, args)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn scan_match(&self, pattern: &str) -> Result<Vec<String>> {
        self.client
            .scan_match(pattern)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn sadd(&self, key: &str, member: &str) -> Result<bool> {
        self.client
            .sadd(key, member)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn srem(&self, key: &str, member: &str) -> Result<bool> {
        self.client
            .srem(key, member)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn smembers(&self, key: &str) -> Result<Vec<String>> {
        self.client
            .smembers(key)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn hincrby(&self, key: &str, field: &str, increment: i64) -> Result<i64> {
        self.client
            .hincrby(key, field, increment)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn time_millis(&self) -> Result<i64> {
        self.client
            .time_millis()
            .await
            .map_err(|e| anyhow::anyhow!(e))
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
        let routing_guard = self.routing_cache.read().await;
        if let Some(ref routing_cache) = *routing_guard {
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

impl RedisRtdb {
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
    #[ignore = "Requires Redis connection"]
    async fn test_redis_rtdb_basic_operations() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Test KV operations
        rtdb.set("test:key", Bytes::from("value"))
            .await
            .expect("Failed to set");
        let value = rtdb.get("test:key").await.expect("Failed to get");
        assert_eq!(value, Some(Bytes::from("value")));

        // Test exists
        assert!(rtdb.exists("test:key").await.unwrap());

        // Test delete
        assert!(rtdb.del("test:key").await.unwrap());
        assert!(!rtdb.exists("test:key").await.unwrap());
    }

    #[tokio::test]
    #[ignore = "Requires Redis connection"]
    async fn test_redis_rtdb_hash_operations() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Test hash operations
        rtdb.hash_set("test:hash", "field1", Bytes::from("value1"))
            .await
            .unwrap();
        rtdb.hash_set("test:hash", "field2", Bytes::from("value2"))
            .await
            .unwrap();

        let value = rtdb.hash_get("test:hash", "field1").await.unwrap();
        assert_eq!(value, Some(Bytes::from("value1")));

        let all = rtdb.hash_get_all("test:hash").await.unwrap();
        assert_eq!(all.len(), 2);

        // Cleanup
        rtdb.del("test:hash").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires Redis connection"]
    async fn test_redis_rtdb_list_operations() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Test list operations
        rtdb.list_lpush("test:list", Bytes::from("value1"))
            .await
            .unwrap();
        rtdb.list_rpush("test:list", Bytes::from("value2"))
            .await
            .unwrap();

        let range = rtdb.list_range("test:list", 0, -1).await.unwrap();
        assert_eq!(range.len(), 2);

        let value = rtdb.list_lpop("test:list").await.unwrap();
        assert_eq!(value, Some(Bytes::from("value1")));

        // Cleanup
        rtdb.del("test:list").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires Redis connection"]
    async fn test_redis_rtdb_point_operations() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Test point operations
        rtdb.write_point("test_inst1", "M", 1, 100.5).await.unwrap();
        rtdb.write_point("test_inst1", "M", 2, 200.3).await.unwrap();

        let value = rtdb.read_point("test_inst1", "M", 1).await.unwrap();
        assert_eq!(value, Some(100.5));

        // Test batch write
        rtdb.write_points_batch("test_inst2", "A", vec![(1, 50.0), (2, 75.5)])
            .await
            .unwrap();

        let points = rtdb.get_instance_points("test_inst2", "A").await.unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points.get(&1), Some(&50.0));

        // Cleanup
        rtdb.del("modsrv:test_inst1:M").await.unwrap();
        rtdb.del("modsrv:test_inst2:A").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires Redis connection"]
    async fn test_redis_rtdb_mapping_operations() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Test mapping operations
        rtdb.write_mapping("channel:1:T:1", "modsrv:inst1:M:1")
            .await
            .unwrap();

        let mapping = rtdb.read_mapping("channel:1:T:1").await.unwrap();
        assert_eq!(mapping, Some("modsrv:inst1:M:1".to_string()));

        // Cleanup
        rtdb.hash_del("route:c2m", "channel:1:T:1").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires Redis connection"]
    async fn test_redis_rtdb_todo_queues() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Enqueue into per-channel queues
        rtdb.enqueue_control(1001, r#"{"cmd":"c1"}"#).await.unwrap();
        rtdb.enqueue_adjustment(1001, r#"{"cmd":"a1"}"#)
            .await
            .unwrap();

        // Pop using list operations and RedisKeys helper
        use voltage_config::comsrv::RedisKeys;
        let c_key = RedisKeys::control_todo(1001);
        let a_key = RedisKeys::adjustment_todo(1001);

        let action1 = rtdb.list_lpop(&c_key).await.unwrap();
        assert!(action1.is_some());
        let s1 = String::from_utf8(action1.unwrap().to_vec()).unwrap();
        assert!(s1.contains("c1"));

        let action2 = rtdb.list_lpop(&a_key).await.unwrap();
        assert!(action2.is_some());
        let s2 = String::from_utf8(action2.unwrap().to_vec()).unwrap();
        assert!(s2.contains("a1"));
    }

    #[tokio::test]
    #[ignore = "Requires Redis connection"]
    async fn test_redis_rtdb_hash_mset() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Test hash_mset
        rtdb.hash_mset(
            "test:hash_batch",
            vec![
                ("f1".to_string(), Bytes::from("v1")),
                ("f2".to_string(), Bytes::from("v2")),
                ("f3".to_string(), Bytes::from("v3")),
            ],
        )
        .await
        .unwrap();

        let all = rtdb.hash_get_all("test:hash_batch").await.unwrap();
        assert_eq!(all.len(), 3);

        // Cleanup
        rtdb.del("test:hash_batch").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires Redis connection"]
    async fn test_redis_rtdb_list_trim() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Populate list
        for i in 0..10 {
            rtdb.list_rpush("test:trim_list", Bytes::from(format!("value{}", i)))
                .await
                .unwrap();
        }

        // Trim to keep only first 5
        rtdb.list_trim("test:trim_list", 0, 4).await.unwrap();

        let range = rtdb.list_range("test:trim_list", 0, -1).await.unwrap();
        assert_eq!(range.len(), 5);

        // Cleanup
        rtdb.del("test:trim_list").await.unwrap();
    }
}
