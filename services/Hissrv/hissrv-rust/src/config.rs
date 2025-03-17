use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Duration;
use regex::Regex;

use crate::error::{HissrvError, Result};

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis server hostname
    #[serde(alias = "hostname")]
    pub host: String,
    /// Redis server port
    pub port: u16,
    /// Redis password
    pub password: Option<String>,
    /// Redis database
    #[serde(alias = "database")]
    pub db: u8,
    /// Connection timeout in seconds
    #[serde(alias = "connection_timeout")]
    pub timeout_seconds: u32,
    /// Redis key pattern
    pub key_pattern: String,
    /// Polling interval in seconds
    #[serde(alias = "polling_interval")]
    pub polling_interval_seconds: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            db: 0,
            timeout_seconds: 5,
            key_pattern: "*".to_string(),
            polling_interval_seconds: 10,
        }
    }
}

impl RedisConfig {
    /// Get polling interval as Duration
    pub fn get_polling_interval(&self) -> Duration {
        Duration::from_secs(self.polling_interval_seconds)
    }
}

/// InfluxDB configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxDBConfig {
    /// InfluxDB server URL
    pub url: String,
    /// InfluxDB organization
    pub org: String,
    /// InfluxDB token
    pub token: String,
    /// InfluxDB bucket
    pub bucket: String,
    /// Batch size
    pub batch_size: usize,
    /// Flush interval in seconds
    #[serde(alias = "flush_interval")]
    pub flush_interval_seconds: u64,
}

impl Default for InfluxDBConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8086".to_string(),
            org: "voltage".to_string(),
            token: "".to_string(),
            bucket: "history".to_string(),
            batch_size: 1000,
            flush_interval_seconds: 30,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    /// Log file path
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
        }
    }
}

/// Tag mapping from Redis to InfluxDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagMapping {
    /// Redis source (key or field)
    pub redis_source: String,
    /// InfluxDB tag name
    #[serde(alias = "influx_tag")]
    pub influxdb_tag_name: String,
    /// Whether to extract from key
    #[serde(default)]
    pub extract_from_key: bool,
    /// Extraction pattern for key
    pub extraction_pattern: Option<String>,
    /// Value to use for tag
    #[serde(default = "default_tag_value")]
    pub value: String,
    /// Redis key pattern for matching
    #[serde(default = "default_pattern", skip_serializing_if = "is_default_pattern")]
    pub redis_key_pattern: Regex,
}

fn default_tag_value() -> String {
    "".to_string()
}

fn default_pattern() -> Regex {
    Regex::new(".*").unwrap()
}

fn is_default_pattern(pattern: &Regex) -> bool {
    pattern.as_str() == ".*"
}

/// Field mapping from Redis to InfluxDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    /// Redis source (key or field)
    pub redis_source: String,
    /// InfluxDB field name
    #[serde(alias = "influx_field")]
    pub influxdb_field_name: String,
    /// Data type
    pub data_type: String,
    /// Scale factor
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f64,
    /// Measurement name
    #[serde(alias = "measurement")]
    pub influxdb_measurement: Option<String>,
    /// Redis key pattern for matching
    #[serde(default = "default_pattern", skip_serializing_if = "is_default_pattern")]
    pub redis_key_pattern: Regex,
}

fn default_scale_factor() -> f64 {
    1.0
}

/// Data mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMappingConfig {
    /// Default measurement name
    pub default_measurement: String,
    /// Tag mappings
    #[serde(default)]
    pub tag_mappings: Vec<TagMapping>,
    /// Field mappings
    pub field_mappings: Vec<FieldMapping>,
}

impl Default for DataMappingConfig {
    fn default() -> Self {
        Self {
            default_measurement: "ems_data".to_string(),
            tag_mappings: Vec::new(),
            field_mappings: Vec::new(),
        }
    }
}

/// Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Redis configuration
    #[serde(default)]
    pub redis: RedisConfig,
    /// InfluxDB configuration
    #[serde(default)]
    pub influxdb: InfluxDBConfig,
    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
    /// Data mapping configuration
    #[serde(default)]
    pub data_mapping: DataMappingConfig,
}

impl Config {
    /// Load configuration from file
    pub fn from_file(path: &Path) -> Result<Self> {
        let mut file = File::open(path)
            .map_err(|e| HissrvError::IOError(e.to_string()))?;
        
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| HissrvError::IOError(e.to_string()))?;
        
        // Parse config file
        let mut config: Config = serde_yaml::from_str(&content)
            .map_err(|e| HissrvError::YamlError(e.to_string()))?;
        
        // Compile regex patterns for mappings
        for tag_mapping in &mut config.data_mapping.tag_mappings {
            if let Some(ref pattern) = tag_mapping.extraction_pattern {
                tag_mapping.redis_key_pattern = Regex::new(pattern)
                    .map_err(|e| HissrvError::ConfigError(format!("Invalid regex pattern: {}", e)))?;
            } else {
                tag_mapping.redis_key_pattern = Regex::new(&format!(".*{}.*", regex::escape(&tag_mapping.redis_source)))
                    .map_err(|e| HissrvError::ConfigError(format!("Failed to create regex: {}", e)))?;
            }
            
            // If value is empty, use redis_source as value
            if tag_mapping.value.is_empty() {
                tag_mapping.value = tag_mapping.redis_source.clone();
            }
        }
        
        for field_mapping in &mut config.data_mapping.field_mappings {
            field_mapping.redis_key_pattern = Regex::new(&format!(".*{}.*", regex::escape(&field_mapping.redis_source)))
                .map_err(|e| HissrvError::ConfigError(format!("Failed to create regex: {}", e)))?;
        }
        
        Ok(config)
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check Redis configuration
        if self.redis.host.is_empty() {
            return Err(HissrvError::ConfigError("Redis host is empty".into()));
        }
        
        // Check InfluxDB configuration
        if self.influxdb.url.is_empty() {
            return Err(HissrvError::ConfigError("InfluxDB URL is empty".into()));
        }
        
        if self.influxdb.org.is_empty() {
            return Err(HissrvError::ConfigError("InfluxDB organization is empty".into()));
        }
        
        if self.influxdb.token.is_empty() {
            return Err(HissrvError::ConfigError("InfluxDB token is empty".into()));
        }
        
        if self.influxdb.bucket.is_empty() {
            return Err(HissrvError::ConfigError("InfluxDB bucket is empty".into()));
        }
        
        // Check data mapping configuration
        if self.data_mapping.default_measurement.is_empty() {
            return Err(HissrvError::ConfigError("Default measurement is empty".into()));
        }
        
        if self.data_mapping.field_mappings.is_empty() {
            return Err(HissrvError::ConfigError("No field mappings defined".into()));
        }
        
        for field_mapping in &self.data_mapping.field_mappings {
            if field_mapping.redis_source.is_empty() {
                return Err(HissrvError::ConfigError("Redis source is empty for field mapping".into()));
            }
            
            if field_mapping.influxdb_field_name.is_empty() {
                return Err(HissrvError::ConfigError("InfluxDB field is empty for field mapping".into()));
            }
            
            if field_mapping.data_type.is_empty() {
                return Err(HissrvError::ConfigError("Data type is empty for field mapping".into()));
            }
            
            match field_mapping.data_type.as_str() {
                "string" | "float" | "integer" | "boolean" => {},
                _ => {
                    return Err(HissrvError::ConfigError(
                        format!("Unsupported data type: {}", field_mapping.data_type)
                    ));
                }
            }
            
            if field_mapping.scale_factor <= 0.0 {
                return Err(HissrvError::ConfigError("Scale factor must be positive".into()));
            }
        }
        
        Ok(())
    }
} 