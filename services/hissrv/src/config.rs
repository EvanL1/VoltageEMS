//! hissrv configuration module - minimal configuration system
//! Supports flexible data source configuration and mapping rules

use crate::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;
use voltage_libs::config::utils::{
    get_global_influxdb_bucket, get_global_influxdb_org, get_global_influxdb_token,
    get_global_influxdb_url, get_global_redis_url,
};
use voltage_libs::config::ConfigLoader;

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    #[serde(default = "default_service_name")]
    pub name: String,
    #[serde(with = "humantime_serde", default = "default_polling_interval")]
    pub polling_interval: Duration,
    #[serde(default)]
    pub enable_api: bool,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_service_name() -> String {
    "hissrv".to_string()
}

fn default_polling_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_port() -> u16 {
    6004 // Fixed port for hissrv
}

/// Redis data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisDataKey {
    pub pattern: String,
    #[serde(rename = "type")]
    pub data_type: String, // "list" or "hash"
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
    #[serde(default)]
    pub data_keys: Vec<RedisDataKey>,
}

fn default_redis_url() -> String {
    get_global_redis_url("HISSRV")
}

/// InfluxDB configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxConfig {
    #[serde(default = "default_influx_url")]
    pub url: String,
    #[serde(default = "default_influx_org")]
    pub org: String,
    #[serde(default = "default_influx_bucket")]
    pub bucket: String,
    #[serde(default = "default_influx_token")]
    pub token: String,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(with = "humantime_serde", default = "default_write_timeout")]
    pub write_timeout: Duration,
}

fn default_influx_url() -> String {
    get_global_influxdb_url("HISSRV")
}

fn default_influx_org() -> String {
    get_global_influxdb_org("HISSRV")
}

fn default_influx_bucket() -> String {
    get_global_influxdb_bucket("HISSRV")
}

fn default_influx_token() -> String {
    get_global_influxdb_token("HISSRV")
}

fn default_batch_size() -> usize {
    1000
}

fn default_write_timeout() -> Duration {
    Duration::from_secs(30)
}

/// Tag extraction rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TagRule {
    #[serde(rename = "extract")]
    Extract { field: String },
    #[serde(rename = "static")]
    Static { value: String },
}

/// Field mapping rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    pub name: String,
    pub field_type: String, // "float", "int", "bool", "string"
}

/// Data mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMapping {
    pub source: String,
    pub measurement: String,
    pub tags: Vec<TagRule>,
    pub fields: Vec<FieldMapping>,
}

/// Complete configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub service: ServiceConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub influxdb: InfluxConfig,
    #[serde(default)]
    pub mappings: Vec<DataMapping>,
}

fn default_version() -> String {
    "0.0.1".to_string()
}

// Default implementation
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "hissrv".to_string(),
            polling_interval: Duration::from_secs(10),
            enable_api: false,
            port: 6004,
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: get_global_redis_url("HISSRV"),
            data_keys: vec![],
        }
    }
}

impl Default for InfluxConfig {
    fn default() -> Self {
        Self {
            url: get_global_influxdb_url("HISSRV"),
            org: get_global_influxdb_org("HISSRV"),
            bucket: get_global_influxdb_bucket("HISSRV"),
            token: get_global_influxdb_token("HISSRV"),
            batch_size: 1000,
            write_timeout: Duration::from_secs(30),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            service: ServiceConfig::default(),
            redis: RedisConfig::default(),
            influxdb: InfluxConfig::default(),
            mappings: vec![],
        }
    }
}

impl Config {
    /// Load configuration file
    pub fn load() -> Result<Self> {
        // Try multiple configuration file paths
        let config_paths = [
            "config/hissrv/hissrv.yaml",
            "config/hissrv.yaml",
            "hissrv.yaml",
        ];

        let mut yaml_path = None;
        for path in &config_paths {
            if Path::new(path).exists() {
                yaml_path = Some(path.to_string());
                break;
            }
        }

        // Use new ConfigLoader
        let loader = ConfigLoader::new()
            .with_defaults(Config::default())
            .with_env_prefix("HISSRV");

        let config = if let Some(path) = yaml_path {
            loader.with_yaml_file(&path).build()
        } else {
            loader.build()
        }
        .map_err(|e| hissrv::anyhow!("Failed to load configuration: {}", e))?;

        // Force hardcoded port
        config.service.port = 6004;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Find mapping rules by source pattern
    pub fn find_mapping(&self, source: &str) -> Option<&DataMapping> {
        self.mappings
            .iter()
            .find(|m| source.starts_with(&m.source.replace("*", "")))
    }

    /// Find mapping rules by source pattern (mutable reference)
    #[allow(dead_code)]
    pub fn find_mapping_mut(&mut self, source: &str) -> Option<&mut DataMapping> {
        self.mappings
            .iter_mut()
            .find(|m| source.starts_with(&m.source.replace("*", "")))
    }

    /// Add new mapping rules
    pub fn add_mapping(&mut self, mapping: DataMapping) -> Result<()> {
        // Check if mapping with same source pattern already exists
        if self.find_mapping(&mapping.source).is_some() {
            return Err(hissrv::anyhow!(
                "Mapping for source '{}' already exists",
                mapping.source
            ));
        }
        self.mappings.push(mapping);
        Ok(())
    }

    /// Update existing mapping rules
    pub fn update_mapping(&mut self, source: &str, new_mapping: DataMapping) -> Result<()> {
        if let Some(pos) = self.mappings.iter().position(|m| m.source == source) {
            self.mappings[pos] = new_mapping;
            Ok(())
        } else {
            Err(hissrv::anyhow!("Mapping for source '{}' not found", source))
        }
    }

    /// Remove mapping rules
    pub fn remove_mapping(&mut self, source: &str) -> Result<()> {
        if let Some(pos) = self.mappings.iter().position(|m| m.source == source) {
            self.mappings.remove(pos);
            Ok(())
        } else {
            Err(hissrv::anyhow!("Mapping for source '{}' not found", source))
        }
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        // Backup original file
        let path = path.as_ref();
        if path.exists() {
            let backup_path = path.with_extension("yaml.bak");
            fs::copy(path, backup_path)?;
        }

        // Write new configuration
        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)?;
        Ok(())
    }

    /// Reload configuration
    pub fn reload() -> Result<Self> {
        Self::load()
    }

    /// Validate configuration integrity
    pub fn validate(&self) -> Result<()> {
        // Validate service configuration
        if self.service.name.is_empty() {
            return Err(hissrv::anyhow!("Service name cannot be empty"));
        }

        // Validate Redis configuration
        if self.redis.url.is_empty() {
            return Err(hissrv::anyhow!("Redis URL cannot be empty"));
        }

        if self.redis.data_keys.is_empty() {
            return Err(hissrv::anyhow!("At least one data key must be configured"));
        }

        // Validate InfluxDB configuration
        if self.influxdb.url.is_empty() {
            return Err(hissrv::anyhow!("InfluxDB URL cannot be empty"));
        }

        if self.influxdb.org.is_empty() {
            return Err(hissrv::anyhow!("InfluxDB organization cannot be empty"));
        }

        if self.influxdb.bucket.is_empty() {
            return Err(hissrv::anyhow!("InfluxDB bucket cannot be empty"));
        }

        if self.influxdb.token.is_empty() {
            return Err(hissrv::anyhow!("InfluxDB token cannot be empty"));
        }

        // Validate mapping rules
        for mapping in &self.mappings {
            if mapping.source.is_empty() {
                return Err(hissrv::anyhow!("Mapping source cannot be empty"));
            }

            if mapping.measurement.is_empty() {
                return Err(hissrv::anyhow!("Mapping measurement cannot be empty"));
            }

            if mapping.fields.is_empty() {
                return Err(hissrv::anyhow!(
                    "At least one field must be defined for mapping '{}'",
                    mapping.source
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> Config {
        Config {
            version: "0.0.1".to_string(),
            service: ServiceConfig {
                name: "hissrv".to_string(),
                polling_interval: Duration::from_secs(10),
                enable_api: false,
                port: 6004,
            },
            redis: RedisConfig {
                url: "redis://localhost".to_string(),
                data_keys: vec![RedisDataKey {
                    pattern: "archive:pending".to_string(),
                    data_type: "list".to_string(),
                }],
            },
            influxdb: InfluxConfig {
                url: "http://localhost:8086".to_string(),
                org: "test".to_string(),
                bucket: "test".to_string(),
                token: "test".to_string(),
                batch_size: 1000,
                write_timeout: Duration::from_secs(30),
            },
            mappings: vec![DataMapping {
                source: "archive:1m:*".to_string(),
                measurement: "metrics_1m".to_string(),
                tags: vec![],
                fields: vec![FieldMapping {
                    name: "value".to_string(),
                    field_type: "float".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn test_find_mapping() {
        let config = create_test_config();
        assert!(config.find_mapping("archive:1m:test").is_some());
        assert!(config.find_mapping("other:data").is_none());
    }

    #[test]
    fn test_add_mapping() {
        let mut config = create_test_config();
        let new_mapping = DataMapping {
            source: "archive:5m:*".to_string(),
            measurement: "metrics_5m".to_string(),
            tags: vec![],
            fields: vec![],
        };

        assert!(config.add_mapping(new_mapping.clone()).is_ok());
        assert!(config.find_mapping("archive:5m:test").is_some());

        // Test duplicate addition
        assert!(config.add_mapping(new_mapping).is_err());
    }

    #[test]
    fn test_update_mapping() {
        let mut config = create_test_config();
        let updated_mapping = DataMapping {
            source: "archive:1m:*".to_string(),
            measurement: "metrics_1m_updated".to_string(),
            tags: vec![],
            fields: vec![],
        };

        assert!(config
            .update_mapping("archive:1m:*", updated_mapping)
            .is_ok());
        assert_eq!(config.mappings[0].measurement, "metrics_1m_updated");

        // Test updating non-existent mapping
        assert!(config
            .update_mapping(
                "non_existent",
                DataMapping {
                    source: "test".to_string(),
                    measurement: "test".to_string(),
                    tags: vec![],
                    fields: vec![],
                }
            )
            .is_err());
    }

    #[test]
    fn test_remove_mapping() {
        let mut config = create_test_config();
        assert!(config.remove_mapping("archive:1m:*").is_ok());
        assert!(config.mappings.is_empty());

        // Test removing non-existent mapping
        assert!(config.remove_mapping("non_existent").is_err());
    }

    #[test]
    fn test_validate_config() {
        let mut config = create_test_config();
        assert!(config.validate().is_ok());

        // Test empty service name
        config.service.name.clear();
        assert!(config.validate().is_err());
    }
}
