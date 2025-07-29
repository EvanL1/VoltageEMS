use thiserror::Error;

/// Result type for rulesrv
pub type Result<T> = std::result::Result<T, RulesrvError>;

/// Errors that can occur in rulesrv
#[derive(Error, Debug)]
pub enum RulesrvError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("Rule not found: {0}")]
    RuleNotFound(String),

    #[error("Rule execution error: {0}")]
    ExecutionError(String),

    #[error("Rule error: {0}")]
    RuleError(String),

    #[error("Action execution error: {0}")]
    ActionExecutionError(String),

    #[error("Action type not supported: {0}")]
    ActionTypeNotSupported(String),

    #[error("Invalid rule definition: {0}")]
    InvalidRule(String),

    #[error("Rule parsing error: {0}")]
    RuleParsingError(String),

    #[error("Rule disabled: {0}")]
    RuleDisabled(String),

    #[error("Lock error")]
    LockError,

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Condition evaluation error: {0}")]
    ConditionError(String),

    #[error("DAG cycle detected")]
    DagCycleError,

    #[error("Node not found in DAG: {0}")]
    NodeNotFound(String),

    #[error("Invalid operator: {0}")]
    InvalidOperator(String),

    #[error("Type mismatch in condition evaluation")]
    TypeMismatch,

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Handler not found for action type: {0}")]
    HandlerNotFound(String),

    #[error("API error: {0}")]
    ApiError(String),

    // Removed voltage_common dependency
    #[error("Internal error: {0}")]
    InternalError(#[from] anyhow::Error),
}

impl From<redis::RedisError> for RulesrvError {
    fn from(err: redis::RedisError) -> Self {
        RulesrvError::RedisError(err.to_string())
    }
}
