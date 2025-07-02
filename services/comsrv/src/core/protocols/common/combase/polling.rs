//! Polling Engine Module
//!
//! This module contains the polling engine implementation for periodic data collection.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use chrono::{DateTime, Utc};

use super::data_types::{PointData, PollingConfig, PollingStats, PollingPoint};

use crate::utils::Result;

/// Polling context containing all necessary data for polling operations
/// This reduces the need for multiple Arc clones
#[derive(Clone)]
struct PollingContext {
    config: Arc<RwLock<PollingConfig>>,
    points: Arc<RwLock<Vec<PollingPoint>>>,
    stats: Arc<RwLock<PollingStats>>,
    is_running: Arc<RwLock<bool>>,
    point_reader: Arc<dyn PointReader>,
    protocol_name: Arc<str>,
    data_callback: Option<Arc<dyn Fn(Vec<PointData>) + Send + Sync>>,
    time_cache: Arc<TimeCache>,
}

impl PollingContext {
    fn new(
        config: Arc<RwLock<PollingConfig>>,
        points: Arc<RwLock<Vec<PollingPoint>>>,
        stats: Arc<RwLock<PollingStats>>,
        is_running: Arc<RwLock<bool>>,
        point_reader: Arc<dyn PointReader>,
        protocol_name: String,
        data_callback: Option<Arc<dyn Fn(Vec<PointData>) + Send + Sync>>,
        time_cache: Arc<TimeCache>,
    ) -> Self {
        Self {
            config,
            points,
            stats,
            is_running,
            point_reader,
            protocol_name: Arc::from(protocol_name),
            data_callback,
            time_cache,
        }
    }
}

/// Time cache to reduce frequent Utc::now() calls
struct TimeCache {
    /// Cached timestamp
    cached_time: Arc<RwLock<DateTime<Utc>>>,
    /// Last update instant
    last_update: Arc<RwLock<Instant>>,
    /// Cache duration in milliseconds
    cache_duration_ms: u64,
}

impl TimeCache {
    /// Create a new time cache with default duration of 100ms
    fn new() -> Self {
        let now_utc = Utc::now();
        let now_instant = Instant::now();
        Self {
            cached_time: Arc::new(RwLock::new(now_utc)),
            last_update: Arc::new(RwLock::new(now_instant)),
            cache_duration_ms: 100, // Cache for 100ms
        }
    }

    /// Get current time, using cache if still valid
    async fn now(&self) -> DateTime<Utc> {
        let current_instant = Instant::now();
        let last_update = *self.last_update.read().await;
        
        // Check if cache is still valid
        if current_instant.duration_since(last_update).as_millis() < self.cache_duration_ms as u128 {
            return *self.cached_time.read().await;
        }

        // Update cache
        let fresh_time = Utc::now();
        *self.cached_time.write().await = fresh_time;
        *self.last_update.write().await = current_instant;
        
        fresh_time
    }
}

/// Polling engine trait for protocol-specific implementations
#[async_trait]
pub trait PollingEngine: Send + Sync {
    /// Start the polling engine
    async fn start_polling(&self, config: PollingConfig, points: Vec<PollingPoint>) -> Result<()>;

    /// Stop the polling engine
    async fn stop_polling(&self) -> Result<()>;

    /// Get current polling statistics
    async fn get_polling_stats(&self) -> PollingStats;

    /// Check if polling is currently active
    async fn is_polling_active(&self) -> bool;

    /// Update polling configuration at runtime
    async fn update_polling_config(&self, config: PollingConfig) -> Result<()>;

    /// Add or update polling points
    async fn update_polling_points(&self, points: Vec<PollingPoint>) -> Result<()>;

    /// Read a single point (protocol-specific implementation)
    async fn read_point(&self, point: &PollingPoint) -> Result<PointData>;

    /// Read multiple points in batch (protocol-specific optimization)
    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>>;
}

/// Point reader trait for protocol-specific point reading
#[async_trait]
pub trait PointReader: Send + Sync {
    /// Read a single point using protocol-specific logic
    async fn read_point(&self, point: &PollingPoint) -> Result<PointData>;

    /// Read multiple points in batch (optional optimization)
    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
        // Default implementation: read points individually
        let mut results = Vec::new();
        for point in points {
            match self.read_point(point).await {
                Ok(data) => results.push(data),
                Err(e) => {
                    // Create error data point
                    results.push(PointData {
                        id: point.id.to_string(),
                        name: point.name.to_string(),
                        value: "null".to_string(),
                        timestamp: chrono::Utc::now(),
                        unit: point.unit.clone(),
                        description: format!("Failed to read point {}: {}", point.id, e),
                    });
                    warn!("Failed to read point {}: {}", point.id, e);
                }
            }
        }
        Ok(results)
    }

    /// Check if the connection is healthy
    async fn is_connected(&self) -> bool;

    /// Get protocol name for logging
    fn protocol_name(&self) -> &str;
}

/// Universal polling engine implementation
pub struct UniversalPollingEngine {
    /// Polling context containing all shared state
    context: PollingContext,
    /// Task handle for polling task
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl UniversalPollingEngine {
    /// Create a new universal polling engine
    pub fn new(protocol_name: String, point_reader: Arc<dyn PointReader>) -> Self {
        let context = PollingContext::new(
            Arc::new(RwLock::new(PollingConfig::default())),
            Arc::new(RwLock::new(Vec::new())),
            Arc::new(RwLock::new(PollingStats::default())),
            Arc::new(RwLock::new(false)),
            point_reader,
            protocol_name,
            None,
            Arc::new(TimeCache::new()),
        );
        
        Self {
            context,
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Set data callback for processing read values
    pub fn set_data_callback<F>(&mut self, callback: F)
    where
        F: Fn(Vec<PointData>) + Send + Sync + 'static,
    {
        // Need to create a new context with the updated callback
        let mut new_context = self.context.clone();
        new_context.data_callback = Some(Arc::new(callback));
        self.context = new_context;
    }
}

#[async_trait]
impl PollingEngine for UniversalPollingEngine {
    async fn start_polling(&self, config: PollingConfig, points: Vec<PollingPoint>) -> Result<()> {
        let mut running = self.context.is_running.write().await;
        if *running {
            return Err(crate::utils::ComSrvError::InvalidOperation(
                "Polling is already running".to_string(),
            ));
        }

        if !config.enabled {
            info!("Polling is disabled for protocol: {}", self.context.protocol_name);
            return Ok(());
        }

        // Update configuration and points
        *self.context.config.write().await = config;
        *self.context.points.write().await = points;

        // Start polling task
        let task_handle = self.start_polling_task().await;
        *self.task_handle.write().await = Some(task_handle);
        *running = true;

        info!("Started polling for protocol: {}", self.context.protocol_name);
        Ok(())
    }

    async fn stop_polling(&self) -> Result<()> {
        let mut running = self.context.is_running.write().await;
        if !*running {
            return Ok(());
        }

        // Stop the polling task
        if let Some(handle) = self.task_handle.write().await.take() {
            handle.abort();
            // Wait for task to finish or timeout
            match tokio::time::timeout(Duration::from_secs(5), handle).await {
                Ok(_) => info!("Polling task stopped gracefully for protocol: {}", self.context.protocol_name),
                Err(_) => warn!("Polling task stop timeout for protocol: {}", self.context.protocol_name),
            }
        }

        *running = false;
        info!("Stopped polling for protocol: {}", self.context.protocol_name);
        Ok(())
    }

    async fn get_polling_stats(&self) -> PollingStats {
        self.context.stats.read().await.clone()
    }

    async fn is_polling_active(&self) -> bool {
        *self.context.is_running.read().await
    }

    async fn update_polling_config(&self, config: PollingConfig) -> Result<()> {
        *self.context.config.write().await = config;
        info!("Updated polling configuration for protocol: {}", self.context.protocol_name);
        Ok(())
    }

    async fn update_polling_points(&self, points: Vec<PollingPoint>) -> Result<()> {
        *self.context.points.write().await = points;
        info!("Updated polling points for protocol: {}", self.context.protocol_name);
        Ok(())
    }

    async fn read_point(&self, point: &PollingPoint) -> Result<PointData> {
        self.context.point_reader.read_point(point).await
    }

    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
        self.context.point_reader.read_points_batch(points).await
    }
}

impl UniversalPollingEngine {
    /// Start the polling task
    async fn start_polling_task(&self) -> JoinHandle<()> {
        // Only clone the context once
        let context = self.context.clone();

        tokio::spawn(async move {
            let mut cycle_number = 0u64;
            let config_guard = context.config.read().await;
            let mut polling_interval = interval(Duration::from_millis(config_guard.interval_ms));
            drop(config_guard);

            while *context.is_running.read().await {
                polling_interval.tick().await;
                cycle_number += 1;

                let cycle_start = Instant::now();
                let config_guard = context.config.read().await;
                let current_config = config_guard.clone();
                drop(config_guard);

                // Check if reader is connected
                if !context.point_reader.is_connected().await {
                    warn!("Point reader not connected for protocol: {}, skipping cycle", context.protocol_name);
                    Self::update_stats(&context.stats, false, 0, cycle_start.elapsed().as_millis() as f64, &context.time_cache).await;
                    continue;
                }

                // Execute polling cycle
                match Self::execute_polling_cycle(
                    &current_config,
                    &context.points,
                    &context.point_reader,
                    &context.protocol_name,
                    cycle_number,
                    &context.time_cache,
                ).await {
                    Ok(data_points) => {
                        let points_read = data_points.len();
                        let cycle_time = cycle_start.elapsed().as_millis() as f64;

                        // Call data callback if set
                        if let Some(ref callback) = context.data_callback {
                            callback(data_points);
                        }

                        Self::update_stats(&context.stats, true, points_read, cycle_time, &context.time_cache).await;
                        debug!("Polling cycle {} completed: {} points read in {:.2}ms", 
                               cycle_number, points_read, cycle_time);
                    }
                    Err(e) => {
                        let cycle_time = cycle_start.elapsed().as_millis() as f64;
                        Self::update_stats(&context.stats, false, 0, cycle_time, &context.time_cache).await;
                        error!("Polling cycle {} failed for protocol {}: {}", 
                               cycle_number, context.protocol_name, e);
                    }
                }
            }

            info!("Polling task stopped for protocol: {}", context.protocol_name);
        })
    }

    /// Execute a single polling cycle
    async fn execute_polling_cycle(
        config: &PollingConfig,
        points: &Arc<RwLock<Vec<PollingPoint>>>,
        point_reader: &Arc<dyn PointReader>,
        protocol_name: &str,
        cycle_number: u64,
        time_cache: &Arc<TimeCache>,
    ) -> Result<Vec<PointData>> {
        let points_guard = points.read().await;
        
        if points_guard.is_empty() {
            debug!("No points to read for protocol: {}", protocol_name);
            return Ok(Vec::new());
        }

        let mut all_data = Vec::new();

        if config.enable_batch_reading {
            // Group points by their group attribute for batch reading  
            let grouped_points = Self::group_points_for_batch_reading_ref(&*points_guard);
            
            for (group_name, group_indices) in grouped_points {
                debug!("Reading point group '{}' with {} points", group_name, group_indices.len());
                
                // Collect points for this group
                let group_points: Vec<PollingPoint> = group_indices.iter()
                    .map(|&idx| points_guard[idx].clone())
                    .collect();
                
                match point_reader.read_points_batch(&group_points).await {
                    Ok(mut batch_data) => {
                        all_data.append(&mut batch_data);
                    }
                    Err(e) => {
                        warn!("Batch read failed for group '{}': {}, falling back to individual reads", 
                              group_name, e);
                        
                        // Fallback to individual reads
                        for &idx in &group_indices {
                            let point = &points_guard[idx];
                            match point_reader.read_point(point).await {
                                Ok(data) => all_data.push(data),
                                Err(e) => {
                                    warn!("Failed to read point {}: {}", point.id, e);
                                    // Create error point data with Arc strings to avoid cloning
                                    all_data.push(PointData {
                                        id: point.id.to_string(),
                                        name: point.name.to_string(),
                                        value: "null".to_string(),
                                        timestamp: time_cache.now().await,
                                        unit: point.unit.clone(),
                                        description: format!("Read error: {}", e),
                                    });
                                }
                            }
                            
                            // Respect point read delay
                            if config.point_read_delay_ms > 0 {
                                tokio::time::sleep(Duration::from_millis(config.point_read_delay_ms)).await;
                            }
                        }
                    }
                }
            }
        } else {
            // Individual point reading - iterate by reference
            for point in points_guard.iter() {
                match point_reader.read_point(point).await {
                    Ok(data) => all_data.push(data),
                    Err(e) => {
                        warn!("Failed to read point {}: {}", point.id, e);
                        all_data.push(PointData {
                            id: point.id.to_string(),
                            name: point.name.to_string(),
                            value: "null".to_string(),
                            timestamp: time_cache.now().await,
                            unit: point.unit.clone(),
                            description: format!("Read error: {}", e),
                        });
                    }
                }
                
                // Respect point read delay
                if config.point_read_delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(config.point_read_delay_ms)).await;
                }
            }
        }

        debug!("Polling cycle {} completed: read {} points", cycle_number, all_data.len());
        Ok(all_data)
    }

    /// Group points by their group attribute for batch reading, returning indices
    fn group_points_for_batch_reading_ref(
        points: &[PollingPoint],
    ) -> HashMap<Arc<str>, Vec<usize>> {
        let mut groups = HashMap::new();
        
        for (idx, point) in points.iter().enumerate() {
            let group_name = if point.group.is_empty() {
                Arc::from("default")
            } else {
                Arc::clone(&point.group)
            };
            
            groups.entry(group_name).or_insert_with(Vec::new).push(idx);
        }
        
        groups
    }

    /// Update polling statistics
    async fn update_stats(
        stats: &Arc<RwLock<PollingStats>>,
        success: bool,
        points_read: usize,
        cycle_time_ms: f64,
        time_cache: &Arc<TimeCache>,
    ) {
        let mut stats_guard = stats.write().await;
        
        stats_guard.total_cycles += 1;
        
        if success {
            stats_guard.successful_cycles += 1;
            stats_guard.total_points_read += points_read as u64;
            stats_guard.last_successful_polling = Some(time_cache.now().await);
            stats_guard.last_polling_error = None;
        } else {
            stats_guard.failed_cycles += 1;
            stats_guard.total_points_failed += points_read as u64;
        }

        // Update average cycle time
        let total_cycles = stats_guard.total_cycles as f64;
        let current_avg = stats_guard.avg_cycle_time_ms;
        stats_guard.avg_cycle_time_ms = if total_cycles == 1.0 {
            cycle_time_ms
        } else {
            (current_avg * (total_cycles - 1.0) + cycle_time_ms) / total_cycles
        };

        // Calculate current polling rate (cycles per second)
        if stats_guard.total_cycles > 1 {
            let uptime_seconds = chrono::Utc::now()
                .signed_duration_since(stats_guard.last_successful_polling.unwrap_or(chrono::Utc::now()))
                .num_seconds() as f64;
            if uptime_seconds > 0.0 {
                stats_guard.current_polling_rate = stats_guard.total_cycles as f64 / uptime_seconds;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock point reader for testing
    struct MockPointReader {
        connected: Arc<Mutex<bool>>,
        fail_reads: Arc<Mutex<bool>>,
        read_delay: Arc<Mutex<Option<Duration>>>,
    }

    impl MockPointReader {
        fn new() -> Self {
            Self {
                connected: Arc::new(Mutex::new(true)),
                fail_reads: Arc::new(Mutex::new(false)),
                read_delay: Arc::new(Mutex::new(None)),
            }
        }

        fn set_connected(&self, connected: bool) {
            *self.connected.lock().unwrap() = connected;
        }

        fn set_fail_reads(&self, fail: bool) {
            *self.fail_reads.lock().unwrap() = fail;
        }

        fn set_read_delay(&self, delay: Option<Duration>) {
            *self.read_delay.lock().unwrap() = delay;
        }
    }

    #[async_trait]
    impl PointReader for MockPointReader {
        async fn read_point(&self, point: &PollingPoint) -> Result<PointData> {
            let delay = *self.read_delay.lock().unwrap();
            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            }

            if *self.fail_reads.lock().unwrap() {
                return Err(crate::utils::ComSrvError::InvalidOperation("Mock read failure".to_string()));
            }

            Ok(PointData {
                id: point.id.to_string(),
                name: point.name.to_string(),
                value: "123.45".to_string(),
                timestamp: chrono::Utc::now(),
                unit: point.unit.clone(),
                description: point.description.clone(),
            })
        }

        async fn is_connected(&self) -> bool {
            *self.connected.lock().unwrap()
        }

        fn protocol_name(&self) -> &str {
            "mock"
        }
    }

    fn create_test_point(id: &str, address: u32) -> PollingPoint {
        PollingPoint {
            id: Arc::from(id),
            name: Arc::from(format!("Test Point {}", id)),
            address,
            data_type: "float".to_string(),
            telemetry_type: crate::core::protocols::common::combase::telemetry::TelemetryType::Telemetry,
            scale: 1.0,
            offset: 0.0,
            unit: "°C".to_string(),
            description: "Test point".to_string(),
            access_mode: "read".to_string(),
            group: Arc::from("default"),
            protocol_params: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_universal_polling_engine_creation() {
        let reader = Arc::new(MockPointReader::new());
        let engine = UniversalPollingEngine::new("test_protocol".to_string(), reader);
        
        assert!(!engine.is_polling_active().await);
        assert_eq!(&*engine.context.protocol_name, "test_protocol");
    }

    #[tokio::test]
    async fn test_successful_polling() {
        let reader = Arc::new(MockPointReader::new());
        let engine = UniversalPollingEngine::new("test_protocol".to_string(), reader);
        
        let mut config = PollingConfig::default();
        config.interval_ms = 100; // Fast polling for test
        config.enabled = true;
        
        let points = vec![
            create_test_point("test1", 1),
            create_test_point("test2", 2),
        ];
        
        // Start polling
        engine.start_polling(config, points).await.unwrap();
        assert!(engine.is_polling_active().await);
        
        // Wait for a few cycles
        tokio::time::sleep(Duration::from_millis(250)).await;
        
        // Check stats
        let stats = engine.get_polling_stats().await;
        assert!(stats.total_cycles > 0);
        assert!(stats.successful_cycles > 0);
        assert!(stats.total_points_read > 0);
        
        // Stop polling
        engine.stop_polling().await.unwrap();
        assert!(!engine.is_polling_active().await);
    }

    #[tokio::test]
    async fn test_polling_with_disconnected_reader() {
        let reader = Arc::new(MockPointReader::new());
        reader.set_connected(false);
        
        let engine = UniversalPollingEngine::new("test_protocol".to_string(), reader);
        
        let mut config = PollingConfig::default();
        config.interval_ms = 50;
        config.enabled = true;
        
        let points = vec![create_test_point("test1", 1)];
        
        engine.start_polling(config, points).await.unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        let stats = engine.get_polling_stats().await;
        assert!(stats.total_cycles > 0);
        assert_eq!(stats.successful_cycles, 0); // All should fail due to disconnection
        
        engine.stop_polling().await.unwrap();
    }
} 