use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use std::time::Instant;
use tower::timeout;
use tower_http::limit::RequestBodyLimitLayer;
use uuid::Uuid;

use crate::api::{models::ErrorResponse, AppState};

/// 请求验证中间件
pub async fn validate_request(
    State(_state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    // 验证 Content-Type
    if let Some(content_type) = request.headers().get("content-type") {
        let content_type_str = content_type.to_str().unwrap_or("");
        if !content_type_str.contains("application/json") && request.method() != "GET" {
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(ErrorResponse {
                    error: "Content-Type must be application/json".to_string(),
                    code: "INVALID_CONTENT_TYPE".to_string(),
                    timestamp: Utc::now(),
                }),
            ));
        }
    }

    Ok(next.run(request).await)
}

/// 请求追踪中间件
pub async fn trace_request(request: Request<axum::body::Body>, next: Next) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();

    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        "Incoming request"
    );

    let response = next.run(request).await;
    let duration = start.elapsed();

    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        status = %response.status(),
        duration_ms = %duration.as_millis(),
        "Request completed"
    );

    response
}

/// 速率限制配置
pub struct RateLimitConfig {
    /// 每分钟最大请求数
    pub requests_per_minute: u32,
    /// 是否按IP限制
    pub per_ip: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            per_ip: true,
        }
    }
}

/// 创建请求体大小限制层
pub fn create_body_limit_layer() -> RequestBodyLimitLayer {
    // 限制请求体大小为 10MB
    RequestBodyLimitLayer::new(10 * 1024 * 1024)
}

/// 错误处理中间件
pub async fn handle_error(err: tower::BoxError) -> (StatusCode, Json<ErrorResponse>) {
    if err.is::<timeout::error::Elapsed>() {
        (
            StatusCode::REQUEST_TIMEOUT,
            Json(ErrorResponse {
                error: "Request timeout".to_string(),
                code: "REQUEST_TIMEOUT".to_string(),
                timestamp: Utc::now(),
            }),
        )
    } else if err.to_string().contains("body limit exceeded") {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ErrorResponse {
                error: "Request body too large".to_string(),
                code: "PAYLOAD_TOO_LARGE".to_string(),
                timestamp: Utc::now(),
            }),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Internal server error".to_string(),
                code: "INTERNAL_ERROR".to_string(),
                timestamp: Utc::now(),
            }),
        )
    }
}
