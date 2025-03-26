use std::io;
use thiserror::Error;
use std::fmt::{self, Display, Formatter};

/// ComSrvError represents all possible errors that can occur in the communication service
#[derive(Error, Debug)]
pub enum ComSrvError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// IO errors
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    /// Protocol errors
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Connection errors
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Timeout errors
    #[error("Timeout error: {0}")]
    TimeoutError(String),

    /// Modbus specific errors
    #[error("Modbus error: {0}")]
    ModbusError(String),

    /// Redis errors
    #[error("Redis error: {0}")]
    RedisError(String),

    /// MQTT errors
    #[error("MQTT error: {0}")]
    MqttError(String),

    /// Channel errors
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Parsing errors
    #[error("Parsing error: {0}")]
    ParsingError(String),

    /// Invalid parameter errors
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Permission errors
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Unknown errors
    #[error("Unknown error: {0}")]
    Unknown(String),

    /// Communication errors
    #[error("Communication error: {0}")]
    CommunicationError(String),

    /// Protocol not supported errors
    #[error("Protocol not supported: {0}")]
    ProtocolNotSupported(String),

    /// Channel not found errors
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    /// Point table errors
    #[error("Point table error: {0}")]
    PointTableError(String),

    /// Point not found errors
    #[error("Point not found: {0}")]
    PointNotFound(String),

    /// Invalid operation errors
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// API errors
    #[error("API error: {0}")]
    ApiError(String),

    /// Internal errors
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Extension trait for mapping errors to ComSrvError
pub trait ErrorExt<T> {
    /// Maps any error to a ComSrvError with a custom message
    fn context<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ErrorExt<T> for std::result::Result<T, E> {
    fn context<C>(self, context: C) -> Result<T>
    where
        C: AsRef<str>,
    {
        self.map_err(|e| {
            // 直接将错误信息转换为带有上下文的Unknown错误
            ComSrvError::Unknown(format!("{}: {}", context.as_ref(), e))
        })
    }
}

/// Convert from serde_yaml error to ComSrvError
impl From<serde_yaml::Error> for ComSrvError {
    fn from(err: serde_yaml::Error) -> Self {
        ComSrvError::SerializationError(err.to_string())
    }
}

/// Convert from redis error to ComSrvError
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

/// Convert from warp error to ComSrvError
impl From<warp::Error> for ComSrvError {
    fn from(err: warp::Error) -> Self {
        ComSrvError::ApiError(format!("Warp error: {}", err))
    }
}

/// Convert from warp rejection to ComSrvError
impl From<warp::reject::Rejection> for ComSrvError {
    fn from(err: warp::reject::Rejection) -> Self {
        ComSrvError::ApiError(format!("API rejection: {:?}", err))
    }
}

/// Convert from address parse error to ComSrvError
impl From<std::net::AddrParseError> for ComSrvError {
    fn from(err: std::net::AddrParseError) -> Self {
        ComSrvError::ConfigError(format!("Address parse error: {}", err))
    }
}

/// Shorthand for Result with ComSrvError
pub type Result<T> = std::result::Result<T, ComSrvError>;

/// Error response structure
#[derive(Debug)]
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
            ComSrvError::SerializationError(_) => ErrorResponse::new("serialization_error", &err.to_string()),
            ComSrvError::CommunicationError(_) => ErrorResponse::new("communication_error", &err.to_string()),
            ComSrvError::ProtocolError(_) => ErrorResponse::new("protocol_error", &err.to_string()),
            ComSrvError::ProtocolNotSupported(_) => ErrorResponse::new("protocol_not_supported", &err.to_string()),
            ComSrvError::ChannelError(_) => ErrorResponse::new("channel_error", &err.to_string()),
            ComSrvError::ChannelNotFound(_) => ErrorResponse::new("channel_not_found", &err.to_string()),
            ComSrvError::PointTableError(_) => ErrorResponse::new("point_table_error", &err.to_string()),
            ComSrvError::PointNotFound(_) => ErrorResponse::new("point_not_found", &err.to_string()),
            ComSrvError::InvalidOperation(_) => ErrorResponse::new("invalid_operation", &err.to_string()),
            ComSrvError::ConnectionError(_) => ErrorResponse::new("connection_error", &err.to_string()),
            ComSrvError::RedisError(_) => ErrorResponse::new("redis_error", &err.to_string()),
            ComSrvError::ApiError(_) => ErrorResponse::new("api_error", &err.to_string()),
            ComSrvError::InternalError(_) => ErrorResponse::new("internal_error", &err.to_string()),
            _ => ErrorResponse::new("unknown_error", &err.to_string()),
        }
    }
} 