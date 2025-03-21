use serde_json;
use serde_yaml;
use redis;
use std::error::Error;
use std::fmt;
use warp::reject;

pub type Result<T> = std::result::Result<T, ModelSrvError>;

/// Model service error types
#[derive(Debug)]
pub enum ModelSrvError {
    /// Redis related errors
    RedisError(String),
    /// IO errors
    IoError(std::io::Error),
    /// Serialization errors
    SerdeError(serde_json::Error),
    /// JSON parsing errors
    JsonError(serde_json::Error),
    /// YAML parsing errors
    YamlError(serde_yaml::Error),
    /// Mutex/RwLock errors
    LockError,
    /// Key not found
    KeyNotFound(String),
    /// Invalid operation
    InvalidOperation(String),
    /// Invalid input data
    InvalidInput(String),
    /// Invalid data format or structure
    InvalidData(String),
    /// Data validation errors
    ValidationError(String),
    /// Model not found
    ModelNotFound(String),
    /// Model already exists
    ModelAlreadyExists(String),
    /// Rule not found
    RuleNotFound(String),
    /// Rule disabled
    RuleDisabled(String),
    /// Rule already exists
    AlreadyExists(String),
    /// Not found error
    NotFound(String),
    /// Action not found
    ActionNotFound(String),
    /// Execution error
    ExecutionError(String),
    /// Data mapping error
    DataMappingError(String),
    /// Configuration error
    ConfigError(String),
    /// Model error
    ModelError(String),
    /// Template error
    TemplateError(String),
    /// Rule execution error
    RuleError(String),
    /// Template not found
    TemplateNotFound(String),
    /// Template already exists
    TemplateAlreadyExists(String),
}

impl fmt::Display for ModelSrvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelSrvError::RedisError(e) => write!(f, "Redis error: {}", e),
            ModelSrvError::IoError(e) => write!(f, "IO error: {}", e),
            ModelSrvError::SerdeError(e) => write!(f, "Serialization error: {}", e),
            ModelSrvError::JsonError(e) => write!(f, "JSON error: {}", e),
            ModelSrvError::YamlError(e) => write!(f, "YAML error: {}", e),
            ModelSrvError::LockError => write!(f, "Lock error"),
            ModelSrvError::KeyNotFound(key) => write!(f, "Key not found: {}", key),
            ModelSrvError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
            ModelSrvError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            ModelSrvError::InvalidData(msg) => write!(f, "Invalid data format or structure: {}", msg),
            ModelSrvError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ModelSrvError::ModelNotFound(id) => write!(f, "Model not found: {}", id),
            ModelSrvError::ModelAlreadyExists(id) => write!(f, "Model already exists: {}", id),
            ModelSrvError::RuleNotFound(id) => write!(f, "Rule not found: {}", id),
            ModelSrvError::RuleDisabled(id) => write!(f, "Rule is disabled: {}", id),
            ModelSrvError::AlreadyExists(msg) => write!(f, "Already exists: {}", msg),
            ModelSrvError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ModelSrvError::ActionNotFound(id) => write!(f, "Action not found: {}", id),
            ModelSrvError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            ModelSrvError::DataMappingError(msg) => write!(f, "Data mapping error: {}", msg),
            ModelSrvError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            ModelSrvError::ModelError(msg) => write!(f, "Model error: {}", msg),
            ModelSrvError::TemplateError(msg) => write!(f, "Template error: {}", msg),
            ModelSrvError::RuleError(msg) => write!(f, "Rule error: {}", msg),
            ModelSrvError::TemplateNotFound(id) => write!(f, "Template not found: {}", id),
            ModelSrvError::TemplateAlreadyExists(id) => write!(f, "Template already exists: {}", id),
        }
    }
}

impl Error for ModelSrvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ModelSrvError::IoError(e) => Some(e),
            ModelSrvError::SerdeError(e) => Some(e),
            ModelSrvError::JsonError(e) => Some(e),
            ModelSrvError::YamlError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<redis::RedisError> for ModelSrvError {
    fn from(err: redis::RedisError) -> Self {
        ModelSrvError::RedisError(err.to_string())
    }
}

impl From<std::io::Error> for ModelSrvError {
    fn from(err: std::io::Error) -> Self {
        ModelSrvError::IoError(err)
    }
}

impl From<serde_json::Error> for ModelSrvError {
    fn from(err: serde_json::Error) -> Self {
        ModelSrvError::SerdeError(err)
    }
}

impl From<config::ConfigError> for ModelSrvError {
    fn from(err: config::ConfigError) -> Self {
        ModelSrvError::ConfigError(err.to_string())
    }
}

impl From<serde_yaml::Error> for ModelSrvError {
    fn from(err: serde_yaml::Error) -> Self {
        ModelSrvError::YamlError(err)
    }
}

impl reject::Reject for ModelSrvError {} 