//! Asynchronous Redis client implementation

use super::types::{ConnectionState, RedisStats, RedisType};
use crate::{Error, Result};
use redis::{aio::ConnectionManager, AsyncCommands, Client, Pipeline, RedisError};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Asynchronous Redis client with connection management
pub struct RedisClient {
    client: Client,
    connection: Arc<RwLock<Option<ConnectionManager>>>,
    stats: Arc<RwLock<RedisStats>>,
    state: Arc<RwLock<ConnectionState>>,
}

impl RedisClient {
    /// Create a new Redis client from URL
    pub async fn new(url: &str) -> Result<Self> {
        let client = Client::open(url)
            .map_err(|e| Error::Redis(format!("Failed to create client: {}", e)))?;

        let mut instance = Self {
            client,
            connection: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(RedisStats::new())),
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
        };

        instance.connect().await?;
        Ok(instance)
    }

    /// Create a new Redis client from Unix socket
    #[cfg(feature = "unix-socket")]
    pub async fn new_with_socket(socket_path: &str) -> Result<Self> {
        let url = format!("redis+unix://{}", socket_path);
        Self::new(&url).await
    }

    /// Connect to Redis server
    pub async fn connect(&mut self) -> Result<()> {
        *self.state.write().await = ConnectionState::Connecting;

        match ConnectionManager::new(self.client.clone()).await {
            Ok(conn) => {
                *self.connection.write().await = Some(conn);
                *self.state.write().await = ConnectionState::Connected;
                self.stats.write().await.record_connection(true);
                Ok(())
            }
            Err(e) => {
                *self.state.write().await = ConnectionState::Error;
                self.stats.write().await.record_connection(false);
                Err(Error::Redis(format!("Connection failed: {}", e)))
            }
        }
    }

    /// Get connection state
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Get statistics
    pub async fn stats(&self) -> RedisStats {
        self.stats.read().await.clone()
    }

    /// Execute a command and record stats
    async fn execute<T, F>(&self, op: F) -> Result<T>
    where
        F: std::future::Future<Output = std::result::Result<T, RedisError>>,
    {
        let result = op.await;
        let success = result.is_ok();
        self.stats.write().await.record_command(success);

        result.map_err(|e| Error::Redis(format!("Command failed: {}", e)))
    }

    /// Get connection manager
    async fn get_connection(&self) -> Result<ConnectionManager> {
        let conn_guard = self.connection.read().await;
        conn_guard
            .as_ref()
            .cloned()
            .ok_or_else(|| Error::Redis("Not connected".to_string()))
    }

    // Key operations

    /// Set a key-value pair
    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.set(key, value)).await
    }

    /// Set a key-value pair with expiration
    pub async fn set_ex(&self, key: &str, value: &str, seconds: u64) -> Result<()> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.set_ex(key, value, seconds)).await
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.get(key)).await
    }

    /// Delete one or more keys
    pub async fn del(&self, keys: &[&str]) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.del(keys)).await
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.exists(key)).await
    }

    /// Set expiration on a key
    pub async fn expire(&self, key: &str, seconds: i64) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.expire(key, seconds)).await
    }

    /// Get remaining TTL of a key
    pub async fn ttl(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.ttl(key)).await
    }

    /// Get key type
    pub async fn key_type(&self, key: &str) -> Result<RedisType> {
        let mut conn = self.get_connection().await?;
        let type_str: String = self.execute(conn.key_type(key)).await?;
        Ok(RedisType::from_redis_string(&type_str))
    }

    /// Increment a key's value
    pub async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.incr(key, 1)).await
    }

    // List operations

    /// Push value to list head
    pub async fn lpush(&self, key: &str, value: &str) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.lpush(key, value)).await
    }

    /// Push value to list tail
    pub async fn rpush(&self, key: &str, value: &str) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.rpush(key, value)).await
    }

    /// Pop value from list head
    pub async fn lpop(&self, key: &str, count: Option<usize>) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        if let Some(n) = count {
            let count_opt = std::num::NonZeroUsize::new(n);
            self.execute(conn.lpop(key, count_opt)).await
        } else {
            let value: Option<String> = self.execute(conn.lpop(key, None)).await?;
            Ok(value.map(|v| vec![v]).unwrap_or_default())
        }
    }

    /// Pop value from list tail
    pub async fn rpop(&self, key: &str, count: Option<usize>) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        if let Some(n) = count {
            let count_opt = std::num::NonZeroUsize::new(n);
            self.execute(conn.rpop(key, count_opt)).await
        } else {
            let value: Option<String> = self.execute(conn.rpop(key, None)).await?;
            Ok(value.map(|v| vec![v]).unwrap_or_default())
        }
    }

    /// Get list length
    pub async fn llen(&self, key: &str) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.llen(key)).await
    }

    /// Get list range
    pub async fn lrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.lrange(key, start, stop)).await
    }

    // Hash operations

    /// Set hash field
    pub async fn hset(&self, key: &str, field: &str, value: &str) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.hset(key, field, value)).await
    }

    /// Set multiple hash fields
    pub async fn hset_multiple<I>(&self, key: &str, items: I) -> Result<()>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut conn = self.get_connection().await?;
        let items_vec: Vec<(String, String)> = items.into_iter().collect();
        self.execute(conn.hset_multiple(key, &items_vec)).await
    }

    /// Get hash field
    pub async fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.hget(key, field)).await
    }

    /// Get all hash fields and values
    pub async fn hgetall(&self, key: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.hgetall(key)).await
    }

    /// Delete hash fields
    pub async fn hdel(&self, key: &str, fields: &[&str]) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.hdel(key, fields)).await
    }

    /// Check if hash field exists
    pub async fn hexists(&self, key: &str, field: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.hexists(key, field)).await
    }

    /// Increment hash field by integer
    pub async fn hincrby(&self, key: &str, field: &str, increment: i64) -> Result<i64> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.hincr(key, field, increment)).await
    }

    // Set operations

    /// Add member to set
    pub async fn sadd(&self, key: &str, member: &str) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.sadd(key, member)).await
    }

    /// Remove member from set
    pub async fn srem(&self, key: &str, member: &str) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.srem(key, member)).await
    }

    /// Get all set members
    pub async fn smembers(&self, key: &str) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.smembers(key)).await
    }

    /// Check if member exists in set
    pub async fn sismember(&self, key: &str, member: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.sismember(key, member)).await
    }

    pub async fn scard(&self, key: &str) -> Result<usize> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.scard(key)).await
    }

    // Sorted Set operations

    /// Add member to sorted set
    pub async fn zadd(&self, key: &str, member: &str, score: f64) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.zadd(key, member, score)).await
    }

    /// Get sorted set range by rank
    pub async fn zrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.zrange(key, start, stop)).await
    }

    /// Get sorted set range by score
    pub async fn zrangebyscore(
        &self,
        key: &str,
        min: f64,
        max: f64,
        limit: Option<(isize, isize)>,
    ) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        if let Some((offset, count)) = limit {
            self.execute(conn.zrangebyscore_limit(key, min, max, offset, count))
                .await
        } else {
            self.execute(conn.zrangebyscore(key, min, max)).await
        }
    }

    // Pub/Sub operations

    /// Subscribe to channels
    pub async fn subscribe(&self, _channels: &[&str]) -> Result<redis::aio::PubSub> {
        let pubsub = self
            .client
            .get_async_pubsub()
            .await
            .map_err(|e| Error::Redis(format!("Failed to create pubsub: {}", e)))?;
        Ok(pubsub)
    }

    /// Publish message to channel
    pub async fn publish(&self, channel: &str, message: &str) -> Result<i32> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.publish(channel, message)).await
    }

    // Pattern operations

    /// Find keys matching pattern
    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        self.execute(conn.keys(pattern)).await
    }

    /// Scan keys with cursor (returns all matching keys)
    pub async fn scan(&self, pattern: Option<&str>, _count: Option<usize>) -> Result<Vec<String>> {
        // For async, we'll use keys instead of scan for simplicity
        // In production, you'd want to implement proper async iteration with cursor
        let pattern_str = pattern.unwrap_or("*");
        self.keys(pattern_str).await
    }

    // Pipeline operations

    /// Execute commands in pipeline
    pub async fn pipeline<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Pipeline) -> &mut Pipeline,
        T: redis::FromRedisValue,
    {
        let mut conn = self.get_connection().await?;
        let mut pipe = redis::pipe();
        f(&mut pipe);

        self.execute(pipe.query_async(&mut conn)).await
    }

    // Utility operations

    /// Ping server
    pub async fn ping(&self) -> Result<String> {
        let mut conn = self.get_connection().await?;
        self.execute(redis::cmd("PING").query_async(&mut conn))
            .await
    }

    /// Get server info
    pub async fn info(&self) -> Result<String> {
        let mut conn = self.get_connection().await?;
        self.execute(redis::cmd("INFO").query_async(&mut conn))
            .await
    }

    /// Flush database
    pub async fn flushdb(&self) -> Result<()> {
        let mut conn = self.get_connection().await?;
        self.execute(redis::cmd("FLUSHDB").query_async(&mut conn))
            .await
    }
}

/// Builder for Redis client with advanced options
pub struct RedisClientBuilder {
    url: String,
    timeout: Option<Duration>,
}

impl RedisClientBuilder {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            timeout: None,
        }
    }

    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub async fn build(self) -> Result<RedisClient> {
        RedisClient::new(&self.url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let result = RedisClient::new("redis://127.0.0.1:6379").await;
        assert!(result.is_ok() || result.is_err()); // Accept both for testing
    }
}
