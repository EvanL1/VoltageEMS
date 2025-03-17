use std::io;

use thiserror::Error;

/// All error types for the hissrv application
#[derive(Error, Debug)]
pub enum HissrvError {
    /// Redis related errors
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    /// InfluxDB related errors
    #[error("InfluxDB error: {0}")]
    InfluxDB(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Serialization/Deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Data processing errors
    #[error("Data processing error: {0}")]
    DataProcessing(String),

    /// General errors
    #[error("{0}")]
    General(String),
}

/// Custom Result type for hissrv
pub type Result<T> = std::result::Result<T, HissrvError>;

/// Helper functions for creating errors
impl HissrvError {
    /// Create a new configuration error
    pub fn config<T: ToString>(msg: T) -> Self {
        HissrvError::Config(msg.to_string())
    }

    /// Create a new InfluxDB error
    pub fn influxdb<T: ToString>(msg: T) -> Self {
        HissrvError::InfluxDB(msg.to_string())
    }

    /// Create a new serialization error
    pub fn serialization<T: ToString>(msg: T) -> Self {
        HissrvError::Serialization(msg.to_string())
    }

    /// Create a new data processing error
    pub fn data_processing<T: ToString>(msg: T) -> Self {
        HissrvError::DataProcessing(msg.to_string())
    }

    /// Create a new general error
    pub fn general<T: ToString>(msg: T) -> Self {
        HissrvError::General(msg.to_string())
    }
}

impl From<String> for HissrvError {
    fn from(err: String) -> Self {
        HissrvError::General(err)
    }
}

impl From<&str> for HissrvError {
    fn from(err: &str) -> Self {
        HissrvError::General(err.to_string())
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Error> for HissrvError {
    fn from(err: reqwest::Error) -> Self {
        HissrvError::InfluxDB(err.to_string())
    }
}
