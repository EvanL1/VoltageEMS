use crate::error::{NetSrvError, Result};
use serde_json::Value;
use tracing::{debug, error};
use voltage_common::redis::RedisClient;
use voltage_config::RedisConfig;

use std::time::{Duration, Instant};
use tokio::time;

pub struct RedisDataFetcher {
    client: Option<RedisClient>,
    config: RedisConfig,
    data_key_pattern: String,
    poll_interval: Duration,
    last_fetch_time: Instant,
}

impl RedisDataFetcher {
    pub fn new(
        config: RedisConfig,
        data_key_pattern: String,
        poll_interval_secs: u64,
    ) -> Result<Self> {
        Ok(RedisDataFetcher {
            client: None,
            config,
            data_key_pattern,
            poll_interval: Duration::from_secs(poll_interval_secs),
            last_fetch_time: Instant::now(),
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        let client = RedisClient::new(&self.config.url)
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to connect to Redis: {}", e)))?;

        self.client = Some(client);
        Ok(())
    }

    pub async fn fetch_data(&mut self) -> Result<Value> {
        if self.client.is_none() {
            self.connect().await?;
        }

        let client = self.client.as_ref().unwrap();

        // 获取匹配的所有键
        let keys: Vec<String> = client
            .keys(&self.data_key_pattern)
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to get keys: {}", e)))?;

        debug!(
            "Found {} keys matching pattern: {}",
            keys.len(),
            self.data_key_pattern
        );

        let mut data = json!({});
        let data_obj = data.as_object_mut().unwrap();

        for key in keys {
            match self.fetch_key_data(&key).await {
                Ok(value) => {
                    data_obj.insert(key, value);
                }
                Err(e) => {
                    error!("Failed to fetch data for key {}: {}", key, e);
                }
            }
        }

        self.last_fetch_time = Instant::now();
        Ok(data)
    }

    async fn fetch_key_data(&self, key: &str) -> Result<Value> {
        let client = self.client.as_ref().unwrap();

        // Try to get as string first
        match client.get(key).await {
            Ok(Some(value)) => {
                // Try to parse as JSON
                if let Ok(json_value) = serde_json::from_str::<Value>(&value) {
                    Ok(json_value)
                } else {
                    Ok(json!(value))
                }
            }
            Ok(None) => Ok(json!(null)),
            Err(_) => {
                // If string get fails, try as hash
                match client.hgetall(key).await {
                    Ok(hash_map) => Ok(json!(hash_map)),
                    Err(e) => Err(NetSrvError::Redis(format!(
                        "Failed to get value for key {}: {}",
                        key, e
                    ))),
                }
            }
        }
    }

    pub fn should_fetch(&self) -> bool {
        self.last_fetch_time.elapsed() >= self.poll_interval
    }

    pub async fn wait_for_next_poll(&self) {
        let elapsed = self.last_fetch_time.elapsed();
        if elapsed < self.poll_interval {
            let remaining = self.poll_interval - elapsed;
            time::sleep(remaining).await;
        }
    }

    pub async fn start_polling(&mut self, tx: tokio::sync::mpsc::Sender<Value>) -> Result<()> {
        loop {
            if self.should_fetch() {
                match self.fetch_data().await {
                    Ok(data) => {
                        if let Err(e) = tx.send(data).await {
                            error!("Failed to send data to channel: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to fetch data from Redis: {}", e);
                        // Try to reconnect
                        if let Err(conn_err) = self.connect().await {
                            error!("Failed to reconnect to Redis: {}", conn_err);
                            // Wait before trying again
                            time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
            self.wait_for_next_poll().await;
        }
    }
}
