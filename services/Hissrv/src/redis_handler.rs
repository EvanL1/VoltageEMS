use crate::config::Config;
use crate::error::{HisSrvError, Result};
use crate::influxdb_handler::{try_parse_numeric, InfluxDBConnection};
use std::collections::{HashMap, HashSet};
use voltage_common::redis::{RedisClient, RedisConfig, RedisType as CommonRedisType};

pub struct RedisConnection {
    client: Option<RedisClient>,
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

impl From<CommonRedisType> for RedisType {
    fn from(common_type: CommonRedisType) -> Self {
        match common_type {
            CommonRedisType::String => RedisType::String,
            CommonRedisType::List => RedisType::List,
            CommonRedisType::Set => RedisType::Set,
            CommonRedisType::ZSet => RedisType::ZSet,
            CommonRedisType::Hash => RedisType::Hash,
            CommonRedisType::None => RedisType::None,
            CommonRedisType::Stream => RedisType::None, // Stream not supported in old enum
        }
    }
}

impl RedisConnection {
    pub fn new() -> Self {
        RedisConnection { client: None }
    }

    pub async fn connect(&mut self, config: &Config) -> Result<()> {
        // Disconnect if already connected
        self.disconnect();

        let redis_config = if !config.redis_socket.is_empty() {
            RedisConfig {
                host: String::new(),
                port: 0,
                password: if config.redis_password.is_empty() {
                    None
                } else {
                    Some(config.redis_password.clone())
                },
                socket: Some(config.redis_socket.clone()),
                database: 0,
                connection_timeout: 10,
                max_retries: 3,
            }
        } else {
            RedisConfig {
                host: config.redis_host.clone(),
                port: config.redis_port,
                password: if config.redis_password.is_empty() {
                    None
                } else {
                    Some(config.redis_password.clone())
                },
                socket: None,
                database: 0,
                connection_timeout: 10,
                max_retries: 3,
            }
        };

        let url = redis_config.to_url();
        let client = RedisClient::new(&url).await.map_err(|e| {
            HisSrvError::ConnectionError(format!("Failed to create Redis client: {}", e))
        })?;

        // Test connection with PING
        let ping_result = client
            .ping()
            .await
            .map_err(|e| HisSrvError::ConnectionError(format!("Redis ping failed: {}", e)))?;

        if ping_result != "PONG" {
            return Err(HisSrvError::ConnectionError(
                "Redis connection test failed".to_string(),
            ));
        }

        if !config.redis_socket.is_empty() {
            println!(
                "Successfully connected to Redis via Unix socket: {}",
                config.redis_socket
            );
        } else {
            println!(
                "Successfully connected to Redis at {}:{}",
                config.redis_host, config.redis_port
            );
        }

        self.client = Some(client);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.client = None;
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    pub async fn get_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        client
            .keys(pattern)
            .await
            .map_err(|e| HisSrvError::ConnectionError(format!("Failed to get keys: {}", e)))
    }

    pub async fn get_type(&self, key: &str) -> Result<RedisType> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        let common_type = client
            .key_type(key)
            .await
            .map_err(|e| HisSrvError::ConnectionError(format!("Failed to get type: {}", e)))?;

        Ok(RedisType::from(common_type))
    }

    pub async fn get_string(&self, key: &str) -> Result<String> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        match client.get(key).await {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(HisSrvError::NotFound(format!("Key not found: {}", key))),
            Err(e) => Err(HisSrvError::ConnectionError(format!(
                "Failed to get string: {}",
                e
            ))),
        }
    }

    pub async fn get_hash(&self, key: &str) -> Result<HashMap<String, String>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        client
            .hgetall(key)
            .await
            .map_err(|e| HisSrvError::ConnectionError(format!("Failed to get hash: {}", e)))
    }

    pub async fn get_list(&self, key: &str) -> Result<Vec<String>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        client
            .lrange(key, 0, -1)
            .await
            .map_err(|e| HisSrvError::ConnectionError(format!("Failed to get list: {}", e)))
    }

    pub async fn get_set(&self, key: &str) -> Result<HashSet<String>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        client
            .smembers(key)
            .await
            .map_err(|e| HisSrvError::ConnectionError(format!("Failed to get set: {}", e)))
    }

    pub async fn get_zset(&self, key: &str) -> Result<Vec<(String, f64)>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        client
            .zrange_withscores(key, 0, -1)
            .await
            .map_err(|e| HisSrvError::ConnectionError(format!("Failed to get sorted set: {}", e)))
    }

    /// Get real-time data from comsrv channel using optimized Hash structure
    pub async fn get_channel_realtime_data(
        &self,
        channel_id: u16,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        let hash_key = format!("comsrv:realtime:channel:{}", channel_id);
        let raw_data = client.hgetall(&hash_key).await.map_err(|e| {
            HisSrvError::ConnectionError(format!("Failed to get channel data: {}", e))
        })?;

        let mut result = HashMap::new();
        for (field, value) in raw_data {
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&value) {
                result.insert(field, json_value);
            }
        }

        Ok(result)
    }

    /// Get real-time data from modsrv module using optimized Hash structure
    pub async fn get_module_realtime_data(
        &self,
        module_id: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HisSrvError::ConnectionError("Not connected to Redis".to_string()))?;

        let hash_key = format!("modsrv:realtime:module:{}", module_id);
        let raw_data = client.hgetall(&hash_key).await.map_err(|e| {
            HisSrvError::ConnectionError(format!("Failed to get module data: {}", e))
        })?;

        let mut result = HashMap::new();
        for (field, value) in raw_data {
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&value) {
                result.insert(field, json_value);
            }
        }

        Ok(result)
    }

    /// Get multiple channels data in batch using Pipeline
    pub async fn get_channels_batch(
        &self,
        channel_ids: &[u16],
    ) -> Result<HashMap<u16, HashMap<String, serde_json::Value>>> {
        if channel_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut result = HashMap::new();

        // Process each channel (could be optimized with pipeline in future)
        for &channel_id in channel_ids {
            match self.get_channel_realtime_data(channel_id).await {
                Ok(data) => {
                    result.insert(channel_id, data);
                }
                Err(e) => {
                    tracing::warn!("Failed to get data for channel {}: {}", channel_id, e);
                }
            }
        }

        Ok(result)
    }
}

pub async fn process_redis_data(
    redis: &mut RedisConnection,
    influxdb: &mut InfluxDBConnection,
    config: &Config,
) -> Result<()> {
    if !config.enable_influxdb || !influxdb.is_connected() {
        if config.verbose {
            println!(
                "InfluxDB writing is disabled. Waiting {} seconds...",
                config.interval_seconds
            );
        }
        return Ok(());
    }

    if !redis.is_connected() {
        println!("Redis connection lost. Attempting to reconnect...");
        if let Err(e) = redis.connect(config).await {
            println!("Failed to reconnect to Redis: {}", e);
            return Err(HisSrvError::ConnectionError(
                "Failed to reconnect to Redis".to_string(),
            ));
        }
    }

    let mut stored_points = 0;
    let mut skipped_points = 0;

    // Check if we should process comsrv channels
    if config
        .redis_key_pattern
        .contains("comsrv:realtime:channel:")
    {
        // Extract channel IDs from pattern or use predefined list
        let channel_ids: Vec<u16> = vec![1, 2, 3]; // TODO: Make configurable

        for channel_id in channel_ids {
            match redis.get_channel_realtime_data(channel_id).await {
                Ok(channel_data) => {
                    if config.verbose {
                        println!(
                            "Processing channel {} with {} points",
                            channel_id,
                            channel_data.len()
                        );
                    }

                    // Convert to format expected by InfluxDB
                    let mut hash_data = HashMap::new();
                    for (field, value) in channel_data {
                        hash_data.insert(field, value.to_string());
                    }

                    let key = format!("comsrv:realtime:channel:{}", channel_id);
                    match influxdb.write_hash_data(&key, hash_data, config).await {
                        Ok(points) => {
                            stored_points += points;
                            if config.verbose > 1 {
                                println!("Stored {} points from channel {}", points, channel_id);
                            }
                        }
                        Err(e) => {
                            println!("Failed to write data for channel {}: {}", channel_id, e);
                            skipped_points += 1;
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to get data for channel {}: {}", channel_id, e);
                    skipped_points += 1;
                }
            }
        }
    }

    // Check if we should process modsrv modules
    if config.redis_key_pattern.contains("modsrv:realtime:module:") {
        // Extract module IDs from pattern or use predefined list
        let module_ids: Vec<&str> = vec!["calc_module_1", "calc_module_2"]; // TODO: Make configurable

        for module_id in module_ids {
            match redis.get_module_realtime_data(module_id).await {
                Ok(module_data) => {
                    if config.verbose {
                        println!(
                            "Processing module {} with {} points",
                            module_id,
                            module_data.len()
                        );
                    }

                    // Convert to format expected by InfluxDB
                    let mut hash_data = HashMap::new();
                    for (field, value) in module_data {
                        hash_data.insert(field, value.to_string());
                    }

                    let key = format!("modsrv:realtime:module:{}", module_id);
                    match influxdb.write_hash_data(&key, hash_data, config).await {
                        Ok(points) => {
                            stored_points += points;
                            if config.verbose > 1 {
                                println!("Stored {} points from module {}", points, module_id);
                            }
                        }
                        Err(e) => {
                            println!("Failed to write data for module {}: {}", module_id, e);
                            skipped_points += 1;
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to get data for module {}: {}", module_id, e);
                    skipped_points += 1;
                }
            }
        }
    }

    // Fall back to old pattern-based processing for backward compatibility
    if !config.redis_key_pattern.contains("comsrv:realtime:")
        && !config.redis_key_pattern.contains("modsrv:realtime:")
    {
        let keys = redis.get_keys(&config.redis_key_pattern).await?;

        if config.verbose {
            println!(
                "Found {} keys matching pattern: {}",
                keys.len(),
                config.redis_key_pattern
            );
        }

        for key in &keys {
            if config.verbose > 1 {
                println!("Processing key: {}", key);
            }

            let key_type = redis.get_type(key).await?;

            match key_type {
                RedisType::Hash => match redis.get_hash(key).await {
                    Ok(hash_data) => match influxdb.write_hash_data(key, hash_data, config).await {
                        Ok(points) => {
                            stored_points += points;
                            if config.verbose > 1 {
                                println!("Stored {} points from key: {}", points, key);
                            }
                        }
                        Err(e) => {
                            println!("Failed to write data for key {}: {}", key, e);
                            skipped_points += 1;
                        }
                    },
                    Err(e) => {
                        println!("Failed to get hash data for key {}: {}", key, e);
                        skipped_points += 1;
                    }
                },
                _ => {
                    if config.verbose > 1 {
                        println!("Skipping non-hash key: {} (type: {:?})", key, key_type);
                    }
                    skipped_points += 1;
                }
            }
        }
    }

    if config.verbose {
        println!(
            "Processed {} points, skipped {} keys",
            stored_points, skipped_points
        );
    }

    Ok(())
}
