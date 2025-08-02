//! Redis Functions-based poller for efficient data collection and Line Protocol conversion

use crate::config::{Config, TagRule};
use crate::Result;
use hissrv::anyhow;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, warn};
use voltage_libs::influxdb::InfluxClient;
use voltage_libs::redis::RedisClient;

/// Data poller using Redis Functions for optimal performance
pub struct Poller {
    redis: RedisClient,
    influx: InfluxClient,
    config: Arc<RwLock<Config>>,
    batch_id_counter: u64,
    config_update_rx: Option<tokio::sync::mpsc::Receiver<()>>,
}

impl Poller {
    /// Create new poller
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        // Read configuration
        let (redis_url, influx_url, influx_org, influx_bucket, influx_token) = {
            let cfg = config
                .read()
                .map_err(|_| anyhow!("Failed to read config"))?;
            (
                cfg.redis.url.clone(),
                cfg.influxdb.url.clone(),
                cfg.influxdb.org.clone(),
                cfg.influxdb.bucket.clone(),
                cfg.influxdb.token.clone(),
            )
        };

        // Create Redis client with Functions support
        let redis = RedisClient::new(&redis_url)
            .await
            .map_err(|e| anyhow!("Failed to create Redis client: {}", e))?;

        info!("Redis Functions client established");

        // Create InfluxDB client
        let influx = InfluxClient::new(&influx_url, &influx_org, &influx_bucket, &influx_token)?;

        // Test InfluxDB connection
        influx
            .ping()
            .await
            .map_err(|e| anyhow!("InfluxDB ping failed: {}", e))?;
        info!("InfluxDB connection established");

        Ok(Self {
            redis,
            influx,
            config,
            batch_id_counter: 0,
            config_update_rx: None,
        })
    }

    /// Create poller with update channel
    pub async fn with_update_channel(
        config: Arc<RwLock<Config>>,
        rx: tokio::sync::mpsc::Receiver<()>,
    ) -> Result<Self> {
        let mut poller = Self::new(config).await?;
        poller.config_update_rx = Some(rx);
        Ok(poller)
    }

    /// Run main polling loop
    pub async fn run(mut self) -> Result<()> {
        let polling_interval = {
            let cfg = self
                .config
                .read()
                .map_err(|_| anyhow!("Failed to read config"))?;
            cfg.service.polling_interval
        };

        let mut interval = tokio::time::interval(polling_interval);
        info!("Starting polling with interval: {:?}", polling_interval);

        // Configure initial mappings
        self.configure_mappings().await?;

        loop {
            // Check for config updates
            if let Some(rx) = &mut self.config_update_rx {
                match rx.try_recv() {
                    Ok(()) => {
                        info!("Received configuration update notification");
                        if let Err(e) = self.reload_config().await {
                            error!("Failed to reload configuration: {}", e);
                        }
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // No updates, continue
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        warn!("Configuration update channel disconnected");
                        self.config_update_rx = None;
                    }
                }
            }

            interval.tick().await;

            // Process data using Redis Functions
            if let Err(e) = self.process_batch().await {
                error!("Failed to process batch: {}", e);
            }
        }
    }

    /// Process a batch of data using Redis Functions
    async fn process_batch(&mut self) -> Result<()> {
        self.batch_id_counter += 1;
        let batch_id = format!("batch_{}", self.batch_id_counter);

        // Get batch configuration
        let max_lines = {
            let cfg = self
                .config
                .read()
                .map_err(|_| anyhow!("Failed to read config"))?;
            cfg.influxdb.batch_size
        };

        debug!("Creating batch {} with max {} lines", batch_id, max_lines);

        // Use Redis Function to create and convert batch
        let batch_result: String = self
            .redis
            .fcall("hissrv_get_batch", &[&batch_id], &[&max_lines.to_string()])
            .await
            .map_err(|e| anyhow!("Failed to get batch: {}", e))?;

        let batch_data: Value = serde_json::from_str(&batch_result)
            .map_err(|e| anyhow!("Failed to parse batch result: {}", e))?;

        let line_count = batch_data
            .get("line_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        if line_count > 0 {
            info!("Retrieved batch {} with {} lines", batch_id, line_count);

            // Write batch to InfluxDB
            self.write_batch_to_influx(&batch_id).await?;

            // Acknowledge batch completion
            let _ack_result: String = self
                .redis
                .fcall("hissrv_ack_batch", &[&batch_id], &["written"])
                .await
                .map_err(|e| anyhow!("Failed to acknowledge batch: {}", e))?;

            debug!("Batch {} written and acknowledged", batch_id);
        }

        Ok(())
    }

    /// Write batch data to InfluxDB
    async fn write_batch_to_influx(&mut self, batch_id: &str) -> Result<()> {
        // For now, get the line protocol data directly from Redis Functions
        // The Redis Function should have prepared the line protocol already
        let line_protocol: String = self
            .redis
            .fcall("hissrv_get_batch_lines", &[batch_id], &[])
            .await
            .map_err(|e| anyhow!("Failed to get batch line protocol: {}", e))?;

        if !line_protocol.is_empty() {
            let line_count = line_protocol.lines().count();

            self.influx
                .write_line_protocol(&line_protocol)
                .await
                .map_err(|e| anyhow!("Failed to write to InfluxDB: {}", e))?;

            info!("Written {} lines to InfluxDB", line_count);
        }

        Ok(())
    }

    /// Configure data mappings in Redis Functions
    async fn configure_mappings(&mut self) -> Result<()> {
        let mappings = {
            let cfg = self
                .config
                .read()
                .map_err(|_| anyhow!("Failed to read config"))?;
            cfg.mappings.clone()
        };

        for (idx, mapping) in mappings.iter().enumerate() {
            let mapping_id = format!("mapping_{}", idx);
            let config = json!({
                "source_pattern": mapping.source,
                "measurement": mapping.measurement,
                "tags": self.convert_tag_rules(&mapping.tags),
                "field_mappings": self.convert_field_mappings(&mapping.fields),
                "enabled": true
            });

            let _config_result: String = self
                .redis
                .fcall(
                    "hissrv_configure_mapping",
                    &[&mapping_id],
                    &[&config.to_string()],
                )
                .await
                .map_err(|e| anyhow!("Failed to configure mapping {}: {}", mapping_id, e))?;

            debug!("Configured mapping: {}", mapping_id);
        }

        Ok(())
    }

    /// Convert tag rules to map format
    fn convert_tag_rules(&self, rules: &[TagRule]) -> HashMap<String, String> {
        let mut tags = HashMap::new();
        for rule in rules {
            match rule {
                TagRule::Static { value } => {
                    // Handle static tag values like "key=value"
                    if let Some((k, v)) = value.split_once('=') {
                        tags.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
                TagRule::Extract { field } => {
                    // Mark fields for extraction from key
                    tags.insert(format!("__extract_{}", field), "true".to_string());
                }
            }
        }
        tags
    }

    /// Convert field mappings to format expected by Redis Functions
    fn convert_field_mappings(
        &self,
        fields: &[crate::config::FieldMapping],
    ) -> HashMap<String, String> {
        let mut mappings = HashMap::new();
        for field in fields {
            mappings.insert(field.name.clone(), field.field_type.clone());
        }
        mappings
    }

    /// Reload configuration
    async fn reload_config(&mut self) -> Result<()> {
        info!("Reloading configuration...");

        // Reconfigure mappings
        self.configure_mappings().await?;

        info!("Configuration reloaded successfully");
        Ok(())
    }

    /// Get statistics from Redis Functions
    #[allow(dead_code)]
    pub async fn get_stats(&mut self) -> Result<Value> {
        let stats_json: String = self
            .redis
            .fcall("hissrv_get_stats", &[], &[])
            .await
            .map_err(|e| anyhow!("Failed to get stats: {}", e))?;

        serde_json::from_str(&stats_json).map_err(|e| anyhow!("Failed to parse stats: {}", e))
    }
}

/// Migration utilities for comparing old vs new approach
pub mod migration {
    use super::*;

    /// Compare old vs new approach performance
    #[allow(dead_code)]
    pub async fn benchmark_comparison(config: Arc<RwLock<Config>>) -> Result<()> {
        info!("Starting benchmark comparison...");

        // Create test data
        let redis_url = {
            let cfg = config.read().unwrap();
            cfg.redis.url.clone()
        };

        let mut redis = RedisClient::new(&redis_url).await?;

        // Create 1000 test data points
        for i in 0..1000 {
            let key = format!("comsrv:{}:T", 1000 + i);
            for j in 0..10 {
                let field = format!("{}", 10000 + j);
                let value = format!("{:.2}", 20.0 + (i as f64) * 0.1);
                redis.hset(&key, &field, value).await?;
            }
        }

        // Benchmark new approach (Redis Functions)
        let new_start = std::time::Instant::now();
        let mut poller = Poller::new(config).await?;
        poller.process_batch().await?;
        let new_duration = new_start.elapsed();

        info!("Redis Functions approach: {:?}", new_duration);

        Ok(())
    }
}
