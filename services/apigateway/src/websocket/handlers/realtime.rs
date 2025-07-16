use actix::{Actor, Addr, Context, AsyncContext, Handler, Message};
use log::{debug, info, error, warn};
use std::sync::Arc;

use crate::redis_client::RedisClient;
use crate::websocket::hub::WsHub;
use crate::websocket::protocol::{DataType, InternalMessage, WsMessage};

/// Redis消息
#[derive(Message, Debug)]
#[rtype(result = "()")]
struct RedisMessage {
    channel: String,
    payload: String,
}

/// Redis订阅器Actor - 负责订阅Redis通道并转发数据到WebSocket
pub struct RedisSubscriber {
    hub: Addr<WsHub>,
    redis: Arc<RedisClient>,
}

impl RedisSubscriber {
    pub fn new(hub: Addr<WsHub>, redis: Arc<RedisClient>) -> Self {
        Self { hub, redis }
    }
    
    /// 启动订阅任务
    pub fn start(hub: Addr<WsHub>, redis: Arc<RedisClient>) -> Addr<Self> {
        RedisSubscriber::new(hub, redis).start()
    }
    
    /// 处理Redis消息并转发到WebSocket
    fn handle_redis_message(&self, channel: &str, payload: &str) {
        debug!("Received Redis message on channel: {}", channel);
        
        // 解析通道名称格式: {channel_id}:{type}:{point_id}
        let parts: Vec<&str> = channel.split(':').collect();
        
        if parts.len() >= 3 {
            // 常规数据通道
            if let (Ok(channel_id), Ok(point_id)) = (parts[0].parse::<u32>(), parts[2].parse::<u32>()) {
                let data_type = match parts[1] {
                    "m" => DataType::Telemetry,
                    "s" => DataType::Signal,
                    "c" => DataType::Control,
                    "a" => DataType::Adjustment,
                    _ => return,
                };
                
                // 解析数据并创建WebSocket消息
                if let Ok(point_data) = serde_json::from_str::<serde_json::Value>(payload) {
                    let ws_message = match data_type {
                        DataType::Telemetry => {
                            if let Some(value) = point_data["value"].as_f64() {
                                WsMessage::TelemetryData {
                                    channel_id,
                                    point_id,
                                    value,
                                    quality: point_data["quality"].as_u64().unwrap_or(192) as u8,
                                    timestamp: point_data["timestamp"].as_i64().unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
                                }
                            } else {
                                return;
                            }
                        }
                        DataType::Signal => {
                            if let Some(value) = point_data["value"].as_bool() {
                                WsMessage::SignalData {
                                    channel_id,
                                    point_id,
                                    value,
                                    quality: point_data["quality"].as_u64().unwrap_or(192) as u8,
                                    timestamp: point_data["timestamp"].as_i64().unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
                                }
                            } else {
                                return;
                            }
                        }
                        _ => return, // 其他类型暂不处理
                    };
                    
                    // 广播给订阅者
                    self.hub.do_send(InternalMessage::Broadcast {
                        channel_id,
                        data_type,
                        message: ws_message,
                    });
                }
            }
        } else if channel.starts_with("alarm:") {
            // 告警通道
            self.handle_alarm_message(channel, payload);
        }
    }
    
    /// 处理告警消息
    fn handle_alarm_message(&self, channel: &str, payload: &str) {
        if let Ok(alarm_data) = serde_json::from_str::<serde_json::Value>(payload) {
            // 解析告警数据
            let alarm_id = alarm_data["alarm_id"].as_str().unwrap_or("").to_string();
            let channel_id = alarm_data["channel_id"].as_u64().unwrap_or(0) as u32;
            let point_id = alarm_data["point_id"].as_u64().unwrap_or(0) as u32;
            let level = alarm_data["level"].as_str().unwrap_or("info").to_string();
            let message = alarm_data["message"].as_str().unwrap_or("").to_string();
            let timestamp = alarm_data["timestamp"].as_i64().unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            
            let ws_message = WsMessage::AlarmEvent {
                alarm_id,
                channel_id,
                point_id,
                level,
                message,
                timestamp,
            };
            
            // 广播告警给所有订阅告警的会话
            self.hub.do_send(InternalMessage::Broadcast {
                channel_id,
                data_type: DataType::Alarm,
                message: ws_message,
            });
        }
    }
}

impl Actor for RedisSubscriber {
    type Context = Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Redis subscriber actor started");
        
        let redis = self.redis.clone();
        let self_addr = ctx.address();
        
        // 启动Redis订阅任务
        tokio::spawn(async move {
            if let Err(e) = start_redis_subscription(redis, self_addr).await {
                error!("Redis subscription error: {}", e);
            }
        });
    }
}

/// 启动Redis订阅
async fn start_redis_subscription(
    redis: Arc<RedisClient>,
    subscriber_addr: Addr<RedisSubscriber>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Redis pub/sub connection");
    
    // 获取PubSub连接
    let mut pubsub = redis.subscribe(&[]).await
        .map_err(|e| format!("Failed to create pubsub connection: {}", e))?;
    
    // 订阅数据通道（使用模式匹配）
    // 订阅所有测量数据
    if let Err(e) = pubsub.psubscribe("*:m:*").await {
        warn!("Failed to subscribe to measurement pattern: {}", e);
    } else {
        info!("Subscribed to measurement data pattern: *:m:*");
    }
    
    // 订阅所有信号数据
    if let Err(e) = pubsub.psubscribe("*:s:*").await {
        warn!("Failed to subscribe to signal pattern: {}", e);
    } else {
        info!("Subscribed to signal data pattern: *:s:*");
    }
    
    // 订阅告警通道
    if let Err(e) = pubsub.subscribe("alarm:new").await {
        warn!("Failed to subscribe to alarm channel: {}", e);
    } else {
        info!("Subscribed to alarm channel: alarm:new");
    }
    
    // 处理消息流
    use futures_util::StreamExt;
    let mut stream = pubsub.on_message();
    
    info!("Redis subscription started, waiting for messages...");
    
    while let Some(msg) = stream.next().await {
        let channel = msg.get_channel_name();
        
        // 获取消息负载
        match msg.get_payload::<String>() {
            Ok(payload) => {
                debug!("Received message on channel {}: {}", channel, payload);
                
                // 发送消息给Actor处理
                subscriber_addr.do_send(RedisMessage {
                    channel: channel.to_string(),
                    payload,
                });
            }
            Err(e) => {
                error!("Failed to get payload from Redis message: {}", e);
            }
        }
    }
    
    warn!("Redis subscription stream ended");
    Ok(())
}

impl Handler<RedisMessage> for RedisSubscriber {
    type Result = ();
    
    fn handle(&mut self, msg: RedisMessage, _ctx: &mut Self::Context) {
        self.handle_redis_message(&msg.channel, &msg.payload);
    }
}