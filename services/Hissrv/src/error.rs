use thiserror::Error;

/// HisSrv 错误类型
#[derive(Error, Debug)]
pub enum HisSrvError {
    #[error("配置错误: {message}")]
    Config { message: String },

    #[error("Redis 连接错误: {message}")]
    Redis { message: String },

    #[error("InfluxDB 错误: {message}")]
    InfluxDB { message: String },

    #[error("数据处理错误: {message}")]
    DataProcessing { message: String },

    #[error("HTTP 错误: {0}")]
    Http(#[from] reqwest::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("时间解析错误: {0}")]
    DateTime(#[from] chrono::ParseError),

    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("内部错误: {0}")]
    Internal(#[from] anyhow::Error),
}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, HisSrvError>;

impl HisSrvError {
    /// 创建配置错误
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// 创建 Redis 错误
    pub fn redis(message: impl Into<String>) -> Self {
        Self::Redis {
            message: message.into(),
        }
    }

    /// 创建 InfluxDB 错误
    pub fn influxdb(message: impl Into<String>) -> Self {
        Self::InfluxDB {
            message: message.into(),
        }
    }

    /// 创建数据处理错误
    pub fn data_processing(message: impl Into<String>) -> Self {
        Self::DataProcessing {
            message: message.into(),
        }
    }

    /// 判断是否为可重试的错误
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Redis { .. } | Self::InfluxDB { .. } | Self::Http(_) => true,
            Self::Config { .. }
            | Self::DataProcessing { .. }
            | Self::Serialization(_)
            | Self::DateTime(_)
            | Self::Io(_)
            | Self::Internal(_) => false,
        }
    }
}

/// 转换 voltage-common 的错误
impl From<voltage_common::error::Error> for HisSrvError {
    fn from(err: voltage_common::error::Error) -> Self {
        Self::redis(err.to_string())
    }
}

/// 转换 Figment 配置错误
impl From<figment::Error> for HisSrvError {
    fn from(err: figment::Error) -> Self {
        Self::config(err.to_string())
    }
}