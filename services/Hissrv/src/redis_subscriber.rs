use crate::config::RedisConfig;
use crate::error::{HisSrvError, Result};
use crate::storage::{DataPoint, DataValue};
use chrono::Utc;
use futures_util::StreamExt;
use redis::aio::PubSub;
use redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use voltage_common::redis::{RedisClient, RedisConfig as CommonRedisConfig};
use voltage_common::types::PointData;

/// 消息类型，用于区分不同的数据类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// 遥测数据 (YC)
    Telemetry,
    /// 信号数据 (YX)
    Signal,
    /// 控制数据 (YK)
    Control,
    /// 调节数据 (YT)
    Adjustment,
    /// 计算数据
    Calculated,
    /// 事件数据
    Event,
    /// 系统状态
    SystemStatus,
}

impl MessageType {
    /// 从通道名称解析消息类型
    pub fn from_channel(channel: &str) -> Option<Self> {
        let parts: Vec<&str> = channel.split(':').collect();
        
        // 新的统一格式: comsrv:{channelID}:{type}:{pointID}
        if parts.len() >= 4 && parts[0] == "comsrv" {
            match parts[2] {
                "m" => Some(MessageType::Telemetry),     // 测量(YC)
                "s" => Some(MessageType::Signal),        // 信号(YX)
                "c" => Some(MessageType::Control),       // 控制(YK)
                "a" => Some(MessageType::Adjustment),    // 调节(YT)
                "calc" => Some(MessageType::Calculated), // 计算点
                _ => None,
            }
        } else if channel.starts_with("event:") {
            Some(MessageType::Event)
        } else if channel.starts_with("system:") {
            Some(MessageType::SystemStatus)
        } else {
            None
        }
    }
}

/// 解析后的通道信息
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub channel_id: u32,
    pub message_type: MessageType,
    pub point_id: u32,
}

impl ChannelInfo {
    /// 从通道名称解析通道信息
    pub fn from_channel(channel: &str) -> Option<Self> {
        let parts: Vec<&str> = channel.split(':').collect();
        
        // 新的统一格式: comsrv:{channelID}:{type}:{pointID}
        if parts.len() == 4 && parts[0] == "comsrv" {
            let channel_id = parts[1].parse::<u32>().ok()?;
            let message_type = MessageType::from_channel(channel)?;
            let point_id = parts[3].parse::<u32>().ok()?;

            Some(ChannelInfo {
                channel_id,
                message_type,
                point_id,
            })
        } else {
            None
        }
    }
}

/// 订阅消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionMessage {
    pub id: String,
    pub channel: String,
    pub channel_info: Option<ChannelInfo>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub point_data: Option<PointData>,
    pub raw_data: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Redis 订阅器状态
#[derive(Debug, Clone, PartialEq)]
pub enum SubscriberState {
    /// 未连接
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 订阅中
    Subscribing,
    /// 运行中
    Running,
    /// 停止中
    Stopping,
    /// 已停止
    Stopped,
}

/// Redis 订阅器配置
#[derive(Debug, Clone)]
pub struct SubscriberConfig {
    /// 最大重连次数
    pub max_reconnect_attempts: u32,
    /// 重连延迟（毫秒）
    pub reconnect_delay_ms: u64,
    /// 批量处理大小
    pub batch_size: usize,
    /// 批量处理超时（毫秒）
    pub batch_timeout_ms: u64,
    /// 是否启用模式匹配订阅
    pub enable_pattern_subscribe: bool,
}

impl Default for SubscriberConfig {
    fn default() -> Self {
        Self {
            max_reconnect_attempts: 10,
            reconnect_delay_ms: 1000,
            batch_size: 100,
            batch_timeout_ms: 100,
            enable_pattern_subscribe: true,
        }
    }
}

/// 增强的 Redis 订阅器
pub struct EnhancedRedisSubscriber {
    client: Option<RedisClient>,
    pubsub: Option<PubSub>,
    config: RedisConfig,
    subscriber_config: SubscriberConfig,
    message_sender: mpsc::UnboundedSender<SubscriptionMessage>,
    state: Arc<RwLock<SubscriberState>>,
    reconnect_attempts: Arc<RwLock<u32>>,
    subscribed_channels: Arc<RwLock<Vec<String>>>,
    subscribed_patterns: Arc<RwLock<Vec<String>>>,
}

impl EnhancedRedisSubscriber {
    pub fn new(
        config: RedisConfig,
        subscriber_config: SubscriberConfig,
        message_sender: mpsc::UnboundedSender<SubscriptionMessage>,
    ) -> Self {
        Self {
            client: None,
            pubsub: None,
            config,
            subscriber_config,
            message_sender,
            state: Arc::new(RwLock::new(SubscriberState::Disconnected)),
            reconnect_attempts: Arc::new(RwLock::new(0)),
            subscribed_channels: Arc::new(RwLock::new(Vec::new())),
            subscribed_patterns: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 获取当前状态
    pub async fn get_state(&self) -> SubscriberState {
        self.state.read().await.clone()
    }

    /// 连接到 Redis
    pub async fn connect(&mut self) -> Result<()> {
        *self.state.write().await = SubscriberState::Connecting;

        let conn_config = &self.config.connection;
        let redis_config = self.build_redis_config(conn_config);

        let url = redis_config.to_url();
        let client = RedisClient::new(&url)
            .await
            .map_err(|e| HisSrvError::ConnectionError {
                message: format!("Failed to create Redis client: {}", e),
                endpoint: url.clone(),
                retry_count: 0,
            })?;

        // 测试连接
        let ping_result = client
            .ping()
            .await
            .map_err(|e| HisSrvError::ConnectionError {
                message: format!("Redis ping failed: {}", e),
                endpoint: url.clone(),
                retry_count: 0,
            })?;

        if ping_result != "PONG" {
            return Err(HisSrvError::ConnectionError {
                message: "Redis connection test failed".to_string(),
                endpoint: url.clone(),
                retry_count: 0,
            });
        }

        info!("Redis subscriber connected successfully");
        self.client = Some(client);
        *self.state.write().await = SubscriberState::Connected;
        *self.reconnect_attempts.write().await = 0;

        Ok(())
    }

    /// 断开连接
    pub async fn disconnect(&mut self) -> Result<()> {
        *self.state.write().await = SubscriberState::Stopping;

        // 清理资源
        self.pubsub = None;
        self.client = None;
        self.subscribed_channels.write().await.clear();
        self.subscribed_patterns.write().await.clear();

        *self.state.write().await = SubscriberState::Stopped;
        info!("Redis subscriber disconnected");

        Ok(())
    }

    /// 重连逻辑
    async fn reconnect(&mut self) -> Result<()> {
        let mut attempts = self.reconnect_attempts.write().await;
        *attempts += 1;

        if *attempts > self.subscriber_config.max_reconnect_attempts {
            error!("Max reconnection attempts reached");
            return Err(HisSrvError::ConnectionError {
                message: "Max reconnection attempts exceeded".to_string(),
                endpoint: "redis".to_string(),
                retry_count: *attempts,
            });
        }

        warn!(
            "Attempting to reconnect... (attempt {}/{})",
            *attempts, self.subscriber_config.max_reconnect_attempts
        );

        // 指数退避
        let delay =
            Duration::from_millis(self.subscriber_config.reconnect_delay_ms * (*attempts as u64));
        sleep(delay).await;

        // 尝试重新连接
        drop(attempts); // 释放锁
        self.connect().await?;

        // 重新订阅之前的通道
        self.resubscribe().await?;

        Ok(())
    }

    /// 重新订阅所有通道
    async fn resubscribe(&mut self) -> Result<()> {
        if self.client.is_none() {
            return Err(HisSrvError::ConnectionError {
                message: "Cannot resubscribe without active connection".to_string(),
                endpoint: "redis".to_string(),
                retry_count: 0,
            });
        }

        let client = self.client.as_ref().unwrap();
        let mut pubsub = client
            .subscribe(&[])
            .await
            .map_err(|e| HisSrvError::RedisError(format!("Failed to create pubsub: {}", e)))?;

        // 重新订阅通道
        let channels = self.subscribed_channels.read().await.clone();
        for channel in channels {
            pubsub
                .subscribe(&channel)
                .await
                .map_err(|e| HisSrvError::RedisError(format!("Failed to subscribe: {}", e)))?;
            info!("Resubscribed to channel: {}", channel);
        }

        // 重新订阅模式
        let patterns = self.subscribed_patterns.read().await.clone();
        for pattern in patterns {
            pubsub.psubscribe(&pattern).await.map_err(|e| {
                HisSrvError::RedisError(format!("Failed to pattern subscribe: {}", e))
            })?;
            info!("Resubscribed to pattern: {}", pattern);
        }

        self.pubsub = Some(pubsub);
        Ok(())
    }

    /// 订阅通道
    pub async fn subscribe_channels(&mut self, channels: Vec<String>) -> Result<()> {
        *self.state.write().await = SubscriberState::Subscribing;

        if self.client.is_none() {
            return Err(HisSrvError::ConnectionError {
                message: "Not connected to Redis".to_string(),
                endpoint: "redis".to_string(),
                retry_count: 0,
            });
        }

        let client = self.client.as_ref().unwrap();
        let mut pubsub = if let Some(ps) = self.pubsub.take() {
            ps
        } else {
            client
                .subscribe(&[])
                .await
                .map_err(|e| HisSrvError::RedisError(format!("Failed to create pubsub: {}", e)))?
        };

        // 订阅通道
        for channel in &channels {
            if self.subscriber_config.enable_pattern_subscribe && channel.contains('*') {
                // 模式订阅
                pubsub.psubscribe(channel).await.map_err(|e| {
                    HisSrvError::RedisError(format!("Failed to pattern subscribe: {}", e))
                })?;
                self.subscribed_patterns.write().await.push(channel.clone());
                info!("Pattern subscribed to: {}", channel);
            } else {
                // 普通订阅
                pubsub
                    .subscribe(channel)
                    .await
                    .map_err(|e| HisSrvError::RedisError(format!("Failed to subscribe: {}", e)))?;
                self.subscribed_channels.write().await.push(channel.clone());
                info!("Subscribed to channel: {}", channel);
            }
        }

        self.pubsub = Some(pubsub);
        Ok(())
    }

    /// 开始监听
    pub async fn start_listening(&mut self) -> Result<()> {
        *self.state.write().await = SubscriberState::Running;
        info!("Starting Redis subscription listener");

        loop {
            match self.get_state().await {
                SubscriberState::Stopped | SubscriberState::Stopping => {
                    info!("Subscriber stopping");
                    break;
                }
                _ => {}
            }

            if let Some(pubsub) = self.pubsub.take() {
                match self.listen_with_reconnect(pubsub).await {
                    Ok(_) => break,
                    Err(e) => {
                        error!("Subscription error: {}", e);
                        // 尝试重连
                        if let Err(reconnect_err) = self.reconnect().await {
                            error!("Reconnection failed: {}", reconnect_err);
                            break;
                        }
                    }
                }
            } else {
                error!("No active pubsub connection");
                break;
            }
        }

        Ok(())
    }

    /// 监听消息并处理重连
    async fn listen_with_reconnect(&mut self, mut pubsub: PubSub) -> Result<()> {
        let mut pubsub_stream = pubsub.into_on_message();
        let mut batch = Vec::new();
        let mut batch_timer = interval(Duration::from_millis(
            self.subscriber_config.batch_timeout_ms,
        ));

        loop {
            tokio::select! {
                // 接收消息
                msg = pubsub_stream.next() => {
                    match msg {
                        Some(msg) => {
                            if let Err(e) = self.process_message(msg, &mut batch).await {
                                warn!("Failed to process message: {}", e);
                            }

                            // 检查批量大小
                            if batch.len() >= self.subscriber_config.batch_size {
                                self.flush_batch(&mut batch).await;
                            }
                        }
                        None => {
                            warn!("PubSub stream ended");
                            return Err(HisSrvError::ConnectionError {
                                message: "PubSub stream closed".to_string(),
                                endpoint: "redis".to_string(),
                                retry_count: 0,
                            });
                        }
                    }
                }
                // 批量超时
                _ = batch_timer.tick() => {
                    if !batch.is_empty() {
                        self.flush_batch(&mut batch).await;
                    }
                }
            }
        }
    }

    /// 处理单个消息
    async fn process_message(
        &self,
        msg: redis::Msg,
        batch: &mut Vec<SubscriptionMessage>,
    ) -> Result<()> {
        let channel_name = msg.get_channel_name();
        match msg.get_payload::<String>() {
            Ok(payload) => {
                debug!("Received message on channel {}: {}", channel_name, payload);

                let subscription_msg = self.parse_message(&channel_name, &payload)?;
                batch.push(subscription_msg);
            }
            Err(e) => {
                error!("Error parsing message payload from Redis: {}", e);
            }
        }

        Ok(())
    }

    /// 解析消息
    fn parse_message(&self, channel: &str, payload: &str) -> Result<SubscriptionMessage> {
        let channel_info = ChannelInfo::from_channel(channel);

        // 尝试解析为 PointData
        let point_data = if let Ok(pd) = serde_json::from_str::<PointData>(payload) {
            Some(pd)
        } else {
            None
        };

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "redis_subscriber".to_string());

        // 添加通道信息到元数据
        if let Some(ref info) = channel_info {
            metadata.insert("channel_id".to_string(), info.channel_id.to_string());
            metadata.insert("point_id".to_string(), info.point_id.to_string());
            metadata.insert(
                "message_type".to_string(),
                format!("{:?}", info.message_type),
            );
        }

        Ok(SubscriptionMessage {
            id: Uuid::new_v4().to_string(),
            channel: channel.to_string(),
            channel_info,
            timestamp: Utc::now(),
            point_data,
            raw_data: if point_data.is_none() {
                Some(payload.to_string())
            } else {
                None
            },
            metadata,
        })
    }

    /// 批量发送消息
    async fn flush_batch(&self, batch: &mut Vec<SubscriptionMessage>) {
        if batch.is_empty() {
            return;
        }

        debug!("Flushing batch of {} messages", batch.len());

        for msg in batch.drain(..) {
            if let Err(e) = self.message_sender.send(msg) {
                error!("Failed to send message to processor: {}", e);
            }
        }
    }

    /// 构建 Redis 配置
    fn build_redis_config(
        &self,
        conn_config: &crate::config::RedisConnection,
    ) -> CommonRedisConfig {
        if !conn_config.socket.is_empty() {
            CommonRedisConfig {
                host: String::new(),
                port: 0,
                password: if conn_config.password.is_empty() {
                    None
                } else {
                    Some(conn_config.password.clone())
                },
                socket: Some(conn_config.socket.clone()),
                database: conn_config.database,
                connection_timeout: conn_config.timeout_seconds,
                max_retries: conn_config.max_retries,
            }
        } else {
            CommonRedisConfig {
                host: conn_config.host.clone(),
                port: conn_config.port,
                password: if conn_config.password.is_empty() {
                    None
                } else {
                    Some(conn_config.password.clone())
                },
                socket: None,
                database: conn_config.database,
                connection_timeout: conn_config.timeout_seconds,
                max_retries: conn_config.max_retries,
            }
        }
    }

    /// 优雅关闭
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Redis subscriber");
        *self.state.write().await = SubscriberState::Stopping;

        // 等待一小段时间让正在处理的消息完成
        sleep(Duration::from_millis(100)).await;

        self.disconnect().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_from_channel() {
        assert!(matches!(
            MessageType::from_channel("comsrv:1001:m:10001"),
            Some(MessageType::Telemetry)
        ));
        assert!(matches!(
            MessageType::from_channel("comsrv:1001:s:20001"),
            Some(MessageType::Signal)
        ));
        assert!(matches!(
            MessageType::from_channel("comsrv:1001:c:30001"),
            Some(MessageType::Control)
        ));
        assert!(matches!(
            MessageType::from_channel("comsrv:1001:a:40001"),
            Some(MessageType::Adjustment)
        ));
        assert!(matches!(
            MessageType::from_channel("event:alarm"),
            Some(MessageType::Event)
        ));
        assert!(matches!(
            MessageType::from_channel("system:status"),
            Some(MessageType::SystemStatus)
        ));
    }

    #[test]
    fn test_channel_info_parsing() {
        let info = ChannelInfo::from_channel("comsrv:1001:m:10001").unwrap();
        assert_eq!(info.channel_id, 1001);
        assert!(matches!(info.message_type, MessageType::Telemetry));
        assert_eq!(info.point_id, 10001);

        assert!(ChannelInfo::from_channel("invalid").is_none());
        assert!(ChannelInfo::from_channel("comsrv:1001:x:10001").is_none());
        assert!(ChannelInfo::from_channel("1001:m:10001").is_none()); // 旧格式应该失败
    }
}
