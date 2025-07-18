//! Common error types for `VoltageEMS` services

use thiserror::Error;

/// Common error type used across `VoltageEMS` services
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration related errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/Deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Database/Storage errors
    #[error("Storage error: {0}")]
    Storage(String),

    /// Network/Communication errors
    #[error("Network error: {0}")]
    Network(String),

    /// Protocol errors
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Redis errors
    #[error("Redis error: {0}")]
    Redis(String),

    /// Invalid input/parameter errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Operation timeout
    #[error("Operation timeout: {0}")]
    Timeout(String),

    /// Service unavailable
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Authentication/Authorization errors
    #[error("Auth error: {0}")]
    Auth(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Generic error with context
    #[error("{message}")]
    Other {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// Result type alias using our Error type
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Error::Config(msg.into())
    }

    /// Create a storage error
    pub fn storage(msg: impl Into<String>) -> Self {
        Error::Storage(msg.into())
    }

    /// Create a network error
    pub fn network(msg: impl Into<String>) -> Self {
        Error::Network(msg.into())
    }

    /// Create a protocol error
    pub fn protocol(msg: impl Into<String>) -> Self {
        Error::Protocol(msg.into())
    }

    /// Create an invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Error::InvalidInput(msg.into())
    }

    /// Create a timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        Error::Timeout(msg.into())
    }

    /// Create an other error with optional source
    pub fn other(message: impl Into<String>) -> Self {
        Error::Other {
            message: message.into(),
            source: None,
        }
    }

    /// Create an other error with source
    pub fn other_with_source(
        message: impl Into<String>,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Error::Other {
            message: message.into(),
            source: Some(source),
        }
    }
}

// Implement From for common error types
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<figment::Error> for Error {
    fn from(err: figment::Error) -> Self {
        Error::Config(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = Error::config("invalid configuration");
        assert_eq!(
            err.to_string(),
            "Configuration error: invalid configuration"
        );

        let err = Error::storage("database connection failed");
        assert_eq!(err.to_string(), "Storage error: database connection failed");
    }

    #[test]
    fn test_error_conversion() {
        let json_err = serde_json::from_str::<String>("invalid json");
        assert!(json_err.is_err());
        let err: Error = json_err.unwrap_err().into();
        assert!(matches!(err, Error::Serialization(_)));
    }
}
