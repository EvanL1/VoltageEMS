//! Error handling for Communication Service
//!
//! This module provides error type definitions and conversions for the Communication Service,
//! adapting voltage-common error types to maintain backward compatibility.

use thiserror::Error;
use voltage_libs::error::Error as CommonError;

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

    /// IEC 60870 protocol specific errors
    #[error("IEC 60870 error: {0}")]
    Iec60870Error(String),

    /// CAN bus protocol specific errors
    #[cfg(feature = "can")]
    #[error("CAN bus error: {0}")]
    CanError(String),

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

    /// Protocol not supported
    #[error("Protocol not supported: {0}")]
    ProtocolNotSupported(String),

    /// Not implemented error
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Parsing errors
    #[error("Parsing error: {0}")]
    ParsingError(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Communication errors
    #[error("Communication error: {0}")]
    CommunicationError(String),

    /// Network errors
    #[error("Network error: {0}")]
    NetworkError(String),

    /// State errors
    #[error("State error: {0}")]
    StateError(String),

    /// Lock errors
    #[error("Lock error: {0}")]
    LockError(String),

    /// Resource exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Configuration errors (duplicate but for compatibility)
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Unknown errors
    #[error("Unknown error: {0}")]
    UnknownError(String),

    /// API errors
    #[error("API error: {0}")]
    ApiError(String),

    /// Storage errors (for Redis, file storage, etc.)
    #[error("Storage error: {0}")]
    Storage(String),
}

/// Result type alias for Communication Service
pub type Result<T> = std::result::Result<T, ComSrvError>;

// Conversion from voltage_libs::Error to ComSrvError
impl From<CommonError> for ComSrvError {
    fn from(err: CommonError) -> Self {
        match err {
            CommonError::Config(msg) => ComSrvError::ConfigError(msg),
            CommonError::Io(e) => ComSrvError::IoError(e.to_string()),
            CommonError::Serialization(msg) => ComSrvError::SerializationError(msg),
            CommonError::Parse(msg) => ComSrvError::InvalidData(msg),
            CommonError::Timeout(msg) => ComSrvError::TimeoutError(msg),
            CommonError::Generic(msg) => ComSrvError::InternalError(msg),
            CommonError::Redis(msg) => ComSrvError::RedisError(msg),
            CommonError::InfluxDB(msg) => ComSrvError::Storage(msg),
            CommonError::Http(msg) => ComSrvError::ConnectionError(msg),
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

// Conversion from redis::RedisError
impl From<redis::RedisError> for ComSrvError {
    fn from(err: redis::RedisError) -> Self {
        ComSrvError::RedisError(format!("Redis error: {err}"))
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

    pub fn iec60870(msg: impl Into<String>) -> Self {
        ComSrvError::Iec60870Error(msg.into())
    }

    #[cfg(feature = "can")]
    pub fn can(msg: impl Into<String>) -> Self {
        ComSrvError::CanError(msg.into())
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
