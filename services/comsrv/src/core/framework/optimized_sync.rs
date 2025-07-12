//! Optimized batch synchronization using flat storage
//!
//! 使用新的扁平化Redis存储结构进行高效批量同步

use crate::plugins::plugin_storage::{DefaultPluginStorage, PluginPointUpdate, PluginStorage};
use crate::utils::error::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
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

/// Optimized batch synchronizer with flat storage
pub struct OptimizedBatchSync {
    /// Configuration
    config: OptimizedSyncConfig,
    /// Plugin storage interface
    storage: Arc<dyn PluginStorage>,
    /// Pending updates buffer
    update_buffer: Arc<Mutex<Vec<PluginPointUpdate>>>,
    /// Sync statistics
    stats: Arc<RwLock<SyncStats>>,
}

impl OptimizedBatchSync {
    /// Create new optimized sync with storage
    pub async fn new(config: OptimizedSyncConfig) -> Result<Self> {
        let storage = Arc::new(DefaultPluginStorage::from_env().await?);
        Ok(Self {
            config,
            storage,
            update_buffer: Arc::new(Mutex::new(Vec::with_capacity(1000))),
            stats: Arc::new(RwLock::new(SyncStats::default())),
        })
    }

    /// Create with custom storage (for testing)
    pub fn with_storage(config: OptimizedSyncConfig, storage: Arc<dyn PluginStorage>) -> Self {
        Self {
            config,
            storage,
            update_buffer: Arc::new(Mutex::new(Vec::with_capacity(1000))),
            stats: Arc::new(RwLock::new(SyncStats::default())),
        }
    }

    /// Buffer point updates for batch sync
    pub async fn buffer_update(&self, update: PluginPointUpdate) -> Result<()> {
        let mut buffer = self.update_buffer.lock().await;
        buffer.push(update);

        // 如果缓冲区满了，立即刷新
        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            self.flush().await?;
        }

        Ok(())
    }

    /// Buffer multiple updates
    pub async fn buffer_updates(&self, updates: Vec<PluginPointUpdate>) -> Result<()> {
        let mut buffer = self.update_buffer.lock().await;
        buffer.extend(updates);

        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            self.flush().await?;
        }

        Ok(())
    }

    /// Flush pending updates to storage
    pub async fn flush(&self) -> Result<()> {
        let start = Instant::now();

        // 取出所有待处理的更新
        let updates = {
            let mut buffer = self.update_buffer.lock().await;
            std::mem::take(&mut *buffer)
        };

        if updates.is_empty() {
            return Ok(());
        }

        let count = updates.len();
        debug!("Flushing {} updates to storage", count);

        // 批量写入存储
        self.storage.write_points(updates).await?;

        // 更新统计信息
        let elapsed = start.elapsed();
        let mut stats = self.stats.write().await;
        stats.total_points += count as u64;
        stats.total_batches += 1;
        stats.avg_batch_size = (stats.avg_batch_size * (stats.total_batches - 1) as f64
            + count as f64)
            / stats.total_batches as f64;
        stats.total_sync_time_ms += elapsed.as_millis() as u64;

        info!("Flushed {} points in {:?}", count, elapsed);

        Ok(())
    }

    /// Start the background sync task
    pub fn start_background_sync(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut sync_interval = interval(Duration::from_millis(self.config.flush_interval_ms));

            loop {
                sync_interval.tick().await;

                if let Err(e) = self.flush().await {
                    error!("Sync flush error: {}", e);
                }
            }
        });
    }

    /// Get sync statistics
    pub async fn get_stats(&self) -> SyncStats {
        self.stats.read().await.clone()
    }
}

/// Sync statistics
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    /// Total points synchronized
    pub total_points: u64,
    /// Total batches sent
    pub total_batches: u64,
    /// Average batch size
    pub avg_batch_size: f64,
    /// Total sync time in ms
    pub total_sync_time_ms: u64,
}
