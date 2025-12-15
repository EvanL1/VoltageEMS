//! Common configuration structures shared across all services
//!
//! This module provides shared types for service configuration including:
//! - Base configuration structs (ApiConfig, RedisConfig, LoggingConfig)
//! - Validation framework (ConfigValidator, ValidationResult)
//! - Hot reload infrastructure (ReloadableService, ReloadResult)
//! - Shared enums (PointRole, InstanceStatus, ResponseStatus, ComparisonOperator)

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use voltage_schema_macro::Schema;

// Re-export PointType from voltage-model and alias as FourRemote for compatibility
pub use voltage_model::PointType;

#[cfg(feature = "schema")]
use schemars::JsonSchema;

// Required for ReloadableService trait and GenericValidator
use anyhow::{Context, Result};

// ============================================================================
// Default configuration constants
// ============================================================================

/// Default Redis host address
pub const DEFAULT_REDIS_HOST: &str = "127.0.0.1";

/// Default Redis port
pub const DEFAULT_REDIS_PORT: u16 = 6379;

/// Default Redis connection URL
pub const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";

/// Default API bind host (listen on all interfaces)
pub const DEFAULT_API_HOST: &str = "0.0.0.0";

/// Localhost address for testing
pub const LOCALHOST_HOST: &str = "127.0.0.1";

// ============================================================================
// Service URL constants
// ============================================================================

/// Default comsrv service URL (localhost)
pub const DEFAULT_COMSRV_URL: &str = "http://localhost:6001";

/// Default modsrv service URL (localhost)
pub const DEFAULT_MODSRV_URL: &str = "http://localhost:6002";

/// Default rules service URL (localhost, merged into modsrv)
pub const DEFAULT_RULES_URL: &str = "http://localhost:6002";

/// Environment variable name for comsrv URL
pub const ENV_COMSRV_URL: &str = "COMSRV_URL";

/// Environment variable name for modsrv URL
pub const ENV_MODSRV_URL: &str = "MODSRV_URL";

/// Environment variable name for rules URL
pub const ENV_RULES_URL: &str = "RULES_URL";

// ============================================================================
// Timeout configuration constants
// ============================================================================

/// Timeout configuration constants for network operations and retry strategies
pub mod timeouts {
    use std::time::Duration;

    // ============ Connection Timeout ============
    /// Default connection timeout in milliseconds
    pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 5000;
    /// Default connection timeout as Duration
    pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS);

    // ============ Request/Read Timeout ============
    /// Default read/request timeout in milliseconds
    pub const DEFAULT_READ_TIMEOUT_MS: u64 = 3000;
    /// Default read/request timeout as Duration
    pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_millis(DEFAULT_READ_TIMEOUT_MS);

    // ============ Reconnection Strategy ============
    /// Minimum reconnection delay in milliseconds (exponential backoff start)
    pub const MIN_RECONNECT_DELAY_MS: u64 = 1000;
    /// Maximum reconnection delay in milliseconds (exponential backoff cap)
    pub const MAX_RECONNECT_DELAY_MS: u64 = 30000;
    /// Cooldown period after max consecutive failures in milliseconds
    pub const RECONNECT_COOLDOWN_MS: u64 = 60000;

    /// Minimum reconnection delay as Duration
    pub const MIN_RECONNECT_DELAY: Duration = Duration::from_millis(MIN_RECONNECT_DELAY_MS);
    /// Maximum reconnection delay as Duration
    pub const MAX_RECONNECT_DELAY: Duration = Duration::from_millis(MAX_RECONNECT_DELAY_MS);
    /// Reconnection cooldown as Duration
    pub const RECONNECT_COOLDOWN: Duration = Duration::from_millis(RECONNECT_COOLDOWN_MS);

    // ============ Task/System Timeouts ============
    /// Default shutdown timeout in milliseconds (graceful shutdown)
    pub const SHUTDOWN_TIMEOUT_MS: u64 = 5000;
    /// Default shutdown timeout as Duration
    pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(SHUTDOWN_TIMEOUT_MS);

    /// Default test timeout in milliseconds (unit tests)
    pub const TEST_TIMEOUT_MS: u64 = 1000;
    /// Default test timeout as Duration
    pub const TEST_TIMEOUT: Duration = Duration::from_millis(TEST_TIMEOUT_MS);
}

// ============================================================================
// Redis routing keys (for cross-service data routing)
// ============================================================================

/// Redis routing keys for data flow between services
///
/// These keys are used for routing data between communication service (comsrv)
/// and model calculation service (modsrv). They enable bidirectional data flow:
/// - Forward: measurements from devices → model calculations (c2m)
/// - Reverse: control actions from models → devices (m2c)
pub struct RedisRoutingKeys;

impl RedisRoutingKeys {
    /// Channel to Model routing table: "route:c2m"
    /// Maps comsrv channel keys to modsrv instance keys for measurements/signals
    pub const CHANNEL_TO_MODEL: &'static str = "route:c2m";

    /// Model to Channel routing table: "route:m2c"
    /// Maps modsrv action keys to comsrv channel keys for control/adjustment commands
    pub const MODEL_TO_CHANNEL: &'static str = "route:m2c";
}

// ============================================================================
// Base service configuration
// ============================================================================

/// Base service configuration shared by all services
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct BaseServiceConfig {
    /// Service name
    #[serde(default = "default_service_name")]
    pub name: String,

    /// Service version
    pub version: Option<String>,

    /// Service description
    pub description: Option<String>,
}

impl Default for BaseServiceConfig {
    fn default() -> Self {
        Self {
            name: default_service_name(),
            version: None,
            description: None,
        }
    }
}

// ============================================================================
// API configuration
// ============================================================================

/// API server configuration
///
/// Note: port field has no default value - each service must set its own default port
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ApiConfig {
    /// Listen host address
    #[serde(default = "default_api_host")]
    pub host: String,

    /// Listen port (no default - set by service-specific config)
    pub port: u16,
}

// ============================================================================
// Redis configuration
// ============================================================================

/// Redis connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RedisConfig {
    /// Redis connection URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Whether Redis is enabled
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

// ============================================================================
// Logging configuration
// ============================================================================

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log directory
    #[serde(default = "default_log_dir")]
    pub dir: String,

    /// Log file prefix
    pub file_prefix: Option<String>,

    /// Log rotation configuration
    #[serde(default)]
    pub rotation: Option<LogRotationConfig>,
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct LogRotationConfig {
    /// Rotation strategy (daily, size, never)
    #[serde(default = "default_rotation_strategy")]
    pub strategy: String,

    /// Maximum file size in MB (for size-based rotation)
    #[serde(default = "default_max_size_mb")]
    pub max_size_mb: u64,

    /// Number of log files to retain
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

// ============================================================================
// Default value functions
// ============================================================================

fn default_service_name() -> String {
    "unnamed_service".to_string()
}

fn default_api_host() -> String {
    DEFAULT_API_HOST.to_string()
}

fn default_redis_url() -> String {
    env::var("REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS_URL.to_string())
}

fn default_log_level() -> String {
    env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

fn default_log_dir() -> String {
    "logs".to_string()
}

fn default_rotation_strategy() -> String {
    "daily".to_string()
}

fn default_max_size_mb() -> u64 {
    100
}

fn default_max_files() -> u32 {
    7
}

/// Serde default for boolean true
pub fn bool_true() -> bool {
    true
}

// ============================================================================
// Default implementations
// ============================================================================

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: 0, // Placeholder - services should provide their own default port
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            enabled: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            dir: default_log_dir(),
            file_prefix: None,
            rotation: None,
        }
    }
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            strategy: default_rotation_strategy(),
            max_size_mb: default_max_size_mb(),
            max_files: default_max_files(),
        }
    }
}

// ============================================================================
// Database Schema Definitions (Shared across services)
// ============================================================================

/// Service configuration table record
/// Supports both global and service-specific configuration with composite primary key
#[allow(dead_code)]
#[derive(Schema)]
#[table(name = "service_config")]
pub struct ServiceConfigRecord {
    #[column(not_null, primary_key)]
    pub service_name: String,

    #[column(not_null, primary_key)]
    pub key: String,

    #[column(not_null)]
    pub value: String,

    #[column(default = "string")]
    pub r#type: String,

    pub description: Option<String>,

    #[column(default = "CURRENT_TIMESTAMP")]
    pub updated_at: String, // TIMESTAMP type
}

/// Sync metadata table record
/// Tracks configuration synchronization status
#[allow(dead_code)]
#[derive(Schema)]
#[table(name = "sync_metadata")]
pub struct SyncMetadataRecord {
    #[column(primary_key)]
    pub service: String,

    #[column(not_null)]
    pub last_sync: String, // TIMESTAMP type

    pub version: Option<String>,
}

/// Service configuration table SQL (generated by Schema macro)
pub const SERVICE_CONFIG_TABLE: &str = ServiceConfigRecord::CREATE_TABLE_SQL;

/// Sync metadata table SQL (generated by Schema macro)
pub const SYNC_METADATA_TABLE: &str = SyncMetadataRecord::CREATE_TABLE_SQL;

// ============================================================================
// Core Validation Framework
// ============================================================================

/// Validation result with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub level: ValidationLevel,
}

impl ValidationResult {
    pub fn new(level: ValidationLevel) -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            level,
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.is_valid = false;
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        if !other.is_valid {
            self.is_valid = false;
        }
    }
}

/// Validation levels for different stages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationLevel {
    /// YAML/CSV syntax validation (Monarch only)
    Syntax,
    /// Schema and required fields validation (Monarch only)
    Schema,
    /// Business rules validation (Monarch and services)
    Business,
    /// Runtime environment validation (Services only)
    Runtime,
}

/// Core trait for configuration validation
pub trait ConfigValidator: Send + Sync {
    /// Validate syntax (YAML/CSV format)
    fn validate_syntax(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Syntax);
        result.add_warning("Syntax validation not implemented for this config type".to_string());
        Ok(result)
    }

    /// Validate schema (required fields, types)
    fn validate_schema(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Schema);
        result.add_warning("Schema validation not implemented for this config type".to_string());
        Ok(result)
    }

    /// Validate business rules
    fn validate_business(&self) -> Result<ValidationResult>;

    /// Validate runtime environment
    fn validate_runtime(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Runtime);
        result.add_warning(
            "Runtime validation not applicable for configuration management".to_string(),
        );
        Ok(result)
    }

    /// Perform full validation up to specified level
    fn validate(&self, up_to_level: ValidationLevel) -> Result<ValidationResult> {
        let mut combined = ValidationResult::new(up_to_level);

        if up_to_level as u8 >= ValidationLevel::Syntax as u8 {
            combined.merge(self.validate_syntax()?);
        }

        if up_to_level as u8 >= ValidationLevel::Schema as u8 {
            combined.merge(self.validate_schema()?);
        }

        if up_to_level as u8 >= ValidationLevel::Business as u8 {
            combined.merge(self.validate_business()?);
        }

        if up_to_level as u8 >= ValidationLevel::Runtime as u8 {
            combined.merge(self.validate_runtime()?);
        }

        Ok(combined)
    }
}

// ============================================================================
// Generic Validator
// ============================================================================

/// Generic configuration validator that works with any config type
///
/// This eliminates the need for separate validator implementations for each service.
/// Instead of defining ComsrvValidator, ModsrvValidator, and RulesValidator separately,
/// use type aliases:
///
/// ```ignore
/// pub type ComsrvValidator = GenericValidator<ComsrvConfig>;
/// pub type ModsrvValidator = GenericValidator<ModsrvConfig>;
/// pub type RulesValidator = GenericValidator<RulesConfig>;
/// ```
pub struct GenericValidator<T> {
    config: Option<T>,
    raw_yaml: Option<serde_yaml::Value>,
}

impl<T: DeserializeOwned + ConfigValidator> GenericValidator<T> {
    /// Create validator from YAML value
    pub fn from_yaml(yaml: serde_yaml::Value) -> Self {
        let config = serde_yaml::from_value(yaml.clone()).ok();
        Self {
            config,
            raw_yaml: Some(yaml),
        }
    }

    /// Create validator from already-parsed config
    pub fn from_config(config: T) -> Self {
        Self {
            config: Some(config),
            raw_yaml: None,
        }
    }

    /// Create validator from file path
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Deserialize directly from string to capture line/column information
        let config = serde_yaml::from_str::<T>(&content).map_err(|e| {
            if let Some(location) = e.location() {
                anyhow::anyhow!(
                    "Configuration error in {}:{}:{}\n  {}",
                    path.display(),
                    location.line(),
                    location.column(),
                    e
                )
            } else {
                anyhow::anyhow!("Configuration error in {}\n  {}", path.display(), e)
            }
        })?;

        // Also parse as YAML Value for raw_yaml field
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

        Ok(Self {
            config: Some(config),
            raw_yaml: Some(yaml),
        })
    }

    /// Get reference to the parsed config
    pub fn config(&self) -> Option<&T> {
        self.config.as_ref()
    }

    /// Take ownership of the parsed config
    pub fn into_config(self) -> Option<T> {
        self.config
    }
}

impl<T: DeserializeOwned + ConfigValidator> ConfigValidator for GenericValidator<T> {
    fn validate_syntax(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Syntax);

        if self.config.is_none() {
            if let Some(yaml) = &self.raw_yaml {
                match serde_yaml::from_value::<T>(yaml.clone()) {
                    Ok(_) => {
                        result.add_warning("Configuration parsed but not stored".to_string());
                    },
                    Err(e) => {
                        result.add_error(format!("Invalid YAML syntax: {}", e));
                    },
                }
            } else {
                result.add_error("No configuration data available".to_string());
            }
        }

        Ok(result)
    }

    fn validate_schema(&self) -> Result<ValidationResult> {
        match &self.config {
            Some(config) => config.validate_schema(),
            None => {
                let mut result = ValidationResult::new(ValidationLevel::Schema);
                result.add_error("Configuration parsing failed".to_string());
                Ok(result)
            },
        }
    }

    fn validate_business(&self) -> Result<ValidationResult> {
        match &self.config {
            Some(config) => config.validate_business(),
            None => {
                let mut result = ValidationResult::new(ValidationLevel::Business);
                result.add_error("Configuration not available".to_string());
                Ok(result)
            },
        }
    }

    fn validate_runtime(&self) -> Result<ValidationResult> {
        match &self.config {
            Some(config) => config.validate_runtime(),
            None => {
                let mut result = ValidationResult::new(ValidationLevel::Runtime);
                result.add_error("Configuration not available".to_string());
                Ok(result)
            },
        }
    }
}

// ============================================================================
// Hot Reload Infrastructure
// ============================================================================

/// Generic reload result for all services
///
/// Provides unified response format for hot reload operations across
/// comsrv, modsrv, and rules services.
///
/// # Type Parameters
/// - `I`: Item identifier type (e.g., `u16` for channel/instance ID, `String` for rule ID)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ReloadResult<I> {
    /// Total number of configuration items in database
    pub total_count: usize,

    /// IDs of newly added items
    pub added: Vec<I>,

    /// IDs of updated items (hot-reloaded)
    pub updated: Vec<I>,

    /// IDs of removed items
    pub removed: Vec<I>,

    /// Error messages (one per failed operation)
    /// Format: "{item_id}: {error_message}"
    pub errors: Vec<String>,

    /// Total reload operation duration in milliseconds
    pub duration_ms: u64,
}

impl<I> ReloadResult<I> {
    /// Check if reload completed without errors
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get total number of successful operations
    pub fn success_count(&self) -> usize {
        self.added.len() + self.updated.len() + self.removed.len()
    }

    /// Get total number of failed operations
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}

impl<I> Default for ReloadResult<I> {
    fn default() -> Self {
        Self {
            total_count: 0,
            added: Vec::new(),
            updated: Vec::new(),
            removed: Vec::new(),
            errors: Vec::new(),
            duration_ms: 0,
        }
    }
}

/// Type alias for channel reload result (comsrv)
pub type ChannelReloadResult = ReloadResult<u32>;

/// Type alias for instance reload result (modsrv)
pub type InstanceReloadResult = ReloadResult<u32>;

/// Type alias for rule reload result (rules)
pub type RuleReloadResult = ReloadResult<String>;

/// Unified hot reload interface for all services
///
/// This trait provides a consistent API for reloading service configurations
/// from SQLite database without restarting the service.
#[allow(async_fn_in_trait)]
pub trait ReloadableService {
    /// Change severity type (e.g., MetadataOnly < NonCritical < Critical)
    type ChangeType: PartialOrd + Eq + Copy;

    /// Configuration item type
    type Config: Clone + Serialize + for<'de> Deserialize<'de>;

    /// Reload operation result type
    type ReloadResult: Serialize + for<'de> Deserialize<'de>;

    /// Reload all configurations from SQLite database
    async fn reload_from_database(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> anyhow::Result<Self::ReloadResult>;

    /// Analyze configuration change severity
    fn analyze_changes(
        &self,
        old_config: &Self::Config,
        new_config: &Self::Config,
    ) -> Self::ChangeType;

    /// Perform hot reload with automatic rollback on failure
    async fn perform_hot_reload(&self, config: Self::Config) -> anyhow::Result<String>;

    /// Rollback to previous configuration
    async fn rollback(&self, previous_config: Self::Config) -> anyhow::Result<String>;
}

/// Helper validation functions
pub mod helpers {
    use super::*;

    /// Validate port number range
    pub fn validate_port(port: u16, service: &str) -> Result<()> {
        if port < 1024 {
            return Err(anyhow::anyhow!(
                "{} port {} is in privileged range (< 1024)",
                service,
                port
            ));
        }
        Ok(())
    }

    /// Validate IP address format
    pub fn validate_ip(ip: &str) -> Result<()> {
        use std::net::IpAddr;
        ip.parse::<IpAddr>()
            .map_err(|_| anyhow::anyhow!("Invalid IP address: {}", ip))?;
        Ok(())
    }

    /// Check if a port is available for binding
    pub fn check_port_available(port: u16) -> Result<()> {
        use std::net::TcpListener;

        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Port {} is not available: {}", port, e)),
        }
    }

    /// Test Redis connection
    pub async fn test_redis_connection(url: &str) -> Result<()> {
        use redis::aio::MultiplexedConnection;
        use redis::cmd;

        let client =
            redis::Client::open(url).map_err(|e| anyhow::anyhow!("Invalid Redis URL: {}", e))?;

        let mut con: MultiplexedConnection = client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to Redis: {}", e))?;

        let _: String = cmd("PING")
            .query_async(&mut con)
            .await
            .map_err(|e| anyhow::anyhow!("Redis ping failed: {}", e))?;

        Ok(())
    }

    /// Check database file accessibility
    pub fn check_database_access(db_path: &std::path::Path) -> Result<()> {
        if !db_path.exists() {
            return Err(anyhow::anyhow!(
                "Database file not found: {}",
                db_path.display()
            ));
        }

        let metadata = std::fs::metadata(db_path)?;
        if metadata.permissions().readonly() {
            return Err(anyhow::anyhow!(
                "Database file is read-only: {}",
                db_path.display()
            ));
        }

        Ok(())
    }
}

// ============================================================================
// Validation implementations for common configs
// ============================================================================

impl BaseServiceConfig {
    /// Validate base service configuration
    pub fn validate(&self, result: &mut ValidationResult) {
        if self.name.is_empty() {
            result.add_error("Service name cannot be empty".to_string());
        }
    }
}

impl ApiConfig {
    /// Validate API configuration
    pub fn validate(&self, result: &mut ValidationResult) {
        // Port validation
        if self.port == 0 {
            result.add_error("API port cannot be 0".to_string());
        } else if self.port < 1024 {
            result.add_warning(format!(
                "API port {} is in system range (< 1024)",
                self.port
            ));
        }

        // Host validation
        if self.host.is_empty() {
            result.add_error("API host cannot be empty".to_string());
        }
    }

    /// Validate port availability (runtime check)
    pub fn validate_runtime(&self, result: &mut ValidationResult) {
        if let Err(e) = helpers::check_port_available(self.port) {
            result.add_error(format!("Port {} not available: {}", self.port, e));
        }
    }
}

impl RedisConfig {
    /// Validate Redis configuration
    pub fn validate(&self, result: &mut ValidationResult) {
        if self.url.is_empty() {
            result.add_error("Redis URL cannot be empty".to_string());
        } else if !self.url.starts_with("redis://") && !self.url.starts_with("rediss://") {
            result.add_warning("Redis URL should start with redis:// or rediss://".to_string());
        }
    }

    /// Validate Redis connectivity (runtime check)
    pub async fn validate_runtime(&self, result: &mut ValidationResult) {
        if self.enabled {
            if let Err(e) = helpers::test_redis_connection(&self.url).await {
                result.add_error(format!("Redis connection failed: {}", e));
            }
        }
    }
}

impl LoggingConfig {
    /// Validate logging configuration
    pub fn validate(&self, result: &mut ValidationResult) {
        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.level.as_str()) {
            result.add_warning(format!("Unrecognized log level: {}", self.level));
        }

        // Validate log directory (will be created if doesn't exist, so just warn)
        if self.dir.is_empty() {
            result.add_error("Log directory cannot be empty".to_string());
        }

        // Validate rotation config if present
        if let Some(rotation) = &self.rotation {
            rotation.validate(result);
        }
    }
}

impl LogRotationConfig {
    /// Validate log rotation configuration
    pub fn validate(&self, result: &mut ValidationResult) {
        let valid_strategies = ["daily", "size", "never"];
        if !valid_strategies.contains(&self.strategy.as_str()) {
            result.add_error(format!("Invalid rotation strategy: {}", self.strategy));
        }

        if self.strategy == "size" && self.max_size_mb == 0 {
            result.add_error("Max size for size-based rotation cannot be 0".to_string());
        }

        if self.max_files == 0 {
            result.add_warning(
                "Max files is 0, log rotation will delete old logs immediately".to_string(),
            );
        }
    }
}

// ============================================================================
// Shared enum types
// ============================================================================

// Re-export PointRole from voltage-model for backward compatibility
pub use voltage_model::PointRole;

/// Instance status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum InstanceStatus {
    /// Instance is running normally
    Running,
    /// Instance is stopped
    Stopped,
    /// Instance has encountered an error
    Error,
    /// Instance is in warning state
    Warning,
    /// Instance is disconnected
    Disconnected,
}

impl InstanceStatus {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Disconnected => "disconnected",
        }
    }

    /// Check if instance is healthy (running or warning)
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Running | Self::Warning)
    }
}

impl FromStr for InstanceStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "running" | "run" | "active" => Ok(Self::Running),
            "stopped" | "stop" | "inactive" => Ok(Self::Stopped),
            "error" | "err" | "failed" => Ok(Self::Error),
            "warning" | "warn" => Ok(Self::Warning),
            "disconnected" | "offline" => Ok(Self::Disconnected),
            _ => Err(format!("Unknown instance status: {}", s)),
        }
    }
}

impl fmt::Display for InstanceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for InstanceStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

/// API Response status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ResponseStatus {
    /// Request succeeded
    Success,
    /// Request failed with error
    Error,
    /// Request is pending/processing
    Pending,
    /// Request partially succeeded
    Partial,
    /// Request timed out
    Timeout,
}

impl ResponseStatus {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Pending => "pending",
            Self::Partial => "partial",
            Self::Timeout => "timeout",
        }
    }

    /// Check if response indicates success (success or partial)
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Success | Self::Partial)
    }

    /// Check if response indicates failure (error or timeout)
    pub fn is_err(&self) -> bool {
        matches!(self, Self::Error | Self::Timeout)
    }
}

impl FromStr for ResponseStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "success" | "ok" | "succeeded" => Ok(Self::Success),
            "error" | "err" | "failed" => Ok(Self::Error),
            "pending" | "processing" | "running" => Ok(Self::Pending),
            "partial" | "incomplete" => Ok(Self::Partial),
            "timeout" | "timed_out" => Ok(Self::Timeout),
            _ => Err(format!("Unknown response status: {}", s)),
        }
    }
}

impl fmt::Display for ResponseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for ResponseStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Comparison operator for rules engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ComparisonOperator {
    /// Equal to (==)
    #[serde(rename = "eq")]
    Equal,
    /// Not equal to (!=)
    #[serde(rename = "ne")]
    NotEqual,
    /// Greater than (>)
    #[serde(rename = "gt")]
    GreaterThan,
    /// Greater than or equal to (>=)
    #[serde(rename = "gte")]
    GreaterThanOrEqual,
    /// Less than (<)
    #[serde(rename = "lt")]
    LessThan,
    /// Less than or equal to (<=)
    #[serde(rename = "lte")]
    LessThanOrEqual,
    /// Value is within range (inclusive)
    #[serde(rename = "in")]
    InRange,
    /// Value is outside range (exclusive)
    #[serde(rename = "not_in")]
    NotInRange,
    /// String contains substring
    #[serde(rename = "contains")]
    Contains,
    /// String matches regex pattern
    #[serde(rename = "matches")]
    Matches,
}

impl ComparisonOperator {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Equal => "eq",
            Self::NotEqual => "ne",
            Self::GreaterThan => "gt",
            Self::GreaterThanOrEqual => "gte",
            Self::LessThan => "lt",
            Self::LessThanOrEqual => "lte",
            Self::InRange => "in",
            Self::NotInRange => "not_in",
            Self::Contains => "contains",
            Self::Matches => "matches",
        }
    }

    /// Get symbol representation
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::InRange => "∈",
            Self::NotInRange => "∉",
            Self::Contains => "⊃",
            Self::Matches => "~",
        }
    }

    /// Compare two f64 values
    pub fn compare_f64(&self, left: f64, right: f64) -> bool {
        match self {
            Self::Equal => (left - right).abs() < f64::EPSILON,
            Self::NotEqual => (left - right).abs() >= f64::EPSILON,
            Self::GreaterThan => left > right,
            Self::GreaterThanOrEqual => left >= right,
            Self::LessThan => left < right,
            Self::LessThanOrEqual => left <= right,
            _ => false, // InRange and NotInRange need special handling
        }
    }

    /// Compare two i64 values
    pub fn compare_i64(&self, left: i64, right: i64) -> bool {
        match self {
            Self::Equal => left == right,
            Self::NotEqual => left != right,
            Self::GreaterThan => left > right,
            Self::GreaterThanOrEqual => left >= right,
            Self::LessThan => left < right,
            Self::LessThanOrEqual => left <= right,
            _ => false, // InRange and NotInRange need special handling
        }
    }
}

impl FromStr for ComparisonOperator {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "eq" | "==" | "=" | "equal" => Ok(Self::Equal),
            "ne" | "!=" | "<>" | "not_equal" => Ok(Self::NotEqual),
            "gt" | ">" | "greater" => Ok(Self::GreaterThan),
            "gte" | ">=" | "greater_equal" => Ok(Self::GreaterThanOrEqual),
            "lt" | "<" | "less" => Ok(Self::LessThan),
            "lte" | "<=" | "less_equal" => Ok(Self::LessThanOrEqual),
            "in" | "within" | "between" => Ok(Self::InRange),
            "not_in" | "outside" | "not_between" => Ok(Self::NotInRange),
            "contains" | "has" | "includes" => Ok(Self::Contains),
            "matches" | "~" | "regex" => Ok(Self::Matches),
            _ => Err(format!("Unknown comparison operator: {}", s)),
        }
    }
}

impl fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

impl Default for ComparisonOperator {
    fn default() -> Self {
        Self::Equal
    }
}

/// FourRemote is an alias for PointType for backward compatibility
///
/// Both represent the same concept: the four remote point types (T/S/C/A)
/// in industrial SCADA systems.
///
/// **Prefer using `PointType` for new code.**
pub type FourRemote = PointType;

/// Helper to convert database string to FourRemote/PointType
pub fn parse_four_remote(s: &str) -> Result<PointType, String> {
    s.parse()
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_point_role_serialization() {
        let role = PointRole::Measurement;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"M\"");

        let role = PointRole::Action;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"A\"");

        let role: PointRole = serde_json::from_str("\"M\"").unwrap();
        assert_eq!(role, PointRole::Measurement);
    }

    #[test]
    fn test_point_role_from_str() {
        assert_eq!(PointRole::from_str("M").unwrap(), PointRole::Measurement);
        assert_eq!(PointRole::from_str("A").unwrap(), PointRole::Action);
        assert_eq!(
            PointRole::from_str("measurement").unwrap(),
            PointRole::Measurement
        );
        assert!(PointRole::from_str("X").is_err());
    }

    #[test]
    fn test_instance_status_methods() {
        assert!(InstanceStatus::Running.is_healthy());
        assert!(InstanceStatus::Warning.is_healthy());
        assert!(!InstanceStatus::Stopped.is_healthy());
        assert!(!InstanceStatus::Error.is_healthy());
    }

    #[test]
    fn test_comparison_operator_compare_methods() {
        let op = ComparisonOperator::GreaterThan;
        assert!(op.compare_f64(5.0, 3.0));
        assert!(!op.compare_f64(3.0, 5.0));

        let op = ComparisonOperator::Equal;
        assert!(op.compare_i64(42, 42));
        assert!(!op.compare_i64(42, 43));
    }

    #[test]
    fn test_four_remote_is_point_type() {
        let fr: FourRemote = FourRemote::Telemetry;
        let pt: PointType = fr;
        assert_eq!(pt, PointType::Telemetry);
    }
}
