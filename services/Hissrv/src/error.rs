//! 错误处理模块

use std::fmt;

/// hissrv 错误类型
#[derive(Debug)]
pub enum HisSrvError {
    /// 配置错误
    Config(String),
    /// Redis 连接错误
    Redis(String),
    /// InfluxDB 写入错误
    InfluxDB(String),
    /// IO 错误
    Io(std::io::Error),
}

impl fmt::Display for HisSrvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HisSrvError::Config(msg) => write!(f, "配置错误: {}", msg),
            HisSrvError::Redis(msg) => write!(f, "Redis错误: {}", msg),
            HisSrvError::InfluxDB(msg) => write!(f, "InfluxDB错误: {}", msg),
            HisSrvError::Io(err) => write!(f, "IO错误: {}", err),
        }
    }
}

impl std::error::Error for HisSrvError {}

impl From<std::io::Error> for HisSrvError {
    fn from(err: std::io::Error) -> Self {
        HisSrvError::Io(err)
    }
}

impl From<figment::Error> for HisSrvError {
    fn from(err: figment::Error) -> Self {
        HisSrvError::Config(err.to_string())
    }
}

impl From<voltage_libs::error::Error> for HisSrvError {
    fn from(err: voltage_libs::error::Error) -> Self {
        match err {
            voltage_libs::error::Error::Redis(msg) => HisSrvError::Redis(msg),
            voltage_libs::error::Error::InfluxDB(msg) => HisSrvError::InfluxDB(msg),
            voltage_libs::error::Error::Http(msg) => HisSrvError::InfluxDB(msg),
            _ => HisSrvError::Config(err.to_string()),
        }
    }
}

impl From<redis::RedisError> for HisSrvError {
    fn from(err: redis::RedisError) -> Self {
        HisSrvError::Redis(err.to_string())
    }
}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, HisSrvError>;
