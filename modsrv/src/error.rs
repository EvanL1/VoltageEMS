use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelSrvError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Model error: {0}")]
    ModelError(String),

    #[error("Data mapping error: {0}")]
    DataMappingError(String),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

pub type Result<T> = std::result::Result<T, ModelSrvError>; 