use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::stream;
use influxdb2::api::write::Point;
use influxdb2::Client;
use log::{debug, error, info, warn};
use tokio::sync::mpsc;
use tokio::time;

use crate::config::{Config, DataMappingConfig, InfluxDBConfig};
use crate::error::{HissrvError, Result};
use crate::redis_handler::RedisDataPoint;

/// InfluxDB handler
pub struct InfluxDBHandler {
    /// InfluxDB configuration
    config: InfluxDBConfig,
    /// Data mapping configuration
    mapping: DataMappingConfig,
    /// InfluxDB client
    client: Option<Client>,
    /// Data points buffer
    buffer: Arc<Mutex<Vec<Point>>>,
}

impl InfluxDBHandler {
    /// Create a new InfluxDB handler
    pub fn new(config: InfluxDBConfig, mapping: DataMappingConfig) -> Self {
        Self {
            config,
            mapping,
            client: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a new InfluxDB handler from config
    pub fn from_config(config: &Config) -> Self {
        Self::new(config.influxdb.clone(), config.data_mapping.clone())
    }

    /// Connect to InfluxDB
    pub fn connect(&mut self) -> Result<()> {
        let client = Client::new(&self.config.url, &self.config.org, &self.config.token);
        self.client = Some(client);
        
        // Test connection by getting health status
        let client = self.client.as_ref().unwrap();
        let health = client.health();
        
        match tokio::runtime::Runtime::new()
            .map_err(|e| HissrvError::IOError(e.to_string()))?
            .block_on(health)
        {
            Ok(health) => {
                if health.status == "pass" {
                    info!("Connected to InfluxDB: {}", self.config.url);
                    Ok(())
                } else {
                    Err(HissrvError::InfluxDBError(format!(
                        "InfluxDB health check failed: {}",
                        health.status
                    )))
                }
            }
            Err(e) => Err(HissrvError::InfluxDBError(format!(
                "Failed to connect to InfluxDB: {}",
                e
            ))),
        }
    }

    /// Flush data to InfluxDB
    pub fn flush(&self) -> Result<()> {
        let client = match &self.client {
            Some(client) => client,
            None => return Err(HissrvError::InfluxDBError("Not connected to InfluxDB".into())),
        };

        let mut buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(e) => {
                return Err(HissrvError::InfluxDBError(format!(
                    "Failed to lock buffer: {}",
                    e
                )))
            }
        };

        if buffer.is_empty() {
            debug!("No data to flush");
            return Ok(());
        }

        let points = std::mem::take(&mut *buffer);
        let bucket = self.config.bucket.clone();
        
        debug!("Flushing {} data points to InfluxDB", points.len());

        // Write data to InfluxDB
        let write_api = client.write_api();
        
        match tokio::runtime::Runtime::new()
            .map_err(|e| HissrvError::IOError(e.to_string()))?
            .block_on(write_api.write(&bucket, stream::iter(points)))
        {
            Ok(_) => {
                info!("Successfully wrote {} data points to InfluxDB", points.len());
                Ok(())
            }
            Err(e) => Err(HissrvError::InfluxDBError(format!(
                "Failed to write data to InfluxDB: {}",
                e
            ))),
        }
    }

    /// Process a Redis data point
    pub fn process_data_point(&self, data_point: &RedisDataPoint) -> Result<()> {
        use crate::redis_handler::RedisData;
        
        match &data_point.data {
            RedisData::String(value) => {
                self.process_string(&data_point.key, value, data_point.timestamp)
            }
            RedisData::Integer(value) => {
                self.process_integer(&data_point.key, *value, data_point.timestamp)
            }
            RedisData::Float(value) => {
                self.process_float(&data_point.key, *value, data_point.timestamp)
            }
            RedisData::Boolean(value) => {
                self.process_boolean(&data_point.key, *value, data_point.timestamp)
            }
            RedisData::Hash(value) => {
                self.process_hash(&data_point.key, value, data_point.timestamp)
            }
            RedisData::None => {
                warn!("Skipping empty data for key: {}", data_point.key);
                Ok(())
            }
        }
    }

    /// Process a string value
    fn process_string(&self, key: &str, value: &str, timestamp: DateTime<Utc>) -> Result<()> {
        let mut point = Point::new(self.get_measurement_name(key))
            .timestamp(timestamp.timestamp_nanos());

        // Add tags
        for tag_mapping in &self.mapping.tag_mappings {
            if key.contains(&tag_mapping.redis_source) {
                point = point.tag(tag_mapping.influx_tag.clone(), tag_mapping.redis_source.clone());
            }
        }

        // Add field
        for field_mapping in &self.mapping.field_mappings {
            if key.contains(&field_mapping.redis_source) {
                point = point.field(field_mapping.influx_field.clone(), value.to_string());
                break;
            }
        }

        // Add to buffer
        let mut buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(e) => {
                return Err(HissrvError::InfluxDBError(format!(
                    "Failed to lock buffer: {}",
                    e
                )))
            }
        };

        buffer.push(point);

        // Check if buffer is full
        if buffer.len() >= self.config.batch_size {
            drop(buffer); // Release the lock before flushing
            self.flush()?;
        }

        Ok(())
    }

    /// Process an integer value
    fn process_integer(&self, key: &str, value: i64, timestamp: DateTime<Utc>) -> Result<()> {
        let mut point = Point::new(self.get_measurement_name(key))
            .timestamp(timestamp.timestamp_nanos());

        // Add tags
        for tag_mapping in &self.mapping.tag_mappings {
            if key.contains(&tag_mapping.redis_source) {
                point = point.tag(tag_mapping.influx_tag.clone(), tag_mapping.redis_source.clone());
            }
        }

        // Add field
        for field_mapping in &self.mapping.field_mappings {
            if key.contains(&field_mapping.redis_source) {
                let scaled_value = (value as f64 * field_mapping.scale_factor) as i64;
                point = point.field(field_mapping.influx_field.clone(), scaled_value);
                break;
            }
        }

        // Add to buffer
        let mut buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(e) => {
                return Err(HissrvError::InfluxDBError(format!(
                    "Failed to lock buffer: {}",
                    e
                )))
            }
        };

        buffer.push(point);

        // Check if buffer is full
        if buffer.len() >= self.config.batch_size {
            drop(buffer); // Release the lock before flushing
            self.flush()?;
        }

        Ok(())
    }

    /// Process a float value
    fn process_float(&self, key: &str, value: f64, timestamp: DateTime<Utc>) -> Result<()> {
        let mut point = Point::new(self.get_measurement_name(key))
            .timestamp(timestamp.timestamp_nanos());

        // Add tags
        for tag_mapping in &self.mapping.tag_mappings {
            if key.contains(&tag_mapping.redis_source) {
                point = point.tag(tag_mapping.influx_tag.clone(), tag_mapping.redis_source.clone());
            }
        }

        // Add field
        for field_mapping in &self.mapping.field_mappings {
            if key.contains(&field_mapping.redis_source) {
                let scaled_value = value * field_mapping.scale_factor;
                point = point.field(field_mapping.influx_field.clone(), scaled_value);
                break;
            }
        }

        // Add to buffer
        let mut buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(e) => {
                return Err(HissrvError::InfluxDBError(format!(
                    "Failed to lock buffer: {}",
                    e
                )))
            }
        };

        buffer.push(point);

        // Check if buffer is full
        if buffer.len() >= self.config.batch_size {
            drop(buffer); // Release the lock before flushing
            self.flush()?;
        }

        Ok(())
    }

    /// Process a boolean value
    fn process_boolean(&self, key: &str, value: bool, timestamp: DateTime<Utc>) -> Result<()> {
        let mut point = Point::new(self.get_measurement_name(key))
            .timestamp(timestamp.timestamp_nanos());

        // Add tags
        for tag_mapping in &self.mapping.tag_mappings {
            if key.contains(&tag_mapping.redis_source) {
                point = point.tag(tag_mapping.influx_tag.clone(), tag_mapping.redis_source.clone());
            }
        }

        // Add field
        for field_mapping in &self.mapping.field_mappings {
            if key.contains(&field_mapping.redis_source) {
                point = point.field(field_mapping.influx_field.clone(), value);
                break;
            }
        }

        // Add to buffer
        let mut buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(e) => {
                return Err(HissrvError::InfluxDBError(format!(
                    "Failed to lock buffer: {}",
                    e
                )))
            }
        };

        buffer.push(point);

        // Check if buffer is full
        if buffer.len() >= self.config.batch_size {
            drop(buffer); // Release the lock before flushing
            self.flush()?;
        }

        Ok(())
    }

    /// Process a hash value
    fn process_hash(
        &self,
        key: &str,
        value: &HashMap<String, String>,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let mut point = Point::new(self.get_measurement_name(key))
            .timestamp(timestamp.timestamp_nanos());

        // Add tags
        for tag_mapping in &self.mapping.tag_mappings {
            if tag_mapping.extract_from_key {
                if key.contains(&tag_mapping.redis_source) {
                    point = point.tag(tag_mapping.influx_tag.clone(), tag_mapping.redis_source.clone());
                }
            } else if let Some(tag_value) = value.get(&tag_mapping.redis_source) {
                point = point.tag(tag_mapping.influx_tag.clone(), tag_value.clone());
            }
        }

        // Add fields from hash
        for field_mapping in &self.mapping.field_mappings {
            if let Some(field_value) = value.get(&field_mapping.redis_source) {
                match field_mapping.data_type.as_str() {
                    "string" => {
                        point = point.field(field_mapping.influx_field.clone(), field_value.clone());
                    }
                    "integer" => {
                        if let Ok(parsed) = field_value.parse::<i64>() {
                            let scaled = (parsed as f64 * field_mapping.scale_factor) as i64;
                            point = point.field(field_mapping.influx_field.clone(), scaled);
                        } else {
                            warn!("Failed to parse '{}' as integer", field_value);
                        }
                    }
                    "float" => {
                        if let Ok(parsed) = field_value.parse::<f64>() {
                            let scaled = parsed * field_mapping.scale_factor;
                            point = point.field(field_mapping.influx_field.clone(), scaled);
                        } else {
                            warn!("Failed to parse '{}' as float", field_value);
                        }
                    }
                    "boolean" => {
                        match field_value.to_lowercase().as_str() {
                            "true" | "1" | "yes" | "on" => {
                                point = point.field(field_mapping.influx_field.clone(), true);
                            }
                            "false" | "0" | "no" | "off" => {
                                point = point.field(field_mapping.influx_field.clone(), false);
                            }
                            _ => {
                                warn!("Failed to parse '{}' as boolean", field_value);
                            }
                        }
                    }
                    _ => {
                        warn!("Unsupported data type: {}", field_mapping.data_type);
                    }
                }
            }
        }

        // Add to buffer
        let mut buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(e) => {
                return Err(HissrvError::InfluxDBError(format!(
                    "Failed to lock buffer: {}",
                    e
                )))
            }
        };

        buffer.push(point);

        // Check if buffer is full
        if buffer.len() >= self.config.batch_size {
            drop(buffer); // Release the lock before flushing
            self.flush()?;
        }

        Ok(())
    }

    /// Get measurement name for a key
    fn get_measurement_name(&self, key: &str) -> String {
        // Check if there's a specific measurement mapping for this key
        for field_mapping in &self.mapping.field_mappings {
            if key.contains(&field_mapping.redis_source) && field_mapping.measurement.is_some() {
                return field_mapping.measurement.clone().unwrap();
            }
        }

        // Fall back to default measurement name
        self.mapping.default_measurement.clone()
    }

    /// Start processing data from channel
    pub fn start_processing(
        &self,
        mut rx: mpsc::Receiver<RedisDataPoint>,
        flush_interval: Duration,
    ) -> Result<()> {
        let client = match &self.client {
            Some(client) => client.clone(),
            None => return Err(HissrvError::InfluxDBError("Not connected to InfluxDB".into())),
        };

        let buffer = Arc::clone(&self.buffer);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(flush_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Time to flush
                        let buffer_clone = Arc::clone(&buffer);
                        let mut buffer_guard = match buffer_clone.lock() {
                            Ok(guard) => guard,
                            Err(e) => {
                                error!("Failed to lock buffer for flushing: {}", e);
                                continue;
                            }
                        };

                        if !buffer_guard.is_empty() {
                            let points = std::mem::take(&mut *buffer_guard);
                            drop(buffer_guard); // Release the lock before flushing

                            match client.write(&config.bucket, stream::iter(points)).await {
                                Ok(_) => {
                                    debug!("Flushed data to InfluxDB");
                                },
                                Err(e) => {
                                    error!("Failed to write to InfluxDB: {}", e);
                                }
                            }
                        }
                    }
                    
                    data = rx.recv() => {
                        match data {
                            Some(data_point) => {
                                // Process data point
                                use crate::redis_handler::RedisData;
                                let mut point = Point::new(data_point.key.clone())
                                    .timestamp(data_point.timestamp.timestamp_nanos());
                                
                                // Add fields based on data type
                                match &data_point.data {
                                    RedisData::String(value) => {
                                        point = point.field("value", value.clone());
                                    }
                                    RedisData::Integer(value) => {
                                        point = point.field("value", *value);
                                    }
                                    RedisData::Float(value) => {
                                        point = point.field("value", *value);
                                    }
                                    RedisData::Boolean(value) => {
                                        point = point.field("value", *value);
                                    }
                                    RedisData::Hash(hash) => {
                                        for (key, value) in hash {
                                            // Try to parse the value
                                            if let Ok(int_value) = value.parse::<i64>() {
                                                point = point.field(key, int_value);
                                            } else if let Ok(float_value) = value.parse::<f64>() {
                                                point = point.field(key, float_value);
                                            } else if let Ok(bool_value) = value.parse::<bool>() {
                                                point = point.field(key, bool_value);
                                            } else {
                                                point = point.field(key, value.clone());
                                            }
                                        }
                                    }
                                    RedisData::None => {
                                        // Skip empty data
                                        continue;
                                    }
                                }
                                
                                // Add to buffer
                                let mut buffer_guard = match buffer.lock() {
                                    Ok(guard) => guard,
                                    Err(e) => {
                                        error!("Failed to lock buffer: {}", e);
                                        continue;
                                    }
                                };
                                
                                buffer_guard.push(point);
                                
                                // Check if buffer is full
                                if buffer_guard.len() >= config.batch_size {
                                    let points = std::mem::take(&mut *buffer_guard);
                                    drop(buffer_guard); // Release the lock
                                    
                                    match client.write(&config.bucket, stream::iter(points)).await {
                                        Ok(_) => {
                                            debug!("Flushed data to InfluxDB (buffer full)");
                                        },
                                        Err(e) => {
                                            error!("Failed to write to InfluxDB: {}", e);
                                        }
                                    }
                                }
                            }
                            None => {
                                // Channel closed
                                info!("Data channel closed, stopping processor");
                                break;
                            }
                        }
                    }
                }
            }
            
            // Final flush before exiting
            let buffer_clone = Arc::clone(&buffer);
            let mut buffer_guard = match buffer_clone.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    error!("Failed to lock buffer for final flush: {}", e);
                    return;
                }
            };

            if !buffer_guard.is_empty() {
                let points = std::mem::take(&mut *buffer_guard);
                drop(buffer_guard); // Release the lock

                if let Err(e) = client.write(&config.bucket, stream::iter(points)).await {
                    error!("Failed to write to InfluxDB on final flush: {}", e);
                } else {
                    debug!("Final flush to InfluxDB");
                }
            }
        });

        Ok(())
    }
}
