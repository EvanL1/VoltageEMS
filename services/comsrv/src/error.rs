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

// ============================================================================
// Backward Compatibility Aliases (deprecated, will be removed)
// ============================================================================

impl ComSrvError {
    // Legacy constructors for backward compatibility
    #[deprecated(note = "Use DataError instead")]
    pub fn serialization(msg: impl Into<String>) -> Self {
        ComSrvError::DataError(format!("Serialization: {}", msg.into()))
    }

    #[deprecated(note = "Use DataError instead")]
    pub fn data_conversion(msg: impl Into<String>) -> Self {
        ComSrvError::DataError(format!("Conversion: {}", msg.into()))
    }

    #[deprecated(note = "Use DataError instead")]
    pub fn invalid_data(msg: impl Into<String>) -> Self {
        ComSrvError::DataError(format!("Invalid: {}", msg.into()))
    }

    #[deprecated(note = "Use DataError instead")]
    pub fn parsing(msg: impl Into<String>) -> Self {
        ComSrvError::DataError(format!("Parsing: {}", msg.into()))
    }

    #[deprecated(note = "Use ProtocolError instead")]
    pub fn modbus(msg: impl Into<String>) -> Self {
        ComSrvError::ProtocolError(format!("Modbus: {}", msg.into()))
    }

    #[deprecated(note = "Use StorageError instead")]
    pub fn redis(msg: impl Into<String>) -> Self {
        ComSrvError::StorageError(format!("Redis: {}", msg.into()))
    }

    // Current constructors
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

impl From<voltage_modbus::ModbusError> for ComSrvError {
    fn from(err: voltage_modbus::ModbusError) -> Self {
        ComSrvError::ProtocolError(format!("Modbus: {}", err))
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
}

// ============================================================================
// API Adaptation: ComSrvError → AppError conversion
// ============================================================================

impl From<ComSrvError> for common::AppError {
    fn from(err: ComSrvError) -> Self {
        use common::{AppError, ErrorInfo};
        use errors::VoltageErrorTrait;

        let status = err.http_status();
        let error_info = ErrorInfo::new(err.to_string())
            .with_code(status.as_u16())
            .with_details(format!(
                "error_code: {}, category: {:?}, retryable: {}",
                err.error_code(),
                err.category(),
                err.is_retryable()
            ));

        AppError::new(status, error_info)
    }
}
