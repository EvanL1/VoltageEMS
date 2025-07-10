use crate::config::RedisConnectionConfig;
use crate::error::{HisSrvError, Result};
use crate::storage::{DataPoint, QueryFilter, QueryResult, Storage, StorageStats};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json;
use voltage_common::redis::{RedisClient, RedisConfig};

pub struct RedisStorage {
    client: Option<RedisClient>,
    config: RedisConnectionConfig,
    connected: bool,
    last_write_time: Option<DateTime<Utc>>,
    last_read_time: Option<DateTime<Utc>>,
}

impl RedisStorage {
    pub fn new(config: RedisConnectionConfig) -> Self {
        Self {
            client: None,
            config,
            connected: false,
            last_write_time: None,
            last_read_time: None,
        }
    }

    fn data_point_to_key(&self, data_point: &DataPoint) -> String {
        format!("hissrv:data:{}", data_point.key)
    }

    fn data_point_to_json(&self, data_point: &DataPoint) -> Result<String> {
        serde_json::to_string(data_point).map_err(|e| {
            HisSrvError::SerializationError(format!("Failed to serialize data point: {}", e))
        })
    }

    fn json_to_data_point(&self, json: &str) -> Result<DataPoint> {
        serde_json::from_str(json).map_err(|e| {
            HisSrvError::SerializationError(format!("Failed to deserialize data point: {}", e))
        })
    }
}

#[async_trait]
impl Storage for RedisStorage {
    async fn connect(&mut self) -> Result<()> {
        let redis_config = if !self.config.socket.is_empty() {
            RedisConfig {
                host: String::new(),
                port: 0,
                password: if self.config.password.is_empty() {
                    None
                } else {
                    Some(self.config.password.clone())
                },
                socket: Some(self.config.socket.clone()),
                database: self.config.database,
                connection_timeout: 10,
                max_retries: 3,
            }
        } else {
            RedisConfig {
                host: self.config.host.clone(),
                port: self.config.port,
                password: if self.config.password.is_empty() {
                    None
                } else {
                    Some(self.config.password.clone())
                },
                socket: None,
                database: self.config.database,
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

        let redis_address = if !self.config.socket.is_empty() {
            self.config.socket.clone()
        } else {
            format!("{}:{}", self.config.host, self.config.port)
        };
        tracing::info!("Successfully connected to Redis at {}", redis_address);

        self.client = Some(client);
        self.connected = true;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        self.connected = false;
        tracing::info!("Disconnected from Redis");
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.connected
    }

    async fn store_data_point(&mut self, data_point: &DataPoint) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let key = self.data_point_to_key(data_point);
        let json = self.data_point_to_json(data_point)?;

        client
            .set(&key, &json)
            .await
            .map_err(|e| HisSrvError::RedisError(format!("Failed to write data point: {}", e)))?;

        // TODO: Add TTL support if needed

        self.last_write_time = Some(Utc::now());
        Ok(())
    }

    async fn store_data_points(&mut self, data_points: &[DataPoint]) -> Result<()> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        // Write each data point individually
        // TODO: Consider using pipeline for better performance
        for data_point in data_points {
            self.store_data_point(data_point).await?;
        }

        Ok(())
    }

    async fn query_data_points(&self, filter: &QueryFilter) -> Result<QueryResult> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let pattern = filter
            .key_pattern
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("hissrv:data:*");

        // Get matching keys
        let keys = client
            .keys(&pattern)
            .await
            .map_err(|e| HisSrvError::RedisError(format!("Failed to query keys: {}", e)))?;

        let mut data_points = Vec::new();

        // Fetch each data point
        for key in keys {
            match client.get(&key).await {
                Ok(Some(json)) => {
                    match self.json_to_data_point(&json) {
                        Ok(data_point) => {
                            // Apply time range filter if specified
                            let include = match (&filter.start_time, &filter.end_time) {
                                (Some(start), Some(end)) => {
                                    data_point.timestamp >= *start && data_point.timestamp <= *end
                                }
                                (Some(start), None) => data_point.timestamp >= *start,
                                (None, Some(end)) => data_point.timestamp <= *end,
                                (None, None) => true,
                            };

                            if include {
                                data_points.push(data_point);
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to deserialize data point from key {}: {}",
                                key,
                                e
                            );
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!("Key {} not found", key);
                }
                Err(e) => {
                    tracing::warn!("Failed to get value for key {}: {}", key, e);
                }
            }
        }

        // Note: Can't update last_read_time in &self method

        let total_count = data_points.len() as u64;
        Ok(QueryResult {
            data_points,
            total_count: Some(total_count),
            has_more: false,
        })
    }

    async fn delete_data_points(&mut self, filter: &QueryFilter) -> Result<u64> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let pattern = filter
            .key_pattern
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("hissrv:data:*");

        // Get matching keys
        let keys = client.keys(pattern).await.map_err(|e| {
            HisSrvError::RedisError(format!("Failed to query keys for deletion: {}", e))
        })?;

        let count = keys.len() as u64;

        if count > 0 {
            // Delete keys
            let keys_vec: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
            client
                .del(&keys_vec)
                .await
                .map_err(|e| HisSrvError::RedisError(format!("Failed to delete keys: {}", e)))?;
        }

        Ok(count)
    }

    async fn get_keys(&self, pattern: Option<&str>) -> Result<Vec<String>> {
        if !self.connected || self.client.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let client = self.client.as_ref().unwrap();
        let search_pattern = pattern.unwrap_or("hissrv:data:*");

        client
            .keys(search_pattern)
            .await
            .map_err(|e| HisSrvError::RedisError(format!("Failed to get keys: {}", e)))
    }

    async fn get_statistics(&self) -> Result<StorageStats> {
        Ok(StorageStats {
            total_data_points: 0,  // Would need to count keys
            storage_size_bytes: 0, // Not easily available in Redis
            last_write_time: self.last_write_time,
            last_read_time: self.last_read_time,
            connection_status: if self.connected {
                "connected".to_string()
            } else {
                "disconnected".to_string()
            },
        })
    }

    fn get_name(&self) -> &str {
        "redis"
    }

    fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "host": self.config.host,
            "port": self.config.port,
            "database": self.config.database,
            "socket": self.config.socket,
        })
    }
}
