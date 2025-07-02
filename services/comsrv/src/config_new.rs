use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use voltage_config::prelude::*;
use figment::value::{Map, Value};

/// Communication service configuration using the unified config framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComServiceConfig {
    /// Base service configuration (flattened)
    #[serde(flatten)]
    pub base: BaseServiceConfig,
    
    /// API configuration
    pub api: ApiConfig,
    
    /// Default paths configuration
    pub default_paths: DefaultPathConfig,
    
    /// Channel configurations
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
    
    /// Protocol-specific settings
    pub protocols: ProtocolSettings,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Enable API
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// API host
    #[serde(default = "default_api_host")]
    pub host: String,
    /// API port
    #[serde(default = "default_api_port")]
    pub port: u16,
    /// API prefix
    #[serde(default = "default_api_prefix")]
    pub prefix: String,
}

/// Default paths configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultPathConfig {
    /// Configuration directory
    #[serde(default = "default_config_dir")]
    pub config_dir: String,
    /// Point table directory
    #[serde(default = "default_point_table_dir")]
    pub point_table_dir: String,
}

/// Channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel ID
    pub id: u16,
    /// Channel name
    pub name: String,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Protocol type
    pub protocol: String,
    /// Protocol parameters
    #[serde(default)]
    pub parameters: Map<String, Value>,
    /// Channel-specific logging
    #[serde(default)]
    pub logging: ChannelLoggingConfig,
    /// Table configuration for CSV loading
    #[serde(default)]
    pub table_config: Option<TableConfig>,
}

/// Channel-specific logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelLoggingConfig {
    /// Override log level
    pub level: Option<String>,
    /// Enable raw data logging
    #[serde(default)]
    pub log_raw_data: bool,
}

/// Table configuration for CSV point tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableConfig {
    /// Four telemetry files
    pub four_telemetry_files: FourTelemetryFiles,
    /// Protocol mapping files
    pub protocol_mapping_files: ProtocolMappingFiles,
}

/// Four telemetry files configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryFiles {
    /// Telemetry (YC) file
    pub telemetry: String,
    /// Control (YK) file
    pub control: String,
    /// Adjustment (YT) file
    pub adjustment: String,
    /// Signal (YX) file
    pub signal: String,
}

/// Protocol mapping files configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMappingFiles {
    /// Mapping files by protocol type
    #[serde(flatten)]
    pub mappings: HashMap<String, String>,
}

/// Protocol-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSettings {
    pub modbus: Option<ModbusSettings>,
    pub iec104: Option<Iec104Settings>,
    pub can: Option<CanSettings>,
    pub gpio: Option<GpioSettings>,
}

/// Modbus protocol settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusSettings {
    /// Default timeout in milliseconds
    #[serde(default = "default_modbus_timeout")]
    pub default_timeout: u32,
    /// Maximum retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Inter-frame delay in milliseconds
    #[serde(default = "default_inter_frame_delay")]
    pub inter_frame_delay: u32,
}

/// IEC104 protocol settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iec104Settings {
    /// T1 timeout (seconds)
    #[serde(default = "default_t1_timeout")]
    pub t1_timeout: u32,
    /// T2 timeout (seconds)
    #[serde(default = "default_t2_timeout")]
    pub t2_timeout: u32,
    /// T3 timeout (seconds)
    #[serde(default = "default_t3_timeout")]
    pub t3_timeout: u32,
    /// K value
    #[serde(default = "default_k_value")]
    pub k_value: u32,
    /// W value
    #[serde(default = "default_w_value")]
    pub w_value: u32,
}

/// CAN protocol settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanSettings {
    /// Bitrate
    #[serde(default = "default_can_bitrate")]
    pub bitrate: u32,
    /// Sample point percentage
    #[serde(default = "default_sample_point")]
    pub sample_point: f32,
}

/// GPIO settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioSettings {
    /// Polling interval in milliseconds
    #[serde(default = "default_gpio_polling")]
    pub polling_interval: u32,
    /// Debounce time in milliseconds
    #[serde(default = "default_debounce_time")]
    pub debounce_time: u32,
}

// Default value functions
fn default_true() -> bool {
    true
}

fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_api_port() -> u16 {
    8091
}

fn default_api_prefix() -> String {
    "/api/v1".to_string()
}

fn default_config_dir() -> String {
    "config".to_string()
}

fn default_point_table_dir() -> String {
    "config/point_tables".to_string()
}

fn default_modbus_timeout() -> u32 {
    1000
}

fn default_max_retries() -> u32 {
    3
}

fn default_inter_frame_delay() -> u32 {
    10
}

fn default_t1_timeout() -> u32 {
    15
}

fn default_t2_timeout() -> u32 {
    10
}

fn default_t3_timeout() -> u32 {
    20
}

fn default_k_value() -> u32 {
    12
}

fn default_w_value() -> u32 {
    8
}

fn default_can_bitrate() -> u32 {
    250000
}

fn default_sample_point() -> f32 {
    0.875
}

fn default_gpio_polling() -> u32 {
    100
}

fn default_debounce_time() -> u32 {
    50
}

impl Configurable for ComServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        // Validate API configuration
        if self.api.port == 0 {
            return Err(ConfigError::Validation("API port cannot be 0".into()));
        }
        
        // Validate channels
        let mut channel_ids = std::collections::HashSet::new();
        for channel in &self.channels {
            if !channel_ids.insert(channel.id) {
                return Err(ConfigError::Validation(
                    format!("Duplicate channel ID: {}", channel.id)
                ));
            }
            
            // Validate protocol
            match channel.protocol.as_str() {
                "modbus" | "iec104" | "can" | "gpio" => {},
                _ => return Err(ConfigError::Validation(
                    format!("Unknown protocol: {}", channel.protocol)
                )),
            }
            
            // Validate table config if present
            if let Some(table_config) = &channel.table_config {
                if table_config.four_telemetry_files.telemetry.is_empty() ||
                   table_config.four_telemetry_files.control.is_empty() ||
                   table_config.four_telemetry_files.adjustment.is_empty() ||
                   table_config.four_telemetry_files.signal.is_empty() {
                    return Err(ConfigError::Validation(
                        format!("Channel {} has incomplete telemetry file configuration", channel.id)
                    ));
                }
            }
        }
        
        // Validate protocol settings
        if let Some(modbus) = &self.protocols.modbus {
            if modbus.default_timeout == 0 {
                return Err(ConfigError::Validation(
                    "Modbus timeout cannot be 0".into()
                ));
            }
        }
        
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ServiceConfig for ComServiceConfig {
    fn base(&self) -> &BaseServiceConfig {
        &self.base
    }
    
    fn base_mut(&mut self) -> &mut BaseServiceConfig {
        &mut self.base
    }
}

impl ComServiceConfig {
    /// Load configuration using the unified framework
    pub async fn load() -> Result<Self> {
        let loader = ConfigLoaderBuilder::new()
            .base_path("config")
            .add_file("comsrv.yml")
            .environment(Environment::from_env())
            .env_prefix("COM")
            .defaults(serde_json::json!({
                "service": {
                    "name": "comsrv",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "Industrial Communication Service"
                },
                "redis": {
                    "url": "redis://localhost:6379",
                    "prefix": "voltage:com:",
                    "pool_size": 50
                },
                "logging": {
                    "level": "info",
                    "console": true,
                    "file": {
                        "path": "logs/comsrv.log",
                        "rotation": "daily",
                        "max_size": "100MB",
                        "max_files": 7
                    }
                },
                "monitoring": {
                    "metrics_enabled": true,
                    "metrics_port": 9091,
                    "health_check_enabled": true,
                    "health_check_port": 8091
                },
                "api": {
                    "enabled": true,
                    "host": "0.0.0.0",
                    "port": 8091,
                    "prefix": "/api/v1"
                },
                "default_paths": {
                    "config_dir": "config",
                    "point_table_dir": "config/point_tables"
                },
                "channels": [],
                "protocols": {
                    "modbus": {
                        "default_timeout": 1000,
                        "max_retries": 3,
                        "inter_frame_delay": 10
                    },
                    "iec104": {
                        "t1_timeout": 15,
                        "t2_timeout": 10,
                        "t3_timeout": 20,
                        "k_value": 12,
                        "w_value": 8
                    },
                    "can": {
                        "bitrate": 250000,
                        "sample_point": 0.875
                    },
                    "gpio": {
                        "polling_interval": 100,
                        "debounce_time": 50
                    }
                }
            }))?
            .build()?;
        
        let config: ComServiceConfig = loader.load_async().await
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;
        
        // Validate complete configuration
        config.validate_all()
            .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;
        
        Ok(config)
    }
    
    /// Generate default configuration file
    pub fn generate_default_config() -> String {
        let config = ComServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "comsrv".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Industrial Communication Service".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "voltage:com:".to_string(),
                    pool_size: 50,
                    database: 0,
                    password: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: Some(voltage_config::base::LogFileConfig {
                        path: "logs/comsrv.log".to_string(),
                        rotation: "daily".to_string(),
                        max_size: "100MB".to_string(),
                        max_files: 7,
                    }),
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9091,
                    health_check_enabled: true,
                    health_check_port: 8091,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8091,
                prefix: "/api/v1".to_string(),
            },
            default_paths: DefaultPathConfig {
                config_dir: "config".to_string(),
                point_table_dir: "config/point_tables".to_string(),
            },
            channels: vec![],
            protocols: ProtocolSettings {
                modbus: Some(ModbusSettings {
                    default_timeout: 1000,
                    max_retries: 3,
                    inter_frame_delay: 10,
                }),
                iec104: Some(Iec104Settings {
                    t1_timeout: 15,
                    t2_timeout: 10,
                    t3_timeout: 20,
                    k_value: 12,
                    w_value: 8,
                }),
                can: Some(CanSettings {
                    bitrate: 250000,
                    sample_point: 0.875,
                }),
                gpio: Some(GpioSettings {
                    polling_interval: 100,
                    debounce_time: 50,
                }),
            },
        };
        
        serde_yaml::to_string(&config).unwrap_or_else(|_| "# Failed to generate config file".to_string())
    }
}

/// CSV point table loader adapter for voltage-config
pub struct CsvPointTableLoader;

impl CsvPointTableLoader {
    /// Load CSV point tables for a channel
    pub async fn load_tables(
        channel: &ChannelConfig,
        base_path: &str,
    ) -> Result<Vec<CombinedPoint>> {
        if let Some(table_config) = &channel.table_config {
            // This would integrate with the existing CSV loading logic
            // from the comsrv config_manager
            todo!("Implement CSV loading integration")
        }
        Ok(vec![])
    }
}

/// Combined point structure
#[derive(Debug, Clone)]
pub struct CombinedPoint {
    /// Four telemetry point info
    pub telemetry: FourTelemetryPoint,
    /// Protocol-specific addresses
    pub addresses: HashMap<String, Value>,
}

/// Four telemetry point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryPoint {
    /// Point number
    pub point_number: u32,
    /// Telemetry type
    pub telemetry_type: String,
    /// Name
    pub name: String,
    /// Data type
    #[serde(default)]
    pub data_type: DataType,
    /// Unit
    #[serde(default)]
    pub unit: Option<String>,
    /// Scale factor
    #[serde(default)]
    pub scale: Option<f64>,
}

/// Data type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Float,
    Int,
    Bool,
    String,
}

impl Default for DataType {
    fn default() -> Self {
        DataType::Float
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_validation() {
        let mut config = ComServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "comsrv".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Test".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "test:".to_string(),
                    pool_size: 10,
                    database: 0,
                    password: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: None,
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9090,
                    health_check_enabled: true,
                    health_check_port: 8080,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8091,
                prefix: "/api/v1".to_string(),
            },
            default_paths: DefaultPathConfig {
                config_dir: "config".to_string(),
                point_table_dir: "config/point_tables".to_string(),
            },
            channels: vec![
                ChannelConfig {
                    id: 1,
                    name: "modbus_channel".to_string(),
                    description: None,
                    protocol: "modbus".to_string(),
                    parameters: Map::new(),
                    logging: Default::default(),
                    table_config: None,
                },
            ],
            protocols: ProtocolSettings {
                modbus: Some(ModbusSettings {
                    default_timeout: 1000,
                    max_retries: 3,
                    inter_frame_delay: 10,
                }),
                iec104: None,
                can: None,
                gpio: None,
            },
        };
        
        // Valid configuration should pass
        assert!(config.validate_all().is_ok());
        
        // Duplicate channel ID should fail
        config.channels.push(ChannelConfig {
            id: 1, // Duplicate ID
            name: "another_channel".to_string(),
            description: None,
            protocol: "modbus".to_string(),
            parameters: Map::new(),
            logging: Default::default(),
            table_config: None,
        });
        assert!(config.validate_all().is_err());
    }
}