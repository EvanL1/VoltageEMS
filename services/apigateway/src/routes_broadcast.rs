use std::sync::Arc;

use axum::{Json, extract::State, response::IntoResponse};
use serde_json::json;
use tracing::error;

use crate::state::AppState;

// ── POST /api/v1/broadcast ────────────────────────────────────────────────────

/// 向所有 WebSocket 客户端广播一条 JSON 消息。
///
/// 把请求体原样转发给当前所有已连接的 WebSocket 客户端，**不做订阅过滤**
/// —— 即使客户端只订阅了特定 channel，仍会收到这条广播。返回投递到的客
/// 户端数量和它们的元信息。运维场景用：手动推送系统提示、强制刷新前端
/// 缓存、debug WebSocket 链路。
#[utoipa::path(post, path = "/api/v1/broadcast", tag = "WebSocket",
    security(("bearer_auth" = [])),
    request_body(content = serde_json::Value, description = "任意 JSON 消息，广播给所有 WS 客户端"),
    responses((status = 200, description = "广播成功")))]
pub async fn broadcast_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let msg = match serde_json::to_string(&body) {
        Ok(s) => s,
        Err(e) => {
            error!("Serialize broadcast body error: {}", e);
            return Json(json!({"success": false, "message": "Invalid JSON data"})).into_response();
        },
    };

    let (count, clients) = state.ws_hub.broadcast(&msg);

    Json(json!({
        "success": true,
        "message": format!("Message broadcast to {} client(s)", count),
        "data": {
            "client_count": count,
            "clients": clients,
            "broadcast_data": body,
        }
    }))
    .into_response()
}

// ── GET /api/v1/broadcast/status ─────────────────────────────────────────────

/// WebSocket 集线器当前的连接状态总览。
///
/// 返回总连接数、已订阅（订阅了至少一个 channel 或 data_type 的）客户
/// 端数、每个连接的元信息（client_id、连接时间）以及完整订阅表。运维
/// 排查"客户端为什么没收到推送"用：先看连接是不是在、再看订阅是不是命
/// 中。
#[utoipa::path(get, path = "/api/v1/broadcast/status", tag = "WebSocket",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "WebSocket 连接状态")))]
pub async fn broadcast_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let status = state.ws_hub.get_status();

    let subscribed_count = status["subscriptions"]
        .as_object()
        .map(|m| {
            m.values()
                .filter(|v| {
                    v["channels"]
                        .as_array()
                        .map(|a| !a.is_empty())
                        .unwrap_or(false)
                        || v["data_types"]
                            .as_array()
                            .map(|a| !a.is_empty())
                            .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);

    Json(json!({
        "success": true,
        "message": "OK",
        "data": {
            "websocket_available": true,
            "connection_count": status["connection_count"],
            "subscribed_count": subscribed_count,
            "connections": status["connections_info"],
            "subscriptions": status["subscriptions"],
        }
    }))
    .into_response()
}
