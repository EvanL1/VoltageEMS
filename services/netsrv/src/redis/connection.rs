use voltage_config::RedisConfig;
use crate::error::{NetSrvError, Result};
use voltage_common::redis::RedisSyncClient;
use std::collections::HashMap;
use tracing::info;

pub struct RedisConnection {
    client: Option<RedisSyncClient>,
    connected: bool,
}

impl RedisConnection {
    pub fn new() -> Self {
        RedisConnection {
            client: None,
            connected: false,
        }
    }

    pub fn connect(&mut self, config: &RedisConfig) -> Result<()> {
        // Disconnect if already connected
        self.disconnect();

        // Use the URL directly from voltage_config RedisConfig
        let url = config.url.clone();
        let client = RedisSyncClient::new(&url)
            .map_err(|e| NetSrvError::Connection(format!("Failed to create Redis client: {}", e)))?;

        // Test connection with PING
        let ping_result = client.ping()
            .map_err(|e| NetSrvError::Connection(format!("Redis ping failed: {}", e)))?;
        
        if ping_result != "PONG" {
            return Err(NetSrvError::Connection(
                "Redis connection test failed".to_string(),
            ));
        }

        info!("Successfully connected to Redis at {}", config.url);

        self.client = Some(client);
        self.connected = true;

        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.client = None;
        self.connected = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn get_keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        if !self.connected || self.client.is_none() {
            return Err(NetSrvError::Connection(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_mut().unwrap();
        let keys = client.keys(pattern)
            .map_err(|e| NetSrvError::Redis(format!("Failed to get keys: {}", e)))?;

        Ok(keys)
    }

    pub fn get_string(&mut self, key: &str) -> Result<String> {
        if !self.connected || self.client.is_none() {
            return Err(NetSrvError::Connection(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_mut().unwrap();
        match client.get(key) {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(NetSrvError::Redis(format!("Key not found: {}", key))),
            Err(e) => Err(NetSrvError::Redis(format!("Failed to get string: {}", e))),
        }
    }

    pub fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        if !self.connected || self.client.is_none() {
            return Err(NetSrvError::Connection(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_mut().unwrap();
        let value = client.hgetall(key)
            .map_err(|e| NetSrvError::Redis(format!("Failed to get hash: {}", e)))?;
        Ok(value)
    }

    pub fn set_hash_field(&mut self, key: &str, field: &str, value: &str) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(NetSrvError::Connection(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_mut().unwrap();
        client.hset(key, field, value)
            .map_err(|e| NetSrvError::Redis(format!("Failed to set hash field: {}", e)))?;
        Ok(())
    }

    pub fn set_hash_multiple(&mut self, key: &str, fields: Vec<(&str, &str)>) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(NetSrvError::Connection(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_mut().unwrap();
        
        // Use pipeline for batch updates
        let mut pipe = client.pipeline();
        for (field, value) in fields {
            pipe.hset(key, field, value);
        }
        
        pipe.execute()
            .map_err(|e| NetSrvError::Redis(format!("Failed to set hash fields: {}", e)))?;
        Ok(())
    }
}