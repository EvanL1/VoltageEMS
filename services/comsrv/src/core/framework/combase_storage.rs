//! ComBase统一存储接口
//!
//! 为ComBase层提供统一的存储接口，集成Redis存储和pub/sub发布功能
//! 实现数据存储和实时发布的一体化操作

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::framework::types::TelemetryType;
use crate::core::redis::storage::RedisStorage;
use crate::plugins::plugin_storage::{telemetry_type_to_redis, PluginPointUpdate};
use crate::utils::error::Result;

/// ComBase层统一存储接口
///
/// 集成Redis存储和pub/sub发布功能，为ComBase实现提供统一的数据操作接口
#[async_trait]
pub trait ComBaseStorage: Send + Sync {
    /// 写入单个点位数据，自动触发pub/sub发布
    async fn store_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// 批量写入点位数据，自动触发批量pub/sub发布
    async fn store_batch(&self, updates: Vec<PluginPointUpdate>) -> Result<()>;

    /// 读取单个点位数据
    async fn read_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>>;

    /// 检查存储连接状态
    async fn is_connected(&self) -> bool;

    /// 关闭存储连接
    async fn close(self);
}

/// 默认的ComBase存储实现
///
/// 包装RedisStorage，提供统一的存储和发布接口
pub struct DefaultComBaseStorage {
    /// Redis存储实例（已集成pub/sub发布功能）
    storage: Arc<Mutex<RedisStorage>>,
}

impl DefaultComBaseStorage {
    /// 创建新的ComBase存储实例
    ///
    /// # Arguments
    /// * `redis_url` - Redis连接URL
    ///
    /// # Returns
    /// * `Result<Self>` - 存储实例或错误
    pub async fn new(redis_url: &str) -> Result<Self> {
        let storage = RedisStorage::new(redis_url).await?;
        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
        })
    }

    /// 从环境变量创建存储实例
    pub async fn from_env() -> Result<Self> {
        let redis_url = std::env::var("COMSRV_SERVICE_REDIS_URL")
            .or_else(|_| std::env::var("REDIS_URL"))
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        Self::new(&redis_url).await
    }

    /// 创建共享的存储实例（用于多个ComBase共享）
    pub fn from_shared_storage(storage: Arc<Mutex<RedisStorage>>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl ComBaseStorage for DefaultComBaseStorage {
    async fn store_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        let mut storage = self.storage.lock().await;

        // RedisStorage的set_point已经集成了pub/sub发布功能
        storage
            .set_point(channel_id, point_type, point_id, value)
            .await
    }

    async fn store_batch(&self, updates: Vec<PluginPointUpdate>) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        // 转换为Redis更新格式
        let redis_updates: Vec<crate::core::redis::types::PointUpdate> = updates
            .into_iter()
            .map(|update| crate::core::redis::types::PointUpdate {
                channel_id: update.channel_id,
                point_type: telemetry_type_to_redis(&update.telemetry_type),
                point_id: update.point_id,
                value: update.value,
            })
            .collect();

        let mut storage = self.storage.lock().await;

        // RedisStorage的set_points已经集成了批量pub/sub发布功能
        storage.set_points(&redis_updates).await
    }

    async fn read_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        let mut storage = self.storage.lock().await;
        storage.get_point(channel_id, point_type, point_id).await
    }

    async fn is_connected(&self) -> bool {
        let mut storage = self.storage.lock().await;
        storage.ping().await.is_ok()
    }

    async fn close(self) {
        // 等待Redis存储完成并关闭发布器
        let storage = Arc::try_unwrap(self.storage)
            .map_err(|_| "Failed to unwrap storage Arc")
            .unwrap()
            .into_inner();
        storage.close().await;
    }
}

/// 用于测试的Mock存储实现
#[cfg(test)]
pub struct MockComBaseStorage {
    data: Arc<Mutex<std::collections::HashMap<String, (f64, i64)>>>,
}

#[cfg(test)]
impl MockComBaseStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl ComBaseStorage for MockComBaseStorage {
    async fn store_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let key = format!(
            "{}:{}:{}",
            channel_id,
            telemetry_type_to_redis(telemetry_type),
            point_id
        );
        let timestamp = chrono::Utc::now().timestamp_millis();
        self.data.lock().await.insert(key, (value, timestamp));
        Ok(())
    }

    async fn store_batch(&self, updates: Vec<PluginPointUpdate>) -> Result<()> {
        for update in updates {
            self.store_point(
                update.channel_id,
                &update.telemetry_type,
                update.point_id,
                update.value,
            )
            .await?;
        }
        Ok(())
    }

    async fn read_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>> {
        let key = format!(
            "{}:{}:{}",
            channel_id,
            telemetry_type_to_redis(telemetry_type),
            point_id
        );
        Ok(self.data.lock().await.get(&key).cloned())
    }

    async fn is_connected(&self) -> bool {
        true
    }

    async fn close(self) {
        // Mock实现：无需操作
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::framework::types::TelemetryType;

    #[tokio::test]
    async fn test_mock_storage() {
        let storage = MockComBaseStorage::new();

        // 测试单点存储
        storage
            .store_point(1001, &TelemetryType::Telemetry, 10001, 25.6)
            .await
            .unwrap();

        // 测试读取
        let result = storage
            .read_point(1001, &TelemetryType::Telemetry, 10001)
            .await
            .unwrap();
        assert!(result.is_some());
        let (value, _) = result.unwrap();
        assert_eq!(value, 25.6);

        // 测试批量存储
        let updates = vec![
            PluginPointUpdate {
                channel_id: 1001,
                telemetry_type: TelemetryType::Signal,
                point_id: 20001,
                value: 1.0,
            },
            PluginPointUpdate {
                channel_id: 1001,
                telemetry_type: TelemetryType::Telemetry,
                point_id: 10002,
                value: 30.5,
            },
        ];

        storage.store_batch(updates).await.unwrap();

        // 验证批量存储结果
        let signal_result = storage
            .read_point(1001, &TelemetryType::Signal, 20001)
            .await
            .unwrap();
        assert!(signal_result.is_some());

        let telemetry_result = storage
            .read_point(1001, &TelemetryType::Telemetry, 10002)
            .await
            .unwrap();
        assert!(telemetry_result.is_some());
    }

    #[tokio::test]
    async fn test_storage_connection() {
        let storage = MockComBaseStorage::new();
        assert!(storage.is_connected().await);
    }
}
