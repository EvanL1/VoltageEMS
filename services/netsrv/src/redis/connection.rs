use crate::config::RedisConfig;
use crate::error::{NetSrvError, Result};
use std::collections::HashMap;
use tracing::info;
use voltage_libs::redis::RedisClient;

pub struct RedisConnection {
    client: Option<RedisClient>,
    connected: bool,
}

impl RedisConnection {
    pub fn new() -> Self {
        RedisConnection {
            client: None,
            connected: false,
        }
    }

    pub async fn connect(&mut self, config: &RedisConfig) -> Result<()> {
        // Disconnect if already connected
        self.disconnect();

        // Create new client
        let client = RedisClient::new(&config.url).await
            .map_err(|e| NetSrvError::Connection(format!("Failed to create Redis client: {}", e)))?;

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

    pub async fn get_string(&mut self, key: &str) -> Result<String> {
        if !self.connected || self.client.is_none() {
            return Err(NetSrvError::Connection(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_mut().unwrap();
        match client.get::<String>(key).await {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(NetSrvError::Redis(format!("Key not found: {}", key))),
            Err(e) => Err(NetSrvError::Redis(format!("Failed to get string: {}", e))),
        }
    }

    pub async fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        if !self.connected || self.client.is_none() {
            return Err(NetSrvError::Connection(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_mut().unwrap();
        client.hgetall(key).await
            .map_err(|e| NetSrvError::Redis(format!("Failed to get hash: {}", e)))
    }
}