//! 框架存储模块
//!
//! 整合了ComBase存储接口和优化的批量同步功能

use super::core::RedisValue;
use crate::plugins::core::{telemetry_type_to_redis, PluginPointUpdate, PluginStorage};
use crate::utils::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{debug, error, info, trace};

// 类型别名
type AsyncRedisClient = voltage_libs::redis::RedisClient;

// ============================================================================
// ComBase统一存储接口
// ============================================================================

/// ComBase层统一存储trait
#[async_trait]
pub trait ComBaseStorage: Send + Sync {
    /// 批量更新并发布数据
    async fn batch_update_and_publish(
        &mut self,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()>;

    /// 单点更新并发布
    async fn update_and_publish(
        &mut self,
        channel_id: u16,
        point_id: u32,
        value: RedisValue,
        telemetry_type: &str,
    ) -> Result<()>;

    /// 获取存储统计信息
    async fn get_stats(&self) -> StorageStats;
}

/// 存储统计信息
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    pub total_updates: u64,
    pub batch_updates: u64,
    pub single_updates: u64,
    pub publish_success: u64,
    pub publish_failed: u64,
    pub storage_errors: u64,
}

/// 默认ComBase存储实现
pub struct DefaultComBaseStorage {
    storage: Arc<Mutex<Box<dyn PluginStorage>>>,
    stats: Arc<Mutex<StorageStats>>,
}

impl DefaultComBaseStorage {
    /// 创建新实例
    pub fn new(storage: Box<dyn PluginStorage>) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
            stats: Arc::new(Mutex::new(StorageStats::default())),
        }
    }

    /// 内部批量更新方法
    async fn internal_batch_update(
        &self,
        _channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()> {
        let storage = self.storage.lock().await;

        // 执行批量更新
        storage.write_points(updates.clone()).await?;

        // TODO: 发布到pub/sub通道的功能需要单独实现
        // 当前 PluginStorage trait 不包含 publish 方法

        Ok(())
    }
}

#[async_trait]
impl ComBaseStorage for DefaultComBaseStorage {
    async fn batch_update_and_publish(
        &mut self,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()> {
        let update_count = updates.len();

        match self.internal_batch_update(channel_id, updates).await {
            Ok(_) => {
                let mut stats = self.stats.lock().await;
                stats.total_updates += update_count as u64;
                stats.batch_updates += 1;
                Ok(())
            }
            Err(e) => {
                let mut stats = self.stats.lock().await;
                stats.storage_errors += 1;
                Err(e)
            }
        }
    }

    async fn update_and_publish(
        &mut self,
        channel_id: u16,
        point_id: u32,
        value: RedisValue,
        telemetry_type: &str,
    ) -> Result<()> {
        let float_value = match value {
            RedisValue::Float(f) => f,
            RedisValue::Integer(i) => i as f64,
            RedisValue::Bool(b) => {
                if b {
                    1.0
                } else {
                    0.0
                }
            }
            RedisValue::String(s) => s.parse::<f64>().unwrap_or(0.0),
            RedisValue::Null => 0.0,
        };

        let update = PluginPointUpdate {
            channel_id,
            point_id,
            value: float_value,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            telemetry_type: telemetry_type.parse().unwrap(), // 需要从字符串转换为枚举
            raw_value: None,
        };

        match self.internal_batch_update(channel_id, vec![update]).await {
            Ok(_) => {
                let mut stats = self.stats.lock().await;
                stats.total_updates += 1;
                stats.single_updates += 1;
                Ok(())
            }
            Err(e) => {
                let mut stats = self.stats.lock().await;
                stats.storage_errors += 1;
                Err(e)
            }
        }
    }

    async fn get_stats(&self) -> StorageStats {
        self.stats.lock().await.clone()
    }
}

// ============================================================================
// 优化的批量同步（来自optimized_sync.rs）
// ============================================================================

/// 优化批量同步配置
#[derive(Debug, Clone)]
pub struct OptimizedSyncConfig {
    /// 批处理大小
    pub batch_size: usize,
    /// 同步间隔（毫秒）
    pub sync_interval_ms: u64,
    /// 最大并发数
    pub max_concurrent: usize,
    /// 启用压缩
    pub enable_compression: bool,
}

impl Default for OptimizedSyncConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            sync_interval_ms: 100,
            max_concurrent: 4,
            enable_compression: false,
        }
    }
}

/// 同步统计信息
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    pub total_synced: u64,
    pub batches_processed: u64,
    pub errors: u64,
    pub last_sync_duration_ms: u64,
    pub average_batch_size: f64,
}

/// 优化的批量同步器
pub struct OptimizedBatchSync {
    config: OptimizedSyncConfig,
    redis_client: Arc<Mutex<AsyncRedisClient>>,
    stats: Arc<Mutex<SyncStats>>,
    pending_updates: Arc<Mutex<Vec<PluginPointUpdate>>>,
}

impl OptimizedBatchSync {
    /// 创建新的批量同步器
    pub fn new(config: OptimizedSyncConfig, redis_client: AsyncRedisClient) -> Self {
        Self {
            config,
            redis_client: Arc::new(Mutex::new(redis_client)),
            stats: Arc::new(Mutex::new(SyncStats::default())),
            pending_updates: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 添加更新到待处理队列
    pub async fn add_update(&self, update: PluginPointUpdate) {
        let mut pending = self.pending_updates.lock().await;
        pending.push(update);

        // 如果达到批处理大小，立即触发同步
        if pending.len() >= self.config.batch_size {
            drop(pending);
            self.sync_batch().await;
        }
    }

    /// 批量添加更新
    pub async fn add_updates(&self, updates: Vec<PluginPointUpdate>) {
        let mut pending = self.pending_updates.lock().await;
        pending.extend(updates);

        // 如果达到批处理大小，立即触发同步
        if pending.len() >= self.config.batch_size {
            drop(pending);
            self.sync_batch().await;
        }
    }

    /// 执行批量同步
    pub async fn sync_batch(&self) {
        let start_time = Instant::now();

        // 获取待处理的更新
        let mut pending = self.pending_updates.lock().await;
        if pending.is_empty() {
            return;
        }

        let batch_size = pending.len().min(self.config.batch_size);
        let updates: Vec<_> = pending.drain(..batch_size).collect();
        drop(pending);

        trace!("Syncing batch of {} updates", updates.len());

        // 按通道ID分组
        let mut updates_by_channel: HashMap<u16, Vec<PluginPointUpdate>> = HashMap::new();
        for update in updates {
            updates_by_channel
                .entry(update.channel_id)
                .or_default()
                .push(update);
        }

        // 并发处理每个通道的更新
        let mut tasks = Vec::new();
        let redis_client = self.redis_client.clone();

        for (channel_id, channel_updates) in updates_by_channel {
            let client = redis_client.clone();
            let task = tokio::spawn(async move {
                Self::sync_channel_updates(client, channel_id, channel_updates).await
            });
            tasks.push(task);

            // 限制并发数
            if tasks.len() >= self.config.max_concurrent {
                for task in tasks.drain(..) {
                    if let Err(e) = task.await {
                        error!("Task failed: {}", e);
                    }
                }
            }
        }

        // 等待剩余任务完成
        for task in tasks {
            if let Err(e) = task.await {
                error!("Task failed: {}", e);
            }
        }

        // 更新统计信息
        let duration = start_time.elapsed().as_millis() as u64;
        let mut stats = self.stats.lock().await;
        stats.batches_processed += 1;
        stats.last_sync_duration_ms = duration;
        stats.average_batch_size = if stats.batches_processed > 0 {
            (stats.total_synced as f64) / (stats.batches_processed as f64)
        } else {
            0.0
        };

        debug!(
            "Batch sync completed in {}ms, processed {} updates",
            duration, batch_size
        );
    }

    /// 同步单个通道的更新
    async fn sync_channel_updates(
        redis_client: Arc<Mutex<AsyncRedisClient>>,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()> {
        let mut client = redis_client.lock().await;
        let mut pipeline = Vec::new();

        for update in &updates {
            let key = format!(
                "{}:{}:{}",
                channel_id,
                telemetry_type_to_redis(&update.telemetry_type),
                update.point_id
            );

            let value_data = serde_json::json!({
                "v": update.value,
                "t": update.timestamp,
            });

            pipeline.push((key, value_data.to_string()));
        }

        // 批量写入Redis
        for (key, value) in pipeline {
            client.set(&key, value).await?;
        }

        // 发布更新通知
        let channel = format!("data:{}:update", channel_id);
        let message = serde_json::json!({
            "count": updates.len(),
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        client.publish(&channel, &message.to_string()).await?;

        Ok(())
    }

    /// 启动定期同步任务
    pub async fn start_periodic_sync(self: Arc<Self>) {
        let sync_interval = Duration::from_millis(self.config.sync_interval_ms);
        let mut ticker = interval(sync_interval);
        let sync_interval_ms = self.config.sync_interval_ms;
        let self_clone = self.clone();

        tokio::spawn(async move {
            loop {
                ticker.tick().await;
                self_clone.sync_batch().await;
            }
        });

        info!("Started periodic sync with interval {}ms", sync_interval_ms);
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> SyncStats {
        self.stats.lock().await.clone()
    }

    /// 强制同步所有待处理的更新
    pub async fn flush(&self) {
        loop {
            let pending_count = {
                let pending = self.pending_updates.lock().await;
                pending.len()
            };

            if pending_count == 0 {
                break;
            }

            self.sync_batch().await;
        }
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 创建带存储的ComBase存储实例
pub fn create_combase_storage(storage: Box<dyn PluginStorage>) -> Box<dyn ComBaseStorage> {
    Box::new(DefaultComBaseStorage::new(storage))
}

/// 创建优化的批量同步器
pub fn create_batch_sync(
    config: OptimizedSyncConfig,
    redis_client: AsyncRedisClient,
) -> Arc<OptimizedBatchSync> {
    Arc::new(OptimizedBatchSync::new(config, redis_client))
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::core::DefaultPluginStorage;

    #[tokio::test]
    async fn test_combase_storage() {
        // 使用默认存储进行测试
        if let Ok(default_storage) = DefaultPluginStorage::from_env().await {
            let plugin_storage = Box::new(default_storage) as Box<dyn PluginStorage>;
            let mut storage = DefaultComBaseStorage::new(plugin_storage);

            // 测试单点更新
            let result = storage
                .update_and_publish(1, 100, RedisValue::Float(42.0), "m")
                .await;
            assert!(result.is_ok());

            // 获取统计信息
            let stats = storage.get_stats().await;
            assert_eq!(stats.total_updates, 1);
            assert_eq!(stats.single_updates, 1);
        }
    }

    #[tokio::test]
    async fn test_batch_sync_config() {
        let config = OptimizedSyncConfig::default();
        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.sync_interval_ms, 100);
        assert_eq!(config.max_concurrent, 4);
        assert!(!config.enable_compression);
    }

    #[tokio::test]
    async fn test_sync_stats() {
        let stats = SyncStats::default();
        assert_eq!(stats.total_synced, 0);
        assert_eq!(stats.batches_processed, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.average_batch_size, 0.0);
    }
}
