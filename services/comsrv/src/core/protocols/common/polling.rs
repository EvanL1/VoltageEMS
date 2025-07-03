//! Polling Engine Module
//!
//! This module contains the polling engine implementation for periodic data collection.
//! Consolidated from combase module.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use chrono::{DateTime, Utc};

use super::data_types::{PointData, PollingConfig, PollingStats, PollingPoint, PollingContext};
use super::traits::PointReader;
use crate::core::transport::traits::Transport;
use crate::utils::Result;

/// Time cache for efficient timestamp generation
pub struct TimeCache {
    last_time: RwLock<DateTime<Utc>>,
}

impl TimeCache {
    pub fn new() -> Self {
        Self {
            last_time: RwLock::new(Utc::now()),
        }
    }

    pub async fn now(&self) -> DateTime<Utc> {
        let now = Utc::now();
        *self.last_time.write().await = now;
        now
    }
}

/// Universal polling engine for data collection
pub struct UniversalPollingEngine {
    /// Polling context (reduces Arc clones from 8 to 1)
    context: Arc<PollingContext>,
    /// Polling task handle
    polling_task: Option<JoinHandle<()>>,
}

impl UniversalPollingEngine {
    /// Create a new polling engine
    pub fn new(
        config: PollingConfig,
        transport: Arc<dyn Transport>,
        protocol_name: String,
    ) -> Self {
        // TODO: Fix polling context after transport trait unification
        let context = Arc::new(PollingContext {
            config: Arc::new(RwLock::new(config)),
            transport,
            point_manager: Arc::new(super::manager::OptimizedPointManager::new(protocol_name.clone())),
            redis_sync: None,
            channel_name: Arc::from(protocol_name),
            stats: Arc::new(RwLock::new(PollingStats::default())),
        });

        Self {
            context,
            polling_task: None,
        }
    }

    /// Start the polling engine
    pub async fn start(&mut self) -> Result<()> {
        let mut is_running = self.context.stats.write().await;
        
        if self.polling_task.is_some() {
            return Ok(()); // Already running
        }

        let context = self.context.clone();
        
        let handle = tokio::spawn(async move {
            Self::polling_loop(context).await;
        });

        self.polling_task = Some(handle);
        info!("Polling engine started for {}", self.context.channel_name);
        
        Ok(())
    }

    /// Stop the polling engine
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.polling_task.take() {
            handle.abort();
            info!("Polling engine stopped for {}", self.context.channel_name);
        }
        Ok(())
    }

    /// Check if the polling engine is running
    pub async fn is_running(&self) -> bool {
        self.polling_task.as_ref().map(|h| !h.is_finished()).unwrap_or(false)
    }

    /// Get polling statistics
    pub async fn get_stats(&self) -> PollingStats {
        self.context.stats.read().await.clone()
    }

    /// Update polling configuration
    pub async fn update_config(&self, config: PollingConfig) {
        *self.context.config.write().await = config;
    }

    /// Main polling loop
    async fn polling_loop(context: Arc<PollingContext>) {
        let config = context.config.read().await;
        let interval_duration = Duration::from_millis(config.interval_ms);
        drop(config);

        let mut interval_timer = interval(interval_duration);
        
        loop {
            interval_timer.tick().await;
            
            if let Err(e) = Self::execute_polling_cycle(&context).await {
                error!("[{}] Polling cycle failed: {}", context.channel_name, e);
                
                let mut stats = context.stats.write().await;
                stats.record_failure(e.to_string());
            }
        }
    }

    /// Execute a single polling cycle
    async fn execute_polling_cycle(context: &Arc<PollingContext>) -> Result<()> {
        let start_time = Instant::now();
        
        // Get readable points for polling
        let readable_points = context.point_manager.get_readable_points().await;
        
        if readable_points.is_empty() {
            debug!("[{}] No readable points configured", context.channel_name);
            return Ok(());
        }

        let config = context.config.read().await;
        let enable_batch_reading = config.enable_batch_reading;
        let max_batch_size = config.max_batch_size;
        drop(config);

        let mut all_data = Vec::new();

        // TODO: Implement actual point reading using transport layer
        // For now, create mock data for testing
        for point in readable_points {
            let mock_data = PointData::new(
                point.id.to_string(),
                point.name.to_string(),
                "42.0".to_string(),
                point.unit.clone(),
            );
            all_data.push(mock_data);
        }

        // Update point manager with new data
        let updates: Vec<(u32, PointData)> = all_data.iter()
            .filter_map(|data| {
                data.id.parse::<u32>().ok().map(|id| (id, data.clone()))
            })
            .collect();
        
        context.point_manager.batch_update_point_data(updates).await?;

        // Send to Redis if configured
        if let Some(redis_sync) = &context.redis_sync {
            let buffer_updates: Vec<(u32, PointData)> = all_data.iter()
                .filter_map(|data| {
                    data.id.parse::<u32>().ok().map(|id| (id, data.clone()))
                })
                .collect();
            redis_sync.buffer_updates(buffer_updates).await;
        }

        // Record statistics
        let elapsed = start_time.elapsed();
        let mut stats = context.stats.write().await;
        stats.record_success(all_data.len(), elapsed.as_millis() as f64);

        debug!("[{}] Polled {} points in {:.2}ms", 
               context.channel_name, all_data.len(), elapsed.as_millis());

        Ok(())
    }

    /// Group points for efficient batch reading
    fn group_points_for_batch_reading(
        points: &[PollingPoint], 
        max_batch_size: usize
    ) -> Vec<Vec<PollingPoint>> {
        let mut batches = Vec::new();
        let mut current_batch = Vec::new();

        for point in points {
            current_batch.push(point.clone());
            
            if current_batch.len() >= max_batch_size {
                batches.push(current_batch);
                current_batch = Vec::new();
            }
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        batches
    }
}

/// Polling engine trait for protocol implementations
#[async_trait]
pub trait PollingEngine: Send + Sync {
    /// Start polling
    async fn start_polling(&mut self) -> Result<()>;
    
    /// Stop polling
    async fn stop_polling(&mut self) -> Result<()>;
    
    /// Check if polling is active
    async fn is_polling(&self) -> bool;
    
    /// Get polling statistics
    async fn get_polling_stats(&self) -> PollingStats;
    
    /// Update polling configuration
    async fn update_polling_config(&mut self, config: PollingConfig) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::protocols::common::manager::generate_test_points;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Mock point reader for testing
    struct MockPointReader {
        fail_reads: Arc<AtomicBool>,
        read_delay: Arc<RwLock<Option<Duration>>>,
    }

    impl MockPointReader {
        fn new() -> Self {
            Self {
                fail_reads: Arc::new(AtomicBool::new(false)),
                read_delay: Arc::new(RwLock::new(None)),
            }
        }

        fn set_fail_reads(&self, fail: bool) {
            self.fail_reads.store(fail, Ordering::Relaxed);
        }

        fn set_read_delay(&self, delay: Option<Duration>) {
            tokio::spawn(async move {
                // Note: This is a simplified mock implementation
            });
        }
    }

    #[async_trait]
    impl PointReader for MockPointReader {
        async fn read_points(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
            if let Some(delay) = *self.read_delay.read().await {
                tokio::time::sleep(delay).await;
            }

            if self.fail_reads.load(Ordering::Relaxed) {
                return Err(crate::utils::ComSrvError::ProtocolError("Mock read failure".to_string()));
            }

            let mut result = Vec::new();
            for point in points {
                let data = PointData::new(
                    point.id.to_string(),
                    point.name.to_string(),
                    "42.0".to_string(),
                    point.unit.clone(),
                );
                result.push(data);
            }
            Ok(result)
        }

        async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
            self.read_points(points).await
        }

        async fn test_read(&self) -> Result<bool> {
            Ok(!self.fail_reads.load(Ordering::Relaxed))
        }
    }

    #[tokio::test]
    async fn test_polling_engine() {
        let config = PollingConfig {
            interval_ms: 100,
            enable_batch_reading: true,
            max_batch_size: 10,
            ..Default::default()
        };

        let mock_reader = Arc::new(MockPointReader::new());
        let mut engine = UniversalPollingEngine::new(
            config,
            mock_reader.clone(),
            "test_protocol".to_string(),
        );

        // Test start/stop
        assert!(!engine.is_running().await);
        
        engine.start().await.unwrap();
        assert!(engine.is_running().await);
        
        // Let it run for a short time
        tokio::time::sleep(Duration::from_millis(250)).await;
        
        engine.stop().await.unwrap();
        assert!(!engine.is_running().await);
        
        // Check statistics
        let stats = engine.get_stats().await;
        assert!(stats.total_polls >= 2); // Should have completed at least 2 polls
    }
}