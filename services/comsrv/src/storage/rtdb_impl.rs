//! 实时数据库(RTDB) 实现

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

/// 实时数据库存储实现
pub struct RtdbStorage {
    redis_url: String,
    #[allow(dead_code)]
    retry_config: RetryConfig,
    publisher: Option<Arc<Publisher>>,
}

impl RtdbStorage {
    /// 创建新的实时数据库实例
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
impl PointStorage for RtdbStorage {
    async fn write_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
        let field = point_id.to_string();
        let data = PointData::new(value);

        let mut client = self.get_client().await?;
        client
            .hset(&hash_key, &field, data.to_redis_value())
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
        let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
        let field = point_id.to_string();
        let data = PointData::new(value);

        let mut client = self.get_client().await?;

        // 使用事务写入多个值
        let mut pipe = redis::pipe();
        pipe.atomic();

        // 写入主Hash值
        pipe.hset(&hash_key, &field, data.to_redis_value());

        // 写入元数据（仍使用单独的键）
        if let Some(raw) = raw_value {
            pipe.hset(format!("{}:raw", hash_key), &field, format!("{:.6}", raw));
            pipe.hset(
                format!("{}:ts", hash_key),
                &field,
                data.timestamp.to_string(),
            );
        }

        let conn = client.get_connection_mut();
        let _: () = pipe
            .query_async(conn)
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

        // 按通道和类型分组
        use std::collections::HashMap;
        let mut grouped: HashMap<String, Vec<&PointUpdate>> = HashMap::new();

        for update in &updates {
            let hash_key = format!("comsrv:{}:{}", update.channel_id, update.point_type);
            grouped.entry(hash_key).or_default().push(update);
        }

        // 批量写入每个Hash
        for (hash_key, updates) in grouped {
            for update in updates {
                let field = update.point_id.to_string();
                pipe.hset(&hash_key, &field, update.data.to_redis_value());

                if let Some(raw) = update.raw_value {
                    pipe.hset(format!("{}:raw", hash_key), &field, format!("{:.6}", raw));
                    pipe.hset(
                        format!("{}:ts", hash_key),
                        &field,
                        update.data.timestamp.to_string(),
                    );
                }
            }
        }

        let conn = client.get_connection_mut();
        let _: () = pipe
            .query_async(conn)
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
        let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
        let field = point_id.to_string();

        let mut client = self.get_client().await?;
        let data: Option<String> = client
            .hget(&hash_key, &field)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to read point: {}", e)))?;

        match data {
            Some(value) => {
                let point = PointData::from_redis_value(&value).map_err(|e| {
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
        let mut results = Vec::new();

        // 解析键并按Hash分组
        for key in keys {
            // 期望格式: "comsrv:{channel_id}:{type}:{point_id}"
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() == 4 {
                let hash_key = format!("{}:{}:{}", parts[0], parts[1], parts[2]);
                let field = parts[3];

                let data: Option<String> = client
                    .hget(&hash_key, field)
                    .await
                    .map_err(|e| ComSrvError::Storage(format!("Failed to read point: {}", e)))?;

                match data {
                    Some(value) => {
                        let point = PointData::from_redis_value(&value).map_err(|e| {
                            ComSrvError::Storage(format!("Failed to parse point data: {}", e))
                        })?;
                        results.push(Some(point));
                    }
                    None => results.push(None),
                }
            } else {
                results.push(None);
            }
        }

        Ok(results)
    }

    async fn get_channel_points(
        &self,
        channel_id: u16,
        point_type: &str,
    ) -> Result<Vec<(u32, PointData)>> {
        let hash_key = format!("comsrv:{}:{}", channel_id, point_type);

        let mut client = self.get_client().await?;

        // 使用HGETALL获取所有字段
        let data: std::collections::HashMap<String, String> = client
            .hgetall(&hash_key)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to get channel points: {}", e)))?;

        let mut results = Vec::new();
        for (field, value) in data {
            if let Ok(point_id) = field.parse::<u32>() {
                if let Ok(point_data) = PointData::from_redis_value(&value) {
                    results.push((point_id, point_data));
                }
            }
        }

        Ok(results)
    }
}
