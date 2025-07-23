//! 多服务Redis订阅器 - 支持comsrv/modsrv/alarmsrv/rulesrv

use crate::{config::StandardRedisConfig, error::Result};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use redis::AsyncCommands;
use std::collections::HashMap;
use tokio::sync::mpsc;
use voltage_libs::redis::RedisClient;

/// 多服务Redis消息 - 支持所有服务通道
#[derive(Debug, Clone)]
pub struct StandardRedisMessage {
    /// 通道键: {service}:{channelID}:{type} 或其他格式
    pub channel_key: String,
    /// 解析的服务名 (comsrv/modsrv/alarmsrv/rulesrv)
    pub service: String,
    /// 解析的通道ID (可选)
    pub channel_id: Option<String>,
    /// 数据类型
    pub data_type: String,
    /// 发布消息内容
    pub message: String,
    /// 接收时间戳
    #[allow(dead_code)]
    pub timestamp: DateTime<Utc>,
}

impl StandardRedisMessage {
    /// 从Redis Pub/Sub消息创建多服务标准消息
    pub fn from_redis_message(channel: &str, payload: &str) -> Option<Self> {
        let parts: Vec<&str> = channel.split(':').collect();

        // 支持多种通道格式
        match parts.len() {
            // 格式1: {service}:{channelID}:{type} (comsrv标准格式)
            3 => {
                let service = parts[0].to_string();
                let channel_id = Some(parts[1].to_string());
                let data_type = parts[2].to_string();

                Some(StandardRedisMessage {
                    channel_key: channel.to_string(),
                    service,
                    channel_id,
                    data_type,
                    message: payload.to_string(),
                    timestamp: Utc::now(),
                })
            }
            // 格式2: {service}:{type} (modsrv/alarmsrv/rulesrv格式)
            2 => {
                let service = parts[0].to_string();
                let data_type = parts[1].to_string();

                Some(StandardRedisMessage {
                    channel_key: channel.to_string(),
                    service,
                    channel_id: None,
                    data_type,
                    message: payload.to_string(),
                    timestamp: Utc::now(),
                })
            }
            // 其他格式暂不支持
            _ => None,
        }
    }

    /// 解析发布消息获取点位数据
    /// 消息格式: "{pointID}:{value:.6}"
    pub fn parse_point_data(&self) -> Option<(String, f64)> {
        let parts: Vec<&str> = self.message.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let point_id = parts[0].to_string();
        let value = parts[1].parse::<f64>().ok()?;

        Some((point_id, value))
    }
}

/// 多服务数据获取结果
#[derive(Debug, Clone)]
pub struct HashBatchData {
    /// Hash键或数据键
    pub data_key: String,
    /// 服务名
    pub service: String,
    /// 通道ID (可选)
    pub channel_id: Option<String>,
    /// 数据类型
    pub data_type: String,
    /// 所有字段数据: {pointID: value} 或单个值
    pub fields: HashMap<String, String>,
    /// 获取时间戳
    pub timestamp: DateTime<Utc>,
}

/// 标准化Redis订阅器
pub struct StandardRedisSubscriber {
    client: RedisClient,
    config: StandardRedisConfig,
}

impl StandardRedisSubscriber {
    /// 创建新的标准订阅器
    pub async fn new(config: StandardRedisConfig) -> Result<Self> {
        let url = config.connection.to_url();
        let mut client = RedisClient::new(&url).await?;

        // 测试连接
        client.ping().await?;
        tracing::info!("Redis连接成功: {}", url);

        Ok(Self { client, config })
    }

    /// 开始多服务订阅并发送消息到处理器
    pub async fn start_standard_subscribe(
        mut self,
        message_sender: mpsc::UnboundedSender<StandardRedisMessage>,
        batch_sender: mpsc::UnboundedSender<HashBatchData>,
    ) -> Result<()> {
        let all_patterns = self.config.get_all_patterns();
        tracing::info!("开始多服务订阅Redis模式: {:?}", all_patterns);

        // 创建订阅
        let mut pubsub = self.client.subscribe(&[]).await?;

        // 订阅所有生效的模式
        for pattern in &all_patterns {
            pubsub.psubscribe(pattern).await?;
            tracing::info!("已订阅模式: {}", pattern);
        }

        // 监听消息
        let mut pubsub_stream = pubsub.on_message();
        loop {
            let msg = pubsub_stream.next().await;
            match msg {
                Some(msg) => {
                    let channel = msg.get_channel_name();

                    match msg.get_payload::<String>() {
                        Ok(payload) => {
                            // 解析为多服务标准消息
                            if let Some(standard_msg) =
                                StandardRedisMessage::from_redis_message(channel, &payload)
                            {
                                tracing::debug!(
                                    "收到{}消息: {} -> {}",
                                    standard_msg.service,
                                    standard_msg.channel_key,
                                    standard_msg.message
                                );

                                // 检查服务和数据类型是否支持
                                if !self.config.is_supported_service_type(
                                    &standard_msg.service,
                                    &standard_msg.data_type,
                                ) {
                                    tracing::debug!(
                                        "跳过不支持的服务数据: {}:{}",
                                        standard_msg.service,
                                        standard_msg.data_type
                                    );
                                    continue;
                                }

                                // 发送标准消息到处理器
                                if let Err(e) = message_sender.send(standard_msg.clone()) {
                                    tracing::error!("发送消息到处理器失败: {}", e);
                                    break;
                                }

                                // 触发数据获取
                                self.fetch_and_send_service_data(&standard_msg, &batch_sender)
                                    .await;
                            } else {
                                tracing::warn!("无法解析通道消息格式: {}", channel);
                            }
                        }
                        Err(e) => {
                            tracing::error!("解析Redis消息载荷失败: {}", e);
                        }
                    }
                }
                None => {
                    tracing::warn!("Redis订阅流已关闭");
                    break;
                }
            }
        }

        tracing::info!("Redis订阅器已停止");
        Ok(())
    }

    /// 获取多服务数据并发送
    async fn fetch_and_send_service_data(
        &mut self,
        standard_msg: &StandardRedisMessage,
        batch_sender: &mpsc::UnboundedSender<HashBatchData>,
    ) {
        match standard_msg.service.as_str() {
            "comsrv" => {
                // comsrv使用Hash存储，需要HGETALL获取批量数据
                self.fetch_comsrv_hash_data(standard_msg, batch_sender)
                    .await;
            }
            "modsrv" | "alarmsrv" | "rulesrv" => {
                // 其他服务可能使用单键存储或其他格式
                self.fetch_other_service_data(standard_msg, batch_sender)
                    .await;
            }
            _ => {
                tracing::warn!("不支持的服务类型: {}", standard_msg.service);
            }
        }
    }

    /// 获取comsrv Hash批量数据
    async fn fetch_comsrv_hash_data(
        &mut self,
        standard_msg: &StandardRedisMessage,
        batch_sender: &mpsc::UnboundedSender<HashBatchData>,
    ) {
        // 获取整个Hash的数据 - 使用HGETALL
        match self
            .client
            .get_connection_mut()
            .hgetall::<&str, HashMap<String, String>>(&standard_msg.channel_key)
            .await
        {
            Ok(fields) => {
                if !fields.is_empty() {
                    let batch_data = HashBatchData {
                        data_key: standard_msg.channel_key.clone(),
                        service: standard_msg.service.clone(),
                        channel_id: standard_msg.channel_id.clone(),
                        data_type: standard_msg.data_type.clone(),
                        fields,
                        timestamp: Utc::now(),
                    };

                    tracing::debug!(
                        "获取{}Hash数据: {} -> {} 个字段",
                        standard_msg.service,
                        batch_data.data_key,
                        batch_data.fields.len()
                    );

                    if let Err(e) = batch_sender.send(batch_data) {
                        tracing::error!("发送批量数据失败: {}", e);
                    }
                } else {
                    tracing::debug!("Hash为空: {}", standard_msg.channel_key);
                }
            }
            Err(e) => {
                tracing::error!("获取Hash数据失败 {}: {}", standard_msg.channel_key, e);
            }
        }
    }

    /// 获取其他服务数据
    async fn fetch_other_service_data(
        &mut self,
        standard_msg: &StandardRedisMessage,
        batch_sender: &mpsc::UnboundedSender<HashBatchData>,
    ) {
        // 对于非comsrv服务，可能使用不同的存储结构
        // 这里使用消息内容作为单个数据点
        let mut fields = HashMap::new();

        // 解析消息内容，尝试提取点位数据
        if let Some((point_id, value)) = standard_msg.parse_point_data() {
            fields.insert(point_id, value.to_string());
        } else {
            // 如果无法解析为点位数据，使用整个消息作为单个值
            fields.insert("value".to_string(), standard_msg.message.clone());
        }

        let batch_data = HashBatchData {
            data_key: standard_msg.channel_key.clone(),
            service: standard_msg.service.clone(),
            channel_id: standard_msg.channel_id.clone(),
            data_type: standard_msg.data_type.clone(),
            fields,
            timestamp: Utc::now(),
        };

        tracing::debug!(
            "获取{}数据: {} -> {} 个字段",
            standard_msg.service,
            batch_data.data_key,
            batch_data.fields.len()
        );

        if let Err(e) = batch_sender.send(batch_data) {
            tracing::error!("发送{}数据失败: {}", standard_msg.service, e);
        }
    }
}
