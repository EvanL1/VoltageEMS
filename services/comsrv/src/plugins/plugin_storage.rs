//! 插件统一存储接口
//!
//! 为所有协议插件提供统一的Redis存储接口，使用扁平化键值结构

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::framework::types::TelemetryType;
use crate::core::redis::storage::RedisStorage;
use crate::core::redis::types::{
    PointConfig, PointUpdate, TYPE_ADJUSTMENT, TYPE_CONTROL, TYPE_MEASUREMENT, TYPE_SIGNAL,
};
use crate::utils::error::Result;

/// 将TelemetryType转换为Redis存储的类型缩写
pub fn telemetry_type_to_redis(telemetry_type: &TelemetryType) -> &'static str {
    match telemetry_type {
        TelemetryType::Telemetry => TYPE_MEASUREMENT, // YC -> m
        TelemetryType::Signal => TYPE_SIGNAL,         // YX -> s
        TelemetryType::Control => TYPE_CONTROL,       // YK -> c
        TelemetryType::Adjustment => TYPE_ADJUSTMENT, // YT -> a
    }
}

/// 插件存储trait
#[async_trait]
pub trait PluginStorage: Send + Sync {
    /// 写入单个点位数据
    async fn write_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// 批量写入点位数据
    async fn write_points(&self, updates: Vec<PluginPointUpdate>) -> Result<()>;

    /// 读取单个点位数据
    async fn read_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>>;

    /// 写入点位配置
    async fn write_config(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        config: PluginPointConfig,
    ) -> Result<()>;

    /// 初始化点位（创建实时数据键，即使没有值）
    async fn initialize_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<()>;
}

/// 插件点位更新数据
#[derive(Debug, Clone)]
pub struct PluginPointUpdate {
    pub channel_id: u16,
    pub telemetry_type: TelemetryType,
    pub point_id: u32,
    pub value: f64,
}

/// 插件点位配置
#[derive(Debug, Clone)]
pub struct PluginPointConfig {
    pub name: String,
    pub unit: String,
    pub scale: f64,
    pub offset: f64,
    pub address: String,
}

impl From<PluginPointConfig> for PointConfig {
    fn from(config: PluginPointConfig) -> Self {
        PointConfig {
            name: config.name,
            unit: config.unit,
            scale: config.scale,
            offset: config.offset,
            address: config.address,
        }
    }
}

/// 默认的插件存储实现
pub struct DefaultPluginStorage {
    storage: Arc<Mutex<RedisStorage>>,
}

impl DefaultPluginStorage {
    /// 创建新的存储实例
    pub async fn new(redis_url: &str) -> Result<Self> {
        let storage = RedisStorage::new(redis_url).await?;
        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
        })
    }

    /// 从环境变量创建
    pub async fn from_env() -> Result<Self> {
        let redis_url = std::env::var("COMSRV_SERVICE_REDIS_URL")
            .or_else(|_| std::env::var("REDIS_URL"))
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        Self::new(&redis_url).await
    }
}

#[async_trait]
impl PluginStorage for DefaultPluginStorage {
    async fn write_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        let mut storage = self.storage.lock().await;
        storage
            .set_point(channel_id, point_type, point_id, value)
            .await
    }

    async fn write_points(&self, updates: Vec<PluginPointUpdate>) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let redis_updates: Vec<PointUpdate> = updates
            .into_iter()
            .map(|update| PointUpdate {
                channel_id: update.channel_id,
                point_type: telemetry_type_to_redis(&update.telemetry_type),
                point_id: update.point_id,
                value: update.value,
            })
            .collect();

        let mut storage = self.storage.lock().await;
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

    async fn write_config(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        config: PluginPointConfig,
    ) -> Result<()> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        let redis_config = config.into();
        let mut storage = self.storage.lock().await;
        storage
            .set_config(channel_id, point_type, point_id, &redis_config)
            .await
    }

    async fn initialize_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<()> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        let mut storage = self.storage.lock().await;
        // 初始化实时数据键，设置为0值
        storage
            .set_point(channel_id, point_type, point_id, 0.0)
            .await
    }
}

/// 用于测试的Mock存储
#[cfg(test)]
pub struct MockPluginStorage {
    data: Arc<Mutex<std::collections::HashMap<String, (f64, i64)>>>,
}

#[cfg(test)]
impl MockPluginStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl PluginStorage for MockPluginStorage {
    async fn write_point(
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

    async fn write_points(&self, updates: Vec<PluginPointUpdate>) -> Result<()> {
        for update in updates {
            self.write_point(
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

    async fn write_config(
        &self,
        _channel_id: u16,
        _telemetry_type: &TelemetryType,
        _point_id: u32,
        _config: PluginPointConfig,
    ) -> Result<()> {
        Ok(())
    }

    async fn initialize_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<()> {
        self.write_point(channel_id, telemetry_type, point_id, 0.0)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_telemetry_type_conversion() {
        assert_eq!(
            telemetry_type_to_redis(&TelemetryType::Telemetry),
            TYPE_MEASUREMENT
        );
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Signal), TYPE_SIGNAL);
        assert_eq!(
            telemetry_type_to_redis(&TelemetryType::Control),
            TYPE_CONTROL
        );
        assert_eq!(
            telemetry_type_to_redis(&TelemetryType::Adjustment),
            TYPE_ADJUSTMENT
        );
    }

    #[tokio::test]
    async fn test_mock_storage() {
        let storage = MockPluginStorage::new();

        // 写入测试
        storage
            .write_point(1001, &TelemetryType::Telemetry, 10001, 25.6)
            .await
            .unwrap();

        // 读取测试
        let result = storage
            .read_point(1001, &TelemetryType::Telemetry, 10001)
            .await
            .unwrap();
        assert!(result.is_some());
        let (value, _) = result.unwrap();
        assert_eq!(value, 25.6);
    }
}
