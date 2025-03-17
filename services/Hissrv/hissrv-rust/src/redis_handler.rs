use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use redis::{Client, Commands, Connection, RedisResult, PipelineCommands};
use tokio::sync::mpsc;
use tokio::time;

use crate::config::{Config, RedisConfig};
use crate::error::{HissrvError, Result};

/// Redis data types
#[derive(Debug, Clone)]
pub enum RedisData {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// Hash map
    Hash(HashMap<String, String>),
    /// No data
    None,
}

/// Redis data point
#[derive(Debug, Clone)]
pub struct RedisDataPoint {
    /// Redis key
    pub key: String,
    /// Redis data
    pub data: RedisData,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Redis client trait
pub trait RedisClientTrait {
    /// Connect to Redis
    fn connect(&mut self) -> Result<()>;
    /// Disconnect from Redis
    fn disconnect(&mut self) -> Result<()>;
    /// Check if connected to Redis
    fn is_connected(&self) -> bool;
    /// Get keys matching a pattern
    fn get_keys(&mut self, pattern: &str) -> Result<Vec<String>>;
    /// Get a string value
    fn get_string(&mut self, key: &str) -> Result<Option<String>>;
    /// Get a hash map
    fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>>;
    /// Start polling Redis
    fn start_polling(&mut self, interval: u64) -> Result<mpsc::Receiver<RedisDataPoint>>;
}

/// Redis handler
pub struct RedisHandler {
    /// Redis configuration
    config: RedisConfig,
    /// Redis client
    client: Option<Client>,
    /// Redis connection
    connection: Arc<Mutex<Option<Connection>>>,
    /// Is connected to Redis
    is_connected: bool,
}

impl RedisHandler {
    /// Create a new Redis handler
    pub fn new(config: RedisConfig) -> Self {
        Self {
            config,
            client: None,
            connection: Arc::new(Mutex::new(None)),
            is_connected: false,
        }
    }
    
    /// Create a new Redis handler from config
    pub fn from_config(config: &Config) -> Self {
        Self::new(config.redis.clone())
    }
    
    /// Parse Redis data
    fn parse_data(&self, data: &str) -> RedisData {
        // Try to parse as integer
        if let Ok(value) = data.parse::<i64>() {
            return RedisData::Integer(value);
        }
        
        // Try to parse as float
        if let Ok(value) = data.parse::<f64>() {
            return RedisData::Float(value);
        }
        
        // Try to parse as boolean
        match data.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => return RedisData::Boolean(true),
            "false" | "0" | "no" | "off" => return RedisData::Boolean(false),
            _ => {}
        }
        
        // Default to string
        RedisData::String(data.to_string())
    }
}

impl RedisClientTrait for RedisHandler {
    fn connect(&mut self) -> Result<()> {
        let connection_string = format!(
            "redis://{}:{}/{}",
            self.config.host, self.config.port, self.config.db
        );
        
        let client = Client::open(connection_string)
            .map_err(|e| HissrvError::RedisError(e.to_string()))?;
        
        let mut connection = client
            .get_connection_with_timeout(Duration::from_secs(self.config.timeout_seconds as u64))
            .map_err(|e| HissrvError::RedisError(e.to_string()))?;
        
        // Authenticate if password is provided
        if let Some(password) = &self.config.password {
            let _: redis::RedisResult<String> = redis::cmd("AUTH")
                .arg(password)
                .query(&mut connection);
        }
        
        // Select database
        let _: redis::RedisResult<()> = redis::cmd("SELECT")
            .arg(self.config.db)
            .query(&mut connection);
        
        self.client = Some(client);
        *self.connection.lock().unwrap() = Some(connection);
        self.is_connected = true;
        
        Ok(())
    }
    
    fn disconnect(&mut self) -> Result<()> {
        // Redis connections are automatically closed when dropped
        *self.connection.lock().unwrap() = None;
        self.client = None;
        self.is_connected = false;
        
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        self.is_connected
    }
    
    fn get_keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        let mut connection_guard = self.connection.lock().unwrap();
        
        if let Some(connection) = connection_guard.as_mut() {
            let keys: Vec<String> = redis::cmd("KEYS")
                .arg(pattern)
                .query(connection)?;
            
            debug!("Found {} keys matching pattern '{}'", keys.len(), pattern);
            Ok(keys)
        } else {
            error!("Not connected to Redis");
            Err(HissrvError::RedisError("Not connected to Redis".into()))
        }
    }
    
    fn get_string(&mut self, key: &str) -> Result<Option<String>> {
        let mut connection_guard = self.connection.lock().unwrap();
        
        if let Some(connection) = connection_guard.as_mut() {
            let result: RedisResult<Option<String>> = connection.get(key);
            
            match result {
                Ok(Some(val)) => Ok(Some(val)),
                Ok(None) => Ok(None),
                Err(e) => {
                    error!("Error getting key '{}': {}", key, e);
                    Err(HissrvError::RedisError(e.to_string()))
                }
            }
        } else {
            error!("Not connected to Redis");
            Err(HissrvError::RedisError("Not connected to Redis".into()))
        }
    }
    
    fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        let mut connection_guard = self.connection.lock().unwrap();
        
        if let Some(connection) = connection_guard.as_mut() {
            let result: RedisResult<HashMap<String, String>> = connection.hgetall(key);
            
            match result {
                Ok(hash) => Ok(hash),
                Err(e) => {
                    error!("Error getting hash '{}': {}", key, e);
                    Err(HissrvError::RedisError(e.to_string()))
                }
            }
        } else {
            error!("Not connected to Redis");
            Err(HissrvError::RedisError("Not connected to Redis".into()))
        }
    }
    
    fn start_polling(&mut self, interval: u64) -> Result<mpsc::Receiver<RedisDataPoint>> {
        if !self.is_connected {
            return Err(HissrvError::RedisError("Not connected to Redis".into()));
        }
        
        let (tx, rx) = mpsc::channel(100);
        let connection_clone = self.connection.clone();
        let pattern = self.config.key_pattern.clone();
        
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(interval));
            
            loop {
                interval.tick().await;
                
                let mut connection_guard = connection_clone.lock().unwrap();
                if connection_guard.is_none() {
                    error!("Redis connection lost");
                    break;
                }
                
                let connection = connection_guard.as_mut().unwrap();
                
                let keys_result: RedisResult<Vec<String>> = redis::cmd("KEYS")
                    .arg(&pattern)
                    .query(connection);
                
                match keys_result {
                    Ok(keys) => {
                        debug!("Found {} keys matching pattern '{}'", keys.len(), pattern);
                        
                        for key in keys {
                            let key_type: RedisResult<String> = redis::cmd("TYPE")
                                .arg(&key)
                                .query(connection);
                            
                            if let Ok(key_type) = key_type {
                                let timestamp = Utc::now();
                                let data_result = match key_type.as_str() {
                                    "string" => {
                                        let result: RedisResult<Option<String>> = connection.get(&key);
                                        match result {
                                            Ok(Some(val)) => {
                                                let data = Self::parse_data(&val);
                                                Some(RedisDataPoint {
                                                    key: key.clone(),
                                                    data,
                                                    timestamp,
                                                })
                                            }
                                            Ok(None) => None,
                                            Err(e) => {
                                                error!("Error getting key '{}': {}", key, e);
                                                None
                                            }
                                        }
                                    }
                                    "hash" => {
                                        let result: RedisResult<HashMap<String, String>> = 
                                            connection.hgetall(&key);
                                        match result {
                                            Ok(hash) => {
                                                Some(RedisDataPoint {
                                                    key: key.clone(),
                                                    data: RedisData::Hash(hash),
                                                    timestamp,
                                                })
                                            }
                                            Err(e) => {
                                                error!("Error getting hash '{}': {}", key, e);
                                                None
                                            }
                                        }
                                    }
                                    _ => {
                                        warn!("Unsupported Redis type for key '{}': {}", key, key_type);
                                        None
                                    }
                                };
                                
                                if let Some(data_point) = data_result {
                                    if tx.send(data_point).await.is_err() {
                                        error!("Failed to send data point, channel closed");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error getting keys: {}", e);
                    }
                }
                
                drop(connection_guard);
            }
        });
        
        Ok(rx)
    }
} 