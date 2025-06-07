use crate::utils::error::{ComSrvError, Result};
use crate::core::config::csv_parser::CsvPointManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::net::SocketAddr;

/// Service configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    pub name: String,
    /// Service description
    pub description: String,
    /// Metrics configuration
    #[serde(default)]
    pub metrics: MetricsConfig,
    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
    /// API configuration
    #[serde(default)]
    pub api: ApiConfig,
    /// Redis configuration
    #[serde(default)]
    pub redis: RedisConfig,
    /// Point tables configuration
    #[serde(default)]
    pub point_tables: PointTablesConfig,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "comsrv".to_string(),
            description: "Communication Service".to_string(),
            metrics: MetricsConfig::default(),
            logging: LoggingConfig::default(),
            api: ApiConfig::default(),
            redis: RedisConfig::default(),
            point_tables: PointTablesConfig::default(),
        }
    }
}

/// API configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Whether API is enabled
    pub enabled: bool,
    /// Bind address for API server
    pub bind_address: String,
    /// API version
    pub version: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "0.0.0.0:3000".to_string(),
            version: "v1".to_string(),
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
    pub enabled: bool,
    pub connection_type: RedisConnectionType,
    pub address: String,
    #[serde(default)]
    pub db: Option<u8>,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: Some(0),
        }
    }
}

impl RedisConfig {
    pub fn to_redis_url(&self) -> String {
        match self.connection_type {
            RedisConnectionType::Tcp => {
                if let Some(db) = self.db {
                    format!("redis://{}/{}", self.address, db)
                } else {
                    format!("redis://{}", self.address)
                }
            }
            RedisConnectionType::Unix => {
                if let Some(db) = self.db {
                    format!("unix:{}?db={}", self.address, db)
                } else {
                    format!("unix://{}", self.address)
                }
            }
        }
    }
}

/// Get Redis configuration
pub fn get_redis_config(config: &Config) -> &RedisConfig {
    &config.service.redis
}

/// Metrics configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub bind_address: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "0.0.0.0:9100".to_string(),
        }
    }
}

/// Logging configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    /// Log file path
    pub file: String,
    /// Maximum log file size in bytes
    pub max_size: u64,
    /// Maximum number of log files to keep
    pub max_files: u32,
    /// Whether to output logs to console
    pub console: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: "/var/log/comsrv/comsrv.log".to_string(),
            max_size: 10485760, // 10MB
            max_files: 5,
            console: true,
        }
    }
}

/// Point tables configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTablesConfig {
    /// Whether CSV point tables are enabled
    pub enabled: bool,
    /// Directory containing CSV point table files
    pub directory: String,
    /// Whether to watch for changes in CSV files
    pub watch_changes: bool,
    /// Reload interval in seconds
    pub reload_interval: u64,
}

impl Default for PointTablesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: "config/points".to_string(),
            watch_changes: true,
            reload_interval: 60,
        }
    }
}

/// Channel parameters, specific to each protocol type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChannelParameters {
    /// Modbus TCP specific parameters
    ModbusTcp {
        host: String,
        port: u16,
        timeout: u64,
        max_retries: u32,
        point_tables: HashMap<String, String>,
        poll_rate: u64,
    },
    /// Modbus RTU specific parameters
    ModbusRtu {
        port: String,
        baud_rate: u32,
        data_bits: u8,
        parity: String,
        stop_bits: u8,
        timeout: u64,
        max_retries: u32,
        point_tables: HashMap<String, String>,
        poll_rate: u64,
        slave_id: u8,
    },
    /// Generic parameters as HashMap
    Generic(HashMap<String, serde_yaml::Value>),
}

impl ChannelParameters {
    /// Get the value of the parameter
    pub fn get(&self, key: &str) -> Option<serde_yaml::Value> {
        match self {
            ChannelParameters::ModbusTcp { host, port, timeout, max_retries, point_tables: _, poll_rate } => {
                match key {
                    "host" => Some(serde_yaml::Value::String(host.clone())),
                    "port" => Some(serde_yaml::Value::Number((*port).into())),
                    "timeout" => Some(serde_yaml::Value::Number((*timeout).into())),
                    "max_retries" => Some(serde_yaml::Value::Number((*max_retries).into())),
                    "poll_rate" => Some(serde_yaml::Value::Number((*poll_rate).into())),
                    _ => None,
                }
            },
            ChannelParameters::ModbusRtu { port, baud_rate, data_bits, parity, stop_bits, timeout, max_retries, point_tables: _, poll_rate, slave_id } => {
                match key {
                    "port" => Some(serde_yaml::Value::String(port.clone())),
                    "baud_rate" => Some(serde_yaml::Value::Number((*baud_rate).into())),
                    "data_bits" => Some(serde_yaml::Value::Number((*data_bits).into())),
                    "parity" => Some(serde_yaml::Value::String(parity.clone())),
                    "stop_bits" => Some(serde_yaml::Value::Number((*stop_bits).into())),
                    "timeout" => Some(serde_yaml::Value::Number((*timeout).into())),
                    "max_retries" => Some(serde_yaml::Value::Number((*max_retries).into())),
                    "poll_rate" => Some(serde_yaml::Value::Number((*poll_rate).into())),
                    "slave_id" => Some(serde_yaml::Value::Number((*slave_id).into())),
                    _ => None,
                }
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
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Unique channel identifier
    pub id: u16,
    /// Human readable name
    pub name: String,
    /// Channel description
    pub description: String,
    /// Protocol type (e.g., "modbus_rtu", "modbus_tcp")
    pub protocol: ProtocolType,
    /// Protocol-specific parameters
    pub parameters: ChannelParameters,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        use std::collections::HashMap;
        Self {
            id: 0,
            name: "Default Channel".to_string(),
            description: "Default channel configuration".to_string(),
            protocol: ProtocolType::ModbusTcp,
            parameters: ChannelParameters::Generic(HashMap::new()),
        }
    }
}

/// Top-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Configuration schema version
    pub version: String,
    /// Service configuration
    pub service: ServiceConfig,
    /// Channel configurations
    pub channels: Vec<ChannelConfig>,
}

/// Configuration manager
#[derive(Clone)]
pub struct ConfigManager {
    /// Configuration data
    config: Config,
    /// Path to configuration file
    config_path: String,
    /// CSV point table manager
    csv_point_manager: CsvPointManager,
}

impl ConfigManager {
    /// Create a new ConfigManager from a configuration file
    pub fn from_file(config_path: impl AsRef<Path>) -> Result<Self> {
        let config_path = config_path.as_ref().to_string_lossy().to_string();
        let config = Self::load_config(&config_path)?;
        
        let mut csv_point_manager = CsvPointManager::new();
        
        // Load CSV point tables if enabled
        if config.service.point_tables.enabled {
            let points_dir = Path::new(&config.service.point_tables.directory);
            if let Err(e) = csv_point_manager.load_from_directory(points_dir) {
                tracing::warn!("Failed to load CSV point tables: {}", e);
            }
        }
        
        let manager = Self {
            config,
            config_path,
            csv_point_manager,
        };
        
        // Validate the configuration
        manager.validate_config()?;
        
        Ok(manager)
    }

    /// Load configuration from file with enhanced error handling
    fn load_config(config_path: &str) -> Result<Config> {
        // Check if file exists
        if !std::path::Path::new(config_path).exists() {
            return Err(ComSrvError::ConfigError(format!(
                "Configuration file not found: {}", config_path
            )));
        }

        // Read file content
        let content = fs::read_to_string(config_path)
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to read configuration file {}: {}", config_path, e
            )))?;

        // Parse YAML content
        let config: Config = serde_yaml::from_str(&content)
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to parse configuration file {}: {}", config_path, e
            )))?;

        Ok(config)
    }

    /// Validate the loaded configuration
    pub fn validate_config(&self) -> Result<()> {
        // Validate service configuration
        if self.config.service.name.is_empty() {
            return Err(ComSrvError::ConfigError("Service name cannot be empty".to_string()));
        }

        // Validate API configuration
        if self.config.service.api.enabled {
            if self.config.service.api.bind_address.is_empty() {
                return Err(ComSrvError::ConfigError("API bind address cannot be empty when API is enabled".to_string()));
            }
            
            // Validate bind address format
            if let Err(e) = self.config.service.api.bind_address.parse::<SocketAddr>() {
                return Err(ComSrvError::ConfigError(format!(
                    "Invalid API bind address format: {}, error: {}", 
                    self.config.service.api.bind_address, e
                )));
            }
        }

        // Validate metrics configuration
        if self.config.service.metrics.enabled {
            if self.config.service.metrics.bind_address.is_empty() {
                return Err(ComSrvError::ConfigError("Metrics bind address cannot be empty when metrics is enabled".to_string()));
            }
            
            // Validate bind address format
            if let Err(e) = self.config.service.metrics.bind_address.parse::<SocketAddr>() {
                return Err(ComSrvError::ConfigError(format!(
                    "Invalid metrics bind address format: {}, error: {}", 
                    self.config.service.metrics.bind_address, e
                )));
            }
        }

        // Validate channels
        let mut channel_ids = std::collections::HashSet::new();
        for channel in &self.config.channels {
            // Check for duplicate channel IDs
            if !channel_ids.insert(channel.id) {
                return Err(ComSrvError::ConfigError(format!(
                    "Duplicate channel ID found: {}", channel.id
                )));
            }

            // Validate channel name
            if channel.name.is_empty() {
                return Err(ComSrvError::ConfigError(format!(
                    "Channel name cannot be empty for channel ID: {}", channel.id
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
            ChannelParameters::ModbusTcp { host, port, timeout, max_retries, poll_rate, .. } => {
                if host.is_empty() {
                    return Err(ComSrvError::ConfigError(format!(
                        "Modbus TCP host cannot be empty for channel {}", channel.id
                    )));
                }
                if *port == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Modbus TCP port cannot be 0 for channel {}", channel.id
                    )));
                }
                if *timeout == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Timeout cannot be 0 for channel {}", channel.id
                    )));
                }
                if *max_retries > 10 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Max retries should not exceed 10 for channel {}", channel.id
                    )));
                }
                if *poll_rate == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Poll rate cannot be 0 for channel {}", channel.id
                    )));
                }
            },
            ChannelParameters::ModbusRtu { port, baud_rate, timeout, max_retries, poll_rate, slave_id, .. } => {
                if port.is_empty() {
                    return Err(ComSrvError::ConfigError(format!(
                        "Modbus RTU port cannot be empty for channel {}", channel.id
                    )));
                }
                if *baud_rate == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Baud rate cannot be 0 for channel {}", channel.id
                    )));
                }
                if *timeout == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Timeout cannot be 0 for channel {}", channel.id
                    )));
                }
                if *max_retries > 10 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Max retries should not exceed 10 for channel {}", channel.id
                    )));
                }
                if *poll_rate == 0 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Poll rate cannot be 0 for channel {}", channel.id
                    )));
                }
                if *slave_id == 0 || *slave_id > 247 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Invalid slave ID {} for channel {}. Must be between 1 and 247", 
                        slave_id, channel.id
                    )));
                }
            },
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
        self.config.channels.iter()
            .find(|c| c.id == channel_id)
    }
    
    /// Check if metrics are enabled
    pub fn get_metrics_enabled(&self) -> bool {
        self.config.service.metrics.enabled
    }
    
    /// Get metrics bind address
    pub fn get_metrics_address(&self) -> &str {
        &self.config.service.metrics.bind_address
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
        &self.config.service.logging.file
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
        let content = serde_yaml::to_string(&self.config)
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to serialize configuration: {}",
                e
            )))?;
        
        fs::write(path, content)
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to write configuration to {}: {}",
                path, e
            )))?;
        
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
        if new_config.service.api.enabled != self.config.service.api.enabled ||
           new_config.service.api.bind_address != self.config.service.api.bind_address ||
           new_config.service.api.version != self.config.service.api.version {
            config_changed = true;
        }
        
        // Check Redis config changes
        if new_config.service.redis.enabled != self.config.service.redis.enabled ||
           match (&new_config.service.redis.connection_type, &self.config.service.redis.connection_type) {
               (RedisConnectionType::Tcp, RedisConnectionType::Tcp) => false,
               (RedisConnectionType::Unix, RedisConnectionType::Unix) => false,
               _ => true,
           } ||
           new_config.service.redis.address != self.config.service.redis.address ||
           new_config.service.redis.db != self.config.service.redis.db {
            config_changed = true;
        }
        
        // Check for channel changes
        for new_channel in &new_config.channels {
            // Check if channel exists in current config
            let existing_channel = self.config.channels.iter()
                .find(|c| c.id == new_channel.id);
            
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
                        (ChannelParameters::ModbusTcp { 
                            host: new_host, 
                            port: new_port, 
                            timeout: new_timeout, 
                            max_retries: new_max_retries, 
                            point_tables: new_point_tables, 
                            poll_rate: new_poll_rate 
                        },
                        ChannelParameters::ModbusTcp { 
                            host: existing_host, 
                            port: existing_port, 
                            timeout: existing_timeout, 
                            max_retries: existing_max_retries, 
                            point_tables: existing_point_tables, 
                            poll_rate: existing_poll_rate 
                        }) => {
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
                            if new_point_tables != existing_point_tables {
                                changed_properties.push("parameters.point_tables".to_string());
                            }
                        },
                        (ChannelParameters::ModbusRtu { 
                            port: new_port, 
                            baud_rate: new_baud_rate, 
                            data_bits: new_data_bits, 
                            parity: new_parity, 
                            stop_bits: new_stop_bits, 
                            timeout: new_timeout, 
                            max_retries: new_max_retries, 
                            point_tables: new_point_tables, 
                            poll_rate: new_poll_rate, 
                            slave_id: new_slave_id 
                        },
                        ChannelParameters::ModbusRtu { 
                            port: existing_port, 
                            baud_rate: existing_baud_rate, 
                            data_bits: existing_data_bits, 
                            parity: existing_parity, 
                            stop_bits: existing_stop_bits, 
                            timeout: existing_timeout, 
                            max_retries: existing_max_retries, 
                            point_tables: existing_point_tables, 
                            poll_rate: existing_poll_rate, 
                            slave_id: existing_slave_id 
                        }) => {
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
                            if new_point_tables != existing_point_tables {
                                changed_properties.push("parameters.point_tables".to_string());
                            }
                        },
                        (ChannelParameters::Generic(new_params), ChannelParameters::Generic(existing_params)) => {
                            // Compare each parameter in the generic map
                            for (key, new_value) in new_params {
                                match existing_params.get(key) {
                                    Some(existing_value) if existing_value != new_value => {
                                        changed_properties.push(format!("parameters.{}", key));
                                    },
                                    None => {
                                        changed_properties.push(format!("parameters.{}", key));
                                    },
                                    _ => {}
                                }
                            }
                            
                            // Check for removed parameters
                            for key in existing_params.keys() {
                                if !new_params.contains_key(key) {
                                    changed_properties.push(format!("parameters.{}.removed", key));
                                }
                            }
                        },
                        _ => {
                            // Different parameter types, consider all parameters changed
                            changed_properties.push("parameters".to_string());
                        }
                    }
                },
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
            let channel_exists = new_config.channels.iter()
                .any(|c| c.id == existing_channel.id);
            
            if !channel_exists {
                config_changed = true;
                channel_changes.insert(
                    existing_channel.id.to_string(), 
                    vec!["removed".to_string()]
                );
            }
        }
        
        // Update config if changed
        if config_changed {
            self.config = new_config;
        }
        
        Ok((config_changed, channel_changes))
    }

    /// Get point tables configuration
    pub fn get_point_tables_config(&self) -> &PointTablesConfig {
        &self.config.service.point_tables
    }

    /// Get CSV point manager reference
    pub fn get_csv_point_manager(&self) -> &CsvPointManager {
        &self.csv_point_manager
    }

    /// Get mutable CSV point manager reference
    pub fn get_csv_point_manager_mut(&mut self) -> &mut CsvPointManager {
        &mut self.csv_point_manager
    }

    /// Reload CSV point tables
    pub fn reload_csv_point_tables(&mut self) -> Result<()> {
        if !self.config.service.point_tables.enabled {
            return Ok(());
        }

        let points_dir = Path::new(&self.config.service.point_tables.directory);
        self.csv_point_manager.load_from_directory(points_dir)?;
        
        tracing::info!("Reloaded CSV point tables from: {}", points_dir.display());
        Ok(())
    }

    /// Get point table names
    pub fn get_point_table_names(&self) -> Vec<String> {
        self.csv_point_manager.get_table_names()
    }

    /// Get Modbus mappings for a specific channel
    pub fn get_modbus_mappings_for_channel(&self, channel_id: u16) -> Result<Vec<crate::core::protocols::modbus::common::ModbusRegisterMapping>> {
        let channel = self.get_channel(channel_id)
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
            ChannelParameters::ModbusTcp { point_tables, .. } |
            ChannelParameters::ModbusRtu { point_tables, .. } => {
                // Use the first point table if multiple are configured
                point_tables.keys().next().cloned()
            },
            ChannelParameters::Generic(map) => {
                // Look for point_table parameter in generic parameters
                map.get("point_table")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            },
        }
    }

    /// Update channel point table mapping
    pub fn update_channel_point_table(&mut self, channel_id: u16, table_name: String) -> Result<()> {
        let channel = self.get_channels_mut().iter_mut()
            .find(|c| c.id == channel_id)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Channel {} not found", channel_id)))?;

        match &mut channel.parameters {
            ChannelParameters::ModbusTcp { point_tables, .. } |
            ChannelParameters::ModbusRtu { point_tables, .. } => {
                point_tables.clear();
                point_tables.insert("default".to_string(), table_name);
            },
            ChannelParameters::Generic(map) => {
                map.insert("point_table".to_string(), serde_yaml::Value::String(table_name));
            },
        }

        Ok(())
    }

    /// Get mutable channels reference
    fn get_channels_mut(&mut self) -> &mut Vec<ChannelConfig> {
        &mut self.config.channels
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
  metrics:
    enabled: true
    bind_address: "127.0.0.1:9090"
  api:
    enabled: true
    bind_address: "127.0.0.1:8080"
    version: "v1"
channels: []
"#;
        
        let config_path = create_test_config_file(temp_dir.path(), config_content);
        let manager = ConfigManager::from_file(&config_path).expect("Failed to create config manager");
        
        assert_eq!(manager.get_service_name(), "test_service");
        assert!(manager.get_metrics_enabled());
        assert!(manager.get_api_enabled());
        assert_eq!(manager.get_channels().len(), 0);
    }

    #[test]
    fn test_config_validation() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        
        // test a valid configuration
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
        
        // test invalid configuration with duplicate channel ID
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
        
        // this configuration should fail due to duplicate channel IDs
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
        
        // verify channel count
        assert_eq!(manager.get_channels().len(), 2);
        
        // verify TCP channel
        let tcp_channel = manager.get_channel(1).expect("TCP channel should exist");
        assert_eq!(tcp_channel.name, "TCP Test Channel");
        assert_eq!(tcp_channel.protocol, ProtocolType::ModbusTcp);
        
        // verify RTU channel
        let rtu_channel = manager.get_channel(2).expect("RTU channel should exist");
        assert_eq!(rtu_channel.name, "RTU Test Channel");
        assert_eq!(rtu_channel.protocol, ProtocolType::ModbusRtu);
        
        // verify non-existent channel
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
            // test conversion from string to enum
            let parsed = ProtocolType::from_str(str_repr).expect("Failed to parse protocol type");
            assert_eq!(parsed, enum_val);
            
            // test conversion from enum to string
            assert_eq!(enum_val.as_str(), str_repr);
            
            // test Display trait
            assert_eq!(format!("{}", enum_val), str_repr);
        }
        
        // test invalid protocol type
        assert!(ProtocolType::from_str("InvalidProtocol").is_err());
    }

    #[test]
    fn test_channel_parameters_get() {
        // test ModbusTcp parameter retrieval
        let tcp_params = ChannelParameters::ModbusTcp {
            host: "192.168.1.100".to_string(),
            port: 502,
            timeout: 5000,
            max_retries: 3,
            point_tables: HashMap::new(),
            poll_rate: 1000,
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
        
        // test ModbusRtu parameter retrieval
        let rtu_params = ChannelParameters::ModbusRtu {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            data_bits: 8,
            parity: "None".to_string(),
            stop_bits: 1,
            timeout: 5000,
            max_retries: 3,
            point_tables: HashMap::new(),
            poll_rate: 1000,
            slave_id: 1,
        };
        
        if let Some(port) = rtu_params.get("port") {
            assert_eq!(port.as_str().unwrap(), "/dev/ttyUSB0");
        } else {
            panic!("Port parameter not found");
        }
        
        if let Some(baud_rate) = rtu_params.get("baud_rate") {
            assert_eq!(baud_rate.as_u64().unwrap(), 9600);
        } else {
            panic!("Baud rate parameter not found");
        }
        
        // test generic parameter retrieval
        let mut generic_map = HashMap::new();
        generic_map.insert("custom_param".to_string(), serde_yaml::Value::String("test_value".to_string()));
        let generic_params = ChannelParameters::Generic(generic_map);
        
        if let Some(custom_param) = generic_params.get("custom_param") {
            assert_eq!(custom_param.as_str().unwrap(), "test_value");
        } else {
            panic!("Custom parameter not found");
        }
        
        // test nonexistent parameter
        assert!(tcp_params.get("nonexistent_param").is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            version: "1.0".to_string(),
            service: ServiceConfig {
                name: "test_service".to_string(),
                description: "Test service for serialization".to_string(),
                metrics: MetricsConfig {
                    enabled: true,
                    bind_address: "127.0.0.1:9090".to_string(),
                },
                logging: LoggingConfig::default(),
                api: ApiConfig::default(),
                redis: RedisConfig::default(),
                point_tables: PointTablesConfig::default(),
            },
            channels: vec![
                ChannelConfig {
                    id: 1,
                    name: "Test TCP Channel".to_string(),
                    description: "TCP test channel".to_string(),
                    protocol: ProtocolType::ModbusTcp,
                    parameters: ChannelParameters::ModbusTcp {
                        host: "192.168.1.100".to_string(),
                        port: 502,
                        timeout: 5000,
                        max_retries: 3,
                        point_tables: HashMap::new(),
                        poll_rate: 1000,
                    },
                },
                ChannelConfig {
                    id: 2,
                    name: "Test RTU Channel".to_string(),
                    description: "RTU test channel".to_string(),
                    protocol: ProtocolType::ModbusRtu,
                    parameters: ChannelParameters::ModbusRtu {
                        port: "/dev/ttyUSB0".to_string(),
                        baud_rate: 9600,
                        data_bits: 8,
                        parity: "None".to_string(),
                        stop_bits: 1,
                        timeout: 5000,
                        max_retries: 3,
                        point_tables: HashMap::new(),
                        poll_rate: 1000,
                        slave_id: 1,
                    },
                },
            ],
        };
        
        // serialize the configuration
        let serialized = serde_yaml::to_string(&config).expect("Failed to serialize config");
        assert!(!serialized.is_empty());
        
        // deserialize the configuration
        let deserialized: Config = serde_yaml::from_str(&serialized).expect("Failed to deserialize config");
        
        // verify deserialization results
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
        // test TCP connection
        let tcp_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "127.0.0.1:6379".to_string(),
            db: Some(0),
        };
        
        let tcp_url = tcp_config.to_redis_url();
        assert_eq!(tcp_url, "redis://127.0.0.1:6379/0");
        
        // test Unix socket connection
        let unix_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Unix,
            address: "/tmp/redis.sock".to_string(),
            db: None,
        };
        
        let unix_url = unix_config.to_redis_url();
        assert_eq!(unix_url, "unix:///tmp/redis.sock");
        
        // test TCP connection without database
        let tcp_no_db_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "127.0.0.1:6379".to_string(),
            db: None,
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
        let mut manager = ConfigManager::from_file(&config_path).expect("Failed to load initial config");
        
        assert_eq!(manager.get_service_name(), "initial_service");
        assert_eq!(manager.get_channels().len(), 1);
        
        // update the configuration file
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
        
        // reload the configuration
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