//! # Error Handling for Model Service
//! 
//! This module provides comprehensive error handling for the Model Service (ModSrv),
//! including custom error types, result handling, and integration with external
//! error types from dependencies.
//! 
//! ## Overview
//! 
//! The error system is designed to provide clear, actionable error information
//! for different failure scenarios in model execution, data processing, and
//! system operations. All errors implement standard Rust error traits and
//! provide detailed context information.
//! 
//! ## Error Categories
//! 
//! - **Configuration Errors**: Invalid configurations, missing files, format issues
//! - **Data Errors**: Invalid data formats, serialization failures, parsing errors
//! - **Storage Errors**: Redis connection issues, I/O failures, permission problems
//! - **Model Errors**: Model execution failures, template issues, validation errors
//! - **Rule Errors**: Rule parsing, execution, and validation failures
//! - **Control Errors**: Action execution failures, permission issues
//! - **System Errors**: Resource lock failures, internal system errors
//! 
//! ## Usage Examples
//! 
//! ```rust
//! use modsrv::{Result, ModelSrvError};
//! 
//! // Function that might fail
//! fn load_model_config(path: &str) -> Result<String> {
//!     if path.is_empty() {
//!         return Err(ModelSrvError::ConfigError(
//!             "Model configuration path cannot be empty".to_string()
//!         ));
//!     }
//!     
//!     // Load and return config...
//!     Ok("config_content".to_string())
//! }
//! 
//! // Error handling with pattern matching
//! match load_model_config("") {
//!     Ok(config) => println!("Loaded config: {}", config),
//!     Err(ModelSrvError::ConfigError(msg)) => {
//!         eprintln!("Configuration error: {}", msg);
//!     },
//!     Err(e) => eprintln!("Other error: {}", e),
//! }
//! ```

use serde_json;
use serde_yaml;
use redis;
use std::error::Error;
use std::fmt;
use warp::reject;

/// Result type alias for Model Service operations
/// 
/// This is a convenience type alias that uses `ModelSrvError` as the error type
/// for all Model Service operations, providing consistent error handling throughout
/// the codebase.
pub type Result<T> = std::result::Result<T, ModelSrvError>;

/// Comprehensive error types for Model Service operations
/// 
/// This enumeration covers all possible error conditions that can occur
/// during model service operations, from configuration and data handling
/// to model execution and control operations.
/// 
/// Each variant provides detailed context about the specific failure,
/// making it easier to diagnose and handle different error conditions
/// appropriately.
#[derive(Debug, Clone)]
pub enum ModelSrvError {
    /// Configuration-related errors
    /// 
    /// Occurs when there are issues with configuration files, invalid
    /// configuration parameters, or missing required configuration values.
    /// 
    /// # Example
    /// ```rust
    /// # use modsrv::ModelSrvError;
    /// let error = ModelSrvError::ConfigError(
    ///     "Missing required Redis host configuration".to_string()
    /// );
    /// ```
    ConfigError(String),
    
    /// Data format and parsing errors
    /// 
    /// Triggered when data doesn't match expected formats, schemas,
    /// or when parsing operations fail due to malformed input.
    FormatError(String),
    
    /// Invalid command or operation errors
    /// 
    /// Used when an unsupported or malformed command is received,
    /// or when operations are attempted in invalid states.
    InvalidCommand(String),
    
    /// Redis database operation errors
    /// 
    /// Covers Redis connection failures, command execution errors,
    /// network issues, and Redis-specific operational problems.
    RedisError(String),
    
    /// Permission and authorization errors
    /// 
    /// Used when operations are denied due to insufficient permissions
    /// or authorization failures.
    PermissionDenied(String),
    
    /// Input/Output operation errors
    /// 
    /// File system errors, network I/O failures, and other
    /// input/output related problems.
    IoError(String),
    
    /// Resource lock acquisition errors
    /// 
    /// Occurs when thread synchronization primitives (mutexes, read-write locks)
    /// cannot be acquired, typically indicating potential deadlocks or
    /// resource contention issues.
    LockError,
    
    /// General serialization/deserialization errors
    /// 
    /// Covers failures in converting data structures to/from various
    /// serialization formats beyond JSON and YAML.
    SerdeError(String),
    
    /// General parsing errors
    /// 
    /// Used for parsing failures that don't fall into more specific
    /// categories like JSON or YAML parsing.
    ParseError(String),
    
    /// JSON format and parsing errors
    /// 
    /// Specific to JSON serialization/deserialization failures,
    /// malformed JSON data, or JSON schema validation errors.
    JsonError(String),
    
    /// Invalid or corrupted data errors
    /// 
    /// Used when data passes format validation but contains
    /// logically invalid or inconsistent values.
    InvalidData(String),
    
    /// YAML format and parsing errors
    /// 
    /// Specific to YAML serialization/deserialization failures,
    /// malformed YAML data, or YAML schema validation errors.
    YamlError(String),
    
    /// Template system errors
    /// 
    /// General template-related errors including template processing,
    /// variable substitution, and template engine failures.
    TemplateError(String),
    
    /// Template not found errors
    /// 
    /// Occurs when attempting to access a template that doesn't exist
    /// in the template registry or storage system.
    TemplateNotFound(String),
    
    /// Model execution and definition errors
    /// 
    /// General model-related errors including execution failures,
    /// invalid model definitions, and model state inconsistencies.
    ModelError(String),
    
    /// Rule not found errors
    /// 
    /// Used when attempting to access a rule that doesn't exist
    /// in the rules engine or storage system.
    RuleNotFound(String),
    
    /// Rule execution failures
    /// 
    /// Occurs during rule evaluation or execution, including
    /// condition evaluation errors and action execution failures.
    RuleExecutionError(String),
    
    /// Invalid rule definition errors
    /// 
    /// Used when rule definitions contain syntax errors, invalid
    /// conditions, or malformed action specifications.
    InvalidRuleDefinition(String),
    
    /// Action handler not found errors
    /// 
    /// Occurs when a rule references an action handler that
    /// is not registered or available in the system.
    ActionHandlerNotFound(String),
    
    /// Invalid action configuration errors
    /// 
    /// Used when action configurations contain invalid parameters,
    /// missing required fields, or inconsistent settings.
    InvalidActionConfig(String),
    
    /// Rule parsing errors
    /// 
    /// Specific to parsing rule definitions from various formats,
    /// including syntax and semantic validation failures.
    RuleParsingError(String),
    
    /// Rule disabled errors
    /// 
    /// Used when attempting to execute a rule that has been
    /// disabled in the configuration or runtime settings.
    RuleDisabled(String),
    
    /// Unsupported action type errors
    /// 
    /// Occurs when a rule specifies an action type that is not
    /// supported by the current system configuration.
    ActionTypeNotSupported(String),
    
    /// Action execution failures
    /// 
    /// Used when action execution fails due to external system
    /// errors, network issues, or target system unavailability.
    ActionExecutionError(String),
    
    /// Template already exists errors
    /// 
    /// Occurs when attempting to create a template with an ID
    /// that already exists in the template registry.
    TemplateAlreadyExists(String),
    
    /// General rule system errors
    /// 
    /// Catch-all for rule-related errors that don't fit into
    /// more specific rule error categories.
    RuleError(String),
    
    /// Authentication and authorization errors
    /// 
    /// Used for authentication failures, invalid credentials,
    /// and authorization token issues.
    InvalidAuth(String),
    
    /// Communication service errors
    /// 
    /// Used when interfacing with external communication services
    /// or when communication protocols fail.
    ComSrvError(String),
    
    /// Model not found errors
    /// 
    /// Occurs when attempting to access a model that doesn't exist
    /// in the model registry or storage system.
    ModelNotFound(String),
    
    /// Model already exists errors
    /// 
    /// Used when attempting to create a model with an ID that
    /// already exists in the model registry.
    ModelAlreadyExists(String),
    
    /// Data validation errors
    /// 
    /// Occurs when data fails validation checks, schema validation,
    /// or business rule validation.
    ValidationError(String),
    
    /// Data mapping configuration errors
    /// 
    /// Used when data mapping configurations are invalid, contain
    /// circular references, or specify non-existent fields.
    DataMappingError(String),
    
    /// Key not found in storage errors
    /// 
    /// Occurs when attempting to access a storage key that
    /// doesn't exist in the backend storage system.
    KeyNotFound(String),
    
    /// Invalid operation errors
    /// 
    /// Used when operations are attempted in invalid contexts,
    /// such as modifying read-only data or using incorrect APIs.
    InvalidOperation(String),
    
    /// Invalid input parameter errors
    /// 
    /// Occurs when function or API inputs don't meet required
    /// criteria, contain invalid values, or are out of range.
    InvalidInput(String),
    
    /// Unauthorized access errors
    /// 
    /// Used when access is denied due to insufficient privileges
    /// or invalid authorization context.
    Unauthorized(String),
    
    /// Not implemented functionality errors
    /// 
    /// Used for features or functionality that are not yet
    /// implemented or are disabled in the current build.
    NotImplemented(String),
}

impl fmt::Display for ModelSrvError {
    /// Format the error for human-readable display
    /// 
    /// Provides consistent, user-friendly error messages that include
    /// context information and actionable details when possible.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelSrvError::RedisError(e) => write!(f, "Redis error: {}", e),
            ModelSrvError::IoError(e) => write!(f, "IO error: {}", e),
            ModelSrvError::SerdeError(e) => write!(f, "Serialization error: {}", e),
            ModelSrvError::JsonError(e) => write!(f, "JSON error: {}", e),
            ModelSrvError::YamlError(e) => write!(f, "YAML error: {}", e),
            ModelSrvError::LockError => write!(f, "Lock error"),
            ModelSrvError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            ModelSrvError::FormatError(msg) => write!(f, "Format error: {}", msg),
            ModelSrvError::InvalidCommand(msg) => write!(f, "Invalid command: {}", msg),
            ModelSrvError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            ModelSrvError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ModelSrvError::TemplateError(msg) => write!(f, "Template error: {}", msg),
            ModelSrvError::TemplateNotFound(id) => write!(f, "Template not found: {}", id),
            ModelSrvError::ModelError(msg) => write!(f, "Model error: {}", msg),
            ModelSrvError::RuleNotFound(id) => write!(f, "Rule not found: {}", id),
            ModelSrvError::RuleExecutionError(msg) => write!(f, "Rule execution error: {}", msg),
            ModelSrvError::InvalidRuleDefinition(msg) => write!(f, "Invalid rule definition: {}", msg),
            ModelSrvError::ActionHandlerNotFound(id) => write!(f, "Action handler not found: {}", id),
            ModelSrvError::InvalidActionConfig(msg) => write!(f, "Invalid action config: {}", msg),
            ModelSrvError::RuleParsingError(msg) => write!(f, "Rule parsing error: {}", msg),
            ModelSrvError::RuleDisabled(id) => write!(f, "Rule is disabled: {}", id),
            ModelSrvError::ActionTypeNotSupported(msg) => write!(f, "Action type not supported: {}", msg),
            ModelSrvError::ActionExecutionError(msg) => write!(f, "Action execution error: {}", msg),
            ModelSrvError::TemplateAlreadyExists(id) => write!(f, "Template already exists: {}", id),
            ModelSrvError::RuleError(msg) => write!(f, "Rule error: {}", msg),
            ModelSrvError::InvalidAuth(msg) => write!(f, "Authentication error: {}", msg),
            ModelSrvError::ComSrvError(msg) => write!(f, "Communication service error: {}", msg),
            ModelSrvError::ModelNotFound(id) => write!(f, "Model not found: {}", id),
            ModelSrvError::ModelAlreadyExists(id) => write!(f, "Model already exists: {}", id),
            ModelSrvError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ModelSrvError::DataMappingError(msg) => write!(f, "Data mapping error: {}", msg),
            ModelSrvError::KeyNotFound(key) => write!(f, "Key not found: {}", key),
            ModelSrvError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
            ModelSrvError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            ModelSrvError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            ModelSrvError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
            ModelSrvError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
        }
    }
}

impl Error for ModelSrvError {
    /// Get the source error that caused this error
    /// 
    /// Currently returns None as most errors are leaf errors,
    /// but this could be extended to provide error chaining
    /// in future versions.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

/// Convert Redis errors to ModelSrvError
/// 
/// Automatically converts Redis connection and operation errors
/// to the appropriate ModelSrvError variant with context preservation.
impl From<redis::RedisError> for ModelSrvError {
    fn from(err: redis::RedisError) -> Self {
        ModelSrvError::RedisError(err.to_string())
    }
}

/// Convert standard I/O errors to ModelSrvError
/// 
/// Handles file system, network, and other I/O related errors
/// by wrapping them in the IoError variant.
impl From<std::io::Error> for ModelSrvError {
    fn from(err: std::io::Error) -> Self {
        ModelSrvError::IoError(err.to_string())
    }
}

/// Convert JSON serialization errors to ModelSrvError
/// 
/// Handles serde_json errors during serialization and deserialization
/// operations, providing clear context about JSON processing failures.
impl From<serde_json::Error> for ModelSrvError {
    fn from(err: serde_json::Error) -> Self {
        ModelSrvError::SerdeError(err.to_string())
    }
}

/// Convert configuration errors to ModelSrvError
/// 
/// Handles configuration file loading and parsing errors
/// from the config crate, providing context about configuration issues.
impl From<config::ConfigError> for ModelSrvError {
    fn from(err: config::ConfigError) -> Self {
        ModelSrvError::ConfigError(err.to_string())
    }
}

/// Convert YAML serialization errors to ModelSrvError
/// 
/// Handles serde_yaml errors during YAML file processing,
/// providing specific context about YAML parsing failures.
impl From<serde_yaml::Error> for ModelSrvError {
    fn from(err: serde_yaml::Error) -> Self {
        ModelSrvError::YamlError(err.to_string())
    }
}

/// Enable ModelSrvError to be used as a Warp rejection
/// 
/// This implementation allows ModelSrvError to be used directly
/// with the Warp web framework for HTTP API error handling.
impl reject::Reject for ModelSrvError {}

/// Convert Tokio join errors to ModelSrvError
/// 
/// Handles errors from Tokio task join operations, typically
/// occurring when background tasks fail or are cancelled.
impl From<tokio::task::JoinError> for ModelSrvError {
    fn from(e: tokio::task::JoinError) -> Self {
        ModelSrvError::IoError(e.to_string())
    }
}

