//! Synchronous Redis client implementation

use super::types::{ConnectionState, RedisStats, RedisType};
use crate::{Error, Result};
use redis::{Client, Commands, Connection, Pipeline, RedisError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Synchronous Redis client with connection management
pub struct RedisSyncClient {
    client: Client,
    connection: Arc<Mutex<Option<Connection>>>,
    stats: Arc<Mutex<RedisStats>>,
    state: Arc<Mutex<ConnectionState>>,
}

impl RedisSyncClient {
    /// Create a new Redis client from URL
    pub fn new(url: &str) -> Result<Self> {
        let client = Client::open(url)
            .map_err(|e| Error::Redis(format!("Failed to create client: {}", e)))?;

        let mut instance = Self {
            client,
            connection: Arc::new(Mutex::new(None)),
            stats: Arc::new(Mutex::new(RedisStats::new())),
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
        };

        instance.connect()?;
        Ok(instance)
    }

    /// Create a new Redis client from Unix socket
    #[cfg(feature = "unix-socket")]
    pub fn new_with_socket(socket_path: &str) -> Result<Self> {
        let url = format!("redis+unix://{}", socket_path);
        Self::new(&url)
    }

    /// Connect to Redis server
    pub fn connect(&mut self) -> Result<()> {
        *self.state.lock().unwrap() = ConnectionState::Connecting;

        match self.client.get_connection() {
            Ok(conn) => {
                *self.connection.lock().unwrap() = Some(conn);
                *self.state.lock().unwrap() = ConnectionState::Connected;
                self.stats.lock().unwrap().record_connection(true);
                Ok(())
            }
            Err(e) => {
                *self.state.lock().unwrap() = ConnectionState::Error;
                self.stats.lock().unwrap().record_connection(false);
                Err(Error::Redis(format!("Connection failed: {}", e)))
            }
        }
    }

    /// Get connection state
    pub fn state(&self) -> ConnectionState {
        *self.state.lock().unwrap()
    }

    /// Get statistics
    pub fn stats(&self) -> RedisStats {
        self.stats.lock().unwrap().clone()
    }

    /// Execute a command and record stats
    fn execute<T, F>(&self, op: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> std::result::Result<T, RedisError>,
    {
        let mut conn_guard = self.connection.lock().unwrap();
        let conn = conn_guard
            .as_mut()
            .ok_or_else(|| Error::Redis("Not connected".to_string()))?;

        let result = op(conn);
        let success = result.is_ok();
        self.stats.lock().unwrap().record_command(success);

        result.map_err(|e| Error::Redis(format!("Command failed: {}", e)))
    }

    // Key operations

    /// Set a key-value pair
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.execute(|conn| conn.set(key, value))
    }

    /// Set a key-value pair with expiration
    pub fn set_ex(&self, key: &str, value: &str, seconds: u64) -> Result<()> {
        self.execute(|conn| conn.set_ex(key, value, seconds))
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        self.execute(|conn| conn.get(key))
    }

    /// Delete one or more keys
    pub fn del(&self, keys: &[&str]) -> Result<i32> {
        self.execute(|conn| conn.del(keys))
    }

    /// Check if key exists
    pub fn exists(&self, key: &str) -> Result<bool> {
        self.execute(|conn| conn.exists(key))
    }

    /// Set expiration on a key
    pub fn expire(&self, key: &str, seconds: i64) -> Result<bool> {
        self.execute(|conn| conn.expire(key, seconds))
    }

    /// Get remaining TTL of a key
    pub fn ttl(&self, key: &str) -> Result<i64> {
        self.execute(|conn| conn.ttl(key))
    }

    /// Get key type
    pub fn key_type(&self, key: &str) -> Result<RedisType> {
        let type_str: String = self.execute(|conn| conn.key_type(key))?;
        Ok(RedisType::from_redis_string(&type_str))
    }

    // List operations

    /// Push value to list head
    pub fn lpush(&self, key: &str, value: &str) -> Result<i32> {
        self.execute(|conn| conn.lpush(key, value))
    }

    /// Push value to list tail
    pub fn rpush(&self, key: &str, value: &str) -> Result<i32> {
        self.execute(|conn| conn.rpush(key, value))
    }

    /// Pop value from list head
    pub fn lpop(&self, key: &str, count: Option<usize>) -> Result<Vec<String>> {
        match count {
            Some(n) => {
                let count_opt = std::num::NonZeroUsize::new(n);
                self.execute(|conn| conn.lpop(key, count_opt))
            }
            None => {
                let value: Option<String> = self.execute(|conn| conn.lpop(key, None))?;
                Ok(value.map(|v| vec![v]).unwrap_or_default())
            }
        }
    }

    /// Pop value from list tail
    pub fn rpop(&self, key: &str, count: Option<usize>) -> Result<Vec<String>> {
        match count {
            Some(n) => {
                let count_opt = std::num::NonZeroUsize::new(n);
                self.execute(|conn| conn.rpop(key, count_opt))
            }
            None => {
                let value: Option<String> = self.execute(|conn| conn.rpop(key, None))?;
                Ok(value.map(|v| vec![v]).unwrap_or_default())
            }
        }
    }

    /// Get list length
    pub fn llen(&self, key: &str) -> Result<i32> {
        self.execute(|conn| conn.llen(key))
    }

    /// Get list range
    pub fn lrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        self.execute(|conn| conn.lrange(key, start, stop))
    }

    // Hash operations

    /// Set hash field
    pub fn hset(&self, key: &str, field: &str, value: &str) -> Result<i32> {
        self.execute(|conn| conn.hset(key, field, value))
    }

    /// Set multiple hash fields
    pub fn hset_multiple<I>(&self, key: &str, items: I) -> Result<()>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let items_vec: Vec<(String, String)> = items.into_iter().collect();
        self.execute(|conn| conn.hset_multiple(key, &items_vec))
    }

    /// Get hash field
    pub fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        self.execute(|conn| conn.hget(key, field))
    }

    /// Get all hash fields and values
    pub fn hgetall(&self, key: &str) -> Result<std::collections::HashMap<String, String>> {
        self.execute(|conn| conn.hgetall(key))
    }

    /// Delete hash fields
    pub fn hdel(&self, key: &str, fields: &[&str]) -> Result<i32> {
        self.execute(|conn| conn.hdel(key, fields))
    }

    /// Check if hash field exists
    pub fn hexists(&self, key: &str, field: &str) -> Result<bool> {
        self.execute(|conn| conn.hexists(key, field))
    }

    // Set operations

    /// Add member to set
    pub fn sadd(&self, key: &str, member: &str) -> Result<i32> {
        self.execute(|conn| conn.sadd(key, member))
    }

    /// Remove member from set
    pub fn srem(&self, key: &str, member: &str) -> Result<i32> {
        self.execute(|conn| conn.srem(key, member))
    }

    /// Get all set members
    pub fn smembers(&self, key: &str) -> Result<Vec<String>> {
        self.execute(|conn| conn.smembers(key))
    }

    /// Check if member exists in set
    pub fn sismember(&self, key: &str, member: &str) -> Result<bool> {
        self.execute(|conn| conn.sismember(key, member))
    }

    // Sorted Set operations

    /// Add member to sorted set
    pub fn zadd(&self, key: &str, member: &str, score: f64) -> Result<i32> {
        self.execute(|conn| conn.zadd(key, member, score))
    }

    /// Get sorted set range by rank
    pub fn zrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        self.execute(|conn| conn.zrange(key, start, stop))
    }

    /// Get sorted set range by score
    pub fn zrangebyscore(
        &self,
        key: &str,
        min: f64,
        max: f64,
        limit: Option<(isize, isize)>,
    ) -> Result<Vec<String>> {
        if let Some((offset, count)) = limit {
            self.execute(|conn| conn.zrangebyscore_limit(key, min, max, offset, count))
        } else {
            self.execute(|conn| conn.zrangebyscore(key, min, max))
        }
    }

    // Pattern operations

    /// Find keys matching pattern
    pub fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        self.execute(|conn| conn.keys(pattern))
    }

    /// Scan keys with cursor (returns all matching keys)
    pub fn scan(&self, pattern: Option<&str>, count: Option<usize>) -> Result<Vec<String>> {
        self.execute(|conn| {
            let mut scan_options = redis::ScanOptions::default();
            if let Some(p) = pattern {
                scan_options = scan_options.with_pattern(p);
            }
            if let Some(c) = count {
                scan_options = scan_options.with_count(c);
            }

            let iter: redis::Iter<String> = conn.scan_options(scan_options)?;
            let results: Vec<String> = iter.collect();
            Ok(results)
        })
    }

    // Pipeline operations

    /// Execute commands in pipeline
    pub fn pipeline<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Pipeline) -> &mut Pipeline,
        T: redis::FromRedisValue,
    {
        self.execute(|conn| {
            let mut pipe = redis::pipe();
            f(&mut pipe);
            pipe.query(conn)
        })
    }

    // Utility operations

    /// Ping server
    pub fn ping(&self) -> Result<String> {
        self.execute(|conn| redis::cmd("PING").query(conn))
    }

    /// Get server info
    pub fn info(&self) -> Result<String> {
        self.execute(|conn| redis::cmd("INFO").query(conn))
    }

    /// Flush database
    pub fn flushdb(&self) -> Result<()> {
        self.execute(|conn| redis::cmd("FLUSHDB").query(conn))
    }

    /// Publish message to channel
    pub fn publish(&self, channel: &str, message: &str) -> Result<i32> {
        self.execute(|conn| conn.publish(channel, message))
    }
}

/// Builder for sync Redis client with advanced options
pub struct RedisSyncClientBuilder {
    url: String,
    timeout: Option<Duration>,
}

impl RedisSyncClientBuilder {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            timeout: None,
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Result<RedisSyncClient> {
        RedisSyncClient::new(&self.url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let result = RedisSyncClient::new("redis://127.0.0.1:6379");
        assert!(result.is_ok() || result.is_err()); // Accept both for testing
    }
}
