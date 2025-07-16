use crate::config::RedisConfig;
use crate::error::{HisSrvError, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use voltage_common::redis::RedisClient;
use voltage_common::types::{PointData, PointValue};

/// 解析后的通道信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub channel_id: u32,
    pub point_id: u32,
    pub point_type: String, // m, s, c, a
}

impl ChannelInfo {
    /// 从 Redis 键解析通道信息
    /// 格式: {channelID}:{type}:{pointID}
    pub fn from_redis_key(key: &str) -> Option<Self> {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 3 {
            return None;
        }

        let channel_id = parts[0].parse::<u32>().ok()?;
        let point_type = parts[1].to_string();
        let point_id = parts[2].parse::<u32>().ok()?;

        // 验证点类型是否有效
        match point_type.as_str() {
            "m" | "s" | "c" | "a" => Some(Self {
                channel_id,
                point_id,
                point_type,
            }),
            _ => None,
        }
    }

    /// 获取点类型的描述
    pub fn point_type_description(&self) -> &'static str {
        match self.point_type.as_str() {
            "m" => "telemetry",     // 遥测
            "s" => "signal",        // 信号
            "c" => "control",       // 控制
            "a" => "adjustment",    // 调节
            _ => "unknown",
        }
    }
}

/// Redis 消息
#[derive(Debug, Clone)]
pub struct RedisMessage {
    pub key: String,
    pub channel_info: Option<ChannelInfo>,
    pub point_data: Option<PointData>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Redis 订阅器
pub struct RedisSubscriber {
    config: RedisConfig,
    client: Option<RedisClient>,
    redis_client: Option<redis::Client>,
    message_sender: mpsc::UnboundedSender<RedisMessage>,
}

impl RedisSubscriber {
    /// 创建新的 Redis 订阅器
    pub fn new(
        config: RedisConfig,
        message_sender: mpsc::UnboundedSender<RedisMessage>,
    ) -> Self {
        Self {
            config,
            client: None,
            redis_client: None,
            message_sender,
        }
    }

    /// 连接到 Redis
    pub async fn connect(&mut self) -> Result<()> {
        info!("连接到 Redis: {}:{}", self.config.connection.host, self.config.connection.port);

        // 构建 Redis 配置
        let redis_config = voltage_common::redis::RedisConfig {
            host: self.config.connection.host.clone(),
            port: self.config.connection.port,
            password: self.config.connection.password.clone(),
            socket: None,
            database: self.config.connection.database,
            connection_timeout: self.config.connection.timeout_seconds,
            max_retries: 3,
        };

        let url = redis_config.to_url();
        
        // 创建 voltage-common 的客户端用于读取数据
        let client = RedisClient::new(&url).await?;
        
        // 创建原生 Redis 客户端用于 pub/sub
        let native_client = redis::Client::open(url.clone())
            .map_err(|e| HisSrvError::redis(format!("创建 Redis 客户端失败: {}", e)))?;

        // 测试连接
        let ping_result = client.ping().await?;
        if ping_result != "PONG" {
            return Err(HisSrvError::redis("Redis 连接测试失败"));
        }

        info!("Redis 连接成功");
        self.client = Some(client);
        self.redis_client = Some(native_client);
        Ok(())
    }

    /// 开始监听 Redis 键空间通知
    pub async fn start_listening(&mut self) -> Result<()> {
        info!("开始监听 Redis 键空间通知");

        // 启用键空间通知（如果尚未启用）
        self.enable_keyspace_notifications().await?;

        // 获取 Redis 客户端
        let redis_client = self.redis_client.as_ref()
            .ok_or_else(|| HisSrvError::redis("Redis 客户端未初始化"))?
            .clone();
            
        // 获取数据读取客户端
        let data_client = self.client.as_ref()
            .ok_or_else(|| HisSrvError::redis("数据客户端未初始化"))?
            .clone();

        // 创建 pub/sub 连接
        let mut pubsub = redis_client.get_async_pubsub().await
            .map_err(|e| HisSrvError::redis(format!("创建 pub/sub 连接失败: {}", e)))?;

        // 订阅键空间通知
        // 监听数据库 0 中所有键的 SET 事件
        let db = self.config.connection.database;
        let patterns: Vec<String> = self.config.subscription.patterns.iter()
            .map(|pattern| format!("__keyspace@{}__:{}", db, pattern))
            .collect();
            
        for pattern in &patterns {
            pubsub.psubscribe(pattern).await
                .map_err(|e| HisSrvError::redis(format!("订阅模式 {} 失败: {}", pattern, e)))?;
            info!("已订阅模式: {}", pattern);
        }

        // 获取消息发送器
        let message_sender = self.message_sender.clone();

        // 创建消息流
        let mut pubsub_stream = pubsub.on_message();

        // 开始处理消息
        info!("开始处理 Redis 键空间通知...");
        
        while let Some(msg) = pubsub_stream.next().await {
            let channel: String = msg.get_channel_name().to_string();
            let payload: String = msg.get_payload().unwrap_or_default();
            
            // 只处理 SET 事件
            if payload != "set" {
                continue;
            }
            
            // 从通道名中提取键名
            // 格式: __keyspace@0__:1001:m:10001
            if let Some(key) = channel.strip_prefix(&format!("__keyspace@{}__:", db)) {
                debug!("收到 SET 事件: {}", key);
                
                // 解析通道信息
                if let Some(channel_info) = ChannelInfo::from_redis_key(key) {
                    // 读取键值
                    match data_client.get(key).await {
                        Ok(Some(value)) => {
                            // 解析点数据
                            let point_data = match serde_json::from_str::<PointData>(&value) {
                                Ok(data) => Some(data),
                                Err(e) => {
                                    warn!("解析点数据失败 {}: {}", key, e);
                                    // 尝试创建基本的点数据
                                    Some(create_basic_point_data(&value))
                                }
                            };
                            
                            // 创建消息
                            let message = RedisMessage {
                                key: key.to_string(),
                                channel_info: Some(channel_info),
                                point_data,
                                timestamp: Utc::now(),
                            };
                            
                            // 发送消息
                            if let Err(e) = message_sender.send(message) {
                                error!("发送消息失败: {}", e);
                            }
                        }
                        Ok(None) => {
                            debug!("键 {} 不存在", key);
                        }
                        Err(e) => {
                            error!("读取键 {} 失败: {}", key, e);
                        }
                    }
                } else {
                    debug!("无法解析键格式: {}", key);
                }
            }
        }
        
        warn!("Redis 订阅流已结束");
        Ok(())
    }

    /// 启用 Redis 键空间通知
    async fn enable_keyspace_notifications(&self) -> Result<()> {
        if let Some(client) = &self.redis_client {
            // 尝试配置键空间通知
            let mut conn = client.get_multiplexed_async_connection().await
                .map_err(|e| HisSrvError::redis(format!("获取连接失败: {}", e)))?;
                
            // 配置键空间通知 (K = 键空间事件, E = 键事件, A = 所有命令)
            let result: redis::RedisResult<String> = redis::cmd("CONFIG")
                .arg("SET")
                .arg("notify-keyspace-events")
                .arg("KEA")
                .query_async(&mut conn)
                .await;
                
            match result {
                Ok(_) => {
                    info!("成功配置 Redis 键空间通知 (notify-keyspace-events KEA)");
                }
                Err(e) => {
                    warn!("配置键空间通知失败: {}. 请确保 Redis 已配置 notify-keyspace-events KEA", e);
                }
            }
        }
        Ok(())
    }


    /// 获取配置
    pub fn config(&self) -> &RedisConfig {
        &self.config
    }

}

/// 创建基本的 PointData（当无法解析 JSON 时）
fn create_basic_point_data(value_str: &str) -> PointData {
    // 尝试解析为不同类型
    let value = if let Ok(f) = value_str.parse::<f64>() {
        PointValue::Float(f)
    } else if let Ok(i) = value_str.parse::<i64>() {
        PointValue::Int(i)
    } else if let Ok(b) = value_str.parse::<bool>() {
        PointValue::Bool(b)
    } else {
        PointValue::String(value_str.to_string())
    };

    PointData {
        point_id: 0, // 默认点ID
        value,
        timestamp: Utc::now(),
        quality: None, // 实际数据中已无此字段
        metadata: None,
    }
}

/// 创建消息通道
pub fn create_message_channel() -> (mpsc::UnboundedSender<RedisMessage>, mpsc::UnboundedReceiver<RedisMessage>) {
    mpsc::unbounded_channel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_info_from_redis_key() {
        // 测试有效的键格式
        let info = ChannelInfo::from_redis_key("1001:m:10001").unwrap();
        assert_eq!(info.channel_id, 1001);
        assert_eq!(info.point_id, 10001);
        assert_eq!(info.point_type, "m");
        assert_eq!(info.point_type_description(), "telemetry");

        // 测试信号类型
        let info = ChannelInfo::from_redis_key("2002:s:20001").unwrap();
        assert_eq!(info.point_type_description(), "signal");

        // 测试控制类型
        let info = ChannelInfo::from_redis_key("3003:c:30001").unwrap();
        assert_eq!(info.point_type_description(), "control");

        // 测试调节类型
        let info = ChannelInfo::from_redis_key("4004:a:40001").unwrap();
        assert_eq!(info.point_type_description(), "adjustment");

        // 测试无效格式
        assert!(ChannelInfo::from_redis_key("invalid").is_none());
        assert!(ChannelInfo::from_redis_key("1001:m").is_none());
        assert!(ChannelInfo::from_redis_key("1001:x:10001").is_none()); // 无效类型
        assert!(ChannelInfo::from_redis_key("abc:m:10001").is_none()); // 无效通道ID
    }

    #[test]
    fn test_create_basic_point_data() {
        // 测试浮点数
        let point = create_basic_point_data("3.14");
        if let PointValue::Float(f) = point.value {
            assert_eq!(f, 3.14);
        } else {
            panic!("应该解析为浮点数");
        }

        // 测试整数
        let point = create_basic_point_data("42");
        if let PointValue::Int(i) = point.value {
            assert_eq!(i, 42);
        } else {
            panic!("应该解析为整数");
        }

        // 测试布尔值
        let point = create_basic_point_data("true");
        if let PointValue::Bool(b) = point.value {
            assert!(b);
        } else {
            panic!("应该解析为布尔值");
        }

        // 测试字符串
        let point = create_basic_point_data("hello");
        if let PointValue::String(s) = point.value {
            assert_eq!(s, "hello");
        } else {
            panic!("应该解析为字符串");
        }
    }
}