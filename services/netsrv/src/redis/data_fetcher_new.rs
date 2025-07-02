use crate::error::{NetSrvError, Result};
use voltage_config::RedisConfig;
use tracing::{debug, error};
use serde_json::{json, Value};
use redis::{AsyncCommands, Client, Connection};

use std::time::{Duration, Instant};
use tokio::time;

pub struct RedisDataFetcher {
    client: redis::Client,
    config: RedisConfig,
    data_key_pattern: String,
    poll_interval: Duration,
    last_fetch_time: Instant,
}

impl RedisDataFetcher {
    pub fn new(config: RedisConfig, data_key_pattern: String, poll_interval_secs: u64) -> Result<Self> {
        let client = Client::open(config.url.clone())
            .map_err(|e| NetSrvError::Redis(format!("Failed to create Redis client: {}", e)))?;
            
        Ok(RedisDataFetcher {
            client,
            config,
            data_key_pattern,
            poll_interval: Duration::from_secs(poll_interval_secs),
            last_fetch_time: Instant::now(),
        })
    }

    pub async fn connect(&self) -> Result<redis::aio::Connection> {
        self.client.get_async_connection().await
            .map_err(|e| NetSrvError::Redis(format!("Failed to connect to Redis: {}", e)))
    }

    pub async fn fetch_data(&mut self) -> Result<Value> {
        let mut conn = self.connect().await?;
        
        // 获取匹配的所有键
        let keys: Vec<String> = conn.keys(&self.data_key_pattern).await
            .map_err(|e| NetSrvError::Redis(format!("Failed to get keys: {}", e)))?;
            
        debug!("Found {} keys matching pattern: {}", keys.len(), self.data_key_pattern);
        
        let mut all_data = json!({});
        
        for key in keys {
            match self.get_data_for_key(&mut conn, &key).await {
                Ok(data) => {
                    // 移除前缀
                    let key_without_prefix = if key.starts_with(&self.config.prefix) {
                        key[self.config.prefix.len()..].to_string()
                    } else {
                        key.clone()
                    };
                    
                    all_data[key_without_prefix] = data;
                }
                Err(e) => {
                    error!("Failed to get data for key {}: {}", key, e);
                }
            }
        }
        
        self.last_fetch_time = Instant::now();
        Ok(all_data)
    }

    pub async fn start_polling(&mut self, tx: tokio::sync::mpsc::Sender<Value>) -> Result<()> {
        let mut interval = time::interval(self.poll_interval);
        
        loop {
            interval.tick().await;
            
            match self.fetch_data().await {
                Ok(data) => {
                    if let Err(e) = tx.send(data).await {
                        error!("Failed to send data to channel: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to fetch data from Redis: {}", e);
                    // 等待一段时间后重试
                    time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn get_data_for_key(&self, conn: &mut redis::aio::Connection, key: &str) -> Result<Value> {
        // 尝试获取哈希表
        let hash_result: redis::RedisResult<std::collections::HashMap<String, String>> = conn.hgetall(key).await;
        if let Ok(hash) = hash_result {
            if !hash.is_empty() {
                return Ok(json!(hash));
            }
        }
        
        // 如果不是哈希表，尝试获取字符串
        let string_result: redis::RedisResult<String> = conn.get(key).await;
        if let Ok(string_value) = string_result {
            // 尝试解析为JSON
            if let Ok(json_value) = serde_json::from_str::<Value>(&string_value) {
                return Ok(json_value);
            }
            // 如果不是JSON，返回字符串
            return Ok(json!(string_value));
        }
        
        Err(NetSrvError::Data(format!("No data found for key: {}", key)))
    }
}