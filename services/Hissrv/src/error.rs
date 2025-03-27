use thiserror::Error;

#[derive(Error, Debug)]
pub enum HisSrvError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("InfluxDB error: {0}")]
    InfluxDBError(#[from] influxdb::Error),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),
}

pub type Result<T> = std::result::Result<T, HisSrvError>; 