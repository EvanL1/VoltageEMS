use serde_json;
use serde_yaml;
use redis;
use std::error::Error;
use std::fmt;
use warp::reject;

pub type Result<T> = std::result::Result<T, ModelSrvError>;

/// Model service error types
#[derive(Debug, Clone)]
pub enum ModelSrvError {
    ConfigError(String),
    FormatError(String),
    InvalidCommand(String),
    RedisError(String),
    PermissionDenied(String),
    IoError(String),
    LockError,
    SerdeError(String),
    ParseError(String),
    JsonError(String),
    InvalidData(String),
    YamlError(String),
    TemplateError(String),
    TemplateNotFound(String),
    ModelError(String),
    RuleNotFound(String),
    RuleExecutionError(String),
    InvalidRuleDefinition(String),
    ActionHandlerNotFound(String),
    InvalidActionConfig(String),
    RuleParsingError(String),
    RuleDisabled(String),
    ActionTypeNotSupported(String),
    ActionExecutionError(String),
    TemplateAlreadyExists(String),
    RuleError(String),
    InvalidAuth(String),
    ComSrvError(String),
    ModelNotFound(String),
    ModelAlreadyExists(String),
    ValidationError(String),
    DataMappingError(String),
    KeyNotFound(String),
    InvalidOperation(String),
    InvalidInput(String),
    Unauthorized(String),
    NotImplemented(String),
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
            ModelSrvError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            ModelSrvError::FormatError(msg) => write!(f, "Format error: {}", msg),
            ModelSrvError::InvalidCommand(msg) => write!(f, "Invalid command: {}", msg),
            ModelSrvError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            ModelSrvError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ModelSrvError::TemplateError(msg) => write!(f, "Template error: {}", msg),
            ModelSrvError::TemplateNotFound(id) => write!(f, "Template not found: {}", id),
            ModelSrvError::ModelError(msg) => write!(f, "Model error: {}", msg),
            ModelSrvError::RuleNotFound(id) => write!(f, "Rule not found: {}", id),
            ModelSrvError::RuleExecutionError(msg) => write!(f, "Rule execution error: {}", msg),
            ModelSrvError::InvalidRuleDefinition(msg) => write!(f, "Invalid rule definition: {}", msg),
            ModelSrvError::ActionHandlerNotFound(id) => write!(f, "Action handler not found: {}", id),
            ModelSrvError::InvalidActionConfig(msg) => write!(f, "Invalid action config: {}", msg),
            ModelSrvError::RuleParsingError(msg) => write!(f, "Rule parsing error: {}", msg),
            ModelSrvError::RuleDisabled(id) => write!(f, "Rule is disabled: {}", id),
            ModelSrvError::ActionTypeNotSupported(msg) => write!(f, "Action type not supported: {}", msg),
            ModelSrvError::ActionExecutionError(msg) => write!(f, "Action execution error: {}", msg),
            ModelSrvError::TemplateAlreadyExists(id) => write!(f, "Template already exists: {}", id),
            ModelSrvError::RuleError(msg) => write!(f, "Rule error: {}", msg),
            ModelSrvError::InvalidAuth(msg) => write!(f, "Authentication error: {}", msg),
            ModelSrvError::ComSrvError(msg) => write!(f, "Communication service error: {}", msg),
            ModelSrvError::ModelNotFound(id) => write!(f, "Model not found: {}", id),
            ModelSrvError::ModelAlreadyExists(id) => write!(f, "Model already exists: {}", id),
            ModelSrvError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ModelSrvError::DataMappingError(msg) => write!(f, "Data mapping error: {}", msg),
            ModelSrvError::KeyNotFound(key) => write!(f, "Key not found: {}", key),
            ModelSrvError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
            ModelSrvError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            ModelSrvError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            ModelSrvError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
            ModelSrvError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
        }
    }
}

impl Error for ModelSrvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl From<redis::RedisError> for ModelSrvError {
    fn from(err: redis::RedisError) -> Self {
        ModelSrvError::RedisError(err.to_string())
    }
}

impl From<std::io::Error> for ModelSrvError {
    fn from(err: std::io::Error) -> Self {
        ModelSrvError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for ModelSrvError {
    fn from(err: serde_json::Error) -> Self {
        ModelSrvError::SerdeError(err.to_string())
    }
}

impl From<config::ConfigError> for ModelSrvError {
    fn from(err: config::ConfigError) -> Self {
        ModelSrvError::ConfigError(err.to_string())
    }
}

impl From<serde_yaml::Error> for ModelSrvError {
    fn from(err: serde_yaml::Error) -> Self {
        ModelSrvError::YamlError(err.to_string())
    }
}

impl reject::Reject for ModelSrvError {}

impl From<tokio::task::JoinError> for ModelSrvError {
    fn from(e: tokio::task::JoinError) -> Self {
        ModelSrvError::IoError(e.to_string())
    }
}

