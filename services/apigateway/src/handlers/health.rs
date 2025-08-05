use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

use crate::response::ApiResponse;
use crate::AppState;

pub async fn health_check() -> impl IntoResponse {
    Json(ApiResponse::success(json!({
        "status": "healthy",
        "service": "apigateway"
    })))
}

pub async fn detailed_health(State(app_state): State<AppState>) -> impl IntoResponse {
    let mut health_status = json!({
        "status": "healthy",
        "service": "apigateway",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "dependencies": {}
    });

    // Check Redis connection
    let redis_status = {
        use crate::redis_client::RedisClientExt;
        match app_state.redis_client.ping_api().await {
            Ok(_pong) => json!({
                "status": "healthy",
                "message": "Redis connection successful"
            }),
            Err(e) => json!({
                "status": "unhealthy",
                "message": format!("Redis error: {}", e)
            }),
        }
    };

    health_status["dependencies"]["redis"] = redis_status;

    // Determine overall health
    let is_healthy = health_status["dependencies"]["redis"]["status"] == "healthy";

    if !is_healthy {
        health_status["status"] = json!("degraded");
    }

    Json(ApiResponse::success(health_status))
}
