use crate::core::config::protocol_table_manager::FourTelemetryTableManager;
use crate::utils::error::{ComSrvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use redis::{Client, Connection};
use serde_json;

/// Default path configuration for channels and tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultPathConfig {
    /// Root directory for all channels (relative to config directory)
    pub channels_root: String,
    /// ComBase point table directory name within each channel
    pub combase_dir: String,
    /// Protocol source table directory name within each channel
    pub protocol_dir: String,
    /// Default file names for different table types
    pub filenames: DefaultFilenames,
}

/// Default file names for point tables and source tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultFilenames {
    /// Telemetry (遥测) point table file
    pub telemetry: String,
    /// Signaling (遥信) point table file
    pub signaling: String,
    /// Control (遥控) point table file
    pub control: String,
    /// Setpoint (遥调) point table file
    pub setpoint: String,
    /// Modbus TCP source table file
    pub modbus_tcp_source: String,
    /// Modbus RTU source table file
    pub modbus_rtu_source: String,
    /// Calculation source table file
    pub calculation_source: String,
    /// Manual source table file
    pub manual_source: String,
}

impl Default for DefaultPathConfig {
    fn default() -> Self {
        Self {
            channels_root: "channels".to_string(),
            combase_dir: "combase".to_string(),
            protocol_dir: "protocol".to_string(),
            filenames: DefaultFilenames::default(),
        }
    }
}

impl Default for DefaultFilenames {
    fn default() -> Self {
        Self {
            telemetry: "telemetry.csv".to_string(),
            signaling: "signaling.csv".to_string(),
            control: "control.csv".to_string(),
            setpoint: "setpoint.csv".to_string(),
            modbus_tcp_source: "modbus_tcp_source.csv".to_string(),
            modbus_rtu_source: "modbus_rtu_source.csv".to_string(),
            calculation_source: "calculation_source.csv".to_string(),
            manual_source: "manual_source.csv".to_string(),
        }
    }
}

/// Service configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    #[serde(default = "default_service_name")]
    pub name: String,
    /// Service description (optional)
    #[serde(default)]
    pub description: Option<String>,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
    /// API configuration
    #[serde(default)]
    pub api: ApiConfig,
    /// Redis configuration
    #[serde(default)]
    pub redis: RedisConfig,
}

fn default_service_name() -> String {
    "comsrv".to_string()
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: default_service_name(),
            description: Some("Communication Service".to_string()),
            logging: LoggingConfig::default(),
            api: ApiConfig::default(),
            redis: RedisConfig::default(),
        }
    }
}

/// API configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Whether API is enabled
    #[serde(default = "default_api_enabled")]
    pub enabled: bool,
    /// Bind address for API server
    #[serde(default = "default_api_bind_address")]
    pub bind_address: String,
    /// API version
    #[serde(default = "default_api_version")]
    pub version: String,
}

fn default_api_enabled() -> bool {
    true
}

fn default_api_bind_address() -> String {
    "127.0.0.1:8080".to_string()
}

fn default_api_version() -> String {
    "v1".to_string()
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: default_api_enabled(),
            bind_address: default_api_bind_address(),
            version: default_api_version(),
        }
    }
}

/// Redis connection type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RedisConnectionType {
    Tcp,
    Unix,
}

/// Redis configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Whether Redis is enabled
    #[serde(default = "default_redis_enabled")]
    pub enabled: bool,
    /// Connection type (TCP or Unix socket)
    #[serde(default)]
    pub connection_type: RedisConnectionType,
    /// Redis server address
    /// For TCP: "127.0.0.1:6379" or "redis://127.0.0.1:6379"
    /// For Unix: "/tmp/redis.sock" or "unix:///tmp/redis.sock"
    #[serde(default = "default_redis_address")]
    pub address: String,
    /// Database number (0-15)
    #[serde(default = "default_redis_db")]
    pub db: u8,
    /// Connection timeout in milliseconds
    #[serde(default = "default_redis_timeout")]
    pub timeout_ms: u64,
    /// Maximum number of connections in pool (optional, no limit if not set)
    #[serde(default)]
    pub max_connections: Option<u32>,
    /// Minimum number of connections in pool (optional, no limit if not set)
    #[serde(default)]
    pub min_connections: Option<u32>,
    /// Connection idle timeout in seconds
    #[serde(default = "default_redis_idle_timeout")]
    pub idle_timeout_secs: u64,
    /// Maximum number of retry attempts
    #[serde(default = "default_redis_max_retries")]
    pub max_retries: u32,
    /// Password for Redis authentication (optional)
    #[serde(default)]
    pub password: Option<String>,
    /// Username for Redis authentication (optional, Redis 6.0+)
    #[serde(default)]
    pub username: Option<String>,
}

fn default_redis_enabled() -> bool {
    true
}

fn default_redis_address() -> String {
    "127.0.0.1:6379".to_string()
}

fn default_redis_db() -> u8 {
    1
}

fn default_redis_timeout() -> u64 {
    5000
}

fn default_redis_idle_timeout() -> u64 {
    300
}

fn default_redis_max_retries() -> u32 {
    3
}

impl Default for RedisConnectionType {
    fn default() -> Self {
        RedisConnectionType::Tcp
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: default_redis_enabled(),
            connection_type: RedisConnectionType::Tcp,
            address: default_redis_address(),
            db: default_redis_db(),
            timeout_ms: default_redis_timeout(),
            max_connections: None,
            min_connections: None,
            idle_timeout_secs: default_redis_idle_timeout(),
            max_retries: default_redis_max_retries(),
            password: None,
            username: None,
        }
    }
}

impl RedisConfig {
    /// Create Redis configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let enabled: bool = std::env::var("REDIS_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .map_err(|_| ComSrvError::ConfigError("Invalid REDIS_ENABLED value".to_string()))?;

        if !enabled {
            return Ok(Self {
                enabled: false,
                ..Default::default()
            });
        }

        let address =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let connection_type = if address.starts_with("unix://") || address.starts_with("/") {
            RedisConnectionType::Unix
        } else {
            RedisConnectionType::Tcp
        };

        Ok(Self {
            enabled,
            connection_type,
            address,
            db: std::env::var("REDIS_DB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            timeout_ms: std::env::var("REDIS_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5000),
            max_connections: std::env::var("REDIS_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok()),
            min_connections: std::env::var("REDIS_MIN_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok()),
            idle_timeout_secs: std::env::var("REDIS_IDLE_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
            max_retries: std::env::var("REDIS_MAX_RETRIES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            password: std::env::var("REDIS_PASSWORD").ok(),
            username: std::env::var("REDIS_USERNAME").ok(),
        })
    }

    /// Validate Redis configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.address.is_empty() {
            return Err(ComSrvError::ConfigError(
                "Redis address cannot be empty".to_string(),
            ));
        }

        if self.max_connections == Some(0) {
            return Err(ComSrvError::ConfigError(
                "Redis max_connections must be greater than 0".to_string(),
            ));
        }

        if self.min_connections > Some(0) && self.min_connections > self.max_connections {
            return Err(ComSrvError::ConfigError(
                "Redis min_connections cannot exceed max_connections".to_string(),
            ));
        }

        if self.timeout_ms == 0 {
            return Err(ComSrvError::ConfigError(
                "Redis timeout_ms must be greater than 0".to_string(),
            ));
        }

        // Validate database number (Redis supports 0-15 by default)
        if self.db > 15 {
            return Err(ComSrvError::ConfigError(
                "Redis database number must be between 0-15".to_string(),
            ));
        }

        Ok(())
    }

    /// Convert to Redis URL string
    pub fn to_redis_url(&self) -> String {
        let base_url = match self.connection_type {
            RedisConnectionType::Tcp => {
                if self.address.starts_with("redis://") || self.address.starts_with("rediss://") {
                    self.address.clone()
                } else {
                    format!("redis://{}", self.address)
                }
            }
            RedisConnectionType::Unix => {
                if self.address.starts_with("unix://") {
                    self.address.clone()
                } else {
                    format!("unix://{}", self.address)
                }
            }
        };

        // Add authentication if provided
        let auth_url = if let (Some(username), Some(password)) = (&self.username, &self.password) {
            base_url.replace("://", &format!("://{}:{}@", username, password))
        } else if let Some(password) = &self.password {
            base_url.replace("://", &format!("://:{}@", password))
        } else {
            base_url
        };

        // Add database number
        format!("{}/{}", auth_url, self.db)
    }

    /// Get connection timeout as Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    /// Get idle timeout as Duration  
    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.idle_timeout_secs)
    }
}

/// Get Redis configuration
pub fn get_redis_config(config: &Config) -> &RedisConfig {
    &config.service.redis
}

/// Logging configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Log file path (optional)
    #[serde(default)]
    pub file: Option<String>,
    /// Maximum log file size in bytes
    #[serde(default = "default_log_max_size")]
    pub max_size: u64,
    /// Maximum number of log files to keep
    #[serde(default = "default_log_max_files")]
    pub max_files: u32,
    /// Whether to output logs to console
    #[serde(default = "default_log_console")]
    pub console: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_max_size() -> u64 {
    104857600 // 100MB
}

fn default_log_max_files() -> u32 {
    5
}

fn default_log_console() -> bool {
    true
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
            max_size: default_log_max_size(),
            max_files: default_log_max_files(),
            console: default_log_console(),
        }
    }
}

/// Point tables configuration

/// Channel parameters, specific to each protocol type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChannelParameters {
    /// Modbus TCP specific parameters
    ModbusTcp {
        host: String,
        port: u16,
        #[serde(default = "default_timeout")]
        timeout: u64,
        #[serde(default = "default_max_retries")]
        max_retries: u32,
        #[serde(default)]
        poll_rate: Option<u64>,
        #[serde(default)]
        slave_id: Option<u8>,
    },
    /// Modbus RTU specific parameters
    ModbusRtu {
        port: String,
        #[serde(default = "default_baud_rate")]
        baud_rate: u32,
        #[serde(default = "default_data_bits")]
        data_bits: u8,
        #[serde(default = "default_parity")]
        parity: String,
        #[serde(default = "default_stop_bits")]
        stop_bits: u8,
        #[serde(default = "default_timeout")]
        timeout: u64,
        #[serde(default = "default_max_retries")]
        max_retries: u32,
        #[serde(default)]
        poll_rate: Option<u64>,
        #[serde(default)]
        slave_id: Option<u8>,
    },
    /// Virtual protocol parameters (no poll_rate needed)
    Virtual {
        #[serde(default = "default_max_retries")]
        max_retries: u32,
        #[serde(default = "default_timeout")]
        timeout: u64,
    },
    /// Generic parameters as HashMap
    Generic(HashMap<String, serde_yaml::Value>),
}

fn default_timeout() -> u64 {
    1000
}

fn default_max_retries() -> u32 {
    3
}

fn default_baud_rate() -> u32 {
    9600
}

fn default_data_bits() -> u8 {
    8
}

fn default_parity() -> String {
    "None".to_string()
}

fn default_stop_bits() -> u8 {
    1
}

impl ChannelParameters {
    /// Get the value of the parameter
    pub fn get(&self, key: &str) -> Option<serde_yaml::Value> {
        match self {
            ChannelParameters::ModbusTcp {
                host,
                port,
                timeout,
                max_retries,
                poll_rate,
                ..
            } => match key {
                "host" => Some(serde_yaml::Value::String(host.clone())),
                "port" => Some(serde_yaml::Value::Number((*port).into())),
                "timeout" => Some(serde_yaml::Value::Number((*timeout).into())),
                "max_retries" => Some(serde_yaml::Value::Number((*max_retries).into())),
                "poll_rate" => poll_rate.map(|v| serde_yaml::Value::Number(v.into())),
                _ => None,
            },
            ChannelParameters::ModbusRtu {
                port,
                baud_rate,
                data_bits,
                parity,
                stop_bits,
                timeout,
                max_retries,
                poll_rate,
                slave_id,
                ..
            } => match key {
                "port" => Some(serde_yaml::Value::String(port.clone())),
                "baud_rate" => Some(serde_yaml::Value::Number((*baud_rate).into())),
                "data_bits" => Some(serde_yaml::Value::Number((*data_bits).into())),
                "parity" => Some(serde_yaml::Value::String(parity.clone())),
                "stop_bits" => Some(serde_yaml::Value::Number((*stop_bits).into())),
                "timeout" => Some(serde_yaml::Value::Number((*timeout).into())),
                "max_retries" => Some(serde_yaml::Value::Number((*max_retries).into())),
                "poll_rate" => poll_rate.map(|v| serde_yaml::Value::Number(v.into())),
                "slave_id" => slave_id.map(|v| serde_yaml::Value::Number(v.into())),
                _ => None,
            },
            ChannelParameters::Virtual { max_retries, timeout } => match key {
                "max_retries" => Some(serde_yaml::Value::Number((*max_retries).into())),
                "timeout" => Some(serde_yaml::Value::Number((*timeout).into())),
                _ => None,
            },
            ChannelParameters::Generic(map) => map.get(key).cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolType {
    ModbusTcp,
    ModbusRtu,
    Virtual,
    Dio,
    Can,
    Iec104,
    Iec61850,
}

impl ProtocolType {
    /// Get the string representation of the protocol type
    pub fn as_str(&self) -> &'static str {
        match self {
            ProtocolType::ModbusTcp => "ModbusTcp",
            ProtocolType::ModbusRtu => "ModbusRtu",
            ProtocolType::Virtual => "Virtual",
            ProtocolType::Dio => "Dio",
            ProtocolType::Can => "Can",
            ProtocolType::Iec104 => "Iec104",
            ProtocolType::Iec61850 => "Iec61850",
        }
    }
}

impl std::fmt::Display for ProtocolType {
    /// Format the protocol type as a string
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ProtocolType {
    type Err = String;

    /// Parse a string into a protocol type
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "modbustcp" => Ok(ProtocolType::ModbusTcp),
            "modbusrtu" => Ok(ProtocolType::ModbusRtu),
            "virtual" => Ok(ProtocolType::Virtual),
            "dio" => Ok(ProtocolType::Dio),
            "can" => Ok(ProtocolType::Can),
            "iec104" => Ok(ProtocolType::Iec104),
            "iec61850" => Ok(ProtocolType::Iec61850),
            _ => Err(format!("Invalid protocol type: {}", s)),
        }
    }
}

/// Channel configuration
///
/// Complete configuration for a single communication channel including
/// protocol type, connection parameters, and metadata.
///
/// # Fields
///
/// * `id` - Unique numeric identifier for the channel
/// * `name` - Human-readable name for the channel
/// * `description` - Detailed description of the channel purpose
/// * `protocol` - Protocol type (ModbusTcp, ModbusRtu, etc.)
/// * `parameters` - Protocol-specific configuration parameters
/// * `point_table` - Point table configuration for this channel
///
/// # Examples
///
/// ```rust
/// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
/// use std::collections::HashMap;
///
/// let config = ChannelConfig {
///     id: 1,
///     name: "Device 1".to_string(),
///     description: "Main temperature sensor".to_string(),
///     protocol: ProtocolType::ModbusTcp,
///     parameters: ChannelParameters::Generic(HashMap::new()),
///     point_table: None,
///     source_tables: None,
///     csv_config: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Unique channel identifier
    pub id: u16,
    /// Human readable name
    pub name: String,
    /// Channel description (optional)
    #[serde(default)]
    pub description: Option<String>,
    /// Protocol type (e.g., "modbus_rtu", "modbus_tcp")
    pub protocol: ProtocolType,
    /// Protocol-specific parameters
    pub parameters: ChannelParameters,
    /// Point table configuration for this channel (ComBase 四遥点表)
    #[serde(default)]
    pub point_table: Option<ChannelPointTableConfig>,
    /// Source table configuration for this channel - completely optional
    #[serde(default)]
    pub source_tables: Option<ChannelSourceTableConfig>,
    /// CSV configuration for ComBase point tables (deprecated - use point_table instead)
    #[serde(default)]
    pub csv_config: Option<ChannelCsvConfig>,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            id: 1,
            name: "Default Channel".to_string(),
            description: Some("Default channel description".to_string()),
            protocol: ProtocolType::Virtual,
            parameters: ChannelParameters::Virtual {
                max_retries: default_max_retries(),
                timeout: default_timeout(),
            },
            point_table: Some(ChannelPointTableConfig::default()),
            source_tables: None, // Source tables are completely optional
            csv_config: None,
        }
    }
}

/// Channel-level point table configuration
///
/// This configuration specifies how point tables are organized for each channel.
/// Supports both default path structure and custom paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPointTableConfig {
    /// Whether point table is enabled for this channel
    pub enabled: bool,
    /// Use default path structure (config/channels/channel_{id}_{name}/combase/)
    #[serde(default)]
    pub use_defaults: Option<bool>,
    /// Custom directory (overrides default if use_defaults is false)
    #[serde(default)]
    pub directory: Option<String>,
    /// Custom file names (overrides defaults if specified)
    #[serde(default)]
    pub telemetry_file: Option<String>,
    #[serde(default)]
    pub signaling_file: Option<String>,
    #[serde(default)]
    pub control_file: Option<String>,
    #[serde(default)]
    pub setpoint_file: Option<String>,
    /// Whether to watch for file changes
    pub watch_changes: bool,
    /// Reload interval in seconds
    pub reload_interval: u64,
}

/// Channel source table configuration
///
/// Configuration for protocol source tables that define data sources
/// and their mapping to point tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSourceTableConfig {
    /// Whether source table loading is enabled
    pub enabled: bool,
    /// Use default path structure (config/channels/channel_{id}_{name}/protocol/)
    #[serde(default)]
    pub use_defaults: Option<bool>,
    /// Custom directory (overrides default if use_defaults is false)
    #[serde(default)]
    pub directory: Option<String>,
    /// Custom source table files (overrides defaults if specified)
    #[serde(default)]
    pub modbus_tcp_source: Option<String>,
    #[serde(default)]
    pub modbus_rtu_source: Option<String>,
    #[serde(default)]
    pub calculation_source: Option<String>,
    #[serde(default)]
    pub manual_source: Option<String>,
    /// Redis prefix for source table data
    #[serde(default)]
    pub redis_prefix: Option<String>,
}

impl Default for ChannelPointTableConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            use_defaults: Some(true),
            directory: None,
            telemetry_file: None,
            signaling_file: None,
            control_file: None,
            setpoint_file: None,
            watch_changes: true,
            reload_interval: 60,
        }
    }
}

impl Default for ChannelSourceTableConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            use_defaults: Some(true),
            directory: None,
            modbus_tcp_source: None,
            modbus_rtu_source: None,
            calculation_source: None,
            manual_source: None,
            redis_prefix: None,
        }
    }
}

impl ChannelPointTableConfig {
    /// Get the full path to a specific telemetry type CSV file
    pub fn get_csv_path(&self, telemetry_type: TelemetryType, defaults: &DefaultPathConfig, channel_id: u16, channel_name: &str) -> Option<PathBuf> {
        // Determine the directory
        let directory = if self.use_defaults.unwrap_or(true) {
            // Use default path: config/channels/channel_{id}_{name}/combase/
            let channel_dir = format!("channel_{}_{}", channel_id, channel_name.to_lowercase().replace(' ', "_"));
            PathBuf::from("config")
                .join(&defaults.channels_root)
                .join(channel_dir)
                .join(&defaults.combase_dir)
        } else {
            // Use custom directory
            PathBuf::from(self.directory.as_ref()?)
        };

        // Determine the filename
        let filename = match telemetry_type {
            TelemetryType::Telemetry => {
                self.telemetry_file.as_ref()
                    .unwrap_or(&defaults.filenames.telemetry)
            },
            TelemetryType::Signaling => {
                self.signaling_file.as_ref()
                    .unwrap_or(&defaults.filenames.signaling)
            },
            TelemetryType::Control => {
                self.control_file.as_ref()
                    .unwrap_or(&defaults.filenames.control)
            },
            TelemetryType::Setpoint => {
                self.setpoint_file.as_ref()
                    .unwrap_or(&defaults.filenames.setpoint)
            },
        };
        
        Some(directory.join(filename))
    }
    
    /// Check if all required point table files are configured
    pub fn is_complete(&self) -> bool {
        self.enabled && 
        self.telemetry_file.is_some() && 
        self.signaling_file.is_some() && 
        self.control_file.is_some() && 
        self.setpoint_file.is_some()
    }
}

/// Top-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Configuration schema version
    pub version: String,
    /// Service configuration
    pub service: ServiceConfig,
    /// Default path configuration for channels and tables
    #[serde(default)]
    pub defaults: DefaultPathConfig,
    /// Channel configurations
    pub channels: Vec<ChannelConfig>,
}

/// Configuration manager for the communication service
pub struct ConfigManager {
    /// Configuration data
    config: Config,
    /// Path to configuration file
    config_path: String,
    /// CSV point table manager
    csv_point_manager: FourTelemetryTableManager,
    /// Optional Redis store for configuration data
    redis_store: Option<crate::core::storage::redis_storage::RedisStore>,
}

impl Clone for ConfigManager {
    fn clone(&self) -> Self {
        // Create a new point table manager with CSV storage using a temporary directory
        let temp_dir = std::env::temp_dir().join(format!("comsrv_clone_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let csv_storage = Box::new(crate::core::config::storage::CsvPointTableStorage::new(&temp_dir));
        let csv_point_manager = FourTelemetryTableManager::new(csv_storage);

        Self {
            config: self.config.clone(),
            config_path: self.config_path.clone(),
            csv_point_manager,
            redis_store: self.redis_store.clone(),
        }
    }
}

impl ConfigManager {
    /// Create configuration manager from file
    pub fn from_file(config_path: impl AsRef<Path>) -> Result<Self> {
        let config_path = config_path.as_ref().to_string_lossy().to_string();
        let config = Self::load_config(&config_path)?;

        // Create point table manager with CSV storage using a temporary directory
        let temp_dir = std::env::temp_dir().join(format!("comsrv_config_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let csv_storage = Box::new(crate::core::config::storage::CsvPointTableStorage::new(&temp_dir));
        let csv_point_manager = FourTelemetryTableManager::new(csv_storage);

        let manager = Self {
            config,
            config_path,
            csv_point_manager,
            redis_store: None,
        };

        manager.validate_config()?;
        Ok(manager)
    }

    /// Create configuration manager with Redis storage support
    pub async fn from_file_with_redis(
        config_path: impl AsRef<Path>,
        redis_store: crate::core::storage::redis_storage::RedisStore,
    ) -> Result<Self> {
        let config_path = config_path.as_ref().to_string_lossy().to_string();
        let config = Self::load_config(&config_path)?;

        // Create point table manager with CSV storage using a temporary directory
        let temp_dir = std::env::temp_dir().join(format!("comsrv_redis_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let csv_storage = Box::new(crate::core::config::storage::CsvPointTableStorage::new(&temp_dir));
        let csv_point_manager = FourTelemetryTableManager::new(csv_storage);

        let manager = Self {
            config,
            config_path,
            csv_point_manager,
            redis_store: Some(redis_store.clone()),
        };

        manager.validate_config()?;

        // Store initial configuration to Redis
        if let Err(e) = manager.sync_config_to_redis().await {
            log::warn!("Failed to sync initial configuration to Redis: {}", e);
        }

        Ok(manager)
    }

    /// Enable Redis storage for configuration data
    pub async fn enable_redis_storage(&mut self, redis_store: crate::core::storage::redis_storage::RedisStore) -> Result<()> {
        self.redis_store = Some(redis_store);
        
        // Sync current configuration to Redis
        self.sync_config_to_redis().await?;
        
        log::info!("Redis storage enabled for ConfigManager");
        Ok(())
    }

    /// Disable Redis storage
    pub fn disable_redis_storage(&mut self) {
        if self.redis_store.is_some() {
            self.redis_store = None;
            log::info!("Redis storage disabled for ConfigManager");
        }
    }

    /// Check if Redis storage is enabled
    pub fn is_redis_enabled(&self) -> bool {
        self.redis_store.is_some()
    }

    /// Synchronize current configuration to Redis
    pub async fn sync_config_to_redis(&self) -> Result<()> {
        if let Some(ref redis_store) = self.redis_store {
            // Store service configuration
            let service_config_data = crate::core::storage::redis_storage::RedisConfigData {
                config_type: "service".to_string(),
                data: serde_json::to_value(&self.config.service)?,
                version: self.config.version.clone(),
                last_updated: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            };
            redis_store.set_config_data("service", &service_config_data).await?;

            // Store each channel configuration
            for channel in &self.config.channels {
                let channel_config_data = crate::core::storage::redis_storage::RedisConfigData {
                    config_type: "channel".to_string(),
                    data: serde_json::to_value(channel)?,
                    version: self.config.version.clone(),
                    last_updated: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                };
                let config_key = format!("channel_{}", channel.id);
                redis_store.set_config_data(&config_key, &channel_config_data).await?;
            }

            log::debug!("Synchronized configuration to Redis: service + {} channels", self.config.channels.len());
        }
        Ok(())
    }

    /// Load configuration from Redis (if available) or fallback to file
    pub async fn load_config_from_redis(&mut self) -> Result<bool> {
        if let Some(ref redis_store) = self.redis_store {
            // Try to load service configuration from Redis
            if let Some(service_config_data) = redis_store.get_config_data("service").await? {
                let service_config: ServiceConfig = serde_json::from_value(service_config_data.data)?;
                
                // Load channel configurations
                let config_names = redis_store.list_config_names().await?;
                let mut channels = Vec::new();
                
                for config_name in config_names {
                    if config_name.starts_with("channel_") {
                        if let Some(channel_config_data) = redis_store.get_config_data(&config_name).await? {
                            let channel_config: ChannelConfig = serde_json::from_value(channel_config_data.data)?;
                            channels.push(channel_config);
                        }
                    }
                }

                // Sort channels by ID
                channels.sort_by_key(|c| c.id);

                // Update configuration
                self.config.service = service_config;
                self.config.channels = channels;

                log::info!("Loaded configuration from Redis: service + {} channels", self.config.channels.len());
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Add or update a channel configuration
    pub async fn upsert_channel(&mut self, channel: ChannelConfig) -> Result<()> {
        // Update in-memory configuration
        if let Some(existing) = self.config.channels.iter_mut().find(|c| c.id == channel.id) {
            *existing = channel.clone();
        } else {
            self.config.channels.push(channel.clone());
            self.config.channels.sort_by_key(|c| c.id);
        }

        // Update Redis if enabled
        if let Some(ref redis_store) = self.redis_store {
            let channel_config_data = crate::core::storage::redis_storage::RedisConfigData {
                config_type: "channel".to_string(),
                data: serde_json::to_value(&channel)?,
                version: self.config.version.clone(),
                last_updated: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            };
            let config_key = format!("channel_{}", channel.id);
            redis_store.set_config_data(&config_key, &channel_config_data).await?;
            
            log::debug!("Updated channel {} configuration in Redis", channel.id);
        }

        Ok(())
    }

    /// Remove a channel configuration
    pub async fn remove_channel(&mut self, channel_id: u16) -> Result<bool> {
        let removed = self.config.channels.iter().position(|c| c.id == channel_id)
            .map(|index| self.config.channels.remove(index))
            .is_some();

        // Remove from Redis if enabled
        if let Some(ref redis_store) = self.redis_store {
            let config_key = format!("channel_{}", channel_id);
            redis_store.delete_key(&format!("comsrv:config:{}", config_key)).await?;
            
            log::debug!("Removed channel {} configuration from Redis", channel_id);
        }

        Ok(removed)
    }

    /// Validate the loaded configuration
    pub fn validate_config(&self) -> Result<()> {
        // Validate service configuration
        if self.config.service.name.is_empty() {
            return Err(ComSrvError::ConfigError(
                "Service name cannot be empty".to_string(),
            ));
        }

        // Validate API configuration
        if self.config.service.api.enabled {
            if self.config.service.api.bind_address.is_empty() {
                return Err(ComSrvError::ConfigError(
                    "API bind address cannot be empty when API is enabled".to_string(),
                ));
            }

            // Validate bind address format
            if let Err(e) = self.config.service.api.bind_address.parse::<SocketAddr>() {
                return Err(ComSrvError::ConfigError(format!(
                    "Invalid API bind address format: {}, error: {}",
                    self.config.service.api.bind_address, e
                )));
            }
        }

        // Validate channels
        let mut channel_ids = std::collections::HashSet::new();
        for channel in &self.config.channels {
            // Check for duplicate channel IDs
            if !channel_ids.insert(channel.id) {
                return Err(ComSrvError::ConfigError(format!(
                    "Duplicate channel ID found: {}",
                    channel.id
                )));
            }

            // Validate channel name
            if channel.name.is_empty() {
                return Err(ComSrvError::ConfigError(format!(
                    "Channel name cannot be empty for channel ID: {}",
                    channel.id
                )));
            }

            // Validate protocol-specific parameters
            self.validate_channel_parameters(&channel)?;
        }

        Ok(())
    }

    /// Validate channel-specific parameters
    fn validate_channel_parameters(&self, channel: &ChannelConfig) -> Result<()> {
        match &channel.parameters {
            ChannelParameters::ModbusTcp {
                host,
                port,
                timeout,
                max_retries,
                poll_rate,
                ..
            } => {
                if host.is_empty() {
                    return Err(ComSrvError::ConfigError(format!(
                        "Modbus TCP host cannot be empty for channel {}",
                        channel.id
                    )));
                }
                if *port == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Modbus TCP port cannot be 0 for channel {}",
                        channel.id
                    )));
                }
                if *timeout == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Timeout cannot be 0 for channel {}",
                        channel.id
                    )));
                }
                if *max_retries > 10 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Max retries should not exceed 10 for channel {}",
                        channel.id
                    )));
                }
                if let Some(rate) = poll_rate {
                    if *rate == 0 {
                        return Err(ComSrvError::ConfigError(format!(
                            "Poll rate cannot be 0 for channel {}",
                            channel.id
                        )));
                    }
                }
            }
            ChannelParameters::ModbusRtu {
                port,
                baud_rate,
                timeout,
                max_retries,
                poll_rate,
                slave_id,
                ..
            } => {
                if port.is_empty() {
                    return Err(ComSrvError::ConfigError(format!(
                        "Modbus RTU port cannot be empty for channel {}",
                        channel.id
                    )));
                }
                if *baud_rate == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Baud rate cannot be 0 for channel {}",
                        channel.id
                    )));
                }
                if *timeout == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Timeout cannot be 0 for channel {}",
                        channel.id
                    )));
                }
                if *max_retries > 10 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Max retries should not exceed 10 for channel {}",
                        channel.id
                    )));
                }
                if let Some(rate) = poll_rate {
                    if *rate == 0 {
                        return Err(ComSrvError::ConfigError(format!(
                            "Poll rate cannot be 0 for channel {}",
                            channel.id
                        )));
                    }
                }
                if let Some(sid) = slave_id {
                    if *sid == 0 || *sid > 247 {
                        return Err(ComSrvError::ConfigError(format!(
                            "Invalid slave ID {} for channel {}. Must be between 1 and 247",
                            sid, channel.id
                        )));
                    }
                }
            }
            ChannelParameters::Virtual { max_retries, timeout } => {
                if *timeout == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Timeout cannot be 0 for channel {}",
                        channel.id
                    )));
                }
                if *max_retries > 10 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Max retries should not exceed 10 for channel {}",
                        channel.id
                    )));
                }
            }
            ChannelParameters::Generic(_) => {
                // Generic validation can be added here if needed
            }
        }
        Ok(())
    }

    /// Get a copy of the configuration (for thread safety)
    pub fn get_config_copy(&self) -> Config {
        self.config.clone()
    }

    /// Get a reference to the current configuration
    pub fn get_config(&self) -> &Config {
        &self.config
    }

    /// Get service configuration
    pub fn get_service_config(&self) -> &ServiceConfig {
        &self.config.service
    }

    /// Get service name
    pub fn get_service_name(&self) -> &str {
        &self.config.service.name
    }

    /// Get channel configurations
    pub fn get_channels(&self) -> &Vec<ChannelConfig> {
        &self.config.channels
    }

    /// Get channel configuration by ID
    ///
    /// # Arguments
    ///
    /// * `channel_id` - Channel ID to look for
    pub fn get_channel(&self, channel_id: u16) -> Option<&ChannelConfig> {
        self.config.channels.iter().find(|c| c.id == channel_id)
    }

    /// Check if API is enabled
    pub fn get_api_enabled(&self) -> bool {
        self.config.service.api.enabled
    }

    /// Get API bind address
    pub fn get_api_address(&self) -> &str {
        &self.config.service.api.bind_address
    }

    /// Get API version
    pub fn get_api_version(&self) -> &str {
        &self.config.service.api.version
    }

    /// Get log level
    pub fn get_log_level(&self) -> &str {
        &self.config.service.logging.level
    }

    /// Get log file path
    pub fn get_log_file(&self) -> &str {
        self.config.service.logging.file.as_deref().unwrap_or("")
    }

    /// Get redis configuration
    pub fn get_redis_config(&self) -> RedisConfig {
        self.config.service.redis.clone()
    }

    /// Save configuration to file
    ///
    /// # Arguments
    ///
    /// * `config_path` - Path to save the configuration to (optional, uses current path if None)
    pub fn save_config(&self, config_path: Option<&str>) -> Result<()> {
        let path = config_path.unwrap_or(&self.config_path);
        let content = serde_yaml::to_string(&self.config).map_err(|e| {
            ComSrvError::ConfigError(format!("Failed to serialize configuration: {}", e))
        })?;

        fs::write(path, content).map_err(|e| {
            ComSrvError::ConfigError(format!("Failed to write configuration to {}: {}", path, e))
        })?;

        Ok(())
    }

    /// Reload configuration from the current config file
    ///
    /// This method reloads the configuration from the current config file.
    /// It returns a Result containing a tuple:
    /// - bool: true if configuration changed, false otherwise
    /// - HashMap&lt;String, Vec&lt;String&gt;&gt;: map of changed channel IDs and their changed properties
    ///
    /// # Returns
    ///
    /// * `Ok((bool, HashMap<String, Vec<String>>))` - Whether configuration changed and what changed
    /// * `Err(ComSrvError)` - Error if configuration cannot be reloaded
    pub fn reload_config(&mut self) -> Result<(bool, HashMap<String, Vec<String>>)> {
        // Load the new configuration
        let new_config = Self::load_config(&self.config_path)?;

        // Compare configurations to detect changes
        let mut config_changed = false;
        let mut channel_changes: HashMap<String, Vec<String>> = HashMap::new();

        // Check for service config changes
        if new_config.service.description != self.config.service.description {
            config_changed = true;
        }

        // Check API config changes
        if new_config.service.api.enabled != self.config.service.api.enabled
            || new_config.service.api.bind_address != self.config.service.api.bind_address
            || new_config.service.api.version != self.config.service.api.version
        {
            config_changed = true;
        }

        // Check Redis config changes
        if new_config.service.redis.enabled != self.config.service.redis.enabled
            || match (
                &new_config.service.redis.connection_type,
                &self.config.service.redis.connection_type,
            ) {
                (RedisConnectionType::Tcp, RedisConnectionType::Tcp) => false,
                (RedisConnectionType::Unix, RedisConnectionType::Unix) => false,
                _ => true,
            }
            || new_config.service.redis.address != self.config.service.redis.address
            || new_config.service.redis.db != self.config.service.redis.db
        {
            config_changed = true;
        }

        // Check for channel changes
        for new_channel in &new_config.channels {
            // Check if channel exists in current config
            let existing_channel = self.config.channels.iter().find(|c| c.id == new_channel.id);

            let channel_id = new_channel.id.clone();
            let mut changed_properties = Vec::new();

            match existing_channel {
                Some(existing) => {
                    // Compare channel properties and update changed ones
                    if new_channel.name != existing.name {
                        changed_properties.push("name".to_string());
                    }

                    if new_channel.description != existing.description {
                        changed_properties.push("description".to_string());
                    }

                    if new_channel.protocol != existing.protocol {
                        changed_properties.push("protocol".to_string());
                    }

                    // Parameters comparison is more complex, depends on the protocol type
                    match (&new_channel.parameters, &existing.parameters) {
                        (
                            ChannelParameters::ModbusTcp {
                                host: new_host,
                                port: new_port,
                                timeout: new_timeout,
                                max_retries: new_max_retries,
                                poll_rate: new_poll_rate,
                                slave_id: new_slave_id,
                            },
                            ChannelParameters::ModbusTcp {
                                host: existing_host,
                                port: existing_port,
                                timeout: existing_timeout,
                                max_retries: existing_max_retries,
                                poll_rate: existing_poll_rate,
                                slave_id: existing_slave_id,
                            },
                        ) => {
                            if new_host != existing_host {
                                changed_properties.push("parameters.host".to_string());
                            }
                            if new_port != existing_port {
                                changed_properties.push("parameters.port".to_string());
                            }
                            if new_timeout != existing_timeout {
                                changed_properties.push("parameters.timeout".to_string());
                            }
                            if new_max_retries != existing_max_retries {
                                changed_properties.push("parameters.max_retries".to_string());
                            }
                            if new_poll_rate != existing_poll_rate {
                                changed_properties.push("parameters.poll_rate".to_string());
                            }
                            if new_slave_id != existing_slave_id {
                                changed_properties.push("parameters.slave_id".to_string());
                            }
                        }
                        (
                            ChannelParameters::ModbusRtu {
                                port: new_port,
                                baud_rate: new_baud_rate,
                                data_bits: new_data_bits,
                                parity: new_parity,
                                stop_bits: new_stop_bits,
                                timeout: new_timeout,
                                max_retries: new_max_retries,
                                poll_rate: new_poll_rate,
                                slave_id: new_slave_id,
                            },
                            ChannelParameters::ModbusRtu {
                                port: existing_port,
                                baud_rate: existing_baud_rate,
                                data_bits: existing_data_bits,
                                parity: existing_parity,
                                stop_bits: existing_stop_bits,
                                timeout: existing_timeout,
                                max_retries: existing_max_retries,
                                poll_rate: existing_poll_rate,
                                slave_id: existing_slave_id,
                            },
                        ) => {
                            if new_port != existing_port {
                                changed_properties.push("parameters.port".to_string());
                            }
                            if new_baud_rate != existing_baud_rate {
                                changed_properties.push("parameters.baud_rate".to_string());
                            }
                            if new_data_bits != existing_data_bits {
                                changed_properties.push("parameters.data_bits".to_string());
                            }
                            if new_parity != existing_parity {
                                changed_properties.push("parameters.parity".to_string());
                            }
                            if new_stop_bits != existing_stop_bits {
                                changed_properties.push("parameters.stop_bits".to_string());
                            }
                            if new_timeout != existing_timeout {
                                changed_properties.push("parameters.timeout".to_string());
                            }
                            if new_max_retries != existing_max_retries {
                                changed_properties.push("parameters.max_retries".to_string());
                            }
                            if new_poll_rate != existing_poll_rate {
                                changed_properties.push("parameters.poll_rate".to_string());
                            }
                            if new_slave_id != existing_slave_id {
                                changed_properties.push("parameters.slave_id".to_string());
                            }
                        }
                        (
                            ChannelParameters::Virtual { max_retries: new_max_retries, timeout: new_timeout },
                            ChannelParameters::Virtual { max_retries: existing_max_retries, timeout: existing_timeout },
                        ) => {
                            if new_timeout != existing_timeout {
                                changed_properties.push("parameters.timeout".to_string());
                            }
                            if new_max_retries != existing_max_retries {
                                changed_properties.push("parameters.max_retries".to_string());
                            }
                        }
                        (
                            ChannelParameters::Generic(new_params),
                            ChannelParameters::Generic(existing_params),
                        ) => {
                            // Compare each parameter in the generic map
                            for (key, new_value) in new_params {
                                match existing_params.get(key) {
                                    Some(existing_value) if existing_value != new_value => {
                                        changed_properties.push(format!("parameters.{}", key));
                                    }
                                    None => {
                                        changed_properties.push(format!("parameters.{}", key));
                                    }
                                    _ => {}
                                }
                            }

                            // Check for removed parameters
                            for key in existing_params.keys() {
                                if !new_params.contains_key(key) {
                                    changed_properties.push(format!("parameters.{}.removed", key));
                                }
                            }
                        }
                        _ => {
                            // Different parameter types, consider all parameters changed
                            changed_properties.push("parameters".to_string());
                        }
                    }
                }
                None => {
                    // New channel
                    changed_properties.push("new_channel".to_string());
                }
            }

            // If any properties changed, add to the changes map
            if !changed_properties.is_empty() {
                config_changed = true;
                channel_changes.insert(channel_id.to_string(), changed_properties);
            }
        }

        // Check for removed channels
        for existing_channel in &self.config.channels {
            let channel_exists = new_config
                .channels
                .iter()
                .any(|c| c.id == existing_channel.id);

            if !channel_exists {
                config_changed = true;
                channel_changes
                    .insert(existing_channel.id.to_string(), vec!["removed".to_string()]);
            }
        }

        // Update config if changed
        if config_changed {
            self.config = new_config;
        }

        Ok((config_changed, channel_changes))
    }

    /// Get CSV point manager reference
    pub fn get_csv_point_manager(&self) -> &FourTelemetryTableManager {
        &self.csv_point_manager
    }

    /// Get mutable CSV point manager reference
    pub fn get_csv_point_manager_mut(&mut self) -> &mut FourTelemetryTableManager {
        &mut self.csv_point_manager
    }

    /// Get point table manager (new storage abstraction)
    pub fn get_point_table_manager(&self) -> &FourTelemetryTableManager {
        &self.csv_point_manager
    }

    /// Get mutable point table manager (new storage abstraction)
    pub fn get_point_table_manager_mut(&mut self) -> &mut FourTelemetryTableManager {
        &mut self.csv_point_manager
    }

    /// Reload CSV point tables
    pub fn reload_csv_point_tables(&mut self) -> Result<()> {
        // In the new architecture, CSV tables are loaded per-channel, not globally
        // This method is kept for compatibility but does nothing
        log::info!("CSV point tables are now managed per-channel, global reload not needed");
        Ok(())
    }

    /// Get point table names
    pub fn get_point_table_names(&self) -> Vec<String> {
        // For backward compatibility, return empty list for now
        // TODO: Consider making this method async or using a different approach
        Vec::new()
    }

    /// Get Modbus mappings for a specific channel
    pub fn get_modbus_mappings_for_channel(
        &self,
        channel_id: u16,
    ) -> Result<Vec<crate::core::protocols::modbus::common::ModbusRegisterMapping>> {
        let channel = self
            .get_channel(channel_id)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Channel {} not found", channel_id)))?;

        // Try to get point table from channel parameters
        if let Some(table_name) = self.get_channel_point_table(&channel.parameters) {
            self.csv_point_manager.to_modbus_mappings(&table_name)
        } else {
            // Return empty mappings if no point table is configured
            Ok(Vec::new())
        }
    }

    /// Get point table name from channel parameters
    fn get_channel_point_table(&self, parameters: &ChannelParameters) -> Option<String> {
        match parameters {
            ChannelParameters::ModbusTcp { .. } => {
                // For typed parameters, we don't store point table names in parameters anymore
                // Point tables are managed through the new ChannelPointTableConfig
                None
            }
            ChannelParameters::ModbusRtu { .. } => {
                // For typed parameters, we don't store point table names in parameters anymore
                // Point tables are managed through the new ChannelPointTableConfig
                None
            }
            ChannelParameters::Virtual { .. } => {
                // For typed parameters, we don't store point table names in parameters anymore
                // Point tables are managed through the new ChannelPointTableConfig
                None
            }
            ChannelParameters::Generic(map) => {
                // Look for point_table parameter in generic parameters
                map.get("point_table")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }
        }
    }

    /// Update channel point table mapping
    pub fn update_channel_point_table(
        &mut self,
        channel_id: u16,
        table_name: String,
    ) -> Result<()> {
        let channel = self
            .get_channels_mut()
            .iter_mut()
            .find(|c| c.id == channel_id)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Channel {} not found", channel_id)))?;

        match &mut channel.parameters {
            ChannelParameters::Generic(map) => {
                map.insert(
                    "point_table".to_string(),
                    serde_yaml::Value::String(table_name),
                );
            }
            _ => {
                // For typed parameters, point tables are now managed through ChannelPointTableConfig
                // This method is kept for backward compatibility but doesn't modify typed parameters
                log::warn!("Point table updates for typed parameters are no longer supported. Use ChannelPointTableConfig instead.");
            }
        }

        Ok(())
    }

    /// Get mutable channels reference
    fn get_channels_mut(&mut self) -> &mut Vec<ChannelConfig> {
        &mut self.config.channels
    }

    /// Get CSV configuration for a specific channel (deprecated - use get_channel_point_table_config instead)
    pub fn get_channel_csv_config(&self, channel_id: u16) -> Option<&ChannelCsvConfig> {
        self.get_channel(channel_id)?.csv_config.as_ref()
    }

    /// Get point table configuration for a specific channel
    pub fn get_channel_point_table_config(&self, channel_id: u16) -> Option<&ChannelPointTableConfig> {
        self.get_channel(channel_id)?.point_table.as_ref()
    }

    /// Get the CSV path for a specific channel and telemetry type
    pub fn get_channel_csv_path(
        &self,
        channel_id: u16,
        telemetry_type: TelemetryType,
    ) -> Option<PathBuf> {
        let csv_config = self.get_channel_csv_config(channel_id)?;
        csv_config.get_csv_path(telemetry_type, channel_id)
    }

    /// Check if a channel has complete CSV configuration
    pub fn is_channel_csv_complete(&self, channel_id: u16) -> bool {
        self.get_channel_csv_config(channel_id)
            .map(|config| config.is_complete())
            .unwrap_or(false)
    }

    /// Get all channels with CSV configuration
    pub fn get_channels_with_csv(&self) -> Vec<&ChannelConfig> {
        self.config
            .channels
            .iter()
            .filter(|channel| channel.csv_config.is_some())
            .collect()
    }

    /// Load ComBase point configurations for a specific channel
    pub fn load_channel_combase_config(&self, channel_id: u16) -> Result<CombaseConfigManager> {
        let csv_config = self.get_channel_csv_config(channel_id).ok_or_else(|| {
            ComSrvError::ConfigError(format!("No CSV config found for channel {}", channel_id))
        })?;

        let base_dir =
            PathBuf::from(&csv_config.csv_directory).join(format!("channel_{}", channel_id));
        let mut combase_manager = CombaseConfigManager::new(base_dir);
        combase_manager.load_all_configs()?;
        Ok(combase_manager)
    }

    /// Load configuration from file with enhanced error handling
    fn load_config(config_path: &str) -> Result<Config> {
        // Check if file exists
        if !std::path::Path::new(config_path).exists() {
            return Err(ComSrvError::ConfigError(format!(
                "Configuration file not found: {}",
                config_path
            )));
        }

        // Read file content
        let content = fs::read_to_string(config_path).map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to read configuration file {}: {}",
                config_path, e
            ))
        })?;

        // Parse YAML content
        let config: Config = serde_yaml::from_str(&content).map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to parse configuration file {}: {}",
                config_path, e
            ))
        })?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Create a test configuration file
    fn create_test_config_file(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
        let config_path = dir.join("test_comsrv.yaml");
        fs::write(&config_path, content).expect("Failed to write test config");
        config_path
    }

    #[test]
    fn test_config_manager_creation() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let config_content = r#"
version: "1.0"
service:
  name: "test_service"
  description: "Test service"
channels: []
"#;

        let config_path = create_test_config_file(temp_dir.path(), config_content);
        let manager =
            ConfigManager::from_file(&config_path).expect("Failed to create config manager");

        assert_eq!(manager.get_service_name(), "test_service");

        assert!(manager.get_api_enabled());
        assert_eq!(manager.get_channels().len(), 0);
    }

    #[test]
    fn test_config_validation() {
        let temp_dir = tempdir().expect("Failed to create temp dir");

        // Valid configuration test
        let valid_config = r#"
version: "1.0"
service:
  name: "valid_service"
  description: "Valid test service"
channels:
  - id: 1
    name: "Test Channel"
    description: "Test channel"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout: 5000
      max_retries: 3
      point_tables: {}
      poll_rate: 1000
"#;

        let config_path = create_test_config_file(temp_dir.path(), valid_config);
        let manager = ConfigManager::from_file(&config_path).expect("Failed to load valid config");
        assert!(manager.validate_config().is_ok());

        // Invalid configuration test - duplicate channel ID
        let invalid_config = r#"
version: "1.0"
service:
  name: "invalid_service"
  description: "Invalid test service"
channels:
  - id: 1
    name: "Channel 1"
    description: "First channel"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout: 5000
      max_retries: 3
      point_tables: {}
      poll_rate: 1000
  - id: 1
    name: "Channel 2"
    description: "Second channel with duplicate ID"
    protocol: "ModbusRtu"
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      parity: "None"
      stop_bits: 1
      timeout: 5000
      max_retries: 3
      point_tables: {}
      poll_rate: 1000
      slave_id: 1
"#;

        let invalid_config_path = temp_dir.path().join("invalid_config.yaml");
        fs::write(&invalid_config_path, invalid_config).expect("Failed to write invalid config");

        // This configuration should fail due to duplicate channel ID
        let result = ConfigManager::from_file(&invalid_config_path);
        assert!(result.is_err());
        if let Err(e) = result {
            let error_message = format!("{}", e);
            assert!(error_message.contains("Duplicate channel ID found: 1"));
        }
    }

    #[test]
    fn test_channel_operations() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let config_content = r#"
version: "1.0"
service:
  name: "test_service"
  description: "Test service"
channels:
  - id: 1
    name: "TCP Test Channel"
    description: "Test TCP channel"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout: 5000
      max_retries: 3
      point_tables: {}
      poll_rate: 1000
  - id: 2
    name: "RTU Test Channel"
    description: "Test RTU channel"
    protocol: "ModbusRtu"
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      parity: "None"
      stop_bits: 1
      timeout: 5000
      max_retries: 3
      point_tables: {}
      poll_rate: 1000
      slave_id: 1
"#;

        let config_path = create_test_config_file(temp_dir.path(), config_content);
        let manager = ConfigManager::from_file(&config_path).expect("Failed to load config");

        // Verify channel count
        assert_eq!(manager.get_channels().len(), 2);

        // Verify TCP channel
        let tcp_channel = manager.get_channel(1).expect("TCP channel should exist");
        assert_eq!(tcp_channel.name, "TCP Test Channel");
        assert_eq!(tcp_channel.protocol, ProtocolType::ModbusTcp);

        // Verify RTU channel
        let rtu_channel = manager.get_channel(2).expect("RTU channel should exist");
        assert_eq!(rtu_channel.name, "RTU Test Channel");
        assert_eq!(rtu_channel.protocol, ProtocolType::ModbusRtu);

        // Verify non-existent channel returns None
        assert!(manager.get_channel(999).is_none());
    }

    #[test]
    fn test_protocol_type_conversion() {
        let protocols = vec![
            ("ModbusTcp", ProtocolType::ModbusTcp),
            ("ModbusRtu", ProtocolType::ModbusRtu),
            ("Virtual", ProtocolType::Virtual),
            ("Dio", ProtocolType::Dio),
            ("Can", ProtocolType::Can),
            ("Iec104", ProtocolType::Iec104),
            ("Iec61850", ProtocolType::Iec61850),
        ];

        for (str_repr, enum_val) in protocols {
            // Test conversion from string to enum
            let parsed = ProtocolType::from_str(str_repr).expect("Failed to parse protocol type");
            assert_eq!(parsed, enum_val);

            // Test conversion from enum to string
            assert_eq!(enum_val.as_str(), str_repr);

            // Test Display trait
            assert_eq!(format!("{}", enum_val), str_repr);
        }

        // Test invalid protocol type handling
        assert!(ProtocolType::from_str("InvalidProtocol").is_err());
    }

    #[test]
    fn test_channel_parameters_get() {
        // Test retrieval of ModbusTcp parameters
        let tcp_params = ChannelParameters::ModbusTcp {
            host: "192.168.1.100".to_string(),
            port: 502,
            timeout: 5000,
            max_retries: 3,
            poll_rate: Some(1000),
            slave_id: Some(1),
        };

        if let Some(host) = tcp_params.get("host") {
            assert_eq!(host.as_str().unwrap(), "192.168.1.100");
        } else {
            panic!("Host parameter not found");
        }

        if let Some(port) = tcp_params.get("port") {
            assert_eq!(port.as_u64().unwrap(), 502);
        } else {
            panic!("Port parameter not found");
        }

        // Test retrieval of ModbusRtu parameters
        let rtu_params = ChannelParameters::ModbusRtu {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            data_bits: 8,
            parity: "None".to_string(),
            stop_bits: 1,
            timeout: 5000,
            max_retries: 3,
            poll_rate: Some(1000),
            slave_id: Some(1),
        };

        if let Some(port) = rtu_params.get("port") {
            assert_eq!(port.as_str().unwrap(), "/dev/ttyUSB0");
        } else {
            panic!("Port parameter not found");
        }

        // Test retrieval of Generic parameters
        let mut generic_map = HashMap::new();
        generic_map.insert(
            "custom_param".to_string(),
            serde_yaml::Value::String("test_value".to_string()),
        );
        let generic_params = ChannelParameters::Generic(generic_map);

        if let Some(custom_param) = generic_params.get("custom_param") {
            assert_eq!(custom_param.as_str().unwrap(), "test_value");
        } else {
            panic!("Custom parameter not found");
        }

        // Test retrieval of a nonexistent parameter
        assert!(tcp_params.get("nonexistent_param").is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            version: "1.0".to_string(),
            service: ServiceConfig {
                name: "test_service".to_string(),
                description: Some("Test service for serialization".to_string()),
                logging: LoggingConfig::default(),
                api: ApiConfig::default(),
                redis: RedisConfig::default(),
            },
            defaults: DefaultPathConfig::default(),
            channels: vec![
                ChannelConfig {
                    id: 1,
                    name: "Test TCP Channel".to_string(),
                    description: Some("TCP test channel".to_string()),
                    protocol: ProtocolType::ModbusTcp,
                    parameters: ChannelParameters::ModbusTcp {
                        host: "192.168.1.100".to_string(),
                        port: 502,
                        timeout: 5000,
                        max_retries: 3,
                        poll_rate: Some(1000),
                        slave_id: Some(1),
                    },
                    point_table: Some(ChannelPointTableConfig::default()),
                    source_tables: None,
                    csv_config: Some(ChannelCsvConfig::default()),
                },
                ChannelConfig {
                    id: 2,
                    name: "Test RTU Channel".to_string(),
                    description: Some("RTU test channel".to_string()),
                    protocol: ProtocolType::ModbusRtu,
                    parameters: ChannelParameters::ModbusRtu {
                        port: "/dev/ttyUSB0".to_string(),
                        baud_rate: 9600,
                        data_bits: 8,
                        parity: "None".to_string(),
                        stop_bits: 1,
                        timeout: 5000,
                        max_retries: 3,
                        poll_rate: Some(1000),
                        slave_id: Some(1),
                    },
                    point_table: Some(ChannelPointTableConfig::default()),
                    source_tables: None,
                    csv_config: Some(ChannelCsvConfig::default()),
                },
            ],
        };

        // Serialize configuration
        let serialized = serde_yaml::to_string(&config).expect("Failed to serialize config");
        assert!(!serialized.is_empty());

        // Deserialize configuration
        let deserialized: Config =
            serde_yaml::from_str(&serialized).expect("Failed to deserialize config");

        // Verify the deserialized result
        assert_eq!(config.version, deserialized.version);
        assert_eq!(config.service.name, deserialized.service.name);
        assert_eq!(config.channels.len(), deserialized.channels.len());

        for (original, deserialized) in config.channels.iter().zip(deserialized.channels.iter()) {
            assert_eq!(original.id, deserialized.id);
            assert_eq!(original.name, deserialized.name);
            assert_eq!(original.protocol, deserialized.protocol);
        }
    }

    #[test]
    fn test_redis_config() {
        // Test TCP connection URL generation
        let tcp_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "127.0.0.1:6379".to_string(),
            db: 0,
            timeout_ms: 5000,
            max_connections: None,
            min_connections: None,
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        let tcp_url = tcp_config.to_redis_url();
        assert_eq!(tcp_url, "redis://127.0.0.1:6379/0");

        // Test Unix socket connection URL generation
        let unix_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Unix,
            address: "/tmp/redis.sock".to_string(),
            db: 0,
            timeout_ms: 5000,
            max_connections: None,
            min_connections: None,
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        let unix_url = unix_config.to_redis_url();
        assert_eq!(unix_url, "unix:///tmp/redis.sock");

        // Test TCP connection without specifying database
        let tcp_no_db_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "127.0.0.1:6379".to_string(),
            db: 0,
            timeout_ms: 5000,
            max_connections: None,
            min_connections: None,
            idle_timeout_secs: 300,
            max_retries: 3,
            password: None,
            username: None,
        };

        let tcp_no_db_url = tcp_no_db_config.to_redis_url();
        assert_eq!(tcp_no_db_url, "redis://127.0.0.1:6379");
    }

    #[test]
    fn test_config_reload() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let initial_config = r#"
version: "1.0"
service:
  name: "initial_service"
  description: "Initial service"
channels:
  - id: 1
    name: "Initial Channel"
    description: "Initial channel"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout: 5000
      max_retries: 3
      point_tables: {}
      poll_rate: 1000
"#;

        let config_path = create_test_config_file(temp_dir.path(), initial_config);
        let mut manager =
            ConfigManager::from_file(&config_path).expect("Failed to load initial config");

        assert_eq!(manager.get_service_name(), "initial_service");
        assert_eq!(manager.get_channels().len(), 1);

        // Update the configuration file
        let updated_config = r#"
version: "1.0"
service:
  name: "updated_service"
  description: "Updated service"
channels:
  - id: 1
    name: "Updated Channel"
    description: "Updated channel"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.200"
      port: 503
      timeout: 6000
      max_retries: 5
      point_tables: {}
      poll_rate: 2000
  - id: 2
    name: "New Channel"
    description: "Newly added channel"
    protocol: "ModbusRtu"
    parameters:
      port: "/dev/ttyUSB1"
      baud_rate: 19200
      data_bits: 8
      parity: "None"
      stop_bits: 1
      timeout: 5000
      max_retries: 3
      point_tables: {}
      poll_rate: 1000
      slave_id: 2
"#;

        fs::write(&config_path, updated_config).expect("Failed to update config file");

        // Reload configuration
        let (config_changed, changes) = manager.reload_config().expect("Failed to reload config");

        assert!(config_changed);
        assert!(!changes.is_empty());
        assert_eq!(manager.get_service_name(), "updated_service");
        assert_eq!(manager.get_channels().len(), 2);

        let updated_channel = manager.get_channel(1).expect("Channel 1 should exist");
        assert_eq!(updated_channel.name, "Updated Channel");

        let new_channel = manager.get_channel(2).expect("Channel 2 should exist");
        assert_eq!(new_channel.name, "New Channel");
        assert_eq!(new_channel.protocol, ProtocolType::ModbusRtu);
    }
}

// ======== Combase Layer Configuration Management ========
// 以下为四遥配置管理相关类型和实现

/// Four telemetry types in industrial automation
/// 四遥类型定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TelemetryType {
    /// 遥测 - Analog measurement data (float)
    Telemetry,
    /// 遥信 - Digital signaling data (bool)
    Signaling,
    /// 遥控 - Digital control commands (bool)
    Control,
    /// 遥调 - Analog setpoint data (float)
    Setpoint,
}

impl TelemetryType {
    /// Get the corresponding CSV file name for this telemetry type
    pub fn csv_filename(&self) -> &'static str {
        match self {
            TelemetryType::Telemetry => "telemetry.csv",
            TelemetryType::Signaling => "signaling.csv",
            TelemetryType::Control => "control.csv",
            TelemetryType::Setpoint => "setpoint.csv",
        }
    }

    /// Parse telemetry type from CSV filename
    pub fn from_filename(filename: &str) -> Option<Self> {
        match filename {
            "telemetry_table.csv" => Some(TelemetryType::Telemetry),
            "signaling_table.csv" => Some(TelemetryType::Signaling),
            "control_table.csv" => Some(TelemetryType::Control),
            "setpoint_table.csv" => Some(TelemetryType::Setpoint),
            _ => None,
        }
    }

    /// Check if this telemetry type uses analog data (float)
    pub fn is_analog(&self) -> bool {
        matches!(self, TelemetryType::Telemetry | TelemetryType::Setpoint)
    }

    /// Check if this telemetry type uses digital data (bool)
    pub fn is_digital(&self) -> bool {
        matches!(self, TelemetryType::Signaling | TelemetryType::Control)
    }

    /// Get the unified data type for this telemetry type
    pub fn data_type(&self) -> CombaseDataType {
        if self.is_analog() {
            CombaseDataType::Float
        } else {
            CombaseDataType::Bool
        }
    }
}

/// Unified data types in Combase layer
/// Combase层统一数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombaseDataType {
    /// Floating point number for analog data
    Float,
    /// Boolean value for digital data
    Bool,
}

/// 数据来源类型枚举（简化版本）
/// Data Source Type Enumeration (Simplified)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataSourceType {
    /// 协议数据 - 通过协议配置ID关联
    Protocol {
        /// 协议配置ID/索引
        config_id: String,
    },
    /// 计算数据 - 通过计算配置ID关联
    Calculation {
        /// 计算配置ID
        calculation_id: String,
    },
    /// 手动输入 - 简单的手动值配置
    Manual {
        /// 是否允许运行时修改
        editable: bool,
        /// 默认值（可选）
        default_value: Option<serde_json::Value>,
    },
}

impl DataSourceType {
    /// 检查数据来源是否为只读
    pub fn is_read_only(&self) -> bool {
        match self {
            DataSourceType::Protocol { .. } => true,
            DataSourceType::Calculation { .. } => true,
            DataSourceType::Manual { editable, .. } => !editable,
        }
    }

    /// 获取数据来源的显示名称
    pub fn display_name(&self) -> String {
        match self {
            DataSourceType::Protocol { config_id } => {
                format!("协议({})", config_id)
            }
            DataSourceType::Calculation { calculation_id } => {
                format!("计算({})", calculation_id)
            }
            DataSourceType::Manual { editable, .. } => {
                if *editable {
                    "手动(可编辑)".to_string()
                } else {
                    "手动(固定)".to_string()
                }
            }
        }
    }

    /// 获取关联的配置ID
    pub fn get_config_id(&self) -> Option<&str> {
        match self {
            DataSourceType::Protocol { config_id } => Some(config_id),
            DataSourceType::Calculation { calculation_id } => Some(calculation_id),
            DataSourceType::Manual { .. } => None,
        }
    }

    /// 验证数据来源配置的有效性
    pub fn validate(&self) -> Result<()> {
        match self {
            DataSourceType::Protocol { config_id } => {
                if config_id.is_empty() {
                    return Err(ComSrvError::ConfigError(
                        "Protocol config ID cannot be empty".to_string(),
                    ));
                }
            }
            DataSourceType::Calculation { calculation_id } => {
                if calculation_id.is_empty() {
                    return Err(ComSrvError::ConfigError(
                        "Calculation ID cannot be empty".to_string(),
                    ));
                }
            }
            DataSourceType::Manual { .. } => {
                // Manual data source is always valid
            }
        }
        Ok(())
    }
}

impl Default for DataSourceType {
    fn default() -> Self {
        DataSourceType::Manual {
            editable: true,
            default_value: None,
        }
    }
}

/// Analog point configuration (Telemetry & Setpoint)
/// 模拟量点位配置（遥测和遥调）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalogPointConfig {
    /// Point ID (unique within table)
    pub id: u32,
    /// English name
    pub name: String,
    /// Chinese name
    pub chinese_name: String,
    /// Data source type 
    #[serde(default)]
    pub data_source: DataSourceType,
    /// Scale factor for engineering unit conversion
    pub scale: f64,
    /// Offset for engineering unit conversion
    pub offset: f64,
    /// Engineering unit
    pub unit: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Group identifier
    pub group: Option<String>,
}

impl AnalogPointConfig {
    /// Convert raw protocol value to engineering units
    pub fn convert_to_engineering(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }

    /// Convert engineering units to raw protocol value
    pub fn convert_from_engineering(&self, engineering_value: f64) -> f64 {
        (engineering_value - self.offset) / self.scale
    }
}

/// Digital point configuration (Signaling & Control)
/// 数字量点位配置（遥信和遥控）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitalPointConfig {
    /// Point ID (unique within table)
    pub id: u32,
    /// English name
    pub name: String,
    /// Chinese name
    pub chinese_name: String,
    /// Data source type
    #[serde(default)]
    pub data_source: DataSourceType,
    /// Description
    pub description: Option<String>,
    /// Group identifier
    pub group: Option<String>,
}

/// Combase point configuration union
/// Combase点位配置联合体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CombasePointConfig {
    /// Analog point (Telemetry/Setpoint)
    Analog(AnalogPointConfig),
    /// Digital point (Signaling/Control)
    Digital(DigitalPointConfig),
}

impl CombasePointConfig {
    /// Get point ID
    pub fn id(&self) -> u32 {
        match self {
            CombasePointConfig::Analog(config) => config.id,
            CombasePointConfig::Digital(config) => config.id,
        }
    }

    /// Get point name
    pub fn name(&self) -> &str {
        match self {
            CombasePointConfig::Analog(config) => &config.name,
            CombasePointConfig::Digital(config) => &config.name,
        }
    }

    /// Get Chinese name
    pub fn chinese_name(&self) -> &str {
        match self {
            CombasePointConfig::Analog(config) => &config.chinese_name,
            CombasePointConfig::Digital(config) => &config.chinese_name,
        }
    }

    /// Get data type
    pub fn data_type(&self) -> CombaseDataType {
        match self {
            CombasePointConfig::Analog(_) => CombaseDataType::Float,
            CombasePointConfig::Digital(_) => CombaseDataType::Bool,
        }
    }
}

/// Combase configuration manager
/// Combase配置管理器
#[derive(Debug, Clone)]
pub struct CombaseConfigManager {
    /// Point configurations grouped by telemetry type
    point_configs: HashMap<TelemetryType, HashMap<u32, CombasePointConfig>>,
    /// Configuration directory path
    config_dir: PathBuf,
}

impl CombaseConfigManager {
    /// Create a new Combase configuration manager
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        Self {
            point_configs: HashMap::new(),
            config_dir: config_dir.as_ref().to_path_buf(),
        }
    }

    /// Load all configuration files from the directory
    pub fn load_all_configs(&mut self) -> Result<()> {
        for telemetry_type in &[
            TelemetryType::Telemetry,
            TelemetryType::Signaling,
            TelemetryType::Control,
            TelemetryType::Setpoint,
        ] {
            self.load_config_for_type(*telemetry_type)?;
        }
        Ok(())
    }

    /// Load configuration for a specific telemetry type
    pub fn load_config_for_type(&mut self, telemetry_type: TelemetryType) -> Result<()> {
        let filename = telemetry_type.csv_filename();
        let file_path = self.config_dir.join(filename);

        if !file_path.exists() {
            log::warn!("Config file not found: {}", file_path.display());
            return Ok(());
        }

        let mut reader = csv::Reader::from_path(&file_path).map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to open CSV file {}: {}",
                file_path.display(),
                e
            ))
        })?;

        let mut configs = HashMap::new();

        if telemetry_type.is_analog() {
            // Load analog point configurations
            for result in reader.deserialize() {
                let record: AnalogPointConfig = result.map_err(|e| {
                    ComSrvError::ConfigError(format!(
                        "Failed to parse analog config in {}: {}",
                        filename, e
                    ))
                })?;

                configs.insert(record.id, CombasePointConfig::Analog(record));
            }
        } else {
            // Load digital point configurations
            for result in reader.deserialize() {
                let record: DigitalPointConfig = result.map_err(|e| {
                    ComSrvError::ConfigError(format!(
                        "Failed to parse digital config in {}: {}",
                        filename, e
                    ))
                })?;

                configs.insert(record.id, CombasePointConfig::Digital(record));
            }
        }

        self.point_configs.insert(telemetry_type, configs);
        log::info!(
            "Loaded {} {} points from {}",
            self.point_configs[&telemetry_type].len(),
            format!("{:?}", telemetry_type).to_lowercase(),
            filename
        );

        Ok(())
    }

    /// Get point configuration by telemetry type and ID
    pub fn get_point_config(
        &self,
        telemetry_type: TelemetryType,
        point_id: u32,
    ) -> Option<&CombasePointConfig> {
        self.point_configs.get(&telemetry_type)?.get(&point_id)
    }

    /// Get all point configurations for a telemetry type
    pub fn get_points_by_type(
        &self,
        telemetry_type: TelemetryType,
    ) -> Option<&HashMap<u32, CombasePointConfig>> {
        self.point_configs.get(&telemetry_type)
    }

    /// Get all telemetry types that have loaded configurations
    pub fn get_loaded_types(&self) -> Vec<TelemetryType> {
        self.point_configs.keys().copied().collect()
    }

    /// Get statistics for loaded configurations
    pub fn get_statistics(&self) -> CombaseStatistics {
        let mut stats = CombaseStatistics::default();

        for (telemetry_type, configs) in &self.point_configs {
            let count = configs.len() as u32;
            match telemetry_type {
                TelemetryType::Telemetry => stats.telemetry_points = count,
                TelemetryType::Signaling => stats.signaling_points = count,
                TelemetryType::Control => stats.control_points = count,
                TelemetryType::Setpoint => stats.setpoint_points = count,
            }
        }

        stats.total_points = stats.telemetry_points
            + stats.signaling_points
            + stats.control_points
            + stats.setpoint_points;
        stats.analog_points = stats.telemetry_points + stats.setpoint_points;
        stats.digital_points = stats.signaling_points + stats.control_points;

        stats
    }

    /// Validate all loaded configurations
    pub fn validate_configs(&self) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        for (telemetry_type, configs) in &self.point_configs {
            // Check for duplicate names within the same type
            let mut names = std::collections::HashSet::new();
            for config in configs.values() {
                if !names.insert(config.name()) {
                    warnings.push(format!(
                        "Duplicate point name '{}' in {:?} table",
                        config.name(),
                        telemetry_type
                    ));
                }
            }

            // Check for missing required fields
            for config in configs.values() {
                if config.name().is_empty() {
                    warnings.push(format!(
                        "Empty name for point ID {} in {:?} table",
                        config.id(),
                        telemetry_type
                    ));
                }
                if config.chinese_name().is_empty() {
                    warnings.push(format!(
                        "Empty Chinese name for point '{}' in {:?} table",
                        config.name(),
                        telemetry_type
                    ));
                }
            }
        }

        Ok(warnings)
    }
}

/// Combase configuration statistics
/// Combase配置统计信息
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CombaseStatistics {
    /// Total number of points
    pub total_points: u32,
    /// Number of telemetry points
    pub telemetry_points: u32,
    /// Number of signaling points
    pub signaling_points: u32,
    /// Number of control points
    pub control_points: u32,
    /// Number of setpoint points
    pub setpoint_points: u32,
    /// Total analog points (telemetry + setpoint)
    pub analog_points: u32,
    /// Total digital points (signaling + control)
    pub digital_points: u32,
}

#[cfg(test)]
mod combase_tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_telemetry_type_filename() {
        assert_eq!(TelemetryType::Telemetry.csv_filename(), "telemetry.csv");
        assert_eq!(TelemetryType::Signaling.csv_filename(), "signaling.csv");
        assert_eq!(TelemetryType::Control.csv_filename(), "control.csv");
        assert_eq!(TelemetryType::Setpoint.csv_filename(), "setpoint.csv");
    }

    #[test]
    fn test_telemetry_type_from_filename() {
        assert_eq!(
            TelemetryType::from_filename("telemetry_table.csv"),
            Some(TelemetryType::Telemetry)
        );
        assert_eq!(
            TelemetryType::from_filename("signaling_table.csv"),
            Some(TelemetryType::Signaling)
        );
        assert_eq!(TelemetryType::from_filename("unknown.csv"), None);
    }

    #[test]
    fn test_telemetry_type_data_types() {
        assert!(TelemetryType::Telemetry.is_analog());
        assert!(TelemetryType::Setpoint.is_analog());
        assert!(TelemetryType::Signaling.is_digital());
        assert!(TelemetryType::Control.is_digital());

        assert_eq!(TelemetryType::Telemetry.data_type(), CombaseDataType::Float);
        assert_eq!(TelemetryType::Signaling.data_type(), CombaseDataType::Bool);
    }

    #[test]
    fn test_analog_point_conversion() {
        let config = AnalogPointConfig {
            id: 1,
            name: "test".to_string(),
            chinese_name: "测试".to_string(),
            data_source: DataSourceType::default(),
            scale: 0.1,
            offset: 10.0,
            unit: Some("°C".to_string()),
            description: None,
            group: None,
        };

        // Raw value 100 -> 100 * 0.1 + 10.0 = 20.0
        assert_eq!(config.convert_to_engineering(100.0), 20.0);

        // Engineering value 20.0 -> (20.0 - 10.0) / 0.1 = 100.0
        assert_eq!(config.convert_from_engineering(20.0), 100.0);
    }

    #[test]
    fn test_combase_config_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let manager = CombaseConfigManager::new(temp_dir.path());

        assert_eq!(manager.config_dir, temp_dir.path());
        assert_eq!(manager.point_configs.len(), 0);
    }

    #[test]
    fn test_load_analog_config() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("telemetry_table.csv");

        let csv_content = r#"id,name,chinese_name,scale,offset,unit,description,group
1,Tank_Temperature,储罐温度,0.1,0.0,°C,主储罐温度传感器,sensors
2,Tank_Pressure,储罐压力,0.01,0.0,bar,储罐压力传感器,sensors"#;

        fs::write(&config_path, csv_content).unwrap();

        let mut manager = CombaseConfigManager::new(temp_dir.path());
        manager
            .load_config_for_type(TelemetryType::Telemetry)
            .unwrap();

        let configs = manager
            .get_points_by_type(TelemetryType::Telemetry)
            .unwrap();
        assert_eq!(configs.len(), 2);

        let point_1 = manager
            .get_point_config(TelemetryType::Telemetry, 1)
            .unwrap();
        assert_eq!(point_1.name(), "Tank_Temperature");
        assert_eq!(point_1.chinese_name(), "储罐温度");
    }

    #[test]
    fn test_load_digital_config() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("signaling_table.csv");

        let csv_content = r#"id,name,chinese_name,description,group
1,Main_Pump_Running,主泵运行,主泵运行状态,pump_status
2,Backup_Pump_Running,备用泵运行,备用泵运行状态,pump_status"#;

        fs::write(&config_path, csv_content).unwrap();

        let mut manager = CombaseConfigManager::new(temp_dir.path());
        manager
            .load_config_for_type(TelemetryType::Signaling)
            .unwrap();

        let configs = manager
            .get_points_by_type(TelemetryType::Signaling)
            .unwrap();
        assert_eq!(configs.len(), 2);

        let point_1 = manager
            .get_point_config(TelemetryType::Signaling, 1)
            .unwrap();
        assert_eq!(point_1.name(), "Main_Pump_Running");
        assert_eq!(point_1.data_type(), CombaseDataType::Bool);
    }

    #[test]
    fn test_statistics() {
        let temp_dir = tempdir().unwrap();
        let mut manager = CombaseConfigManager::new(temp_dir.path());

        // Create test configs
        let mut telemetry_configs = HashMap::new();
        telemetry_configs.insert(
            1,
            CombasePointConfig::Analog(AnalogPointConfig {
                id: 1,
                name: "test1".to_string(),
                chinese_name: "测试1".to_string(),
                data_source: DataSourceType::default(),
                scale: 1.0,
                offset: 0.0,
                unit: None,
                description: None,
                group: None,
            }),
        );

        let mut signaling_configs = HashMap::new();
        signaling_configs.insert(
            1,
            CombasePointConfig::Digital(DigitalPointConfig {
                id: 1,
                name: "test2".to_string(),
                chinese_name: "测试2".to_string(),
                data_source: DataSourceType::default(),
                description: None,
                group: None,
            }),
        );
        signaling_configs.insert(
            2,
            CombasePointConfig::Digital(DigitalPointConfig {
                id: 2,
                name: "test3".to_string(),
                chinese_name: "测试3".to_string(),
                data_source: DataSourceType::default(),
                description: None,
                group: None,
            }),
        );

        manager
            .point_configs
            .insert(TelemetryType::Telemetry, telemetry_configs);
        manager
            .point_configs
            .insert(TelemetryType::Signaling, signaling_configs);

        let stats = manager.get_statistics();
        assert_eq!(stats.total_points, 3);
        assert_eq!(stats.telemetry_points, 1);
        assert_eq!(stats.signaling_points, 2);
        assert_eq!(stats.analog_points, 1);
        assert_eq!(stats.digital_points, 2);
    }
}

/// Channel-level CSV configuration for ComBase point tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCsvConfig {
    /// Base directory for CSV files (relative to config file)
    pub csv_directory: String,
    /// Telemetry CSV file name (遥测)
    #[serde(default)]
    pub telemetry_file: Option<String>,
    /// Signaling CSV file name (遥信)  
    #[serde(default)]
    pub signaling_file: Option<String>,
    /// Control CSV file name (遥控)
    #[serde(default)]
    pub control_file: Option<String>,
    /// Setpoint CSV file name (遥调)
    #[serde(default)]
    pub setpoint_file: Option<String>,
}

impl Default for ChannelCsvConfig {
    fn default() -> Self {
        Self {
            csv_directory: "config/channels".to_string(),
            telemetry_file: Some("telemetry.csv".to_string()),
            signaling_file: Some("signaling.csv".to_string()),
            control_file: Some("control.csv".to_string()),
            setpoint_file: Some("setpoint.csv".to_string()),
        }
    }
}

impl ChannelCsvConfig {
    /// Get the full path for a telemetry type CSV file
    pub fn get_csv_path(&self, telemetry_type: TelemetryType, channel_id: u16) -> Option<PathBuf> {
        let filename = match telemetry_type {
            TelemetryType::Telemetry => self.telemetry_file.as_ref()?,
            TelemetryType::Signaling => self.signaling_file.as_ref()?,
            TelemetryType::Control => self.control_file.as_ref()?,
            TelemetryType::Setpoint => self.setpoint_file.as_ref()?,
        };

        let mut path = PathBuf::from(&self.csv_directory);
        path.push(format!("channel_{}", channel_id));
        path.push(filename);
        Some(path)
    }

    /// Check if all required CSV files are configured
    pub fn is_complete(&self) -> bool {
        self.telemetry_file.is_some()
            && self.signaling_file.is_some()
            && self.control_file.is_some()
            && self.setpoint_file.is_some()
    }
}

/// Data source configuration using table reference approach
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    /// Source table name (e.g., "modbus_tcp", "calculation", "manual")
    pub source_table: String,
    /// Source data index/ID within the table (must be numeric)
    pub source_data: u32,
}

impl Default for DataSource {
    fn default() -> Self {
        Self {
            source_table: "manual".to_string(),
            source_data: 1,
        }
    }
}

/// Modbus TCP source table entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusTcpSource {
    pub source_id: u32,
    pub protocol_type: String,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_type: String,
    pub byte_order: String,
    pub bit_index: Option<u8>,
    pub scaling_factor: Option<f64>,
    pub description: Option<String>,
}

/// Calculation source table entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationSource {
    pub source_id: u32,
    pub calculation_type: String,
    pub expression: String,
    pub source_points: String,  // Comma-separated point IDs
    pub update_interval_ms: u64,
    pub description: Option<String>,
}

/// Manual source table entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualSource {
    pub source_id: u32,
    pub manual_type: String,
    pub editable: bool,
    pub default_value: String,
    pub value_type: String,
    pub description: Option<String>,
}

/// Source tables container using Redis storage
#[derive(Debug, Clone)]
pub struct SourceTables {
    redis_client: Client,
    key_prefix: String,
}

impl SourceTables {
    /// Create new SourceTables with Redis client
    pub fn new(redis_url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let redis_client = Client::open(redis_url)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to connect to Redis: {}", e)))?;
        
        Ok(Self {
            redis_client,
            key_prefix: key_prefix.unwrap_or("comsrv:source_tables").to_string(),
        })
    }

    /// Get Redis connection
    fn get_connection(&self) -> Result<Connection> {
        self.redis_client.get_connection()
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to get Redis connection: {}", e)))
    }

    /// Generate Redis key for source table
    fn get_redis_key(&self, table_name: &str, source_id: u32) -> String {
        format!("{}:{}:{}", self.key_prefix, table_name, source_id)
    }

    /// Load source tables from CSV files into Redis
    pub fn load_from_csv_to_redis(
        &self,
        modbus_tcp_path: Option<&str>,
        calculation_path: Option<&str>,
        manual_path: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.get_connection()?;

        // Load Modbus TCP source table
        if let Some(path) = modbus_tcp_path {
            if std::path::Path::new(path).exists() {
                let mut reader = csv::Reader::from_path(path)
                    .map_err(|e| ComSrvError::ConfigError(format!("Failed to read CSV file {}: {}", path, e)))?;
                
                for result in reader.deserialize() {
                    let record: ModbusTcpSource = result
                        .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse CSV record: {}", e)))?;
                    
                    let key = self.get_redis_key("modbus_tcp", record.source_id);
                    let value = serde_json::to_string(&record)
                        .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize record: {}", e)))?;
                    
                    redis::cmd("SET")
                        .arg(&key)
                        .arg(&value)
                        .execute(&mut conn);
                }
            }
        }

        // Load calculation source table
        if let Some(path) = calculation_path {
            if std::path::Path::new(path).exists() {
                let mut reader = csv::Reader::from_path(path)
                    .map_err(|e| ComSrvError::ConfigError(format!("Failed to read CSV file {}: {}", path, e)))?;
                
                for result in reader.deserialize() {
                    let record: CalculationSource = result
                        .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse CSV record: {}", e)))?;
                    
                    let key = self.get_redis_key("calculation", record.source_id);
                    let value = serde_json::to_string(&record)
                        .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize record: {}", e)))?;
                    
                    redis::cmd("SET")
                        .arg(&key)
                        .arg(&value)
                        .execute(&mut conn);
                }
            }
        }

        // Load manual source table
        if let Some(path) = manual_path {
            if std::path::Path::new(path).exists() {
                let mut reader = csv::Reader::from_path(path)
                    .map_err(|e| ComSrvError::ConfigError(format!("Failed to read CSV file {}: {}", path, e)))?;
                
                for result in reader.deserialize() {
                    let record: ManualSource = result
                        .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse CSV record: {}", e)))?;
                    
                    let key = self.get_redis_key("manual", record.source_id);
                    let value = serde_json::to_string(&record)
                        .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize record: {}", e)))?;
                    
                    redis::cmd("SET")
                        .arg(&key)
                        .arg(&value)
                        .execute(&mut conn);
                }
            }
        }

        Ok(())
    }

    /// Get Modbus TCP source from Redis
    pub fn get_modbus_tcp_source(&self, source_id: u32) -> Result<Option<ModbusTcpSource>> {
        let mut conn = self.get_connection()?;
        let key = self.get_redis_key("modbus_tcp", source_id);
        
        let value: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query(&mut conn)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to get from Redis: {}", e)))?;
        
        match value {
            Some(json_str) => {
                let source: ModbusTcpSource = serde_json::from_str(&json_str)
                    .map_err(|e| ComSrvError::ConfigError(format!("Failed to deserialize source: {}", e)))?;
                Ok(Some(source))
            }
            None => Ok(None),
        }
    }

    /// Get calculation source from Redis
    pub fn get_calculation_source(&self, source_id: u32) -> Result<Option<CalculationSource>> {
        let mut conn = self.get_connection()?;
        let key = self.get_redis_key("calculation", source_id);
        
        let value: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query(&mut conn)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to get from Redis: {}", e)))?;
        
        match value {
            Some(json_str) => {
                let source: CalculationSource = serde_json::from_str(&json_str)
                    .map_err(|e| ComSrvError::ConfigError(format!("Failed to deserialize source: {}", e)))?;
                Ok(Some(source))
            }
            None => Ok(None),
        }
    }

    /// Get manual source from Redis
    pub fn get_manual_source(&self, source_id: u32) -> Result<Option<ManualSource>> {
        let mut conn = self.get_connection()?;
        let key = self.get_redis_key("manual", source_id);
        
        let value: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query(&mut conn)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to get from Redis: {}", e)))?;
        
        match value {
            Some(json_str) => {
                let source: ManualSource = serde_json::from_str(&json_str)
                    .map_err(|e| ComSrvError::ConfigError(format!("Failed to deserialize source: {}", e)))?;
                Ok(Some(source))
            }
            None => Ok(None),
        }
    }

    /// Resolve data source from Redis
    pub fn resolve_source(&self, data_source: &DataSource) -> Result<Option<SourceResolution>> {
        match data_source.source_table.as_str() {
            "modbus_tcp" => {
                if let Some(src) = self.get_modbus_tcp_source(data_source.source_data)? {
                    Ok(Some(SourceResolution::ModbusTcp(src)))
                } else {
                    Ok(None)
                }
            }
            "calculation" => {
                if let Some(src) = self.get_calculation_source(data_source.source_data)? {
                    Ok(Some(SourceResolution::Calculation(src)))
                } else {
                    Ok(None)
                }
            }
            "manual" => {
                if let Some(src) = self.get_manual_source(data_source.source_data)? {
                    Ok(Some(SourceResolution::Manual(src)))
                } else {
                    Ok(None)
                }
            }
            _ => Err(ComSrvError::ConfigError(format!(
                "Unknown source table: {}", 
                data_source.source_table
            ))),
        }
    }

    /// Validate that data source exists in Redis
    pub fn validate_data_source(&self, data_source: &DataSource) -> Result<()> {
        let mut conn = self.get_connection()?;
        let key = self.get_redis_key(&data_source.source_table, data_source.source_data);
        
        let exists: bool = redis::cmd("EXISTS")
            .arg(&key)
            .query(&mut conn)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to check Redis key existence: {}", e)))?;

        if !exists {
            return Err(ComSrvError::ConfigError(format!(
                "Source data {} not found in table {}",
                data_source.source_data, data_source.source_table
            )));
        }

        Ok(())
    }

    /// Add or update Modbus TCP source in Redis
    pub fn set_modbus_tcp_source(&self, source: &ModbusTcpSource) -> Result<()> {
        let mut conn = self.get_connection()?;
        let key = self.get_redis_key("modbus_tcp", source.source_id);
        let value = serde_json::to_string(source)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize source: {}", e)))?;
        
        redis::cmd("SET")
            .arg(&key)
            .arg(&value)
            .execute(&mut conn);
        
        Ok(())
    }

    /// Add or update calculation source in Redis
    pub fn set_calculation_source(&self, source: &CalculationSource) -> Result<()> {
        let mut conn = self.get_connection()?;
        let key = self.get_redis_key("calculation", source.source_id);
        let value = serde_json::to_string(source)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize source: {}", e)))?;
        
        redis::cmd("SET")
            .arg(&key)
            .arg(&value)
            .execute(&mut conn);
        
        Ok(())
    }

    /// Add or update manual source in Redis
    pub fn set_manual_source(&self, source: &ManualSource) -> Result<()> {
        let mut conn = self.get_connection()?;
        let key = self.get_redis_key("manual", source.source_id);
        let value = serde_json::to_string(source)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize source: {}", e)))?;
        
        redis::cmd("SET")
            .arg(&key)
            .arg(&value)
            .execute(&mut conn);
        
        Ok(())
    }

    /// Delete source from Redis
    pub fn delete_source(&self, table_name: &str, source_id: u32) -> Result<bool> {
        let mut conn = self.get_connection()?;
        let key = self.get_redis_key(table_name, source_id);
        
        let deleted: i32 = redis::cmd("DEL")
            .arg(&key)
            .query(&mut conn)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to delete from Redis: {}", e)))?;
        
        Ok(deleted > 0)
    }

    /// List all source IDs for a table
    pub fn list_source_ids(&self, table_name: &str) -> Result<Vec<u32>> {
        let mut conn = self.get_connection()?;
        let pattern = format!("{}:{}:*", self.key_prefix, table_name);
        
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query(&mut conn)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to list Redis keys: {}", e)))?;
        
        let mut source_ids = Vec::new();
        for key in keys {
            if let Some(id_str) = key.split(':').last() {
                if let Ok(id) = id_str.parse::<u32>() {
                    source_ids.push(id);
                }
            }
        }
        
        source_ids.sort();
        Ok(source_ids)
    }

    /// Clear all source tables from Redis
    pub fn clear_all_sources(&self) -> Result<()> {
        let mut conn = self.get_connection()?;
        let pattern = format!("{}:*", self.key_prefix);
        
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query(&mut conn)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to list Redis keys: {}", e)))?;
        
        if !keys.is_empty() {
            redis::cmd("DEL")
                .arg(&keys)
                .execute(&mut conn);
        }
        
        Ok(())
    }
}

/// Source resolution result
#[derive(Debug, Clone)]
pub enum SourceResolution {
    ModbusTcp(ModbusTcpSource),
    Calculation(CalculationSource),
    Manual(ManualSource),
}
