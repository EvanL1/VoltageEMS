use crate::config::redis_config::RedisConfig;
use crate::error::{NetSrvError, Result};
use crate::redis::RedisConnection;
use serde_json::{json, Value};
use tracing::{debug, error};

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
        // check if connected
        if !self.is_connected() {
            self.connect().await?;
        }

        // get all matching keys
        let mut all_data = json!({});
        let mut conn = self.connection.lock().unwrap();

        for pattern in &self.config.data_keys {
            let keys = conn.get_keys(pattern)?;
            debug!("Found {} keys matching pattern: {}", keys.len(), pattern);

            for key in keys {
                // get the type of the key and get the data
                match self.get_data_for_key(&mut conn, &key) {
                    Ok(data) => {
                        // convert the key name to the one without the prefix
                        let key_without_prefix = if key.starts_with(&self.config.prefix) {
                            key[self.config.prefix.len()..].to_string()
                        } else {
                            key.clone()
                        };

                        // add the data to the result
                        all_data[key_without_prefix] = data;
                    }
                    Err(e) => {
                        error!("Failed to get data for key {}: {}", key, e);
                    }
                }
            }
        }

        // update the last fetch time
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
                    // try to reconnect
                    if let Err(conn_err) = self.connect().await {
                        error!("Failed to reconnect to Redis: {}", conn_err);
                        // wait for a while and try again
                        time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }

    fn get_data_for_key(&self, conn: &mut RedisConnection, key: &str) -> Result<Value> {
        // try to get the hash table
        let hash_result = conn.get_hash(key);
        if let Ok(hash) = hash_result {
            if !hash.is_empty() {
                return Ok(json!(hash));
            }
        }

        // if not a hash table, try to get the string
        let string_result = conn.get_string(key);
        if let Ok(string_value) = string_result {
            // try to parse the string as JSON
            let json_result = serde_json::from_str::<Value>(&string_value);
            if let Ok(json_value) = json_result {
                return Ok(json_value);
            }
            // if not JSON, return as a normal string
            return Ok(json!(string_value));
        }

        // if all failed, return an empty object
        Err(NetSrvError::Data(format!("No data found for key: {}", key)))
    }
}
