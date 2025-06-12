//! # Configuration Management for Model Service
//! 
//! This module provides comprehensive configuration management for the Model Service (ModSrv),
//! including configuration file loading, validation, and default value handling. It supports
//! multiple configuration file formats and provides a type-safe interface for all configuration
//! options.
//! 
//! ## Overview
//! 
//! The configuration system is designed to be flexible and extensible, supporting both
//! file-based configuration and programmatic configuration. It includes validation,
//! default values, and clear error messages for configuration issues.
//! 
//! ## Supported Formats
//! 
//! - **YAML**: Primary configuration format with support for complex nested structures
//! - **TOML**: Alternative format for simpler configurations
//! - **JSON**: Support for JSON-based configuration files
//! 
//! ## Configuration Structure
//! 
//! The configuration is organized into logical sections:
//! - **Redis**: Database connection and key management
//! - **Logging**: Log levels, outputs, and formatting
//! - **Model**: Model execution and template management
//! - **Control**: Automated control operations
//! - **API**: HTTP API server configuration
//! - **Monitoring**: Performance monitoring and alerting
//! - **Storage**: Data storage backend configuration
//! 
//! ## Usage Examples
//! 
//! ### Loading from File
//! 
//! ```rust
//! use modsrv::config::Config;
//! 
//! // Load from YAML file
//! let config = Config::from_file("config.yaml")?;
//! 
//! // Access configuration sections
//! println!("Redis host: {}", config.redis.host);
//! println!("API port: {}", config.api.port);
//! println!("Log level: {}", config.logging.level);
//! ```
//! 
//! ### Using Default Configuration
//! 
//! ```rust
//! use modsrv::config::Config;
//! 
//! // Create with sensible defaults
//! let config = Config::default();
//! 
//! // Customize as needed
//! let mut config = config;
//! config.redis.host = "redis.example.com".to_string();
//! config.api.port = 9000;
//! ```
//! 
//! ### Configuration File Example
//! 
//! ```yaml
//! # Redis configuration
//! redis:
//!   host: "localhost"
//!   port: 6379
//!   database: 0
//!   key_prefix: "ems:"
//! 
//! # Logging configuration
//! logging:
//!   level: "info"
//!   file: "/var/log/modsrv.log"
//!   console: true
//! 
//! # Model execution configuration
//! model:
//!   update_interval_ms: 1000
//!   config_key_pattern: "ems:model:config:*"
//!   data_key_pattern: "ems:data:*"
//!   output_key_pattern: "ems:model:output:*"
//!   templates_dir: "/opt/templates"
//! 
//! # Control operations
//! control:
//!   operation_key_pattern: "ems:control:operation:*"
//!   enabled: true
//! 
//! # API server
//! api:
//!   host: "0.0.0.0"
//!   port: 8000
//! 
//! # Monitoring
//! monitoring:
//!   enabled: true
//!   notification_threshold_ms: 5000
//! 
//! # Storage configuration
//! use_redis: true
//! storage_mode: "hybrid"
//! sync_interval_secs: 60
//! ```

use crate::error::Result;
use config::{Config as ConfigLib, ConfigError, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use crate::storage::SyncMode;
use crate::error::{ModelSrvError};

/// Redis database connection configuration
/// 
/// Configures the connection to Redis database including host, port,
/// database selection, and key prefix for data organization.
/// 
/// # Fields
/// 
/// * `host` - Redis server hostname or IP address
/// * `port` - Redis server port number (typically 6379)
/// * `database` - Redis database number to use (0-15)
/// * `key_prefix` - Prefix for all Redis keys to avoid conflicts
/// 
/// # Examples
/// 
/// ```rust
/// use modsrv::config::RedisConfig;
/// 
/// let redis_config = RedisConfig {
///     host: "redis.example.com".to_string(),
///     port: 6379,
///     database: 1,
///     key_prefix: "myapp:".to_string(),
/// };
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RedisConfig {
    /// Redis server hostname or IP address
    pub host: String,
    /// Redis server port number
    pub port: u16,
    /// Redis database number to use
    pub database: u8,
    /// Prefix for all Redis keys
    pub key_prefix: String,
}

/// Logging system configuration
/// 
/// Controls logging behavior including log levels, output destinations,
/// and formatting options for the model service.
/// 
/// # Fields
/// 
/// * `level` - Log level (trace, debug, info, warn, error)
/// * `file` - Log file path for file-based logging
/// * `console` - Whether to enable console logging output
/// 
/// # Examples
/// 
/// ```rust
/// use modsrv::config::LoggingConfig;
/// 
/// let logging_config = LoggingConfig {
///     level: "debug".to_string(),
///     file: "/var/log/modsrv.log".to_string(),
///     console: true,
/// };
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    /// Log level filter (trace, debug, info, warn, error)
    pub level: String,
    /// Path to log file for persistent logging
    pub file: String,
    /// Enable console output for logs
    pub console: bool,
}

/// Model execution and template configuration
/// 
/// Configures the model execution engine including update intervals,
/// data key patterns, and template management settings.
/// 
/// # Fields
/// 
/// * `update_interval_ms` - Model execution interval in milliseconds
/// * `config_key_pattern` - Redis key pattern for model configurations
/// * `data_key_pattern` - Redis key pattern for input data
/// * `output_key_pattern` - Redis key pattern for model outputs
/// * `templates_dir` - Directory containing model templates
/// 
/// # Examples
/// 
/// ```rust
/// use modsrv::config::ModelConfig;
/// 
/// let model_config = ModelConfig {
///     update_interval_ms: 2000,
///     config_key_pattern: "models:config:*".to_string(),
///     data_key_pattern: "data:*".to_string(),
///     output_key_pattern: "outputs:*".to_string(),
///     templates_dir: "/opt/templates".to_string(),
/// };
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelConfig {
    /// Model execution interval in milliseconds
    pub update_interval_ms: u64,
    /// Redis key pattern for model configurations
    pub config_key_pattern: String,
    /// Redis key pattern for input data
    pub data_key_pattern: String,
    /// Redis key pattern for model outputs
    pub output_key_pattern: String,
    /// Directory containing model templates
    #[serde(default = "default_templates_dir")]
    pub templates_dir: String,
}

/// HTTP API server configuration
/// 
/// Configures the REST API server including network binding
/// and service endpoints.
/// 
/// # Fields
/// 
/// * `host` - Host address to bind the API server
/// * `port` - Port number for the API server
/// 
/// # Examples
/// 
/// ```rust
/// use modsrv::config::ApiConfig;
/// 
/// let api_config = ApiConfig {
///     host: "127.0.0.1".to_string(),
///     port: 9000,
/// };
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiConfig {
    /// API server host address
    #[serde(default = "default_api_host")]
    pub host: String,
    
    /// API server port number
    #[serde(default = "default_api_port")]
    pub port: u16,
}

impl Default for ApiConfig {
    /// Create default API configuration
    /// 
    /// # Returns
    /// 
    /// Default configuration with host "0.0.0.0" and port 8000
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_api_port(),
        }
    }
}

/// Default API server host address
/// 
/// # Returns
/// 
/// "0.0.0.0" to bind to all available interfaces
fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

/// Default API server port number
/// 
/// # Returns
/// 
/// Port 8000 as the default API server port
fn default_api_port() -> u16 {
    8000
}

/// Default templates directory path
/// 
/// # Returns
/// 
/// "templates" as the default templates directory
fn default_templates_dir() -> String {
    "templates".to_string()
}

/// Control operations configuration
/// 
/// Configures the automated control system including operation
/// key patterns and enable/disable settings.
/// 
/// # Fields
/// 
/// * `operation_key_pattern` - Redis key pattern for control operations
/// * `enabled` - Whether control operations are enabled
/// 
/// # Examples
/// 
/// ```rust
/// use modsrv::config::ControlConfig;
/// 
/// let control_config = ControlConfig {
///     operation_key_pattern: "controls:*".to_string(),
///     enabled: true,
/// };
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ControlConfig {
    /// Redis key pattern for control operations
    pub operation_key_pattern: String,
    /// Whether control operations are enabled
    pub enabled: bool,
}

/// System monitoring and alerting configuration
/// 
/// Configures performance monitoring, alerting thresholds,
/// and notification settings.
/// 
/// # Fields
/// 
/// * `enabled` - Whether monitoring is enabled
/// * `notification_threshold_ms` - Threshold for performance notifications
/// 
/// # Examples
/// 
/// ```rust
/// use modsrv::config::MonitoringConfig;
/// 
/// let monitoring_config = MonitoringConfig {
///     enabled: true,
///     notification_threshold_ms: Some(5000),
/// };
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MonitoringConfig {
    /// Whether monitoring is enabled
    pub enabled: bool,
    /// Performance notification threshold in milliseconds
    pub notification_threshold_ms: Option<u128>,
}

impl Default for MonitoringConfig {
    /// Create default monitoring configuration
    /// 
    /// # Returns
    /// 
    /// Default configuration with monitoring disabled
    fn default() -> Self {
        Self {
            enabled: false,
            notification_threshold_ms: None,
        }
    }
}

/// Configuration for the Model Service
/// 
/// This structure holds all configuration sections required for the
/// service to operate correctly, including database connections,
/// logging settings, model execution parameters, and system monitoring.
/// 
/// # Structure
/// 
/// The configuration is organized into logical sections:
/// * `redis` - Database connection settings
/// * `logging` - Logging system settings
/// * `model` - Model execution parameters
/// * `control` - Control operation settings
/// * `api` - HTTP API server settings
/// * `monitoring` - System monitoring settings
/// * `templates_dir` - Template file location
/// * `log_level` - Global log level setting
/// * `use_redis` - Whether to use Redis backend
/// * `storage_mode` - Storage backend mode
/// * `sync_interval_secs` - Data synchronization interval
/// 
/// # Examples
/// 
/// ```rust
/// use modsrv::config::Config;
/// 
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Load from file
/// let config = Config::from_file("config.yaml")?;
/// 
/// // Use defaults
/// let config = Config::default();
/// 
/// // Access configuration sections
/// println!("Using Redis: {}", config.use_redis);
/// println!("API endpoint: {}:{}", config.api.host, config.api.port);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Redis database configuration
    pub redis: RedisConfig,
    /// Logging system configuration
    pub logging: LoggingConfig,
    /// Model execution configuration
    pub model: ModelConfig,
    /// Control operations configuration
    pub control: ControlConfig,
    /// HTTP API server configuration
    #[serde(default)]
    pub api: ApiConfig,
    /// System monitoring configuration
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    /// Template directory path
    pub templates_dir: String,
    /// Global log level setting
    pub log_level: String,
    
    /// Whether to use Redis storage backend
    #[serde(default = "default_use_redis")]
    pub use_redis: bool,
    
    /// Storage backend mode (memory, redis, hybrid)
    #[serde(default = "default_storage_mode")]
    pub storage_mode: String,
    
    /// Data synchronization interval in seconds
    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,
}

/// Default Redis usage setting
/// 
/// # Returns
/// 
/// `true` to use Redis by default
fn default_use_redis() -> bool {
    true
}

/// Default storage mode setting
/// 
/// # Returns
/// 
/// "hybrid" as the default storage mode
fn default_storage_mode() -> String {
    "hybrid".to_string()
}

/// Default synchronization interval
/// 
/// # Returns
/// 
/// 60 seconds as the default sync interval
fn default_sync_interval_secs() -> u64 {
    60
}

impl Config {
    /// Create configuration from file (legacy method)
    /// 
    /// Loads configuration from a file with automatic format detection
    /// based on file extension. This method is deprecated in favor of
    /// `from_file()` for better error handling.
    /// 
    /// # Arguments
    /// 
    /// * `config_file` - Path to configuration file
    /// 
    /// # Returns
    /// 
    /// * `Ok(Config)` - Successfully loaded configuration
    /// * `Err(ModelSrvError)` - Configuration loading or parsing error
    /// 
    /// # Supported Formats
    /// 
    /// - `.yaml`, `.yml` - YAML format
    /// - `.toml` - TOML format
    /// - Others default to YAML
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use modsrv::config::Config;
    /// 
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::new("config.yaml")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(config_file: &str) -> Result<Self> {
        let config_path = Path::new(config_file);
        if !config_path.exists() {
            return Err(ConfigError::NotFound(config_file.to_string()).into());
        }

        // Determine format based on file extension
        let format = match config_path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => FileFormat::Yaml,
            Some("toml") => FileFormat::Toml,
            _ => FileFormat::Yaml, // Default to YAML
        };

        let config = ConfigLib::builder()
            .add_source(File::with_name(config_file).format(format))
            .build()?;

        let mut config: Config = config.try_deserialize()?;
        config.templates_dir = config.model.templates_dir.clone();
        Ok(config)
    }
    
    /// Load configuration from file
    /// 
    /// Loads and parses configuration from a file with automatic format
    /// detection. Supports YAML, TOML, and JSON formats with comprehensive
    /// error reporting.
    /// 
    /// # Arguments
    /// 
    /// * `path` - Path to configuration file
    /// 
    /// # Returns
    /// 
    /// * `Ok(Config)` - Successfully loaded and validated configuration
    /// * `Err(ModelSrvError)` - File access, parsing, or validation error
    /// 
    /// # Format Detection
    /// 
    /// File format is determined by extension:
    /// - `.yaml`, `.yml` → YAML parser
    /// - `.toml` → TOML parser  
    /// - All others → YAML parser (default)
    /// 
    /// # Error Handling
    /// 
    /// Provides detailed error messages for:
    /// - File not found or access denied
    /// - Invalid file format or syntax
    /// - Missing required configuration fields
    /// - Invalid configuration values
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use modsrv::config::Config;
    /// 
    /// // Load YAML configuration
    /// let config = Config::from_file("config.yaml")?;
    /// 
    /// // Load TOML configuration
    /// let config = Config::from_file("config.toml")?;
    /// 
    /// // Use Path for type safety
    /// use std::path::Path;
    /// let config = Config::from_file(Path::new("config.yaml"))?;
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        
        // Determine format based on file extension
        let format = match path_ref.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => FileFormat::Yaml,
            Some("toml") => FileFormat::Toml,
            _ => FileFormat::Yaml, // Default to YAML
        };
        
        let config = ConfigLib::builder()
            .add_source(File::with_name(path_ref.to_str().unwrap()).format(format))
            .build()
            .map_err(|e| ModelSrvError::ConfigError(e.to_string()))?;
            
        config.try_deserialize()
            .map_err(|e| ModelSrvError::ConfigError(e.to_string()))
    }

    /// Create default configuration
    /// 
    /// Returns a configuration instance with sensible defaults for all
    /// settings. Useful for testing, development, or as a starting point
    /// for custom configurations.
    /// 
    /// # Returns
    /// 
    /// Complete configuration with production-ready defaults
    /// 
    /// # Default Values
    /// 
    /// - **Redis**: localhost:6379, database 0, key prefix "ems:"
    /// - **Logging**: Info level, file and console output enabled
    /// - **Model**: 1-second update interval, standard key patterns
    /// - **Control**: Enabled with standard operation patterns
    /// - **API**: Bind to all interfaces on port 8000
    /// - **Monitoring**: Disabled by default
    /// - **Storage**: Hybrid mode with Redis backend
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use modsrv::config::Config;
    /// 
    /// let mut config = Config::default();
    /// 
    /// // Customize as needed
    /// config.redis.host = "redis.example.com".to_string();
    /// config.api.port = 9000;
    /// config.model.update_interval_ms = 500;
    /// ```
    pub fn default() -> Self {
        Config {
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                database: 0,
                key_prefix: "ems:".to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: "/var/log/ems/modelsrv.log".to_string(),
                console: true,
            },
            model: ModelConfig {
                update_interval_ms: 1000,
                config_key_pattern: "ems:model:config:*".to_string(),
                data_key_pattern: "ems:data:*".to_string(),
                output_key_pattern: "ems:model:output:*".to_string(),
                templates_dir: "/opt/voltageems/modsrv/templates".to_string(),
            },
            control: ControlConfig {
                operation_key_pattern: "ems:control:operation:*".to_string(),
                enabled: true,
            },
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                port: 8000,
            },
            monitoring: MonitoringConfig {
                enabled: false,
                notification_threshold_ms: None,
            },
            templates_dir: "/opt/voltageems/modsrv/templates".to_string(),
            log_level: "info".to_string(),
            use_redis: true,
            storage_mode: "hybrid".to_string(),
            sync_interval_secs: 60,
        }
    }

    pub fn get_sync_mode(&self) -> SyncMode {
        match self.storage_mode.as_str() {
            "write_through" => SyncMode::WriteThrough,
            "write_back" => SyncMode::WriteBack(Duration::from_secs(self.sync_interval_secs)),
            "on_demand" => SyncMode::OnDemand,
            _ => SyncMode::WriteThrough, // Default to use WriteThrough
        }
    }
} 