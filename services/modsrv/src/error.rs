//! Error handling for Model Service
//!
//! This module provides error type definitions and conversions for the Model Service,
//! adapting voltage-common error types to maintain backward compatibility.

use thiserror::Error;
use voltage_common::Error as CommonError;

/// Model Service Error Type
#[derive(Error, Debug, Clone)]
pub enum ModelSrvError {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Data format and parsing errors
    #[error("Format error: {0}")]
    FormatError(String),

    /// Invalid command or operation errors
    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    /// Redis database operation errors
    #[error("Redis error: {0}")]
    RedisError(String),

    /// Permission and authorization errors
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Input/Output operation errors
    #[error("IO error: {0}")]
    IoError(String),

    /// Resource lock acquisition errors
    #[error("Lock error: Could not acquire resource lock")]
    LockError,

    /// Serialization/Deserialization errors
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Resource not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Model execution errors
    #[error("Model error: {0}")]
    ModelError(String),

    /// Template processing errors
    #[error("Template error: {0}")]
    TemplateError(String),

    /// Rule processing errors
    #[error("Rule error: {0}")]
    RuleError(String),

    /// Action execution errors
    #[error("Action error: {0}")]
    ActionError(String),

    /// DAG (Directed Acyclic Graph) errors
    #[error("DAG error: {0}")]
    DagError(String),

    /// Invalid parameter errors
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Invalid data errors
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Internal service errors
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Unknown or unclassified errors
    #[error("Unknown error: {0}")]
    UnknownError(String),

    /// Invalid operation errors
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// JSON processing errors
    #[error("JSON error: {0}")]
    JsonError(String),

    /// YAML processing errors
    #[error("YAML error: {0}")]
    YamlError(String),

    /// Data mapping errors
    #[error("Data mapping error: {0}")]
    DataMappingError(String),

    /// Template not found errors
    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    /// Template already exists errors
    #[error("Template already exists: {0}")]
    TemplateAlreadyExists(String),

    /// Model not found errors
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Model already exists errors
    #[error("Model already exists: {0}")]
    ModelAlreadyExists(String),

    /// Rule not found errors
    #[error("Rule not found: {0}")]
    RuleNotFound(String),

    /// Rule disabled errors
    #[error("Rule disabled: {0}")]
    RuleDisabled(String),

    /// Rule parsing errors
    #[error("Rule parsing error: {0}")]
    RuleParsingError(String),

    /// Validation errors
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Action type not supported errors
    #[error("Action type not supported: {0}")]
    ActionTypeNotSupported(String),

    /// Action execution errors
    #[error("Action execution error: {0}")]
    ActionExecutionError(String),

    /// Command not found errors
    #[error("Command not found: {0}")]
    CommandNotFound(String),

    /// Timeout error
    #[error("Timeout error: {0}")]
    TimeoutError(String),

    /// Invalid value error
    #[error("Invalid value: {0}")]
    InvalidValue(String),

    /// Instance not found error
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    /// Not supported error
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// Invalid model error
    #[error("Invalid model: {0}")]
    InvalidModel(String),
}

/// Result type alias for Model Service
pub type Result<T> = std::result::Result<T, ModelSrvError>;

// Conversion from voltage_common::Error to ModelSrvError
impl From<CommonError> for ModelSrvError {
    fn from(err: CommonError) -> Self {
        match err {
            CommonError::Config(msg) => ModelSrvError::ConfigError(msg),
            CommonError::Io(e) => ModelSrvError::IoError(e.to_string()),
            CommonError::Serialization(msg) => ModelSrvError::SerializationError(msg),
            CommonError::Storage(msg) => ModelSrvError::RedisError(msg),
            CommonError::InvalidInput(msg) => ModelSrvError::InvalidParameter(msg),
            CommonError::Auth(msg) => ModelSrvError::PermissionDenied(msg),
            CommonError::Other { message, .. } => ModelSrvError::InternalError(message),
            _ => ModelSrvError::InternalError(err.to_string()),
        }
    }
}

// Conversion from std::io::Error
impl From<std::io::Error> for ModelSrvError {
    fn from(err: std::io::Error) -> Self {
        ModelSrvError::IoError(err.to_string())
    }
}

// Conversion from serde_json::Error
impl From<serde_json::Error> for ModelSrvError {
    fn from(err: serde_json::Error) -> Self {
        ModelSrvError::SerializationError(format!("JSON error: {}", err))
    }
}

// Conversion from serde_yaml::Error
impl From<serde_yaml::Error> for ModelSrvError {
    fn from(err: serde_yaml::Error) -> Self {
        ModelSrvError::SerializationError(format!("YAML error: {}", err))
    }
}

// Conversion from anyhow::Error
impl From<anyhow::Error> for ModelSrvError {
    fn from(err: anyhow::Error) -> Self {
        ModelSrvError::InternalError(err.to_string())
    }
}

// Helper methods for creating errors
impl ModelSrvError {
    pub fn config(msg: impl Into<String>) -> Self {
        ModelSrvError::ConfigError(msg.into())
    }

    pub fn format(msg: impl Into<String>) -> Self {
        ModelSrvError::FormatError(msg.into())
    }

    pub fn invalid_command(msg: impl Into<String>) -> Self {
        ModelSrvError::InvalidCommand(msg.into())
    }

    pub fn redis(msg: impl Into<String>) -> Self {
        ModelSrvError::RedisError(msg.into())
    }

    pub fn permission_denied(msg: impl Into<String>) -> Self {
        ModelSrvError::PermissionDenied(msg.into())
    }

    pub fn io(msg: impl Into<String>) -> Self {
        ModelSrvError::IoError(msg.into())
    }

    pub fn lock() -> Self {
        ModelSrvError::LockError
    }

    pub fn serialization(msg: impl Into<String>) -> Self {
        ModelSrvError::SerializationError(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        ModelSrvError::NotFound(msg.into())
    }

    pub fn model(msg: impl Into<String>) -> Self {
        ModelSrvError::ModelError(msg.into())
    }

    pub fn template(msg: impl Into<String>) -> Self {
        ModelSrvError::TemplateError(msg.into())
    }

    pub fn rule(msg: impl Into<String>) -> Self {
        ModelSrvError::RuleError(msg.into())
    }

    pub fn action(msg: impl Into<String>) -> Self {
        ModelSrvError::ActionError(msg.into())
    }

    pub fn dag(msg: impl Into<String>) -> Self {
        ModelSrvError::DagError(msg.into())
    }

    pub fn invalid_parameter(msg: impl Into<String>) -> Self {
        ModelSrvError::InvalidParameter(msg.into())
    }

    pub fn invalid_data(msg: impl Into<String>) -> Self {
        ModelSrvError::InvalidData(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        ModelSrvError::InternalError(msg.into())
    }

    pub fn unknown(msg: impl Into<String>) -> Self {
        ModelSrvError::UnknownError(msg.into())
    }

    pub fn invalid_value(msg: impl Into<String>) -> Self {
        ModelSrvError::InvalidValue(msg.into())
    }

    pub fn instance_not_found(msg: impl Into<String>) -> Self {
        ModelSrvError::InstanceNotFound(msg.into())
    }

    pub fn not_supported(msg: impl Into<String>) -> Self {
        ModelSrvError::NotSupported(msg.into())
    }

    pub fn invalid_model(msg: impl Into<String>) -> Self {
        ModelSrvError::InvalidModel(msg.into())
    }
}
