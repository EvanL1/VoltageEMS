//! Common configuration utilities for VoltageEMS services

use crate::{Error, Result};
use figment::{
    providers::{Env, Format, Json, Toml, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Base service configuration that all services should include
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    /// Service name
    pub name: String,
    /// Service version
    pub version: String,
    /// Service host
    pub host: String,
    /// Service port
    pub port: u16,
    /// Environment (development, staging, production)
    pub environment: Environment,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            environment: Environment::Development,
        }
    }
}

/// Load configuration from multiple sources
///
/// Priority (highest to lowest):
/// 1. Environment variables (prefixed)
/// 2. Local config file (e.g., config.local.yaml)
/// 3. Environment-specific file (e.g., config.production.yaml)
/// 4. Default config file (e.g., config.yaml)
/// 5. Default values
pub fn load_config<T>(service_name: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    let env = std::env::var("VOLTAGE_ENV").unwrap_or_else(|_| "development".to_string());

    let figment = Figment::new()
        // Start with defaults
        .merge(Toml::file("config/default.toml"))
        .merge(Yaml::file("config/default.yaml"))
        .merge(Json::file("config/default.json"))
        // Environment-specific config
        .merge(Toml::file(format!("config/{}.toml", env)))
        .merge(Yaml::file(format!("config/{}.yaml", env)))
        .merge(Json::file(format!("config/{}.json", env)))
        // Local overrides (not committed to git)
        .merge(Toml::file("config/local.toml"))
        .merge(Yaml::file("config/local.yaml"))
        .merge(Json::file("config/local.json"))
        // Service-specific config
        .merge(Toml::file(format!("config/{}.toml", service_name)))
        .merge(Yaml::file(format!("config/{}.yaml", service_name)))
        // Environment variables with prefix
        .merge(Env::prefixed(&format!("{}_", service_name.to_uppercase())));

    figment
        .extract()
        .map_err(|e| Error::Config(format!("Failed to load configuration: {}", e)))
}

/// Load configuration from a specific file
pub fn load_config_from_file<T, P>(path: P) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Config("Config file must have an extension".to_string()))?;

    let figment = match extension {
        "toml" => Figment::new().merge(Toml::file(path)),
        "yaml" | "yml" => Figment::new().merge(Yaml::file(path)),
        "json" => Figment::new().merge(Json::file(path)),
        _ => {
            return Err(Error::Config(format!(
                "Unsupported config file format: {}",
                extension
            )))
        }
    };

    figment
        .extract()
        .map_err(|e| Error::Config(format!("Failed to load configuration from file: {}", e)))
}

/// Save configuration to a file
pub fn save_config_to_file<T, P>(config: &T, path: P) -> Result<()>
where
    T: Serialize,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Config("Config file must have an extension".to_string()))?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = match extension {
        "toml" => {
            toml::to_string_pretty(config).map_err(|e| Error::Serialization(e.to_string()))?
        }
        "yaml" | "yml" => {
            serde_yaml::to_string(config).map_err(|e| Error::Serialization(e.to_string()))?
        }
        "json" => {
            serde_json::to_string_pretty(config).map_err(|e| Error::Serialization(e.to_string()))?
        }
        _ => {
            return Err(Error::Config(format!(
                "Unsupported config file format: {}",
                extension
            )))
        }
    };

    std::fs::write(path, content)?;
    Ok(())
}

/// Merge two configurations, with the second taking precedence
pub fn merge_configs<T>(base: T, overlay: T) -> Result<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let base_value = serde_json::to_value(base)?;
    let overlay_value = serde_json::to_value(overlay)?;

    let merged = merge_json_values(base_value, overlay_value);

    serde_json::from_value(merged)
        .map_err(|e| Error::Serialization(format!("Failed to merge configurations: {}", e)))
}

fn merge_json_values(base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (k, v) in overlay_map {
                match base_map.get(&k) {
                    Some(base_v) if base_v.is_object() && v.is_object() => {
                        base_map.insert(k, merge_json_values(base_v.clone(), v));
                    }
                    _ => {
                        base_map.insert(k, v);
                    }
                }
            }
            Value::Object(base_map)
        }
        (_, overlay) => overlay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestConfig {
        name: String,
        port: u16,
        nested: NestedConfig,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct NestedConfig {
        enabled: bool,
        value: i32,
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.yaml");

        let config = TestConfig {
            name: "test".to_string(),
            port: 8080,
            nested: NestedConfig {
                enabled: true,
                value: 42,
            },
        };

        save_config_to_file(&config, &config_path).unwrap();
        let loaded: TestConfig = load_config_from_file(&config_path).unwrap();

        assert_eq!(config, loaded);
    }

    #[test]
    fn test_merge_configs() {
        let base = TestConfig {
            name: "base".to_string(),
            port: 8080,
            nested: NestedConfig {
                enabled: false,
                value: 10,
            },
        };

        let overlay = TestConfig {
            name: "overlay".to_string(),
            port: 9090,
            nested: NestedConfig {
                enabled: true,
                value: 20,
            },
        };

        let merged = merge_configs(base, overlay).unwrap();
        assert_eq!(merged.name, "overlay");
        assert_eq!(merged.port, 9090);
        assert!(merged.nested.enabled);
        assert_eq!(merged.nested.value, 20);
    }
}
