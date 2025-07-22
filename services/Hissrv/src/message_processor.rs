use crate::batch_writer::{BatchWriteBuffer, BatchWriterConfig};
use crate::config::Config;
use crate::error::{HisSrvError, Result};
use crate::storage::influxdb_storage::{InfluxDBBatchWriter, InfluxDBStorage};
use crate::storage::{DataPoint, DataValue, Storage};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use voltage_libs::redis::RedisClient;

/// Message processor with batch writing support
pub struct MessageProcessor {
    config: Arc<Config>,
    redis_client: Arc<RedisClient>,
    batch_buffer: Option<Arc<BatchWriteBuffer<InfluxDBBatchWriter>>>,
    channel_cache: Arc<Mutex<HashMap<String, ChannelInfo>>>,
}

#[derive(Clone, Debug)]
struct ChannelInfo {
    channel_id: u16,
    protocol_type: String,
    last_update: chrono::DateTime<Utc>,
}

impl MessageProcessor {
    pub async fn new(config: Config, redis_client: RedisClient) -> Result<Self> {
        let config = Arc::new(config);
        let redis_client = Arc::new(redis_client);

        Ok(Self {
            config,
            redis_client,
            batch_buffer: None,
            channel_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Initialize storage with batch writer
    pub async fn init_storage(&mut self, storage: Arc<Mutex<InfluxDBStorage>>) -> Result<()> {
        // Create batch writer configuration
        let batch_config = BatchWriterConfig {
            max_batch_size: self.config.storage.backends.influxdb.batch_size as usize,
            flush_interval_secs: self.config.storage.backends.influxdb.flush_interval,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_wal: true,
            wal_path: format!("./data/{}/wal", self.config.service.name),
        };

        // Create batch writer
        let batch_writer = InfluxDBBatchWriter::new(storage);
        let batch_buffer = Arc::new(BatchWriteBuffer::new(batch_writer, batch_config)?);

        // Recover from WAL if exists
        batch_buffer.recover().await?;

        // Start the flush task
        let buffer_clone = batch_buffer.clone();
        tokio::spawn(async move {
            buffer_clone.start_flush_task().await;
        });

        self.batch_buffer = Some(batch_buffer);

        info!(
            "Initialized batch writer with buffer size: {}, flush interval: {}s",
            self.config.storage.backends.influxdb.batch_size,
            self.config.storage.backends.influxdb.flush_interval
        );

        Ok(())
    }

    /// Start processing messages from Redis
    pub async fn start(&self) -> Result<()> {
        let channels = self.get_subscription_channels();

        if channels.is_empty() {
            return Err(HisSrvError::ConfigError(
                "No channels configured for subscription".to_string(),
            ));
        }

        info!("Subscribing to channels: {:?}", channels);

        // Create subscriber
        let mut subscriber = self.redis_client.subscribe(&channels).await?;

        // Process messages
        loop {
            match subscriber.next_message().await {
                Ok((channel, message)) => {
                    if let Err(e) = self.process_message(&channel, &message).await {
                        error!("Error processing message from channel {}: {}", channel, e);
                    }
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    // Reconnect after error
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Process a single message
    async fn process_message(&self, channel: &str, message: &str) -> Result<()> {
        debug!("Received message from channel {}: {}", channel, message);

        // Parse the channel to extract information
        let channel_info = self.parse_channel(channel)?;

        // Parse the message as JSON
        let data: serde_json::Value = serde_json::from_str(message)
            .map_err(|e| HisSrvError::ParseError(format!("Invalid JSON: {}", e)))?;

        // Convert to data points based on channel type
        let data_points = match channel_info.protocol_type.as_str() {
            "telemetry" | "measurement" => self.parse_telemetry_data(&channel_info, &data).await?,
            "signal" | "status" => self.parse_signal_data(&channel_info, &data).await?,
            "event" => self.parse_event_data(&channel_info, &data).await?,
            _ => {
                warn!("Unknown channel type: {}", channel_info.protocol_type);
                Vec::new()
            }
        };

        // Add to batch buffer
        if let Some(buffer) = &self.batch_buffer {
            for point in data_points {
                buffer.add(point).await?;
            }
        }

        Ok(())
    }

    /// Parse channel name to extract information
    fn parse_channel(&self, channel: &str) -> Result<ChannelInfo> {
        // Expected format: "channel:{id}:{type}" or "{id}:{type}:*"
        let parts: Vec<&str> = channel.split(':').collect();

        if parts.len() < 2 {
            return Err(HisSrvError::ParseError(format!(
                "Invalid channel format: {}",
                channel
            )));
        }

        let channel_id = if parts[0] == "channel" && parts.len() >= 3 {
            parts[1]
                .parse::<u16>()
                .map_err(|_| HisSrvError::ParseError(format!("Invalid channel ID: {}", parts[1])))?
        } else {
            parts[0]
                .parse::<u16>()
                .map_err(|_| HisSrvError::ParseError(format!("Invalid channel ID: {}", parts[0])))?
        };

        let protocol_type = if parts.len() >= 3 {
            parts[2].to_string()
        } else {
            parts[1].to_string()
        };

        Ok(ChannelInfo {
            channel_id,
            protocol_type,
            last_update: Utc::now(),
        })
    }

    /// Parse telemetry/measurement data
    async fn parse_telemetry_data(
        &self,
        channel_info: &ChannelInfo,
        data: &serde_json::Value,
    ) -> Result<Vec<DataPoint>> {
        let mut points = Vec::new();

        // Handle both single value and batch formats
        if let Some(obj) = data.as_object() {
            // Check if it's a batch update
            if let Some(values) = obj.get("values").and_then(|v| v.as_object()) {
                // Batch format: {"values": {"point_id": value, ...}, "timestamp": ...}
                let timestamp = obj
                    .get("timestamp")
                    .and_then(|t| t.as_i64())
                    .map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap_or_else(Utc::now))
                    .unwrap_or_else(Utc::now);

                for (point_id, value) in values {
                    let key = format!("{}:m:{}", channel_info.channel_id, point_id);

                    let data_value = self.parse_value(value)?;

                    points.push(DataPoint {
                        key,
                        value: data_value,
                        timestamp,
                        tags: HashMap::from([
                            (
                                "channel_id".to_string(),
                                channel_info.channel_id.to_string(),
                            ),
                            ("point_type".to_string(), "measurement".to_string()),
                            ("point_id".to_string(), point_id.clone()),
                        ]),
                        metadata: HashMap::new(),
                    });
                }
            } else {
                // Single value format: {"point_id": "xxx", "value": yyy, "timestamp": ...}
                if let (Some(point_id), Some(value)) = (
                    obj.get("point_id").and_then(|p| p.as_str()),
                    obj.get("value"),
                ) {
                    let key = format!("{}:m:{}", channel_info.channel_id, point_id);
                    let timestamp = obj
                        .get("timestamp")
                        .and_then(|t| t.as_i64())
                        .map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap_or_else(Utc::now))
                        .unwrap_or_else(Utc::now);

                    let data_value = self.parse_value(value)?;

                    points.push(DataPoint {
                        key,
                        value: data_value,
                        timestamp,
                        tags: HashMap::from([
                            (
                                "channel_id".to_string(),
                                channel_info.channel_id.to_string(),
                            ),
                            ("point_type".to_string(), "measurement".to_string()),
                            ("point_id".to_string(), point_id.to_string()),
                        ]),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Ok(points)
    }

    /// Parse signal/status data
    async fn parse_signal_data(
        &self,
        channel_info: &ChannelInfo,
        data: &serde_json::Value,
    ) -> Result<Vec<DataPoint>> {
        let mut points = Vec::new();

        if let Some(obj) = data.as_object() {
            // Similar to telemetry but with 's' prefix for signals
            if let Some(values) = obj.get("values").and_then(|v| v.as_object()) {
                let timestamp = obj
                    .get("timestamp")
                    .and_then(|t| t.as_i64())
                    .map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap_or_else(Utc::now))
                    .unwrap_or_else(Utc::now);

                for (point_id, value) in values {
                    let key = format!("{}:s:{}", channel_info.channel_id, point_id);

                    let data_value = if let Some(bool_val) = value.as_bool() {
                        DataValue::Boolean(bool_val)
                    } else if let Some(int_val) = value.as_i64() {
                        DataValue::Boolean(int_val != 0)
                    } else {
                        DataValue::Boolean(false)
                    };

                    points.push(DataPoint {
                        key,
                        value: data_value,
                        timestamp,
                        tags: HashMap::from([
                            (
                                "channel_id".to_string(),
                                channel_info.channel_id.to_string(),
                            ),
                            ("point_type".to_string(), "signal".to_string()),
                            ("point_id".to_string(), point_id.clone()),
                        ]),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Ok(points)
    }

    /// Parse event data
    async fn parse_event_data(
        &self,
        channel_info: &ChannelInfo,
        data: &serde_json::Value,
    ) -> Result<Vec<DataPoint>> {
        let mut points = Vec::new();

        if let Some(obj) = data.as_object() {
            let event_type = obj
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");

            let key = format!("{}:event:{}", channel_info.channel_id, event_type);
            let timestamp = obj
                .get("timestamp")
                .and_then(|t| t.as_i64())
                .map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap_or_else(Utc::now))
                .unwrap_or_else(Utc::now);

            // Store the entire event as JSON
            let data_value = DataValue::Json(data.clone());

            points.push(DataPoint {
                key,
                value: data_value,
                timestamp,
                tags: HashMap::from([
                    (
                        "channel_id".to_string(),
                        channel_info.channel_id.to_string(),
                    ),
                    ("point_type".to_string(), "event".to_string()),
                    ("event_type".to_string(), event_type.to_string()),
                ]),
                metadata: HashMap::new(),
            });
        }

        Ok(points)
    }

    /// Parse JSON value to DataValue
    fn parse_value(&self, value: &serde_json::Value) -> Result<DataValue> {
        match value {
            serde_json::Value::String(s) => {
                // Try to parse as number first
                if let Ok(f) = s.parse::<f64>() {
                    Ok(DataValue::Float(f))
                } else {
                    Ok(DataValue::String(s.clone()))
                }
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(DataValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(DataValue::Float(f))
                } else {
                    Ok(DataValue::Float(0.0))
                }
            }
            serde_json::Value::Bool(b) => Ok(DataValue::Boolean(*b)),
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                Ok(DataValue::Json(value.clone()))
            }
            serde_json::Value::Null => Ok(DataValue::String("null".to_string())),
        }
    }

    /// Get subscription channels from configuration
    fn get_subscription_channels(&self) -> Vec<String> {
        let mut channels = Vec::new();

        // Add configured channels
        for channel_pattern in &self.config.redis.subscription.channels {
            channels.push(channel_pattern.clone());
        }

        // If no channels configured, use default patterns based on new architecture
        if channels.is_empty() {
            // Subscribe to all channel data updates
            channels.push("channel:*:telemetry".to_string());
            channels.push("channel:*:signal".to_string());
            channels.push("channel:*:event".to_string());

            // Also subscribe to flattened key patterns
            channels.push("*:m:*".to_string()); // Measurements
            channels.push("*:s:*".to_string()); // Signals
            channels.push("*:c:*".to_string()); // Controls
            channels.push("*:a:*".to_string()); // Adjustments
        }

        channels
    }

    /// Get statistics
    pub async fn get_stats(&self) -> Option<crate::batch_writer::BatchWriteStats> {
        if let Some(buffer) = &self.batch_buffer {
            Some(buffer.get_stats().await)
        } else {
            None
        }
    }

    /// Shutdown the processor
    pub async fn shutdown(&self) -> Result<()> {
        if let Some(buffer) = &self.batch_buffer {
            buffer.shutdown().await?;
        }
        Ok(())
    }
}
