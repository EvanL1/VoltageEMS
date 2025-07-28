use thiserror::Error;

/// 基础库错误类型
#[derive(Debug, Error)]
pub enum Error {
    /// Redis 错误
    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    Redis(String),

    /// `InfluxDB` 错误
    #[cfg(feature = "influxdb")]
    #[error("InfluxDB error: {0}")]
    InfluxDB(String),

    /// HTTP 错误
    #[cfg(feature = "influxdb")]
    #[error("HTTP error: {0}")]
    Http(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// 解析错误
    #[error("Parse error: {0}")]
    Parse(String),

    /// 超时错误
    #[error("Operation timeout: {0}")]
    Timeout(String),

    /// 通用错误
    #[error("{0}")]
    Generic(String),
}

/// 错误结果类型
pub type Result<T> = std::result::Result<T, Error>;

// Redis 错误转换
#[cfg(feature = "redis")]
impl From<redis::RedisError> for Error {
    fn from(err: redis::RedisError) -> Self {
        Error::Redis(err.to_string())
    }
}

// 序列化错误转换
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}
