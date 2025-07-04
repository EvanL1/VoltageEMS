use async_trait::async_trait;
use crate::config::RedisConnectionConfig;
use crate::error::{HisSrvError, Result};
use crate::storage::{DataPoint, QueryFilter, QueryResult, Storage, StorageStats};
use redis::{Client, AsyncCommands};
use chrono::{DateTime, Utc};
use serde_json;

pub struct RedisStorage {
    client: Option<Client>,
    connection: Option<redis::aio::Connection>,
    config: RedisConnectionConfig,
    connected: bool,
    last_write_time: Option<DateTime<Utc>>,
    last_read_time: Option<DateTime<Utc>>,
}

impl RedisStorage {
    pub fn new(config: RedisConnectionConfig) -> Self {
        Self {
            client: None,
            connection: None,
            config,
            connected: false,
            last_write_time: None,
            last_read_time: None,
        }
    }

    fn get_redis_url(&self) -> String {
        if !self.config.socket.is_empty() {
            format!("unix://{}", self.config.socket)
        } else {
            if self.config.password.is_empty() {
                format!("redis://{}:{}/{}", self.config.host, self.config.port, self.config.database)
            } else {
                format!(
                    "redis://:{}@{}:{}/{}",
                    self.config.password, self.config.host, self.config.port, self.config.database
                )
            }
        }
    }

    fn data_point_to_key(&self, data_point: &DataPoint) -> String {
        format!("hissrv:data:{}", data_point.key)
    }

    fn data_point_to_json(&self, data_point: &DataPoint) -> Result<String> {
        serde_json::to_string(data_point)
            .map_err(|e| HisSrvError::SerializationError(format!("Failed to serialize data point: {}", e)))
    }

    fn json_to_data_point(&self, json: &str) -> Result<DataPoint> {
        serde_json::from_str(json)
            .map_err(|e| HisSrvError::SerializationError(format!("Failed to deserialize data point: {}", e)))
    }
}

#[async_trait]
impl Storage for RedisStorage {
    async fn connect(&mut self) -> Result<()> {
        let redis_url = self.get_redis_url();
        let client = Client::open(redis_url.clone())?;

        // Test connection
        let mut conn = client.get_async_connection().await?;
        let ping_result: String = redis::cmd("PING").query_async(&mut conn).await?;
        
        if ping_result != "PONG" {
            return Err(HisSrvError::ConnectionError("Redis connection test failed".to_string()));
        }

        let redis_address = if !self.config.socket.is_empty() { 
            self.config.socket.clone()
        } else { 
            format!("{}:{}", self.config.host, self.config.port) 
        };
        tracing::info!("Successfully connected to Redis at {}", redis_address);

        self.client = Some(client);
        self.connection = Some(conn);
        self.connected = true;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connection = None;
        self.client = None;
        self.connected = false;
        tracing::info!("Disconnected from Redis");
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.connected && self.connection.is_some()
    }

    async fn store_data_point(&mut self, data_point: &DataPoint) -> Result<()> {
        if !self.connected || self.connection.is_none() {
            return Err(HisSrvError::ConnectionError("Not connected to Redis".to_string()));
        }

        let key = self.data_point_to_key(data_point);
        let json_data = self.data_point_to_json(data_point)?;

        // Store as JSON string with timestamp as score for sorted set
        let score = data_point.timestamp.timestamp() as f64;
        let conn = self.connection.as_mut().unwrap();
        let _: () = conn.zadd(&key, &json_data, score).await?;

        // Also store the latest value for quick access
        let latest_key = format!("{}:latest", key);
        let _: () = conn.set(&latest_key, &json_data).await?;

        // Set expiration based on retention policy (convert days to seconds)
        let expiry_seconds = 30 * 24 * 60 * 60; // Default 30 days
        let _: () = conn.expire(&key, expiry_seconds).await?;
        let _: () = conn.expire(&latest_key, expiry_seconds).await?;

        self.last_write_time = Some(Utc::now());
        Ok(())
    }

    async fn store_data_points(&mut self, data_points: &[DataPoint]) -> Result<()> {
        if !self.connected || self.connection.is_none() {
            return Err(HisSrvError::ConnectionError("Not connected to Redis".to_string()));
        }

        // Prepare all data first to avoid borrowing issues
        let mut operations = Vec::new();
        for data_point in data_points {
            let key = self.data_point_to_key(data_point);
            let json_data = self.data_point_to_json(data_point)?;
            let score = data_point.timestamp.timestamp() as f64;
            let latest_key = format!("{}:latest", key);
            
            operations.push((key, json_data, score, latest_key));
        }

        let conn = self.connection.as_mut().unwrap();
        
        // Use pipeline for better performance
        let mut pipe = redis::pipe();
        pipe.atomic();

        for (key, json_data, score, latest_key) in operations {
            pipe.zadd(&key, &json_data, score);
            
            // Update latest value
            pipe.set(&latest_key, &json_data);
            
            // Set expiration
            let expiry_seconds = 30 * 24 * 60 * 60; // Default 30 days
            pipe.expire(&key, expiry_seconds);
            pipe.expire(&latest_key, expiry_seconds);
        }

        let _: () = pipe.query_async(conn).await?;
        self.last_write_time = Some(Utc::now());
        Ok(())
    }

    async fn query_data_points(&self, filter: &QueryFilter) -> Result<QueryResult> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError("Not connected to Redis".to_string()));
        }

        let mut data_points = Vec::new();

        // Get keys matching pattern
        let key_pattern = if let Some(pattern) = &filter.key_pattern {
            format!("hissrv:data:{}", pattern)
        } else {
            "hissrv:data:*".to_string()
        };

        // Get keys using a new connection
        let keys: Vec<String> = {
            let mut temp_conn = self.client.as_ref().unwrap().get_async_connection().await?;
            temp_conn.keys(key_pattern).await?
        };

        for key in keys {
            // Skip latest keys
            if key.ends_with(":latest") {
                continue;
            }

            let mut start_score = f64::NEG_INFINITY;
            let mut end_score = f64::INFINITY;

            // Apply time range filters
            if let Some(start_time) = filter.start_time {
                start_score = start_time.timestamp() as f64;
            }
            if let Some(end_time) = filter.end_time {
                end_score = end_time.timestamp() as f64;
            }

            // Get data from sorted set within time range
            let results: Vec<String> = {
                let mut temp_conn = self.client.as_ref().unwrap().get_async_connection().await?;
                temp_conn.zrangebyscore(&key, start_score, end_score).await?
            };

            for json_data in results {
                match self.json_to_data_point(&json_data) {
                    Ok(data_point) => {
                        // Apply tag filters
                        let mut matches = true;
                        for (tag_key, tag_value) in &filter.tags {
                            if let Some(actual_value) = data_point.tags.get(tag_key) {
                                if actual_value != tag_value {
                                    matches = false;
                                    break;
                                }
                            } else {
                                matches = false;
                                break;
                            }
                        }

                        if matches {
                            data_points.push(data_point);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse data point from Redis: {}", e);
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        data_points.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply limit and offset
        let total_count = data_points.len() as u64;
        let offset = filter.offset.unwrap_or(0) as usize;
        let limit = filter.limit.map(|l| l as usize);

        if offset < data_points.len() {
            data_points = data_points.into_iter().skip(offset).collect();
        } else {
            data_points.clear();
        }

        if let Some(l) = limit {
            if data_points.len() > l {
                data_points.truncate(l);
            }
        }

        let has_more = offset + data_points.len() < total_count as usize;

        Ok(QueryResult {
            data_points,
            total_count: Some(total_count),
            has_more,
        })
    }

    async fn delete_data_points(&mut self, filter: &QueryFilter) -> Result<u64> {
        if !self.connected || self.connection.is_none() {
            return Err(HisSrvError::ConnectionError("Not connected to Redis".to_string()));
        }

        let conn = self.connection.as_mut().unwrap();
        let mut deleted_count = 0u64;

        // Get keys matching pattern
        let key_pattern = if let Some(pattern) = &filter.key_pattern {
            format!("hissrv:data:{}", pattern)
        } else {
            "hissrv:data:*".to_string()
        };

        let keys: Vec<String> = conn.keys(key_pattern).await?;

        for key in keys {
            if key.ends_with(":latest") {
                continue;
            }

            if filter.start_time.is_some() || filter.end_time.is_some() {
                // Delete by score range
                let mut start_score = f64::NEG_INFINITY;
                let mut end_score = f64::INFINITY;

                if let Some(start_time) = filter.start_time {
                    start_score = start_time.timestamp() as f64;
                }
                if let Some(end_time) = filter.end_time {
                    end_score = end_time.timestamp() as f64;
                }

                let removed: u64 = redis::cmd("ZREMRANGEBYSCORE")
                    .arg(&key)
                    .arg(start_score)
                    .arg(end_score)
                    .query_async(conn)
                    .await?;
                deleted_count += removed;
            } else {
                // Delete entire key
                let removed: u64 = conn.del(&key).await?;
                if removed > 0 {
                    deleted_count += 1;
                    // Also delete latest key
                    let latest_key = format!("{}:latest", key);
                    let _: u64 = conn.del(&latest_key).await?;
                }
            }
        }

        Ok(deleted_count)
    }

    async fn get_keys(&self, pattern: Option<&str>) -> Result<Vec<String>> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError("Not connected to Redis".to_string()));
        }

        let key_pattern = if let Some(p) = pattern {
            format!("hissrv:data:{}", p)
        } else {
            "hissrv:data:*".to_string()
        };

        // Get keys using a new connection
        let keys: Vec<String> = {
            let mut temp_conn = self.client.as_ref().unwrap().get_async_connection().await?;
            temp_conn.keys(key_pattern).await?
        };
        
        // Remove prefix and filter out latest keys
        let clean_keys: Vec<String> = keys
            .into_iter()
            .filter(|k| !k.ends_with(":latest"))
            .map(|k| k.strip_prefix("hissrv:data:").unwrap_or(&k).to_string())
            .collect();

        Ok(clean_keys)
    }

    async fn get_statistics(&self) -> Result<StorageStats> {
        if !self.connected || self.connection.is_none() {
            return Ok(StorageStats {
                total_data_points: 0,
                storage_size_bytes: 0,
                last_write_time: self.last_write_time,
                last_read_time: self.last_read_time,
                connection_status: "disconnected".to_string(),
            });
        }

        // Get database size and key count
        let _info: String = {
            let mut temp_conn = self.client.as_ref().unwrap().get_async_connection().await?;
            redis::cmd("INFO")
                .arg("memory")
                .query_async(&mut temp_conn)
                .await
                .unwrap_or_default()
        };
        let _keyspace: String = {
            let mut temp_conn = self.client.as_ref().unwrap().get_async_connection().await?;
            redis::cmd("INFO")
                .arg("keyspace")
                .query_async(&mut temp_conn)
                .await
                .unwrap_or_default()
        };

        // Parse memory usage (simplified)
        let storage_size_bytes = 0u64; // TODO: Parse from info string
        let total_data_points = 0u64; // TODO: Count keys matching pattern

        Ok(StorageStats {
            total_data_points,
            storage_size_bytes,
            last_write_time: self.last_write_time,
            last_read_time: self.last_read_time,
            connection_status: "connected".to_string(),
        })
    }

    fn get_name(&self) -> &str {
        "redis"
    }

    fn get_config(&self) -> serde_json::Value {
        serde_json::to_value(&self.config).unwrap_or_default()
    }
}