//! 简化的 ComBase 存储模块
//!
//! 直接使用 Redis 操作，移除复杂的抽象层

use crate::core::sync::DataSync;
use crate::plugins::core::{telemetry_type_to_redis, PluginPointUpdate};
use crate::storage;
use crate::utils::error::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};
use voltage_libs::redis::RedisClient;

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

/// ComBase 存储管理器
pub struct ComBaseStorage {
    redis_client: Arc<Mutex<RedisClient>>,
    stats: Arc<Mutex<StorageStats>>,
    data_sync: Option<Arc<dyn DataSync>>,
}

impl ComBaseStorage {
    /// 创建新的存储实例
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = storage::create_redis_client(redis_url).await?;

        Ok(Self {
            redis_client: Arc::new(Mutex::new(client)),
            stats: Arc::new(Mutex::new(StorageStats::default())),
            data_sync: None,
        })
    }

    /// 设置数据同步器
    pub fn set_data_sync(&mut self, data_sync: Arc<dyn DataSync>) {
        self.data_sync = Some(data_sync);
    }

    /// 批量更新并发布数据
    pub async fn batch_update_and_publish(
        &self,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let mut stats = self.stats.lock().await;
        stats.batch_updates += 1;
        stats.total_updates += updates.len() as u64;

        // 转换为存储格式
        let storage_updates: Vec<storage::PointUpdate> = updates
            .iter()
            .map(|u| storage::PointUpdate {
                channel_id,
                point_type: telemetry_type_to_redis(&u.telemetry_type).to_string(),
                point_id: u.point_id,
                value: u.value,
            })
            .collect();

        // 批量写入 Redis
        let mut client = self.redis_client.lock().await;
        if let Err(e) = storage::write_batch(&mut client, storage_updates).await {
            warn!("Failed to batch write to Redis: {}", e);
            stats.storage_errors += 1;
            return Err(e);
        }

        // 发布更新
        for update in &updates {
            let point_type = telemetry_type_to_redis(&update.telemetry_type);
            if let Err(e) = storage::publish_update(
                &mut client,
                channel_id,
                point_type,
                update.point_id,
                update.value,
            )
            .await
            {
                warn!("Failed to publish update: {}", e);
                stats.publish_failed += 1;
            } else {
                stats.publish_success += 1;
            }
        }

        // 触发数据同步
        if let Some(ref data_sync) = self.data_sync {
            for update in &updates {
                let point_type = telemetry_type_to_redis(&update.telemetry_type);

                if let Err(e) = data_sync
                    .sync_telemetry(channel_id, point_type, update.point_id, update.value)
                    .await
                {
                    debug!("Sync failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// 单点更新并发布
    pub async fn update_and_publish(
        &self,
        channel_id: u16,
        point_id: u32,
        value: f64,
        telemetry_type: &str,
    ) -> Result<()> {
        let mut stats = self.stats.lock().await;
        stats.single_updates += 1;
        stats.total_updates += 1;

        let mut client = self.redis_client.lock().await;

        // 写入 Redis
        if let Err(e) =
            storage::write_point(&mut client, channel_id, telemetry_type, point_id, value).await
        {
            warn!("Failed to write point: {}", e);
            stats.storage_errors += 1;
            return Err(e);
        }

        // 发布更新
        if let Err(e) =
            storage::publish_update(&mut client, channel_id, telemetry_type, point_id, value).await
        {
            warn!("Failed to publish: {}", e);
            stats.publish_failed += 1;
        } else {
            stats.publish_success += 1;
        }

        // 触发数据同步
        if let Some(ref data_sync) = self.data_sync {
            if let Err(e) = data_sync
                .sync_telemetry(channel_id, telemetry_type, point_id, value)
                .await
            {
                debug!("Sync failed: {}", e);
            }
        }

        Ok(())
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> StorageStats {
        self.stats.lock().await.clone()
    }
}

impl std::fmt::Debug for ComBaseStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComBaseStorage")
            .field("has_data_sync", &self.data_sync.is_some())
            .finish()
    }
}
