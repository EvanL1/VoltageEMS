use axum::{extract::State, response::IntoResponse};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::ApiResult;
use crate::response::success_response;
use crate::AppState;

pub async fn health_check() -> ApiResult<impl IntoResponse> {
    Ok(success_response(json!({
        "status": "healthy",
        "service": "apigateway"
    })))
}

pub async fn simple_health() -> ApiResult<impl IntoResponse> {
    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    Ok(success_response(json!({
        "status": "healthy",
        "uptime": uptime,
        "version": env!("CARGO_PKG_VERSION")
    })))
}

pub async fn detailed_health(
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    let mut health_status = json!({
        "status": "healthy",
        "service": "apigateway",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "dependencies": {}
    });

    // Check Redis connection
    let redis_status = {
        match state.redis_client.ping().await {
            Ok(_) => json!({
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
        if let Some(service_url) = state.config.get_service_url(service) {
            let health_url = format!("{}/health", service_url);
            let status = match state.http_client
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

    Ok(success_response(health_status))
}