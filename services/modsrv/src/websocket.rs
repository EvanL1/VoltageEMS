//! WebSocket实时数据推送模块
//!
//! 提供WebSocket连接管理和实时数据推送功能

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use voltage_libs::types::StandardFloat;

use crate::model::ModelManager;

/// WebSocket消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WsMessage {
    /// 订阅消息
    Subscribe {
        /// 要订阅的点位列表，None表示订阅所有
        points: Option<Vec<String>>,
    },
    /// 数据更新消息
    Update {
        model_id: String,
        timestamp: i64,
        monitoring: HashMap<String, f64>,
    },
    /// 心跳请求
    Ping { timestamp: i64 },
    /// 心跳响应
    Pong { timestamp: i64 },
    /// 错误消息
    Error { code: String, message: String },
}

/// 紧凑格式的更新消息
#[derive(Debug, Clone, Serialize)]
pub struct CompactUpdate {
    /// 类型标识: "u" = update
    pub t: &'static str,
    /// 模型ID
    pub m: String,
    /// 时间戳
    pub ts: i64,
    /// 数据数组 [[name, value], ...]
    pub d: Vec<(String, f64)>,
}

/// WebSocket连接信息
pub struct WsConnection {
    /// 连接ID
    pub id: String,
    /// 模型ID
    pub model_id: String,
    /// 订阅的点位（None表示全部）
    pub subscribed_points: Option<Vec<String>>,
    /// 消息发送通道
    pub tx: mpsc::Sender<Message>,
}

/// WebSocket连接管理器
pub struct WsConnectionManager {
    /// 所有活跃连接
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
    /// 模型管理器引用
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

    /// 处理新的WebSocket连接
    pub async fn handle_connection(&self, ws: WebSocket, model_id: String) {
        let (mut ws_sender, mut ws_receiver) = ws.split();
        let (tx, mut rx) = mpsc::channel(100);

        let conn_id = Uuid::new_v4().to_string();
        let conn = WsConnection {
            id: conn_id.clone(),
            model_id: model_id.clone(),
            subscribed_points: None,
            tx,
        };

        // 存储连接
        {
            let mut connections = self.connections.write().await;
            connections.insert(conn_id.clone(), conn);
            info!("WebSocket连接建立: {} -> {}", conn_id, model_id);
        }

        // 发送任务
        let _conn_id_clone = conn_id.clone();
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_sender.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // 接收任务
        let _connections = self.connections.clone();
        let _model_manager = self.model_manager.clone();
        let conn_id_recv = conn_id.clone();

        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = self.handle_message(&conn_id_recv, &text).await {
                        error!("处理WebSocket消息失败: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket连接关闭: {}", conn_id_recv);
                    break;
                }
                Ok(Message::Ping(_data)) => {
                    // Axum自动处理Ping/Pong
                }
                Err(e) => {
                    error!("WebSocket错误: {}", e);
                    break;
                }
                _ => {}
            }
        }

        // 清理连接
        {
            let mut connections = self.connections.write().await;
            connections.remove(&conn_id);
            info!("WebSocket连接移除: {}", conn_id);
        }

        // 取消发送任务
        send_task.abort();
    }

    /// 处理客户端消息
    async fn handle_message(
        &self,
        conn_id: &str,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let msg: serde_json::Value = serde_json::from_str(text)?;

        match msg.get("type").and_then(|t| t.as_str()) {
            Some("subscribe") => {
                let points = msg.get("points").and_then(|p| p.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });

                self.handle_subscribe(conn_id, points).await?;
            }
            Some("ping") => {
                let timestamp = msg
                    .get("timestamp")
                    .and_then(|t| t.as_i64())
                    .unwrap_or_else(|| chrono::Utc::now().timestamp());

                self.send_to_connection(conn_id, WsMessage::Pong { timestamp })
                    .await?;
            }
            _ => {
                warn!("未知的WebSocket消息类型: {}", text);
            }
        }

        Ok(())
    }

    /// 处理订阅消息
    async fn handle_subscribe(
        &self,
        conn_id: &str,
        points: Option<Vec<String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut connections = self.connections.write().await;

        if let Some(conn) = connections.get_mut(conn_id) {
            conn.subscribed_points = points.clone();
            info!("连接 {} 订阅点位: {:?}", conn_id, points);

            // 立即发送当前值
            if let Some(_model) = self.model_manager.get_model(&conn.model_id).await {
                let values = self
                    .model_manager
                    .get_all_monitoring_values(&conn.model_id)
                    .await
                    .unwrap_or_default();

                let filtered_values = if let Some(ref points) = points {
                    values
                        .into_iter()
                        .filter(|(name, _)| points.contains(name))
                        .collect()
                } else {
                    values
                };

                if !filtered_values.is_empty() {
                    let update = WsMessage::Update {
                        model_id: conn.model_id.clone(),
                        timestamp: chrono::Utc::now().timestamp(),
                        monitoring: filtered_values
                            .into_iter()
                            .map(|(k, v)| (k, v.value()))
                            .collect(),
                    };

                    let _ = conn
                        .tx
                        .send(Message::Text(serde_json::to_string(&update)?.into()))
                        .await;
                }
            }
        }

        Ok(())
    }

    /// 广播更新到指定模型的所有连接
    pub async fn broadcast_update(&self, model_id: &str, updates: HashMap<String, StandardFloat>) {
        let connections = self.connections.read().await;
        let timestamp = chrono::Utc::now().timestamp();

        // 使用紧凑格式
        let compact_msg = CompactUpdate {
            t: "u",
            m: model_id.to_string(),
            ts: timestamp,
            d: updates
                .iter()
                .map(|(k, v)| (k.clone(), v.value()))
                .collect(),
        };

        let _msg_text = match serde_json::to_string(&compact_msg) {
            Ok(text) => text,
            Err(e) => {
                error!("序列化WebSocket消息失败: {}", e);
                return;
            }
        };

        for conn in connections.values() {
            if conn.model_id == model_id {
                // 过滤订阅的点位
                let filtered_updates = if let Some(ref points) = conn.subscribed_points {
                    compact_msg
                        .d
                        .iter()
                        .filter(|(name, _)| points.contains(name))
                        .cloned()
                        .collect()
                } else {
                    compact_msg.d.clone()
                };

                if !filtered_updates.is_empty() {
                    let filtered_msg = CompactUpdate {
                        t: "u",
                        m: model_id.to_string(),
                        ts: timestamp,
                        d: filtered_updates,
                    };

                    if let Ok(text) = serde_json::to_string(&filtered_msg) {
                        let _ = conn.tx.send(Message::Text(text.into())).await;
                    }
                }
            }
        }
    }

    /// 发送消息到指定连接
    async fn send_to_connection(
        &self,
        conn_id: &str,
        msg: WsMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let connections = self.connections.read().await;

        if let Some(conn) = connections.get(conn_id) {
            let text = serde_json::to_string(&msg)?;
            conn.tx.send(Message::Text(text.into())).await?;
        }

        Ok(())
    }

    /// 获取连接数统计
    pub async fn get_connection_stats(&self) -> HashMap<String, usize> {
        let connections = self.connections.read().await;
        let mut stats = HashMap::new();

        for conn in connections.values() {
            *stats.entry(conn.model_id.clone()).or_insert(0) += 1;
        }

        stats
    }

    /// 启动心跳任务
    pub async fn start_heartbeat(&self) {
        let connections = self.connections.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

            loop {
                interval.tick().await;

                let conns = connections.read().await;
                let timestamp = chrono::Utc::now().timestamp();

                for conn in conns.values() {
                    let msg = WsMessage::Ping { timestamp };
                    if let Ok(text) = serde_json::to_string(&msg) {
                        let _ = conn.tx.send(Message::Text(text.into())).await;
                    }
                }
            }
        });
    }
}

/// WebSocket处理器（用于Axum路由）
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(model_id): Path<String>,
    State(state): State<crate::api::ApiState>,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        state.ws_manager.handle_connection(socket, model_id).await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = WsMessage::Update {
            model_id: "test".to_string(),
            timestamp: 1234567890,
            monitoring: HashMap::from([("voltage".to_string(), 230.123456)]),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"update\""));
        assert!(json.contains("\"model_id\":\"test\""));
    }

    #[test]
    fn test_compact_update() {
        let update = CompactUpdate {
            t: "u",
            m: "test".to_string(),
            ts: 1234567890,
            d: vec![
                ("voltage".to_string(), 230.123456),
                ("current".to_string(), 15.678900),
            ],
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"t\":\"u\""));
        assert!(json.contains("\"m\":\"test\""));
        assert!(json.len() < 100); // 确保紧凑
    }
}
