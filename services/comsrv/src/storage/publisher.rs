//! Redis Pub/Sub 发布器

use async_trait::async_trait;
use tokio::sync::mpsc;
use voltage_libs::redis::RedisClient;

use super::PointUpdate;
use crate::utils::error::{ComSrvError, Result};

/// 发布器配置
#[derive(Debug, Clone)]
pub struct PublisherConfig {
    /// 批量发送大小
    pub batch_size: usize,
    /// 批量发送间隔（毫秒）
    pub flush_interval_ms: u64,
    /// 通道缓冲区大小
    pub channel_buffer: usize,
}

impl Default for PublisherConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            flush_interval_ms: 100,
            channel_buffer: 10000,
        }
    }
}

/// 发布器trait
#[async_trait]
pub trait PublishUpdates: Send + Sync {
    /// 发布单个更新
    async fn publish(&self, update: PointUpdate) -> Result<()>;

    /// 批量发布
    async fn publish_batch(&self, updates: Vec<PointUpdate>) -> Result<()>;
}

/// Redis发布器
pub struct Publisher {
    redis_url: String,
    tx: mpsc::Sender<PointUpdate>,
}

impl Publisher {
    /// 创建新的发布器
    pub async fn new(redis_url: String, config: PublisherConfig) -> Result<Self> {
        let (tx, rx) = mpsc::channel(config.channel_buffer);

        // 启动后台发布任务
        let redis_url_clone = redis_url.clone();
        tokio::spawn(async move {
            Self::publish_task(redis_url_clone, rx, config).await;
        });

        Ok(Self { redis_url, tx })
    }

    /// 后台发布任务
    async fn publish_task(
        redis_url: String,
        mut rx: mpsc::Receiver<PointUpdate>,
        config: PublisherConfig,
    ) {
        let mut buffer = Vec::with_capacity(config.batch_size);
        let flush_interval = tokio::time::Duration::from_millis(config.flush_interval_ms);
        let mut interval = tokio::time::interval(flush_interval);

        loop {
            tokio::select! {
                Some(update) = rx.recv() => {
                    buffer.push(update);

                    if buffer.len() >= config.batch_size {
                        if let Err(e) = Self::flush_buffer(&redis_url, &mut buffer).await {
                            tracing::error!("Failed to flush buffer: {}", e);
                        }
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        if let Err(e) = Self::flush_buffer(&redis_url, &mut buffer).await {
                            tracing::error!("Failed to flush buffer: {}", e);
                        }
                    }
                }
                else => {
                    // 通道关闭，退出
                    break;
                }
            }
        }
    }

    /// 刷新缓冲区
    async fn flush_buffer(redis_url: &str, buffer: &mut Vec<PointUpdate>) -> Result<()> {
        if buffer.is_empty() {
            return Ok(());
        }

        let mut client = RedisClient::new(redis_url)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {}", e)))?;

        let mut pipe = redis::pipe();

        for update in buffer.iter() {
            // 发布到通道
            let channel = format!(
                "comsrv:{}:{}:{}",
                update.channel_id, update.point_type, update.point_id
            );
            let message = serde_json::json!({
                "point_id": update.point_id,
                "value": update.data.value,
                "timestamp": update.data.timestamp,
                "quality": update.data.quality,
                "raw_value": update.raw_value,
            })
            .to_string();

            pipe.publish(&channel, &message);
        }

        let conn = client.get_connection_mut();
        pipe.query_async(conn)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to publish batch: {}", e)))?;

        buffer.clear();
        Ok(())
    }
}

#[async_trait]
impl PublishUpdates for Publisher {
    async fn publish(&self, update: PointUpdate) -> Result<()> {
        self.tx
            .send(update)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to send update: {}", e)))?;
        Ok(())
    }

    async fn publish_batch(&self, updates: Vec<PointUpdate>) -> Result<()> {
        for update in updates {
            self.tx
                .send(update)
                .await
                .map_err(|e| ComSrvError::Storage(format!("Failed to send update: {}", e)))?;
        }
        Ok(())
    }
}
