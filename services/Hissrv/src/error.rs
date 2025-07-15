use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HisSrvError {
    // Redis相关错误
    #[error("Redis error: {message}")]
    RedisError {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Redis connection error: {message}")]
    RedisConnectionError {
        message: String,
        retry_after: Option<u64>, // 秒
    },

    // InfluxDB相关错误
    #[error("InfluxDB error: {0}")]
    InfluxDBError(#[from] influxdb::Error),

    #[error("InfluxDB write error: {message}, batch_size: {batch_size}")]
    InfluxDBWriteError {
        message: String,
        batch_size: usize,
        failed_points: Option<Vec<String>>,
    },

    #[error("InfluxDB query error: {message}")]
    InfluxDBQueryError {
        message: String,
        query: String,
    },

    // IO相关错误
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    // 解析错误
    #[error("Parse error: {message} at {location}")]
    ParseError {
        message: String,
        location: String,
        raw_data: Option<String>,
    },

    #[error("Invalid data format: {message}")]
    InvalidFormat {
        message: String,
        expected: String,
        actual: String,
    },

    // 配置错误
    #[error("Configuration error: {message}")]
    ConfigError {
        message: String,
        field: Option<String>,
        suggestion: Option<String>,
    },

    #[error("Missing configuration: {field}")]
    MissingConfig { field: String },

    // 连接错误
    #[error("Connection error: {message}")]
    ConnectionError {
        message: String,
        endpoint: String,
        retry_count: u32,
    },

    #[error("Connection timeout: endpoint={endpoint}, timeout={timeout_secs}s")]
    ConnectionTimeout {
        endpoint: String,
        timeout_secs: u64,
    },

    // 序列化错误
    #[error("Serialization error: {message}")]
    SerializationError {
        message: String,
        data_type: String,
    },

    #[error("Deserialization error: {message}")]
    DeserializationError {
        message: String,
        data_type: String,
        raw_data: Option<String>,
    },

    // 存储错误
    #[error("Storage error: {backend} - {message}")]
    StorageError {
        backend: String,
        message: String,
        operation: String,
    },

    #[error("Storage backend not available: {backend}")]
    StorageBackendUnavailable {
        backend: String,
        reason: String,
    },

    // 查找错误
    #[error("Not found: {resource_type} - {identifier}")]
    NotFound {
        resource_type: String,
        identifier: String,
    },

    // 写入错误
    #[error("Write error: {message}, points_affected: {points_affected}")]
    WriteError {
        message: String,
        points_affected: usize,
        partial_success: bool,
    },

    // 批处理错误
    #[error("Batch processing error: {message}, failed: {failed_count}/{total_count}")]
    BatchError {
        message: String,
        failed_count: usize,
        total_count: usize,
        failed_items: Option<Vec<String>>,
    },

    // API错误
    #[error("API error: {status_code} - {message}")]
    ApiError {
        status_code: u16,
        message: String,
        request_id: Option<String>,
    },

    // 认证错误
    #[error("Authentication error: {message}")]
    AuthError {
        message: String,
        realm: Option<String>,
    },

    // 权限错误
    #[error("Permission denied: {resource} - {action}")]
    PermissionError { resource: String, action: String },

    // 限流错误
    #[error("Rate limit exceeded: {limit} requests per {window_secs}s")]
    RateLimitError {
        limit: u32,
        window_secs: u64,
        retry_after: Option<u64>,
    },

    // 验证错误
    #[error("Validation error: {field} - {message}")]
    ValidationError {
        field: String,
        message: String,
        value: Option<String>,
    },

    // 超时错误
    #[error("Operation timeout: {operation} exceeded {timeout_secs}s")]
    TimeoutError {
        operation: String,
        timeout_secs: u64,
    },

    // 内部错误
    #[error("Internal error: {message}")]
    InternalError {
        message: String,
        context: Option<String>,
    },

    // 通用错误（用于包装其他错误）
    #[error("{message}")]
    Other {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl HisSrvError {
    /// 创建一个带有源错误的Redis错误
    pub fn redis_error<E>(message: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::RedisError {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// 创建一个存储错误
    pub fn storage_error(
        backend: impl Into<String>,
        operation: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::StorageError {
            backend: backend.into(),
            message: message.into(),
            operation: operation.into(),
        }
    }

    /// 检查错误是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RedisConnectionError { .. }
                | Self::ConnectionError { .. }
                | Self::ConnectionTimeout { .. }
                | Self::StorageBackendUnavailable { .. }
                | Self::RateLimitError { .. }
                | Self::TimeoutError { .. }
        )
    }

    /// 获取建议的重试延迟（秒）
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            Self::RedisConnectionError { retry_after, .. } => *retry_after,
            Self::RateLimitError { retry_after, .. } => *retry_after,
            Self::ConnectionError { retry_count, .. } => {
                // 指数退避: 2^retry_count秒，最多64秒
                Some((2_u64.pow(*retry_count)).min(64))
            }
            _ => None,
        }
    }

    /// 获取错误恢复建议
    pub fn recovery_suggestion(&self) -> Option<String> {
        match self {
            Self::ConfigError { suggestion, .. } => suggestion.clone(),
            Self::MissingConfig { field } => {
                Some(format!("请在配置文件中设置 {} 字段", field))
            }
            Self::ConnectionTimeout { endpoint, .. } => {
                Some(format!("检查 {} 的网络连接和防火墙设置", endpoint))
            }
            Self::StorageBackendUnavailable { backend, reason } => {
                Some(format!("确保 {} 服务正在运行: {}", backend, reason))
            }
            Self::AuthError { .. } => Some("检查认证凭据是否正确".to_string()),
            Self::PermissionError { resource, action } => {
                Some(format!("确保当前用户有权限对 {} 执行 {} 操作", resource, action))
            }
            _ => None,
        }
    }

    /// 获取错误上下文信息
    pub fn context(&self) -> ErrorContext {
        ErrorContext {
            error_type: self.error_type(),
            is_retryable: self.is_retryable(),
            retry_after: self.retry_after(),
            suggestion: self.recovery_suggestion(),
            severity: self.severity(),
        }
    }

    /// 获取错误类型
    fn error_type(&self) -> &'static str {
        match self {
            Self::RedisError { .. } | Self::RedisConnectionError { .. } => "redis",
            Self::InfluxDBError(_) | Self::InfluxDBWriteError { .. } | Self::InfluxDBQueryError { .. } => "influxdb",
            Self::IOError(_) => "io",
            Self::ParseError { .. } | Self::InvalidFormat { .. } => "parse",
            Self::ConfigError { .. } | Self::MissingConfig { .. } => "config",
            Self::ConnectionError { .. } | Self::ConnectionTimeout { .. } => "connection",
            Self::SerializationError { .. } | Self::DeserializationError { .. } => "serialization",
            Self::StorageError { .. } | Self::StorageBackendUnavailable { .. } => "storage",
            Self::NotFound { .. } => "not_found",
            Self::WriteError { .. } | Self::BatchError { .. } => "write",
            Self::ApiError { .. } => "api",
            Self::AuthError { .. } | Self::PermissionError { .. } => "auth",
            Self::RateLimitError { .. } => "rate_limit",
            Self::ValidationError { .. } => "validation",
            Self::TimeoutError { .. } => "timeout",
            Self::InternalError { .. } | Self::Other { .. } => "internal",
        }
    }

    /// 获取错误严重程度
    fn severity(&self) -> ErrorSeverity {
        match self {
            Self::NotFound { .. } | Self::ValidationError { .. } => ErrorSeverity::Info,
            Self::RateLimitError { .. } | Self::AuthError { .. } | Self::PermissionError { .. } => {
                ErrorSeverity::Warning
            }
            Self::ConnectionError { .. }
            | Self::ConnectionTimeout { .. }
            | Self::StorageBackendUnavailable { .. }
            | Self::WriteError { .. }
            | Self::BatchError { .. } => ErrorSeverity::Error,
            Self::InternalError { .. } | Self::ConfigError { .. } | Self::MissingConfig { .. } => {
                ErrorSeverity::Critical
            }
            _ => ErrorSeverity::Error,
        }
    }
}

/// 错误上下文信息
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub error_type: &'static str,
    pub is_retryable: bool,
    pub retry_after: Option<u64>,
    pub suggestion: Option<String>,
    pub severity: ErrorSeverity,
}

/// 错误严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Error => write!(f, "ERROR"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

pub type Result<T> = std::result::Result<T, HisSrvError>;

// 为常见的外部错误类型实现转换
impl From<redis::RedisError> for HisSrvError {
    fn from(err: redis::RedisError) -> Self {
        Self::redis_error("Redis operation failed", err)
    }
}

impl From<serde_json::Error> for HisSrvError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError {
            message: err.to_string(),
            data_type: "JSON".to_string(),
        }
    }
}

impl From<figment::Error> for HisSrvError {
    fn from(err: figment::Error) -> Self {
        Self::ConfigError {
            message: err.to_string(),
            field: None,
            suggestion: Some("检查配置文件格式和环境变量设置".to_string()),
        }
    }
}
