use thiserror::Error;

/// Basic library error type (基础library error type)
#[derive(Debug, Error)]
pub enum Error {
    /// Redis error
    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    Redis(String),

    /// `InfluxDB` error
    #[cfg(feature = "influxdb")]
    #[error("InfluxDB error: {0}")]
    InfluxDB(String),

    /// HTTP error
    #[cfg(feature = "influxdb")]
    #[error("HTTP error: {0}")]
    Http(String),

    /// Configurationerror
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// serializingerror
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Parseerror
    #[error("Parse error: {0}")]
    Parse(String),

    /// timeouterror
    #[error("Operation timeout: {0}")]
    Timeout(String),

    /// Generic error (通用error)
    #[error("{0}")]
    Generic(String),
}

/// Errorresulttype
pub type Result<T> = std::result::Result<T, Error>;

// Redis errorconverting
#[cfg(feature = "redis")]
impl From<redis::RedisError> for Error {
    fn from(err: redis::RedisError) -> Self {
        Error::Redis(err.to_string())
    }
}

// serializingerrorconverting
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}
