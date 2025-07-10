use crate::error::{ModelSrvError, Result};
use chrono;
use serde_json::Value;
use std::collections::HashMap;
use voltage_common::redis::{RedisConfig, RedisSyncClient, RedisType as CommonRedisType};

/// Redis connection handler using voltage-common
pub struct RedisConnection {
    /// Redis sync client from voltage-common
    client: RedisSyncClient,
}

impl RedisConnection {
    /// Create a new Redis connection with default settings
    pub fn new() -> Self {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let client = RedisSyncClient::new(&redis_url).expect("Redis client creation failed");
        Self { client }
    }

    /// Create a new Redis connection with custom configuration
    pub fn with_config(host: &str, port: u16, password: Option<String>) -> Result<Self> {
        let config = RedisConfig {
            host: host.to_string(),
            port,
            password,
            socket: None,
            database: 0,
            connection_timeout: 10,
            max_retries: 3,
        };

        let url = config.to_url();
        let client = RedisSyncClient::new(&url)
            .map_err(|e| ModelSrvError::RedisError(format!("Redis connection failed: {}", e)))?;

        Ok(Self { client })
    }

    /// Create a connection from configuration
    pub fn from_config(config: &Value) -> Result<Self> {
        let redis_config = RedisConfig {
            host: config["host"].as_str().unwrap_or("127.0.0.1").to_string(),
            port: config["port"].as_u64().unwrap_or(6379) as u16,
            password: config["password"].as_str().map(|s| s.to_string()),
            socket: config["socket"].as_str().map(|s| s.to_string()),
            database: config["database"].as_u64().unwrap_or(0) as u8,
            connection_timeout: config["connection_timeout"].as_u64().unwrap_or(10),
            max_retries: config["max_retries"].as_u64().unwrap_or(3) as u32,
        };

        let url = redis_config.to_url();
        let client = RedisSyncClient::new(&url)
            .map_err(|e| ModelSrvError::RedisError(format!("Redis connection failed: {}", e)))?;

        Ok(Self { client })
    }

    /// Clone the connection to create a duplicate
    pub fn duplicate(&self) -> Result<Self> {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let client = RedisSyncClient::new(&redis_url)
            .map_err(|e| ModelSrvError::RedisError(format!("Redis connection failed: {}", e)))?;

        Ok(Self { client })
    }

    /// Get keys matching a pattern
    pub fn get_keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        self.client
            .keys(pattern)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get keys: {}", e)))
    }

    /// Get a string value from Redis
    pub fn get_string(&mut self, key: &str) -> Result<String> {
        match self.client.get(key) {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(ModelSrvError::NotFound(format!("Key not found: {}", key))),
            Err(e) => Err(ModelSrvError::RedisError(format!(
                "Failed to get string for key {}: {}",
                key, e
            ))),
        }
    }

    /// Get a hash value from Redis
    pub fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        self.client.hgetall(key).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get hash for key {}: {}", key, e))
        })
    }

    /// Set a field in a hash
    pub fn set_hash_field(&mut self, key: &str, field: &str, value: &str) -> Result<()> {
        self.client.hset(key, field, value).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set hash field for key {}: {}", key, e))
        })?;
        Ok(())
    }

    /// Set an entire hash
    pub fn set_hash(&mut self, key: &str, map: HashMap<String, String>) -> Result<()> {
        self.client
            .hset_multiple(key, map.into_iter())
            .map_err(|e| {
                ModelSrvError::RedisError(format!("Failed to set hash for key {}: {}", key, e))
            })?;
        Ok(())
    }

    /// Append a value to a list
    pub fn rpush(&mut self, key: &str, value: &str) -> Result<()> {
        self.client.rpush(key, value).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to push to list {}: {}", key, e))
        })?;
        Ok(())
    }

    /// Get all values from a list
    pub fn get_list(&mut self, key: &str) -> Result<Vec<String>> {
        self.client.lrange(key, 0, -1).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get list for key {}: {}", key, e))
        })
    }

    /// Set a string value in Redis
    pub fn set_string(&mut self, key: &str, value: &str) -> Result<()> {
        self.client.set(key, value).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set string for key {}: {}", key, e))
        })?;
        Ok(())
    }

    /// Check if a key exists
    pub fn exists(&mut self, key: &str) -> Result<bool> {
        self.client.exists(key).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to check if key {} exists: {}", key, e))
        })
    }

    /// Delete a key
    pub fn delete(&mut self, key: &str) -> Result<()> {
        self.client.del(&[key]).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to delete key {}: {}", key, e))
        })?;
        Ok(())
    }

    /// Publish a message to a channel
    pub fn publish(&mut self, channel: &str, message: &str) -> Result<()> {
        self.client.publish(channel, message).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to publish to channel {}: {}", channel, e))
        })?;
        Ok(())
    }

    /// Execute a custom command
    pub fn execute_command(&mut self, cmd: &str, args: Vec<&str>) -> Result<String> {
        // Use ping as a placeholder for custom commands
        // In future, we can extend voltage-common to support custom commands
        if cmd == "PING" {
            return self.client.ping().map_err(|e| {
                ModelSrvError::RedisError(format!("Failed to execute command {}: {}", cmd, e))
            });
        }
        Err(ModelSrvError::RedisError(
            "Custom commands not yet supported in voltage-common".to_string(),
        ))
    }

    /// Update a single point value using Hash structure for optimized query
    pub fn update_point_value(
        &mut self,
        module_id: &str,
        point_id: &str,
        value: &serde_json::Value,
    ) -> Result<()> {
        let hash_key = format!("modsrv:realtime:module:{}", module_id);
        let field = point_id;

        let value_data = serde_json::json!({
            "value": value,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "quality": "good"
        });

        self.set_hash_field(&hash_key, field, &value_data.to_string())?;

        // Also publish to channel for subscribers
        let channel = format!("modsrv:updates:module:{}", module_id);
        let update_msg = serde_json::json!({
            "module_id": module_id,
            "point_id": point_id,
            "value": value,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        self.publish(&channel, &update_msg.to_string())?;

        Ok(())
    }

    /// Batch update multiple point values using Hash structure
    pub fn batch_update_points(
        &mut self,
        module_id: &str,
        points: Vec<(String, serde_json::Value)>,
    ) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        let hash_key = format!("modsrv:realtime:module:{}", module_id);
        let timestamp = chrono::Utc::now().to_rfc3339();

        // Prepare fields for hash update
        let mut fields = HashMap::new();
        for (point_id, value) in &points {
            let value_data = serde_json::json!({
                "value": value,
                "timestamp": &timestamp,
                "quality": "good"
            });
            fields.insert(point_id.clone(), value_data.to_string());
        }

        // Update all fields in one operation
        self.set_hash(&hash_key, fields)?;

        // Publish batch update notification
        let channel = format!("modsrv:updates:module:{}", module_id);
        let update_msg = serde_json::json!({
            "module_id": module_id,
            "points": points.into_iter().map(|(id, val)| {
                serde_json::json!({
                    "point_id": id,
                    "value": val
                })
            }).collect::<Vec<_>>(),
            "timestamp": timestamp
        });
        self.publish(&channel, &update_msg.to_string())?;

        Ok(())
    }

    /// Get all realtime values for a module
    pub fn get_module_realtime_values(
        &mut self,
        module_id: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let hash_key = format!("modsrv:realtime:module:{}", module_id);
        let raw_values = self.get_hash(&hash_key)?;

        let mut result = HashMap::new();
        for (point_id, value_str) in raw_values {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&value_str) {
                result.insert(point_id, value);
            }
        }

        Ok(result)
    }

    /// Get a single point value for a module
    pub fn get_point_value(
        &mut self,
        module_id: &str,
        point_id: &str,
    ) -> Result<serde_json::Value> {
        let hash_key = format!("modsrv:realtime:module:{}", module_id);

        // Use hget from voltage-common (we need to extend the trait if not available)
        let all_values = self.get_hash(&hash_key)?;

        if let Some(value_str) = all_values.get(point_id) {
            serde_json::from_str(value_str)
                .map_err(|e| ModelSrvError::RedisError(format!("Failed to parse value: {}", e)))
        } else {
            Err(ModelSrvError::NotFound(format!(
                "Point {} not found in module {}",
                point_id, module_id
            )))
        }
    }

    // Note: Direct connection access is not available with voltage-common client
    // Use the provided methods instead

    /// Connect to Redis with URL
    pub fn connect(&mut self, url: &str) -> Result<()> {
        self.client = RedisSyncClient::new(url)
            .map_err(|e| ModelSrvError::RedisError(format!("Redis connection failed: {}", e)))?;
        Ok(())
    }

    /// Get the type of a key
    pub fn get_type(&mut self, key: &str) -> Result<RedisType> {
        let key_type = self.client.key_type(key).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to get type for key {}: {}", key, e))
        })?;

        Ok(RedisType::from(key_type))
    }

    // Note: Raw connection access is not available with voltage-common client
    // Use the provided methods instead
}

impl Clone for RedisConnection {
    fn clone(&self) -> Self {
        match self.duplicate() {
            Ok(conn) => conn,
            Err(_) => Self::new(), // fallback to new connection if duplicate fails
        }
    }
}

// Note: Deref and DerefMut traits are not implemented as voltage-common
// does not expose the underlying connection. Use the provided methods instead.

/// Redis key types
#[derive(Debug, Clone, PartialEq)]
pub enum RedisType {
    /// String value
    String,
    /// List value
    List,
    /// Set value
    Set,
    /// Sorted set value
    ZSet,
    /// Hash value
    Hash,
    /// Key does not exist
    None,
    /// Unknown type
    Unknown,
}

impl From<CommonRedisType> for RedisType {
    fn from(common_type: CommonRedisType) -> Self {
        match common_type {
            CommonRedisType::String => RedisType::String,
            CommonRedisType::List => RedisType::List,
            CommonRedisType::Set => RedisType::Set,
            CommonRedisType::ZSet => RedisType::ZSet,
            CommonRedisType::Hash => RedisType::Hash,
            CommonRedisType::None => RedisType::None,
            CommonRedisType::Stream => RedisType::Unknown,
        }
    }
}
