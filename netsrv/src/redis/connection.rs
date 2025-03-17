use crate::config::redis_config::RedisConfig;
use crate::error::{NetSrvError, Result};
use log::{debug, error, info};
use redis::{Client, Connection, Commands};
use std::collections::HashMap;

pub struct RedisConnection {
    client: Option<Client>,
    conn: Option<Connection>,
    connected: bool,
}

impl RedisConnection {
    pub fn new() -> Self {
        RedisConnection {
            client: None,
            conn: None,
            connected: false,
        }
    }

    pub fn connect(&mut self, config: &RedisConfig) -> Result<()> {
        // Disconnect if already connected
        self.disconnect();

        let client = if !config.socket.is_empty() {
            // Connect using Unix socket
            Client::open(format!("unix://{}", config.socket))?
        } else {
            // Connect using TCP
            let redis_url = if config.password.is_empty() {
                format!("redis://{}:{}", config.host, config.port)
            } else {
                format!(
                    "redis://:{}@{}:{}",
                    config.password, config.host, config.port
                )
            };
            Client::open(redis_url)?
        };

        let mut conn = client.get_connection()?;

        // Test connection with PING
        let ping_result: String = redis::cmd("PING").query(&mut conn)?;
        if ping_result != "PONG" {
            return Err(NetSrvError::ConnectionError(
                "Redis connection test failed".to_string(),
            ));
        }

        if !config.socket.is_empty() {
            info!(
                "Successfully connected to Redis via Unix socket: {}",
                config.socket
            );
        } else {
            info!(
                "Successfully connected to Redis at {}:{}",
                config.host, config.port
            );
        }

        self.client = Some(client);
        self.conn = Some(conn);
        self.connected = true;

        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.conn = None;
        self.client = None;
        self.connected = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn get_keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        if !self.connected || self.conn.is_none() {
            return Err(NetSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query(conn)?;

        Ok(keys)
    }

    pub fn get_string(&mut self, key: &str) -> Result<String> {
        if !self.connected || self.conn.is_none() {
            return Err(NetSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let value: String = conn.get(key)?;
        Ok(value)
    }

    pub fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        if !self.connected || self.conn.is_none() {
            return Err(NetSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let value: HashMap<String, String> = conn.hgetall(key)?;
        Ok(value)
    }
} 