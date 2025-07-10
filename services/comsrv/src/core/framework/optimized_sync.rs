//! Optimized Redis batch synchronization
//!
//! This module implements high-performance batch updates using the new
//! separated config/realtime data structure.

use crate::core::framework::realtime_data::RealtimeBatch;
use crate::core::redis::optimized_redis::OptimizedRedisStorage;
use crate::utils::error::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info};

/// Configuration for optimized batch sync
#[derive(Debug, Clone)]
pub struct OptimizedSyncConfig {
    /// Channel ID
    pub channel_id: u16,
    /// Batch size before flush
    pub batch_size: usize,
    /// Flush interval
    pub flush_interval_ms: u64,
    /// Enable compression for large batches
    pub enable_compression: bool,
}

impl Default for OptimizedSyncConfig {
    fn default() -> Self {
        Self {
            channel_id: 1,
            batch_size: 100,
            flush_interval_ms: 1000,
            enable_compression: false,
        }
    }
}

/// Optimized batch synchronizer
pub struct OptimizedBatchSync {
    /// Configuration
    config: OptimizedSyncConfig,
    /// Redis storage
    storage: Arc<Mutex<OptimizedRedisStorage>>,
    /// Current batch
    current_batch: Arc<RwLock<RealtimeBatch>>,
    /// Statistics
    stats: Arc<RwLock<SyncStats>>,
    /// Running flag
    running: Arc<RwLock<bool>>,
}

/// Synchronization statistics
#[derive(Debug, Default, Clone)]
pub struct SyncStats {
    pub total_points_synced: u64,
    pub total_batches: u64,
    pub average_batch_size: f64,
    pub average_sync_time_ms: f64,
    pub failed_syncs: u64,
    pub last_sync_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl OptimizedBatchSync {
    /// Create new optimized batch synchronizer
    pub fn new(config: OptimizedSyncConfig, storage: Arc<Mutex<OptimizedRedisStorage>>) -> Self {
        let batch = RealtimeBatch::new(config.channel_id);

        Self {
            config,
            storage,
            current_batch: Arc::new(RwLock::new(batch)),
            stats: Arc::new(RwLock::new(SyncStats::default())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Add a point value with raw data
    pub async fn add_point(&self, point_id: String, raw_value: f64, eng_value: f64) {
        let mut batch = self.current_batch.write().await;
        batch.add_point(point_id, raw_value, eng_value);

        // Check if we should flush
        if batch.len() >= self.config.batch_size {
            drop(batch); // Release lock before flushing
            if let Err(e) = self.flush().await {
                error!("Failed to flush batch: {}", e);
            }
        }
    }

    /// Add multiple points
    pub async fn add_points(&self, points: Vec<(String, f64, f64)>) {
        let mut batch = self.current_batch.write().await;

        for (point_id, raw, value) in points {
            batch.add_point(point_id, raw, value);
        }

        // Check if we should flush
        if batch.len() >= self.config.batch_size {
            drop(batch); // Release lock before flushing
            if let Err(e) = self.flush().await {
                error!("Failed to flush batch: {}", e);
            }
        }
    }

    /// Start background sync task
    pub fn start_background_sync(self: Arc<Self>) {
        let sync = Arc::clone(&self);

        tokio::spawn(async move {
            *sync.running.write().await = true;
            info!(
                "Started optimized batch sync for channel {}",
                sync.config.channel_id
            );

            let mut ticker = interval(Duration::from_millis(sync.config.flush_interval_ms));

            while *sync.running.read().await {
                ticker.tick().await;

                if let Err(e) = sync.flush().await {
                    error!("Background sync error: {}", e);
                    sync.stats.write().await.failed_syncs += 1;
                }
            }

            info!(
                "Stopped optimized batch sync for channel {}",
                sync.config.channel_id
            );
        });
    }

    /// Stop background sync
    pub async fn stop(&self) {
        *self.running.write().await = false;

        // Final flush
        if let Err(e) = self.flush().await {
            error!("Failed to flush on stop: {}", e);
        }
    }

    /// Flush current batch to Redis
    async fn flush(&self) -> Result<()> {
        let start = std::time::Instant::now();

        // Take current batch and replace with new one
        let mut batch = self.current_batch.write().await;
        if batch.is_empty() {
            return Ok(());
        }

        let to_sync = std::mem::replace(&mut *batch, RealtimeBatch::new(self.config.channel_id));
        drop(batch);

        // Store to Redis
        let mut storage = self.storage.lock().await;
        storage.store_realtime_batch(&to_sync).await?;

        // Update statistics
        let elapsed = start.elapsed();
        let mut stats = self.stats.write().await;
        stats.total_points_synced += to_sync.len() as u64;
        stats.total_batches += 1;
        stats.average_batch_size = (stats.average_batch_size * (stats.total_batches - 1) as f64
            + to_sync.len() as f64)
            / stats.total_batches as f64;
        stats.average_sync_time_ms = (stats.average_sync_time_ms
            * (stats.total_batches - 1) as f64
            + elapsed.as_millis() as f64)
            / stats.total_batches as f64;
        stats.last_sync_time = Some(chrono::Utc::now());

        debug!(
            "Flushed {} points in {:?} (avg: {:.2}ms)",
            to_sync.len(),
            elapsed,
            stats.average_sync_time_ms
        );

        Ok(())
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> SyncStats {
        self.stats.read().await.clone()
    }

    /// Get current batch size
    pub async fn get_batch_size(&self) -> usize {
        self.current_batch.read().await.len()
    }
}

/// Helper to convert old PointData to new format
pub fn convert_point_data(
    point_id: String,
    value: String,
    scale: f64,
    offset: f64,
) -> (String, f64, f64) {
    // Parse raw value
    let raw = value.parse::<f64>().unwrap_or(0.0);

    // Apply scaling
    let eng_value = raw * scale + offset;

    (point_id, raw, eng_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_accumulation() {
        // This would require a mock storage
        // Skipping for now
    }

    #[test]
    fn test_value_conversion() {
        let (id, raw, eng) =
            convert_point_data("test".to_string(), "1000".to_string(), 0.1, -273.15);

        assert_eq!(id, "test");
        assert_eq!(raw, 1000.0);
        assert_eq!(eng, -173.15); // 1000 * 0.1 - 273.15
    }
}
