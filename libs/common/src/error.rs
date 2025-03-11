use thiserror::Error;

/// Basic library error type
#[derive(Debug, Error)]
pub enum Error {
    /// Redis error
    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    Redis(String),

    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Parse error
    #[error("Parse error: {0}")]
    Parse(String),

    /// Timeout error
    #[error("Operation timeout: {0}")]
    Timeout(String),

    /// Generic error
    #[error("{0}")]
    Generic(String),
}

/// Error result type
pub type Result<T> = std::result::Result<T, Error>;

// Redis error converting
#[cfg(feature = "redis")]
impl From<redis::RedisError> for Error {
    fn from(err: redis::RedisError) -> Self {
        Error::Redis(err.to_string())
    }
}

// Serialization error converting
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}
