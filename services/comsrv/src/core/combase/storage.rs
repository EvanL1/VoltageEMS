//! 框架存储模块
//!
//! `整合了ComBase存储接口和优化的批量同步功能`

use super::core::RedisValue;
use crate::core::sync::{DataSync, LuaSyncManager};
use crate::plugins::core::{telemetry_type_to_redis, PluginPointUpdate, PluginStorage};
use crate::utils::error::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};

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

/// `默认ComBase存储实现`
pub struct DefaultComBaseStorage {
    storage: Arc<Mutex<Box<dyn PluginStorage>>>,
    stats: Arc<Mutex<StorageStats>>,
    sync_manager: Option<Arc<LuaSyncManager>>,
}

impl DefaultComBaseStorage {
    /// 创建新实例
    pub fn new(storage: Box<dyn PluginStorage>) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
            stats: Arc::new(Mutex::new(StorageStats::default())),
            sync_manager: None,
        }
    }

    /// 设置同步管理器
    pub fn set_sync_manager(&mut self, sync_manager: Arc<LuaSyncManager>) {
        self.sync_manager = Some(sync_manager);
    }

    /// 内部批量更新方法
    async fn internal_batch_update(
        &self,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()> {
        let storage = self.storage.lock().await;

        // 执行批量更新
        storage.write_points(updates.clone()).await?;

        // 如果启用了 Lua 同步，异步同步数据
        if let Some(sync_manager) = &self.sync_manager {
            let sync_updates: Vec<(u16, String, u32, f64)> = updates
                .into_iter()
                .map(|update| {
                    (
                        channel_id,
                        telemetry_type_to_redis(&update.telemetry_type).to_string(),
                        update.point_id,
                        update.value,
                    )
                })
                .collect();

            // 异步同步，不阻塞主流程
            if !sync_updates.is_empty() {
                let update_count = sync_updates.len();
                match sync_manager.batch_sync(sync_updates).await {
                    Ok(()) => debug!("Batch sync initiated for {} points", update_count),
                    Err(e) => warn!("Batch sync failed (non-blocking): {}", e),
                }
            }
        }

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
            Ok(()) => {
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
            #[allow(clippy::cast_precision_loss)]
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
            timestamp: i64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            )
            .unwrap_or(i64::MAX),
            telemetry_type: telemetry_type.parse().unwrap(), // 需要从字符串转换为枚举
            raw_value: None,
        };

        match self
            .internal_batch_update(channel_id, vec![update.clone()])
            .await
        {
            Ok(()) => {
                let mut stats = self.stats.lock().await;
                stats.total_updates += 1;
                stats.single_updates += 1;

                // 单点同步（如果启用）
                if let Some(sync_manager) = &self.sync_manager {
                    match sync_manager
                        .sync_measurement(channel_id, telemetry_type, point_id, float_value)
                        .await
                    {
                        Ok(()) => debug!("Single point sync initiated"),
                        Err(e) => warn!("Single point sync failed (non-blocking): {}", e),
                    }
                }

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

impl std::fmt::Debug for DefaultComBaseStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultComBaseStorage")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// `创建带存储的ComBase存储实例`
pub fn create_combase_storage(storage: Box<dyn PluginStorage>) -> Box<dyn ComBaseStorage> {
    Box::new(DefaultComBaseStorage::new(storage))
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
}
