//! Redis implementation of RTDB traits

use crate::traits::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use common::redis::RedisClient;
use std::collections::HashMap;
use std::sync::Arc;

/// Redis-backed RTDB implementation
///
/// This is a pure storage abstraction. For routing logic, use the
/// `voltage-routing` library which handles M2C routing externally.
pub struct RedisRtdb {
    client: Arc<RedisClient>,
}

impl RedisRtdb {
    /// Create new Redis RTDB from URL
    pub async fn new(url: &str) -> Result<Self> {
        Ok(Self {
            client: Arc::new(RedisClient::new(url).await?),
        })
    }

    /// Create from existing RedisClient
    pub fn from_client(client: Arc<RedisClient>) -> Self {
        Self { client }
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

    async fn hash_del_many(&self, key: &str, fields: &[String]) -> Result<usize> {
        self.client
            .hdel_many(key, fields)
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

    async fn pipeline_hash_mset(
        &self,
        operations: Vec<(String, Vec<(String, Bytes)>)>,
    ) -> Result<()> {
        if operations.is_empty() {
            return Ok(());
        }

        // Convert Bytes to String for the client
        let string_operations: Result<Vec<(String, Vec<(String, String)>)>> = operations
            .into_iter()
            .map(|(key, fields)| {
                let string_fields: Result<Vec<(String, String)>> = fields
                    .into_iter()
                    .map(|(f, v)| {
                        let s = String::from_utf8(v.to_vec()).context("UTF-8 conversion failed")?;
                        Ok((f, s))
                    })
                    .collect();
                Ok((key, string_fields?))
            })
            .collect();

        self.client
            .pipeline_hmset(&string_operations?)
            .await
            .map_err(|e| anyhow::anyhow!(e))
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
    async fn test_redis_rtdb_todo_queues() {
        let rtdb = RedisRtdb::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Enqueue into per-channel queues
        rtdb.enqueue_control(1001, r#"{"cmd":"c1"}"#).await.unwrap();
        rtdb.enqueue_adjustment(1001, r#"{"cmd":"a1"}"#)
            .await
            .unwrap();

        // Pop using list operations
        let c_key = "comsrv:1001:C:TODO";
        let a_key = "comsrv:1001:A:TODO";

        let action1 = rtdb.list_lpop(c_key).await.unwrap();
        assert!(action1.is_some());
        let s1 = String::from_utf8(action1.unwrap().to_vec()).unwrap();
        assert!(s1.contains("c1"));

        let action2 = rtdb.list_lpop(a_key).await.unwrap();
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
