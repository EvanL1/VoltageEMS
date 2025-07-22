use anyhow::Result;
use redis::{aio::PubSub, AsyncCommands};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use crate::engine::executor::RuleExecutor;

/// 订阅的数据更新事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataUpdate {
    /// 数据源标识
    pub source: String,
    /// 数据类型
    pub data_type: String,
    /// 新值
    pub value: serde_json::Value,
    /// 时间戳
    pub timestamp: i64,
    /// 附加元数据
    pub metadata: Option<HashMap<String, String>>,
}

/// Redis 订阅器
pub struct RedisSubscriber {
    redis_client: redis::Client,
    rule_executor: Arc<RuleExecutor>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    subscriptions: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl RedisSubscriber {
    /// 创建新的订阅器
    pub fn new(redis_url: &str, rule_executor: Arc<RuleExecutor>) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;

        Ok(Self {
            redis_client,
            rule_executor,
            shutdown_tx: None,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 启动订阅器
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Redis subscriber");

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        // 创建订阅连接
        let mut pubsub = self.redis_client.get_async_pubsub().await?;

        // 订阅默认通道
        self.subscribe_default_channels(&mut pubsub).await?;

        // 克隆必要的引用
        let rule_executor = Arc::clone(&self.rule_executor);
        let subscriptions = Arc::clone(&self.subscriptions);

        // 启动订阅处理任务
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut pubsub_stream = pubsub.on_message();

            loop {
                tokio::select! {
                    Some(msg) = pubsub_stream.next() => {
                        if let Err(e) = Self::handle_message(msg, &rule_executor).await {
                            error!("Error handling message: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Redis subscriber shutting down");
                        break;
                    }
                }
            }
        });

        info!("Redis subscriber started");
        Ok(())
    }

    /// 停止订阅器
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Redis subscriber");

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        Ok(())
    }

    /// 订阅默认通道
    async fn subscribe_default_channels(&self, pubsub: &mut PubSub) -> Result<()> {
        // 使用模式订阅来支持通配符
        // 订阅 modsrv 模型输出
        pubsub.psubscribe("modsrv:outputs:*").await?;

        // 订阅告警事件
        pubsub.psubscribe("alarm:event:*").await?;

        info!("Subscribed to default channels");

        // 更新订阅记录
        let mut subs = self.subscriptions.write().await;
        subs.insert("modsrv".to_string(), vec!["modsrv:outputs:*".to_string()]);
        subs.insert("alarm".to_string(), vec!["alarm:event:*".to_string()]);

        Ok(())
    }

    /// 添加自定义订阅
    pub async fn add_subscription(&self, pattern: &str) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        conn.subscribe(pattern).await?;

        info!("Added subscription: {}", pattern);

        // 更新订阅记录
        let mut subs = self.subscriptions.write().await;
        subs.entry("custom".to_string())
            .or_insert_with(Vec::new)
            .push(pattern.to_string());

        Ok(())
    }

    /// 移除订阅
    pub async fn remove_subscription(&self, pattern: &str) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        conn.unsubscribe(pattern).await?;

        info!("Removed subscription: {}", pattern);

        // 更新订阅记录
        let mut subs = self.subscriptions.write().await;
        for (_, patterns) in subs.iter_mut() {
            patterns.retain(|p| p != pattern);
        }

        Ok(())
    }

    /// 处理消息
    async fn handle_message(msg: redis::Msg, rule_executor: &Arc<RuleExecutor>) -> Result<()> {
        let channel = msg.get_channel_name();
        let payload: String = msg.get_payload()?;

        debug!("Received message on channel {}: {}", channel, payload);

        // 解析数据更新
        let update = Self::parse_update(channel, &payload)?;

        // 创建规则上下文
        let mut context = HashMap::new();
        context.insert(
            "source".to_string(),
            serde_json::Value::String(update.source.clone()),
        );
        context.insert(
            "data_type".to_string(),
            serde_json::Value::String(update.data_type.clone()),
        );
        context.insert("value".to_string(), update.value.clone());
        context.insert(
            "timestamp".to_string(),
            serde_json::Value::Number(update.timestamp.into()),
        );

        if let Some(metadata) = &update.metadata {
            for (key, value) in metadata {
                context.insert(
                    format!("metadata.{}", key),
                    serde_json::Value::String(value.clone()),
                );
            }
        }

        // 触发规则评估
        // 获取所有规则并检查是否需要执行
        match rule_executor.list_rules().await {
            Ok(rules) => {
                for rule in rules {
                    if rule.enabled {
                        // 将context转换为JSON对象
                        let context_value =
                            serde_json::Value::Object(context.clone().into_iter().collect());

                        // 执行简单规则
                        match rule_executor
                            .execute_simple_rule(&rule, &context_value)
                            .await
                        {
                            Ok(triggered) => {
                                if triggered {
                                    info!("Rule '{}' triggered on channel {}", rule.name, channel);
                                }
                            }
                            Err(e) => {
                                error!("Error executing rule '{}': {}", rule.id, e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error listing rules: {}", e);
            }
        }

        Ok(())
    }

    /// 解析更新数据
    fn parse_update(channel: &str, payload: &str) -> Result<DataUpdate> {
        // 根据通道类型解析数据
        let parts: Vec<&str> = channel.split(':').collect();

        let (source, data_type) = match parts.as_slice() {
            ["modsrv", "outputs", model_id] => {
                (format!("modsrv:{}", model_id), "model_output".to_string())
            }
            ["alarm", "event", alarm_id] => {
                (format!("alarm:{}", alarm_id), "alarm_event".to_string())
            }
            _ => (channel.to_string(), "unknown".to_string()),
        };

        // 尝试解析 JSON 负载
        let value = match serde_json::from_str::<serde_json::Value>(payload) {
            Ok(v) => v,
            Err(_) => {
                // 如果不是 JSON，作为字符串处理
                serde_json::Value::String(payload.to_string())
            }
        };

        Ok(DataUpdate {
            source,
            data_type,
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
            metadata: None,
        })
    }

    /// 获取当前订阅列表
    pub async fn get_subscriptions(&self) -> HashMap<String, Vec<String>> {
        self.subscriptions.read().await.clone()
    }
}

/// 批量数据获取器
pub struct BatchDataFetcher {
    redis_client: redis::Client,
}

impl BatchDataFetcher {
    /// 创建新的批量获取器
    pub fn new(redis_url: &str) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;
        Ok(Self { redis_client })
    }

    /// 批量获取点位数据
    pub async fn fetch_points(&self, point_ids: &[String]) -> Result<HashMap<String, f64>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let mut result = HashMap::new();

        // 使用 pipeline 批量获取
        let mut pipe = redis::pipe();
        for point_id in point_ids {
            pipe.get(point_id);
        }

        let values: Vec<Option<String>> = pipe.query_async(&mut conn).await?;

        for (i, value) in values.iter().enumerate() {
            if let Some(v) = value {
                // 解析值（格式：value:timestamp）
                if let Some(val_str) = v.split(':').next() {
                    if let Ok(val) = val_str.parse::<f64>() {
                        result.insert(point_ids[i].clone(), val);
                    }
                }
            }
        }

        Ok(result)
    }

    /// 获取模型输出
    pub async fn fetch_model_outputs(
        &self,
        model_id: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 获取模型输出 hash
        let key = format!("modsrv:output:{}", model_id);
        let outputs: HashMap<String, String> = conn.hgetall(&key).await?;

        let mut result = HashMap::new();
        for (field, value) in outputs {
            if let Ok(v) = serde_json::from_str(&value) {
                result.insert(field, v);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_update() {
        let update = RedisSubscriber::parse_update(
            "modsrv:outputs:model1",
            r#"{"value": 42.5, "unit": "kW"}"#,
        )
        .unwrap();

        assert_eq!(update.source, "modsrv:model1");
        assert_eq!(update.data_type, "model_output");
        assert_eq!(update.value["value"], 42.5);
    }

    #[tokio::test]
    async fn test_parse_update_non_json() {
        let update = RedisSubscriber::parse_update("point:update:10001", "25.6").unwrap();

        assert_eq!(update.source, "point:10001");
        assert_eq!(update.data_type, "point_value");
        assert_eq!(update.value, serde_json::Value::String("25.6".to_string()));
    }
}
