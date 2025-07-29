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

    // Check backend services
    let mut services_status = json!({});
    let services = vec!["comsrv", "modsrv", "hissrv", "netsrv", "alarmsrv"];

    for service in services {
        if let Some(service_url) = app_state.config.get_service_url(service) {
            let health_url = format!("{}/health", service_url);
            let status = match app_state
                .http_client
                .get(&health_url)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => json!({
                    "status": "healthy",
                    "message": format!("{} is responding", service)
                }),
                Ok(response) => json!({
                    "status": "unhealthy",
                    "message": format!("{} returned status: {}", service, response.status())
                }),
                Err(e) => json!({
                    "status": "unhealthy",
                    "message": format!("{} is unreachable: {}", service, e)
                }),
            };
            services_status[service] = status;
        }
    }

    health_status["dependencies"]["redis"] = redis_status;
    health_status["dependencies"]["services"] = services_status;

    // Determine overall health
    let is_healthy = health_status["dependencies"]["redis"]["status"] == "healthy";

    if !is_healthy {
        health_status["status"] = json!("degraded");
    }

    Json(ApiResponse::success(health_status))
}
