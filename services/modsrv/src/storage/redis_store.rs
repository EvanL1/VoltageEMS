use crate::error::{ModelSrvError, Result};
use crate::redis_handler::{RedisConnection, RedisType};
use std::collections::HashMap;
use std::sync::Arc;

/// Redis store implementation
pub struct RedisStore {
    /// Redis connection instance
    redis: RedisConnection,
}

impl RedisStore {
    /// Create a new Redis store
    pub fn new(redis: RedisConnection) -> Self {
        Self { redis }
    }
    
    /// Create a new Redis store from Arc<RedisConnection>
    pub fn from_arc(redis: Arc<RedisConnection>) -> Self {
        // We need to create a new connection using the same configuration
        // because we can't get mutable access to the Arc<RedisConnection>
        let mut new_conn = RedisConnection::new();
        // For simplicity, we'll use a default connection string
        // In a real implementation, we would extract the connection details from the Arc
        if let Err(e) = new_conn.connect("redis://localhost:6379") {
            // Log error but continue with a disconnected instance
            eprintln!("Failed to connect to Redis: {}", e);
        }
        Self { redis: new_conn }
    }
    
    pub fn get_connection(&self) -> Result<redis::Connection> {
        self.redis.get_raw_connection()
    }

    /// Get the type of a key in Redis
    pub fn get_type(&self, key: &str) -> Result<RedisType> {
        let mut redis = self.redis.duplicate()?;
        redis.get_type(key).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get type for key {}: {}", key, e))
        })
    }
}

impl super::DataStore for RedisStore {
    /// Get a string value from Redis
    fn get_string(&self, key: &str) -> Result<String> {
        let mut redis = self.redis.duplicate()?;
        redis.get_string(key).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get string for key {}: {}", key, e))
        })
    }
    
    /// Set a string value in Redis
    fn set_string(&self, key: &str, value: &str) -> Result<()> {
        let mut redis = self.redis.duplicate()?;
        redis.set_string(key, value).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set string for key {}: {}", key, e))
        })
    }
    
    /// Get a hash value from Redis
    fn get_hash(&self, key: &str) -> Result<HashMap<String, String>> {
        let mut redis = self.redis.duplicate()?;
        redis.get_hash(key).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get hash for key {}: {}", key, e))
        })
    }

    /// Set a hash field in Redis
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()> {
        let mut redis = self.redis.duplicate()?;
        redis.set_hash_field(key, field, value).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set hash field for key {}: {}", key, e))
        })
    }

    /// Set a hash table in Redis
    fn set_hash(&self, key: &str, hash: &HashMap<String, String>) -> Result<()> {
        let mut redis = self.redis.duplicate()?;
        redis.set_hash(key, hash).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set hash for key {}: {}", key, e))
        })
    }

    /// Get keys matching a pattern from Redis
    fn get_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut redis = self.redis.duplicate()?;
        redis.get_keys(pattern).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get keys for pattern {}: {}", pattern, e))
        })
    }

    /// Check if a key exists in Redis
    fn exists(&self, key: &str) -> Result<bool> {
        let mut redis = self.redis.duplicate()?;
        redis.exists(key).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to check if key {} exists: {}", key, e))
        })
    }

    /// Delete a key from Redis
    fn delete(&self, key: &str) -> Result<()> {
        let mut redis = self.redis.duplicate()?;
        redis.delete(key).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to delete key {}: {}", key, e))
        })
    }
} 