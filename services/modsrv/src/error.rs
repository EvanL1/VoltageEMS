use thiserror::Error;
use std::io;
use serde_json;
use serde_yaml;
use config::ConfigError;
use redis;

pub type Result<T> = std::result::Result<T, ModelSrvError>;

#[derive(Error, Debug)]
pub enum ModelSrvError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
    
    #[error("Redis error: {0}")]
    RedisError(String),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Template error: {0}")]
    TemplateError(String),
    
    #[error("Model error: {0}")]
    ModelError(String),
    
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    
    #[error("Lock error")]
    LockError,
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Sync error: {0}")]
    SyncError(String),
    
    #[error("Data mapping error: {0}")]
    DataMappingError(String),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("Device error: {0}")]
    DeviceError(String),
    
    #[error("Rule error: {0}")]
    RuleError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<ConfigError> for ModelSrvError {
    fn from(err: ConfigError) -> Self {
        ModelSrvError::ConfigError(err.to_string())
    }
}

impl From<redis::RedisError> for ModelSrvError {
    fn from(err: redis::RedisError) -> Self {
        ModelSrvError::RedisError(err.to_string())
    }
} 