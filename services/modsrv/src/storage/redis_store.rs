use std::sync::{Arc, Mutex};
use crate::error::{Result, ModelSrvError};
use crate::redis_handler::RedisConnection;
use crate::redis_handler::RedisType;
use crate::config::Config;
use super::DataStore;
use std::collections::HashMap;

/// Redis存储实现
pub struct RedisStore {
    connection: Arc<Mutex<RedisConnection>>,
    key_prefix: String,
}

impl RedisStore {
    /// 创建新的Redis存储
    pub fn new(config: &Config) -> Result<Self> {
        let mut connection = RedisConnection::new();
        let redis_url = format!("redis://{}:{}", config.redis.host, config.redis.port);
        connection.connect(&redis_url)?;
        
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            key_prefix: config.redis.key_prefix.clone(),
        })
    }
}

impl DataStore for RedisStore {
    fn get_string(&self, key: &str) -> Result<String> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.get_string(key)
    }
    
    fn set_string(&self, key: &str, value: &str) -> Result<()> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.set_string(key, value)
    }
    
    fn get_hash(&self, key: &str) -> Result<HashMap<String, String>> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.get_hash(key)
    }
    
    fn set_hash(&self, key: &str, hash: &HashMap<String, String>) -> Result<()> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.set_hash(key, hash)
    }
    
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.set_hash_field(key, field, value)
    }
    
    fn get_type(&self, key: &str) -> Result<RedisType> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.get_type(key)
    }
    
    fn get_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.get_keys(pattern)
    }
    
    fn exists(&self, key: &str) -> Result<bool> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.exists(key)
    }
    
    fn delete(&self, key: &str) -> Result<()> {
        let mut connection = self.connection.lock().map_err(|_| ModelSrvError::LockError)?;
        connection.delete(key)
    }
} 