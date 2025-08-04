//! Configuration type definitions
//!
//! Contains all configuration-related type definitions

use crate::core::combase::CommandTriggerConfig;
use crate::core::sync::LuaSyncConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use voltage_libs::config::utils::{get_global_log_level, get_global_redis_url};

// ============================================================================
// Application configuration
// ============================================================================

/// Application configuration root structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Service configuration
    #[serde(default)]
    pub service: ServiceConfig,

    /// Channel configuration list
    pub channels: Vec<ChannelConfig>,
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    #[serde(default = "default_service_name")]
    pub name: String,

    /// Service version
    pub version: Option<String>,

    /// Service description
    pub description: Option<String>,

    /// API configuration
    #[serde(default)]
    pub api: ApiConfig,

    /// Redis configuration
    #[serde(default)]
    pub redis: RedisConfig,

    /// Log configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Lua synchronization configuration
    #[serde(default)]
    pub lua_sync: LuaSyncConfig,

    /// Command trigger configuration
    #[serde(default)]
    pub command_trigger: CommandTriggerConfig,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Listen address
    #[serde(default = "default_api_host")]
    pub host: String,

    /// Listen port
    #[serde(default = "default_api_port")]
    pub port: u16,

    /// Worker thread count
    #[serde(default = "default_workers")]
    pub workers: usize,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,

    /// Connection timeout (milliseconds)
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Whether enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Log configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Log file path
    pub file: Option<PathBuf>,

    /// Whether to output to console
    #[serde(default = "default_true")]
    pub console: bool,

    /// Log rotation configuration
    #[serde(default)]
    pub rotation: LogRotationConfig,
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    /// Rotation strategy
    #[serde(default = "default_rotation_strategy")]
    pub strategy: String,

    /// Maximum file size (MB)
    #[serde(default = "default_max_size")]
    pub max_size_mb: u64,

    /// Number of files to retain
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

// ============================================================================
// Channel configuration
// ============================================================================

/// Channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel ID
    pub id: u16,

    /// Channel name
    pub name: String,

    /// Description
    pub description: Option<String>,

    /// Protocol type
    pub protocol: String,

    /// Protocol parameters (generic HashMap storage)
    #[serde(default)]
    pub parameters: HashMap<String, serde_yaml::Value>,

    /// Channel log configuration
    #[serde(default)]
    pub logging: ChannelLoggingConfig,

    /// Table configuration
    pub table_config: Option<TableConfig>,

    // Under the four-telemetry separated architecture, unified points field is no longer needed
    /// Four-telemetry point mapping - stores four telemetry types separately
    #[serde(skip)]
    pub telemetry_points: HashMap<u32, CombinedPoint>,
    #[serde(skip)]
    pub signal_points: HashMap<u32, CombinedPoint>,
    #[serde(skip)]
    pub control_points: HashMap<u32, CombinedPoint>,
    #[serde(skip)]
    pub adjustment_points: HashMap<u32, CombinedPoint>,
}

/// Channel log configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelLoggingConfig {
    /// Whether enabled
    #[serde(default)]
    pub enabled: bool,

    /// Log level
    pub level: Option<String>,

    /// Log file
    pub file: Option<String>,

    /// Whether to include protocol details
    #[serde(default)]
    pub protocol_details: bool,
}

/// Table configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableConfig {
    /// Four-telemetry path
    #[serde(alias = "four_telemetry_route", default = "default_four_remote_route")]
    pub four_remote_route: String,

    /// Four-telemetry files
    #[serde(alias = "four_telemetry_files", default)]
    pub four_remote_files: FourRemoteFiles,

    /// Protocol mapping path
    #[serde(default = "default_protocol_mapping_route")]
    pub protocol_mapping_route: String,

    /// Protocol mapping files
    #[serde(alias = "protocol_mapping_files", default)]
    pub protocol_mapping_file: ProtocolMappingFiles,
}

impl Default for TableConfig {
    fn default() -> Self {
        Self {
            four_remote_route: default_four_remote_route(),
            four_remote_files: FourRemoteFiles::default(),
            protocol_mapping_route: default_protocol_mapping_route(),
            protocol_mapping_file: ProtocolMappingFiles::default(),
        }
    }
}

/// Four-telemetry files configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourRemoteFiles {
    /// Telemetry file
    #[serde(default = "default_telemetry_file")]
    pub telemetry_file: String,

    /// Signal file
    #[serde(default = "default_signal_file")]
    pub signal_file: String,

    /// Adjustment file
    #[serde(default = "default_adjustment_file")]
    pub adjustment_file: String,

    /// Control file
    #[serde(default = "default_control_file")]
    pub control_file: String,
}

impl Default for FourRemoteFiles {
    fn default() -> Self {
        Self {
            telemetry_file: default_telemetry_file(),
            signal_file: default_signal_file(),
            adjustment_file: default_adjustment_file(),
            control_file: default_control_file(),
        }
    }
}

/// Protocol mapping files configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMappingFiles {
    /// Telemetry mapping file
    #[serde(default = "default_telemetry_mapping")]
    pub telemetry_mapping: String,

    /// Signal mapping file
    #[serde(default = "default_signal_mapping")]
    pub signal_mapping: String,

    /// Adjustment mapping file
    #[serde(default = "default_adjustment_mapping")]
    pub adjustment_mapping: String,

    /// Control mapping file
    #[serde(default = "default_control_mapping")]
    pub control_mapping: String,
}

impl Default for ProtocolMappingFiles {
    fn default() -> Self {
        Self {
            telemetry_mapping: default_telemetry_mapping(),
            signal_mapping: default_signal_mapping(),
            adjustment_mapping: default_adjustment_mapping(),
            control_mapping: default_control_mapping(),
        }
    }
}

/// Combined point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedPoint {
    pub point_id: u32,
    pub signal_name: String,
    pub telemetry_type: String,
    pub data_type: String,
    pub protocol_params: HashMap<String, String>,
    pub scaling: Option<ScalingInfo>,
}

/// Scaling information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingInfo {
    pub scale: f64,
    pub offset: f64,
    pub unit: Option<String>,
    pub reverse: Option<bool>,
}

// ============================================================================
// Protocol configuration
// ============================================================================

// Under the four-telemetry separated architecture, UnifiedPointMapping is no longer needed, replaced with CombinedPoint

/// Scaling parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingParams {
    pub scale: f64,
    pub offset: f64,
    pub unit: Option<String>,
    pub reverse: Option<bool>,
}

/// Protocol mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub protocol_params: HashMap<String, String>,
}

/// Telemetry type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TelemetryType {
    /// Telemetry (YC)
    #[serde(rename = "T")]
    Telemetry,
    /// Signal (YX)
    #[serde(rename = "S")]
    Signal,
    /// Control (YK)
    #[serde(rename = "C")]
    Control,
    /// Adjustment (YT)
    #[serde(rename = "A")]
    Adjustment,
}

impl std::str::FromStr for TelemetryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "T" | "telemetry" | "Telemetry" => Ok(TelemetryType::Telemetry),
            "S" | "signal" | "Signal" => Ok(TelemetryType::Signal),
            "C" | "control" | "Control" => Ok(TelemetryType::Control),
            "A" | "adjustment" | "Adjustment" => Ok(TelemetryType::Adjustment),
            _ => Err(format!("Invalid remote type: {s}")),
        }
    }
}

/// Protocol type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolType {
    ModbusTcp,
    ModbusRtu,
    Can,
    Iec60870,
    Virtual,
    GrpcModbus, // gRPC 插件 - Modbus
}

impl std::str::FromStr for ProtocolType {
    type Err = crate::utils::error::ComSrvError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 使用normalize_protocol_name函数来实现大小写不敏感的匹配
        let normalized = crate::utils::normalize_protocol_name(s);
        match normalized.as_str() {
            "modbus_tcp" => Ok(ProtocolType::ModbusTcp),
            "modbus_rtu" => Ok(ProtocolType::ModbusRtu),
            "can" => Ok(ProtocolType::Can),
            "iec60870" => Ok(ProtocolType::Iec60870),
            "virtual" => Ok(ProtocolType::Virtual),
            "grpc_modbus" => Ok(ProtocolType::GrpcModbus),
            _ => Err(crate::utils::error::ComSrvError::ConfigError(format!(
                "Unknown protocol type: {s}"
            ))),
        }
    }
}

// ============================================================================
// Default value functions
// ============================================================================

fn default_service_name() -> String {
    "comsrv".to_string()
}

fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_api_port() -> u16 {
    8081 // Fixed port - ComSrv standard port
}

fn default_workers() -> usize {
    num_cpus::get()
}

fn default_redis_url() -> String {
    get_global_redis_url("COMSRV")
}

fn default_pool_size() -> u32 {
    10
}

fn default_timeout() -> u64 {
    5000
}

fn default_true() -> bool {
    true
}

// Default path configuration
fn default_four_remote_route() -> String {
    "four_remote".to_string()
}

fn default_protocol_mapping_route() -> String {
    "protocol_mapping".to_string()
}

// Default file name configuration
fn default_telemetry_file() -> String {
    "telemetry.csv".to_string()
}

fn default_signal_file() -> String {
    "signal.csv".to_string()
}

fn default_control_file() -> String {
    "control.csv".to_string()
}

fn default_adjustment_file() -> String {
    "adjustment.csv".to_string()
}

fn default_telemetry_mapping() -> String {
    "telemetry_mapping.csv".to_string()
}

fn default_signal_mapping() -> String {
    "signal_mapping.csv".to_string()
}

fn default_control_mapping() -> String {
    "control_mapping.csv".to_string()
}

fn default_adjustment_mapping() -> String {
    "adjustment_mapping.csv".to_string()
}

fn default_log_level() -> String {
    get_global_log_level("COMSRV")
}

fn default_log_format() -> String {
    "pretty".to_string()
}

fn default_rotation_strategy() -> String {
    "daily".to_string()
}

fn default_max_size() -> u64 {
    100
}

fn default_max_files() -> u32 {
    7
}

// ============================================================================
// Implementation
// ============================================================================

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_api_port(),
            workers: default_workers(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            pool_size: default_pool_size(),
            timeout_ms: default_timeout(),
            enabled: default_true(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            file: None,
            console: default_true(),
            rotation: LogRotationConfig::default(),
        }
    }
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            strategy: default_rotation_strategy(),
            max_size_mb: default_max_size(),
            max_files: default_max_files(),
        }
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "comsrv".to_string(),
            version: Some("0.0.1".to_string()),
            description: Some("Communication Service".to_string()),
            api: ApiConfig::default(),
            redis: RedisConfig::default(),
            logging: LoggingConfig::default(),
            lua_sync: LuaSyncConfig::default(),
            command_trigger: CommandTriggerConfig::default(),
        }
    }
}

impl ChannelConfig {
    /// Get parameter value
    pub fn get_parameter(&self, key: &str) -> Option<&serde_yaml::Value> {
        self.parameters.get(key)
    }

    /// Get string parameter
    pub fn get_string_parameter(&self, key: &str) -> Option<String> {
        self.parameters
            .get(key)
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string)
    }

    /// Get integer parameter
    pub fn get_int_parameter(&self, key: &str) -> Option<i64> {
        self.parameters.get(key).and_then(serde_yaml::Value::as_i64)
    }

    /// Get boolean parameter
    pub fn get_bool_parameter(&self, key: &str) -> Option<bool> {
        self.parameters
            .get(key)
            .and_then(serde_yaml::Value::as_bool)
    }

    /// Get point by telemetry type
    pub fn get_point(
        &self,
        telemetry_type: TelemetryType,
        point_id: u32,
    ) -> Option<&CombinedPoint> {
        match telemetry_type {
            TelemetryType::Telemetry => self.telemetry_points.get(&point_id),
            TelemetryType::Signal => self.signal_points.get(&point_id),
            TelemetryType::Control => self.control_points.get(&point_id),
            TelemetryType::Adjustment => self.adjustment_points.get(&point_id),
        }
    }

    /// Add point to corresponding HashMap
    pub fn add_point(&mut self, point: CombinedPoint) -> Result<(), String> {
        let telemetry_type = TelemetryType::from_str(&point.telemetry_type)
            .map_err(|e| format!("Invalid telemetry type: {e}"))?;

        let target_hashmap = match telemetry_type {
            TelemetryType::Telemetry => &mut self.telemetry_points,
            TelemetryType::Signal => &mut self.signal_points,
            TelemetryType::Control => &mut self.control_points,
            TelemetryType::Adjustment => &mut self.adjustment_points,
        };

        target_hashmap.insert(point.point_id, point);
        Ok(())
    }

    /// Get total points count
    pub fn get_total_points_count(&self) -> usize {
        self.telemetry_points.len()
            + self.signal_points.len()
            + self.control_points.len()
            + self.adjustment_points.len()
    }

    /// Get all points of specified type
    pub fn get_points_by_type(
        &self,
        telemetry_type: TelemetryType,
    ) -> &HashMap<u32, CombinedPoint> {
        match telemetry_type {
            TelemetryType::Telemetry => &self.telemetry_points,
            TelemetryType::Signal => &self.signal_points,
            TelemetryType::Control => &self.control_points,
            TelemetryType::Adjustment => &self.adjustment_points,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let api = ApiConfig::default();
        assert_eq!(api.host, "0.0.0.0");
        assert_eq!(api.port, 8081);

        let redis = RedisConfig::default();
        assert_eq!(redis.url, "redis://127.0.0.1:6379");
        assert!(redis.enabled);

        let logging = LoggingConfig::default();
        assert_eq!(logging.level, "info");
        assert!(logging.console);
    }

    #[test]
    fn test_channel_config_parameters() {
        let mut channel = ChannelConfig {
            id: 1,
            name: "Test".to_string(),
            description: None,
            protocol: "modbus".to_string(),
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
            table_config: None,
            telemetry_points: HashMap::new(),
            signal_points: HashMap::new(),
            control_points: HashMap::new(),
            adjustment_points: HashMap::new(),
        };

        // Add parameters
        channel.parameters.insert(
            "host".to_string(),
            serde_yaml::Value::String("192.168.1.1".to_string()),
        );
        channel.parameters.insert(
            "port".to_string(),
            serde_yaml::Value::Number(serde_yaml::Number::from(502)),
        );
        channel
            .parameters
            .insert("enabled".to_string(), serde_yaml::Value::Bool(true));

        // Test getting parameters
        assert_eq!(
            channel.get_string_parameter("host"),
            Some("192.168.1.1".to_string())
        );
        assert_eq!(channel.get_int_parameter("port"), Some(502));
        assert_eq!(channel.get_bool_parameter("enabled"), Some(true));
        assert_eq!(channel.get_string_parameter("missing"), None);
    }
}
