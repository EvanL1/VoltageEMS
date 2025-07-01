use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Bad gateway: {0}")]
    BadGateway(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Internal server error: {0}")]
    InternalError(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        let error_response = json!({
            "error": {
                "code": status.as_u16(),
                "message": self.to_string(),
                "type": self.error_type(),
            }
        });

        HttpResponse::build(status).json(error_response)
    }

    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Redis(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Request(_) => StatusCode::BAD_GATEWAY,
            ApiError::ServiceNotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadGateway(_) => StatusCode::BAD_GATEWAY,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
        }
    }
}

impl ApiError {
    fn error_type(&self) -> &'static str {
        match self {
            ApiError::Redis(_) => "redis_error",
            ApiError::Request(_) => "request_error",
            ApiError::ServiceNotFound(_) => "service_not_found",
            ApiError::BadGateway(_) => "bad_gateway",
            ApiError::ServiceUnavailable(_) => "service_unavailable",
            ApiError::InternalError(_) => "internal_error",
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Timeout(_) => "timeout",
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;