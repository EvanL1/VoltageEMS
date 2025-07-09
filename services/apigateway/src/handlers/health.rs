use actix_web::{get, web, HttpResponse};
use serde_json::json;
use std::sync::Arc;

use crate::config::Config;
use crate::error::ApiResult;
use crate::redis_client::RedisClient;
use crate::response::success_response;

#[get("/health")]
pub async fn health_check() -> ApiResult<HttpResponse> {
    Ok(success_response(json!({
        "status": "healthy",
        "service": "apigateway"
    })))
}

pub async fn simple_health() -> ApiResult<HttpResponse> {
    Ok(success_response(json!({
        "status": "ok"
    })))
}

#[get("/health/detailed")]
pub async fn detailed_health(
    config: web::Data<Config>,
    redis_client: web::Data<Arc<RedisClient>>,
    http_client: web::Data<Arc<reqwest::Client>>,
) -> ApiResult<HttpResponse> {
    let mut health_status = json!({
        "status": "healthy",
        "service": "apigateway",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "dependencies": {}
    });

    // Check Redis connection
    let redis_status = {
        match redis_client.ping().await {
            Ok(true) => json!({
                "status": "healthy",
                "message": "Redis connection successful"
            }),
            Ok(false) => json!({
                "status": "unhealthy",
                "message": "Redis ping failed"
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
        if let Some(service_url) = config.get_service_url(service) {
            let health_url = format!("{}/health", service_url);
            let status = match http_client
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
