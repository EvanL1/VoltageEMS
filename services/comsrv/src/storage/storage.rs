//! Redis 存储实现

use async_trait::async_trait;
use std::sync::Arc;
use voltage_libs::redis::RedisClient;

use super::{PointData, PointStorage, PointUpdate, PublishUpdates, Publisher, PublisherConfig};
use crate::utils::error::{ComSrvError, Result};

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始重试延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 最大重试延迟（毫秒）
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 2000,
        }
    }
}

/// Redis 存储实现
pub struct Storage {
    redis_url: String,
    retry_config: RetryConfig,
    publisher: Option<Arc<Publisher>>,
}

impl Storage {
    /// 创建新的存储实例
    pub async fn new(redis_url: &str) -> Result<Self> {
        // 测试连接
        let mut client = RedisClient::new(redis_url)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {}", e)))?;
        client
            .ping()
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to ping Redis: {}", e)))?;

        Ok(Self {
            redis_url: redis_url.to_string(),
            retry_config: RetryConfig::default(),
            publisher: None,
        })
    }

    /// 带配置创建
    pub async fn with_config(
        redis_url: &str,
        retry_config: RetryConfig,
        publisher_config: Option<PublisherConfig>,
    ) -> Result<Self> {
        // 测试连接
        let mut client = RedisClient::new(redis_url)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {}", e)))?;
        client
            .ping()
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to ping Redis: {}", e)))?;

        let publisher = if let Some(pub_config) = publisher_config {
            Some(Arc::new(
                Publisher::new(redis_url.to_string(), pub_config).await?,
            ))
        } else {
            None
        };

        Ok(Self {
            redis_url: redis_url.to_string(),
            retry_config,
            publisher,
        })
    }

    /// 获取 Redis 客户端
    async fn get_client(&self) -> Result<RedisClient> {
        RedisClient::new(&self.redis_url)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {}", e)))
    }
}

#[async_trait]
impl PointStorage for Storage {
    async fn write_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let key = format!("{}:{}:{}", channel_id, point_type, point_id);
        let data = PointData::new(value);

        let mut client = self.get_client().await?;
        client
            .set(&key, data.to_redis())
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to write point: {}", e)))?;

        Ok(())
    }

    async fn write_point_with_metadata(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
        raw_value: Option<f64>,
    ) -> Result<()> {
        let key = format!("{}:{}:{}", channel_id, point_type, point_id);
        let data = PointData::new(value);

        let mut client = self.get_client().await?;

        // 使用事务写入多个值
        let mut pipe = redis::pipe();
        pipe.atomic();

        // 写入主值
        pipe.set(&key, data.to_redis());

        // 写入元数据
        if let Some(raw) = raw_value {
            pipe.set(format!("{}:raw", key), format!("{:.6}", raw));
            pipe.set(format!("{}:ts", key), data.timestamp);
        }

        let conn = client.get_connection_mut();
        pipe.query_async(conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to write with metadata: {}", e)))?;

        // 发布更新
        if let Some(ref publisher) = self.publisher {
            let update = PointUpdate::new(channel_id, point_type.to_string(), point_id, value)
                .with_raw_value(raw_value.unwrap_or(value));
            publisher.publish(update).await?;
        }

        Ok(())
    }

    async fn write_batch(&self, updates: Vec<PointUpdate>) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let mut client = self.get_client().await?;
        let mut pipe = redis::pipe();
        pipe.atomic();

        for update in &updates {
            let key = update.key();
            pipe.set(&key, update.data.to_redis());

            if let Some(raw) = update.raw_value {
                pipe.set(format!("{}:raw", key), format!("{:.6}", raw));
                pipe.set(format!("{}:ts", key), update.data.timestamp);
            }
        }

        let conn = client.get_connection_mut();
        pipe.query_async(conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to write batch: {}", e)))?;

        // 批量发布
        if let Some(ref publisher) = self.publisher {
            publisher.publish_batch(updates).await?;
        }

        Ok(())
    }

    async fn read_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<PointData>> {
        let key = format!("{}:{}:{}", channel_id, point_type, point_id);

        let mut client = self.get_client().await?;
        let data: Option<String> = client
            .get(&key)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to read point: {}", e)))?;

        match data {
            Some(value) => {
                let point = PointData::from_redis(&value).map_err(|e| {
                    ComSrvError::Storage(format!("Failed to parse point data: {}", e))
                })?;
                Ok(Some(point))
            }
            None => Ok(None),
        }
    }

    async fn read_points(&self, keys: Vec<String>) -> Result<Vec<Option<PointData>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let mut client = self.get_client().await?;
        let values: Vec<Option<String>> = client
            .mget(&keys.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to read points: {}", e)))?;

        let points: Result<Vec<Option<PointData>>> = values
            .into_iter()
            .map(|opt_value| match opt_value {
                Some(value) => PointData::from_redis(&value).map(Some).map_err(|e| {
                    ComSrvError::Storage(format!("Failed to parse point data: {}", e))
                }),
                None => Ok(None),
            })
            .collect();

        points
    }

    async fn get_channel_points(
        &self,
        channel_id: u16,
        point_type: &str,
    ) -> Result<Vec<(u32, PointData)>> {
        let pattern = format!("{}:{}:*", channel_id, point_type);

        let mut client = self.get_client().await?;

        // 获取所有匹配的键
        let keys: Vec<String> = client
            .keys(&pattern)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to scan keys: {}", e)))?;

        if keys.is_empty() {
            return Ok(vec![]);
        }

        // 批量读取值
        let values: Vec<Option<String>> = client
            .mget(&keys.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to read values: {}", e)))?;

        // 解析结果
        let mut results = Vec::new();
        for (key, value) in keys.iter().zip(values.iter()) {
            if let Some(value_str) = value {
                // 从键中提取点位ID
                if let Some(point_id_str) = key.split(':').nth(2) {
                    if let Ok(point_id) = point_id_str.parse::<u32>() {
                        if let Ok(data) = PointData::from_redis(value_str) {
                            results.push((point_id, data));
                        }
                    }
                }
            }
        }

        Ok(results)
    }
}
