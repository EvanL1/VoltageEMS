use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use redis::{Client, Connection, Commands, RedisResult, Value};
use std::collections::HashMap;
use log::{info, error, debug};

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

    /// 将值推送到列表的右端（尾部）
    pub fn push_list(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            conn.rpush(key, value).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// 将值推送到列表的左端（头部）
    pub fn push_list_front(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(conn) = &mut self.connection {
            conn.lpush(key, value).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// 从列表的右端（尾部）弹出值
    pub fn pop_list(&mut self, key: &str) -> Result<Option<String>> {
        if let Some(conn) = &mut self.connection {
            conn.rpop::<_, Option<String>>(key, None)
                .map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// 从列表的左端（头部）弹出值
    pub fn pop_list_front(&mut self, key: &str) -> Result<Option<String>> {
        if let Some(conn) = &mut self.connection {
            conn.lpop::<_, Option<String>>(key, None)
                .map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// 获取列表的长度
    pub fn list_len(&mut self, key: &str) -> Result<usize> {
        if let Some(conn) = &mut self.connection {
            conn.llen(key).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// 获取列表的范围
    pub fn list_range(&mut self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        if let Some(conn) = &mut self.connection {
            conn.lrange(key, start, stop).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// 阻塞式从列表弹出值，支持超时
    pub fn blpop(&mut self, key: &str, timeout_seconds: usize) -> Result<Option<(String, String)>> {
        if let Some(conn) = &mut self.connection {
            conn.blpop(key, timeout_seconds).map_err(|e| ModelSrvError::RedisError(e.to_string()))
        } else {
            Err(ModelSrvError::RedisError("Not connected to Redis".to_string()))
        }
    }

    /// 阻塞式从多个列表弹出值，支持超时
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

    /// 发布消息到指定的频道
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
} 