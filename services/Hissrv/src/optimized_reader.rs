//! Optimized reader for new Redis data structure
//!
//! This module handles reading from the new separated config/realtime structure
//! and converting it to InfluxDB format.

use crate::error::{HisSrvError, Result};
use crate::influxdb_handler::InfluxDBConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};
use voltage_common::redis::RedisClient;

/// Optimized real-time value structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeValue {
    /// Raw value from device
    pub raw: f64,
    /// Engineering value after scaling
    pub value: f64,
    /// Timestamp in milliseconds
    pub ts: i64,
}

/// Point configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointConfig {
    pub name: String,
    pub unit: String,
    pub telemetry_type: String,
    pub description: String,
    pub scale: f64,
    pub offset: f64,
    pub address: String,
}

/// Optimized data reader
pub struct OptimizedReader {
    redis_client: RedisClient,
    config_cache: HashMap<u16, HashMap<String, PointConfig>>,
    cache_ttl_seconds: u64,
    last_cache_update: std::time::Instant,
}

impl OptimizedReader {
    /// Create new optimized reader
    pub fn new(redis_client: RedisClient) -> Self {
        Self {
            redis_client,
            config_cache: HashMap::new(),
            cache_ttl_seconds: 300, // 5 minutes
            last_cache_update: std::time::Instant::now(),
        }
    }

    /// Get channel configuration (with caching)
    pub async fn get_channel_config(
        &mut self,
        channel_id: u16,
    ) -> Result<HashMap<String, PointConfig>> {
        // Check cache validity
        if self.last_cache_update.elapsed().as_secs() > self.cache_ttl_seconds {
            self.config_cache.clear();
            self.last_cache_update = std::time::Instant::now();
        }

        // Return from cache if available
        if let Some(config) = self.config_cache.get(&channel_id) {
            return Ok(config.clone());
        }

        // Load from Redis
        let key = format!("comsrv:config:channel:{}:points", channel_id);
        let data: HashMap<String, String> =
            self.redis_client.hgetall(&key).await.map_err(|e| {
                HisSrvError::ConnectionError(format!("Failed to get config: {}", e))
            })?;

        let mut config = HashMap::new();
        for (point_id, json) in data {
            match serde_json::from_str::<PointConfig>(&json) {
                Ok(point_config) => {
                    config.insert(point_id, point_config);
                }
                Err(e) => {
                    warn!("Failed to parse config for point {}: {}", point_id, e);
                }
            }
        }

        // Update cache
        self.config_cache.insert(channel_id, config.clone());

        Ok(config)
    }

    /// Get channel realtime data
    pub async fn get_channel_realtime(
        &self,
        channel_id: u16,
    ) -> Result<HashMap<String, RealtimeValue>> {
        let key = format!("comsrv:realtime:channel:{}", channel_id);
        let data: HashMap<String, String> = self.redis_client.hgetall(&key).await.map_err(|e| {
            HisSrvError::ConnectionError(format!("Failed to get realtime data: {}", e))
        })?;

        let mut values = HashMap::new();
        for (point_id, json) in data {
            match serde_json::from_str::<RealtimeValue>(&json) {
                Ok(value) => {
                    values.insert(point_id, value);
                }
                Err(e) => {
                    warn!("Failed to parse value for point {}: {}", point_id, e);
                }
            }
        }

        Ok(values)
    }

    /// Process channel data for InfluxDB
    pub async fn process_channel_for_influxdb(
        &mut self,
        channel_id: u16,
        influxdb: &mut InfluxDBConnection,
        measurement: &str,
    ) -> Result<usize> {
        // Get configuration and realtime data
        let config = self.get_channel_config(channel_id).await?;
        let realtime = self.get_channel_realtime(channel_id).await?;

        if config.is_empty() || realtime.is_empty() {
            debug!("No data for channel {}", channel_id);
            return Ok(0);
        }

        let mut points_written = 0;

        // Process each point
        for (point_id, rt_value) in realtime {
            if let Some(point_config) = config.get(&point_id) {
                // Create InfluxDB point
                let tags = vec![
                    ("channel_id", channel_id.to_string()),
                    ("point_id", point_id.clone()),
                    ("point_name", point_config.name.clone()),
                    ("telemetry_type", point_config.telemetry_type.clone()),
                ];

                let fields = vec![
                    ("raw_value", rt_value.raw.to_string()),
                    ("value", rt_value.value.to_string()),
                    ("unit", point_config.unit.clone()),
                ];

                // Convert timestamp from milliseconds to nanoseconds
                let timestamp_ns = rt_value.ts * 1_000_000;

                // Write to InfluxDB
                match influxdb
                    .write_point(measurement, tags, fields, timestamp_ns)
                    .await
                {
                    Ok(_) => {
                        points_written += 1;
                    }
                    Err(e) => {
                        warn!("Failed to write point {} to InfluxDB: {}", point_id, e);
                    }
                }
            } else {
                warn!("No configuration found for point {}", point_id);
            }
        }

        info!(
            "Wrote {} points from channel {} to InfluxDB",
            points_written, channel_id
        );
        Ok(points_written)
    }

    /// Process multiple channels
    pub async fn process_channels(
        &mut self,
        channel_ids: &[u16],
        influxdb: &mut InfluxDBConnection,
        measurement: &str,
    ) -> Result<usize> {
        let mut total_written = 0;

        for &channel_id in channel_ids {
            match self
                .process_channel_for_influxdb(channel_id, influxdb, measurement)
                .await
            {
                Ok(count) => total_written += count,
                Err(e) => warn!("Failed to process channel {}: {}", channel_id, e),
            }
        }

        Ok(total_written)
    }

    /// Get module realtime data (for modsrv)
    pub async fn get_module_realtime(
        &self,
        module_id: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let key = format!("modsrv:realtime:module:{}", module_id);
        let data: HashMap<String, String> = self.redis_client.hgetall(&key).await.map_err(|e| {
            HisSrvError::ConnectionError(format!("Failed to get module data: {}", e))
        })?;

        let mut values = HashMap::new();
        for (point_id, json) in data {
            match serde_json::from_str::<serde_json::Value>(&json) {
                Ok(value) => {
                    values.insert(point_id, value);
                }
                Err(e) => {
                    warn!("Failed to parse module value for {}: {}", point_id, e);
                }
            }
        }

        Ok(values)
    }
}

/// Helper to merge config and realtime for backward compatibility
pub fn merge_for_compatibility(
    config: &HashMap<String, PointConfig>,
    realtime: &HashMap<String, RealtimeValue>,
) -> Vec<serde_json::Value> {
    let mut result = Vec::new();

    for (point_id, rt_value) in realtime {
        if let Some(cfg) = config.get(point_id) {
            let merged = serde_json::json!({
                "id": point_id,
                "name": cfg.name,
                "raw": rt_value.raw,
                "value": rt_value.value,
                "timestamp": chrono::DateTime::from_timestamp_millis(rt_value.ts)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
                "unit": cfg.unit,
                "telemetry_type": cfg.telemetry_type,
                "description": cfg.description,
            });
            result.push(merged);
        }
    }

    result
}
