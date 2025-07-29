//! WebSocket管理模块
//!
//! 提供WebSocket连接管理和实时数据推送功能

use axum::{
    extract::{ws::WebSocket, Path, State, WebSocketUpgrade},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use voltage_libs::redis::EdgeRedis;

use crate::api::ApiState;
use crate::model::ModelManager;

/// WebSocket消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// 数据更新
    Update { point: String, value: f64 },
    /// 心跳
    Heartbeat,
    /// 错误
    Error { message: String },
}

/// WebSocket连接信息
struct WsConnection {
    model_id: String,
    tx: mpsc::UnboundedSender<WsMessage>,
}

/// WebSocket连接管理器
pub struct WsConnectionManager {
    connections: Arc<RwLock<HashMap<String, Vec<WsConnection>>>>,
    model_manager: Arc<ModelManager>,
}

impl WsConnectionManager {
    /// 创建新的连接管理器
    pub fn new(model_manager: Arc<ModelManager>) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            model_manager,
        }
    }

    /// 启动Redis订阅（监听数据更新）
    pub async fn start_redis_subscription(&self) -> Result<(), Box<dyn std::error::Error>> {
        let connections = self.connections.clone();

        // 创建新的Redis连接用于订阅
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let client = redis::Client::open(redis_url)?;
        let mut pubsub = client.get_async_pubsub().await?;

        // 订阅所有模型的更新通道
        pubsub.psubscribe("modsrv:*:update").await?;

        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut stream = pubsub.on_message();

            while let Some(msg) = stream.next().await {
                let channel: String = msg.get_channel().unwrap();
                let payload: String = msg.get_payload().unwrap();

                // 解析channel获取model_id
                if let Some(model_id) = parse_model_id_from_channel(&channel) {
                    // 解析payload获取point和value
                    if let Some((point, value)) = parse_update_payload(&payload) {
                        // 广播给订阅该模型的所有连接
                        let conns = connections.read().await;
                        if let Some(model_conns) = conns.get(&model_id) {
                            let update_msg = WsMessage::Update { point, value };
                            for conn in model_conns {
                                if let Err(e) = conn.tx.send(update_msg.clone()) {
                                    warn!("发送WebSocket消息失败: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// 添加连接
    async fn add_connection(
        &self,
        model_id: String,
        tx: mpsc::UnboundedSender<WsMessage>,
    ) -> String {
        let conn_id = uuid::Uuid::new_v4().to_string();
        let conn = WsConnection {
            model_id: model_id.clone(),
            tx,
        };

        let mut connections = self.connections.write().await;
        connections
            .entry(model_id)
            .or_insert_with(Vec::new)
            .push(conn);

        conn_id
    }

    /// 移除连接
    async fn remove_connection(&self, model_id: &str, _conn_id: &str) {
        let mut connections = self.connections.write().await;
        if let Some(model_conns) = connections.get_mut(model_id) {
            // 由于我们简化了实现，这里只是清理断开的连接
            model_conns.retain(|conn| {
                // 测试连接是否还活着
                !conn.tx.is_closed()
            });

            if model_conns.is_empty() {
                connections.remove(model_id);
            }
        }
    }

    /// 启动心跳任务
    pub async fn start_heartbeat(&self) {
        let connections = self.connections.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let mut conns = connections.write().await;
                for (model_id, model_conns) in conns.iter_mut() {
                    model_conns.retain(|conn| {
                        if conn.tx.send(WsMessage::Heartbeat).is_err() {
                            debug!("移除断开的WebSocket连接: {}", model_id);
                            false
                        } else {
                            true
                        }
                    });
                }

                // 清理空的模型条目
                conns.retain(|_, v| !v.is_empty());
            }
        });
    }
}

/// WebSocket处理器
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> Response {
    // 检查模型是否存在
    if state.model_manager.get_model(&model_id).is_none() {
        return ws.on_upgrade(move |socket| async move {
            handle_error_socket(socket, "模型不存在").await;
        });
    }

    ws.on_upgrade(move |socket| handle_socket(socket, model_id, state))
}

/// 处理WebSocket连接
async fn handle_socket(socket: WebSocket, model_id: String, state: ApiState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

    // 添加连接
    let conn_id = state.ws_manager.add_connection(model_id.clone(), tx).await;
    info!("WebSocket连接建立: model_id={}", model_id);

    // 发送初始数据
    if let Ok(values) = state.model_manager.get_model_values(&model_id).await {
        for (point, value) in values {
            let msg = WsMessage::Update { point, value };
            if sender
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&msg).unwrap().into(),
                ))
                .await
                .is_err()
            {
                break;
            }
        }
    }

    // 启动发送任务
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let text = serde_json::to_string(&msg).unwrap();
            if sender
                .send(axum::extract::ws::Message::Text(text.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // 启动接收任务（处理客户端消息）
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                axum::extract::ws::Message::Text(text) => {
                    debug!("收到WebSocket消息: {}", text);
                }
                axum::extract::ws::Message::Close(_) => {
                    debug!("WebSocket连接关闭");
                    break;
                }
                _ => {}
            }
        }
    });

    // 等待任一任务结束
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // 清理连接
    state
        .ws_manager
        .remove_connection(&model_id, &conn_id)
        .await;
    info!("WebSocket连接断开: model_id={}", model_id);
}

/// 处理错误的WebSocket连接
async fn handle_error_socket(mut socket: WebSocket, error: &str) {
    let error_msg = WsMessage::Error {
        message: error.to_string(),
    };

    if let Ok(text) = serde_json::to_string(&error_msg) {
        let _ = socket
            .send(axum::extract::ws::Message::Text(text.into()))
            .await;
    }

    let _ = socket.close().await;
}

/// 解析channel获取model_id
fn parse_model_id_from_channel(channel: &str) -> Option<String> {
    // channel格式: "modsrv:{model_id}:update"
    let parts: Vec<&str> = channel.split(':').collect();
    if parts.len() == 3 && parts[0] == "modsrv" && parts[2] == "update" {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// 解析更新payload
fn parse_update_payload(payload: &str) -> Option<(String, f64)> {
    // payload格式: "{point}:{value}"
    let parts: Vec<&str> = payload.split(':').collect();
    if parts.len() == 2 {
        if let Ok(value) = parts[1].parse::<f64>() {
            return Some((parts[0].to_string(), value));
        }
    }
    None
}
