//! Error handling for Communication Service
//!
//! This module provides error type definitions and conversions for the Communication Service.
//! Error types have been consolidated from 27 variants to 15 for maintainability.

use errors::VoltageError;
use thiserror::Error;

/// Communication Service Error Type (Simplified: 15 variants)
#[derive(Error, Debug, Clone)]
pub enum ComSrvError {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Input/Output operation errors
    #[error("IO error: {0}")]
    IoError(String),

    /// Protocol communication errors (includes Modbus)
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Connection establishment and maintenance errors (includes NotConnected)
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Data handling errors (serialization, parsing, conversion, validation)
    #[error("Data error: {0}")]
    DataError(String),

    /// Operation timeout errors
    #[error("Timeout error: {0}")]
    TimeoutError(String),

    /// Storage errors (Redis, database)
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Resource errors (exhaustion, busy)
    #[error("Resource error: {0}")]
    ResourceError(String),

    /// Channel errors (not found, exists, operation failed)
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Point errors (not found, table error)
    #[error("Point error: {0}")]
    PointError(String),

    /// Validation errors (invalid parameter, operation, not supported)
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Permission errors
    #[error("Permission error: {0}")]
    PermissionError(String),

    /// State and synchronization errors (lock, sync)
    #[error("State error: {0}")]
    StateError(String),

    /// Batch operation errors
    #[error("Batch error: {0}")]
    BatchError(String),

    /// Internal errors (unknown, API, general)
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Result type alias for Communication Service
pub type Result<T> = std::result::Result<T, ComSrvError>;

impl ComSrvError {
    pub fn config(msg: impl Into<String>) -> Self {
        ComSrvError::ConfigError(msg.into())
    }

    pub fn io(msg: impl Into<String>) -> Self {
        ComSrvError::IoError(msg.into())
    }

    pub fn protocol(msg: impl Into<String>) -> Self {
        ComSrvError::ProtocolError(msg.into())
    }

    pub fn connection(msg: impl Into<String>) -> Self {
        ComSrvError::ConnectionError(msg.into())
    }

    pub fn data(msg: impl Into<String>) -> Self {
        ComSrvError::DataError(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        ComSrvError::TimeoutError(msg.into())
    }

    pub fn storage(msg: impl Into<String>) -> Self {
        ComSrvError::StorageError(msg.into())
    }

    pub fn resource(msg: impl Into<String>) -> Self {
        ComSrvError::ResourceError(msg.into())
    }

    pub fn channel(msg: impl Into<String>) -> Self {
        ComSrvError::ChannelError(msg.into())
    }

    pub fn point(msg: impl Into<String>) -> Self {
        ComSrvError::PointError(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        ComSrvError::ValidationError(msg.into())
    }

    pub fn permission(msg: impl Into<String>) -> Self {
        ComSrvError::PermissionError(msg.into())
    }

    pub fn state(msg: impl Into<String>) -> Self {
        ComSrvError::StateError(msg.into())
    }

    pub fn batch(msg: impl Into<String>) -> Self {
        ComSrvError::BatchError(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        ComSrvError::InternalError(msg.into())
    }

    // Convenience constructors for specific cases
    pub fn channel_not_found(id: impl std::fmt::Display) -> Self {
        ComSrvError::ChannelError(format!("Channel not found: {}", id))
    }

    pub fn channel_exists(id: u32) -> Self {
        ComSrvError::ChannelError(format!("Channel already exists: {}", id))
    }

    /// Invalid channel ID (out of bounds for pre-allocated Vec)
    pub fn invalid_channel_id(id: u32) -> Self {
        ComSrvError::ChannelError(format!("Invalid channel ID: {} (must be < 10000)", id))
    }

    pub fn point_not_found(id: impl std::fmt::Display) -> Self {
        ComSrvError::PointError(format!("Point not found: {}", id))
    }

    pub fn not_connected() -> Self {
        ComSrvError::ConnectionError("Not connected".to_string())
    }
}

// ============================================================================
// From implementations for external error types
// ============================================================================

impl From<std::io::Error> for ComSrvError {
    fn from(err: std::io::Error) -> Self {
        ComSrvError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for ComSrvError {
    fn from(err: serde_json::Error) -> Self {
        ComSrvError::DataError(format!("JSON: {err}"))
    }
}

impl From<serde_yaml::Error> for ComSrvError {
    fn from(err: serde_yaml::Error) -> Self {
        ComSrvError::DataError(format!("YAML: {err}"))
    }
}

impl From<anyhow::Error> for ComSrvError {
    fn from(err: anyhow::Error) -> Self {
        ComSrvError::ConfigError(format!("Validation: {err}"))
    }
}

impl From<crate::protocols::GatewayError> for ComSrvError {
    fn from(err: crate::protocols::GatewayError) -> Self {
        use crate::protocols::GatewayError;
        match err {
            // Connection errors
            GatewayError::Connection(msg) => ComSrvError::ConnectionError(msg),
            GatewayError::NotConnected => ComSrvError::ConnectionError("Not connected".into()),
            GatewayError::ConnectionTimeout(ms) => {
                ComSrvError::TimeoutError(format!("Connection timeout: {}ms", ms))
            },
            GatewayError::ChannelClosed => ComSrvError::ChannelError("Channel closed".into()),

            // Protocol errors
            GatewayError::Protocol(msg) => ComSrvError::ProtocolError(msg),
            GatewayError::InvalidResponse(msg) => {
                ComSrvError::ProtocolError(format!("Invalid response: {}", msg))
            },
            GatewayError::Modbus(msg) => ComSrvError::ProtocolError(format!("Modbus: {}", msg)),
            GatewayError::Iec104(msg) => ComSrvError::ProtocolError(format!("IEC 104: {}", msg)),
            GatewayError::Dnp3(msg) => ComSrvError::ProtocolError(format!("DNP3: {}", msg)),
            GatewayError::OpcUa(msg) => ComSrvError::ProtocolError(format!("OPC UA: {}", msg)),

            // Data errors
            GatewayError::InvalidData(msg) => ComSrvError::DataError(msg),
            GatewayError::DataConversion(msg) => {
                ComSrvError::DataError(format!("Conversion: {}", msg))
            },
            GatewayError::PointNotFound(id) => {
                ComSrvError::PointError(format!("Not found: {}", id))
            },

            // Configuration errors
            GatewayError::Config(msg) => ComSrvError::ConfigError(msg),
            GatewayError::InvalidAddress(msg) => {
                ComSrvError::ConfigError(format!("Invalid address: {}", msg))
            },
            GatewayError::Unsupported(msg) => {
                ComSrvError::ValidationError(format!("Unsupported: {}", msg))
            },

            // IO/Timeout errors
            GatewayError::Io(io_err) => ComSrvError::IoError(io_err.to_string()),
            GatewayError::ReadTimeout => ComSrvError::TimeoutError("Read timeout".into()),
            GatewayError::WriteTimeout => ComSrvError::TimeoutError("Write timeout".into()),

            // Internal errors
            GatewayError::Internal(msg) => ComSrvError::InternalError(msg),
        }
    }
}

// ============================================================================
// Extension trait for adding context to errors
// ============================================================================

/// Extension trait for adding context to errors
pub trait ErrorExt<T> {
    fn config_error(self, msg: &str) -> Result<T>;
    fn io_error(self, msg: &str) -> Result<T>;
    fn protocol_error(self, msg: &str) -> Result<T>;
    fn connection_error(self, msg: &str) -> Result<T>;
    fn data_error(self, msg: &str) -> Result<T>;
    fn context(self, msg: &str) -> Result<T>;
}

impl<T, E> ErrorExt<T> for std::result::Result<T, E>
where
    E: std::fmt::Display,
{
    fn config_error(self, msg: &str) -> Result<T> {
        self.map_err(|e| ComSrvError::ConfigError(format!("{msg}: {e}")))
    }

    fn io_error(self, msg: &str) -> Result<T> {
        self.map_err(|e| ComSrvError::IoError(format!("{msg}: {e}")))
    }

    fn protocol_error(self, msg: &str) -> Result<T> {
        self.map_err(|e| ComSrvError::ProtocolError(format!("{msg}: {e}")))
    }

    fn connection_error(self, msg: &str) -> Result<T> {
        self.map_err(|e| ComSrvError::ConnectionError(format!("{msg}: {e}")))
    }

    fn data_error(self, msg: &str) -> Result<T> {
        self.map_err(|e| ComSrvError::DataError(format!("{msg}: {e}")))
    }

    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| ComSrvError::InternalError(format!("{msg}: {e}")))
    }
}

// ============================================================================
// Conversion from ComSrvError to VoltageError for API boundaries
// ============================================================================

impl From<ComSrvError> for VoltageError {
    fn from(err: ComSrvError) -> Self {
        match err {
            ComSrvError::ConfigError(msg) => VoltageError::Configuration(msg),
            ComSrvError::IoError(msg) => VoltageError::Io(std::io::Error::other(msg)),
            ComSrvError::ProtocolError(msg) => VoltageError::Protocol {
                protocol: "comsrv".to_string(),
                message: msg,
            },
            ComSrvError::ConnectionError(msg) => VoltageError::Communication(msg),
            ComSrvError::DataError(msg) => VoltageError::Validation(msg),
            ComSrvError::TimeoutError(msg) => VoltageError::Timeout(msg),
            ComSrvError::StorageError(msg) => VoltageError::Database(msg),
            ComSrvError::ResourceError(msg) => VoltageError::ResourceBusy(msg),
            ComSrvError::ChannelError(msg) => {
                if msg.contains("not found") {
                    VoltageError::ChannelNotFound(msg)
                } else if msg.contains("exists") {
                    VoltageError::AlreadyExists(msg)
                } else {
                    VoltageError::Processing(msg)
                }
            },
            ComSrvError::PointError(msg) => VoltageError::NotFound {
                resource: format!("Point: {}", msg),
            },
            ComSrvError::ValidationError(msg) => VoltageError::Validation(msg),
            ComSrvError::PermissionError(msg) => VoltageError::Forbidden(msg),
            ComSrvError::StateError(msg) => VoltageError::Internal(msg),
            ComSrvError::BatchError(msg) => VoltageError::Internal(msg),
            ComSrvError::InternalError(msg) => VoltageError::Internal(msg),
        }
    }
}

// ============================================================================
// ComSrvError implements VoltageErrorTrait
// ============================================================================

use errors::{ErrorCategory, VoltageErrorTrait};

impl VoltageErrorTrait for ComSrvError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::ConfigError(_) => "COMSRV_CONFIG_ERROR",
            Self::IoError(_) => "COMSRV_IO_ERROR",
            Self::ProtocolError(_) => "COMSRV_PROTOCOL_ERROR",
            Self::ConnectionError(_) => "COMSRV_CONNECTION_ERROR",
            Self::DataError(_) => "COMSRV_DATA_ERROR",
            Self::TimeoutError(_) => "COMSRV_TIMEOUT",
            Self::StorageError(_) => "COMSRV_STORAGE_ERROR",
            Self::ResourceError(_) => "COMSRV_RESOURCE_ERROR",
            Self::ChannelError(_) => "COMSRV_CHANNEL_ERROR",
            Self::PointError(_) => "COMSRV_POINT_ERROR",
            Self::ValidationError(_) => "COMSRV_VALIDATION_ERROR",
            Self::PermissionError(_) => "COMSRV_PERMISSION_ERROR",
            Self::StateError(_) => "COMSRV_STATE_ERROR",
            Self::BatchError(_) => "COMSRV_BATCH_ERROR",
            Self::InternalError(_) => "COMSRV_INTERNAL_ERROR",
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            Self::ConfigError(_) => ErrorCategory::Configuration,
            Self::IoError(_) => ErrorCategory::Internal,
            Self::ProtocolError(_) => ErrorCategory::Protocol,
            Self::ConnectionError(_) => ErrorCategory::Connection,
            Self::DataError(_) => ErrorCategory::Validation,
            Self::TimeoutError(_) => ErrorCategory::Timeout,
            Self::StorageError(_) => ErrorCategory::Database,
            Self::ResourceError(_) => ErrorCategory::ResourceExhausted,
            Self::ChannelError(_) => ErrorCategory::NotFound,
            Self::PointError(_) => ErrorCategory::NotFound,
            Self::ValidationError(_) => ErrorCategory::Validation,
            Self::PermissionError(_) => ErrorCategory::Permission,
            Self::StateError(_) => ErrorCategory::ResourceBusy,
            Self::BatchError(_) => ErrorCategory::Internal,
            Self::InternalError(_) => ErrorCategory::Internal,
        }
    }

    fn suggestion(&self) -> Option<String> {
        match self {
            Self::ConfigError(_) => Some(
                "Check comsrv configuration in config/comsrv/ and run 'monarch sync comsrv'".to_string()
            ),
            Self::ChannelError(msg) => {
                if msg.contains("not found") {
                    Some("Use 'monarch channels list' to see available channels".to_string())
                } else if msg.contains("exists") {
                    Some("Channel already exists. Use a different ID or update the existing channel".to_string())
                } else {
                    Some("Check channel configuration and status with 'monarch channels status <id>'".to_string())
                }
            },
            Self::PointError(msg) => {
                if msg.contains("not found") {
                    Some("Verify the point exists in the channel configuration. Use GET /api/channels/{id}/points to list points".to_string())
                } else {
                    Some("Check point configuration in the channel's CSV files".to_string())
                }
            },
            Self::ConnectionError(_) => Some(
                "Verify the device is reachable and check network/serial port settings".to_string()
            ),
            Self::ProtocolError(_) => Some(
                "Check protocol configuration (Modbus slave ID, function codes, register addresses)".to_string()
            ),
            Self::TimeoutError(_) => Some(
                "Increase timeout settings or check device responsiveness".to_string()
            ),
            Self::StorageError(_) => Some(
                "Run 'monarch doctor' to check Redis connection".to_string()
            ),
            Self::ValidationError(_) => None, // Validation errors should be specific in the message
            Self::DataError(_) => Some(
                "Check data format and types. Verify scale/offset configuration in point definitions".to_string()
            ),
            _ => None,
        }
    }
}

// ============================================================================
// API Adaptation: ComSrvError → AppError conversion
// ============================================================================

impl From<ComSrvError> for common::AppError {
    fn from(err: ComSrvError) -> Self {
        use common::{AppError, ErrorInfo};
        use errors::VoltageErrorTrait;

        let status = err.http_status();
        let mut error_info = ErrorInfo::new(err.to_string())
            .with_code(status.as_u16())
            .with_details(format!(
                "error_code: {}, category: {:?}, retryable: {}",
                err.error_code(),
                err.category(),
                err.is_retryable()
            ));

        // Add suggestion if available
        if let Some(suggestion) = err.suggestion() {
            error_info = error_info.with_suggestion(suggestion);
        }

        AppError::new(status, error_info)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use errors::{ErrorCategory, VoltageErrorTrait};

    // ========================================================================
    // Constructor tests - verify all error constructors work correctly
    // ========================================================================

    #[test]
    fn test_config_constructor() {
        let err = ComSrvError::config("missing field");
        assert!(matches!(err, ComSrvError::ConfigError(_)));
        assert!(err.to_string().contains("missing field"));
    }

    #[test]
    fn test_io_constructor() {
        let err = ComSrvError::io("read failed");
        assert!(matches!(err, ComSrvError::IoError(_)));
        assert!(err.to_string().contains("read failed"));
    }

    #[test]
    fn test_protocol_constructor() {
        let err = ComSrvError::protocol("invalid frame");
        assert!(matches!(err, ComSrvError::ProtocolError(_)));
        assert!(err.to_string().contains("invalid frame"));
    }

    #[test]
    fn test_connection_constructor() {
        let err = ComSrvError::connection("refused");
        assert!(matches!(err, ComSrvError::ConnectionError(_)));
        assert!(err.to_string().contains("refused"));
    }

    #[test]
    fn test_data_constructor() {
        let err = ComSrvError::data("invalid format");
        assert!(matches!(err, ComSrvError::DataError(_)));
        assert!(err.to_string().contains("invalid format"));
    }

    #[test]
    fn test_timeout_constructor() {
        let err = ComSrvError::timeout("5000ms");
        assert!(matches!(err, ComSrvError::TimeoutError(_)));
        assert!(err.to_string().contains("5000ms"));
    }

    #[test]
    fn test_storage_constructor() {
        let err = ComSrvError::storage("redis unavailable");
        assert!(matches!(err, ComSrvError::StorageError(_)));
        assert!(err.to_string().contains("redis unavailable"));
    }

    #[test]
    fn test_resource_constructor() {
        let err = ComSrvError::resource("pool exhausted");
        assert!(matches!(err, ComSrvError::ResourceError(_)));
        assert!(err.to_string().contains("pool exhausted"));
    }

    #[test]
    fn test_channel_constructor() {
        let err = ComSrvError::channel("closed");
        assert!(matches!(err, ComSrvError::ChannelError(_)));
        assert!(err.to_string().contains("closed"));
    }

    #[test]
    fn test_point_constructor() {
        let err = ComSrvError::point("invalid address");
        assert!(matches!(err, ComSrvError::PointError(_)));
        assert!(err.to_string().contains("invalid address"));
    }

    #[test]
    fn test_validation_constructor() {
        let err = ComSrvError::validation("out of range");
        assert!(matches!(err, ComSrvError::ValidationError(_)));
        assert!(err.to_string().contains("out of range"));
    }

    #[test]
    fn test_permission_constructor() {
        let err = ComSrvError::permission("access denied");
        assert!(matches!(err, ComSrvError::PermissionError(_)));
        assert!(err.to_string().contains("access denied"));
    }

    #[test]
    fn test_state_constructor() {
        let err = ComSrvError::state("lock poisoned");
        assert!(matches!(err, ComSrvError::StateError(_)));
        assert!(err.to_string().contains("lock poisoned"));
    }

    #[test]
    fn test_batch_constructor() {
        let err = ComSrvError::batch("3 operations failed");
        assert!(matches!(err, ComSrvError::BatchError(_)));
        assert!(err.to_string().contains("3 operations failed"));
    }

    #[test]
    fn test_internal_constructor() {
        let err = ComSrvError::internal("unexpected state");
        assert!(matches!(err, ComSrvError::InternalError(_)));
        assert!(err.to_string().contains("unexpected state"));
    }

    // ========================================================================
    // Convenience constructor tests
    // ========================================================================

    #[test]
    fn test_channel_not_found() {
        let err = ComSrvError::channel_not_found(1001);
        assert!(matches!(err, ComSrvError::ChannelError(_)));
        assert!(err.to_string().contains("not found"));
        assert!(err.to_string().contains("1001"));
    }

    #[test]
    fn test_channel_exists() {
        let err = ComSrvError::channel_exists(1002);
        assert!(matches!(err, ComSrvError::ChannelError(_)));
        assert!(err.to_string().contains("already exists"));
        assert!(err.to_string().contains("1002"));
    }

    #[test]
    fn test_invalid_channel_id() {
        let err = ComSrvError::invalid_channel_id(99999);
        assert!(matches!(err, ComSrvError::ChannelError(_)));
        assert!(err.to_string().contains("Invalid channel ID"));
        assert!(err.to_string().contains("99999"));
    }

    #[test]
    fn test_point_not_found() {
        let err = ComSrvError::point_not_found("T:100");
        assert!(matches!(err, ComSrvError::PointError(_)));
        assert!(err.to_string().contains("not found"));
        assert!(err.to_string().contains("T:100"));
    }

    #[test]
    fn test_not_connected() {
        let err = ComSrvError::not_connected();
        assert!(matches!(err, ComSrvError::ConnectionError(_)));
        assert!(err.to_string().contains("Not connected"));
    }

    // ========================================================================
    // From trait implementation tests
    // ========================================================================

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: ComSrvError = io_err.into();
        assert!(matches!(err, ComSrvError::IoError(_)));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let err: ComSrvError = json_err.into();
        assert!(matches!(err, ComSrvError::DataError(_)));
        assert!(err.to_string().contains("JSON"));
    }

    #[test]
    fn test_from_serde_yaml_error() {
        let yaml_err = serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: :").unwrap_err();
        let err: ComSrvError = yaml_err.into();
        assert!(matches!(err, ComSrvError::DataError(_)));
        assert!(err.to_string().contains("YAML"));
    }

    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let err: ComSrvError = anyhow_err.into();
        assert!(matches!(err, ComSrvError::ConfigError(_)));
        assert!(err.to_string().contains("Validation"));
    }

    // ========================================================================
    // VoltageErrorTrait implementation tests
    // ========================================================================

    #[test]
    fn test_error_codes() {
        assert_eq!(
            ComSrvError::ConfigError("".into()).error_code(),
            "COMSRV_CONFIG_ERROR"
        );
        assert_eq!(
            ComSrvError::IoError("".into()).error_code(),
            "COMSRV_IO_ERROR"
        );
        assert_eq!(
            ComSrvError::ProtocolError("".into()).error_code(),
            "COMSRV_PROTOCOL_ERROR"
        );
        assert_eq!(
            ComSrvError::ConnectionError("".into()).error_code(),
            "COMSRV_CONNECTION_ERROR"
        );
        assert_eq!(
            ComSrvError::DataError("".into()).error_code(),
            "COMSRV_DATA_ERROR"
        );
        assert_eq!(
            ComSrvError::TimeoutError("".into()).error_code(),
            "COMSRV_TIMEOUT"
        );
        assert_eq!(
            ComSrvError::StorageError("".into()).error_code(),
            "COMSRV_STORAGE_ERROR"
        );
        assert_eq!(
            ComSrvError::ResourceError("".into()).error_code(),
            "COMSRV_RESOURCE_ERROR"
        );
        assert_eq!(
            ComSrvError::ChannelError("".into()).error_code(),
            "COMSRV_CHANNEL_ERROR"
        );
        assert_eq!(
            ComSrvError::PointError("".into()).error_code(),
            "COMSRV_POINT_ERROR"
        );
        assert_eq!(
            ComSrvError::ValidationError("".into()).error_code(),
            "COMSRV_VALIDATION_ERROR"
        );
        assert_eq!(
            ComSrvError::PermissionError("".into()).error_code(),
            "COMSRV_PERMISSION_ERROR"
        );
        assert_eq!(
            ComSrvError::StateError("".into()).error_code(),
            "COMSRV_STATE_ERROR"
        );
        assert_eq!(
            ComSrvError::BatchError("".into()).error_code(),
            "COMSRV_BATCH_ERROR"
        );
        assert_eq!(
            ComSrvError::InternalError("".into()).error_code(),
            "COMSRV_INTERNAL_ERROR"
        );
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(
            ComSrvError::ConfigError("".into()).category(),
            ErrorCategory::Configuration
        );
        assert_eq!(
            ComSrvError::IoError("".into()).category(),
            ErrorCategory::Internal
        );
        assert_eq!(
            ComSrvError::ProtocolError("".into()).category(),
            ErrorCategory::Protocol
        );
        assert_eq!(
            ComSrvError::ConnectionError("".into()).category(),
            ErrorCategory::Connection
        );
        assert_eq!(
            ComSrvError::DataError("".into()).category(),
            ErrorCategory::Validation
        );
        assert_eq!(
            ComSrvError::TimeoutError("".into()).category(),
            ErrorCategory::Timeout
        );
        assert_eq!(
            ComSrvError::StorageError("".into()).category(),
            ErrorCategory::Database
        );
        assert_eq!(
            ComSrvError::ResourceError("".into()).category(),
            ErrorCategory::ResourceExhausted
        );
        assert_eq!(
            ComSrvError::ChannelError("".into()).category(),
            ErrorCategory::NotFound
        );
        assert_eq!(
            ComSrvError::PointError("".into()).category(),
            ErrorCategory::NotFound
        );
        assert_eq!(
            ComSrvError::ValidationError("".into()).category(),
            ErrorCategory::Validation
        );
        assert_eq!(
            ComSrvError::PermissionError("".into()).category(),
            ErrorCategory::Permission
        );
        assert_eq!(
            ComSrvError::StateError("".into()).category(),
            ErrorCategory::ResourceBusy
        );
        assert_eq!(
            ComSrvError::BatchError("".into()).category(),
            ErrorCategory::Internal
        );
        assert_eq!(
            ComSrvError::InternalError("".into()).category(),
            ErrorCategory::Internal
        );
    }

    #[test]
    fn test_is_retryable() {
        // Retryable errors (Network, Timeout, ResourceBusy categories)
        assert!(ComSrvError::TimeoutError("".into()).is_retryable());
        assert!(ComSrvError::StateError("".into()).is_retryable()); // ResourceBusy

        // Non-retryable errors
        assert!(!ComSrvError::ConfigError("".into()).is_retryable());
        assert!(!ComSrvError::ValidationError("".into()).is_retryable());
        assert!(!ComSrvError::ChannelError("".into()).is_retryable());
        assert!(!ComSrvError::InternalError("".into()).is_retryable());
    }

    // ========================================================================
    // Suggestion tests
    // ========================================================================

    #[test]
    fn test_config_error_suggestion() {
        let err = ComSrvError::ConfigError("test".into());
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("monarch sync"));
    }

    #[test]
    fn test_channel_not_found_suggestion() {
        let err = ComSrvError::channel_not_found(1001);
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("monarch channels list"));
    }

    #[test]
    fn test_channel_exists_suggestion() {
        let err = ComSrvError::channel_exists(1001);
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("already exists"));
    }

    #[test]
    fn test_point_not_found_suggestion() {
        let err = ComSrvError::point_not_found("T:100");
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("/api/channels"));
    }

    #[test]
    fn test_connection_error_suggestion() {
        let err = ComSrvError::ConnectionError("test".into());
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("reachable"));
    }

    #[test]
    fn test_protocol_error_suggestion() {
        let err = ComSrvError::ProtocolError("test".into());
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("Modbus"));
    }

    #[test]
    fn test_timeout_error_suggestion() {
        let err = ComSrvError::TimeoutError("test".into());
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("timeout"));
    }

    #[test]
    fn test_storage_error_suggestion() {
        let err = ComSrvError::StorageError("test".into());
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("monarch doctor"));
    }

    #[test]
    fn test_data_error_suggestion() {
        let err = ComSrvError::DataError("test".into());
        let suggestion = err.suggestion();
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("scale/offset"));
    }

    #[test]
    fn test_validation_error_no_suggestion() {
        let err = ComSrvError::ValidationError("test".into());
        assert!(err.suggestion().is_none());
    }

    // ========================================================================
    // ComSrvError to VoltageError conversion tests
    // ========================================================================

    #[test]
    fn test_to_voltage_error_config() {
        let err: VoltageError = ComSrvError::ConfigError("test".into()).into();
        assert!(matches!(err, VoltageError::Configuration(_)));
    }

    #[test]
    fn test_to_voltage_error_io() {
        let err: VoltageError = ComSrvError::IoError("test".into()).into();
        assert!(matches!(err, VoltageError::Io(_)));
    }

    #[test]
    fn test_to_voltage_error_protocol() {
        let err: VoltageError = ComSrvError::ProtocolError("test".into()).into();
        assert!(matches!(err, VoltageError::Protocol { .. }));
    }

    #[test]
    fn test_to_voltage_error_connection() {
        let err: VoltageError = ComSrvError::ConnectionError("test".into()).into();
        assert!(matches!(err, VoltageError::Communication(_)));
    }

    #[test]
    fn test_to_voltage_error_data() {
        let err: VoltageError = ComSrvError::DataError("test".into()).into();
        assert!(matches!(err, VoltageError::Validation(_)));
    }

    #[test]
    fn test_to_voltage_error_timeout() {
        let err: VoltageError = ComSrvError::TimeoutError("test".into()).into();
        assert!(matches!(err, VoltageError::Timeout(_)));
    }

    #[test]
    fn test_to_voltage_error_storage() {
        let err: VoltageError = ComSrvError::StorageError("test".into()).into();
        assert!(matches!(err, VoltageError::Database(_)));
    }

    #[test]
    fn test_to_voltage_error_resource() {
        let err: VoltageError = ComSrvError::ResourceError("test".into()).into();
        assert!(matches!(err, VoltageError::ResourceBusy(_)));
    }

    #[test]
    fn test_to_voltage_error_channel_not_found() {
        let err: VoltageError = ComSrvError::channel_not_found(1001).into();
        assert!(matches!(err, VoltageError::ChannelNotFound(_)));
    }

    #[test]
    fn test_to_voltage_error_channel_exists() {
        let err: VoltageError = ComSrvError::channel_exists(1001).into();
        assert!(matches!(err, VoltageError::AlreadyExists(_)));
    }

    #[test]
    fn test_to_voltage_error_channel_other() {
        let err: VoltageError = ComSrvError::ChannelError("some error".into()).into();
        assert!(matches!(err, VoltageError::Processing(_)));
    }

    #[test]
    fn test_to_voltage_error_point() {
        let err: VoltageError = ComSrvError::PointError("test".into()).into();
        assert!(matches!(err, VoltageError::NotFound { .. }));
    }

    #[test]
    fn test_to_voltage_error_validation() {
        let err: VoltageError = ComSrvError::ValidationError("test".into()).into();
        assert!(matches!(err, VoltageError::Validation(_)));
    }

    #[test]
    fn test_to_voltage_error_permission() {
        let err: VoltageError = ComSrvError::PermissionError("test".into()).into();
        assert!(matches!(err, VoltageError::Forbidden(_)));
    }

    #[test]
    fn test_to_voltage_error_state() {
        let err: VoltageError = ComSrvError::StateError("test".into()).into();
        assert!(matches!(err, VoltageError::Internal(_)));
    }

    #[test]
    fn test_to_voltage_error_batch() {
        let err: VoltageError = ComSrvError::BatchError("test".into()).into();
        assert!(matches!(err, VoltageError::Internal(_)));
    }

    #[test]
    fn test_to_voltage_error_internal() {
        let err: VoltageError = ComSrvError::InternalError("test".into()).into();
        assert!(matches!(err, VoltageError::Internal(_)));
    }

    // ========================================================================
    // ErrorExt trait tests
    // ========================================================================

    #[test]
    fn test_error_ext_config_error() {
        let result: core::result::Result<(), &str> = Err("test");
        let converted: Result<()> = result.config_error("config failed");
        let err = converted.unwrap_err();
        assert!(matches!(err, ComSrvError::ConfigError(_)));
        assert!(err.to_string().contains("config failed"));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_error_ext_io_error() {
        let result: core::result::Result<(), &str> = Err("test");
        let converted: Result<()> = result.io_error("io failed");
        let err = converted.unwrap_err();
        assert!(matches!(err, ComSrvError::IoError(_)));
    }

    #[test]
    fn test_error_ext_protocol_error() {
        let result: core::result::Result<(), &str> = Err("test");
        let converted: Result<()> = result.protocol_error("protocol failed");
        let err = converted.unwrap_err();
        assert!(matches!(err, ComSrvError::ProtocolError(_)));
    }

    #[test]
    fn test_error_ext_connection_error() {
        let result: core::result::Result<(), &str> = Err("test");
        let converted: Result<()> = result.connection_error("connection failed");
        let err = converted.unwrap_err();
        assert!(matches!(err, ComSrvError::ConnectionError(_)));
    }

    #[test]
    fn test_error_ext_data_error() {
        let result: core::result::Result<(), &str> = Err("test");
        let converted: Result<()> = result.data_error("data failed");
        let err = converted.unwrap_err();
        assert!(matches!(err, ComSrvError::DataError(_)));
    }

    #[test]
    fn test_error_ext_context() {
        let result: core::result::Result<(), &str> = Err("test");
        let converted: Result<()> = result.context("context failed");
        let err = converted.unwrap_err();
        assert!(matches!(err, ComSrvError::InternalError(_)));
    }

    #[test]
    fn test_error_ext_on_ok() {
        let result: std::result::Result<i32, &str> = Ok(42);
        let value = result.config_error("should not happen").unwrap();
        assert_eq!(value, 42);
    }

    // ========================================================================
    // Error display format tests
    // ========================================================================

    #[test]
    fn test_error_display_format() {
        let err = ComSrvError::ConfigError("missing key".into());
        assert_eq!(err.to_string(), "Configuration error: missing key");

        let err = ComSrvError::IoError("read failed".into());
        assert_eq!(err.to_string(), "IO error: read failed");

        let err = ComSrvError::ProtocolError("invalid frame".into());
        assert_eq!(err.to_string(), "Protocol error: invalid frame");
    }

    // ========================================================================
    // Clone and Debug trait tests
    // ========================================================================

    #[test]
    fn test_error_clone() {
        let err = ComSrvError::ConfigError("test".into());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_error_debug() {
        let err = ComSrvError::ConfigError("test".into());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("ConfigError"));
        assert!(debug_str.contains("test"));
    }
}
