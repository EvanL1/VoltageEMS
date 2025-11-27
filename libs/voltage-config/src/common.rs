//! Common configuration structures shared across all services

use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::str::FromStr;

#[cfg(feature = "schema")]
use schemars::JsonSchema;

// Required for ReloadableService trait
use anyhow::Result;

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

/// Default rulesrv service URL (localhost, merged into modsrv)
pub const DEFAULT_RULESRV_URL: &str = "http://localhost:6002";

/// Environment variable name for comsrv URL
pub const ENV_COMSRV_URL: &str = "COMSRV_URL";

/// Environment variable name for modsrv URL
pub const ENV_MODSRV_URL: &str = "MODSRV_URL";

/// Environment variable name for rulesrv URL
pub const ENV_RULESRV_URL: &str = "RULESRV_URL";

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
    #[serde(default = "default_true")]
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

fn default_true() -> bool {
    true
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
// Core Validation Framework (moved from validation.rs)
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
// Hot Reload Infrastructure
// ============================================================================

/// Generic reload result for all services
///
/// Provides unified response format for hot reload operations across
/// comsrv, modsrv, and rulesrv services.
///
/// # Type Parameters
/// - `I`: Item identifier type (e.g., `u16` for channel/instance ID, `String` for rule ID)
///
/// # Examples
/// ```
/// use voltage_config::common::ReloadResult;
///
/// let result: ReloadResult<u16> = ReloadResult {
///     total_count: 10,
///     added: vec![1001, 1002],
///     updated: vec![1003],
///     removed: vec![1004],
///     errors: vec![],
///     duration_ms: 150,
/// };
/// ```
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
pub type ChannelReloadResult = ReloadResult<u16>;

/// Type alias for instance reload result (modsrv)
pub type InstanceReloadResult = ReloadResult<u16>;

/// Type alias for rule reload result (rulesrv)
pub type RuleReloadResult = ReloadResult<String>;

/// Unified hot reload interface for all services
///
/// This trait provides a consistent API for reloading service configurations
/// from SQLite database without restarting the service.
///
/// # Design Principles
/// 1. **Incremental Updates**: Only modified items are reloaded
/// 2. **Intelligent Change Detection**: Different severity levels (Metadata/NonCritical/Critical)
/// 3. **Automatic Rollback**: Failed reloads restore previous configuration
/// 4. **Zero Downtime**: Services remain operational during reload
///
/// # Type Parameters
/// - `ChangeType`: Enum representing change severity (must be comparable)
/// - `Config`: Configuration type (must be cloneable and serializable)
/// - `ReloadResult`: Result type for reload operations
///
/// # Examples
/// ```ignore
/// // comsrv implementation
/// impl ReloadableService for ChannelManager {
///     type ChangeType = ChannelChangeType;
///     type Config = ChannelConfig;
///     type ReloadResult = ChannelReloadResult;
///
///     async fn reload_from_database(&self, pool: &SqlitePool) -> Result<Self::ReloadResult> {
///         // Load from database and sync runtime state
///     }
/// }
/// ```
#[allow(async_fn_in_trait)]
pub trait ReloadableService {
    /// Change severity type (e.g., MetadataOnly < NonCritical < Critical)
    type ChangeType: PartialOrd + Eq + Copy;

    /// Configuration item type
    type Config: Clone + Serialize + for<'de> Deserialize<'de>;

    /// Reload operation result type
    type ReloadResult: Serialize + for<'de> Deserialize<'de>;

    /// Reload all configurations from SQLite database
    ///
    /// This method performs incremental synchronization:
    /// 1. Load configurations from SQLite
    /// 2. Compare with runtime state
    /// 3. Add new items
    /// 4. Update changed items (with hot reload)
    /// 5. Remove deleted items
    ///
    /// # Arguments
    /// * `pool` - SQLite connection pool
    ///
    /// # Returns
    /// Result containing reload statistics and errors
    ///
    /// # Errors
    /// - Database connection failures
    /// - Configuration parsing errors
    /// - Validation errors
    ///
    /// # Side Effects
    /// - Modifies runtime service state to match database
    /// - May restart communication channels/instances/rules
    async fn reload_from_database(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> anyhow::Result<Self::ReloadResult>;

    /// Analyze configuration change severity
    ///
    /// Determines whether a configuration change requires:
    /// - **MetadataOnly**: No restart (name/description changes)
    /// - **NonCritical**: Optional restart (timeout/retry changes)
    /// - **Critical**: Mandatory restart (connection parameters change)
    ///
    /// # Arguments
    /// * `old_config` - Previous configuration
    /// * `new_config` - New configuration
    ///
    /// # Returns
    /// Change severity level
    fn analyze_changes(
        &self,
        old_config: &Self::Config,
        new_config: &Self::Config,
    ) -> Self::ChangeType;

    /// Perform hot reload with automatic rollback on failure
    ///
    /// Execution flow:
    /// 1. Save current configuration (for rollback)
    /// 2. Stop old runtime instance
    /// 3. Start new runtime instance with new_config
    /// 4. If step 3 fails → rollback to saved configuration
    ///
    /// # Arguments
    /// * `config` - New configuration to apply
    ///
    /// # Returns
    /// Success message or error
    ///
    /// # Errors
    /// - Configuration validation errors
    /// - Resource initialization errors (network, database)
    /// - Rollback failures (critical - service may be in inconsistent state)
    ///
    /// # Safety
    /// This operation is designed to be atomic - either fully succeeds or
    /// fully rolls back. However, rollback failure is a critical error.
    async fn perform_hot_reload(&self, config: Self::Config) -> anyhow::Result<String>;

    /// Rollback to previous configuration
    ///
    /// Used internally by `perform_hot_reload` when reload fails.
    /// Can also be called explicitly for manual recovery.
    ///
    /// # Arguments
    /// * `previous_config` - Last known good configuration
    ///
    /// # Returns
    /// Confirmation message or error
    ///
    /// # Errors
    /// - Configuration restoration errors
    /// - Resource re-initialization errors
    ///
    /// # Warning
    /// Rollback failure is a critical error that may leave the service
    /// in an inconsistent state. Manual intervention may be required.
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

/// Protocol types for communication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ProtocolType {
    /// Modbus TCP protocol
    ModbusTcp,
    /// Modbus RTU protocol
    ModbusRtu,
    /// CAN bus protocol
    Can,
    /// Virtual/simulated protocol
    Virtual,
}

impl ProtocolType {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ModbusTcp => "modbus_tcp",
            Self::ModbusRtu => "modbus_rtu",
            Self::Can => "can",
            Self::Virtual => "virtual",
        }
    }

    /// Check if this is a Modbus protocol variant
    pub fn is_modbus(&self) -> bool {
        matches!(self, Self::ModbusTcp | Self::ModbusRtu)
    }
}

impl FromStr for ProtocolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "modbus_tcp" | "modbus-tcp" | "modbustcp" => Ok(Self::ModbusTcp),
            "modbus_rtu" | "modbus-rtu" | "modbusrtu" => Ok(Self::ModbusRtu),
            "can" => Ok(Self::Can),
            "virtual" | "virt" => Ok(Self::Virtual),
            _ => Err(format!("Unknown protocol type: {}", s)),
        }
    }
}

impl fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for ProtocolType {
    fn default() -> Self {
        Self::Virtual
    }
}

/// Point role types (Measurement/Action)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum PointRole {
    /// Measurement point (M)
    #[serde(rename = "M")]
    Measurement,
    /// Action point (A)
    #[serde(rename = "A")]
    Action,
}

impl PointRole {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Measurement => "M",
            Self::Action => "A",
        }
    }
}

impl FromStr for PointRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "M" | "MEASUREMENT" => Ok(Self::Measurement),
            "A" | "ACTION" => Ok(Self::Action),
            _ => Err(format!("Unknown point role: {}", s)),
        }
    }
}

impl fmt::Display for PointRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for PointRole {
    fn default() -> Self {
        Self::Measurement
    }
}

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

/// Four-Remote enumeration
///
/// T - Telemetry
/// S - Signal
/// C - Control
/// A - Adjustment
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum FourRemote {
    /// Telemetry - measurements like voltage, current, power
    #[serde(rename = "T", alias = "YC", alias = "yc", alias = "telemetry")]
    Telemetry,

    /// Signal - status indicators like on/off, open/closed
    #[serde(rename = "S", alias = "YX", alias = "yx", alias = "signal")]
    Signal,

    /// Control - commands to control devices
    #[serde(rename = "C", alias = "YK", alias = "yk", alias = "control")]
    Control,

    /// Adjustment - setpoint values for remote adjustment
    #[serde(
        rename = "A",
        alias = "YT",
        alias = "yt",
        alias = "adjustment",
        alias = "setpoint"
    )]
    Adjustment,
}

impl FourRemote {
    /// Get the single-character code for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Telemetry => "T",
            Self::Signal => "S",
            Self::Control => "C",
            Self::Adjustment => "A",
        }
    }

    /// Check if this is an input type (T or S)
    pub fn is_input(&self) -> bool {
        matches!(self, Self::Telemetry | Self::Signal)
    }

    /// Check if this is an output type (C or A)
    pub fn is_output(&self) -> bool {
        matches!(self, Self::Control | Self::Adjustment)
    }
}

impl fmt::Display for FourRemote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for FourRemote {
    fn default() -> Self {
        Self::Telemetry
    }
}

impl FromStr for FourRemote {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let u = s.to_uppercase();
        match u.as_str() {
            "T" | "YC" => Ok(Self::Telemetry),
            "S" | "YX" => Ok(Self::Signal),
            "C" | "YK" => Ok(Self::Control),
            "A" | "YT" => Ok(Self::Adjustment),
            _ => Err(format!(
                "Invalid four-remote type: {}. Must be one of T/S/C/A or YC/YX/YK/YT",
                s
            )),
        }
    }
}

/// Helper to convert database string to FourRemote
pub fn parse_four_remote(s: &str) -> Result<FourRemote, String> {
    s.parse()
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    // ============ ProtocolType Tests ============
    #[test]
    fn test_protocol_type_serialization() {
        // Test JSON serialization
        let protocol = ProtocolType::ModbusTcp;
        let json = serde_json::to_string(&protocol).unwrap();
        assert_eq!(json, "\"modbus_tcp\"");

        // Test deserialization
        let protocol: ProtocolType = serde_json::from_str("\"modbus_rtu\"").unwrap();
        assert_eq!(protocol, ProtocolType::ModbusRtu);

        // Test all variants
        assert_eq!(
            serde_json::to_string(&ProtocolType::Can).unwrap(),
            "\"can\""
        );
        assert_eq!(
            serde_json::to_string(&ProtocolType::Virtual).unwrap(),
            "\"virtual\""
        );
    }

    #[test]
    fn test_protocol_type_from_str() {
        // Test standard names
        assert_eq!(
            ProtocolType::from_str("modbus_tcp").unwrap(),
            ProtocolType::ModbusTcp
        );
        assert_eq!(
            ProtocolType::from_str("modbus_rtu").unwrap(),
            ProtocolType::ModbusRtu
        );
        assert_eq!(ProtocolType::from_str("can").unwrap(), ProtocolType::Can);
        assert_eq!(
            ProtocolType::from_str("virtual").unwrap(),
            ProtocolType::Virtual
        );

        // Test variations
        assert_eq!(
            ProtocolType::from_str("modbus-tcp").unwrap(),
            ProtocolType::ModbusTcp
        );
        assert_eq!(
            ProtocolType::from_str("modbustcp").unwrap(),
            ProtocolType::ModbusTcp
        );

        // Test invalid
        assert!(ProtocolType::from_str("unknown").is_err());
    }

    #[test]
    fn test_protocol_type_methods() {
        let modbus_tcp = ProtocolType::ModbusTcp;
        assert_eq!(modbus_tcp.as_str(), "modbus_tcp");
        assert!(modbus_tcp.is_modbus());

        let can = ProtocolType::Can;
        assert!(!can.is_modbus());

        // Test Display
        assert_eq!(modbus_tcp.to_string(), "modbus_tcp");

        // Test Default
        assert_eq!(ProtocolType::default(), ProtocolType::Virtual);
    }

    // ============ PointRole Tests ============
    #[test]
    fn test_point_role_serialization() {
        // Test JSON serialization with serde rename
        let role = PointRole::Measurement;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"M\"");

        let role = PointRole::Action;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"A\"");

        // Test deserialization
        let role: PointRole = serde_json::from_str("\"M\"").unwrap();
        assert_eq!(role, PointRole::Measurement);
    }

    #[test]
    fn test_point_role_from_str() {
        // Test short forms
        assert_eq!(PointRole::from_str("M").unwrap(), PointRole::Measurement);
        assert_eq!(PointRole::from_str("A").unwrap(), PointRole::Action);

        // Test long forms
        assert_eq!(
            PointRole::from_str("measurement").unwrap(),
            PointRole::Measurement
        );
        assert_eq!(PointRole::from_str("action").unwrap(), PointRole::Action);

        // Test case insensitive
        assert_eq!(PointRole::from_str("m").unwrap(), PointRole::Measurement);
        assert_eq!(
            PointRole::from_str("MEASUREMENT").unwrap(),
            PointRole::Measurement
        );

        // Test invalid
        assert!(PointRole::from_str("X").is_err());
    }

    // ============ InstanceStatus Tests ============
    #[test]
    fn test_instance_status_serialization() {
        // Test all variants
        assert_eq!(
            serde_json::to_string(&InstanceStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&InstanceStatus::Stopped).unwrap(),
            "\"stopped\""
        );
        assert_eq!(
            serde_json::to_string(&InstanceStatus::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&InstanceStatus::Warning).unwrap(),
            "\"warning\""
        );
        assert_eq!(
            serde_json::to_string(&InstanceStatus::Disconnected).unwrap(),
            "\"disconnected\""
        );

        // Test deserialization
        let status: InstanceStatus = serde_json::from_str("\"running\"").unwrap();
        assert_eq!(status, InstanceStatus::Running);
    }

    #[test]
    fn test_instance_status_methods() {
        // Test is_healthy method - Running and Warning are healthy
        assert!(InstanceStatus::Running.is_healthy());
        assert!(InstanceStatus::Warning.is_healthy()); // Warning is still healthy
        assert!(!InstanceStatus::Stopped.is_healthy());
        assert!(!InstanceStatus::Error.is_healthy());
        assert!(!InstanceStatus::Disconnected.is_healthy());

        // Test Display - returns lowercase strings
        assert_eq!(InstanceStatus::Running.to_string(), "running");
        assert_eq!(InstanceStatus::Error.to_string(), "error");
        assert_eq!(InstanceStatus::Warning.to_string(), "warning");

        // Test Default
        assert_eq!(InstanceStatus::default(), InstanceStatus::Stopped);
    }

    // ============ ResponseStatus Tests ============
    #[test]
    fn test_response_status_serialization() {
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Success).unwrap(),
            "\"success\""
        );
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Partial).unwrap(),
            "\"partial\""
        );
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Timeout).unwrap(),
            "\"timeout\""
        );
    }

    #[test]
    fn test_response_status_methods() {
        // Test is_ok method
        assert!(ResponseStatus::Success.is_ok());
        assert!(ResponseStatus::Partial.is_ok());
        assert!(!ResponseStatus::Error.is_ok());
        assert!(!ResponseStatus::Timeout.is_ok());
        assert!(!ResponseStatus::Pending.is_ok());

        // Test is_err method
        assert!(ResponseStatus::Error.is_err());
        assert!(ResponseStatus::Timeout.is_err());
        assert!(!ResponseStatus::Success.is_err());
        assert!(!ResponseStatus::Partial.is_err());
        assert!(!ResponseStatus::Pending.is_err());

        // Test Default
        assert_eq!(ResponseStatus::default(), ResponseStatus::Pending);
    }

    // ============ ComparisonOperator Tests ============
    #[test]
    fn test_comparison_operator_serialization() {
        // Test serde rename attributes
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::Equal).unwrap(),
            "\"eq\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::NotEqual).unwrap(),
            "\"ne\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::GreaterThan).unwrap(),
            "\"gt\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::GreaterThanOrEqual).unwrap(),
            "\"gte\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::LessThan).unwrap(),
            "\"lt\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::LessThanOrEqual).unwrap(),
            "\"lte\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::InRange).unwrap(),
            "\"in\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::NotInRange).unwrap(),
            "\"not_in\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::Contains).unwrap(),
            "\"contains\""
        );
        assert_eq!(
            serde_json::to_string(&ComparisonOperator::Matches).unwrap(),
            "\"matches\""
        );
    }

    #[test]
    fn test_comparison_operator_from_str() {
        // Test primary names
        assert_eq!(
            ComparisonOperator::from_str("eq").unwrap(),
            ComparisonOperator::Equal
        );
        assert_eq!(
            ComparisonOperator::from_str("gt").unwrap(),
            ComparisonOperator::GreaterThan
        );

        // Test symbol aliases
        assert_eq!(
            ComparisonOperator::from_str("==").unwrap(),
            ComparisonOperator::Equal
        );
        assert_eq!(
            ComparisonOperator::from_str("!=").unwrap(),
            ComparisonOperator::NotEqual
        );
        assert_eq!(
            ComparisonOperator::from_str(">").unwrap(),
            ComparisonOperator::GreaterThan
        );
        assert_eq!(
            ComparisonOperator::from_str(">=").unwrap(),
            ComparisonOperator::GreaterThanOrEqual
        );
        assert_eq!(
            ComparisonOperator::from_str("<").unwrap(),
            ComparisonOperator::LessThan
        );
        assert_eq!(
            ComparisonOperator::from_str("<=").unwrap(),
            ComparisonOperator::LessThanOrEqual
        );

        // Test word aliases
        assert_eq!(
            ComparisonOperator::from_str("equal").unwrap(),
            ComparisonOperator::Equal
        );
        assert_eq!(
            ComparisonOperator::from_str("contains").unwrap(),
            ComparisonOperator::Contains
        );
        assert_eq!(
            ComparisonOperator::from_str("matches").unwrap(),
            ComparisonOperator::Matches
        );
    }

    #[test]
    fn test_comparison_operator_compare_methods() {
        let op = ComparisonOperator::GreaterThan;
        assert!(op.compare_f64(5.0, 3.0));
        assert!(!op.compare_f64(3.0, 5.0));

        let op = ComparisonOperator::Equal;
        assert!(op.compare_i64(42, 42));
        assert!(!op.compare_i64(42, 43));

        // Test floating point equality - f64::EPSILON is very small
        assert!(ComparisonOperator::Equal.compare_f64(1.0, 1.0));
        // These values differ by more than f64::EPSILON, so not equal
        assert!(!ComparisonOperator::Equal.compare_f64(1.0, 1.0000000001));
        assert!(ComparisonOperator::NotEqual.compare_f64(1.0, 1.0000000001));
    }

    #[test]
    fn test_comparison_operator_symbols() {
        assert_eq!(ComparisonOperator::Equal.symbol(), "==");
        assert_eq!(ComparisonOperator::GreaterThan.symbol(), ">");
        assert_eq!(ComparisonOperator::InRange.symbol(), "∈");
        assert_eq!(ComparisonOperator::NotInRange.symbol(), "∉");
        assert_eq!(ComparisonOperator::Contains.symbol(), "⊃");
        assert_eq!(ComparisonOperator::Matches.symbol(), "~");
    }

    // ============ FourRemote Tests ============
    #[test]
    fn test_four_remote_serialization() {
        // Test serde rename to single letters
        assert_eq!(
            serde_json::to_string(&FourRemote::Telemetry).unwrap(),
            "\"T\""
        );
        assert_eq!(serde_json::to_string(&FourRemote::Signal).unwrap(), "\"S\"");
        assert_eq!(
            serde_json::to_string(&FourRemote::Control).unwrap(),
            "\"C\""
        );
        assert_eq!(
            serde_json::to_string(&FourRemote::Adjustment).unwrap(),
            "\"A\""
        );
    }

    #[test]
    fn test_four_remote_from_str() {
        assert_eq!(FourRemote::from_str("T").unwrap(), FourRemote::Telemetry);
        assert_eq!(FourRemote::from_str("S").unwrap(), FourRemote::Signal);
        assert_eq!(FourRemote::from_str("C").unwrap(), FourRemote::Control);
        assert_eq!(FourRemote::from_str("A").unwrap(), FourRemote::Adjustment);
        // IEC synonyms
        assert_eq!(FourRemote::from_str("YC").unwrap(), FourRemote::Telemetry);
        assert_eq!(FourRemote::from_str("YX").unwrap(), FourRemote::Signal);
        assert_eq!(FourRemote::from_str("YK").unwrap(), FourRemote::Control);
        assert_eq!(FourRemote::from_str("YT").unwrap(), FourRemote::Adjustment);

        // Test invalid
        assert!(FourRemote::from_str("X").is_err());
    }

    #[test]
    fn test_four_remote_methods() {
        // Test input/output classification
        assert!(FourRemote::Telemetry.is_input());
        assert!(FourRemote::Signal.is_input());
        assert!(!FourRemote::Control.is_input());
        assert!(!FourRemote::Adjustment.is_input());

        assert!(!FourRemote::Telemetry.is_output());
        assert!(!FourRemote::Signal.is_output());
        assert!(FourRemote::Control.is_output());
        assert!(FourRemote::Adjustment.is_output());

        // Test Default
        assert_eq!(FourRemote::default(), FourRemote::Telemetry);
    }

    #[test]
    fn test_parse_four_remote_helper() {
        assert_eq!(parse_four_remote("T").unwrap(), FourRemote::Telemetry);
        assert_eq!(parse_four_remote("yc").unwrap(), FourRemote::Telemetry);
        assert!(parse_four_remote("invalid").is_err());
    }
}
