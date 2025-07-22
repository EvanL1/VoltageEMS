use crate::error::{ModelSrvError, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use voltage_common::redis::{RedisConfig, RedisSyncClient};

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

    /// Set a string value in Redis
    pub fn set_string(&mut self, key: &str, value: &str) -> Result<()> {
        self.client.set(key, value).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to set string for key {}: {}", key, e))
        })
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

    /// Set multiple fields in a hash
    pub fn set_hash_fields(&mut self, key: &str, fields: &HashMap<String, String>) -> Result<()> {
        for (field, value) in fields {
            self.set_hash_field(key, field, value)?;
        }
        Ok(())
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
        self.client
            .publish(channel, message)
            .map_err(|e| {
                ModelSrvError::RedisError(format!(
                    "Failed to publish to channel {}: {}",
                    channel, e
                ))
            })
            .map(|_| ())
    }

    /// Set a hash in Redis
    pub fn set_hash(&mut self, key: &str, fields: HashMap<String, String>) -> Result<()> {
        for (field, value) in fields {
            self.set_hash_field(key, &field, &value)?;
        }
        Ok(())
    }

    /// Push to a list
    pub fn rpush(&mut self, key: &str, value: &str) -> Result<()> {
        self.client.rpush(key, value).map_err(|e| {
            ModelSrvError::RedisError(format!("Failed to rpush to key {}: {}", key, e))
        })?;
        Ok(())
    }

    /// Get a list
    pub fn get_list(&mut self, key: &str) -> Result<Vec<String>> {
        self.client
            .lrange(key, 0, -1)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get list {}: {}", key, e)))
    }

    /// Get a value from Redis and parse as JSON
    pub fn get_json_value(&mut self, key: &str) -> Result<Option<Value>> {
        let str_value = self.get_string(key)?;
        match serde_json::from_str(&str_value) {
            Ok(json_value) => Ok(Some(json_value)),
            Err(e) => Err(ModelSrvError::FormatError(format!(
                "Failed to parse JSON for key {}: {}",
                key, e
            ))),
        }
    }

    /// Set a JSON value in Redis
    pub fn set_json_value(&mut self, key: &str, value: &Value) -> Result<()> {
        let json_str = serde_json::to_string(value).map_err(|e| {
            ModelSrvError::SerializationError(format!(
                "Failed to serialize JSON for key {}: {}",
                key, e
            ))
        })?;
        self.client
            .set(key, &json_str)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to set key {}: {}", key, e)))
    }

    /// Get all keys matching a pattern and their values as JSON
    pub fn get_all_json(&mut self, pattern: &str) -> Result<HashMap<String, Value>> {
        let keys = self.get_keys(pattern)?;
        let mut results = HashMap::new();
        for key in keys {
            if let Ok(Some(value)) = self.get_json_value(&key) {
                results.insert(key, value);
            }
        }
        Ok(results)
    }

    /// Check if a key exists
    pub fn exists(&mut self, key: &str) -> Result<bool> {
        self.client
            .exists(key)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to check key existence: {}", e)))
    }

    /// Set expiration on a key (TTL in seconds)
    pub fn expire(&mut self, key: &str, seconds: u64) -> Result<()> {
        self.client
            .expire(key, seconds as i64)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to set expiration: {}", e)))
            .map(|_| ())
    }

    /// Save a model configuration
    pub fn save_model_config(&mut self, key: &str, config: &Value) -> Result<()> {
        let config_str = serde_json::to_string_pretty(config).map_err(|e| {
            ModelSrvError::SerializationError(format!("Failed to serialize config: {}", e))
        })?;

        self.client
            .set(key, &config_str)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to save config: {}", e)))?;

        Ok(())
    }

    /// Load a model configuration
    pub fn load_model_config(&mut self, key: &str) -> Result<Value> {
        let config_str = self.get_string(key)?;
        serde_json::from_str(&config_str)
            .map_err(|e| ModelSrvError::FormatError(format!("Failed to parse config: {}", e)))
    }

    /// Get a connection that can be cloned
    pub fn get_clonable_connection(&self) -> Result<RedisConnection> {
        self.duplicate()
    }
}

/// Redis handler for async operations
pub struct RedisHandler {
    connection: Arc<RwLock<RedisConnection>>,
}

impl RedisHandler {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(RedisConnection::new())),
        }
    }

    pub fn from_connection(connection: RedisConnection) -> Self {
        Self {
            connection: Arc::new(RwLock::new(connection)),
        }
    }

    /// Get a value from Redis
    pub async fn get<T: std::str::FromStr>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.connection.write().await;
        match conn.get_string(key) {
            Ok(value) => match value.parse::<T>() {
                Ok(parsed) => Ok(Some(parsed)),
                Err(_) => Err(ModelSrvError::FormatError(format!(
                    "Failed to parse value for key {}",
                    key
                ))),
            },
            Err(ModelSrvError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Set a value in Redis
    pub async fn set(&self, key: &str, value: String) -> Result<()> {
        let conn = self.connection.write().await;
        conn.client
            .set(key, &value)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to set key {}: {}", key, e)))
    }

    /// Publish a message to a channel
    pub async fn publish(&self, channel: &str, message: String) -> Result<()> {
        let mut conn = self.connection.write().await;
        conn.publish(channel, &message)
    }

    /// Get async pubsub (placeholder - needs proper async implementation)
    pub async fn get_async_pubsub(&self) -> Result<AsyncPubSub> {
        Ok(AsyncPubSub::new())
    }
}

/// Placeholder for async PubSub
pub struct AsyncPubSub {
    _phantom: std::marker::PhantomData<()>,
}

impl AsyncPubSub {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    pub async fn subscribe(&mut self, _channel: &str) -> Result<()> {
        // TODO: Implement async subscribe
        Ok(())
    }

    pub async fn unsubscribe(&mut self, _channel: &str) -> Result<()> {
        // TODO: Implement async unsubscribe
        Ok(())
    }
}

/// PubSub message stream placeholder
pub struct PubSubStream {
    _phantom: std::marker::PhantomData<()>,
}

impl futures::Stream for PubSubStream {
    type Item = PubSubMessage;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Pending
    }
}

impl AsyncPubSub {
    pub fn on_message(&mut self) -> PubSubStream {
        PubSubStream {
            _phantom: std::marker::PhantomData,
        }
    }
}

/// PubSub message
pub struct PubSubMessage {
    pub channel: String,
    pub payload: String,
}

impl PubSubMessage {
    pub fn get_payload<T: std::str::FromStr>(&self) -> Result<T> {
        self.payload
            .parse::<T>()
            .map_err(|_| ModelSrvError::FormatError("Failed to parse payload".to_string()))
    }
}

// Default implementation for RedisConnection
impl Default for RedisConnection {
    fn default() -> Self {
        Self::new()
    }
}

// Clone implementation for RedisConnection
impl Clone for RedisConnection {
    fn clone(&self) -> Self {
        Self::new()
    }
}
