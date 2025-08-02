//! Simple Error Handling for AlarmSrv
//!
//! This module provides unified error types without complex abstractions.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;

/// Result type alias
pub type Result<T> = std::result::Result<T, AlarmError>;

/// Simplified alarm error types
#[derive(Debug)]
pub enum AlarmError {
    /// Redis connection or operation error
    Redis(redis::RedisError),
    /// Serialization/deserialization error
    Serialization(serde_json::Error),
    /// Alarm not found
    AlarmNotFound(String),
    /// Invalid alarm state transition
    InvalidStateTransition { from: String, to: String },
    /// Configuration error
    Config(String),
    /// Invalid input
    InvalidInput(String),
    /// Internal server error
    Internal(String),
}

impl fmt::Display for AlarmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlarmError::Redis(err) => write!(f, "Redis error: {}", err),
            AlarmError::Serialization(err) => write!(f, "Serialization error: {}", err),
            AlarmError::AlarmNotFound(id) => write!(f, "Alarm not found: {}", id),
            AlarmError::InvalidStateTransition { from, to } => {
                write!(f, "Invalid state transition from {} to {}", from, to)
            },
            AlarmError::Config(msg) => write!(f, "Configuration error: {}", msg),
            AlarmError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            AlarmError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AlarmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AlarmError::Redis(err) => Some(err),
            AlarmError::Serialization(err) => Some(err),
            _ => None,
        }
    }
}

// Conversion from common error types
impl From<redis::RedisError> for AlarmError {
    fn from(err: redis::RedisError) -> Self {
        AlarmError::Redis(err)
    }
}

impl From<serde_json::Error> for AlarmError {
    fn from(err: serde_json::Error) -> Self {
        AlarmError::Serialization(err)
    }
}

impl From<anyhow::Error> for AlarmError {
    fn from(err: anyhow::Error) -> Self {
        AlarmError::Internal(err.to_string())
    }
}

// HTTP response conversion for Axum
impl IntoResponse for AlarmError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AlarmError::AlarmNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AlarmError::InvalidInput(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AlarmError::InvalidStateTransition { .. } => (StatusCode::CONFLICT, self.to_string()),
            AlarmError::Config(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Configuration error".to_string(),
            ),
            AlarmError::Redis(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            ),
            AlarmError::Serialization(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Serialization error".to_string(),
            ),
            AlarmError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };

        let body = Json(json!({
            "error": message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

/// Helper function to create an alarm not found error
pub fn alarm_not_found(id: &str) -> AlarmError {
    AlarmError::AlarmNotFound(id.to_string())
}

/// Helper function to create an invalid input error
pub fn invalid_input(msg: &str) -> AlarmError {
    AlarmError::InvalidInput(msg.to_string())
}

/// Helper function to create an internal error
pub fn internal_error(msg: &str) -> AlarmError {
    AlarmError::Internal(msg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = AlarmError::AlarmNotFound("test-id".to_string());
        assert_eq!(format!("{}", error), "Alarm not found: test-id");

        let error = AlarmError::InvalidStateTransition {
            from: "New".to_string(),
            to: "Resolved".to_string(),
        };
        assert!(format!("{}", error).contains("Invalid state transition"));
    }

    #[test]
    fn test_helper_functions() {
        let error = alarm_not_found("test");
        assert!(matches!(error, AlarmError::AlarmNotFound(_)));

        let error = invalid_input("bad data");
        assert!(matches!(error, AlarmError::InvalidInput(_)));

        let error = internal_error("something went wrong");
        assert!(matches!(error, AlarmError::Internal(_)));
    }
}
