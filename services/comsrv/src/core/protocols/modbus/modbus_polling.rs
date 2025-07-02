//! Modbus-specific polling engine
//! 
//! This module implements a polling mechanism specifically designed for Modbus protocol.
//! Unlike generic polling, this takes advantage of Modbus-specific features like:
//! - Batch reading optimization for consecutive registers
//! - Slave-specific polling intervals
//! - Function code optimization
//! - Exception handling for slave devices

use crate::core::config::types::protocol::TelemetryType;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// Simplified point mapping for Modbus
/// Only contains essential fields: point_id and telemetry type
#[derive(Debug, Clone)]
pub struct ModbusPoint {
    /// Unique point identifier (matches four-remote table)
    pub point_id: String,
    /// Telemetry type (YC/YX/YK/YT)
    pub telemetry_type: TelemetryType,
    /// Modbus slave ID
    pub slave_id: u8,
    /// Function code for reading
    pub function_code: u8,
    /// Register address
    pub register_address: u16,
    /// Optional data transformation
    pub scale_factor: Option<f64>,
}

/// Modbus-specific polling configuration
#[derive(Debug, Clone)]
pub struct ModbusPollingConfig {
    /// Default polling interval in milliseconds
    pub default_interval_ms: u64,
    /// Enable batch reading optimization
    pub enable_batch_reading: bool,
    /// Maximum registers per batch read
    pub max_batch_size: u16,
    /// Timeout for each read operation
    pub read_timeout_ms: u64,
    /// Slave-specific configurations
    pub slave_configs: HashMap<u8, SlavePollingConfig>,
}

/// Per-slave polling configuration
#[derive(Debug, Clone)]
pub struct SlavePollingConfig {
    /// Polling interval for this slave (overrides default)
    pub interval_ms: Option<u64>,
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
    /// Retry count on failure
    pub retry_count: u8,
}

/// Modbus polling engine
pub struct ModbusPollingEngine {
    /// Polling configuration
    config: ModbusPollingConfig,
    /// Points organized by slave ID
    points_by_slave: HashMap<u8, Vec<ModbusPoint>>,
    /// Redis manager for storing results (TODO: implement later)
    // redis_manager: Option<Arc<RedisManager>>,
    /// Running state
    is_running: Arc<RwLock<bool>>,
    /// Polling tasks handles
    task_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl ModbusPollingEngine {
    /// Create a new Modbus polling engine
    pub fn new(config: ModbusPollingConfig) -> Self {
        Self {
            config,
            points_by_slave: HashMap::new(),
            // redis_manager: None,
            is_running: Arc::new(RwLock::new(false)),
            task_handles: Arc::new(RwLock::new(Vec::new())),
        }
    }

    // TODO: Set Redis manager for storing polled data
    // pub fn set_redis_manager(&mut self, redis_manager: Arc<RedisManager>) {
    //     self.redis_manager = Some(redis_manager);
    // }

    /// Add points for polling
    pub fn add_points(&mut self, points: Vec<ModbusPoint>) {
        for point in points {
            self.points_by_slave
                .entry(point.slave_id)
                .or_insert_with(Vec::new)
                .push(point);
        }

        // Sort points by register address for batch optimization
        for points in self.points_by_slave.values_mut() {
            points.sort_by_key(|p| (p.function_code, p.register_address));
        }
    }

    /// Start polling for all configured slaves
    pub async fn start<F>(&self, read_callback: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(u8, u8, u16, u16) -> futures::future::BoxFuture<'static, Result<Vec<u16>, Box<dyn std::error::Error>>> 
            + Send + Sync + 'static + Clone,
    {
        *self.is_running.write().await = true;
        let mut handles = vec![];

        for (&slave_id, points) in &self.points_by_slave {
            let interval_ms = self.config.slave_configs
                .get(&slave_id)
                .and_then(|cfg| cfg.interval_ms)
                .unwrap_or(self.config.default_interval_ms);

            let points = points.clone();
            let is_running = self.is_running.clone();
            // let redis_manager = self.redis_manager.clone();
            let read_cb = read_callback.clone();
            let enable_batch = self.config.enable_batch_reading;
            let max_batch_size = self.config.max_batch_size;

            let handle = tokio::spawn(async move {
                let mut ticker = interval(Duration::from_millis(interval_ms));
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                while *is_running.read().await {
                    ticker.tick().await;
                    
                    if enable_batch {
                        // Batch reading optimization
                        let batches = optimize_batch_reading(&points, max_batch_size);
                        for batch in batches {
                            if let Err(e) = poll_batch(slave_id, &batch, &read_cb).await {
                                error!("Failed to poll batch for slave {}: {}", slave_id, e);
                            }
                        }
                    } else {
                        // Individual point reading
                        for point in &points {
                            if let Err(e) = poll_single_point(slave_id, point, &read_cb).await {
                                error!("Failed to poll point {} for slave {}: {}", point.point_id, slave_id, e);
                            }
                        }
                    }
                }
            });

            handles.push(handle);
        }

        *self.task_handles.write().await = handles;
        info!("Modbus polling engine started for {} slaves", self.points_by_slave.len());
        Ok(())
    }

    /// Stop polling
    pub async fn stop(&self) {
        *self.is_running.write().await = false;
        
        let handles = std::mem::take(&mut *self.task_handles.write().await);
        for handle in handles {
            let _ = handle.await;
        }
        
        info!("Modbus polling engine stopped");
    }
}

/// Optimize points into batches for efficient reading
fn optimize_batch_reading(points: &[ModbusPoint], max_batch_size: u16) -> Vec<Vec<&ModbusPoint>> {
    let mut batches = Vec::new();
    let mut current_batch = Vec::new();
    let mut last_fc = 0u8;
    let mut last_addr = 0u16;

    for point in points {
        // Start new batch if function code changes or gap is too large
        if !current_batch.is_empty() && 
           (point.function_code != last_fc || 
            point.register_address > last_addr + max_batch_size ||
            current_batch.len() >= max_batch_size as usize) {
            batches.push(current_batch);
            current_batch = Vec::new();
        }

        current_batch.push(point);
        last_fc = point.function_code;
        last_addr = point.register_address;
    }

    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    batches
}

/// Poll a batch of points
async fn poll_batch<F>(
    slave_id: u8,
    batch: &[&ModbusPoint],
    read_callback: &F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(u8, u8, u16, u16) -> futures::future::BoxFuture<'static, Result<Vec<u16>, Box<dyn std::error::Error>>>,
{
    if batch.is_empty() {
        return Ok(());
    }

    let first_point = batch[0];
    let last_point = batch[batch.len() - 1];
    let start_addr = first_point.register_address;
    let count = (last_point.register_address - start_addr + 1) as u16;

    debug!(
        "Batch reading slave {} fc {} addr {} count {}",
        slave_id, first_point.function_code, start_addr, count
    );

    match read_callback(slave_id, first_point.function_code, start_addr, count).await {
        Ok(values) => {
            // Map values back to points
            for point in batch {
                let offset = (point.register_address - start_addr) as usize;
                if offset < values.len() {
                    let value = values[offset] as f64;
                    let scaled_value = point.scale_factor.map(|s| value * s).unwrap_or(value);
                    
                    // TODO: Store in Redis when available
                    // let point_data = PointData {
                    //     point_id: point.point_id.clone(),
                    //     value: scaled_value,
                    //     timestamp: chrono::Utc::now().timestamp_millis(),
                    //     quality: 192, // Good quality
                    // };
                    
                    debug!("Point {} value: {}", point.point_id, scaled_value);
                }
            }
            Ok(())
        }
        Err(e) => {
            warn!("Batch read failed for slave {}: {}", slave_id, e);
            Err(e)
        }
    }
}

/// Poll a single point
async fn poll_single_point<F>(
    slave_id: u8,
    point: &ModbusPoint,
    read_callback: &F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(u8, u8, u16, u16) -> futures::future::BoxFuture<'static, Result<Vec<u16>, Box<dyn std::error::Error>>>,
{
    debug!(
        "Reading point {} from slave {} fc {} addr {}",
        point.point_id, slave_id, point.function_code, point.register_address
    );

    match read_callback(slave_id, point.function_code, point.register_address, 1).await {
        Ok(values) => {
            if !values.is_empty() {
                let value = values[0] as f64;
                let scaled_value = point.scale_factor.map(|s| value * s).unwrap_or(value);
                
                // TODO: Store in Redis when available
                // let point_data = PointData {
                //     point_id: point.point_id.clone(),
                //     value: scaled_value,
                //     timestamp: chrono::Utc::now().timestamp_millis(),
                //     quality: 192, // Good quality
                // };
                
                debug!("Point {} value: {}", point.point_id, scaled_value);
            }
            Ok(())
        }
        Err(e) => {
            warn!("Failed to read point {} from slave {}: {}", point.point_id, slave_id, e);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_optimization() {
        let points = vec![
            ModbusPoint {
                point_id: "1".to_string(),
                telemetry_type: TelemetryType::Telemetry,
                slave_id: 1,
                function_code: 3,
                register_address: 100,
                scale_factor: None,
            },
            ModbusPoint {
                point_id: "2".to_string(),
                telemetry_type: TelemetryType::Telemetry,
                slave_id: 1,
                function_code: 3,
                register_address: 101,
                scale_factor: None,
            },
            ModbusPoint {
                point_id: "3".to_string(),
                telemetry_type: TelemetryType::Signal,
                slave_id: 1,
                function_code: 1,
                register_address: 0,
                scale_factor: None,
            },
        ];

        let batches = optimize_batch_reading(&points, 10);
        assert_eq!(batches.len(), 2); // Should split by function code
    }
}