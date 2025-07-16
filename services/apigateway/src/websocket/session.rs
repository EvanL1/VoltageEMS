use actix::{
    Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner,
    Handler, Running, StreamHandler, WrapFuture,
};
use actix_web_actors::ws;
use actix_web_actors::ws::{Message as ActixWsMessage, ProtocolError};
use log::{error, info, warn};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::redis_client::RedisClient;
use crate::websocket::hub::WsHub;
use crate::websocket::protocol::{
    codec::{decode_message, encode_message, TextMessage, WsMessageWrapper},
    DataType, InternalMessage, RpcError, SubscriptionFilter, WsMessage,
};

/// 心跳间隔
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
/// 客户端超时时间
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

/// WebSocket会话Actor
pub struct WsSession {
    /// 唯一会话ID
    pub id: String,
    /// Hub地址
    pub hub: Addr<WsHub>,
    /// Redis客户端
    pub redis: Arc<RedisClient>,
    /// 心跳时间戳
    pub heartbeat: Instant,
    /// 订阅的通道
    pub subscribed_channels: HashSet<u32>,
    /// 订阅的数据类型
    pub subscribed_types: HashSet<DataType>,
    /// 认证令牌
    pub auth_token: Option<String>,
    /// 用户ID
    pub user_id: Option<String>,
}

impl WsSession {
    pub fn new(hub: Addr<WsHub>, redis: Arc<RedisClient>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            hub,
            redis,
            heartbeat: Instant::now(),
            subscribed_channels: HashSet::new(),
            subscribed_types: HashSet::new(),
            auth_token: None,
            user_id: None,
        }
    }

    /// 启动心跳检测
    fn start_heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                warn!("WebSocket client heartbeat timeout, disconnecting");
                ctx.stop();
                return;
            }
            
            ctx.ping(b"ping");
        });
    }

    /// 处理订阅请求
    fn handle_subscribe(&mut self, channels: Vec<u32>, types: Vec<String>, ctx: &mut ws::WebsocketContext<Self>) {
        // 解析数据类型
        let mut parsed_types = HashSet::new();
        for type_str in &types {
            if let Some(data_type) = DataType::from_str(type_str) {
                parsed_types.insert(data_type);
            } else {
                warn!("Invalid data type: {}", type_str);
            }
        }

        // 更新本地订阅状态
        self.subscribed_channels.extend(&channels);
        self.subscribed_types.extend(&parsed_types);

        // 通知Hub
        self.hub.do_send(InternalMessage::Subscribe {
            session_id: self.id.clone(),
            filter: SubscriptionFilter {
                channels: channels.clone(),
                types: parsed_types.iter().cloned().collect(),
            },
        });

        // 发送确认消息
        let response = WsMessage::Response {
            id: 0, // Subscribe不需要id
            result: Some(serde_json::json!({
                "subscribed_channels": channels,
                "subscribed_types": types,
            })),
            error: None,
        };

        if let Ok(msg) = encode_message(&response) {
            ctx.text(msg);
        }
    }

    /// 处理取消订阅请求
    fn handle_unsubscribe(&mut self, channels: Vec<u32>, types: Option<Vec<String>>, ctx: &mut ws::WebsocketContext<Self>) {
        // 只有当指定了类型时才移除特定类型
        if let Some(type_list) = &types {
            for type_str in type_list {
                if let Some(data_type) = DataType::from_str(type_str) {
                    self.subscribed_types.remove(&data_type);
                }
            }
        }
        // 注意：不要移除通道，只移除指定的数据类型

        // 通知Hub
        let parsed_types = if let Some(ref type_list) = types {
            type_list.iter()
                .filter_map(|t| DataType::from_str(t))
                .collect()
        } else {
            vec![]
        };

        self.hub.do_send(InternalMessage::Unsubscribe {
            session_id: self.id.clone(),
            filter: SubscriptionFilter {
                channels: channels.clone(),
                types: parsed_types,
            },
        });

        // 发送响应
        let response = WsMessage::Response {
            id: 0, // unsubscribe不需要请求ID
            result: Some(serde_json::json!({
                "message": "Unsubscribed successfully",
                "channels": channels,
                "types": types
            })),
            error: None,
        };
        ctx.text(serde_json::to_string(&response).unwrap());
    }

    /// 处理控制命令
    fn handle_control(&mut self, channel_id: u32, point_id: u32, value: serde_json::Value, params: Option<serde_json::Value>, ctx: &mut ws::WebsocketContext<Self>) {
        let redis = self.redis.clone();
        let session_id = self.id.clone();

        // 异步发送控制命令到Redis
        let fut = async move {
            let command = serde_json::json!({
                "session_id": session_id,
                "channel_id": channel_id,
                "point_id": point_id,
                "value": value,
                "params": params,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });

            let channel = format!("cmd:{}:control", channel_id);
            match redis.publish(&channel, &command.to_string()).await {
                Ok(_) => {
                    info!("Control command sent: channel={}, point={}", channel_id, point_id);
                    Some(WsMessage::ControlFeedback {
                        channel_id,
                        point_id,
                        success: true,
                        message: Some("Command sent successfully".to_string()),
                        timestamp: chrono::Utc::now().timestamp_millis(),
                    })
                }
                Err(e) => {
                    error!("Failed to send control command: {}", e);
                    Some(WsMessage::ControlFeedback {
                        channel_id,
                        point_id,
                        success: false,
                        message: Some(format!("Failed to send command: {}", e)),
                        timestamp: chrono::Utc::now().timestamp_millis(),
                    })
                }
            }
        };

        fut.into_actor(self)
            .map(|msg_opt, _act, ctx| {
                if let Some(msg) = msg_opt {
                    if let Ok(encoded) = encode_message(&msg) {
                        ctx.text(encoded);
                    }
                }
            })
            .spawn(ctx);
    }

    /// 处理RPC请求
    fn handle_rpc_request(&mut self, id: u64, method: &str, params: Option<serde_json::Value>, ctx: &mut ws::WebsocketContext<Self>) {
        // TODO: 实现具体的RPC方法处理
        match method {
            "get_channels" => {
                // 示例：获取可用通道列表
                let response = WsMessage::Response {
                    id,
                    result: Some(serde_json::json!({
                        "channels": [1001, 1002, 1003]
                    })),
                    error: None,
                };
                if let Ok(msg) = encode_message(&response) {
                    ctx.text(msg);
                }
            }
            _ => {
                // 方法未找到
                let response = WsMessage::Response {
                    id,
                    result: None,
                    error: Some(RpcError::method_not_found()),
                };
                if let Ok(msg) = encode_message(&response) {
                    ctx.text(msg);
                }
            }
        }
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("WebSocket session started: {}", self.id);
        
        // 启动心跳
        self.start_heartbeat(ctx);
        
        // 注册到Hub
        self.hub.do_send(InternalMessage::Connect {
            session_id: self.id.clone(),
            addr: ctx.address(),
        });

        // 发送连接成功消息
        let msg = WsMessage::Connected {
            session_id: self.id.clone(),
            version: "1.0.0".to_string(),
            features: vec![
                "realtime_data".to_string(),
                "control".to_string(),
                "rpc".to_string(),
            ],
        };
        
        if let Ok(encoded) = encode_message(&msg) {
            ctx.text(encoded);
        }
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        info!("WebSocket session stopping: {}", self.id);
        
        // 从Hub注销
        self.hub.do_send(InternalMessage::Disconnect {
            session_id: self.id.clone(),
        });
        
        Running::Stop
    }
}

/// 处理WebSocket消息
impl StreamHandler<Result<ActixWsMessage, ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ActixWsMessage, ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ActixWsMessage::Ping(msg)) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ActixWsMessage::Pong(_)) => {
                self.heartbeat = Instant::now();
            }
            Ok(ActixWsMessage::Text(text)) => {
                match decode_message(&text) {
                    Ok(ws_msg) => {
                        match ws_msg {
                            WsMessage::Subscribe { channels, types } => {
                                self.handle_subscribe(channels, types, ctx);
                            }
                            WsMessage::Unsubscribe { channels, types } => {
                                self.handle_unsubscribe(channels, types, ctx);
                            }
                            WsMessage::Control { channel_id, point_id, value, params } => {
                                self.handle_control(channel_id, point_id, value, params, ctx);
                            }
                            WsMessage::Request { id, method, params } => {
                                self.handle_rpc_request(id, &method, params, ctx);
                            }
                            WsMessage::Ping { timestamp } => {
                                let pong = WsMessage::Pong {
                                    timestamp: timestamp.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
                                };
                                if let Ok(msg) = encode_message(&pong) {
                                    ctx.text(msg);
                                }
                            }
                            _ => {
                                warn!("Unhandled message type");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to decode message: {}", e);
                        let error = WsMessage::Error {
                            code: "DECODE_ERROR".to_string(),
                            message: format!("Failed to decode message: {}", e),
                            data: None,
                        };
                        if let Ok(msg) = encode_message(&error) {
                            ctx.text(msg);
                        }
                    }
                }
            }
            Ok(ActixWsMessage::Binary(bin)) => {
                warn!("Binary messages not supported yet");
                ctx.binary(bin);
            }
            Ok(ActixWsMessage::Close(reason)) => {
                info!("WebSocket closing: {:?}", reason);
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ActixWsMessage::Continuation(_)) => {
                ctx.stop();
            }
            Ok(ActixWsMessage::Nop) => {}
            Err(e) => {
                error!("WebSocket error: {}", e);
                ctx.stop();
            }
        }
    }
}

/// 处理来自Hub的消息
impl Handler<WsMessageWrapper> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: WsMessageWrapper, ctx: &mut Self::Context) {
        if let Ok(encoded) = encode_message(&msg.0) {
            ctx.text(encoded);
        }
    }
}

/// 处理文本消息
impl Handler<TextMessage> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: TextMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}