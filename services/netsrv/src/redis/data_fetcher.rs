use crate::config::redis_config::RedisConfig;
use crate::error::{NetSrvError, Result};
use crate::redis::RedisConnection;
use log::{debug, error, info, warn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time;

pub struct RedisDataFetcher {
    connection: Arc<Mutex<RedisConnection>>,
    config: RedisConfig,
    last_fetch_time: Instant,
}

impl RedisDataFetcher {
    pub fn new(config: RedisConfig) -> Self {
        RedisDataFetcher {
            connection: Arc::new(Mutex::new(RedisConnection::new())),
            config,
            last_fetch_time: Instant::now(),
        }
    }

    pub async fn connect(&self) -> Result<()> {
        let mut conn = self.connection.lock().unwrap();
        conn.connect(&self.config)
    }

    pub fn is_connected(&self) -> bool {
        let conn = self.connection.lock().unwrap();
        conn.is_connected()
    }

    pub async fn fetch_data(&mut self) -> Result<Value> {
        // 检查是否已连接
        if !self.is_connected() {
            self.connect().await?;
        }

        // 获取所有匹配的键
        let mut all_data = json!({});
        let mut conn = self.connection.lock().unwrap();

        for pattern in &self.config.data_keys {
            let keys = conn.get_keys(pattern)?;
            debug!("Found {} keys matching pattern: {}", keys.len(), pattern);

            for key in keys {
                // 获取键的类型并根据类型获取数据
                match self.get_data_for_key(&mut conn, &key) {
                    Ok(data) => {
                        // 将键名转换为没有前缀的形式
                        let key_without_prefix = if key.starts_with(&self.config.prefix) {
                            key[self.config.prefix.len()..].to_string()
                        } else {
                            key.clone()
                        };

                        // 将数据添加到结果中
                        all_data[key_without_prefix] = data;
                    }
                    Err(e) => {
                        error!("Failed to get data for key {}: {}", key, e);
                    }
                }
            }
        }

        // 更新最后获取时间
        self.last_fetch_time = Instant::now();

        Ok(all_data)
    }

    pub async fn start_polling(&mut self, tx: tokio::sync::mpsc::Sender<Value>) -> Result<()> {
        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);
        let mut interval = time::interval(poll_interval);

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
                    // 尝试重新连接
                    if let Err(conn_err) = self.connect().await {
                        error!("Failed to reconnect to Redis: {}", conn_err);
                        // 等待一段时间后再尝试
                        time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }

    fn get_data_for_key(&self, conn: &mut RedisConnection, key: &str) -> Result<Value> {
        // 尝试获取哈希表
        let hash_result = conn.get_hash(key);
        if let Ok(hash) = hash_result {
            if !hash.is_empty() {
                return Ok(json!(hash));
            }
        }

        // 如果不是哈希表，尝试获取字符串
        let string_result = conn.get_string(key);
        if let Ok(string_value) = string_result {
            // 尝试将字符串解析为 JSON
            let json_result = serde_json::from_str::<Value>(&string_value);
            if let Ok(json_value) = json_result {
                return Ok(json_value);
            }
            // 如果不是 JSON，则作为普通字符串返回
            return Ok(json!(string_value));
        }

        // 如果都失败了，返回空对象
        Err(NetSrvError::DataError(format!("No data found for key: {}", key)))
    }
} 