//! Modbus-specific polling engine
//! 
//! This module implements a polling mechanism specifically designed for Modbus protocol.
//! Unlike generic polling, this takes advantage of Modbus-specific features like:
//! - Batch reading optimization for consecutive registers
//! - Slave-specific polling intervals
//! - Function code optimization
//! - Exception handling for slave devices

use crate::core::config::types::protocol::TelemetryType;
use crate::core::protocols::common::data_types::TelemetryType as CommonTelemetryType;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use byteorder::{BigEndian, LittleEndian, ByteOrder};

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
    /// Data format (e.g., "float32_be", "uint16", "bool")
    pub data_format: String,
    /// Number of registers to read (e.g., 2 for float32)
    pub register_count: u16,
    /// Byte order for multi-register values (e.g., "ABCD", "CDAB")
    pub byte_order: Option<String>,
}

// Re-export configuration types from config module
pub use crate::core::config::types::channel_parameters::{
    ModbusPollingConfig,
    SlavePollingConfig,
};

/// Modbus polling statistics
#[derive(Debug, Clone, Default)]
pub struct ModbusPollingStats {
    pub total_polls: u64,
    pub successful_polls: u64,
    pub failed_polls: u64,
    pub total_points_read: u64,
    pub average_poll_time_ms: f64,
    pub last_poll_time: Option<chrono::DateTime<chrono::Utc>>,
    pub slave_stats: HashMap<u8, SlavePollingStats>,
}

/// Per-slave polling statistics
#[derive(Debug, Clone, Default)]
pub struct SlavePollingStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
    pub last_error: Option<String>,
}

/// Modbus polling engine
pub struct ModbusPollingEngine {
    /// Polling configuration
    config: ModbusPollingConfig,
    /// Batch optimization configuration
    batch_config: ModbusBatchConfig,
    /// Points organized by slave ID
    points_by_slave: HashMap<u8, Vec<ModbusPoint>>,
    /// Redis manager for storing results
    redis_manager: Option<Arc<crate::core::protocols::common::redis::RedisBatchSync>>,
    /// Running state
    is_running: Arc<RwLock<bool>>,
    /// Polling tasks handles
    task_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    /// Polling statistics
    stats: Arc<RwLock<ModbusPollingStats>>,
}

impl ModbusPollingEngine {
    /// Create a new Modbus polling engine
    pub fn new(config: ModbusPollingConfig) -> Self {
        Self {
            config,
            batch_config: ModbusBatchConfig::default(),
            points_by_slave: HashMap::new(),
            redis_manager: None,
            is_running: Arc::new(RwLock::new(false)),
            task_handles: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(ModbusPollingStats::default())),
        }
    }

    /// Create a new Modbus polling engine with custom batch config
    pub fn new_with_batch_config(config: ModbusPollingConfig, batch_config: ModbusBatchConfig) -> Self {
        Self {
            config,
            batch_config,
            points_by_slave: HashMap::new(),
            redis_manager: None,
            is_running: Arc::new(RwLock::new(false)),
            task_handles: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(ModbusPollingStats::default())),
        }
    }

    /// Set Redis manager for storing polled data
    pub fn set_redis_manager(&mut self, redis_manager: Arc<crate::core::protocols::common::redis::RedisBatchSync>) {
        self.redis_manager = Some(redis_manager);
    }

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
    pub async fn start<F>(&self, read_callback: F) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(u8, u8, u16, u16) -> futures::future::BoxFuture<'static, Result<Vec<u16>, Box<dyn std::error::Error + Send + Sync>>> 
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
            let redis_manager = self.redis_manager.clone();
            let read_cb = read_callback.clone();
            let enable_batch = self.config.enable_batch_reading;
            let stats = self.stats.clone();
            let batch_config = self.batch_config.clone();

            let handle = tokio::spawn(async move {
                let mut ticker = interval(Duration::from_millis(interval_ms));
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                while *is_running.read().await {
                    ticker.tick().await;
                    
                    let poll_start = std::time::Instant::now();
                    let mut points_read = 0;
                    let mut poll_success = true;
                    
                    if enable_batch {
                        // Batch reading optimization
                        let batches = optimize_batch_reading(&points, &batch_config, slave_id);
                        for batch in batches {
                            match poll_batch(slave_id, &batch, &read_cb, &redis_manager, &stats).await {
                                Ok(count) => points_read += count,
                                Err(e) => {
                                    let err_msg = e.to_string();
                                    drop(e); // Explicitly drop e to satisfy Send requirement
                                    error!("Failed to poll batch for slave {}: {}", slave_id, err_msg);
                                    poll_success = false;
                                    update_slave_stats(&stats, slave_id, false, 0.0, Some(err_msg)).await;
                                }
                            }
                        }
                    } else {
                        // Individual point reading
                        for point in &points {
                            match poll_single_point(slave_id, point, &read_cb, &redis_manager, &stats).await {
                                Ok(_) => points_read += 1,
                                Err(e) => {
                                    let err_msg = e.to_string();
                                    drop(e); // Explicitly drop e to satisfy Send requirement
                                    error!("Failed to poll point {} for slave {}: {}", point.point_id, slave_id, err_msg);
                                    poll_success = false;
                                    update_slave_stats(&stats, slave_id, false, 0.0, Some(err_msg)).await;
                                }
                            }
                        }
                    }
                    
                    let poll_duration = poll_start.elapsed().as_millis() as f64;
                    update_global_stats(&stats, poll_success, points_read, poll_duration).await;
                    update_slave_stats(&stats, slave_id, poll_success, poll_duration, None).await;
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
    
    /// Get polling statistics
    pub async fn get_stats(&self) -> ModbusPollingStats {
        self.stats.read().await.clone()
    }
    
    /// Reset polling statistics
    pub async fn reset_stats(&self) {
        *self.stats.write().await = ModbusPollingStats::default();
    }
    
    /// Check if polling is active
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
    
    /// Get points by slave ID
    pub fn get_points_by_slave(&self, slave_id: u8) -> Option<&Vec<ModbusPoint>> {
        self.points_by_slave.get(&slave_id)
    }
}

/// Batch optimization configuration
#[derive(Debug, Clone)]
pub struct ModbusBatchConfig {
    /// Maximum address gap to merge into single request
    pub max_gap: u16,
    /// Maximum batch size (registers/coils)
    pub max_batch_size: u16,
    /// Whether to merge different function codes
    pub merge_function_codes: bool,
    /// Device-specific limits
    pub device_limits: std::collections::HashMap<u8, DeviceLimit>,
}

#[derive(Debug, Clone)]
pub struct DeviceLimit {
    /// Maximum PDU size for this device
    pub max_pdu_size: u16,
    /// Maximum registers per read
    pub max_registers_per_read: u16,
    /// Maximum coils per read
    pub max_coils_per_read: u16,
}

impl Default for ModbusBatchConfig {
    fn default() -> Self {
        Self {
            max_gap: 10, // Allow up to 10 register gap
            max_batch_size: 125, // Standard Modbus limit
            merge_function_codes: false,
            device_limits: std::collections::HashMap::new(),
        }
    }
}

/// Optimize points into batches for efficient reading
fn optimize_batch_reading(
    points: &[ModbusPoint], 
    config: &ModbusBatchConfig,
    slave_id: u8,
) -> Vec<Vec<ModbusPoint>> {
    let mut batches = Vec::new();
    let mut current_batch: Vec<ModbusPoint> = Vec::new();
    let mut last_fc = 0u8;
    let mut last_addr = 0u16;
    
    // Get device-specific limits if available
    let device_limit = config.device_limits.get(&slave_id);
    let max_batch = device_limit
        .map(|d| d.max_registers_per_read.min(config.max_batch_size))
        .unwrap_or(config.max_batch_size);

    for point in points {
        let should_start_new_batch = if current_batch.is_empty() {
            false
        } else {
            // Check if we should start a new batch
            let fc_changed = point.function_code != last_fc && !config.merge_function_codes;
            let gap_too_large = point.register_address > last_addr + 1 + config.max_gap;
            let batch_full = current_batch.len() >= max_batch as usize;
            // Check if adding this point would exceed the batch size
            let first_addr = current_batch[0].register_address;
            let last_point = &current_batch[current_batch.len() - 1];
            let _current_span = last_point.register_address - first_addr + last_point.register_count;
            let new_span = point.register_address - first_addr + point.register_count;
            let span_too_large = new_span > max_batch;
            
            fc_changed || gap_too_large || batch_full || span_too_large
        };

        if should_start_new_batch {
            debug!(
                "Starting new batch: fc_changed={}, gap={}, size={}, span={}",
                point.function_code != last_fc,
                point.register_address.saturating_sub(last_addr + 1),
                current_batch.len(),
                if current_batch.is_empty() { 0 } else { 
                    point.register_address - current_batch[0].register_address + 1 
                }
            );
            batches.push(current_batch);
            current_batch = Vec::new();
        }

        current_batch.push(point.clone());
        last_fc = point.function_code;
        last_addr = point.register_address + point.register_count - 1; // Update to last register of this point
    }

    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    // Log optimization results
    let total_points = points.len();
    let total_batches = batches.len();
    let optimization_ratio = if total_points > 0 {
        (total_points as f64 - total_batches as f64) / total_points as f64 * 100.0
    } else {
        0.0
    };
    
    debug!(
        "Batch optimization: {} points → {} batches ({}% reduction)",
        total_points, total_batches, optimization_ratio as i32
    );

    batches
}

/// Parse Modbus register values according to data format
fn parse_modbus_value(registers: &[u16], data_format: &str, byte_order: Option<&str>) -> f64 {
    match data_format {
        "float32" | "float32_be" | "float32_le" => {
            if registers.len() < 2 {
                warn!("Not enough registers for float32: {} registers", registers.len());
                return 0.0;
            }
            
            // Convert two u16 registers to bytes
            let mut bytes = [0u8; 4];
            
            // Handle byte order (default to big-endian for Modbus)
            match byte_order.unwrap_or("ABCD") {
                "ABCD" => {
                    // Big-endian (normal Modbus order)
                    BigEndian::write_u16(&mut bytes[0..2], registers[0]);
                    BigEndian::write_u16(&mut bytes[2..4], registers[1]);
                    BigEndian::read_f32(&bytes) as f64
                },
                "DCBA" => {
                    // Little-endian with swapped registers
                    BigEndian::write_u16(&mut bytes[0..2], registers[1]);
                    BigEndian::write_u16(&mut bytes[2..4], registers[0]);
                    LittleEndian::read_f32(&bytes) as f64
                },
                "BADC" => {
                    // Middle-endian (swapped bytes within registers)
                    LittleEndian::write_u16(&mut bytes[0..2], registers[0]);
                    LittleEndian::write_u16(&mut bytes[2..4], registers[1]);
                    BigEndian::read_f32(&bytes) as f64
                },
                "CDAB" => {
                    // Middle-endian (swapped registers)
                    BigEndian::write_u16(&mut bytes[0..2], registers[1]);
                    BigEndian::write_u16(&mut bytes[2..4], registers[0]);
                    BigEndian::read_f32(&bytes) as f64
                },
                _ => {
                    // Default to big-endian
                    BigEndian::write_u16(&mut bytes[0..2], registers[0]);
                    BigEndian::write_u16(&mut bytes[2..4], registers[1]);
                    BigEndian::read_f32(&bytes) as f64
                }
            }
        },
        "uint32" | "uint32_be" => {
            if registers.len() < 2 {
                return 0.0;
            }
            ((registers[0] as u32) << 16 | registers[1] as u32) as f64
        },
        "int32" | "int32_be" => {
            if registers.len() < 2 {
                return 0.0;
            }
            let val = ((registers[0] as u32) << 16 | registers[1] as u32) as i32;
            val as f64
        },
        "uint16" => {
            if registers.is_empty() {
                return 0.0;
            }
            registers[0] as f64
        },
        "int16" => {
            if registers.is_empty() {
                return 0.0;
            }
            registers[0] as i16 as f64
        },
        "bool" => {
            if registers.is_empty() {
                return 0.0;
            }
            if registers[0] != 0 { 1.0 } else { 0.0 }
        },
        _ => {
            // Default to uint16
            if registers.is_empty() {
                return 0.0;
            }
            registers[0] as f64
        }
    }
}

/// Poll a batch of points
async fn poll_batch<F>(
    slave_id: u8,
    batch: &[ModbusPoint],
    read_callback: &F,
    redis_manager: &Option<Arc<crate::core::protocols::common::redis::RedisBatchSync>>,
    _stats: &Arc<RwLock<ModbusPollingStats>>,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn(u8, u8, u16, u16) -> futures::future::BoxFuture<'static, Result<Vec<u16>, Box<dyn std::error::Error + Send + Sync>>>,
{
    if batch.is_empty() {
        return Ok(0);
    }

    let first_point = &batch[0];
    let last_point = &batch[batch.len() - 1];
    let start_addr = first_point.register_address;
    // Calculate total count including the last point's register count
    let count = (last_point.register_address - start_addr + last_point.register_count) as u16;

    debug!(
        "Batch reading slave {} fc {} addr {} count {}",
        slave_id, first_point.function_code, start_addr, count
    );

    match read_callback(slave_id, first_point.function_code, start_addr, count).await {
        Ok(values) => {
            let mut point_data_list = Vec::new();
            let mut points_read = 0;
            
            // Map values back to points
            for point in batch {
                let offset = (point.register_address - start_addr) as usize;
                let reg_count = point.register_count as usize;
                
                if offset + reg_count <= values.len() {
                    // Extract the required registers for this point
                    let point_registers = &values[offset..offset + reg_count];
                    
                    // Parse the value according to data format
                    let value = parse_modbus_value(
                        point_registers, 
                        &point.data_format, 
                        point.byte_order.as_deref()
                    );
                    
                    let scaled_value = point.scale_factor.map(|s| value * s).unwrap_or(value);
                    
                    let point_data = crate::core::protocols::common::data_types::PointData {
                        id: point.point_id.clone(),
                        name: format!("Point_{}", point.point_id),
                        value: scaled_value.to_string(),
                        timestamp: chrono::Utc::now(),
                        unit: String::new(),
                        description: format!("Modbus point from slave {}", slave_id),
                        telemetry_type: Some(match point.telemetry_type {
                            TelemetryType::Telemetry => CommonTelemetryType::Telemetry,
                            TelemetryType::Signal => CommonTelemetryType::Signal,
                            TelemetryType::Control => CommonTelemetryType::Control,
                            TelemetryType::Adjustment => CommonTelemetryType::Adjustment,
                        }),
                        channel_id: None, // Will be set by Redis sync if configured
                    };
                    
                    point_data_list.push(point_data);
                    points_read += 1;
                    debug!("Point {} value: {}", point.point_id, scaled_value);
                }
            }
            
            // Store in Redis if available
            if let Some(redis) = redis_manager {
                if let Err(e) = redis.batch_update_values(point_data_list).await {
                    warn!("Failed to store batch data in Redis: {}", e);
                }
            }
            
            Ok(points_read)
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
    redis_manager: &Option<Arc<crate::core::protocols::common::redis::RedisBatchSync>>,
    _stats: &Arc<RwLock<ModbusPollingStats>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn(u8, u8, u16, u16) -> futures::future::BoxFuture<'static, Result<Vec<u16>, Box<dyn std::error::Error + Send + Sync>>>,
{
    debug!(
        "Reading point {} from slave {} fc {} addr {}",
        point.point_id, slave_id, point.function_code, point.register_address
    );

    match read_callback(slave_id, point.function_code, point.register_address, point.register_count).await {
        Ok(values) => {
            if values.len() >= point.register_count as usize {
                // Parse the value according to data format
                let value = parse_modbus_value(
                    &values[..point.register_count as usize], 
                    &point.data_format, 
                    point.byte_order.as_deref()
                );
                
                let scaled_value = point.scale_factor.map(|s| value * s).unwrap_or(value);
                
                let point_data = crate::core::protocols::common::data_types::PointData {
                    id: point.point_id.clone(),
                    name: format!("Point_{}", point.point_id),
                    value: scaled_value.to_string(),
                    timestamp: chrono::Utc::now(),
                    unit: String::new(),
                    description: format!("Modbus point from slave {}", slave_id),
                    telemetry_type: Some(match point.telemetry_type {
                        TelemetryType::Telemetry => CommonTelemetryType::Telemetry,
                        TelemetryType::Signal => CommonTelemetryType::Signal,
                        TelemetryType::Control => CommonTelemetryType::Control,
                        TelemetryType::Adjustment => CommonTelemetryType::Adjustment,
                    }),
                    channel_id: None, // Will be set by Redis sync if configured
                };
                
                // Store in Redis if available
                if let Some(redis) = redis_manager {
                    if let Err(e) = redis.update_value(point_data).await {
                        warn!("Failed to store point data in Redis: {}", e);
                    }
                }
                
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

/// Update global polling statistics
async fn update_global_stats(
    _stats: &Arc<RwLock<ModbusPollingStats>>,
    success: bool,
    points_read: usize,
    duration_ms: f64,
) {
    let mut stats = _stats.write().await;
    stats.total_polls += 1;
    
    if success {
        stats.successful_polls += 1;
        stats.total_points_read += points_read as u64;
    } else {
        stats.failed_polls += 1;
    }
    
    // Update average poll time
    let total_time = stats.average_poll_time_ms * (stats.total_polls - 1) as f64 + duration_ms;
    stats.average_poll_time_ms = total_time / stats.total_polls as f64;
    
    stats.last_poll_time = Some(chrono::Utc::now());
}

/// Update per-slave statistics
async fn update_slave_stats(
    _stats: &Arc<RwLock<ModbusPollingStats>>,
    slave_id: u8,
    success: bool,
    duration_ms: f64,
    error: Option<String>,
) {
    let mut stats = _stats.write().await;
    let slave_stats = stats.slave_stats.entry(slave_id).or_insert_with(SlavePollingStats::default);
    
    slave_stats.total_requests += 1;
    
    if success {
        slave_stats.successful_requests += 1;
        slave_stats.last_error = None;
    } else {
        slave_stats.failed_requests += 1;
        if let Some(err) = error {
            slave_stats.last_error = Some(err);
        }
    }
    
    // Update average response time
    let total_time = slave_stats.average_response_time_ms * (slave_stats.total_requests - 1) as f64 + duration_ms;
    slave_stats.average_response_time_ms = total_time / slave_stats.total_requests as f64;
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

        let config = ModbusBatchConfig::default();
        let batches = optimize_batch_reading(&points, &config, 1);
        assert_eq!(batches.len(), 2); // Should split by function code
    }

    #[test]
    fn test_batch_optimization_with_gap() {
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
                telemetry_type: TelemetryType::Telemetry,
                slave_id: 1,
                function_code: 3,
                register_address: 120,
                scale_factor: None,
            },
        ];

        let mut config = ModbusBatchConfig::default();
        config.max_gap = 5; // Small gap
        let batches = optimize_batch_reading(&points, &config, 1);
        assert_eq!(batches.len(), 2); // Should split due to gap

        config.max_gap = 30; // Large gap
        let batches = optimize_batch_reading(&points, &config, 1);
        assert_eq!(batches.len(), 1); // Should merge into one batch
    }

    #[test]
    fn test_batch_optimization_with_device_limit() {
        let mut points = vec![];
        for i in 0..200 {
            points.push(ModbusPoint {
                point_id: format!("{}", i),
                telemetry_type: TelemetryType::Telemetry,
                slave_id: 1,
                function_code: 3,
                register_address: 100 + i,
                scale_factor: None,
            });
        }

        let mut config = ModbusBatchConfig::default();
        config.device_limits.insert(1, DeviceLimit {
            max_pdu_size: 253,
            max_registers_per_read: 50,
            max_coils_per_read: 2000,
        });
        
        let batches = optimize_batch_reading(&points, &config, 1);
        // Should split into 4 batches (200 / 50)
        assert_eq!(batches.len(), 4);
        for batch in &batches {
            assert!(batch.len() <= 50);
        }
    }
}