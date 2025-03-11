//! RuleSrv Error Types
//!
//! Domain-specific error handling for Rule Service

use thiserror::Error;

/// RuleSrv Result type alias
pub type Result<T> = std::result::Result<T, RuleSrvError>;

/// Rule Service errors with domain-specific semantics
#[derive(Error, Debug, Clone)]
pub enum RuleSrvError {
    // ============================================================================
    // Configuration Errors
    // ============================================================================
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    #[error("Configuration error: {field}: {message}")]
    ConfigurationError { field: String, message: String },

    // ============================================================================
    // Database Errors
    // ============================================================================
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("SQLite error: {0}")]
    SqliteError(String),

    #[error("Redis error: {0}")]
    RedisError(String),

    // ============================================================================
    // Rule Management Errors
    // ============================================================================
    #[error("Rule not found: {0}")]
    RuleNotFound(String),

    #[error("Rule already exists: {0}")]
    RuleExists(String),

    #[error("Rule error: {0}")]
    RuleError(String),

    #[error("Invalid rule: {0}")]
    InvalidRule(String),

    #[error("Rule disabled: {0}")]
    RuleDisabled(String),

    #[error("Rule conflict: {0}")]
    RuleConflict(String),

    // ============================================================================
    // Execution Engine Errors
    // ============================================================================
    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Evaluation error: {0}")]
    EvaluationError(String),

    #[error("Condition error: {0}")]
    ConditionError(String),

    #[error("Action error: {0}")]
    ActionError(String),

    #[error("Expression error: {0}")]
    ExpressionError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    // ============================================================================
    // Scheduler Errors
    // ============================================================================
    #[error("Scheduler error: {0}")]
    SchedulerError(String),

    #[error("Cron error: {0}")]
    CronError(String),

    // ============================================================================
    // Data Operation Errors
    // ============================================================================
    #[error("Data error: {0}")]
    DataError(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    // ============================================================================
    // Internal Errors
    // ============================================================================
    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Unknown error: {0}")]
    UnknownError(String),
}

// ============================================================================
// VoltageErrorTrait Implementation
// ============================================================================

impl voltage_config::error::VoltageErrorTrait for RuleSrvError {
    fn error_code(&self) -> &'static str {
        match self {
            // Configuration
            Self::ConfigError(_) => "RULESRV_CONFIG_ERROR",
            Self::InvalidConfig(_) => "RULESRV_INVALID_CONFIG",
            Self::MissingConfig(_) => "RULESRV_MISSING_CONFIG",
            Self::ConfigurationError { .. } => "RULESRV_CONFIGURATION_ERROR",

            // Database
            Self::DatabaseError(_) => "RULESRV_DATABASE_ERROR",
            Self::SqliteError(_) => "RULESRV_SQLITE_ERROR",
            Self::RedisError(_) => "RULESRV_REDIS_ERROR",

            // Rule Management
            Self::RuleNotFound(_) => "RULESRV_RULE_NOT_FOUND",
            Self::RuleExists(_) => "RULESRV_RULE_EXISTS",
            Self::RuleError(_) => "RULESRV_RULE_ERROR",
            Self::InvalidRule(_) => "RULESRV_INVALID_RULE",
            Self::RuleDisabled(_) => "RULESRV_RULE_DISABLED",
            Self::RuleConflict(_) => "RULESRV_RULE_CONFLICT",

            // Execution Engine
            Self::ExecutionError(_) => "RULESRV_EXECUTION_ERROR",
            Self::EvaluationError(_) => "RULESRV_EVALUATION_ERROR",
            Self::ConditionError(_) => "RULESRV_CONDITION_ERROR",
            Self::ActionError(_) => "RULESRV_ACTION_ERROR",
            Self::ExpressionError(_) => "RULESRV_EXPRESSION_ERROR",
            Self::TimeoutError(_) => "RULESRV_TIMEOUT_ERROR",

            // Scheduler
            Self::SchedulerError(_) => "RULESRV_SCHEDULER_ERROR",
            Self::CronError(_) => "RULESRV_CRON_ERROR",

            // Data
            Self::DataError(_) => "RULESRV_DATA_ERROR",
            Self::InvalidData(_) => "RULESRV_INVALID_DATA",
            Self::SerializationError(_) => "RULESRV_SERIALIZATION_ERROR",

            // Internal
            Self::InternalError(_) => "RULESRV_INTERNAL_ERROR",
            Self::IoError(_) => "RULESRV_IO_ERROR",
            Self::LockError(_) => "RULESRV_LOCK_ERROR",
            Self::UnknownError(_) => "RULESRV_UNKNOWN_ERROR",
        }
    }

    fn category(&self) -> voltage_config::error::ErrorCategory {
        use voltage_config::error::ErrorCategory;

        match self {
            // Configuration → Configuration
            Self::ConfigError(_)
            | Self::InvalidConfig(_)
            | Self::MissingConfig(_)
            | Self::ConfigurationError { .. } => ErrorCategory::Configuration,

            // Database → Database
            Self::DatabaseError(_) | Self::SqliteError(_) | Self::RedisError(_) => {
                ErrorCategory::Database
            },

            // Rule NotFound → NotFound
            Self::RuleNotFound(_) => ErrorCategory::NotFound,

            // Rule Exists/Conflict → Conflict
            Self::RuleExists(_) | Self::RuleConflict(_) => ErrorCategory::Conflict,

            // Rule validation → Validation
            Self::InvalidRule(_) | Self::InvalidData(_) => ErrorCategory::Validation,

            // Timeout → Timeout
            Self::TimeoutError(_) => ErrorCategory::Timeout,

            // Execution/Evaluation/Condition/Action/Expression → RuleEngine
            Self::ExecutionError(_)
            | Self::EvaluationError(_)
            | Self::ConditionError(_)
            | Self::ActionError(_)
            | Self::ExpressionError(_) => ErrorCategory::RuleEngine,

            // All other errors → Internal
            Self::RuleError(_)
            | Self::RuleDisabled(_)
            | Self::SchedulerError(_)
            | Self::CronError(_)
            | Self::DataError(_)
            | Self::SerializationError(_)
            | Self::InternalError(_)
            | Self::IoError(_)
            | Self::LockError(_) => ErrorCategory::Internal,

            // Unknown → Unknown
            Self::UnknownError(_) => ErrorCategory::Unknown,
        }
    }
}

// ============================================================================
// API Adaptation: RuleSrvError → AppError conversion
// ============================================================================

/// Automatically convert RuleSrvError to AppError using VoltageErrorTrait for HTTP status mapping
impl From<RuleSrvError> for voltage_config::api::AppError {
    fn from(err: RuleSrvError) -> Self {
        use voltage_config::{
            api::{AppError, ErrorInfo},
            error::VoltageErrorTrait,
        };

        let status = err.http_status();
        let error_info = ErrorInfo::new(err.to_string())
            .with_code(status.as_u16())
            .with_details(format!(
                "error_code: {}, category: {:?}, retryable: {}",
                err.error_code(),
                err.category(),
                err.is_retryable()
            ));

        AppError::new(status, error_info)
    }
}

/// Implement IntoResponse so RuleSrvError can be returned directly from Axum handlers
impl axum::response::IntoResponse for RuleSrvError {
    fn into_response(self) -> axum::response::Response {
        let app_error: voltage_config::api::AppError = self.into();
        app_error.into_response()
    }
}

// ============================================================================
// Interoperability conversions
// ============================================================================

/// Convert from VoltageError
impl From<voltage_config::error::VoltageError> for RuleSrvError {
    fn from(err: voltage_config::error::VoltageError) -> Self {
        use voltage_config::error::VoltageError as VE;
        match err {
            VE::Configuration(msg) => Self::ConfigError(msg),
            VE::InvalidConfig { field, reason } => Self::ConfigurationError {
                field,
                message: reason,
            },
            VE::MissingConfig(msg) => Self::MissingConfig(msg),
            VE::Database(msg) => Self::DatabaseError(msg),
            VE::Sqlite(e) => Self::SqliteError(e.to_string()),
            VE::Redis(e) => Self::RedisError(e.to_string()),
            VE::Io(e) => Self::IoError(e.to_string()),
            VE::Timeout(_) => Self::TimeoutError("Operation timeout".to_string()),
            VE::Serialization(e) => Self::SerializationError(e.to_string()),
            _ => Self::InternalError(err.to_string()),
        }
    }
}

/// Convert from RtdbError
impl From<voltage_rtdb::error::RtdbError> for RuleSrvError {
    fn from(err: voltage_rtdb::error::RtdbError) -> Self {
        use voltage_rtdb::error::RtdbError as RE;
        match err {
            RE::ConnectionError(msg) => Self::RedisError(format!("RTDB connection: {}", msg)),
            RE::KeyNotFound(key) => Self::DataError(format!("Key not found: {}", key)),
            RE::InvalidDataType { .. } => Self::DataError(err.to_string()),
            RE::SerializationError(msg) => Self::SerializationError(msg),
            _ => Self::InternalError(err.to_string()),
        }
    }
}

/// Convert from SQLx Error
impl From<sqlx::Error> for RuleSrvError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => Self::RuleNotFound("Database row not found".to_string()),
            sqlx::Error::Database(e) => Self::SqliteError(e.to_string()),
            _ => Self::DatabaseError(err.to_string()),
        }
    }
}

/// Convert from IO Error
impl From<std::io::Error> for RuleSrvError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err.to_string())
    }
}

/// Convert from serde_json Error
impl From<serde_json::Error> for RuleSrvError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError(err.to_string())
    }
}

/// Convert from anyhow Error
impl From<anyhow::Error> for RuleSrvError {
    fn from(err: anyhow::Error) -> Self {
        Self::InternalError(err.to_string())
    }
}
