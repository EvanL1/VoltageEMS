//! Configuration types for comsrv
//! 
//! This module defines all configuration structures used by the communication service.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use figment::value::{Map, Value};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Service configuration
    pub service: ServiceConfig,
    
    /// API configuration
    pub api: ApiConfig,
    
    /// Redis configuration
    pub redis: RedisConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// Default paths configuration
    pub default_paths: DefaultPathConfig,
    
    /// Channel configurations
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    pub name: String,
    
    /// Service version
    pub version: String,
    
    /// Service description
    pub description: String,
    
    /// Service instance ID
    #[serde(default)]
    pub instance_id: String,
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

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    #[serde(default = "default_redis_url")]
    pub url: String,
    
    /// Redis prefix
    #[serde(default = "default_redis_prefix")]
    pub prefix: String,
    
    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    
    /// Database number
    #[serde(default)]
    pub database: u32,
    
    /// Password
    #[serde(default)]
    pub password: Option<String>,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,
    
    /// Enable console logging
    #[serde(default = "default_true")]
    pub console: bool,
    
    /// Log file configuration
    #[serde(default)]
    pub file: Option<LogFileConfig>,
    
    /// JSON format
    #[serde(default)]
    pub json_format: bool,
}

/// Log file configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileConfig {
    /// Log file path
    pub path: String,
    
    /// Rotation strategy
    #[serde(default = "default_rotation")]
    pub rotation: String,
    
    /// Max file size
    #[serde(default = "default_max_size")]
    pub max_size: String,
    
    /// Max number of files
    #[serde(default = "default_max_files")]
    pub max_files: u32,
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
    
    /// Table configuration
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

/// Table configuration
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

/// Protocol address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolAddress {
    /// Protocol type
    pub protocol: String,
    
    /// Address value
    pub address: Value,
}

/// Unified point mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPointMapping {
    /// Point ID
    pub point_id: String,
    
    /// Four telemetry info
    pub four_telemetry: FourTelemetryPoint,
    
    /// Protocol addresses
    pub protocol_addresses: Vec<ProtocolAddress>,
}

/// Combined point (four telemetry + protocol mapping)
#[derive(Debug, Clone)]
pub struct CombinedPoint {
    /// Four telemetry point
    pub telemetry: FourTelemetryPoint,
    
    /// Protocol-specific addresses
    pub addresses: HashMap<String, Value>,
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

fn default_redis_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_redis_prefix() -> String {
    "voltage:com:".to_string()
}

fn default_pool_size() -> u32 {
    50
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_rotation() -> String {
    "daily".to_string()
}

fn default_max_size() -> String {
    "100MB".to_string()
}

fn default_max_files() -> u32 {
    7
}

fn default_config_dir() -> String {
    "config".to_string()
}

fn default_point_table_dir() -> String {
    "config/point_tables".to_string()
}