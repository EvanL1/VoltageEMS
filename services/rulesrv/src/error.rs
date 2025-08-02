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

    #[error("Rule error: {0}")]
    RuleError(String),

    #[error("Rule parsing error: {0}")]
    RuleParsingError(String),

    #[error("Action execution error: {0}")]
    ActionExecutionError(String),

    #[error("API error: {0}")]
    ApiError(String),
}

impl From<redis::RedisError> for RulesrvError {
    fn from(err: redis::RedisError) -> Self {
        RulesrvError::RedisError(err.to_string())
    }
}
