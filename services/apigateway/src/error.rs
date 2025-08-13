use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::fmt;

use crate::response::ApiResponse;

// Type aliases for backward compatibility
pub type Result<T> = std::result::Result<T, ApiGatewayError>;
pub type ApiResult<T> = Result<T>;

#[derive(Debug)]
#[non_exhaustive]
pub enum ApiGatewayError {
    // Request errors
    #[allow(dead_code)]
    BadRequest(String),
    #[allow(dead_code)]
    NotFound(String),
    #[allow(dead_code)]
    MethodNotAllowed,

    // Internal errors
    InternalError(String),
    RedisError(String),
}

impl fmt::Display for ApiGatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiGatewayError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            ApiGatewayError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ApiGatewayError::MethodNotAllowed => write!(f, "Method not allowed"),
            ApiGatewayError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            ApiGatewayError::RedisError(msg) => write!(f, "Redis error: {}", msg),
        }
    }
}

impl IntoResponse for ApiGatewayError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiGatewayError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            ApiGatewayError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ApiGatewayError::MethodNotAllowed => {
                (StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED")
            },
            ApiGatewayError::InternalError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR")
            },
            ApiGatewayError::RedisError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "REDIS_ERROR"),
        };

        let response = ApiResponse::<()>::error(code, &self.to_string(), None);

        (status, Json(response)).into_response()
    }
}

// Implement From traits for common error types
impl From<serde_json::Error> for ApiGatewayError {
    fn from(err: serde_json::Error) -> Self {
        ApiGatewayError::InternalError(format!("JSON error: {}", err))
    }
}

impl From<std::io::Error> for ApiGatewayError {
    fn from(err: std::io::Error) -> Self {
        ApiGatewayError::InternalError(format!("IO error: {}", err))
    }
}

impl From<voltage_libs::error::Error> for ApiGatewayError {
    fn from(err: voltage_libs::error::Error) -> Self {
        match err {
            voltage_libs::error::Error::Redis(msg) => ApiGatewayError::RedisError(msg),
            _ => ApiGatewayError::InternalError(err.to_string()),
        }
    }
}
