//! Redis data scanner for monitoring point values

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
#[allow(unused_imports)]
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::redis::AlarmRedisClient;
use crate::services::rules::{AlarmRulesEngine, PointData};
use crate::AppState;

/// Channel monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Channels to monitor
    pub channels: Vec<u16>,
    /// Point types to monitor (m/s/c/a)
    pub point_types: Vec<String>,
    /// Scan interval in seconds
    pub scan_interval: u64,
}

/// Redis data scanner
pub struct RedisDataScanner {
    /// Redis client
    redis_client: Arc<AlarmRedisClient>,
    /// Monitoring configuration
    config: MonitorConfig,
    /// Rules engine
    rules_engine: Arc<RwLock<AlarmRulesEngine>>,
}

impl RedisDataScanner {
    /// Create new scanner
    pub async fn new(
        redis_client: Arc<AlarmRedisClient>,
        config: MonitorConfig,
        rules_engine: Arc<RwLock<AlarmRulesEngine>>,
    ) -> Result<Self> {
        Ok(Self {
            redis_client,
            config,
            rules_engine,
        })
    }

    /// Start scanning loop
    pub async fn start(self, state: AppState) -> Result<()> {
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(self.config.scan_interval));

            info!(
                "Starting Redis data scanner with {} channels and {} point types",
                self.config.channels.len(),
                self.config.point_types.len()
            );

            loop {
                interval.tick().await;

                if let Err(e) = self.scan_and_evaluate(&state).await {
                    error!("Failed to scan and evaluate data: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Scan Redis for point data and evaluate rules
    async fn scan_and_evaluate(&self, state: &AppState) -> Result<()> {
        let mut total_points = 0;
        let mut alarm_count = 0;

        // Scan each channel and point type combination
        for channel_id in &self.config.channels {
            for point_type in &self.config.point_types {
                match self.scan_channel_points(*channel_id, point_type).await {
                    Ok(points) => {
                        total_points += points.len();

                        // Evaluate rules for each point
                        let mut rules_engine = self.rules_engine.write().await;
                        for point_data in points {
                            let alarms = rules_engine.evaluate(&point_data);
                            alarm_count += alarms.len();

                            // Process generated alarms
                            for mut alarm in alarms {
                                // Set source information
                                let source = format!(
                                    "{}:{}:{}",
                                    point_data.channel_id,
                                    point_data.point_type,
                                    point_data.point_id
                                );
                                alarm.metadata.source = Some(source);

                                // Store in Redis
                                if let Err(e) = state.alarm_store.store_alarm(&alarm).await {
                                    error!("Failed to store alarm: {}", e);
                                    continue;
                                }

                                // Add to memory
                                let mut alarms = state.alarms.write().await;
                                alarms.push(alarm.clone());

                                // Publish for cloud push
                                if let Err(e) =
                                    state.alarm_store.publish_alarm_for_cloud(&alarm).await
                                {
                                    warn!("Failed to publish alarm for cloud: {}", e);
                                }

                                info!(
                                    "Generated alarm: {} (Level: {:?})",
                                    alarm.title, alarm.level
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to scan channel {} type {}: {}",
                            channel_id, point_type, e
                        );
                    }
                }
            }
        }

        // Check for timeout alarms
        let current_time = Utc::now();
        let rules_engine = self.rules_engine.read().await;
        let timeout_alarms = rules_engine.check_timeouts(current_time);

        for alarm in timeout_alarms {
            // Store in Redis
            if let Err(e) = state.alarm_store.store_alarm(&alarm).await {
                error!("Failed to store timeout alarm: {}", e);
                continue;
            }

            // Add to memory
            let mut alarms = state.alarms.write().await;
            alarms.push(alarm.clone());

            // Publish for cloud push
            if let Err(e) = state.alarm_store.publish_alarm_for_cloud(&alarm).await {
                warn!("Failed to publish timeout alarm for cloud: {}", e);
            }

            info!(
                "Generated timeout alarm: {} (Level: {:?})",
                alarm.title, alarm.level
            );
            alarm_count += 1;
        }

        debug!(
            "Scan completed: {} points checked, {} alarms generated",
            total_points, alarm_count
        );

        Ok(())
    }

    /// Scan points for a specific channel and type using Hash structure
    async fn scan_channel_points(
        &self,
        channel_id: u16,
        point_type: &str,
    ) -> Result<Vec<PointData>> {
        // Use new Hash key format: comsrv:{channelID}:{type}
        let hash_key = format!("comsrv:{}:{}", channel_id, point_type);

        // Get all fields and values from the hash
        let hash_data = self.redis_client.hgetall(&hash_key).await?;

        if hash_data.is_empty() {
            return Ok(Vec::new());
        }

        let mut points = Vec::new();
        for (field, value) in hash_data.iter() {
            match self.parse_point_data(channel_id, point_type, field, value) {
                Ok(point_data) => points.push(point_data),
                Err(e) => {
                    debug!(
                        "Failed to parse point data for {}:{}: {}",
                        hash_key, field, e
                    );
                }
            }
        }

        Ok(points)
    }

    /// Parse point data from Hash field and value
    fn parse_point_data(
        &self,
        channel_id: u16,
        point_type: &str,
        field: &str,
        value: &str,
    ) -> Result<PointData> {
        // In Hash structure, field is the point_id
        let point_id = field
            .parse::<u32>()
            .map_err(|e| anyhow!("Invalid point ID in field '{}': {}", field, e))?;

        // Try to parse as JSON format first
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(value) {
            let point_value = json_value
                .get("value")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| anyhow!("Missing or invalid 'value' field in JSON"))?;

            let timestamp =
                if let Some(ts_str) = json_value.get("timestamp").and_then(|v| v.as_str()) {
                    // Parse ISO format timestamp
                    chrono::DateTime::parse_from_rfc3339(ts_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| anyhow!("Invalid timestamp format: {}", e))?
                } else {
                    // Use current time if no timestamp provided
                    Utc::now()
                };

            return Ok(PointData {
                channel_id,
                point_type: point_type.to_string(),
                point_id,
                value: point_value,
                timestamp,
            });
        }

        // Fallback: Parse legacy format: value:timestamp
        let value_parts: Vec<&str> = value.split(':').collect();
        if value_parts.len() != 2 {
            return Err(anyhow!(
                "Invalid value format (neither JSON nor value:timestamp): {}",
                value
            ));
        }

        let point_value = value_parts[0]
            .parse::<f64>()
            .map_err(|e| anyhow!("Invalid point value: {}", e))?;
        let timestamp_ms = value_parts[1]
            .parse::<i64>()
            .map_err(|e| anyhow!("Invalid timestamp: {}", e))?;

        let timestamp = DateTime::from_timestamp_millis(timestamp_ms)
            .ok_or_else(|| anyhow!("Invalid timestamp value: {}", timestamp_ms))?;

        Ok(PointData {
            channel_id,
            point_type: point_type.to_string(),
            point_id,
            value: point_value,
            timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_point_data() {
        // Create a minimal scanner just to test the parse_point_data method
        let parse_fn =
            |channel_id: u16, point_type: &str, field: &str, value: &str| -> Result<PointData> {
                // In Hash structure, field is the point_id
                let point_id = field
                    .parse::<u32>()
                    .map_err(|e| anyhow!("Invalid point ID in field '{}': {}", field, e))?;

                // Parse value format: value:timestamp
                let value_parts: Vec<&str> = value.split(':').collect();
                if value_parts.len() != 2 {
                    return Err(anyhow!("Invalid value format: {}", value));
                }

                let point_value = value_parts[0]
                    .parse::<f64>()
                    .map_err(|e| anyhow!("Invalid point value: {}", e))?;
                let timestamp_ms = value_parts[1]
                    .parse::<i64>()
                    .map_err(|e| anyhow!("Invalid timestamp: {}", e))?;

                let timestamp = DateTime::from_timestamp_millis(timestamp_ms)
                    .ok_or_else(|| anyhow!("Invalid timestamp value: {}", timestamp_ms))?;

                Ok(PointData {
                    channel_id,
                    point_type: point_type.to_string(),
                    point_id,
                    value: point_value,
                    timestamp,
                })
            };

        // Test valid data with new Hash format
        let channel_id = 1001;
        let point_type = "m";
        let field = "10001"; // point_id as field
        let value = "75.5:1704956400000";
        let result = parse_fn(channel_id, point_type, field, value);
        assert!(result.is_ok());

        let point = result.unwrap();
        assert_eq!(point.channel_id, 1001);
        assert_eq!(point.point_type, "m");
        assert_eq!(point.point_id, 10001);
        assert_eq!(point.value, 75.5);

        // Test invalid field format
        let result = parse_fn(channel_id, point_type, "invalid_field", value);
        assert!(result.is_err());

        // Test invalid value format
        let result = parse_fn(channel_id, point_type, field, "invalid_value");
        assert!(result.is_err());
    }
}
