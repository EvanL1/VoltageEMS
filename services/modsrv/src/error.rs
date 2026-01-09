//! ModSrv Error Types
//!
//! Domain-specific error handling for Model Service.
//!
//! Simplified error types (15 variants) - reduced from 40+ to improve maintainability.
//! All errors map to ErrorCategory for HTTP status codes.

use thiserror::Error;

/// ModSrv Result type alias
pub type Result<T> = std::result::Result<T, ModSrvError>;

/// Model Service errors with domain-specific semantics
///
/// Simplified to core error categories that callers can meaningfully handle.
#[derive(Error, Debug, Clone)]
pub enum ModSrvError {
    // ============================================================================
    // Configuration Errors
    // ============================================================================
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    // ============================================================================
    // Database Errors
    // ============================================================================
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Redis error: {0}")]
    RedisError(String),

    // ============================================================================
    // Instance Management Errors
    // ============================================================================
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    #[error("Instance already exists: {0}")]
    InstanceExists(String),

    // ============================================================================
    // Rule Engine Errors
    // ============================================================================
    #[error("Rule not found: {0}")]
    RuleNotFound(String),

    #[error("Rule already exists: {0}")]
    RuleExists(String),

    #[error("Invalid rule: {0}")]
    InvalidRule(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Scheduler error: {0}")]
    SchedulerError(String),

    // ============================================================================
    // Validation Errors
    // ============================================================================
    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Invalid routing: {0}")]
    InvalidRouting(String),

    // ============================================================================
    // Data Operation Errors
    // ============================================================================
    #[error("Serialization error: {0}")]
    SerializationError(String),

    // ============================================================================
    // Internal Errors
    // ============================================================================
    #[error("Internal error: {0}")]
    InternalError(String),
}

// ============================================================================
// VoltageErrorTrait Implementation
// ============================================================================

impl errors::VoltageErrorTrait for ModSrvError {
    fn error_code(&self) -> &'static str {
        match self {
            // Configuration
            Self::ConfigError(_) => "MODSRV_CONFIG_ERROR",
            Self::InvalidConfig(_) => "MODSRV_INVALID_CONFIG",
            Self::MissingConfig(_) => "MODSRV_MISSING_CONFIG",

            // Database
            Self::DatabaseError(_) => "MODSRV_DATABASE_ERROR",
            Self::RedisError(_) => "MODSRV_REDIS_ERROR",

            // Instance
            Self::InstanceNotFound(_) => "MODSRV_INSTANCE_NOT_FOUND",
            Self::InstanceExists(_) => "MODSRV_INSTANCE_EXISTS",

            // Rule Engine
            Self::RuleNotFound(_) => "MODSRV_RULE_NOT_FOUND",
            Self::RuleExists(_) => "MODSRV_RULE_EXISTS",
            Self::InvalidRule(_) => "MODSRV_INVALID_RULE",
            Self::ParseError(_) => "MODSRV_PARSE_ERROR",
            Self::ExecutionError(_) => "MODSRV_EXECUTION_ERROR",
            Self::SchedulerError(_) => "MODSRV_SCHEDULER_ERROR",

            // Validation
            Self::InvalidData(_) => "MODSRV_INVALID_DATA",
            Self::InvalidRouting(_) => "MODSRV_INVALID_ROUTING",

            // Data
            Self::SerializationError(_) => "MODSRV_SERIALIZATION_ERROR",

            // Internal
            Self::InternalError(_) => "MODSRV_INTERNAL_ERROR",
        }
    }

    fn category(&self) -> errors::ErrorCategory {
        use errors::ErrorCategory;

        match self {
            // Configuration → Configuration
            Self::ConfigError(_) | Self::InvalidConfig(_) | Self::MissingConfig(_) => {
                ErrorCategory::Configuration
            },

            // Database → Database
            Self::DatabaseError(_) | Self::RedisError(_) => ErrorCategory::Database,

            // NotFound
            Self::InstanceNotFound(_) | Self::RuleNotFound(_) => ErrorCategory::NotFound,

            // Conflict
            Self::InstanceExists(_) | Self::RuleExists(_) => ErrorCategory::Conflict,

            // Validation
            Self::InvalidData(_)
            | Self::InvalidRouting(_)
            | Self::InvalidRule(_)
            | Self::ParseError(_) => ErrorCategory::Validation,

            // Internal (execution, scheduling, serialization, etc.)
            Self::ExecutionError(_)
            | Self::SchedulerError(_)
            | Self::SerializationError(_)
            | Self::InternalError(_) => ErrorCategory::Internal,
        }
    }
}

// ============================================================================
// API Adaptation: ModSrvError → AppError conversion
// ============================================================================

/// Automatically convert ModSrvError to AppError using VoltageErrorTrait for HTTP status mapping
impl From<ModSrvError> for common::AppError {
    fn from(err: ModSrvError) -> Self {
        use common::{AppError, ErrorInfo};
        use errors::VoltageErrorTrait;

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

/// Implement IntoResponse so ModSrvError can be returned directly from Axum handlers
impl axum::response::IntoResponse for ModSrvError {
    fn into_response(self) -> axum::response::Response {
        let app_error: common::AppError = self.into();
        app_error.into_response()
    }
}

// ============================================================================
// Interoperability conversions
// ============================================================================

/// Convert from VoltageError
impl From<errors::VoltageError> for ModSrvError {
    fn from(err: errors::VoltageError) -> Self {
        use errors::VoltageError as VE;
        match err {
            VE::Configuration(msg) => Self::ConfigError(msg),
            VE::InvalidConfig { field, reason } => {
                Self::InvalidConfig(format!("{}: {}", field, reason))
            },
            VE::MissingConfig(msg) => Self::MissingConfig(msg),
            VE::Database(msg) => Self::DatabaseError(msg),
            VE::Sqlite(e) => Self::DatabaseError(format!("SQLite: {}", e)),
            VE::Redis(e) => Self::RedisError(e.to_string()),
            VE::Io(e) => Self::InternalError(format!("IO: {}", e)),
            VE::Timeout(d) => Self::InternalError(format!("Timeout: {:?}", d)),
            VE::Serialization(e) => Self::SerializationError(e.to_string()),
            _ => Self::InternalError(err.to_string()),
        }
    }
}

/// Convert from RtdbError
impl From<voltage_rtdb::error::RtdbError> for ModSrvError {
    fn from(err: voltage_rtdb::error::RtdbError) -> Self {
        use voltage_rtdb::error::RtdbError as RE;
        match err {
            RE::ConnectionError(msg) => Self::RedisError(format!("RTDB connection: {}", msg)),
            RE::KeyNotFound(key) => Self::InternalError(format!("Key not found: {}", key)),
            RE::InvalidDataType { .. } => Self::InternalError(err.to_string()),
            RE::SerializationError(msg) => Self::SerializationError(msg),
            _ => Self::InternalError(err.to_string()),
        }
    }
}

/// Convert from SQLx Error
impl From<sqlx::Error> for ModSrvError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => {
                Self::InstanceNotFound("Database row not found".to_string())
            },
            sqlx::Error::Database(e) => Self::DatabaseError(e.to_string()),
            _ => Self::DatabaseError(err.to_string()),
        }
    }
}

/// Convert from IO Error
impl From<std::io::Error> for ModSrvError {
    fn from(err: std::io::Error) -> Self {
        Self::InternalError(format!("IO: {}", err))
    }
}

/// Convert from serde_json Error
impl From<serde_json::Error> for ModSrvError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError(err.to_string())
    }
}

/// Convert from anyhow Error
impl From<anyhow::Error> for ModSrvError {
    fn from(err: anyhow::Error) -> Self {
        Self::InternalError(err.to_string())
    }
}

/// Convert from voltage_rules::RuleError
impl From<voltage_rules::RuleError> for ModSrvError {
    fn from(err: voltage_rules::RuleError) -> Self {
        use voltage_rules::RuleError as RE;
        match err {
            RE::NotFound(id) => Self::RuleNotFound(id),
            RE::AlreadyExists(id) => Self::RuleExists(id),
            RE::InvalidFormat(msg) => Self::InvalidRule(msg),
            RE::ParseError(msg) => Self::ParseError(msg),
            RE::ExecutionError(msg) => Self::ExecutionError(msg),
            RE::ConditionError(msg) => Self::ExecutionError(format!("Condition: {}", msg)),
            RE::ActionError(msg) => Self::ExecutionError(format!("Action: {}", msg)),
            RE::DatabaseError(msg) => Self::DatabaseError(msg),
            RE::SerializationError(msg) => Self::SerializationError(msg),
            RE::SchedulerError(msg) => Self::SchedulerError(msg),
            RE::RtdbError(msg) => Self::RedisError(msg),
            RE::RoutingError(msg) => Self::InternalError(format!("Routing: {}", msg)),
        }
    }
}
