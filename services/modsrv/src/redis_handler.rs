use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use redis::{Client, Connection, Commands, RedisResult, Value};
use std::collections::HashMap;
use log::{info, error, debug};

pub struct RedisConnection {
    client: Option<Client>,
    conn: Option<Connection>,
    connected: bool,
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
        RedisConnection {
            client: None,
            conn: None,
            connected: false,
        }
    }

    pub fn connect(&mut self, config: &Config) -> Result<()> {
        // Disconnect if already connected
        self.disconnect();

        let client = if !config.redis.socket.is_empty() {
            // Connect using Unix socket
            Client::open(format!("unix://{}", config.redis.socket))?
        } else {
            // Connect using TCP
            let redis_url = if config.redis.password.is_empty() {
                format!("redis://{}:{}", config.redis.host, config.redis.port)
            } else {
                format!(
                    "redis://:{}@{}:{}",
                    config.redis.password, config.redis.host, config.redis.port
                )
            };
            Client::open(redis_url)?
        };

        let mut conn = client.get_connection()?;

        // Test connection with PING
        let ping_result: String = redis::cmd("PING").query(&mut conn)?;
        if ping_result != "PONG" {
            return Err(ModelSrvError::ConnectionError(
                "Redis connection test failed".to_string(),
            ));
        }

        if !config.redis.socket.is_empty() {
            info!(
                "Successfully connected to Redis via Unix socket: {}",
                config.redis.socket
            );
        } else {
            info!(
                "Successfully connected to Redis at {}:{}",
                config.redis.host, config.redis.port
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
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query(conn)?;

        Ok(keys)
    }

    pub fn get_type(&mut self, key: &str) -> Result<RedisType> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let type_str: String = redis::cmd("TYPE")
            .arg(key)
            .query(conn)?;

        match type_str.as_str() {
            "string" => Ok(RedisType::String),
            "list" => Ok(RedisType::List),
            "set" => Ok(RedisType::Set),
            "hash" => Ok(RedisType::Hash),
            "zset" => Ok(RedisType::ZSet),
            "none" => Ok(RedisType::None),
            _ => Ok(RedisType::None),
        }
    }

    pub fn get_string(&mut self, key: &str) -> Result<String> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let value: String = conn.get(key)?;
        Ok(value)
    }

    pub fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let value: HashMap<String, String> = conn.hgetall(key)?;
        Ok(value)
    }

    pub fn set_hash_field(&mut self, key: &str, field: &str, value: &str) -> Result<()> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        conn.hset(key, field, value)?;
        Ok(())
    }

    pub fn set_hash(&mut self, key: &str, hash: &HashMap<String, String>) -> Result<()> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        
        // Convert HashMap to a flat vector of alternating keys and values
        let mut fields_and_values = Vec::with_capacity(hash.len() * 2);
        for (field, value) in hash {
            fields_and_values.push(field.as_str());
            fields_and_values.push(value.as_str());
        }
        
        if !fields_and_values.is_empty() {
            conn.hset_multiple(key, &fields_and_values)?;
        }
        
        Ok(())
    }

    /// 将值推送到列表的右端（尾部）
    pub fn push_list(&mut self, key: &str, value: &str) -> Result<()> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        conn.rpush(key, value)?;
        Ok(())
    }

    /// 将值推送到列表的左端（头部）
    pub fn push_list_front(&mut self, key: &str, value: &str) -> Result<()> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        conn.lpush(key, value)?;
        Ok(())
    }

    /// 从列表的右端（尾部）弹出值
    pub fn pop_list(&mut self, key: &str) -> Result<Option<String>> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let value: Option<String> = conn.rpop(key)?;
        Ok(value)
    }

    /// 从列表的左端（头部）弹出值
    pub fn pop_list_front(&mut self, key: &str) -> Result<Option<String>> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let value: Option<String> = conn.lpop(key)?;
        Ok(value)
    }

    /// 获取列表的长度
    pub fn list_len(&mut self, key: &str) -> Result<usize> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let len: usize = conn.llen(key)?;
        Ok(len)
    }

    /// 获取列表的范围
    pub fn list_range(&mut self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let values: Vec<String> = conn.lrange(key, start, stop)?;
        Ok(values)
    }

    /// 阻塞式从列表弹出值，支持超时
    pub fn blpop(&mut self, key: &str, timeout_seconds: usize) -> Result<Option<(String, String)>> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let result: Option<(String, String)> = conn.blpop(key, timeout_seconds)?;
        Ok(result)
    }

    /// 阻塞式从多个列表弹出值，支持超时
    pub fn blpop_multiple(&mut self, keys: &[&str], timeout_seconds: usize) -> Result<Option<(String, String)>> {
        if !self.connected || self.conn.is_none() {
            return Err(ModelSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let result: Option<(String, String)> = conn.blpop(keys, timeout_seconds)?;
        Ok(result)
    }
} 