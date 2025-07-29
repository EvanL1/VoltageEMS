use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
    pub timestamp: String,
}

#[derive(Serialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    pub fn error(code: &str, message: &str, details: Option<String>) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(ErrorInfo {
                code: code.to_string(),
                message: message.to_string(),
                details,
            }),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

pub fn success_response<T: Serialize>(data: T) -> impl IntoResponse {
    Json(ApiResponse::success(data))
}

#[allow(dead_code)]
pub fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    details: Option<String>,
) -> impl IntoResponse {
    (
        status,
        Json(ApiResponse::<()>::error(code, message, details)),
    )
}

#[allow(dead_code)]
pub fn paginated_response<T: Serialize>(
    data: Vec<T>,
    total: usize,
    offset: usize,
    limit: usize,
) -> impl IntoResponse {
    success_response(json!({
        "items": data,
        "pagination": {
            "total": total,
            "offset": offset,
            "limit": limit,
        }
    }))
}
