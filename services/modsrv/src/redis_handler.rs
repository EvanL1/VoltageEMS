use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use redis::{Client, Connection, Commands, RedisResult, Value};
use std::collections::HashMap;
use log::{info, error, debug};

/// Redis connection handler
pub struct RedisConnection {
    client: Option<Client>,
    connection: Option<Connection>,
}

#[derive(Debug, PartialEq)]
pub enum RedisType {
    String,
    List,
    Set,
    Hash,
    ZSet,
    None,
}

impl RedisConnection {
    pub fn new() -> Self {
        Self {
            client: None,
            connection: None,
        }
    }

    pub fn from_config(config: &crate::config::RedisConfig) -> Result<Self> {
        let mut conn = Self::new();
        let redis_url = format!("redis://{}:{}/{}", config.host, config.port, config.database);
        conn.connect(&redis_url)?;
        Ok(conn)
    }

    pub fn connect(&mut self, redis_url: &str) -> Result<()> {
        let client = Client::open(redis_url)
            .map_err(|e| ModelSrvError::RedisError(e.to_string()))?;
            
        let connection = client.get_connection()
            .map_err(|e| ModelSrvError::RedisError(e.to_string()))?;
            
        self.client = Some(client);
        self.connection = Some(connection);
        
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connection = None;
        self.client = None;
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    pub fn get_keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        if let Some(conn) = &mut self.connection {
            conn.keys(pattern).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    pub fn get_type(&mut self, key: &str) -> Result<RedisType> {
        if let Some(conn) = &mut self.connection {
            let type_str: String = redis::cmd("TYPE")
                .arg(key)
                .query(conn)
                .map_err(|e| ModelSrvError::RedisError(e.to_string()))?;

            match type_str.as_str() {
                "string" => Ok(RedisType::String),
                "list" => Ok(RedisType::List),
                "set" => Ok(RedisType::Set),
                "hash" => Ok(RedisType::Hash),
                "zset" => Ok(RedisType::ZSet),
                "none" => Ok(RedisType::None),
                _ => Ok(RedisType::None),
            }
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    pub fn get_string(&mut self, key: &str) -> Result<String> {
        if let Some(conn) = &mut self.connection {
            conn.get(key).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    pub fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        if let Some(conn) = &mut self.connection {
            let value: HashMap<String, String> = conn.hgetall(key).map_err(|e| ModelSrvError::RedisError(e.to_string()))?;
            Ok(value)
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    pub fn set_hash_field(&mut self, key: &str, field: &str, value: &str) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            conn.hset(key, field, value).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    pub fn set_hash(&mut self, key: &str, hash: &HashMap<String, String>) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            let fields_and_values: Vec<(&str, &str)> = hash
                .iter()
                .map(|(field, value)| (field.as_str(), value.as_str()))
                .collect();
            
            if !fields_and_values.is_empty() {
                conn.hset_multiple::<_, _, _, ()>(key, &fields_and_values)
                    .map_err(|e| ModelSrvError::RedisError(e.to_string()))?;
            }
            
            Ok(())
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Push a value to the right end (tail) of the list
    pub fn push_list(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            conn.rpush(key, value).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Push a value to the left end (head) of the list
    pub fn push_list_front(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            conn.lpush(key, value).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Pop a value from the right end (tail) of the list
    pub fn pop_list(&mut self, key: &str) -> Result<Option<String>> {
        if let Some(conn) = &mut self.connection {
            conn.rpop::<_, Option<String>>(key, None)
                .map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Pop a value from the left end (head) of the list
    pub fn pop_list_front(&mut self, key: &str) -> Result<Option<String>> {
        if let Some(conn) = &mut self.connection {
            conn.lpop::<_, Option<String>>(key, None)
                .map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Get the length of the list
    pub fn list_len(&mut self, key: &str) -> Result<usize> {
        if let Some(conn) = &mut self.connection {
            conn.llen(key).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Get a range of values from the list
    pub fn list_range(&mut self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        if let Some(conn) = &mut self.connection {
            conn.lrange(key, start, stop).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Block and pop a value from a list with timeout
    pub fn blpop(&mut self, key: &str, timeout_seconds: usize) -> Result<Option<(String, String)>> {
        if let Some(conn) = &mut self.connection {
            conn.blpop(key, timeout_seconds).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Block and pop a value from multiple lists with timeout
    pub fn blpop_multiple(&mut self, keys: &[&str], timeout_seconds: usize) -> Result<Option<(String, String)>> {
        if let Some(conn) = &mut self.connection {
            conn.blpop(keys, timeout_seconds).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    pub fn set_string(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            conn.set(key, value).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    pub fn exists(&mut self, key: &str) -> Result<bool> {
        if let Some(conn) = &mut self.connection {
            conn.exists(key).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    pub fn delete(&mut self, key: &str) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            conn.del(key).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Publish a message to the specified channel
    pub fn publish(&mut self, channel: &str, message: &str) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            let _: () = redis::cmd("PUBLISH")
                .arg(channel)
                .arg(message)
                .query(conn)
                .map_err(|e| ModelSrvError::RedisError(e.to_string()))?;
            Ok(())
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// Create a new Redis connection with the same configuration
    pub fn duplicate(&self) -> Result<Self> {
        let mut conn = Self::new();
        
        if let Some(client) = &self.client {
            // Try to create a new connection from the original client
            let connection = client.get_connection()
                .map_err(|e| ModelSrvError::RedisError(format!("Failed to duplicate connection: {}", e)))?;
            
            conn.client = Some(client.clone());
            conn.connection = Some(connection);
            return Ok(conn);
        }
        
        // If no client is available, try to get connection info from environment variables
        if let Ok(redis_url) = std::env::var("REDIS_URL") {
            conn.connect(&redis_url)
                .map_err(|e| ModelSrvError::RedisError(format!("Failed to duplicate connection: {}", e)))?;
            return Ok(conn);
        }
        
        // If all methods fail, return an error
        Err(ModelSrvError::RedisError("No connection information available to duplicate".to_string()))
    }
} 