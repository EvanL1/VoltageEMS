use crate::auth::Claims;
use crate::AppState;
use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        Extension, State,
    },
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// WebSocket消息类型
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WsMessage {
    /// 订阅请求
    Subscribe {
        patterns: Vec<String>,
    },
    /// 取消订阅
    Unsubscribe {
        patterns: Vec<String>,
    },
    /// 数据更新
    Update {
        channel: String,
        data: serde_json::Value,
    },
    /// 心跳
    Ping,
    Pong,
    /// 错误消息
    Error {
        message: String,
    },
}

/// WebSocket处理器
pub async fn ws_handler(
    State(app_state): State<AppState>,
    Extension(claims): Extension<Claims>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, app_state, claims))
}

async fn handle_socket(mut socket: WebSocket, _app_state: AppState, claims: Claims) {
    info!("WebSocket connection established for user: {}", claims.sub);

    // 简化版本：仅处理基本的ping/pong
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(ws_msg) => {
                if let Ok(text) = ws_msg.to_text() {
                    if let Ok(msg) = serde_json::from_str::<WsMessage>(text) {
                        match msg {
                            WsMessage::Ping => {
                                let pong = serde_json::to_string(&WsMessage::Pong).unwrap();
                                if socket
                                    .send(axum::extract::ws::Message::Text(pong.into()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            WsMessage::Subscribe { patterns } => {
                                info!("User {} subscribing to: {:?}", claims.sub, patterns);
                                // TODO: 实现Redis订阅逻辑
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(e) => {
                warn!("WebSocket error: {}", e);
                break;
            }
        }
    }

    info!("WebSocket connection closed for user: {}", claims.sub);
}
