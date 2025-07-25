use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::fmt;

use crate::response::ApiResponse;

// Type aliases for backward compatibility
pub type ApiError = ApiGatewayError;
pub type ApiResult<T> = Result<T, ApiGatewayError>;

#[derive(Debug)]
pub enum ApiGatewayError {
    // Service errors
    ServiceUnavailable(String),
    ServiceTimeout(String),
    ServiceError(String),

    // Auth errors
    Unauthorized,
    Forbidden,
    InvalidToken(String),
    TokenExpired,

    // Request errors
    BadRequest(String),
    NotFound(String),
    MethodNotAllowed,

    // Internal errors
    InternalError(String),
    DatabaseError(String),
    RedisError(String),
    InfluxDb(String),

    // Config errors
    ConfigFetchError(String),
    ConfigParseError(String),
    ConfigUpdateError(String),
    ConfigChecksumError,
    ConfigSubscriptionError(String),
}

impl fmt::Display for ApiGatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiGatewayError::ServiceUnavailable(service) => {
                write!(f, "Service unavailable: {}", service)
            }
            ApiGatewayError::ServiceTimeout(service) => {
                write!(f, "Service timeout: {}", service)
            }
            ApiGatewayError::ServiceError(msg) => write!(f, "Service error: {}", msg),

            ApiGatewayError::Unauthorized => write!(f, "Unauthorized"),
            ApiGatewayError::Forbidden => write!(f, "Forbidden"),
            ApiGatewayError::InvalidToken(msg) => write!(f, "Invalid token: {}", msg),
            ApiGatewayError::TokenExpired => write!(f, "Token expired"),

            ApiGatewayError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            ApiGatewayError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ApiGatewayError::MethodNotAllowed => write!(f, "Method not allowed"),

            ApiGatewayError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            ApiGatewayError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            ApiGatewayError::RedisError(msg) => write!(f, "Redis error: {}", msg),
            ApiGatewayError::InfluxDb(msg) => write!(f, "InfluxDB error: {}", msg),

            ApiGatewayError::ConfigFetchError(msg) => write!(f, "Config fetch error: {}", msg),
            ApiGatewayError::ConfigParseError(msg) => write!(f, "Config parse error: {}", msg),
            ApiGatewayError::ConfigUpdateError(msg) => write!(f, "Config update error: {}", msg),
            ApiGatewayError::ConfigChecksumError => {
                write!(f, "Config checksum verification failed")
            }
            ApiGatewayError::ConfigSubscriptionError(msg) => {
                write!(f, "Config subscription error: {}", msg)
            }
        }
    }
}

impl IntoResponse for ApiGatewayError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiGatewayError::ServiceUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE")
            }
            ApiGatewayError::ServiceTimeout(_) => (StatusCode::GATEWAY_TIMEOUT, "SERVICE_TIMEOUT"),
            ApiGatewayError::ServiceError(_) => (StatusCode::BAD_GATEWAY, "SERVICE_ERROR"),

            ApiGatewayError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            ApiGatewayError::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            ApiGatewayError::InvalidToken(_) => (StatusCode::UNAUTHORIZED, "INVALID_TOKEN"),
            ApiGatewayError::TokenExpired => (StatusCode::UNAUTHORIZED, "TOKEN_EXPIRED"),

            ApiGatewayError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            ApiGatewayError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ApiGatewayError::MethodNotAllowed => {
                (StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED")
            }

            ApiGatewayError::InternalError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR")
            }
            ApiGatewayError::DatabaseError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR")
            }
            ApiGatewayError::RedisError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "REDIS_ERROR"),
            ApiGatewayError::InfluxDb(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INFLUXDB_ERROR"),

            ApiGatewayError::ConfigFetchError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "CONFIG_FETCH_ERROR")
            }
            ApiGatewayError::ConfigParseError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "CONFIG_PARSE_ERROR")
            }
            ApiGatewayError::ConfigUpdateError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "CONFIG_UPDATE_ERROR")
            }
            ApiGatewayError::ConfigChecksumError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "CONFIG_CHECKSUM_ERROR")
            }
            ApiGatewayError::ConfigSubscriptionError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONFIG_SUBSCRIPTION_ERROR",
            ),
        };

        let response = ApiResponse::<()>::error(code, &self.to_string(), None);

        (status, Json(response)).into_response()
    }
}

// Implement From traits for common error types
impl From<reqwest::Error> for ApiGatewayError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ApiGatewayError::ServiceTimeout("Request timeout".to_string())
        } else if err.is_connect() {
            ApiGatewayError::ServiceUnavailable("Connection failed".to_string())
        } else {
            ApiGatewayError::ServiceError(err.to_string())
        }
    }
}

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

impl From<voltage_libs::error::VoltageError> for ApiGatewayError {
    fn from(err: voltage_libs::error::VoltageError) -> Self {
        match err {
            voltage_libs::error::VoltageError::Storage(msg) => ApiGatewayError::RedisError(msg),
            voltage_libs::error::VoltageError::Network(msg) => ApiGatewayError::ServiceError(msg),
            voltage_libs::error::VoltageError::Config(msg) => {
                ApiGatewayError::ConfigParseError(msg)
            }
            _ => ApiGatewayError::InternalError(err.to_string()),
        }
    }
}

// Helper function to convert ApiResult to Response
pub fn api_result_to_response<T>(result: ApiResult<T>) -> axum::response::Response
where
    T: IntoResponse,
{
    match result {
        Ok(value) => value.into_response(),
        Err(error) => error.into_response(),
    }
}

// Removed orphan trait implementation for ApiResult<T>
// Use api_result_to_response helper function instead
