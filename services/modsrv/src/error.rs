//! ModSrv Error Types
//!
//! Domain-specific error handling for Model Service

use thiserror::Error;

/// ModSrv Result type alias
pub type Result<T> = std::result::Result<T, ModSrvError>;

/// Model Service errors with domain-specific semantics
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
    // Instance Management Errors
    // ============================================================================
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    #[error("Instance already exists: {0}")]
    InstanceExists(String),

    #[error("Instance error: {0}")]
    InstanceError(String),

    #[error("Invalid instance: {0}")]
    InvalidInstance(String),

    #[error("Instance state error: {0}")]
    InstanceStateError(String),

    // ============================================================================
    // Product Management Errors
    // ============================================================================
    #[error("Product not found: {0}")]
    ProductNotFound(String),

    #[error("Product error: {0}")]
    ProductError(String),

    #[error("Invalid product: {0}")]
    InvalidProduct(String),

    // ============================================================================
    // Routing Management Errors
    // ============================================================================
    #[error("Routing error: {0}")]
    RoutingError(String),

    #[error("Routing not found: {0}")]
    RoutingNotFound(String),

    #[error("Invalid routing: {0}")]
    InvalidRouting(String),

    #[error("Routing conflict: {0}")]
    RoutingConflict(String),

    // ============================================================================
    // Calculation Errors
    // ============================================================================
    #[error("Calculation error: {0}")]
    CalculationError(String),

    #[error("Calculation not found: {0}")]
    CalculationNotFound(String),

    #[error("Expression error: {0}")]
    ExpressionError(String),

    #[error("Invalid calculation: {0}")]
    InvalidCalculation(String),

    // ============================================================================
    // Rule Engine Errors
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

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Evaluation error: {0}")]
    EvaluationError(String),

    #[error("Scheduler error: {0}")]
    SchedulerError(String),

    // ============================================================================
    // Point Operation Errors
    // ============================================================================
    #[error("Point not found: {0}")]
    PointNotFound(String),

    #[error("Point error: {0}")]
    PointError(String),

    // ============================================================================
    // Data Operation Errors
    // ============================================================================
    #[error("Data error: {0}")]
    DataError(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Data conversion error: {0}")]
    DataConversionError(String),

    // ============================================================================
    // Internal Errors
    // ============================================================================
    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Unknown error: {0}")]
    UnknownError(String),
}

// ============================================================================
// VoltageErrorTrait Implementation
// ============================================================================

impl voltage_config::error::VoltageErrorTrait for ModSrvError {
    fn error_code(&self) -> &'static str {
        match self {
            // Configuration
            Self::ConfigError(_) => "MODSRV_CONFIG_ERROR",
            Self::InvalidConfig(_) => "MODSRV_INVALID_CONFIG",
            Self::MissingConfig(_) => "MODSRV_MISSING_CONFIG",
            Self::ConfigurationError { .. } => "MODSRV_CONFIGURATION_ERROR",

            // Database
            Self::DatabaseError(_) => "MODSRV_DATABASE_ERROR",
            Self::SqliteError(_) => "MODSRV_SQLITE_ERROR",
            Self::RedisError(_) => "MODSRV_REDIS_ERROR",

            // Instance
            Self::InstanceNotFound(_) => "MODSRV_INSTANCE_NOT_FOUND",
            Self::InstanceExists(_) => "MODSRV_INSTANCE_EXISTS",
            Self::InstanceError(_) => "MODSRV_INSTANCE_ERROR",
            Self::InvalidInstance(_) => "MODSRV_INVALID_INSTANCE",
            Self::InstanceStateError(_) => "MODSRV_INSTANCE_STATE_ERROR",

            // Product
            Self::ProductNotFound(_) => "MODSRV_PRODUCT_NOT_FOUND",
            Self::ProductError(_) => "MODSRV_PRODUCT_ERROR",
            Self::InvalidProduct(_) => "MODSRV_INVALID_PRODUCT",

            // Routing
            Self::RoutingError(_) => "MODSRV_ROUTING_ERROR",
            Self::RoutingNotFound(_) => "MODSRV_ROUTING_NOT_FOUND",
            Self::InvalidRouting(_) => "MODSRV_INVALID_ROUTING",
            Self::RoutingConflict(_) => "MODSRV_ROUTING_CONFLICT",

            // Calculation
            Self::CalculationError(_) => "MODSRV_CALCULATION_ERROR",
            Self::CalculationNotFound(_) => "MODSRV_CALCULATION_NOT_FOUND",
            Self::ExpressionError(_) => "MODSRV_EXPRESSION_ERROR",
            Self::InvalidCalculation(_) => "MODSRV_INVALID_CALCULATION",

            // Rule Engine
            Self::RuleNotFound(_) => "MODSRV_RULE_NOT_FOUND",
            Self::RuleExists(_) => "MODSRV_RULE_EXISTS",
            Self::RuleError(_) => "MODSRV_RULE_ERROR",
            Self::InvalidRule(_) => "MODSRV_INVALID_RULE",
            Self::RuleDisabled(_) => "MODSRV_RULE_DISABLED",
            Self::ParseError(_) => "MODSRV_PARSE_ERROR",
            Self::ExecutionError(_) => "MODSRV_EXECUTION_ERROR",
            Self::EvaluationError(_) => "MODSRV_EVALUATION_ERROR",
            Self::SchedulerError(_) => "MODSRV_SCHEDULER_ERROR",

            // Point
            Self::PointNotFound(_) => "MODSRV_POINT_NOT_FOUND",
            Self::PointError(_) => "MODSRV_POINT_ERROR",

            // Data
            Self::DataError(_) => "MODSRV_DATA_ERROR",
            Self::InvalidData(_) => "MODSRV_INVALID_DATA",
            Self::SerializationError(_) => "MODSRV_SERIALIZATION_ERROR",
            Self::DataConversionError(_) => "MODSRV_DATA_CONVERSION_ERROR",

            // Internal
            Self::InternalError(_) => "MODSRV_INTERNAL_ERROR",
            Self::IoError(_) => "MODSRV_IO_ERROR",
            Self::TimeoutError(_) => "MODSRV_TIMEOUT_ERROR",
            Self::LockError(_) => "MODSRV_LOCK_ERROR",
            Self::UnknownError(_) => "MODSRV_UNKNOWN_ERROR",
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

            // Instance NotFound → NotFound
            Self::InstanceNotFound(_)
            | Self::ProductNotFound(_)
            | Self::RoutingNotFound(_)
            | Self::CalculationNotFound(_)
            | Self::PointNotFound(_)
            | Self::RuleNotFound(_) => ErrorCategory::NotFound,

            // Instance Exists → Conflict
            Self::InstanceExists(_) | Self::RoutingConflict(_) | Self::RuleExists(_) => {
                ErrorCategory::Conflict
            },

            // Instance/Product/Routing/Calculation validation → Validation
            Self::InvalidInstance(_)
            | Self::InvalidProduct(_)
            | Self::InvalidRouting(_)
            | Self::InvalidCalculation(_)
            | Self::InvalidData(_)
            | Self::InvalidRule(_)
            | Self::ParseError(_) => ErrorCategory::Validation,

            // Calculation → Calculation
            Self::CalculationError(_) | Self::ExpressionError(_) | Self::EvaluationError(_) => {
                ErrorCategory::Calculation
            },

            // Rule Engine → Internal (execution/scheduling errors)
            Self::RuleError(_)
            | Self::RuleDisabled(_)
            | Self::ExecutionError(_)
            | Self::SchedulerError(_) => ErrorCategory::Internal,

            // Timeout → Timeout
            Self::TimeoutError(_) => ErrorCategory::Timeout,

            // All other instance/product/routing/point/data errors → Internal
            Self::InstanceError(_)
            | Self::InstanceStateError(_)
            | Self::ProductError(_)
            | Self::RoutingError(_)
            | Self::PointError(_)
            | Self::DataError(_)
            | Self::SerializationError(_)
            | Self::DataConversionError(_)
            | Self::InternalError(_)
            | Self::IoError(_)
            | Self::LockError(_) => ErrorCategory::Internal,

            // Unknown → Unknown
            Self::UnknownError(_) => ErrorCategory::Unknown,
        }
    }
}

// ============================================================================
// API Adaptation: ModSrvError → AppError conversion
// ============================================================================

/// Automatically convert ModSrvError to AppError using VoltageErrorTrait for HTTP status mapping
impl From<ModSrvError> for voltage_config::api::AppError {
    fn from(err: ModSrvError) -> Self {
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

/// Implement IntoResponse so ModSrvError can be returned directly from Axum handlers
impl axum::response::IntoResponse for ModSrvError {
    fn into_response(self) -> axum::response::Response {
        let app_error: voltage_config::api::AppError = self.into();
        app_error.into_response()
    }
}

// ============================================================================
// Interoperability conversions
// ============================================================================

/// Convert from VoltageError
impl From<voltage_config::error::VoltageError> for ModSrvError {
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
impl From<voltage_rtdb::error::RtdbError> for ModSrvError {
    fn from(err: voltage_rtdb::error::RtdbError) -> Self {
        use voltage_rtdb::error::RtdbError as RE;
        match err {
            RE::ConnectionError(msg) => Self::RedisError(format!("RTDB connection: {}", msg)),
            RE::KeyNotFound(key) => Self::DataError(format!("Key not found: {}", key)),
            RE::InvalidDataType { .. } => Self::DataConversionError(err.to_string()),
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
            sqlx::Error::Database(e) => Self::SqliteError(e.to_string()),
            _ => Self::DatabaseError(err.to_string()),
        }
    }
}

/// Convert from IO Error
impl From<std::io::Error> for ModSrvError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err.to_string())
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
            RE::ConditionError(msg) => Self::EvaluationError(msg),
            RE::ActionError(msg) => Self::ExecutionError(format!("Action: {}", msg)),
            RE::DatabaseError(msg) => Self::DatabaseError(msg),
            RE::SerializationError(msg) => Self::SerializationError(msg),
            RE::SchedulerError(msg) => Self::SchedulerError(msg),
            RE::RtdbError(msg) => Self::RedisError(msg),
            RE::RoutingError(msg) => Self::RoutingError(msg),
        }
    }
}
