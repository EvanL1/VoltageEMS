use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;
use tracing::error;

use crate::state::AppState;

// ── POST /api/v1/broadcast ────────────────────────────────────────────────────

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
            return Json(json!({"success": false, "message": "无效的JSON数据"})).into_response();
        },
    };

    let (count, clients) = state.ws_hub.broadcast(&msg);

    Json(json!({
        "success": true,
        "message": format!("消息已广播到 {} 个客户端", count),
        "data": {
            "client_count": count,
            "clients": clients,
            "broadcast_data": body,
        }
    }))
    .into_response()
}

// ── GET /api/v1/broadcast/status ─────────────────────────────────────────────

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
        "message": "获取成功",
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
