//! Unified error handling for VoltageEMS services
//!
//! This module provides a comprehensive error system that all services can use,
//! eliminating the need for service-specific error types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// ErrorInfo - API error response type
// ============================================================================

/// Standard error information for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ErrorInfo {
    /// Error code (HTTP status or custom)
    pub code: u16,
    /// Error message
    pub message: String,
    /// Detailed error description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Field-specific errors for validation
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub field_errors: HashMap<String, Vec<String>>,
}

impl ErrorInfo {
    /// Create a new ErrorInfo with just a message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: 500,
            message: message.into(),
            details: None,
            field_errors: HashMap::new(),
        }
    }

    /// Set the error code
    pub fn with_code(mut self, code: u16) -> Self {
        self.code = code;
        self
    }

    /// Add details
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Add a field error
    pub fn add_field_error(mut self, field: impl Into<String>, error: impl Into<String>) -> Self {
        self.field_errors
            .entry(field.into())
            .or_default()
            .push(error.into());
        self
    }
}

// ============================================================================
// VoltageError - Main error type
// ============================================================================

/// Main error type for all VoltageEMS services
#[derive(Debug, Error)]
pub enum VoltageError {
    // ======================================
    // Configuration Errors
    // ======================================
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Invalid configuration: {field}: {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("Missing required configuration: {0}")]
    MissingConfig(String),

    #[error("Configuration database not found at {path}. Run 'monarch sync {service}' first")]
    DatabaseNotFound { path: String, service: String },

    // ======================================
    // Database Errors
    // ======================================
    #[error("Database error: {0}")]
    Database(String),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Query failed: {query}: {error}")]
    QueryFailed { query: String, error: String },

    // ======================================
    // Protocol & Communication Errors
    // ======================================
    #[error("Protocol error: {protocol}: {message}")]
    Protocol { protocol: String, message: String },

    #[error("Communication error: {0}")]
    Communication(String),

    #[error("Connection failed: {endpoint}: {reason}")]
    ConnectionFailed { endpoint: String, reason: String },

    #[error("Timeout waiting for response from {0}")]
    Timeout(String),

    #[error("Modbus error: {0}")]
    Modbus(String),

    #[error("gRPC error: {0}")]
    Grpc(String),

    // ======================================
    // Calculation & Processing Errors
    // ======================================
    #[error("Calculation error: {0}")]
    Calculation(String),

    #[error("Invalid expression: {expression}: {error}")]
    InvalidExpression { expression: String, error: String },

    #[error("Division by zero in calculation: {context}")]
    DivisionByZero { context: String },

    #[error("Data type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Processing error: {0}")]
    Processing(String),

    // ======================================
    // API & HTTP Errors
    // ======================================
    #[error("API error: {0}")]
    Api(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {resource}")]
    NotFound { resource: String },

    #[error("Conflict: {resource} already exists")]
    Conflict { resource: String },

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    // ======================================
    // Validation Errors
    // ======================================
    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Invalid parameter: {param}: {reason}")]
    InvalidParameter { param: String, reason: String },

    #[error("Out of range: {value} not in [{min}, {max}]")]
    OutOfRange {
        value: String,
        min: String,
        max: String,
    },

    #[error("Pattern mismatch: {value} does not match {pattern}")]
    PatternMismatch { value: String, pattern: String },

    // ======================================
    // Resource & Instance Errors
    // ======================================
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    #[error("Product not found: {0}")]
    ProductNotFound(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Point not found: {point_type}:{point_id}")]
    PointNotFound { point_type: String, point_id: i32 },

    #[error("Rule not found: {0}")]
    RuleNotFound(String),

    #[error("Resource busy: {0}")]
    ResourceBusy(String),

    #[error("Resource already exists: {0}")]
    AlreadyExists(String),

    // ======================================
    // File & I/O Errors
    // ======================================
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Parse error: {file}: {error}")]
    ParseError { file: String, error: String },

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    // ======================================
    // Service & Runtime Errors
    // ======================================
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Service startup failed: {0}")]
    StartupFailed(String),

    #[error("Shutdown error: {0}")]
    ShutdownError(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Internal error: {0}")]
    Internal(String),

    // ======================================
    // External Service Errors
    // ======================================
    #[error("External service error: {service}: {message}")]
    ExternalService { service: String, message: String },

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    // ======================================
    // Mapping & Routing Errors
    // ======================================
    #[error("Mapping not found: {from} -> {to}")]
    MappingNotFound { from: String, to: String },

    #[error("Routing error: {0}")]
    RoutingError(String),

    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    // ======================================
    // Catch-all for other errors
    // ======================================
    #[error("Unknown error: {0}")]
    Unknown(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias using VoltageError
pub type VoltageResult<T> = Result<T, VoltageError>;

impl VoltageError {
    /// Get the appropriate HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            // 400 Bad Request
            Self::BadRequest(_)
            | Self::Validation(_)
            | Self::InvalidParameter { .. }
            | Self::OutOfRange { .. }
            | Self::PatternMismatch { .. }
            | Self::InvalidExpression { .. }
            | Self::TypeMismatch { .. } => 400,

            // 401 Unauthorized
            Self::Unauthorized(_) => 401,

            // 403 Forbidden
            Self::Forbidden(_) => 403,

            // 404 Not Found
            Self::NotFound { .. }
            | Self::InstanceNotFound(_)
            | Self::ProductNotFound(_)
            | Self::ChannelNotFound(_)
            | Self::PointNotFound { .. }
            | Self::RuleNotFound(_)
            | Self::FileNotFound(_)
            | Self::MappingNotFound { .. } => 404,

            // 409 Conflict
            Self::Conflict { .. } | Self::AlreadyExists(_) | Self::CircularDependency(_) => 409,

            // 429 Too Many Requests
            Self::RateLimitExceeded => 429,

            // 500 Internal Server Error
            Self::Configuration(_)
            | Self::InvalidConfig { .. }
            | Self::MissingConfig(_)
            | Self::DatabaseNotFound { .. }
            | Self::Database(_)
            | Self::Sqlite(_)
            | Self::Redis(_)
            | Self::QueryFailed { .. }
            | Self::Calculation(_)
            | Self::DivisionByZero { .. }
            | Self::Processing(_)
            | Self::Internal(_)
            | Self::Runtime(_)
            | Self::Unknown(_)
            | Self::Other(_) => 500,

            // 502 Bad Gateway
            Self::Protocol { .. }
            | Self::Communication(_)
            | Self::ConnectionFailed { .. }
            | Self::Modbus(_)
            | Self::Grpc(_)
            | Self::ExternalService { .. }
            | Self::HttpClient(_) => 502,

            // 503 Service Unavailable
            Self::ServiceUnavailable(_) | Self::StartupFailed(_) | Self::ResourceBusy(_) => 503,

            // 504 Gateway Timeout
            Self::Timeout(_) => 504,

            // Other errors default to 500
            _ => 500,
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout(_)
                | Self::ServiceUnavailable(_)
                | Self::ResourceBusy(_)
                | Self::ConnectionFailed { .. }
                | Self::Communication(_)
                | Self::Redis(_)
        )
    }

    /// Convert to API ErrorInfo for HTTP responses
    pub fn to_error_info(&self) -> ErrorInfo {
        let mut error_info = ErrorInfo::new(self.to_string()).with_code(self.status_code());

        // Add details for specific error types
        match self {
            Self::InvalidParameter { param, reason } => {
                error_info = error_info.add_field_error(param, reason);
            },
            Self::Validation(msg) => {
                error_info = error_info.with_details(msg.clone());
            },
            Self::QueryFailed { query, error } => {
                error_info = error_info.with_details(format!("Query: {}, Error: {}", query, error));
            },
            _ => {},
        }

        error_info
    }
}

// Conversion traits for common error types
impl From<serde_json::Error> for VoltageError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<serde_yaml::Error> for VoltageError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Deserialization(err.to_string())
    }
}

impl From<std::num::ParseIntError> for VoltageError {
    fn from(err: std::num::ParseIntError) -> Self {
        Self::Validation(format!("Invalid integer: {}", err))
    }
}

impl From<std::num::ParseFloatError> for VoltageError {
    fn from(err: std::num::ParseFloatError) -> Self {
        Self::Validation(format!("Invalid float: {}", err))
    }
}

// Helper macros for creating errors
#[macro_export]
macro_rules! config_error {
    ($msg:expr) => {
        $crate::VoltageError::Configuration($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::VoltageError::Configuration(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! validation_error {
    ($msg:expr) => {
        $crate::VoltageError::Validation($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::VoltageError::Validation(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! protocol_error {
    ($protocol:expr, $msg:expr) => {
        $crate::VoltageError::Protocol {
            protocol: $protocol.to_string(),
            message: $msg.to_string(),
        }
    };
}

// ============================================================================
// VoltageError implements VoltageErrorTrait
// ============================================================================

impl VoltageErrorTrait for VoltageError {
    fn error_code(&self) -> &'static str {
        match self {
            // Configuration Errors
            Self::Configuration(_) => "CONFIGURATION_ERROR",
            Self::InvalidConfig { .. } => "INVALID_CONFIG",
            Self::MissingConfig(_) => "MISSING_CONFIG",
            Self::DatabaseNotFound { .. } => "DATABASE_NOT_FOUND",

            // Database Errors
            Self::Database(_) => "DATABASE_ERROR",
            Self::Sqlite(_) => "SQLITE_ERROR",
            Self::Redis(_) => "REDIS_ERROR",
            Self::QueryFailed { .. } => "QUERY_FAILED",

            // Protocol & Communication Errors
            Self::Protocol { .. } => "PROTOCOL_ERROR",
            Self::Communication(_) => "COMMUNICATION_ERROR",
            Self::ConnectionFailed { .. } => "CONNECTION_FAILED",
            Self::Timeout(_) => "TIMEOUT",
            Self::Modbus(_) => "MODBUS_ERROR",
            Self::Grpc(_) => "GRPC_ERROR",

            // Calculation & Processing
            Self::Calculation(_) => "CALCULATION_ERROR",
            Self::InvalidExpression { .. } => "INVALID_EXPRESSION",
            Self::DivisionByZero { .. } => "DIVISION_BY_ZERO",
            Self::TypeMismatch { .. } => "TYPE_MISMATCH",
            Self::Processing(_) => "PROCESSING_ERROR",

            // API & HTTP
            Self::Api(_) => "API_ERROR",
            Self::BadRequest(_) => "BAD_REQUEST",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::Conflict { .. } => "CONFLICT",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",

            // Validation
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::InvalidParameter { .. } => "INVALID_PARAMETER",
            Self::OutOfRange { .. } => "OUT_OF_RANGE",
            Self::PatternMismatch { .. } => "PATTERN_MISMATCH",

            // Resources
            Self::InstanceNotFound(_) => "INSTANCE_NOT_FOUND",
            Self::ProductNotFound(_) => "PRODUCT_NOT_FOUND",
            Self::ChannelNotFound(_) => "CHANNEL_NOT_FOUND",
            Self::PointNotFound { .. } => "POINT_NOT_FOUND",
            Self::RuleNotFound(_) => "RULE_NOT_FOUND",
            Self::ResourceBusy(_) => "RESOURCE_BUSY",
            Self::AlreadyExists(_) => "ALREADY_EXISTS",

            // File & I/O
            Self::Io(_) => "IO_ERROR",
            Self::FileNotFound(_) => "FILE_NOT_FOUND",
            Self::ParseError { .. } => "PARSE_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::Deserialization(_) => "DESERIALIZATION_ERROR",

            // Service & Runtime
            Self::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            Self::StartupFailed(_) => "STARTUP_FAILED",
            Self::ShutdownError(_) => "SHUTDOWN_ERROR",
            Self::Runtime(_) => "RUNTIME_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",

            // External Services
            Self::ExternalService { .. } => "EXTERNAL_SERVICE_ERROR",
            Self::HttpClient(_) => "HTTP_CLIENT_ERROR",

            // Mapping & Routing
            Self::MappingNotFound { .. } => "MAPPING_NOT_FOUND",
            Self::RoutingError(_) => "ROUTING_ERROR",
            Self::CircularDependency(_) => "CIRCULAR_DEPENDENCY",

            // Other
            Self::Unknown(_) => "UNKNOWN_ERROR",
            Self::Other(_) => "OTHER_ERROR",
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            // Configuration -> Configuration
            Self::Configuration(_)
            | Self::InvalidConfig { .. }
            | Self::MissingConfig(_)
            | Self::DatabaseNotFound { .. } => ErrorCategory::Configuration,

            // Database -> Database
            Self::Database(_) | Self::Sqlite(_) | Self::Redis(_) | Self::QueryFailed { .. } => {
                ErrorCategory::Database
            },

            // Protocol -> Protocol
            Self::Protocol { .. } | Self::Modbus(_) | Self::Grpc(_) => ErrorCategory::Protocol,

            // Connection -> Connection
            Self::ConnectionFailed { .. } => ErrorCategory::Connection,

            // Communication/Network -> Network
            Self::Communication(_) | Self::ServiceUnavailable(_) | Self::HttpClient(_) => {
                ErrorCategory::Network
            },

            // Timeout -> Timeout
            Self::Timeout(_) => ErrorCategory::Timeout,

            // Calculation -> Calculation
            Self::Calculation(_)
            | Self::InvalidExpression { .. }
            | Self::DivisionByZero { .. }
            | Self::TypeMismatch { .. }
            | Self::Processing(_) => ErrorCategory::Calculation,

            // Validation -> Validation
            Self::Validation(_)
            | Self::InvalidParameter { .. }
            | Self::OutOfRange { .. }
            | Self::PatternMismatch { .. }
            | Self::BadRequest(_) => ErrorCategory::Validation,

            // NotFound -> NotFound
            Self::NotFound { .. }
            | Self::InstanceNotFound(_)
            | Self::ProductNotFound(_)
            | Self::ChannelNotFound(_)
            | Self::PointNotFound { .. }
            | Self::RuleNotFound(_)
            | Self::FileNotFound(_) => ErrorCategory::NotFound,

            // Conflict -> Conflict
            Self::Conflict { .. } | Self::AlreadyExists(_) => ErrorCategory::Conflict,

            // Permission -> Permission
            Self::Unauthorized(_) | Self::Forbidden(_) => ErrorCategory::Permission,

            // ResourceBusy -> ResourceBusy
            Self::ResourceBusy(_) => ErrorCategory::ResourceBusy,

            // ResourceExhausted -> ResourceExhausted
            Self::RateLimitExceeded => ErrorCategory::ResourceExhausted,

            // Internal -> Internal
            Self::Internal(_)
            | Self::Runtime(_)
            | Self::Api(_)
            | Self::StartupFailed(_)
            | Self::ShutdownError(_) => ErrorCategory::Internal,

            // Routing/Mapping -> Internal (mapping errors are considered internal)
            Self::MappingNotFound { .. } | Self::RoutingError(_) | Self::CircularDependency(_) => {
                ErrorCategory::Internal
            },

            // Serialization/IO -> Internal
            Self::Io(_)
            | Self::ParseError { .. }
            | Self::Serialization(_)
            | Self::Deserialization(_) => ErrorCategory::Internal,

            // External Service -> Network
            Self::ExternalService { .. } => ErrorCategory::Network,

            // Unknown -> Unknown
            Self::Unknown(_) | Self::Other(_) => ErrorCategory::Unknown,
        }
    }
}

// ============================================================================
// VoltageEMS Error Trait - Architectural layer
// ============================================================================

/// Error category enum - used for classification and metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    // Infrastructure layer
    Configuration,
    Database,
    Network,
    Timeout,

    // Business logic layer
    Validation,
    NotFound,
    Conflict,
    Permission,

    // Protocol/communication layer (comsrv-specific)
    Protocol,
    Connection,

    // Calculation layer (modsrv-specific)
    Calculation,

    // Rule engine layer (rules-specific)
    RuleEngine,

    // System level
    Internal,
    ResourceBusy,
    ResourceExhausted,
    DataCorruption,

    // Others
    Unknown,
}

/// VoltageEMS error capability trait
///
/// Defines a unified interface that all VoltageEMS service error types should implement.
/// Each service can keep its own domain-specific error type (e.g., ComSrvError) and gain a common
/// interface by implementing this trait.
///
/// # Design principles
///
/// 1. Domain preservation: keep service-specific error variants
/// 2. Unified interface: present a common outward-facing interface via the trait
/// 3. Sensible defaults: provide default behavior to reduce boilerplate
/// 4. Extensible: allow services to override defaults for special logic
pub trait VoltageErrorTrait: std::error::Error + Send + Sync + 'static {
    /// Get error code (for API, logs, monitoring)
    fn error_code(&self) -> &'static str;

    /// Get error category (for classification/metrics)
    fn category(&self) -> ErrorCategory;

    /// Whether the error is retryable (default implementation is category-based)
    fn is_retryable(&self) -> bool {
        matches!(
            self.category(),
            ErrorCategory::Network | ErrorCategory::Timeout | ErrorCategory::ResourceBusy
        )
    }

    /// Recommended retry delay in milliseconds
    fn retry_delay_ms(&self) -> u64 {
        match self.category() {
            ErrorCategory::Network => 1000,
            ErrorCategory::Timeout => 500,
            ErrorCategory::ResourceBusy => 2000,
            ErrorCategory::Connection => 1500,
            _ => 0,
        }
    }

    /// Maximum retry attempts
    fn max_retries(&self) -> u32 {
        if self.is_retryable() {
            3
        } else {
            0
        }
    }

    /// Convert to HTTP status code
    #[cfg(feature = "axum-support")]
    fn http_status(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self.category() {
            ErrorCategory::Configuration => StatusCode::BAD_REQUEST,
            ErrorCategory::Validation => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCategory::NotFound => StatusCode::NOT_FOUND,
            ErrorCategory::Permission => StatusCode::FORBIDDEN,
            ErrorCategory::Conflict => StatusCode::CONFLICT,
            ErrorCategory::Timeout => StatusCode::REQUEST_TIMEOUT,
            ErrorCategory::Network => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCategory::ResourceBusy => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCategory::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Convert into an Axum HTTP response
    #[cfg(feature = "axum-support")]
    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    fn into_http_response(self) -> axum::response::Response
    where
        Self: Sized,
    {
        use axum::response::{IntoResponse, Json};
        use serde_json::json;

        (
            self.http_status(),
            Json(json!({
                "error_code": self.error_code(),
                "message": self.to_string(),
                "category": format!("{:?}", self.category()),
                "retryable": self.is_retryable(),
                "retry_delay_ms": self.retry_delay_ms(),
            })),
        )
            .into_response()
    }

    /// Get log level
    fn log_level(&self) -> tracing::Level {
        use tracing::Level;
        match self.category() {
            ErrorCategory::Internal | ErrorCategory::Database | ErrorCategory::DataCorruption => {
                Level::ERROR
            },
            ErrorCategory::Network
            | ErrorCategory::Timeout
            | ErrorCategory::Connection
            | ErrorCategory::Protocol => Level::WARN,
            ErrorCategory::Validation | ErrorCategory::NotFound => Level::INFO,
            _ => Level::WARN,
        }
    }

    /// Whether an alert should be triggered
    fn should_alert(&self) -> bool {
        matches!(
            self.category(),
            ErrorCategory::Internal | ErrorCategory::Database | ErrorCategory::DataCorruption
        )
    }
}

// Tests
#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(VoltageError::BadRequest("test".into()).status_code(), 400);
        assert_eq!(VoltageError::Unauthorized("test".into()).status_code(), 401);
        assert_eq!(
            VoltageError::NotFound {
                resource: "test".into()
            }
            .status_code(),
            404
        );
        assert_eq!(VoltageError::Internal("test".into()).status_code(), 500);
        assert_eq!(
            VoltageError::ServiceUnavailable("test".into()).status_code(),
            503
        );
    }

    #[test]
    fn test_error_retryable() {
        assert!(VoltageError::Timeout("test".into()).is_retryable());
        assert!(VoltageError::ServiceUnavailable("test".into()).is_retryable());
        assert!(!VoltageError::BadRequest("test".into()).is_retryable());
        assert!(!VoltageError::NotFound {
            resource: "test".into()
        }
        .is_retryable());
    }

    #[test]
    fn test_error_info() {
        let error = VoltageError::InvalidParameter {
            param: "name".into(),
            reason: "too short".into(),
        };
        let info = error.to_error_info();
        assert_eq!(info.code, 400);
        assert!(info.field_errors.contains_key("name"));
    }
}
