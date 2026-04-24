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

impl From<serde_yml::Error> for ComSrvError {
    fn from(err: serde_yml::Error) -> Self {
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

    /// Helper macro to test constructor + display + error_code + category in one shot
    macro_rules! test_error_variant {
        ($constructor:ident, $variant:pat, $msg:expr, $code:expr, $category:expr) => {
            let err = ComSrvError::$constructor($msg);
            assert!(matches!(err, $variant));
            assert!(err.to_string().contains($msg));
            assert_eq!(err.error_code(), $code);
            assert_eq!(err.category(), $category);
        };
    }

    #[test]
    fn test_all_constructors_and_traits() {
        test_error_variant!(
            config,
            ComSrvError::ConfigError(_),
            "missing field",
            "COMSRV_CONFIG_ERROR",
            ErrorCategory::Configuration
        );
        test_error_variant!(
            io,
            ComSrvError::IoError(_),
            "read failed",
            "COMSRV_IO_ERROR",
            ErrorCategory::Internal
        );
        test_error_variant!(
            protocol,
            ComSrvError::ProtocolError(_),
            "invalid frame",
            "COMSRV_PROTOCOL_ERROR",
            ErrorCategory::Protocol
        );
        test_error_variant!(
            connection,
            ComSrvError::ConnectionError(_),
            "refused",
            "COMSRV_CONNECTION_ERROR",
            ErrorCategory::Connection
        );
        test_error_variant!(
            data,
            ComSrvError::DataError(_),
            "invalid format",
            "COMSRV_DATA_ERROR",
            ErrorCategory::Validation
        );
        test_error_variant!(
            timeout,
            ComSrvError::TimeoutError(_),
            "5000ms",
            "COMSRV_TIMEOUT",
            ErrorCategory::Timeout
        );
        test_error_variant!(
            storage,
            ComSrvError::StorageError(_),
            "redis down",
            "COMSRV_STORAGE_ERROR",
            ErrorCategory::Database
        );
        test_error_variant!(
            resource,
            ComSrvError::ResourceError(_),
            "pool exhausted",
            "COMSRV_RESOURCE_ERROR",
            ErrorCategory::ResourceExhausted
        );
        test_error_variant!(
            channel,
            ComSrvError::ChannelError(_),
            "closed",
            "COMSRV_CHANNEL_ERROR",
            ErrorCategory::NotFound
        );
        test_error_variant!(
            point,
            ComSrvError::PointError(_),
            "bad address",
            "COMSRV_POINT_ERROR",
            ErrorCategory::NotFound
        );
        test_error_variant!(
            validation,
            ComSrvError::ValidationError(_),
            "out of range",
            "COMSRV_VALIDATION_ERROR",
            ErrorCategory::Validation
        );
        test_error_variant!(
            permission,
            ComSrvError::PermissionError(_),
            "access denied",
            "COMSRV_PERMISSION_ERROR",
            ErrorCategory::Permission
        );
        test_error_variant!(
            state,
            ComSrvError::StateError(_),
            "lock poisoned",
            "COMSRV_STATE_ERROR",
            ErrorCategory::ResourceBusy
        );
        test_error_variant!(
            batch,
            ComSrvError::BatchError(_),
            "3 failed",
            "COMSRV_BATCH_ERROR",
            ErrorCategory::Internal
        );
        test_error_variant!(
            internal,
            ComSrvError::InternalError(_),
            "unexpected",
            "COMSRV_INTERNAL_ERROR",
            ErrorCategory::Internal
        );
    }

    #[test]
    fn test_convenience_constructors() {
        let err = ComSrvError::channel_not_found(1001);
        assert!(err.to_string().contains("not found") && err.to_string().contains("1001"));

        let err = ComSrvError::channel_exists(1002);
        assert!(err.to_string().contains("already exists") && err.to_string().contains("1002"));

        let err = ComSrvError::invalid_channel_id(99999);
        assert!(err.to_string().contains("Invalid channel ID"));

        let err = ComSrvError::point_not_found("T:100");
        assert!(err.to_string().contains("not found") && err.to_string().contains("T:100"));

        let err = ComSrvError::not_connected();
        assert!(matches!(err, ComSrvError::ConnectionError(_)));
    }

    #[test]
    fn test_from_external_errors() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: ComSrvError = io_err.into();
        assert!(matches!(err, ComSrvError::IoError(_)));

        let json_err = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
        let err: ComSrvError = json_err.into();
        assert!(matches!(err, ComSrvError::DataError(_)) && err.to_string().contains("JSON"));

        let yaml_err = serde_yml::from_str::<serde_yml::Value>("invalid: yaml: :").unwrap_err();
        let err: ComSrvError = yaml_err.into();
        assert!(matches!(err, ComSrvError::DataError(_)) && err.to_string().contains("YAML"));

        let anyhow_err = anyhow::anyhow!("something went wrong");
        let err: ComSrvError = anyhow_err.into();
        assert!(matches!(err, ComSrvError::ConfigError(_)));
    }

    #[test]
    fn test_is_retryable() {
        assert!(ComSrvError::TimeoutError("".into()).is_retryable());
        assert!(ComSrvError::StateError("".into()).is_retryable());
        assert!(!ComSrvError::ConfigError("".into()).is_retryable());
        assert!(!ComSrvError::ValidationError("".into()).is_retryable());
    }

    #[test]
    fn test_suggestions() {
        assert!(ComSrvError::ConfigError("t".into())
            .suggestion()
            .unwrap()
            .contains("monarch sync"));
        assert!(ComSrvError::channel_not_found(1)
            .suggestion()
            .unwrap()
            .contains("monarch channels"));
        assert!(ComSrvError::channel_exists(1)
            .suggestion()
            .unwrap()
            .contains("already exists"));
        assert!(ComSrvError::point_not_found("T:1")
            .suggestion()
            .unwrap()
            .contains("/api/channels"));
        assert!(ComSrvError::ConnectionError("t".into())
            .suggestion()
            .unwrap()
            .contains("reachable"));
        assert!(ComSrvError::ProtocolError("t".into())
            .suggestion()
            .unwrap()
            .contains("Modbus"));
        assert!(ComSrvError::TimeoutError("t".into())
            .suggestion()
            .unwrap()
            .contains("timeout"));
        assert!(ComSrvError::StorageError("t".into())
            .suggestion()
            .unwrap()
            .contains("monarch doctor"));
        assert!(ComSrvError::DataError("t".into())
            .suggestion()
            .unwrap()
            .contains("scale/offset"));
        assert!(ComSrvError::ValidationError("t".into())
            .suggestion()
            .is_none());
    }

    #[test]
    fn test_to_voltage_error_conversions() {
        assert!(matches!(
            VoltageError::from(ComSrvError::ConfigError("t".into())),
            VoltageError::Configuration(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::IoError("t".into())),
            VoltageError::Io(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::ProtocolError("t".into())),
            VoltageError::Protocol { .. }
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::ConnectionError("t".into())),
            VoltageError::Communication(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::DataError("t".into())),
            VoltageError::Validation(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::TimeoutError("t".into())),
            VoltageError::Timeout(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::StorageError("t".into())),
            VoltageError::Database(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::ResourceError("t".into())),
            VoltageError::ResourceBusy(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::channel_not_found(1)),
            VoltageError::ChannelNotFound(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::channel_exists(1)),
            VoltageError::AlreadyExists(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::ChannelError("other".into())),
            VoltageError::Processing(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::PointError("t".into())),
            VoltageError::NotFound { .. }
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::ValidationError("t".into())),
            VoltageError::Validation(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::PermissionError("t".into())),
            VoltageError::Forbidden(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::StateError("t".into())),
            VoltageError::Internal(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::BatchError("t".into())),
            VoltageError::Internal(_)
        ));
        assert!(matches!(
            VoltageError::from(ComSrvError::InternalError("t".into())),
            VoltageError::Internal(_)
        ));
    }

    #[test]
    fn test_error_ext_trait() {
        // Test each conversion method
        let err: Result<()> = Err::<(), &str>("test").config_error("cfg");
        assert!(matches!(err.unwrap_err(), ComSrvError::ConfigError(_)));

        let err: Result<()> = Err::<(), &str>("test").io_error("io");
        assert!(matches!(err.unwrap_err(), ComSrvError::IoError(_)));

        let err: Result<()> = Err::<(), &str>("test").protocol_error("proto");
        assert!(matches!(err.unwrap_err(), ComSrvError::ProtocolError(_)));

        let err: Result<()> = Err::<(), &str>("test").connection_error("conn");
        assert!(matches!(err.unwrap_err(), ComSrvError::ConnectionError(_)));

        let err: Result<()> = Err::<(), &str>("test").data_error("data");
        assert!(matches!(err.unwrap_err(), ComSrvError::DataError(_)));

        let err: Result<()> = Err::<(), &str>("test").context("ctx");
        assert!(matches!(err.unwrap_err(), ComSrvError::InternalError(_)));

        // Ok values pass through
        assert_eq!(Ok::<i32, &str>(42).config_error("nope").unwrap(), 42);
    }

    #[test]
    fn test_error_display_format() {
        assert_eq!(
            ComSrvError::ConfigError("missing key".into()).to_string(),
            "Configuration error: missing key"
        );
        assert_eq!(
            ComSrvError::IoError("read failed".into()).to_string(),
            "IO error: read failed"
        );
    }

    #[test]
    fn test_debug() {
        let err = ComSrvError::ConfigError("test".into());
        assert!(format!("{:?}", err).contains("ConfigError"));
    }
}
