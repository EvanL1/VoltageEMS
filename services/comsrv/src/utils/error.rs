//! # Communication Service Error Handling
//!
//! This module provides comprehensive error handling for the Communication Service (ComSrv),
//! including error types, conversion utilities, and standardized error responses for API endpoints.
//!
//! ## Overview
//!
//! The error handling system is designed to provide clear, actionable error information
//! across all communication protocols and system operations. It includes automatic error
//! conversion, contextual error information, and standardized error responses for web APIs.
//!
//! ## Error Categories
//!
//! ### Configuration Errors
//! - Invalid configuration files or parameters
//! - Missing required configuration values
//! - Configuration format violations
//!
//! ### Communication Errors
//! - Protocol-specific communication failures
//! - Network connectivity issues
//! - Device communication timeouts
//!
//! ### System Errors
//! - I/O operation failures
//! - Resource access problems
//! - Internal system errors
//!
//! ### Protocol Errors
//! - Modbus communication errors
//! - IEC 60870 protocol errors
//! - MQTT messaging errors
//!
//! ## Error Context Enhancement
//!
//! The [`ErrorExt`] trait provides convenient methods for adding context to errors:
//!
//! ```rust
//! use comsrv::utils::{ErrorExt, Result};
//!
//! fn load_config() -> Result<String> {
//!     std::fs::read_to_string("config.yaml")
//!         .config_error("Failed to load main configuration file")
//! }
//!
//! fn connect_device() -> Result<()> {
//!     // ... connection logic
//!     Ok(())
//! }
//!
//! // Usage with context
//! fn initialize_system() -> Result<()> {
//!     load_config().context("System initialization failed")?;
//!     connect_device().connection_error("Failed to establish device connection")?;
//!     Ok(())
//! }
//! ```
//!
//! ## API Error Responses
//!
//! For web API endpoints, errors are converted to standardized [`ErrorResponse`] objects:
//!
//! ```rust
//! use comsrv::utils::error::{ComSrvError, ErrorResponse};
//! use warp::reply::json;
//!
//! async fn api_handler() -> Result<warp::reply::Json, warp::Rejection> {
//!     // Simulate some operation that might fail
//!     let result: Result<String, ComSrvError> = Ok("success".to_string());
//!     
//!     match result {
//!         Ok(data) => Ok(warp::reply::json(&data)),
//!         Err(e) => {
//!             let error_response = ErrorResponse::from(e);
//!             // In real code, you'd return an error response
//!             Ok(warp::reply::json(&error_response))
//!         }
//!     }
//! }
//! ```
//!
//! ## Error Recovery Strategies
//!
//! Different error types suggest different recovery strategies:
//!
//! ```
//! use comsrv::utils::{ComSrvError, Result};
//! use tokio::time::{sleep, Duration};
//!
//! async fn robust_operation() -> Result<String> {
//!     for attempt in 1..=3 {
//!         match risky_operation().await {
//!             Ok(result) => return Ok(result),
//!             Err(ComSrvError::TimeoutError(_)) |
//!             Err(ComSrvError::NetworkError(_)) => {
//!                 if attempt < 3 {
//!                     sleep(Duration::from_millis(1000 * attempt)).await;
//!                     continue;
//!                 }
//!             },
//!             Err(ComSrvError::ConfigError(_)) => {
//!                 // Don't retry configuration errors
//!                 return Err(ComSrvError::ConfigError(
//!                     "Configuration must be fixed before retrying".to_string()
//!                 ));
//!             },
//!             Err(e) => return Err(e),
//!         }
//!     }
//!     Err(ComSrvError::TimeoutError("Operation failed after 3 attempts".to_string()))
//! }
//!
//! async fn risky_operation() -> Result<String> {
//!     // Simulate a risky operation
//!     Ok("Success".to_string())
//! }
//! ```
//!
//! ## Logging Integration
//!
//! Errors integrate with the logging system for comprehensive error tracking:
//!
//! ```rust
//! use tracing::{error, warn};
//! use comsrv::utils::{ComSrvError, Result};
//!
//! fn handle_error(result: Result<()>) {
//!     if let Err(e) = result {
//!         match &e {
//!             ComSrvError::ConfigError(msg) => {
//!                 error!("Configuration error: {}", msg);
//!                 // Might need service restart
//!             },
//!             ComSrvError::TimeoutError(msg) => {
//!                 warn!("Timeout occurred: {}", msg);
//!                 // Usually recoverable
//!             },
//!             _ => {
//!                 error!("Unexpected error: {}", e);
//!             }
//!         }
//!     }
//! }
//! ```

use std::fmt::{self, Display, Formatter};
use std::io;
use thiserror::Error;

/// Comprehensive error type for all Communication Service operations
///
/// This enumeration covers all possible error conditions that can occur
/// during communication service operations, from configuration loading
/// to protocol communication and system resource management.
///
/// Each variant provides detailed context about the specific failure,
/// making it easier to diagnose issues and implement appropriate
/// recovery strategies.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ComSrvError {
    /// Configuration-related errors
    ///
    /// Covers issues with configuration files, invalid parameters,
    /// and missing required configuration values.
    ///
    /// # Examples
    /// - Invalid YAML syntax in configuration files
    /// - Missing required configuration parameters
    /// - Invalid parameter values or ranges
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Input/Output operation errors
    ///
    /// File system, network I/O, and other input/output related failures.
    ///
    /// # Examples
    /// - File not found or permission denied
    /// - Network socket creation failures
    /// - Disk full or other storage issues
    #[error("IO error: {0}")]
    IoError(String),

    /// General protocol communication errors
    ///
    /// Protocol-level communication issues that don't fit into
    /// more specific protocol categories.
    ///
    /// # Examples
    /// - Invalid protocol message format
    /// - Unsupported protocol version
    /// - Protocol state machine violations
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Connection establishment and maintenance errors
    ///
    /// Network connection issues including establishment failures,
    /// connection loss, and authentication problems.
    ///
    /// # Examples
    /// - TCP connection refused
    /// - Connection timeout
    /// - Authentication failure
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Data serialization and deserialization errors
    ///
    /// Issues with converting data between different formats
    /// including JSON, YAML, and binary protocols.
    ///
    /// # Examples
    /// - Invalid JSON format
    /// - YAML parsing errors
    /// - Binary data corruption
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Data conversion and transformation errors
    ///
    /// Issues with converting data between protocol layer and combase layer,
    /// including type conversions and value transformations.
    ///
    /// # Examples
    /// - Cannot convert boolean to float
    /// - Invalid data type for conversion
    /// - Conversion overflow or underflow
    #[error("Data conversion error: {0}")]
    DataConversionError(String),

    /// Invalid data format or content errors
    ///
    /// Issues with data validation, format checking,
    /// and content verification.
    ///
    /// # Examples
    /// - Invalid data format
    /// - Data content validation failure
    /// - Insufficient data for operation
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Operation timeout errors
    ///
    /// Operations that exceed their configured timeout limits.
    ///
    /// # Examples
    /// - Device response timeout
    /// - Database query timeout
    /// - API request timeout
    #[error("Timeout error: {0}")]
    TimeoutError(String),

    /// Modbus protocol specific errors
    ///
    /// Errors specific to Modbus communication including
    /// protocol violations and device exceptions.
    ///
    /// # Examples
    /// - Modbus exception responses
    /// - Invalid function codes
    /// - CRC validation failures
    #[error("Modbus error: {0}")]
    ModbusError(String),

    /// Redis database operation errors
    ///
    /// Issues with Redis connectivity, commands, and data operations.
    ///
    /// # Examples
    /// - Redis connection failure
    /// - Invalid Redis command
    /// - Redis authentication failure
    #[error("Redis error: {0}")]
    RedisError(String),

    /// Communication channel management errors
    ///
    /// Issues with creating, managing, or operating communication channels.
    ///
    /// # Examples
    /// - Channel creation failure
    /// - Channel not in expected state
    /// - Channel resource exhaustion
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Data parsing and format errors
    ///
    /// Issues with parsing data formats, protocol messages,
    /// and structured data validation.
    ///
    /// # Examples
    /// - Invalid CSV format
    /// - Malformed XML/JSON
    /// - Protocol message parsing failure
    #[error("Parsing error: {0}")]
    ParsingError(String),

    /// Invalid parameter or argument errors
    ///
    /// Function parameters or configuration values that are
    /// outside valid ranges or formats.
    ///
    /// # Examples
    /// - Parameter out of valid range
    /// - Invalid argument type
    /// - Missing required parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Permission and authorization errors
    ///
    /// Access control violations and permission denied errors.
    ///
    /// # Examples
    /// - File permission denied
    /// - Insufficient user privileges
    /// - Resource access forbidden
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Resource not found errors
    ///
    /// Requested resources, files, or entities that don't exist.
    ///
    /// # Examples
    /// - Configuration file not found
    /// - Channel ID not found
    /// - Requested endpoint not available
    #[error("Not found: {0}")]
    NotFound(String),

    /// Unclassified or unexpected errors
    ///
    /// Errors that don't fit into other specific categories
    /// or represent unexpected system conditions.
    ///
    /// # Examples
    /// - Unexpected system state
    /// - Third-party library errors
    /// - Runtime environment issues
    #[error("Unknown error: {0}")]
    UnknownError(String),

    /// General communication system errors
    ///
    /// High-level communication system failures that affect
    /// multiple protocols or the overall communication service.
    ///
    /// # Examples
    /// - Communication service shutdown
    /// - System resource exhaustion
    /// - Critical service component failure
    #[error("Communication error: {0}")]
    CommunicationError(String),

    /// Unsupported protocol errors
    ///
    /// Attempts to use protocols or features that are not
    /// supported in the current configuration.
    ///
    /// # Examples
    /// - Protocol not compiled in
    /// - Feature not enabled
    /// - Unsupported protocol version
    #[error("Protocol not supported: {0}")]
    ProtocolNotSupported(String),

    /// Channel lookup and access errors
    ///
    /// Issues with finding or accessing specific communication channels.
    ///
    /// # Examples
    /// - Channel ID not found
    /// - Channel not initialized
    /// - Channel access denied
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    /// Point table management errors
    ///
    /// Issues with loading, parsing, or managing point table configurations.
    ///
    /// # Examples
    /// - Point table file not found
    /// - Invalid point table format
    /// - Point table validation failure
    #[error("Point table error: {0}")]
    PointTableError(String),

    /// Point data access errors
    ///
    /// Issues with accessing specific data points or point configurations.
    ///
    /// # Examples
    /// - Point ID not found
    /// - Point data not available
    /// - Point access permission denied
    #[error("Point not found: {0}")]
    PointNotFound(String),

    /// Invalid operation or state errors
    ///
    /// Operations attempted in invalid states or contexts.
    ///
    /// # Examples
    /// - Operation not allowed in current state
    /// - Invalid operation sequence
    /// - Resource already in use
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Web API specific errors
    ///
    /// Issues specific to HTTP API endpoints and web service operations.
    ///
    /// # Examples
    /// - Invalid API request format
    /// - API authentication failure
    /// - API rate limiting
    #[error("API error: {0}")]
    ApiError(String),

    /// Internal system errors
    ///
    /// Library or service internal errors that indicate bugs
    /// or unexpected conditions within the system itself.
    ///
    /// # Examples
    /// - Internal state corruption
    /// - Unexpected code path execution
    /// - Resource management failures
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Network communication errors
    ///
    /// Low-level network communication issues including
    /// connectivity and transport layer problems.
    ///
    /// # Examples
    /// - Network interface down
    /// - DNS resolution failure
    /// - Routing problems
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Thread synchronization and locking errors
    ///
    /// Issues with thread synchronization primitives such as
    /// mutexes, locks, and other concurrent access mechanisms.
    ///
    /// # Examples
    /// - Mutex poisoned by panic
    /// - Lock acquisition timeout
    /// - Deadlock detection
    #[error("Lock error: {0}")]
    LockError(String),

    /// System state-related errors
    ///
    /// Errors related to invalid system or component states,
    /// such as attempting operations when the system is not ready.
    ///
    /// # Examples
    /// - Service not initialized
    /// - Component already running
    /// - Invalid state transition
    #[error("State error: {0}")]
    StateError(String),

    /// Resource exhaustion errors
    ///
    /// Errors that occur when system resources are exhausted
    /// or usage limits are exceeded.
    ///
    /// # Examples
    /// - Connection pool full
    /// - Memory limit exceeded
    /// - Too many open connections
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Configuration validation and setup errors
    ///
    /// Specific errors related to configuration validation,
    /// parameter checking, and system setup issues.
    ///
    /// # Examples
    /// - Invalid configuration parameter value
    /// - Missing required configuration section
    /// - Configuration dependency not met
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// Extension trait for enhancing error handling with context information
///
/// This trait provides convenient methods for adding context to errors,
/// making them more informative and easier to debug. It supports both
/// static context and dynamic context generation.
///
/// # Examples
///
/// ```rust
/// use comsrv::utils::{ErrorExt, Result};
///
/// fn read_config_file() -> Result<String> {
///     std::fs::read_to_string("config.yaml")
///         .config_error("Failed to read configuration file")
/// }
///
/// fn dynamic_context() -> Result<String> {
///     std::fs::read_to_string("data.txt")
///         .with_context(|| format!("Failed to read file at {}", std::env::current_dir().unwrap().display()))
/// }
/// ```
pub trait ErrorExt<T> {
    /// Add context to any error with a static message
    ///
    /// Maps any error to a `ComSrvError::UnknownError` with additional context.
    ///
    /// # Arguments
    ///
    /// * `context` - Context message to add to the error
    ///
    /// # Returns
    ///
    /// Result with enhanced error information
    fn context<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>;

    /// Add dynamic context to any error
    ///
    /// Similar to `context()` but allows for dynamic context generation
    /// using a closure, which is only called if an error occurs.
    ///
    /// # Arguments
    ///
    /// * `f` - Closure that generates the context message
    ///
    /// # Returns
    ///
    /// Result with enhanced error information
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: AsRef<str>,
        F: FnOnce() -> C;

    /// Map error to ConfigError variant with context
    ///
    /// Specifically maps errors to `ComSrvError::ConfigError`,
    /// indicating configuration-related issues.
    ///
    /// # Arguments
    ///
    /// * `context` - Context message describing the configuration issue
    ///
    /// # Returns
    ///
    /// Result with ConfigError variant
    fn config_error<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>;

    /// Map error to ProtocolError variant with context
    ///
    /// Specifically maps errors to `ComSrvError::ProtocolError`,
    /// indicating protocol communication issues.
    ///
    /// # Arguments
    ///
    /// * `context` - Context message describing the protocol issue
    ///
    /// # Returns
    ///
    /// Result with ProtocolError variant
    fn protocol_error<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>;

    /// Map error to ConnectionError variant with context
    ///
    /// Specifically maps errors to `ComSrvError::ConnectionError`,
    /// indicating network connection issues.
    ///
    /// # Arguments
    ///
    /// * `context` - Context message describing the connection issue
    ///
    /// # Returns
    ///
    /// Result with ConnectionError variant
    fn connection_error<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ErrorExt<T> for std::result::Result<T, E> {
    fn context<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>,
    {
        self.map_err(|e| ComSrvError::UnknownError(format!("{}: {}", context.as_ref(), e)))
    }

    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: AsRef<str>,
        F: FnOnce() -> C,
    {
        self.map_err(|e| ComSrvError::UnknownError(format!("{}: {}", f().as_ref(), e)))
    }

    fn config_error<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>,
    {
        self.map_err(|e| ComSrvError::ConfigError(format!("{}: {}", context.as_ref(), e)))
    }

    fn protocol_error<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>,
    {
        self.map_err(|e| ComSrvError::ProtocolError(format!("{}: {}", context.as_ref(), e)))
    }

    fn connection_error<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>,
    {
        self.map_err(|e| ComSrvError::ConnectionError(format!("{}: {}", context.as_ref(), e)))
    }
}

/// Convert from serde_yaml error to ComSrvError
///
/// Automatically converts YAML parsing errors to appropriate
/// ComSrvError variants for consistent error handling.
impl From<serde_yaml::Error> for ComSrvError {
    fn from(err: serde_yaml::Error) -> Self {
        ComSrvError::SerializationError(err.to_string())
    }
}

/// Convert from io::Error to ComSrvError
///
/// Automatically converts standard I/O errors to ComSrvError
/// for consistent error handling across the system.
impl From<io::Error> for ComSrvError {
    fn from(err: io::Error) -> Self {
        ComSrvError::IoError(err.to_string())
    }
}

/// Convert from redis error to ComSrvError
///
/// Automatically converts Redis client errors to appropriate
/// ComSrvError variants.
impl From<redis::RedisError> for ComSrvError {
    fn from(err: redis::RedisError) -> Self {
        ComSrvError::RedisError(err.to_string())
    }
}

/// Convert from serde_json error to ComSrvError
impl From<serde_json::Error> for ComSrvError {
    fn from(err: serde_json::Error) -> Self {
        ComSrvError::SerializationError(err.to_string())
    }
}

/// Convert from tokio_serial error to ComSrvError
impl From<tokio_serial::Error> for ComSrvError {
    fn from(err: tokio_serial::Error) -> Self {
        ComSrvError::CommunicationError(format!("Serial port error: {}", err))
    }
}

/// Convert from axum HTTP error to ComSrvError
impl From<axum::http::Error> for ComSrvError {
    fn from(error: axum::http::Error) -> Self {
        ComSrvError::InternalError(format!("HTTP error: {}", error))
    }
}

/// Convert from address parse error to ComSrvError
impl From<std::net::AddrParseError> for ComSrvError {
    fn from(err: std::net::AddrParseError) -> Self {
        ComSrvError::ConfigError(format!("Address parse error: {}", err))
    }
}

/// Convert from transport error to ComSrvError
impl From<crate::core::transport::traits::TransportError> for ComSrvError {
    fn from(err: crate::core::transport::traits::TransportError) -> Self {
        match err {
            crate::core::transport::traits::TransportError::ConnectionFailed(msg) => ComSrvError::ConnectionError(msg),
            crate::core::transport::traits::TransportError::ConnectionLost(msg) => ComSrvError::ConnectionError(msg),
            crate::core::transport::traits::TransportError::SendFailed(msg) => ComSrvError::CommunicationError(msg),
            crate::core::transport::traits::TransportError::ReceiveFailed(msg) => ComSrvError::CommunicationError(msg),
            crate::core::transport::traits::TransportError::Timeout(msg) => ComSrvError::TimeoutError(msg),
            crate::core::transport::traits::TransportError::ConfigError(msg) => ComSrvError::ConfigError(msg),
            crate::core::transport::traits::TransportError::IoError(msg) => ComSrvError::IoError(msg),
            crate::core::transport::traits::TransportError::ProtocolError(msg) => ComSrvError::ProtocolError(msg),
        }
    }
}

/// Shorthand for Result with ComSrvError
pub type Result<T> = std::result::Result<T, ComSrvError>;

/// HTTP API error response structure
///
/// Standardized error response format for HTTP API endpoints.
/// Provides consistent error reporting across all API operations.
///
/// # Fields
///
/// * `code` - Error code identifier for programmatic handling
/// * `message` - Human-readable error description
///
/// # JSON Format
///
/// ```json
/// {
///   "code": "CONFIG_ERROR",
///   "message": "Invalid configuration parameter: timeout must be positive"
/// }
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

impl Display for ErrorResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl From<ComSrvError> for ErrorResponse {
    fn from(err: ComSrvError) -> Self {
        match err {
            ComSrvError::ConfigError(_) => ErrorResponse::new("config_error", &err.to_string()),
            ComSrvError::IoError(_) => ErrorResponse::new("io_error", &err.to_string()),
            ComSrvError::SerializationError(_) => {
                ErrorResponse::new("serialization_error", &err.to_string())
            }
            ComSrvError::CommunicationError(_) => {
                ErrorResponse::new("communication_error", &err.to_string())
            }
            ComSrvError::ProtocolError(_) => ErrorResponse::new("protocol_error", &err.to_string()),
            ComSrvError::ProtocolNotSupported(_) => {
                ErrorResponse::new("protocol_not_supported", &err.to_string())
            }
            ComSrvError::ChannelError(_) => ErrorResponse::new("channel_error", &err.to_string()),
            ComSrvError::ChannelNotFound(_) => {
                ErrorResponse::new("channel_not_found", &err.to_string())
            }
            ComSrvError::PointTableError(_) => {
                ErrorResponse::new("point_table_error", &err.to_string())
            }
            ComSrvError::PointNotFound(_) => {
                ErrorResponse::new("point_not_found", &err.to_string())
            }
            ComSrvError::InvalidOperation(_) => {
                ErrorResponse::new("invalid_operation", &err.to_string())
            }
            ComSrvError::ConnectionError(_) => {
                ErrorResponse::new("connection_error", &err.to_string())
            }
            ComSrvError::RedisError(_) => ErrorResponse::new("redis_error", &err.to_string()),
            ComSrvError::ApiError(_) => ErrorResponse::new("api_error", &err.to_string()),
            ComSrvError::InternalError(_) => ErrorResponse::new("internal_error", &err.to_string()),
            ComSrvError::UnknownError(_) => ErrorResponse::new("unknown_error", &err.to_string()),
            ComSrvError::NetworkError(_) => ErrorResponse::new("network_error", &err.to_string()),
            ComSrvError::LockError(_) => ErrorResponse::new("lock_error", &err.to_string()),
            _ => ErrorResponse::new("unknown_error", &err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let config_error = ComSrvError::ConfigError("Test config error".to_string());
        assert_eq!(
            config_error.to_string(),
            "Configuration error: Test config error"
        );

        let io_error = ComSrvError::IoError("Test IO error".to_string());
        assert_eq!(io_error.to_string(), "IO error: Test IO error");

        let protocol_error = ComSrvError::ProtocolError("Test protocol error".to_string());
        assert_eq!(
            protocol_error.to_string(),
            "Protocol error: Test protocol error"
        );
    }

    #[test]
    fn test_error_from_conversion() {
        let serde_error = serde_yaml::from_str::<u32>("invalid yaml").unwrap_err();
        let comsrv_error: ComSrvError = serde_error.into();
        assert!(matches!(comsrv_error, ComSrvError::SerializationError(_)));

        let json_error = serde_json::from_str::<u32>("invalid json").unwrap_err();
        let comsrv_error: ComSrvError = json_error.into();
        assert!(matches!(comsrv_error, ComSrvError::SerializationError(_)));
    }

    #[test]
    fn test_error_ext() {
        let result: std::result::Result<u32, std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ));

        let with_context = result.context("Custom context");
        assert!(with_context.is_err());

        let error = with_context.unwrap_err();
        assert!(error.to_string().contains("Custom context"));
        assert!(error.to_string().contains("File not found"));
    }

    #[test]
    fn test_error_ext_variants() {
        let result: std::result::Result<u32, std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Test error",
        ));

        let config_error = result.config_error("Config context");
        assert!(config_error.is_err());
        assert!(matches!(
            config_error.unwrap_err(),
            ComSrvError::ConfigError(_)
        ));
    }

    #[test]
    fn test_error_response() {
        let error_response = ErrorResponse::new("TEST001", "Test error message");
        assert_eq!(error_response.code, "TEST001");
        assert_eq!(error_response.message, "Test error message");
        assert_eq!(error_response.to_string(), "TEST001: Test error message");
    }

    #[test]
    fn test_error_response_from_comsrv_error() {
        let comsrv_error = ComSrvError::ConfigError("Configuration problem".to_string());
        let error_response: ErrorResponse = comsrv_error.into();
        assert_eq!(error_response.code, "config_error");
        assert!(error_response.message.contains("Configuration problem"));
    }

    #[test]
    fn test_all_error_variants() {
        let errors = vec![
            ComSrvError::ConfigError("config".to_string()),
            ComSrvError::IoError("io".to_string()),
            ComSrvError::ProtocolError("protocol".to_string()),
            ComSrvError::ConnectionError("connection".to_string()),
            ComSrvError::SerializationError("serialization".to_string()),
            ComSrvError::TimeoutError("timeout".to_string()),
            ComSrvError::ModbusError("modbus".to_string()),
            ComSrvError::RedisError("redis".to_string()),
            ComSrvError::NetworkError("mqtt".to_string()),
            ComSrvError::ChannelError("channel".to_string()),
            ComSrvError::ParsingError("parsing".to_string()),
            ComSrvError::InvalidParameter("invalid param".to_string()),
            ComSrvError::PermissionDenied("permission".to_string()),
            ComSrvError::NotFound("not found".to_string()),
            ComSrvError::UnknownError("unknown".to_string()),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
