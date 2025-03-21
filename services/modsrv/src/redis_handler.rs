use crate::error::{ModelSrvError, Result};
use redis::{Client, Connection, Commands as RedisCommands};
use serde_json::Value;
use std::collections::HashMap;

/// Redis connection handler
pub struct RedisConnection {
    /// Redis connection
    conn: Connection,
}

impl RedisConnection {
    /// Create a new Redis connection with default settings
    pub fn new() -> Self {
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let client = Client::open(redis_url.as_str()).expect("Redis client creation failed");
        let conn = client.get_connection().expect("Redis connection failed");
        Self { conn }
    }

    /// Create a new Redis connection with custom configuration
    pub fn with_config(host: &str, port: u16, password: Option<String>) -> Result<Self> {
        let url = match password {
            Some(pw) => format!("redis://:{}@{}:{}", pw, host, port),
            None => format!("redis://{}:{}", host, port),
        };
        
        let client = Client::open(url.as_str())
            .map_err(|e| ModelSrvError::RedisError(format!("Redis client creation failed: {}", e)))?;
        
        let conn = client.get_connection()
            .map_err(|e| ModelSrvError::RedisError(format!("Redis connection failed: {}", e)))?;
        
        Ok(Self { conn })
    }
    
    /// Create a connection from configuration
    pub fn from_config(config: &Value) -> Result<Self> {
        let host = config["host"].as_str().unwrap_or("127.0.0.1");
        let port = config["port"].as_u64().unwrap_or(6379) as u16;
        let password = config["password"].as_str().map(|s| s.to_string());
        
        Self::with_config(host, port, password)
    }
    
    /// Clone the connection to create a duplicate
    pub fn duplicate(&self) -> Result<Self> {
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let client = Client::open(redis_url.as_str())
            .map_err(|e| ModelSrvError::RedisError(format!("Redis client creation failed: {}", e)))?;
        
        let conn = client.get_connection()
            .map_err(|e| ModelSrvError::RedisError(format!("Redis connection failed: {}", e)))?;
        
        Ok(Self { conn })
    }

    /// Get keys matching a pattern
    pub fn get_keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        self.conn.keys(pattern)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get keys: {}", e)))
    }

    /// Get a string value from Redis
    pub fn get_string(&mut self, key: &str) -> Result<String> {
        self.conn.get(key)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get string for key {}: {}", key, e)))
    }

    /// Get a hash value from Redis
    pub fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        self.conn.hgetall(key)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get hash for key {}: {}", key, e)))
    }

    /// Set a field in a hash
    pub fn set_hash_field(&mut self, key: &str, field: &str, value: &str) -> Result<()> {
        self.conn.hset::<_, _, _, ()>(key, field, value)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to set hash field for key {}: {}", key, e)))?;
        Ok(())
    }

    /// Set an entire hash
    pub fn set_hash(&mut self, key: &str, map: HashMap<String, String>) -> Result<()> {
        let items: Vec<(&str, &str)> = map.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        
        self.conn.hset_multiple::<_, _, _, ()>(key, &items)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to set hash for key {}: {}", key, e)))?;
        Ok(())
    }

    /// Append a value to a list
    pub fn rpush(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.rpush::<_, _, ()>(key, value)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to push to list {}: {}", key, e)))?;
        Ok(())
    }

    /// Get all values from a list
    pub fn get_list(&mut self, key: &str) -> Result<Vec<String>> {
        self.conn.lrange(key, 0, -1)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get list for key {}: {}", key, e)))
    }

    /// Set a string value in Redis
    pub fn set_string(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.set::<_, _, ()>(key, value)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to set string for key {}: {}", key, e)))?;
        Ok(())
    }

    /// Check if a key exists
    pub fn exists(&mut self, key: &str) -> Result<bool> {
        self.conn.exists(key)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to check if key {} exists: {}", key, e)))
    }

    /// Delete a key
    pub fn delete(&mut self, key: &str) -> Result<()> {
        self.conn.del::<_, ()>(key)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to delete key {}: {}", key, e)))?;
        Ok(())
    }
    
    /// Publish a message to a channel
    pub fn publish(&mut self, channel: &str, message: &str) -> Result<()> {
        self.conn.publish::<_, _, ()>(channel, message)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to publish to channel {}: {}", channel, e)))?;
        Ok(())
    }
    
    /// Execute a custom command
    pub fn execute_command(&mut self, cmd: &str, args: Vec<&str>) -> Result<String> {
        let mut command = redis::cmd(cmd);
        for arg in args {
            command.arg(arg);
        }
        
        command.query(&mut self.conn)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to execute command {}: {}", cmd, e)))
    }
    
    /// Get mutable connection for direct use
    pub fn get_mut_connection(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Connect to Redis with URL
    pub fn connect(&mut self, url: &str) -> Result<()> {
        let client = Client::open(url)
            .map_err(|e| ModelSrvError::RedisError(format!("Redis client creation failed: {}", e)))?;
        
        self.conn = client.get_connection()
            .map_err(|e| ModelSrvError::RedisError(format!("Redis connection failed: {}", e)))?;
        
        Ok(())
    }

    /// Get the type of a key
    pub fn get_type(&mut self, key: &str) -> Result<RedisType> {
        let key_type: String = redis::cmd("TYPE")
            .arg(key)
            .query(&mut self.conn)
            .map_err(|e| ModelSrvError::RedisError(format!("Failed to get type for key {}: {}", key, e)))?;
        
        match key_type.as_str() {
            "string" => Ok(RedisType::String),
            "list" => Ok(RedisType::List),
            "set" => Ok(RedisType::Set),
            "zset" => Ok(RedisType::ZSet),
            "hash" => Ok(RedisType::Hash),
            "none" => Ok(RedisType::None),
            _ => Ok(RedisType::Unknown)
        }
    }
    
    /// Get raw connection for direct access
    pub fn get_raw_connection(&self) -> Result<redis::Connection> {
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let client = Client::open(redis_url.as_str())
            .map_err(|e| ModelSrvError::RedisError(format!("Redis client creation failed: {}", e)))?;
        
        client.get_connection()
            .map_err(|e| ModelSrvError::RedisError(format!("Redis connection failed: {}", e)))
    }
}

impl Clone for RedisConnection {
    fn clone(&self) -> Self {
        match self.duplicate() {
            Ok(conn) => conn,
            Err(_) => Self::new() // fallback to new connection if duplicate fails
        }
    }
}

impl redis::ConnectionLike for RedisConnection {
    fn req_packed_command(&mut self, cmd: &[u8]) -> redis::RedisResult<redis::Value> {
        self.conn.req_packed_command(cmd)
    }

    fn req_packed_commands(
        &mut self,
        cmd: &[u8],
        offset: usize,
        count: usize,
    ) -> redis::RedisResult<Vec<redis::Value>> {
        self.conn.req_packed_commands(cmd, offset, count)
    }

    fn get_db(&self) -> i64 {
        self.conn.get_db()
    }

    fn check_connection(&mut self) -> bool {
        self.conn.check_connection()
    }

    fn is_open(&self) -> bool {
        self.conn.is_open()
    }
}

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
    Unknown
} 