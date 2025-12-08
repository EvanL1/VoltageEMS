//! Error handling for Communication Service
//!
//! This module provides error type definitions and conversions for the Communication Service,
//! adapting voltage-common error types to maintain backward compatibility.

use common::error::Error as CommonError;
use thiserror::Error;
use voltage_config::error::VoltageError;

/// Communication Service Error Type
#[derive(Error, Debug, Clone)]
pub enum ComSrvError {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Input/Output operation errors
    #[error("IO error: {0}")]
    IoError(String),

    /// General protocol communication errors
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Connection establishment and maintenance errors
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Not connected error
    #[error("Not connected")]
    NotConnected,

    /// Not supported error
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// Data serialization and deserialization errors
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Data conversion and transformation errors
    #[error("Data conversion error: {0}")]
    DataConversionError(String),

    /// Invalid data format or content errors
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Operation timeout errors
    #[error("Timeout error: {0}")]
    TimeoutError(String),

    /// Modbus protocol specific errors
    #[error("Modbus error: {0}")]
    ModbusError(String),

    /// Redis data access errors
    #[error("Redis error: {0}")]
    RedisError(String),

    /// Resource access and permission errors
    #[error("Resource error: {0}")]
    ResourceError(String),

    /// General internal errors
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Invalid parameter errors
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Channel not found errors
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    /// Channel operation errors
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Point not found errors
    #[error("Point not found: {0}")]
    PointNotFound(String),

    /// Point table errors
    #[error("Point table error: {0}")]
    PointTableError(String),

    /// Invalid operation errors
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Channel already exists
    #[error("Channel already exists: {0}")]
    ChannelExists(u16),

    /// Batch operation failed
    #[error("Batch operation failed: {0}")]
    BatchOperationFailed(String),

    /// Synchronization error
    #[error("Sync error: {0}")]
    SyncError(String),

    /// Parsing errors
    #[error("Parsing error: {0}")]
    ParsingError(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// State errors
    #[error("State error: {0}")]
    StateError(String),

    /// Lock errors
    #[error("Lock error: {0}")]
    LockError(String),

    /// Resource exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Unknown errors
    #[error("Unknown error: {0}")]
    UnknownError(String),

    /// API errors
    #[error("API error: {0}")]
    ApiError(String),
}

/// Result type alias for Communication Service
pub type Result<T> = std::result::Result<T, ComSrvError>;

// Conversion from common::Error to ComSrvError
impl From<CommonError> for ComSrvError {
    #[allow(unreachable_patterns)]
    fn from(err: CommonError) -> Self {
        match err {
            CommonError::Config(msg) => ComSrvError::ConfigError(msg),
            CommonError::Io(e) => ComSrvError::IoError(e.to_string()),
            CommonError::Serialization(msg) => ComSrvError::SerializationError(msg),
            CommonError::Parse(msg) => ComSrvError::InvalidData(msg),
            CommonError::Timeout(msg) => ComSrvError::TimeoutError(msg),
            CommonError::Generic(msg) => ComSrvError::InternalError(msg),
            CommonError::Redis(msg) => ComSrvError::RedisError(msg),
            CommonError::Http(msg) => ComSrvError::ConnectionError(msg),
            _ => ComSrvError::InternalError("Unknown error".to_string()),
        }
    }
}

// Conversion from std::io::Error
impl From<std::io::Error> for ComSrvError {
    fn from(err: std::io::Error) -> Self {
        ComSrvError::IoError(err.to_string())
    }
}

// Conversion from serde_json::Error
impl From<serde_json::Error> for ComSrvError {
    fn from(err: serde_json::Error) -> Self {
        ComSrvError::SerializationError(format!("JSON error: {err}"))
    }
}

// Conversion from serde_yaml::Error
impl From<serde_yaml::Error> for ComSrvError {
    fn from(err: serde_yaml::Error) -> Self {
        ComSrvError::SerializationError(format!("YAML error: {err}"))
    }
}

// Conversion from figment::Error
impl From<figment::Error> for ComSrvError {
    fn from(err: figment::Error) -> Self {
        ComSrvError::ConfigError(format!("Configuration error: {err}"))
    }
}

// Conversion from voltage_comlink::ComLinkError
impl From<voltage_comlink::ComLinkError> for ComSrvError {
    fn from(err: voltage_comlink::ComLinkError) -> Self {
        use voltage_comlink::ComLinkError;
        match err {
            ComLinkError::Protocol(msg) => ComSrvError::ProtocolError(msg),
            ComLinkError::Connection(msg) => ComSrvError::ConnectionError(msg),
            ComLinkError::NotConnected => ComSrvError::NotConnected,
            ComLinkError::Io(msg) => ComSrvError::IoError(msg),
            ComLinkError::Timeout(msg) => ComSrvError::TimeoutError(msg),
            ComLinkError::InvalidData(msg) => ComSrvError::InvalidData(msg),
            ComLinkError::DataConversion(msg) => ComSrvError::DataConversionError(msg),
            ComLinkError::Config(msg) => ComSrvError::ConfigError(msg),
            ComLinkError::ChannelNotFound(id) => ComSrvError::ChannelNotFound(id.to_string()),
            ComLinkError::PointNotFound(msg) => ComSrvError::PointNotFound(msg),
            ComLinkError::NotSupported(msg) => ComSrvError::NotSupported(msg),
            ComLinkError::Internal(msg) => ComSrvError::InternalError(msg),
            ComLinkError::Modbus(msg) => ComSrvError::ModbusError(msg),
            ComLinkError::Can(msg) => ComSrvError::ProtocolError(format!("CAN: {}", msg)),
        }
    }
}

// Conversion from anyhow::Error
impl From<anyhow::Error> for ComSrvError {
    fn from(err: anyhow::Error) -> Self {
        ComSrvError::ConfigError(format!("Validation error: {err}"))
    }
}

// Conversion from voltage_modbus::ModbusError
impl From<voltage_modbus::ModbusError> for ComSrvError {
    fn from(err: voltage_modbus::ModbusError) -> Self {
        ComSrvError::ModbusError(err.to_string())
    }
}

// Helper methods for creating errors
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

    pub fn serialization(msg: impl Into<String>) -> Self {
        ComSrvError::SerializationError(msg.into())
    }

    pub fn data_conversion(msg: impl Into<String>) -> Self {
        ComSrvError::DataConversionError(msg.into())
    }

    pub fn invalid_data(msg: impl Into<String>) -> Self {
        ComSrvError::InvalidData(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        ComSrvError::TimeoutError(msg.into())
    }

    pub fn modbus(msg: impl Into<String>) -> Self {
        ComSrvError::ModbusError(msg.into())
    }

    pub fn redis(msg: impl Into<String>) -> Self {
        ComSrvError::RedisError(msg.into())
    }

    pub fn resource(msg: impl Into<String>) -> Self {
        ComSrvError::ResourceError(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        ComSrvError::InternalError(msg.into())
    }
}

/// Extension trait for adding context to errors
pub trait ErrorExt<T> {
    fn config_error(self, msg: &str) -> Result<T>;
    fn io_error(self, msg: &str) -> Result<T>;
    fn protocol_error(self, msg: &str) -> Result<T>;
    fn connection_error(self, msg: &str) -> Result<T>;
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

    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| ComSrvError::InternalError(format!("{msg}: {e}")))
    }
}

// Conversion from ComSrvError to VoltageError for API boundaries
impl From<ComSrvError> for VoltageError {
    fn from(err: ComSrvError) -> Self {
        match err {
            // Configuration errors
            ComSrvError::ConfigError(msg) => VoltageError::Configuration(msg),

            // I/O errors
            ComSrvError::IoError(msg) => VoltageError::Io(std::io::Error::other(msg)),

            // Protocol and communication errors
            ComSrvError::ProtocolError(msg) | ComSrvError::ModbusError(msg) => {
                VoltageError::Protocol {
                    protocol: "comsrv".to_string(),
                    message: msg,
                }
            },

            ComSrvError::ConnectionError(msg) => VoltageError::Communication(msg),

            ComSrvError::NotConnected => VoltageError::Communication("Not connected".to_string()),

            // Timeout errors
            ComSrvError::TimeoutError(msg) => VoltageError::Timeout(msg),

            // Redis errors
            ComSrvError::RedisError(msg) => VoltageError::Database(format!("Redis: {}", msg)),

            // Data errors
            ComSrvError::SerializationError(msg) => VoltageError::Serialization(msg),
            ComSrvError::DataConversionError(msg)
            | ComSrvError::InvalidData(msg)
            | ComSrvError::ParsingError(msg) => VoltageError::Validation(msg),

            // Resource errors
            ComSrvError::ChannelNotFound(id) => VoltageError::ChannelNotFound(id),
            ComSrvError::PointNotFound(msg) => VoltageError::NotFound {
                resource: format!("Point: {}", msg),
            },
            ComSrvError::ChannelExists(id) => {
                VoltageError::AlreadyExists(format!("Channel {}", id))
            },

            // Invalid operations
            ComSrvError::InvalidParameter(msg) => VoltageError::InvalidParameter {
                param: "unknown".to_string(),
                reason: msg,
            },
            ComSrvError::InvalidOperation(msg) | ComSrvError::NotSupported(msg) => {
                VoltageError::Validation(msg)
            },

            // Permission and state errors
            ComSrvError::PermissionDenied(msg) => VoltageError::Forbidden(msg),
            ComSrvError::StateError(msg) | ComSrvError::LockError(msg) => {
                VoltageError::Internal(msg)
            },

            // Resource exhaustion
            ComSrvError::ResourceExhausted(msg) => VoltageError::ResourceBusy(msg),

            // General errors
            ComSrvError::InternalError(msg)
            | ComSrvError::UnknownError(msg)
            | ComSrvError::SyncError(msg)
            | ComSrvError::BatchOperationFailed(msg) => VoltageError::Internal(msg),

            // API errors
            ComSrvError::ApiError(msg) => VoltageError::Api(msg),

            // Point table and channel errors
            ComSrvError::PointTableError(msg)
            | ComSrvError::ChannelError(msg)
            | ComSrvError::ResourceError(msg) => VoltageError::Processing(msg),
        }
    }
}

// ============================================================================
// ComSrvError implements VoltageErrorTrait
// ============================================================================

use voltage_config::error::{ErrorCategory, VoltageErrorTrait};

impl VoltageErrorTrait for ComSrvError {
    fn error_code(&self) -> &'static str {
        match self {
            // Configuration
            Self::ConfigError(_) => "COMSRV_CONFIG_ERROR",

            // IO
            Self::IoError(_) => "COMSRV_IO_ERROR",

            // Protocol
            Self::ProtocolError(_) => "COMSRV_PROTOCOL_ERROR",
            Self::ModbusError(_) => "COMSRV_MODBUS_ERROR",

            // Connection
            Self::ConnectionError(_) => "COMSRV_CONNECTION_ERROR",
            Self::NotConnected => "COMSRV_NOT_CONNECTED",

            // Timeout
            Self::TimeoutError(_) => "COMSRV_TIMEOUT",

            // Data Handling
            Self::SerializationError(_) => "COMSRV_SERIALIZATION_ERROR",
            Self::DataConversionError(_) => "COMSRV_DATA_CONVERSION_ERROR",
            Self::InvalidData(_) => "COMSRV_INVALID_DATA",
            Self::ParsingError(_) => "COMSRV_PARSING_ERROR",

            // Resources
            Self::ChannelNotFound(_) => "COMSRV_CHANNEL_NOT_FOUND",
            Self::ChannelExists(_) => "COMSRV_CHANNEL_EXISTS",
            Self::ChannelError(_) => "COMSRV_CHANNEL_ERROR",
            Self::PointNotFound(_) => "COMSRV_POINT_NOT_FOUND",
            Self::PointTableError(_) => "COMSRV_POINT_TABLE_ERROR",
            Self::ResourceError(_) => "COMSRV_RESOURCE_ERROR",
            Self::ResourceExhausted(_) => "COMSRV_RESOURCE_EXHAUSTED",

            // Validation
            Self::InvalidParameter(_) => "COMSRV_INVALID_PARAMETER",
            Self::InvalidOperation(_) => "COMSRV_INVALID_OPERATION",
            Self::NotSupported(_) => "COMSRV_NOT_SUPPORTED",

            // Redis
            Self::RedisError(_) => "COMSRV_REDIS_ERROR",

            // Sync and Batch Operations
            Self::SyncError(_) => "COMSRV_SYNC_ERROR",
            Self::BatchOperationFailed(_) => "COMSRV_BATCH_OPERATION_FAILED",

            // State and Locking
            Self::StateError(_) => "COMSRV_STATE_ERROR",
            Self::LockError(_) => "COMSRV_LOCK_ERROR",

            // Permission
            Self::PermissionDenied(_) => "COMSRV_PERMISSION_DENIED",

            // API
            Self::ApiError(_) => "COMSRV_API_ERROR",

            // General
            Self::InternalError(_) => "COMSRV_INTERNAL_ERROR",
            Self::UnknownError(_) => "COMSRV_UNKNOWN_ERROR",
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            // Configuration → Configuration
            Self::ConfigError(_) => ErrorCategory::Configuration,

            // Protocol → Protocol
            Self::ProtocolError(_) | Self::ModbusError(_) => ErrorCategory::Protocol,

            // Connection → Connection
            Self::ConnectionError(_) | Self::NotConnected => ErrorCategory::Connection,

            // Timeout → Timeout
            Self::TimeoutError(_) => ErrorCategory::Timeout,

            // Database → Database
            Self::RedisError(_) => ErrorCategory::Database,

            // Validation → Validation
            Self::InvalidParameter(_)
            | Self::InvalidOperation(_)
            | Self::InvalidData(_)
            | Self::NotSupported(_) => ErrorCategory::Validation,

            // NotFound → NotFound
            Self::ChannelNotFound(_) | Self::PointNotFound(_) => ErrorCategory::NotFound,

            // Conflict → Conflict
            Self::ChannelExists(_) => ErrorCategory::Conflict,

            // Permission → Permission
            Self::PermissionDenied(_) => ErrorCategory::Permission,

            // ResourceBusy → ResourceBusy (sync errors as busy)
            Self::SyncError(_) => ErrorCategory::ResourceBusy,

            // ResourceExhausted → ResourceExhausted
            Self::ResourceExhausted(_) => ErrorCategory::ResourceExhausted,

            // Internal → Internal
            Self::InternalError(_)
            | Self::IoError(_)
            | Self::SerializationError(_)
            | Self::DataConversionError(_)
            | Self::ParsingError(_)
            | Self::ChannelError(_)
            | Self::PointTableError(_)
            | Self::ResourceError(_)
            | Self::BatchOperationFailed(_)
            | Self::StateError(_)
            | Self::LockError(_)
            | Self::ApiError(_) => ErrorCategory::Internal,

            // Unknown → Unknown
            Self::UnknownError(_) => ErrorCategory::Unknown,
        }
    }
}

// ============================================================================
// API Adaptation: ComSrvError → AppError conversion
// ============================================================================

/// Automatically convert ComSrvError to AppError using VoltageErrorTrait for HTTP status mapping
impl From<ComSrvError> for voltage_config::api::AppError {
    fn from(err: ComSrvError) -> Self {
        use voltage_config::{
            api::{AppError, ErrorInfo},
            error::VoltageErrorTrait,
        };

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
