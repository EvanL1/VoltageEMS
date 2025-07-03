use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use thiserror::Error;

use crate::response::error_response;

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

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        let error_code = self.error_code();
        let message = self.to_string();
        
        error_response(status, &error_code, &message, None)
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
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
        }
    }
}

impl ApiError {
    fn error_code(&self) -> String {
        match self {
            ApiError::Redis(_) => "REDIS_ERROR",
            ApiError::Request(_) => "REQUEST_ERROR",
            ApiError::ServiceNotFound(_) => "SERVICE_NOT_FOUND",
            ApiError::BadGateway(_) => "BAD_GATEWAY",
            ApiError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            ApiError::InternalError(_) => "INTERNAL_ERROR",
            ApiError::BadRequest(_) => "BAD_REQUEST",
            ApiError::Timeout(_) => "TIMEOUT",
            ApiError::Unauthorized(_) => "UNAUTHORIZED",
            ApiError::Forbidden(_) => "FORBIDDEN",
            ApiError::NotFound(_) => "NOT_FOUND",
        }.to_string()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;