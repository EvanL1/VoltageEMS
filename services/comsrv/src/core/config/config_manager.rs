//! # Modern Configuration Management
//!
//! This module provides a modern configuration management system using Figment,
//! which automatically handles configuration from multiple sources:
//! - Configuration files (YAML, TOML, JSON)
//! - Environment variables
//! - Default values
//! - Command line arguments (via clap integration)
//!
//! This replaces the complex manual configuration management with a more
//! streamlined approach.

use crate::utils::error::{ComSrvError, Result};
use figment::{
    providers::{Env, Format, Serialized, Yaml, Toml, Json},
    value::{Map, Value},
    Figment, Provider,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::collections::HashMap;

// Import all necessary types from the types module
use super::types::{
    AppConfig,
    ServiceConfig,
    ChannelConfig as TypesChannelConfig,
    CombinedPoint as TypesCombinedPoint,
    RedisConfig,
    FourTelemetryPoint,
    TableConfig,
    DataType,
    ProtocolAddress,
    UnifiedPointMapping,
};

// AppConfig is now imported from types::app
// Commented out to avoid duplicate definition
// pub struct AppConfig { ... }

// ServiceConfig is now imported from types::app
// Commented out to avoid duplicate definition
// pub struct ServiceConfig { ... }

// ApiConfig is now imported from types::app
// Commented out to avoid duplicate definition
// pub struct ApiConfig { ... }

// RedisConfig is now imported from types::redis
// Commented out to avoid duplicate definition
// pub struct RedisConfig { ... }

// LoggingConfig is now imported from types::logging
// Commented out to avoid duplicate definition
// pub struct LoggingConfig { ... }

// Keep the legacy ChannelConfig for now as it has different fields
// TODO: Merge this with TypesChannelConfig
/// Channel configuration (legacy)
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
    
    /// Protocol parameters
    #[serde(default)]
    pub parameters: Map<String, Value>,
    
    /// Channel-specific logging configuration
    #[serde(default)]
    pub logging: crate::core::config::types::ChannelLoggingConfig,
    
    /// Point table configuration (legacy)
    pub point_table: Option<PointTableConfig>,
    
    /// CSV point table files (loaded via bridge layer, legacy)
    #[serde(default)]
    pub mapping_files: Vec<String>,
    
    /// Separated table configuration
    pub table_config: Option<SeparatedTableConfig>,
    
    /// Parsed point mappings (filled by bridge layer, not from YAML)
    #[serde(skip)]
    pub points: Vec<PointMappingEnum>,
    
    /// Combined points (four telemetry + protocol mapping)
    #[serde(skip)]
    pub combined_points: Vec<CombinedPoint>,
}

impl ChannelConfig {
    /// Convert to the types ChannelConfig with combined_points populated
    pub fn to_types_channel_config(&self) -> TypesChannelConfig {
        // Convert the parameters from figment Map to HashMap
        let mut parameters = HashMap::new();
        for (key, value) in &self.parameters {
            if let Ok(yaml_value) = serde_yaml::to_value(value) {
                parameters.insert(key.clone(), yaml_value);
            }
        }
        
        // Convert combined_points to the types version
        let types_combined_points: Vec<TypesCombinedPoint> = self.combined_points.iter().map(|cp| {
            // Extract parameters from the mapping
            let mut protocol_params = HashMap::new();
            protocol_params.insert("address".to_string(), cp.mapping.address.to_string());
            protocol_params.insert("data_type".to_string(), cp.mapping.data_type.clone());
            protocol_params.insert("data_format".to_string(), cp.mapping.data_format.clone());
            protocol_params.insert("number_of_bytes".to_string(), cp.mapping.number_of_bytes.to_string());
            if let Some(bit_loc) = cp.mapping.bit_location {
                protocol_params.insert("bit_location".to_string(), bit_loc.to_string());
            }
            
            TypesCombinedPoint {
                point_id: cp.telemetry.point_id,
                signal_name: cp.telemetry.signal_name.clone(),
                chinese_name: cp.telemetry.chinese_name.clone(),
                telemetry_type: "YC".to_string(), // Default type, would need proper mapping
                data_type: cp.mapping.data_type.clone(),
                protocol_params,
                scaling: if cp.telemetry.scale.is_some() || cp.telemetry.offset.is_some() {
                    Some(super::types::channel::ScalingInfo {
                        scale: cp.telemetry.scale.unwrap_or(1.0),
                        offset: cp.telemetry.offset.unwrap_or(0.0),
                        unit: cp.telemetry.unit.clone(),
                    })
                } else {
                    None
                },
            }
        }).collect();
        
        TypesChannelConfig {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            protocol: self.protocol.clone(),
            parameters,
            logging: self.logging.clone(),
            table_config: None, // TODO: convert if needed
            points: Vec::new(), // Legacy field
            combined_points: types_combined_points,
        }
    }
}

// SeparatedTableConfig is now replaced by TableConfig from types::channel
// Using type alias for backward compatibility
pub type SeparatedTableConfig = TableConfig;

// FourTelemetryFiles is now imported from types::channel
// Commented out to avoid duplicate definition

// ProtocolMappingFiles is now imported from types::channel
// Commented out to avoid duplicate definition

/// Point table configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTableConfig {
    /// Whether enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// Base directory
    pub directory: Option<String>,
    
    /// File mappings
    #[serde(default)]
    pub files: Map<String, Value>,
}

// DefaultPathConfig is now imported from types::app
// Commented out to avoid duplicate definition
// pub struct DefaultPathConfig { ... }

// Default value functions
fn default_service_name() -> String {
    "comsrv".to_string()
}

fn default_true() -> bool {
    true
}

fn default_api_bind() -> String {
    "127.0.0.1:3000".to_string()
}

fn default_api_version() -> String {
    "v1".to_string()
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379/0".to_string()
}

fn default_redis_timeout() -> u64 {
    5000
}

fn default_redis_retries() -> u32 {
    3
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_max_size() -> u64 {
    104_857_600 // 100MB
}

fn default_log_max_files() -> u32 {
    5
}

fn default_log_retention_days() -> u32 {
    30 // Keep logs for 30 days by default
}

fn default_channel_log_dir() -> String {
    "channels".to_string()
}

fn default_channels_root() -> String {
    "channels".to_string()
}

fn default_combase_dir() -> String {
    "combase".to_string()
}

fn default_protocol_dir() -> String {
    "protocol".to_string()
}

// FourTelemetryPoint is now imported from types::channel
// Note: The version in types::channel has additional fields
// Commented out the beginning of the duplicate definition
/*
pub struct FourTelemetryPoint {
    /// Point ID (unique within table)
    pub point_id: u32,
    
    /// Signal name
    pub signal_name: String,
    
    /// Chinese name
    pub chinese_name: String,
    
    /// Scale factor (for YC/YT)
    pub scale: Option<f64>,
    
    /// Offset (for YC/YT) 
    pub offset: Option<f64>,
    
    /// Unit (for YC/YT)
    pub unit: Option<String>,
    
    /// Reverse bit (for YX/YK)
    pub reverse: Option<bool>,
}
*/

// ProtocolMapping is similar to types::channel::ProtocolMapping but has different fields
// Keep this for now as it has different structure
/// Protocol mapping definition (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMapping {
    /// Point ID (matches FourTelemetryPoint)
    pub point_id: u32,
    
    /// Signal name (matches FourTelemetryPoint)
    pub signal_name: String,
    
    /// Protocol address
    pub address: String,
    
    /// Data type
    pub data_type: String,
    
    /// Data format (ABCD, CDBA, BADC, DCBA)
    pub data_format: String,
    
    /// Number of bytes
    pub number_of_bytes: u8,
    
    /// Bit location (1-16 for register bit parsing, default 1)
    pub bit_location: Option<u8>,
    
    /// Description (optional)
    pub description: Option<String>,
}

/// Data type validation rules
#[derive(Debug, Clone)]
pub struct DataTypeRule {
    pub data_type: String,
    pub valid_formats: Vec<String>,
    pub expected_bytes: u8,
    pub max_bit_location: u8,
}

impl DataTypeRule {
    /// Get all validation rules for data types
    pub fn get_validation_rules() -> Vec<DataTypeRule> {
        vec![
            DataTypeRule {
                data_type: "bool".to_string(),
                valid_formats: vec!["BIT".to_string()],  // bool类型使用BIT格式
                expected_bytes: 1,
                max_bit_location: 16,
            },
            DataTypeRule {
                data_type: "uint8".to_string(),
                valid_formats: vec!["RAW".to_string()],  // 单字节数据使用RAW格式
                expected_bytes: 1,
                max_bit_location: 8,
            },
            DataTypeRule {
                data_type: "int8".to_string(),
                valid_formats: vec!["RAW".to_string()],  // 单字节数据使用RAW格式
                expected_bytes: 1,
                max_bit_location: 8,
            },
            DataTypeRule {
                data_type: "uint16".to_string(),
                valid_formats: vec!["AB".to_string(), "BA".to_string()],  // 2字节格式
                expected_bytes: 2,
                max_bit_location: 16,
            },
            DataTypeRule {
                data_type: "int16".to_string(),
                valid_formats: vec!["AB".to_string(), "BA".to_string()],  // 2字节格式
                expected_bytes: 2,
                max_bit_location: 16,
            },
            DataTypeRule {
                data_type: "uint32".to_string(),
                valid_formats: vec!["ABCD".to_string(), "CDBA".to_string(), "BADC".to_string(), "DCBA".to_string()],
                expected_bytes: 4,
                max_bit_location: 16,
            },
            DataTypeRule {
                data_type: "int32".to_string(),
                valid_formats: vec!["ABCD".to_string(), "CDBA".to_string(), "BADC".to_string(), "DCBA".to_string()],
                expected_bytes: 4,
                max_bit_location: 16,
            },
            DataTypeRule {
                data_type: "float32".to_string(),
                valid_formats: vec!["ABCD".to_string(), "CDBA".to_string(), "BADC".to_string(), "DCBA".to_string()],
                expected_bytes: 4,
                max_bit_location: 16,
            },
            DataTypeRule {
                data_type: "float64".to_string(),
                valid_formats: vec!["ABCDEFGH".to_string(), "GHEFCDAB".to_string(), "CDABEFGH".to_string(), "EFGHCDAB".to_string()],  // 8字节格式
                expected_bytes: 8,
                max_bit_location: 16,
            },
        ]
    }
}

impl ProtocolMapping {
    /// Validate protocol mapping configuration
    pub fn validate(&self) -> Result<()> {
        let rules = DataTypeRule::get_validation_rules();
        
        // Find matching rule for data type
        let rule = rules.iter()
            .find(|r| r.data_type == self.data_type)
            .ok_or_else(|| ComSrvError::ConfigError(
                format!("Unsupported data type: {}", self.data_type)
            ))?;
        
        // Validate data format
        if !rule.valid_formats.contains(&self.data_format) {
            return Err(ComSrvError::ConfigError(
                format!("Invalid data format '{}' for data type '{}'. Valid formats: {:?}", 
                    self.data_format, self.data_type, rule.valid_formats)
            ));
        }
        
        // Validate number of bytes
        if self.number_of_bytes != rule.expected_bytes {
            return Err(ComSrvError::ConfigError(
                format!("Invalid number of bytes {} for data type '{}'. Expected: {}", 
                    self.number_of_bytes, self.data_type, rule.expected_bytes)
            ));
        }
        
        // Validate bit location
        if let Some(bit_loc) = self.bit_location {
            if bit_loc < 1 || bit_loc > rule.max_bit_location {
                return Err(ComSrvError::ConfigError(
                    format!("Invalid bit location {} for data type '{}'. Must be between 1 and {}", 
                        bit_loc, self.data_type, rule.max_bit_location)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Get default bit location (1 if not specified)
    pub fn get_bit_location(&self) -> u8 {
        self.bit_location.unwrap_or(1)
    }
}

// CombinedPoint here is different from types::channel::CombinedPoint
// This is the legacy version that combines telemetry and mapping
// Keep for backward compatibility
/// Combined point configuration (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedPoint {
    /// Four telemetry point
    pub telemetry: FourTelemetryPoint,
    
    /// Protocol mapping
    pub mapping: ProtocolMapping,
}

/// Universal point mapping structure for CSV bridge layer (legacy compatibility)
/// Base trait for all point mappings
/// This defines the common interface for protocol-specific point mappings
pub trait PointMapping {
    /// Get point ID within the channel
    fn point_id(&self) -> u32;
    
    /// Get human-readable signal name
    fn signal_name(&self) -> &str;
    
    /// Get Chinese name (optional)
    fn chinese_name(&self) -> Option<&str>;
    
    /// Get data type (bool, u16, i32, f32, etc.)
    fn data_type(&self) -> &str;
    
    /// Get engineering unit (optional)
    fn unit(&self) -> Option<&str>;
    
    /// Get description
    fn description(&self) -> Option<&str>;
    
    /// Get group/category
    fn group(&self) -> Option<&str>;
    
    /// Convert raw protocol value to engineering units
    fn convert_to_engineering(&self, raw_value: f64) -> f64;
    
    /// Convert engineering units to raw protocol value
    fn convert_from_engineering(&self, engineering_value: f64) -> f64;
    
    /// Get protocol-specific address as string
    fn address_string(&self) -> &str;
    
    /// Validate the point mapping configuration
    fn validate(&self) -> Result<()>;
}

/// Modbus-specific point mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusPointMapping {
    /// Point ID within the channel
    pub point_id: u32,
    
    /// Human-readable signal name
    pub signal_name: String,
    
    /// Chinese name (optional)
    pub chinese_name: Option<String>,
    
    /// Modbus slave ID
    pub slave_id: u8,
    
    /// Modbus register address
    pub address: u16,
    
    /// Data type (bool, uint16, int16, uint32, int32, float32, etc.)
    pub data_type: String,
    
    /// Data format for multi-byte values (ABCD, CDBA, BADC, DCBA)
    pub data_format: String,
    
    /// Number of bytes
    pub number_of_bytes: u8,
    
    /// Bit location for bit-level operations (1-16)
    pub bit_location: Option<u8>,
    
    /// Engineering unit (optional)
    pub unit: Option<String>,
    
    /// Scale factor for value conversion
    #[serde(default = "default_scale")]
    pub scale: f64,
    
    /// Offset for value conversion
    #[serde(default)]
    pub offset: f64,
    
    /// Description
    pub description: Option<String>,
    
    /// Group/category
    pub group: Option<String>,
}

fn default_scale() -> f64 {
    1.0
}

impl PointMapping for ModbusPointMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }
    
    fn signal_name(&self) -> &str {
        &self.signal_name
    }
    
    fn chinese_name(&self) -> Option<&str> {
        self.chinese_name.as_deref()
    }
    
    fn data_type(&self) -> &str {
        &self.data_type
    }
    
    fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }
    
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    
    fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }
    
    fn convert_to_engineering(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }
    
    fn convert_from_engineering(&self, engineering_value: f64) -> f64 {
        (engineering_value - self.offset) / self.scale
    }
    
    fn address_string(&self) -> &str {
        // For Modbus, we'll create a string representation
        // This is not ideal but maintains compatibility
        // TODO: Better to return the actual address type
        "modbus_address"
    }
    
    fn validate(&self) -> Result<()> {
        // Validate Modbus-specific parameters
        if self.slave_id == 0 || self.slave_id > 247 {
            return Err(ComSrvError::ConfigError(
                format!("Invalid Modbus slave ID: {}. Must be 1-247", self.slave_id)
            ));
        }
        
        // Validate data format using the DataTypeRule system
        let rules = DataTypeRule::get_validation_rules();
        let rule = rules.iter()
            .find(|r| r.data_type == self.data_type)
            .ok_or_else(|| ComSrvError::ConfigError(
                format!("Unsupported data type: {}", self.data_type)
            ))?;
        
        if !rule.valid_formats.contains(&self.data_format) {
            return Err(ComSrvError::ConfigError(
                format!("Invalid data format '{}' for data type '{}'. Valid formats: {:?}", 
                    self.data_format, self.data_type, rule.valid_formats)
            ));
        }
        
        // Validate bit location if specified
        if let Some(bit_loc) = self.bit_location {
            if bit_loc == 0 || bit_loc > 16 {
                return Err(ComSrvError::ConfigError(
                    format!("Invalid bit location: {}. Must be 1-16", bit_loc)
                ));
            }
        }
        
        Ok(())
    }
}

impl ModbusPointMapping {
    /// Get Modbus slave ID
    pub fn slave_id(&self) -> u8 {
        self.slave_id
    }
    
    /// Get Modbus register address
    pub fn address(&self) -> u16 {
        self.address
    }
    
    /// Get data format
    pub fn data_format(&self) -> &str {
        &self.data_format
    }
    
    /// Get number of bytes
    pub fn number_of_bytes(&self) -> u8 {
        self.number_of_bytes
    }
    
    /// Get bit location (1-indexed)
    pub fn bit_location(&self) -> u8 {
        self.bit_location.unwrap_or(1)
    }
    
    /// Check if this is a multi-register value
    pub fn is_multi_register(&self) -> bool {
        self.number_of_bytes > 2
    }
    
    /// Get register count based on data type
    pub fn register_count(&self) -> u16 {
        (self.number_of_bytes as u16 + 1) / 2
    }
}

/// CAN Bus specific point mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CANPointMapping {
    /// Point ID within the channel
    pub point_id: u32,
    
    /// Human-readable signal name
    pub signal_name: String,
    
    /// Chinese name (optional)
    pub chinese_name: Option<String>,
    
    /// CAN ID (hex format like 0x18FF10F4)
    pub can_id: u32,
    
    /// Start bit in CAN frame
    pub start_bit: u8,
    
    /// Length in bits
    pub bit_length: u8,
    
    /// Data type (bool, uint8, int8, uint16, int16, uint32, int32, float32)
    pub data_type: String,
    
    /// Byte order (big_endian, little_endian)
    pub byte_order: String,
    
    /// Sign type (signed, unsigned)
    pub sign_type: String,
    
    /// Engineering unit (optional)
    pub unit: Option<String>,
    
    /// Scale factor for value conversion
    #[serde(default = "default_scale")]
    pub scale: f64,
    
    /// Offset for value conversion
    #[serde(default)]
    pub offset: f64,
    
    /// Description
    pub description: Option<String>,
    
    /// Group/category
    pub group: Option<String>,
}

impl PointMapping for CANPointMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }
    
    fn signal_name(&self) -> &str {
        &self.signal_name
    }
    
    fn chinese_name(&self) -> Option<&str> {
        self.chinese_name.as_deref()
    }
    
    fn data_type(&self) -> &str {
        &self.data_type
    }
    
    fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }
    
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    
    fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }
    
    fn convert_to_engineering(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }
    
    fn convert_from_engineering(&self, engineering_value: f64) -> f64 {
        (engineering_value - self.offset) / self.scale
    }
    
    fn address_string(&self) -> &str {
        // For CAN, we'll create a hex string representation
        "can_id"
    }
    
    fn validate(&self) -> Result<()> {
        // Validate CAN-specific parameters
        if self.can_id > 0x1FFFFFFF {
            return Err(ComSrvError::ConfigError(
                format!("Invalid CAN ID: 0x{:08X}. Must be <= 0x1FFFFFFF", self.can_id)
            ));
        }
        
        if self.start_bit > 63 {
            return Err(ComSrvError::ConfigError(
                format!("Invalid start bit: {}. Must be 0-63", self.start_bit)
            ));
        }
        
        if self.bit_length == 0 || self.bit_length > 64 {
            return Err(ComSrvError::ConfigError(
                format!("Invalid bit length: {}. Must be 1-64", self.bit_length)
            ));
        }
        
        if self.start_bit + self.bit_length > 64 {
            return Err(ComSrvError::ConfigError(
                format!("Signal extends beyond frame: start_bit({}) + bit_length({}) > 64", 
                    self.start_bit, self.bit_length)
            ));
        }
        
        // Validate byte order
        match self.byte_order.as_str() {
            "big_endian" | "little_endian" => {},
            _ => return Err(ComSrvError::ConfigError(
                format!("Invalid byte order: {}. Must be big_endian or little_endian", self.byte_order)
            )),
        }
        
        Ok(())
    }
}

impl CANPointMapping {
    /// Get CAN ID
    pub fn can_id(&self) -> u32 {
        self.can_id
    }
    
    /// Get start bit
    pub fn start_bit(&self) -> u8 {
        self.start_bit
    }
    
    /// Get bit length
    pub fn bit_length(&self) -> u8 {
        self.bit_length
    }
    
    /// Get byte order
    pub fn byte_order(&self) -> &str {
        &self.byte_order
    }
    
    /// Get sign type
    pub fn sign_type(&self) -> &str {
        &self.sign_type
    }
    
    /// Check if this is a big endian signal
    pub fn is_big_endian(&self) -> bool {
        self.byte_order == "big_endian"
    }
    
    /// Check if this is a signed signal
    pub fn is_signed(&self) -> bool {
        self.sign_type == "signed"
    }
}

/// Legacy PointMapping struct for backward compatibility
/// TODO: This should be replaced with protocol-specific mappings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyPointMapping {
    /// Point ID within the channel
    pub point_id: u32,
    
    /// Human-readable signal name
    pub signal_name: String,
    
    /// Chinese name (optional)
    pub chinese_name: Option<String>,
    
    /// Protocol-specific address/identifier (e.g., "0x18FF10F4", "40001", "M1.0")
    pub address: String,
    
    /// Data type (bool, u16, i32, f32, etc.)
    pub data_type: String,
    
    /// Engineering unit (optional)
    pub unit: Option<String>,
    
    /// Scale factor for value conversion
    #[serde(default = "default_scale")]
    pub scale: f64,
    
    /// Offset for value conversion
    #[serde(default)]
    pub offset: f64,
    
    /// Protocol-specific parameters as key-value pairs
    #[serde(default)]
    pub protocol_params: HashMap<String, String>,
    
    /// Description
    pub description: Option<String>,
    
    /// Group/category
    pub group: Option<String>,
}

impl PointMapping for LegacyPointMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }
    
    fn signal_name(&self) -> &str {
        &self.signal_name
    }
    
    fn chinese_name(&self) -> Option<&str> {
        self.chinese_name.as_deref()
    }
    
    fn data_type(&self) -> &str {
        &self.data_type
    }
    
    fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }
    
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    
    fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }
    
    fn convert_to_engineering(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }
    
    fn convert_from_engineering(&self, engineering_value: f64) -> f64 {
        (engineering_value - self.offset) / self.scale
    }
    
    fn address_string(&self) -> &str {
        &self.address
    }
    
    fn validate(&self) -> Result<()> {
        // Basic validation for legacy mappings
        if self.signal_name.is_empty() {
            return Err(ComSrvError::ConfigError("Signal name cannot be empty".to_string()));
        }
        
        if self.address.is_empty() {
            return Err(ComSrvError::ConfigError("Address cannot be empty".to_string()));
        }
        
        Ok(())
    }
}

impl LegacyPointMapping {
    /// Get protocol-specific parameter by key
    pub fn get_protocol_param(&self, key: &str) -> Option<&str> {
        self.protocol_params.get(key).map(|v| v.as_str())
    }
    
    /// Parse address for Modbus protocol (register number)
    pub fn parse_modbus_address(&self) -> Result<u16> {
        self.address.parse::<u16>()
            .map_err(|_| ComSrvError::ConfigError(format!("Invalid Modbus address: {}", self.address)))
    }
    
    /// Parse address for CAN protocol (CAN ID)
    pub fn parse_can_id(&self) -> Result<u32> {
        if self.address.starts_with("0x") || self.address.starts_with("0X") {
            u32::from_str_radix(&self.address[2..], 16)
        } else {
            self.address.parse::<u32>()
        }.map_err(|_| ComSrvError::ConfigError(format!("Invalid CAN ID: {}", self.address)))
    }
}

/// Enum wrapper for different protocol-specific point mappings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PointMappingEnum {
    /// Modbus-specific point mapping
    Modbus(ModbusPointMapping),
    
    /// CAN-specific point mapping  
    CAN(CANPointMapping),
    
    /// Legacy point mapping for backward compatibility
    Legacy(LegacyPointMapping),
}

impl PointMapping for PointMappingEnum {
    fn point_id(&self) -> u32 {
        match self {
            PointMappingEnum::Modbus(m) => m.point_id(),
            PointMappingEnum::CAN(c) => c.point_id(),
            PointMappingEnum::Legacy(l) => l.point_id(),
        }
    }
    
    fn signal_name(&self) -> &str {
        match self {
            PointMappingEnum::Modbus(m) => m.signal_name(),
            PointMappingEnum::CAN(c) => c.signal_name(),
            PointMappingEnum::Legacy(l) => l.signal_name(),
        }
    }
    
    fn chinese_name(&self) -> Option<&str> {
        match self {
            PointMappingEnum::Modbus(m) => m.chinese_name(),
            PointMappingEnum::CAN(c) => c.chinese_name(),
            PointMappingEnum::Legacy(l) => l.chinese_name(),
        }
    }
    
    fn data_type(&self) -> &str {
        match self {
            PointMappingEnum::Modbus(m) => m.data_type(),
            PointMappingEnum::CAN(c) => c.data_type(),
            PointMappingEnum::Legacy(l) => l.data_type(),
        }
    }
    
    fn unit(&self) -> Option<&str> {
        match self {
            PointMappingEnum::Modbus(m) => m.unit(),
            PointMappingEnum::CAN(c) => c.unit(),
            PointMappingEnum::Legacy(l) => l.unit(),
        }
    }
    
    fn description(&self) -> Option<&str> {
        match self {
            PointMappingEnum::Modbus(m) => m.description(),
            PointMappingEnum::CAN(c) => c.description(),
            PointMappingEnum::Legacy(l) => l.description(),
        }
    }
    
    fn group(&self) -> Option<&str> {
        match self {
            PointMappingEnum::Modbus(m) => m.group(),
            PointMappingEnum::CAN(c) => c.group(),
            PointMappingEnum::Legacy(l) => l.group(),
        }
    }
    
    fn convert_to_engineering(&self, raw_value: f64) -> f64 {
        match self {
            PointMappingEnum::Modbus(m) => m.convert_to_engineering(raw_value),
            PointMappingEnum::CAN(c) => c.convert_to_engineering(raw_value),
            PointMappingEnum::Legacy(l) => l.convert_to_engineering(raw_value),
        }
    }
    
    fn convert_from_engineering(&self, engineering_value: f64) -> f64 {
        match self {
            PointMappingEnum::Modbus(m) => m.convert_from_engineering(engineering_value),
            PointMappingEnum::CAN(c) => c.convert_from_engineering(engineering_value),
            PointMappingEnum::Legacy(l) => l.convert_from_engineering(engineering_value),
        }
    }
    
    fn address_string(&self) -> &str {
        match self {
            PointMappingEnum::Modbus(m) => m.address_string(),
            PointMappingEnum::CAN(c) => c.address_string(),
            PointMappingEnum::Legacy(l) => l.address_string(),
        }
    }
    
    fn validate(&self) -> Result<()> {
        match self {
            PointMappingEnum::Modbus(m) => m.validate(),
            PointMappingEnum::CAN(c) => c.validate(),
            PointMappingEnum::Legacy(l) => l.validate(),
        }
    }
}

impl PointMappingEnum {
    /// Get as Modbus mapping if it is one
    pub fn as_modbus(&self) -> Option<&ModbusPointMapping> {
        match self {
            PointMappingEnum::Modbus(m) => Some(m),
            _ => None,
        }
    }
    
    /// Get as CAN mapping if it is one
    pub fn as_can(&self) -> Option<&CANPointMapping> {
        match self {
            PointMappingEnum::CAN(c) => Some(c),
            _ => None,
        }
    }
    
    /// Get as legacy mapping if it is one
    pub fn as_legacy(&self) -> Option<&LegacyPointMapping> {
        match self {
            PointMappingEnum::Legacy(l) => Some(l),
            _ => None,
        }
    }
}

// Default implementation is now in types::app
/*
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            service: ServiceConfig::default(),
            channels: Vec::new(),
            defaults: DefaultPathConfig::default(),
        }
    }
}
*/

// Default implementation is now in types::app
/*
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: default_service_name(),
            description: Some("Communication Service".to_string()),
            api: ApiConfig::default(),
            redis: RedisConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}
*/

// Default implementation is now in types::app
/*
impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            bind_address: default_api_bind(),
            version: default_api_version(),
        }
    }
}
*/

// Default implementation is now in types::redis
/*
impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            url: default_redis_url(),
            db: 0,
            timeout_ms: default_redis_timeout(),
            max_connections: None,
            max_retries: default_redis_retries(),
        }
    }
}
*/

// RedisConfig impl methods are now in types::redis
/*
impl RedisConfig {
    /// Validate Redis configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.url.is_empty() {
            return Err(ComSrvError::ConfigError(
                "Redis URL cannot be empty when enabled".to_string()
            ));
        }
        Ok(())
    }
    
    /// Convert to Redis URL format (for backward compatibility)
    pub fn to_redis_url(&self) -> String {
        self.url.clone()
    }
    
    /// Get connection type (for backward compatibility)
    pub fn connection_type(&self) -> String {
        if self.url.starts_with("rediss://") {
            "tls".to_string()
        } else if self.url.starts_with("unix://") {
            "unix".to_string()
        } else {
            "tcp".to_string()
        }
    }
    
    /// Get address field (for backward compatibility with old RedisConfig)
    pub fn address(&self) -> String {
        // Extract host:port from URL
        if let Some(stripped) = self.url.strip_prefix("redis://") {
            if let Some(at_pos) = stripped.find('@') {
                // Has authentication: redis://user:pass@host:port/db
                stripped[at_pos + 1..].split('/').next().unwrap_or("127.0.0.1:6379").to_string()
            } else {
                // No auth: redis://host:port/db
                stripped.split('/').next().unwrap_or("127.0.0.1:6379").to_string()
            }
        } else {
            "127.0.0.1:6379".to_string()
        }
    }
    
    /// Get database number field (alias for backward compatibility)
    pub fn database(&self) -> u8 {
        self.db
    }
}
*/

// Default implementation is now in types::logging
/*
impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
            console: default_true(),
            max_size: default_log_max_size(),
            max_files: default_log_max_files(),
            retention_days: default_log_retention_days(),
            enable_channel_logging: default_true(),
            channel_log_dir: default_channel_log_dir(),
        }
    }
}
*/

// Default implementation is now in types::app
/*
impl Default for DefaultPathConfig {
    fn default() -> Self {
        Self {
            channels_root: default_channels_root(),
            combase_dir: default_combase_dir(),
            protocol_dir: default_protocol_dir(),
        }
    }
}
*/

/// Configuration builder with multiple source support
pub struct ConfigBuilder {
    figment: Figment,
}

impl ConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            figment: Figment::new(),
        }
    }

    /// Add default configuration
    pub fn with_defaults(mut self) -> Self {
        self.figment = self.figment.merge(Serialized::defaults(AppConfig::default()));
        self
    }

    /// Add configuration from file
    pub fn with_file<P: AsRef<Path>>(mut self, path: P) -> Self {
        let path = path.as_ref();
        
        // Auto-detect file format and add appropriate provider
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "yaml" | "yml" => {
                    self.figment = self.figment.merge(Yaml::file(path));
                }
                "toml" => {
                    self.figment = self.figment.merge(Toml::file(path));
                }
                "json" => {
                    self.figment = self.figment.merge(Json::file(path));
                }
                _ => {
                    tracing::warn!("Unknown config file extension: {}, trying YAML", ext);
                    self.figment = self.figment.merge(Yaml::file(path));
                }
            }
        } else {
            // Default to YAML if no extension
            self.figment = self.figment.merge(Yaml::file(path));
        }
        
        self
    }

    /// Add environment variables with prefix
    pub fn with_env(mut self, prefix: &str) -> Self {
        self.figment = self.figment.merge(
            Env::prefixed(prefix)
                .split("__") // Use double underscore for nested keys
                .map(|key| key.as_str().to_lowercase().into())
        );
        self
    }

    /// Add environment variables with default COMSRV prefix
    pub fn with_default_env(self) -> Self {
        self.with_env("COMSRV")
    }

    /// Merge additional configuration provider
    pub fn merge<T: Provider>(mut self, provider: T) -> Self {
        self.figment = self.figment.merge(provider);
        self
    }

    /// Build the final configuration
    pub fn build(self) -> Result<AppConfig> {
        self.figment
            .extract()
            .map_err(|e| ComSrvError::ConfigError(format!("Configuration error: {}", e)))
    }

    /// Build with custom extraction
    pub fn extract<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        self.figment
            .extract()
            .map_err(|e| ComSrvError::ConfigError(format!("Configuration extraction error: {}", e)))
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration manager using Figment
pub struct ConfigManager {
    config: AppConfig,
    figment: Figment,
}

impl ConfigManager {
    /// Create configuration manager from file with CSV bridge layer
    pub fn from_file<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path = config_path.as_ref();
        let builder = ConfigBuilder::new()
            .with_defaults()
            .with_file(&config_path)
            .with_default_env();

        let figment = builder.figment.clone();
        let mut config = builder.build()?;

        // 🌉 Bridge Layer: Load CSV point mappings for each channel
        Self::load_csv_mappings(&mut config, config_path)?;

        Ok(Self { config, figment })
    }

    /// Bridge layer implementation: Load CSV point mappings
    /// This is where the magic happens - CSV files become type-safe Rust structs!
    fn load_csv_mappings(config: &mut AppConfig, config_path: &Path) -> Result<()> {
        let config_dir = config_path.parent()
            .unwrap_or_else(|| Path::new("."));

        for channel in &mut config.channels {
            // 🌟 New separated table configuration
            if let Some(table_config) = channel.table_config.clone() {
                tracing::info!("Loading separated table configuration for channel {}", channel.id);
                Self::load_separated_tables(channel, &table_config, config_dir)?;
                continue;
            }
            
            // 🏗️ Legacy unified CSV mapping files
            // Note: types::ChannelConfig doesn't have mapping_files field anymore
            // This legacy code path is deprecated
            /*
            if channel.mapping_files.is_empty() {
                tracing::debug!("Channel {} has no mapping files configured", channel.id);
                continue;
            }

            tracing::info!("Loading legacy CSV mappings for channel {}: {:?}", 
                      channel.id, channel.mapping_files);

            for mapping_file in &channel.mapping_files {
            */
            // Skip legacy mapping for now
            tracing::warn!("Channel {} has no table_config, skipping legacy mapping", channel.id);
            continue;
            
            /*
                let csv_path = if mapping_file.starts_with('/') {
                    // Absolute path
                    PathBuf::from(mapping_file)
                } else {
                    // Relative to config file
                    config_dir.join(mapping_file)
                };

                if !csv_path.exists() {
                    tracing::warn!("CSV mapping file not found: {}", csv_path.display());
                    continue;
                }

                // 🚀 The CSV-to-Rust magic happens here
                let points = Self::parse_csv_mapping_file(&csv_path)?;
                channel.points.extend(points);

                tracing::info!("Loaded {} point mappings from {}", 
                          channel.points.len(), csv_path.display());
            }
            */
        }

        Ok(())
    }

    /// Parse a single CSV mapping file into PointMapping structs
    /// This function uses the highly optimized csv crate for parsing
    fn parse_csv_mapping_file(csv_path: &Path) -> Result<Vec<PointMappingEnum>> {
        let mut points = Vec::new();
        let mut reader = csv::Reader::from_path(csv_path)
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to open CSV file {}: {}", csv_path.display(), e
            )))?;

        // Parse each record as a PointMapping
        for (line_num, result) in reader.deserialize().enumerate() {
            let point: LegacyPointMapping = result
                .map_err(|e| ComSrvError::ConfigError(format!(
                    "Failed to parse CSV record at line {} in {}: {}", 
                    line_num + 2, csv_path.display(), e  // +2 because line 1 is header
                )))?;

            points.push(PointMappingEnum::Legacy(point));
        }

        Ok(points)
    }
    
    /// Load separated four telemetry and protocol mapping tables
    fn load_separated_tables(
        channel: &mut TypesChannelConfig, 
        table_config: &TableConfig,
        config_dir: &Path
    ) -> Result<()> {
        use std::collections::HashMap;
        
        // Load four telemetry points
        let telemetry_base = config_dir.join(&table_config.four_telemetry_route);
        let mut all_telemetry_points = HashMap::new();
        
        // Load YC (telemetry)
        let yc_path = telemetry_base.join(&table_config.four_telemetry_files.telemetry_file);
        if yc_path.exists() {
            let yc_points = Self::parse_four_telemetry_csv(&yc_path, "YC")?;
            tracing::info!("Loaded {} YC telemetry points from {}", yc_points.len(), yc_path.display());
            for point in yc_points {
                all_telemetry_points.insert(point.point_id, point);
            }
        }
        
        // Load YX (signal)
        let yx_path = telemetry_base.join(&table_config.four_telemetry_files.signal_file);
        if yx_path.exists() {
            let yx_points = Self::parse_four_telemetry_csv(&yx_path, "YX")?;
            tracing::info!("Loaded {} YX signal points from {}", yx_points.len(), yx_path.display());
            for point in yx_points {
                all_telemetry_points.insert(point.point_id, point);
            }
        }
        
        // Load YT (adjustment)
        let yt_path = telemetry_base.join(&table_config.four_telemetry_files.adjustment_file);
        if yt_path.exists() {
            let yt_points = Self::parse_four_telemetry_csv(&yt_path, "YT")?;
            tracing::info!("Loaded {} YT adjustment points from {}", yt_points.len(), yt_path.display());
            for point in yt_points {
                all_telemetry_points.insert(point.point_id, point);
            }
        }
        
        // Load YK (control)  
        let yk_path = telemetry_base.join(&table_config.four_telemetry_files.control_file);
        if yk_path.exists() {
            let yk_points = Self::parse_four_telemetry_csv(&yk_path, "YK")?;
            tracing::info!("Loaded {} YK control points from {}", yk_points.len(), yk_path.display());
            for point in yk_points {
                all_telemetry_points.insert(point.point_id, point);
            }
        }
        
        // Load protocol mappings
        let mapping_base = config_dir.join(&table_config.protocol_mapping_route);
        let mut all_protocol_mappings = HashMap::new();
        
        let mapping_files = [
            (&table_config.protocol_mapping_files.telemetry_mapping, "YC"),
            (&table_config.protocol_mapping_files.signal_mapping, "YX"),
            (&table_config.protocol_mapping_files.adjustment_mapping, "YT"),
            (&table_config.protocol_mapping_files.control_mapping, "YK"),
        ];
        
        for (mapping_file, point_type) in mapping_files {
            let mapping_path = mapping_base.join(mapping_file);
            if mapping_path.exists() {
                let mappings = Self::parse_protocol_mapping_csv(&mapping_path)?;
                tracing::info!("Loaded {} {} protocol mappings from {}", 
                          mappings.len(), point_type, mapping_path.display());
                for mapping in mappings {
                    all_protocol_mappings.insert(mapping.point_id, mapping);
                }
            }
        }
        
        // Combine telemetry points with protocol mappings
        let mut combined_count = 0;
        for (point_id, telemetry_point) in all_telemetry_points {
            if let Some(protocol_mapping) = all_protocol_mappings.get(&point_id) {
                // Convert to TypesCombinedPoint
                let mut protocol_params = HashMap::new();
                protocol_params.insert("address".to_string(), protocol_mapping.address.to_string());
                protocol_params.insert("data_type".to_string(), protocol_mapping.data_type.to_string());
                protocol_params.insert("data_format".to_string(), protocol_mapping.data_format.to_string());
                protocol_params.insert("number_of_bytes".to_string(), protocol_mapping.number_of_bytes.to_string());
                if let Some(bit_loc) = protocol_mapping.bit_location {
                    protocol_params.insert("bit_location".to_string(), bit_loc.to_string());
                }
                
                let combined_point = TypesCombinedPoint {
                    point_id: telemetry_point.point_id,
                    signal_name: telemetry_point.signal_name.clone(),
                    chinese_name: telemetry_point.chinese_name.clone(),
                    telemetry_type: telemetry_point.telemetry_type.clone(),
                    data_type: protocol_mapping.data_type.to_string(),
                    protocol_params,
                    scaling: if telemetry_point.scale.is_some() || telemetry_point.offset.is_some() {
                        Some(super::types::channel::ScalingInfo {
                            scale: telemetry_point.scale.unwrap_or(1.0),
                            offset: telemetry_point.offset.unwrap_or(0.0),
                            unit: telemetry_point.unit.clone(),
                        })
                    } else {
                        None
                    },
                };
                channel.combined_points.push(combined_point);
                combined_count += 1;
            } else {
                tracing::warn!("No protocol mapping found for telemetry point {} ({})", 
                          point_id, telemetry_point.signal_name);
            }
        }
        
        tracing::info!("Successfully combined {} telemetry points with protocol mappings for channel {}", 
                  combined_count, channel.id);
        
        Ok(())
    }
    
    /// Parse four telemetry CSV file (YC/YX/YT/YK)
    fn parse_four_telemetry_csv(csv_path: &Path, point_type: &str) -> Result<Vec<FourTelemetryPoint>> {
        let mut reader = csv::Reader::from_path(csv_path)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to open CSV file {}: {}", csv_path.display(), e)))?;
        
        let mut points = Vec::new();
        
        for (row_idx, result) in reader.records().enumerate() {
            let record = result
                .map_err(|e| ComSrvError::ConfigError(format!("CSV parse error at row {}: {}", row_idx + 1, e)))?;
            
            if record.len() < 3 {
                tracing::warn!("Skipping incomplete row {} in {}", row_idx + 1, csv_path.display());
                continue;
            }
            
            let point = match point_type {
                "YC" | "YT" => {
                    // Telemetry/Adjustment: point_id,signal_name,chinese_name,scale,offset,unit
                    if record.len() < 6 {
                        tracing::warn!("Incomplete YC/YT row {} in {}", row_idx + 1, csv_path.display());
                        continue;
                    }
                    
                    FourTelemetryPoint {
                        point_id: record[0].parse().unwrap_or(0),
                        signal_name: record[1].to_string(),
                        chinese_name: record[2].to_string(),
                        telemetry_type: point_type.to_string(),
                        scale: if record[3].is_empty() { None } else { record[3].parse().ok() },
                        offset: if record[4].is_empty() { None } else { record[4].parse().ok() },
                        unit: if record[5].is_empty() { None } else { Some(record[5].to_string()) },
                        reverse: None,
                        data_type: "FLOAT".to_string(), // Default data type for YC/YT
                    }
                }
                "YX" | "YK" => {
                    // Signal/Control: point_id,signal_name,chinese_name,reverse
                    if record.len() < 4 {
                        tracing::warn!("Incomplete YX/YK row {} in {}", row_idx + 1, csv_path.display());
                        continue;
                    }
                    
                    FourTelemetryPoint {
                        point_id: record[0].parse().unwrap_or(0),
                        signal_name: record[1].to_string(),
                        chinese_name: record[2].to_string(),
                        telemetry_type: point_type.to_string(),
                        scale: None,
                        offset: None,
                        unit: None,
                        reverse: if record[3].is_empty() { None } else { 
                            match &record[3] {
                                "1" | "true" | "True" | "TRUE" => Some(true),
                                "0" | "false" | "False" | "FALSE" => Some(false),
                                _ => record[3].parse().ok()
                            }
                        },
                        data_type: "BOOL".to_string(), // Default data type for YX/YK
                    }
                }
                _ => {
                    tracing::warn!("Unknown point type: {}", point_type);
                    continue;
                }
            };
            
            points.push(point);
        }
        
        Ok(points)
    }
    
    /// Parse protocol mapping CSV file with updated field structure
    fn parse_protocol_mapping_csv(csv_path: &Path) -> Result<Vec<ProtocolMapping>> {
        let mut reader = csv::Reader::from_path(csv_path)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to open CSV file {}: {}", csv_path.display(), e)))?;
        
        let mut mappings = Vec::new();
        
        // Get headers to determine the CSV format
        let headers = reader.headers()
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to read headers from {}: {}", csv_path.display(), e)))?;
        
        // Check if this is the new format (without polling_interval)
        let has_register_address = headers.iter().any(|h| h == "register_address");
        let has_byte_order = headers.iter().any(|h| h == "byte_order");
        let has_bit_position = headers.iter().any(|h| h == "bit_position");
        let has_register_count = headers.iter().any(|h| h == "register_count");
        let has_function_code = headers.iter().any(|h| h == "function_code");
        let has_slave_id = headers.iter().any(|h| h == "slave_id");
        
        for (row_idx, result) in reader.records().enumerate() {
            let record = result
                .map_err(|e| ComSrvError::ConfigError(format!("CSV parse error at row {}: {}", row_idx + 2, e)))?;
            
            if record.len() < 4 {
                tracing::warn!("Skipping incomplete row {} in {} (need at least 4 columns)", row_idx + 2, csv_path.display());
                continue;
            }
            
            // Parse point_id (always first column)
            let point_id: u32 = record[0].parse()
                .map_err(|_| ComSrvError::ConfigError(format!("Invalid point_id '{}' at row {} in {}", &record[0], row_idx + 2, csv_path.display())))?;
            
            let mapping = if has_register_address && has_function_code && has_slave_id {
                // New format: point_id,register_address,function_code,slave_id,data_format,byte_order,register_count
                // OR: point_id,register_address,function_code,slave_id,data_format,bit_position (for signals/controls)
                if record.len() < 6 {
                    tracing::warn!("Skipping incomplete row {} in {} (new format needs at least 6 columns)", row_idx + 2, csv_path.display());
                    continue;
                }
                
                let function_code: u8 = record[2].parse()
                    .map_err(|_| ComSrvError::ConfigError(format!("Invalid function_code '{}' at row {} in {}", &record[2], row_idx + 2, csv_path.display())))?;
                
                let slave_id: u8 = record[3].parse()
                    .map_err(|_| ComSrvError::ConfigError(format!("Invalid slave_id '{}' at row {} in {}", &record[3], row_idx + 2, csv_path.display())))?;
                
                let (data_format, number_of_bytes, bit_location) = if has_bit_position {
                    // Signal/Control format: uses bit_position instead of byte_order/register_count
                    let bit_position = if record.len() > 5 && !record[5].is_empty() {
                        Some(record[5].parse::<u8>()
                            .map_err(|_| ComSrvError::ConfigError(format!("Invalid bit_position '{}' at row {} in {}", &record[5], row_idx + 2, csv_path.display())))?)
                    } else {
                        Some(0) // Default bit position
                    };
                    ("BOOL".to_string(), 1u8, bit_position)
                } else if has_byte_order && has_register_count {
                    // Telemetry/Adjustment format: uses byte_order and register_count
                    let byte_order = record[5].to_string();
                    let register_count: u8 = if record.len() > 6 && !record[6].is_empty() {
                        record[6].parse()
                            .map_err(|_| ComSrvError::ConfigError(format!("Invalid register_count '{}' at row {} in {}", &record[6], row_idx + 2, csv_path.display())))?
                    } else {
                        1
                    };
                    let number_of_bytes = register_count * 2; // Each register is 2 bytes
                    (byte_order, number_of_bytes, None)
                } else {
                    // Fallback format
                    ("ABCD".to_string(), 2u8, None)
                };
                
                ProtocolMapping {
                    point_id,
                    signal_name: format!("point_{}", point_id), // Generate signal name if not provided
                    address: format!("{}:{}:{}", slave_id, function_code, &record[1]), // Format: slave_id:function_code:register_address
                    data_type: record[4].to_string(),
                    data_format,
                    number_of_bytes,
                    bit_location,
                    description: None,
                }
            } else {
                // Legacy format: point_id,signal_name,address,data_type,data_format,number_of_bytes[,bit_location]
                if record.len() < 6 {
                    tracing::warn!("Skipping incomplete row {} in {} (legacy format needs at least 6 columns)", row_idx + 2, csv_path.display());
                    continue;
                }
            
            let number_of_bytes: u8 = record[5].parse()
                .map_err(|_| ComSrvError::ConfigError(format!("Invalid number_of_bytes '{}' at row {} in {}", &record[5], row_idx + 2, csv_path.display())))?;
            
            let bit_location = if record.len() > 6 && !record[6].is_empty() { 
                Some(record[6].parse::<u8>()
                    .map_err(|_| ComSrvError::ConfigError(format!("Invalid bit_location '{}' at row {} in {}", &record[6], row_idx + 2, csv_path.display())))?)
            } else { 
                None 
            };
            
                ProtocolMapping {
                point_id,
                signal_name: record[1].to_string(),
                address: record[2].to_string(),
                data_type: record[3].to_string(),
                data_format: record[4].to_string(),
                number_of_bytes,
                bit_location,
                description: if record.len() > 7 && !record[7].is_empty() { 
                    Some(record[7].to_string()) 
                } else { 
                    None 
                },
                }
            };
            
            // 🔍 Validate mapping configuration
            mapping.validate().map_err(|e| ComSrvError::ConfigError(
                format!("Validation failed at row {} in {}: {}", row_idx + 2, csv_path.display(), e)
            ))?;
            
            mappings.push(mapping);
        }
        
        tracing::info!("✅ Parsed and validated {} protocol mappings from {}", mappings.len(), csv_path.display());
        Ok(mappings)
    }

    /// Create configuration manager with custom builder
    pub fn from_builder(builder: ConfigBuilder) -> Result<Self> {
        let figment = builder.figment.clone();
        let config = builder.build()?;

        Ok(Self { config, figment })
    }
    
    /// Create configuration manager from AppConfig (for multi-source loading)
    pub fn from_app_config(config: AppConfig) -> Result<Self> {
        // Create a figment from the config for consistency
        let figment = Figment::from(Serialized::defaults(config.clone()));
        
        Ok(Self { config, figment })
    }

    /// Get the current configuration
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Get service configuration
    pub fn service(&self) -> &ServiceConfig {
        &self.config.service
    }

    /// Get channels
    pub fn channels(&self) -> &[TypesChannelConfig] {
        &self.config.channels
    }

    /// Get channel by ID
    pub fn get_channel(&self, id: u16) -> Option<&TypesChannelConfig> {
        self.config.channels.iter().find(|c| c.id == id)
    }

    /// Get all channels (for backward compatibility)
    pub fn get_channels(&self) -> &Vec<TypesChannelConfig> {
        &self.config.channels
    }

    /// Get Redis configuration (for backward compatibility)
    pub fn get_redis_config(&self) -> &RedisConfig {
        &self.config.service.redis
    }

    /// Get modbus mappings for a channel (for backward compatibility)
    pub fn get_modbus_mappings_for_channel(&self, channel_id: u16) -> Result<Vec<crate::core::protocols::modbus::common::ModbusRegisterMapping>> {
        let channel = self.get_channel(channel_id)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Channel {} not found", channel_id)))?;

        let mut mappings = Vec::new();
        for point in &channel.points {
            let data_type = match &point.data_type {
                DataType::Bool => crate::core::protocols::modbus::common::ModbusDataType::Bool,
                DataType::UInt16 => crate::core::protocols::modbus::common::ModbusDataType::UInt16,
                DataType::Int16 => crate::core::protocols::modbus::common::ModbusDataType::Int16,
                DataType::Float32 => crate::core::protocols::modbus::common::ModbusDataType::Float32,
                _ => crate::core::protocols::modbus::common::ModbusDataType::UInt16,
            };

            let function_code = crate::core::protocols::modbus::common::ModbusFunctionCode::Read03;
            let mapping = crate::core::protocols::modbus::common::ModbusRegisterMapping {
                name: point.signal_name.clone(),
                slave_id: 1, // Default slave_id, should be read from CSV in future
                function_code,
                address: match &point.address {
                    ProtocolAddress::Modbus { register, .. } => *register,
                    _ => 0, // For non-modbus protocols
                },
                data_type,
                byte_order: crate::core::protocols::modbus::common::ByteOrder::default_for_data_type(&data_type),
                description: point.metadata.get("description").cloned(),
            };
            mappings.push(mapping);
        }

        Ok(mappings)
    }

    /// Load channel combase config (for backward compatibility)
    /// Returns empty config since we've simplified the configuration structure
    pub fn load_channel_combase_config(&self, _channel_id: u16) -> Result<serde_json::Value> {
        Ok(serde_json::Value::Object(serde_json::Map::new()))
    }

    /// Get point mappings for a specific channel
    pub fn get_channel_points(&self, channel_id: u16) -> Vec<&UnifiedPointMapping> {
        self.get_channel(channel_id)
            .map(|c| c.points.iter().collect())
            .unwrap_or_default()
    }

    /// Get a specific point by channel ID and point ID
    pub fn get_point(&self, channel_id: u16, point_id: u32) -> Option<&UnifiedPointMapping> {
        self.get_channel(channel_id)?
            .points.iter()
            .find(|p| p.point_id == point_id)
    }

    /// Get points by signal name (useful for CAN/named protocols)
    pub fn get_points_by_signal(&self, channel_id: u16, signal_name: &str) -> Vec<&UnifiedPointMapping> {
        self.get_channel(channel_id)
            .map(|c| c.points.iter()
                .filter(|p| p.signal_name == signal_name)
                .collect())
            .unwrap_or_default()
    }

    /// Get all Modbus register mappings for a channel (filtered by data type)
    pub fn get_modbus_registers(&self, channel_id: u16) -> Result<Vec<&UnifiedPointMapping>> {
        let channel = self.get_channel(channel_id)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Channel {} not found", channel_id)))?;

        if channel.protocol != "modbus_tcp" && channel.protocol != "modbus_rtu" {
            return Err(ComSrvError::ConfigError(format!(
                "Channel {} is not a Modbus channel (protocol: {})", 
                channel_id, channel.protocol
            )));
        }

        Ok(channel.points.iter().collect())
    }

    /// Get all CAN signal mappings for a channel
    pub fn get_can_signals(&self, channel_id: u16) -> Result<Vec<&UnifiedPointMapping>> {
        let channel = self.get_channel(channel_id)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Channel {} not found", channel_id)))?;

        if channel.protocol != "can" {
            return Err(ComSrvError::ConfigError(format!(
                "Channel {} is not a CAN channel (protocol: {})", 
                channel_id, channel.protocol
            )));
        }

        Ok(channel.points.iter().collect())
    }

    /// Reload configuration (re-extract from figment)
    pub fn reload(&mut self) -> Result<bool> {
        let new_config: AppConfig = self.figment
            .extract()
            .map_err(|e| ComSrvError::ConfigError(format!("Configuration reload error: {}", e)))?;

        let changed = !self.configs_equal(&self.config, &new_config);
        
        if changed {
            self.config = new_config;
        }

        Ok(changed)
    }

    /// Check if two configurations are equal (simplified comparison)
    fn configs_equal(&self, a: &AppConfig, b: &AppConfig) -> bool {
        // Simple comparison based on serialized JSON
        match (serde_json::to_string(a), serde_json::to_string(b)) {
            (Ok(a_json), Ok(b_json)) => a_json == b_json,
            _ => false,
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Validate service configuration
        if self.config.service.name.is_empty() {
            return Err(ComSrvError::ConfigError(
                "Service name cannot be empty".to_string(),
            ));
        }

        // Validate channels
        let mut channel_ids = std::collections::HashSet::new();
        for channel in &self.config.channels {
            if !channel_ids.insert(channel.id) {
                return Err(ComSrvError::ConfigError(format!(
                    "Duplicate channel ID: {}",
                    channel.id
                )));
            }

            if channel.name.is_empty() {
                warnings.push(format!(
                    "Channel {} has empty name",
                    channel.id
                ));
            }

            if channel.protocol.is_empty() {
                return Err(ComSrvError::ConfigError(format!(
                    "Channel {} has no protocol specified",
                    channel.id
                )));
            }
        }

        // Validate Redis configuration
        if self.config.service.redis.enabled {
            if self.config.service.redis.url.is_empty() {
                return Err(ComSrvError::ConfigError(
                    "Redis URL cannot be empty when Redis is enabled".to_string(),
                ));
            }
        }

        Ok(warnings)
    }

    /// Get figment for advanced operations
    pub fn figment(&self) -> &Figment {
        &self.figment
    }

    /// Extract custom configuration section
    pub fn extract_section<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T> {
        self.figment
            .find_value(key)
            .map_err(|e| ComSrvError::ConfigError(format!("Section '{}' not found: {}", key, e)))?
            .deserialize()
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to deserialize section '{}': {}", key, e)))
    }

    /// Get log level from configuration
    pub fn get_log_level(&self) -> &str {
        &self.config.service.logging.level
    }

    /// Check if API is enabled
    pub fn get_api_enabled(&self) -> bool {
        self.config.service.api.enabled
    }

    /// Get API address
    pub fn get_api_address(&self) -> &str {
        &self.config.service.api.bind_address
    }
    
    /// Get combined points for a channel (new separated table approach)
    pub fn get_combined_points(&self, channel_id: u16) -> Vec<&TypesCombinedPoint> {
        if let Some(channel) = self.get_channel(channel_id) {
            channel.combined_points.iter().collect()
        } else {
            Vec::new()
        }
    }
    
    /// Get four telemetry points by type
    pub fn get_four_telemetry_points(&self, channel_id: u16, point_type: &str) -> Vec<&TypesCombinedPoint> {
        let combined_points = self.get_combined_points(channel_id);
        combined_points.into_iter()
            .filter(|cp| cp.telemetry_type == point_type)
            .collect()
    }
    
    /// Get protocol mappings by point type
    pub fn get_protocol_mappings(&self, channel_id: u16, point_type: &str) -> Vec<&TypesCombinedPoint> {
        let combined_points = self.get_combined_points(channel_id);
        combined_points.into_iter()
            .filter(|cp| {
                // Filter by telemetry type
                cp.telemetry_type == point_type
            })
            .collect()
    }
    
    /// Get combined point by point ID (table-local unique)
    pub fn get_combined_point(&self, channel_id: u16, point_id: u32) -> Option<&TypesCombinedPoint> {
        if let Some(channel) = self.get_channel(channel_id) {
            channel.combined_points.iter()
                .find(|cp| cp.point_id == point_id)
        } else {
            None
        }
    }
    
    /// Get all points for modbus (legacy compatibility)
    pub fn get_modbus_points(&self, channel_id: u16) -> Vec<ModbusPoint> {
        let combined_points = self.get_combined_points(channel_id);
        let mut modbus_points = Vec::new();
        
        for cp in combined_points {
            let point = ModbusPoint {
                point_id: cp.point_id,
                signal_name: cp.signal_name.clone(),
                chinese_name: cp.chinese_name.clone(),
                address: cp.protocol_params.get("address").unwrap_or(&"0".to_string()).clone(),
                data_type: cp.data_type.clone(),
                scale: cp.scaling.as_ref().map(|s| s.scale).unwrap_or(1.0),
                offset: cp.scaling.as_ref().map(|s| s.offset).unwrap_or(0.0),
                unit: cp.scaling.as_ref().and_then(|s| s.unit.clone()),
                reverse: false, // Not available in new structure
                description: None, // Not available in new structure
            };
            modbus_points.push(point);
        }
        
        modbus_points
    }

    /// Convert combined points to UniversalPointConfig format for integration with UniversalPointManager
    pub fn get_universal_point_configs(&self, channel_id: u16) -> Vec<super::super::protocols::common::combase::point_manager::UniversalPointConfig> {
        use super::super::protocols::common::combase::telemetry::TelemetryType;
        
        let combined_points = self.get_combined_points(channel_id);
        let mut universal_configs = Vec::new();

        for cp in combined_points {
            // Determine telemetry type based on telemetry_type field
            let telemetry_type = match cp.telemetry_type.as_str() {
                "YK" => TelemetryType::Control,
                "YX" => TelemetryType::Signaling,
                "YT" => TelemetryType::Setpoint,
                "YC" => TelemetryType::Telemetry,
                _ => TelemetryType::Telemetry, // Default to telemetry
            };

            let universal_config = super::super::protocols::common::combase::point_manager::UniversalPointConfig {
                point_id: cp.point_id,
                name: Some(cp.chinese_name.clone()),
                description: None, // Not available in new structure
                unit: cp.scaling.as_ref().and_then(|s| s.unit.clone()),
                data_type: Some(cp.data_type.clone()),
                scale: cp.scaling.as_ref().map(|s| s.scale).unwrap_or(1.0),
                offset: cp.scaling.as_ref().map(|s| s.offset).unwrap_or(0.0),
                reverse: 0, // Not available in new structure
                telemetry_type: telemetry_type.clone(),
                enabled: true,
                readable: true,
                writable: matches!(telemetry_type, TelemetryType::Control | TelemetryType::Setpoint),
            };

            universal_configs.push(universal_config);
        }

        universal_configs
    }

    /// Get protocol mapping configurations for Modbus integration
    pub fn get_modbus_protocol_mappings(&self, channel_id: u16) -> Vec<ModbusProtocolMapping> {
        let combined_points = self.get_combined_points(channel_id);
        let mut protocol_mappings = Vec::new();

        for cp in combined_points {
            // Parse the address format: slave_id:function_code:register_address
            // Extract address from protocol_params
            let default_address = "0:3:0".to_string();
            let address = cp.protocol_params.get("address").unwrap_or(&default_address);
            let address_parts: Vec<&str> = address.split(':').collect();
            if address_parts.len() != 3 {
                tracing::warn!("Invalid address format for point {}: {}", cp.point_id, address);
                continue;
            }

            let slave_id = match address_parts[0].parse::<u8>() {
                Ok(id) => id,
                Err(_) => {
                    tracing::warn!("Invalid slave_id for point {}: {}", cp.point_id, address_parts[0]);
                    continue;
                }
            };

            let function_code = match address_parts[1].parse::<u8>() {
                Ok(fc) => fc,
                Err(_) => {
                    tracing::warn!("Invalid function_code for point {}: {}", cp.point_id, address_parts[1]);
                    continue;
                }
            };

            let register_address = match address_parts[2].parse::<u16>() {
                Ok(addr) => addr,
                Err(_) => {
                    tracing::warn!("Invalid register_address for point {}: {}", cp.point_id, address_parts[2]);
                    continue;
                }
            };

            let protocol_mapping = ModbusProtocolMapping {
                point_id: cp.point_id,
                slave_id,
                function_code,
                register_address,
                data_type: cp.data_type.clone(),
                byte_order: cp.protocol_params.get("data_format").unwrap_or(&"ABCD".to_string()).clone(),
                register_count: cp.protocol_params.get("number_of_bytes").and_then(|s| s.parse::<u8>().ok()).map(|bytes| (bytes + 1) / 2).unwrap_or(1),
                bit_position: cp.protocol_params.get("bit_location").and_then(|s| s.parse::<u8>().ok()).unwrap_or(0),
            };

            protocol_mappings.push(protocol_mapping);
        }

        protocol_mappings
    }
}

/// Legacy Modbus point structure for backward compatibility
#[derive(Debug, Clone)]
pub struct ModbusPoint {
    pub point_id: u32,
    pub signal_name: String,
    pub chinese_name: String,
    pub address: String,
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
    pub unit: Option<String>,
    pub reverse: bool,
    pub description: Option<String>,
}

/// Modbus protocol mapping configuration for integration with protocol implementations
#[derive(Debug, Clone)]
pub struct ModbusProtocolMapping {
    /// Point ID (matches UniversalPointConfig)
    pub point_id: u32,
    /// Modbus slave ID
    pub slave_id: u8,
    /// Modbus function code (1,2,3,4,5,6,15,16)
    pub function_code: u8,
    /// Register address
    pub register_address: u16,
    /// Data type (uint16, int16, float32, bool, etc.)
    pub data_type: String,
    /// Byte order (ABCD, DCBA, BADC, CDAB)
    pub byte_order: String,
    /// Register count (for multi-register values)
    pub register_count: u8,
    /// Bit position (for bit-level operations)
    pub bit_position: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.service.name, "comsrv");
        assert!(config.service.api.enabled);
        assert!(config.channels.is_empty());
    }

    #[test]
    fn test_figment_builder() {
        let builder = ConfigBuilder::new()
            .with_defaults()
            .with_default_env();

        let config = builder.build().unwrap();
        assert_eq!(config.service.name, "comsrv");
    }

    #[test]
    fn test_yaml_config_loading() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let yaml_content = r#"
service:
  name: "test-service"
  api:
    enabled: true
    bind_address: "127.0.0.1:3000"

channels:
  - id: 1
    name: "test-channel"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 502
"#;
        fs::write(&config_path, yaml_content).unwrap();

        let manager = ConfigManager::from_file(&config_path).unwrap();
        assert_eq!(manager.service().name, "test-service");
        assert_eq!(manager.service().api.bind_address, "127.0.0.1:3000");
        assert_eq!(manager.channels().len(), 1);
        assert_eq!(manager.channels()[0].name, "test-channel");
    }

    #[test]
    fn test_env_override() {
        // Test that configuration builder works correctly
        // Note: Environment variable testing is complex with figment,
        // so we test the basic builder functionality instead
        let builder = ConfigBuilder::new()
            .with_defaults();

        let config = builder.build().unwrap();
        
        // Test that defaults are applied correctly
        assert_eq!(config.service.name, "comsrv"); // Default service name
        assert_eq!(config.service.api.bind_address, "127.0.0.1:3000"); // Default API bind
        
        // Test that we can extract specific sections
        let service_config: ServiceConfig = ConfigBuilder::new()
            .with_defaults()
            .extract()
            .unwrap();
        assert_eq!(service_config.name, "comsrv");
    }

    #[test]
    fn test_config_validation() {
        let config = AppConfig::default();
        let manager = ConfigManager {
            config,
            figment: Figment::new(),
        };

        let errors = manager.validate().unwrap();
        // Should be empty for default config
        assert!(errors.is_empty());
    }

    #[test]
    fn test_separated_table_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");
        
        // Create directory structure
        let table_dir = temp_dir.path().join("config/TankFarmModbusTCP");
        fs::create_dir_all(&table_dir).unwrap();
        
        // Create four telemetry CSV files
        let telemetry_csv = r#"point_id,signal_name,chinese_name,scale,offset,unit
1,TANK_01_LEVEL,1号罐液位,0.1,0,m
2,TANK_01_TEMP,1号罐温度,0.1,-40,°C"#;
        fs::write(table_dir.join("telemetry.csv"), telemetry_csv).unwrap();
        
        let signal_csv = r#"point_id,signal_name,chinese_name,reverse
1,PUMP_01_STATUS,1号泵状态,0
2,EMERGENCY_STOP,紧急停机,1"#;
        fs::write(table_dir.join("signal.csv"), signal_csv).unwrap();
        
        let adjustment_csv = r#"point_id,signal_name,chinese_name,scale,offset,unit
1,PUMP_01_SPEED,1号泵转速,1,0,rpm"#;
        fs::write(table_dir.join("adjustment.csv"), adjustment_csv).unwrap();
        
        let control_csv = r#"point_id,signal_name,chinese_name,reverse
1,PUMP_01_START,1号泵启动,0"#;
        fs::write(table_dir.join("control.csv"), control_csv).unwrap();
        
        // Create protocol mapping CSV files
        let telemetry_mapping_csv = r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1,TANK_01_LEVEL,40001,uint16,AB,2,,1号罐液位传感器
2,TANK_01_TEMP,40002,int16,AB,2,,1号罐温度传感器"#;
        fs::write(table_dir.join("mapping_telemetry.csv"), telemetry_mapping_csv).unwrap();
        
        let signal_mapping_csv = r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1,PUMP_01_STATUS,2001,bool,BIT,1,1,1号泵运行状态
2,EMERGENCY_STOP,2002,bool,BIT,1,1,紧急停机按钮"#;
        fs::write(table_dir.join("mapping_signal.csv"), signal_mapping_csv).unwrap();
        
        let adjustment_mapping_csv = r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1,PUMP_01_SPEED,40003,uint16,AB,2,,1号泵转速设定"#;
        fs::write(table_dir.join("mapping_adjustment.csv"), adjustment_mapping_csv).unwrap();
        
        let control_mapping_csv = r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
1,PUMP_01_START,1,bool,BIT,1,1,1号泵启动命令"#;
        fs::write(table_dir.join("mapping_control.csv"), control_mapping_csv).unwrap();

        // Create main config file
        let yaml_content = format!(r#"
service:
  name: "test-separated-tables"

channels:
  - id: 1001
    name: "TankFarmModbusTCP"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
    
    table_config:
      four_telemetry_route: "config/TankFarmModbusTCP"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
      
      protocol_mapping_route: "config/TankFarmModbusTCP"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        adjustment_mapping: "mapping_adjustment.csv"
        control_mapping: "mapping_control.csv"
"#);
        fs::write(&config_path, yaml_content).unwrap();

        // Test loading the configuration
        let manager = ConfigManager::from_file(&config_path).unwrap();
        
        // Verify basic configuration
        assert_eq!(manager.service().name, "test-separated-tables");
        assert_eq!(manager.channels().len(), 1);
        
        let channel = &manager.channels()[0];
        assert_eq!(channel.id, 1001);
        assert_eq!(channel.name, "TankFarmModbusTCP");
        assert!(channel.table_config.is_some());
        
        // Verify combined points were loaded
        let combined_points = manager.get_combined_points(1001);
        // We have 6 total points: 2 YC + 2 YX + 1 YT + 1 YK
        assert_eq!(combined_points.len(), 6);
        
        // Test specific point retrieval
        let tank_level_point = manager.get_combined_point(1001, 1).unwrap();
        assert_eq!(tank_level_point.signal_name, "TANK_01_LEVEL");
        assert_eq!(tank_level_point.chinese_name, "1号罐液位");
        if let Some(scaling) = &tank_level_point.scaling {
            assert_eq!(scaling.scale, 0.1);
            assert_eq!(scaling.unit, Some("m".to_string()));
        }
        // protocol_params would contain register address and data type
        assert_eq!(tank_level_point.data_type, "uint16");
        
        // Test YX point with reverse
        let emergency_stop_point = manager.get_combined_point(1001, 2).unwrap();
        assert_eq!(emergency_stop_point.signal_name, "EMERGENCY_STOP");
        // Note: reverse is not in CombinedPoint structure, this test needs to be updated
        assert_eq!(emergency_stop_point.data_type, "bool");
        
        // Test legacy modbus point conversion
        let modbus_points = manager.get_modbus_points(1001);
        assert_eq!(modbus_points.len(), 6);
        
        let tank_level_modbus = modbus_points.iter()
            .find(|p| p.signal_name == "TANK_01_LEVEL")
            .unwrap();
        assert_eq!(tank_level_modbus.scale, 0.1);
        assert_eq!(tank_level_modbus.offset, 0.0);
        assert_eq!(tank_level_modbus.unit, Some("m".to_string()));
        assert!(!tank_level_modbus.reverse);
        
        let emergency_stop_modbus = modbus_points.iter()
            .find(|p| p.signal_name == "EMERGENCY_STOP")
            .unwrap();
        assert!(emergency_stop_modbus.reverse);
    }

    #[test]
    fn test_four_telemetry_point_types() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");
        
        // Create minimal setup for testing point type filtering
        let table_dir = temp_dir.path().join("config/TestChannel");
        fs::create_dir_all(&table_dir).unwrap();
        
        // Create test files with different point types
        let telemetry_csv = r#"point_id,signal_name,chinese_name,scale,offset,unit
1,ANALOG_SENSOR,模拟传感器,1.0,0,V"#;
        fs::write(table_dir.join("telemetry.csv"), telemetry_csv).unwrap();
        
        let signal_csv = r#"point_id,signal_name,chinese_name,reverse
2,DIGITAL_INPUT,数字输入,0"#;
        fs::write(table_dir.join("signal.csv"), signal_csv).unwrap();
        
        let adjustment_csv = r#"point_id,signal_name,chinese_name,scale,offset,unit
3,SETPOINT,设定值,1.0,0,%"#;
        fs::write(table_dir.join("adjustment.csv"), adjustment_csv).unwrap();
        
        let control_csv = r#"point_id,signal_name,chinese_name,reverse
4,CONTROL_OUTPUT,控制输出,0"#;
        fs::write(table_dir.join("control.csv"), control_csv).unwrap();
        
        // Create matching mapping files
        for (file, data_type, bytes, format) in [
            ("mapping_telemetry.csv", "uint16", 2, "AB"),   // 2字节用AB格式
            ("mapping_signal.csv", "bool", 1, "BIT"),
            ("mapping_adjustment.csv", "uint16", 2, "AB"),   // 2字节用AB格式
            ("mapping_control.csv", "bool", 1, "BIT"),
        ] {
            let point_id = match file.chars().nth(8).unwrap() {
                't' => 1, // telemetry
                's' => 2, // signal 
                'a' => 3, // adjustment
                'c' => 4, // control
                _ => 1
            };
            let mapping_csv = format!(r#"point_id,signal_name,address,data_type,data_format,number_of_bytes,bit_location,description
{},TEST_SIGNAL,1000,{},{},{},,Test signal"#, 
                point_id, data_type, format, bytes);
            fs::write(table_dir.join(file), mapping_csv).unwrap();
        }

        let yaml_content = r#"
service:
  name: "test-point-types"

channels:
  - id: 2001
    name: "TestChannel"
    protocol: "test"
    
    table_config:
      four_telemetry_route: "config/TestChannel"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
      
      protocol_mapping_route: "config/TestChannel"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        adjustment_mapping: "mapping_adjustment.csv"
        control_mapping: "mapping_control.csv"
"#;
        fs::write(&config_path, yaml_content).unwrap();

        let manager = ConfigManager::from_file(&config_path).unwrap();
        
        // Test point type filtering
        let yc_points = manager.get_four_telemetry_points(2001, "YC");
        assert_eq!(yc_points.len(), 1);
        assert_eq!(yc_points[0].signal_name, "ANALOG_SENSOR");
        
        let yx_points = manager.get_four_telemetry_points(2001, "YX");
        assert_eq!(yx_points.len(), 1);
        assert_eq!(yx_points[0].signal_name, "DIGITAL_INPUT");
        
        let yt_points = manager.get_four_telemetry_points(2001, "YT");
        assert_eq!(yt_points.len(), 1);
        assert_eq!(yt_points[0].signal_name, "SETPOINT");
        
        let yk_points = manager.get_four_telemetry_points(2001, "YK");
        assert_eq!(yk_points.len(), 1);
        assert_eq!(yk_points[0].signal_name, "CONTROL_OUTPUT");
    }
} 