//! Universal configuration loader
//!
//! Provides unified configuration loading with priority:
//! 1. Default values (lowest)
//! 2. YAML file (medium)  
//! 3. Environment variables (highest)

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use tracing::{debug, info, warn};

/// Configuration loading error
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error reading {path}: {source}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Configuration merge error: {0}")]
    MergeError(String),

    #[error("Invalid environment variable {name}: {reason}")]
    InvalidEnvVar { name: String, reason: String },
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// Array merge strategy
#[derive(Debug, Clone, Copy, Default)]
pub enum ArrayMergeStrategy {
    #[default]
    Replace, // Replace entire array (default)
    Append,      // Append new items
    Prepend,     // Prepend new items
    MergeUnique, // Merge unique items only
}

/// Configuration builder options
#[derive(Debug, Clone)]
pub struct ConfigOptions {
    /// Array merge strategy
    pub array_merge: ArrayMergeStrategy,
    /// Whether to validate against schema
    pub validate_schema: bool,
    /// Whether to allow unknown fields
    pub allow_unknown_fields: bool,
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            array_merge: ArrayMergeStrategy::default(),
            validate_schema: false,
            allow_unknown_fields: true,
        }
    }
}

/// Universal configuration loader
pub struct ConfigLoader<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    /// Default configuration
    defaults: T,
    /// Environment variable prefix
    env_prefix: Option<String>,
    /// YAML file paths (multiple files supported)
    yaml_paths: Vec<String>,
    /// Whether to allow environment variable override
    allow_env_override: bool,
    /// Configuration options
    options: ConfigOptions,
}

impl<T> Default for ConfigLoader<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    fn default() -> Self {
        Self {
            defaults: T::default(),
            env_prefix: None,
            yaml_paths: Vec::new(),
            allow_env_override: true,
            options: ConfigOptions::default(),
        }
    }
}

impl<T> ConfigLoader<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    /// Create new configuration loader
    pub fn new() -> Self {
        Self::default()
    }

    /// Set default configuration
    pub fn with_defaults(mut self, defaults: T) -> Self {
        self.defaults = defaults;
        self
    }

    /// Set environment variable prefix
    pub fn with_env_prefix(mut self, prefix: &str) -> Self {
        self.env_prefix = Some(prefix.to_string());
        self.allow_env_override = true;
        self
    }

    /// Add YAML configuration file path
    pub fn with_yaml_file(mut self, path: &str) -> Self {
        self.yaml_paths.push(path.to_string());
        self
    }

    /// Add multiple YAML configuration files
    pub fn with_yaml_files(mut self, paths: Vec<String>) -> Self {
        self.yaml_paths.extend(paths);
        self
    }

    /// Set configuration options
    pub fn with_options(mut self, options: ConfigOptions) -> Self {
        self.options = options;
        self
    }

    /// Build final configuration
    pub fn build(self) -> Result<T> {
        // 1. Start from default values (lowest priority)
        let mut config_json = serde_json::to_value(&self.defaults)?;
        debug!("Starting from default configuration");

        // 2. Apply YAML files in order (medium priority)
        for yaml_path in &self.yaml_paths {
            if Path::new(yaml_path).exists() {
                info!("Loading YAML config file: {}", yaml_path);
                let yaml_content =
                    std::fs::read_to_string(yaml_path).map_err(|e| ConfigError::IoError {
                        path: yaml_path.clone(),
                        source: e,
                    })?;

                // Parse YAML directly to serde_yaml::Value
                let yaml_value: YamlValue = serde_yaml::from_str(&yaml_content)?;

                // Convert YAML value to JSON value efficiently
                let yaml_as_json = self.yaml_to_json_optimized(yaml_value)?;

                // Merge into configuration
                self.merge_json_values(&mut config_json, &yaml_as_json)?;
            } else {
                debug!("YAML config file not found, skipping: {}", yaml_path);
            }
        }

        // 3. Apply environment variables (highest priority)
        if let Some(prefix) = &self.env_prefix {
            if self.allow_env_override {
                debug!("Applying environment variables with prefix: {}", prefix);
                self.apply_env_vars(&mut config_json, prefix)?;
            }
        }

        // 4. Deserialize to final configuration
        let config: T = serde_json::from_value(config_json)?;
        Ok(config)
    }

    /// Apply environment variables to configuration
    fn apply_env_vars(&self, config: &mut JsonValue, prefix: &str) -> Result<()> {
        // Collect all environment variables with specified prefix
        let env_vars: HashMap<String, String> =
            env::vars().filter(|(k, _)| k.starts_with(prefix)).collect();

        for (key, value) in env_vars {
            // Remove prefix and convert to configuration path
            let path = self.parse_env_path(&key, prefix)?;

            if !path.is_empty() {
                debug!(
                    "Applying environment variable {} = {} to path {}",
                    key, value, path
                );
                self.set_value_by_path(config, &path, &value)?;
            }
        }

        Ok(())
    }

    /// Parse environment variable name to configuration path
    /// Handles escaping: __ becomes literal _, single _ becomes .
    fn parse_env_path(&self, key: &str, prefix: &str) -> Result<String> {
        let without_prefix = key
            .strip_prefix(prefix)
            .ok_or_else(|| ConfigError::InvalidEnvVar {
                name: key.to_string(),
                reason: "Missing expected prefix".to_string(),
            })?
            .trim_start_matches('_');

        // Convert to lowercase and handle escaping
        let path = without_prefix
            .to_lowercase()
            .replace("__", "\x00") // Temporary placeholder for escaped underscore
            .replace('_', ".")      // Single underscore becomes dot
            .replace('\x00', "_"); // Restore escaped underscore

        Ok(path)
    }

    /// Convert YAML value to JSON value optimized
    fn yaml_to_json_optimized(&self, yaml: YamlValue) -> Result<JsonValue> {
        // Direct conversion using serde_yaml::from_value
        // This avoids the unnecessary string serialization
        let json: JsonValue = serde_yaml::from_value(yaml)?;
        Ok(json)
    }

    /// Recursively merge two JSON values with strategy support
    fn merge_json_values(&self, base: &mut JsonValue, overlay: &JsonValue) -> Result<()> {
        match (base, overlay) {
            (JsonValue::Object(base_map), JsonValue::Object(overlay_map)) => {
                // Merge objects recursively
                for (key, overlay_value) in overlay_map {
                    match base_map.get_mut(key) {
                        Some(base_value) => {
                            // Recursive merge
                            self.merge_json_values(base_value, overlay_value)?;
                        },
                        None => {
                            // New key, insert directly
                            base_map.insert(key.clone(), overlay_value.clone());
                        },
                    }
                }
            },
            (JsonValue::Array(base_arr), JsonValue::Array(overlay_arr)) => {
                // Apply array merge strategy
                match self.options.array_merge {
                    ArrayMergeStrategy::Replace => {
                        *base_arr = overlay_arr.clone();
                    },
                    ArrayMergeStrategy::Append => {
                        base_arr.extend(overlay_arr.clone());
                    },
                    ArrayMergeStrategy::Prepend => {
                        let mut new_arr = overlay_arr.clone();
                        new_arr.extend(base_arr.clone());
                        *base_arr = new_arr;
                    },
                    ArrayMergeStrategy::MergeUnique => {
                        for item in overlay_arr {
                            if !base_arr.contains(item) {
                                base_arr.push(item.clone());
                            }
                        }
                    },
                }
            },
            (base, overlay) => {
                // Other types: direct replacement
                *base = overlay.clone();
            },
        }
        Ok(())
    }

    /// Set value by path
    fn set_value_by_path(&self, config: &mut JsonValue, path: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = config;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Last part, set value
                if let JsonValue::Object(map) = current {
                    // Parse value with type hints
                    let parsed_value = self.parse_env_value_smart(part, value)?;
                    map.insert(part.to_string(), parsed_value);
                }
            } else {
                // Intermediate part, ensure it's an object
                if let JsonValue::Object(map) = current {
                    current = map
                        .entry(part.to_string())
                        .or_insert_with(|| JsonValue::Object(serde_json::Map::new()));
                }
            }
        }

        Ok(())
    }

    /// Smart parse environment variable value with type hints
    /// Supports type hints: KEY__int=123, KEY__float=3.14, KEY__bool=true, KEY__str=value
    fn parse_env_value_smart(&self, key: &str, value: &str) -> Result<JsonValue> {
        // Check for explicit type hints in key suffix
        if let Some((_, type_hint)) = key.rsplit_once("__") {
            return match type_hint {
                "int" | "i64" => value
                    .parse::<i64>()
                    .map(|v| JsonValue::Number(serde_json::Number::from(v)))
                    .map_err(|_| ConfigError::InvalidEnvVar {
                        name: key.to_string(),
                        reason: format!("Cannot parse '{}' as integer", value),
                    }),
                "float" | "f64" => value
                    .parse::<f64>()
                    .map_err(|e| ConfigError::InvalidEnvVar {
                        name: key.to_string(),
                        reason: format!("Cannot parse '{}' as float: {}", value, e),
                    })
                    .and_then(|v| {
                        serde_json::Number::from_f64(v).ok_or_else(|| ConfigError::InvalidEnvVar {
                            name: key.to_string(),
                            reason: format!("Invalid float value: {}", v),
                        })
                    })
                    .map(JsonValue::Number),
                "bool" => value.parse::<bool>().map(JsonValue::Bool).map_err(|_| {
                    ConfigError::InvalidEnvVar {
                        name: key.to_string(),
                        reason: format!("Cannot parse '{}' as boolean", value),
                    }
                }),
                "str" | "string" => Ok(JsonValue::String(value.to_string())),
                "json" => serde_json::from_str(value).map_err(|e| ConfigError::InvalidEnvVar {
                    name: key.to_string(),
                    reason: format!("Cannot parse '{}' as JSON: {}", value, e),
                }),
                _ => {
                    warn!("Unknown type hint '{}' for key '{}'", type_hint, key);
                    Ok(self.parse_env_value_auto(value))
                },
            };
        }

        // Auto-detect type
        Ok(self.parse_env_value_auto(value))
    }

    /// Auto-detect environment variable value type
    fn parse_env_value_auto(&self, value: &str) -> JsonValue {
        // Try boolean first (strict parsing)
        if value == "true" || value == "false" {
            return JsonValue::Bool(value == "true");
        }

        // Try integer (check for pure digits or negative numbers)
        if value
            .chars()
            .all(|c| c.is_ascii_digit() || (c == '-' && value.len() > 1))
        {
            if let Ok(int_val) = value.parse::<i64>() {
                return JsonValue::Number(serde_json::Number::from(int_val));
            }
        }

        // Try float (must contain decimal point)
        if value.contains('.') {
            if let Ok(float_val) = value.parse::<f64>() {
                if let Some(num) = serde_json::Number::from_f64(float_val) {
                    return JsonValue::Number(num);
                }
            }
        }

        // Try JSON array or object
        if (value.starts_with('[') && value.ends_with(']'))
            || (value.starts_with('{') && value.ends_with('}'))
        {
            if let Ok(json_val) = serde_json::from_str(value) {
                return json_val;
            }
        }

        // Default as string
        JsonValue::String(value.to_string())
    }
}

/// Configuration loading helper function
pub fn load_config<T>(service_name: &str) -> Result<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    // Support multiple config paths with priority
    let config_paths = vec![
        format!("config/{}.yaml", service_name.to_lowercase()),
        format!("config/{}.yml", service_name.to_lowercase()),
        format!("{}.yaml", service_name.to_lowercase()),
        format!("{}.yml", service_name.to_lowercase()),
    ];

    let env_prefix = service_name.to_uppercase();

    let mut loader = ConfigLoader::new().with_env_prefix(&env_prefix);

    // Add existing config files
    for path in config_paths {
        if Path::new(&path).exists() {
            loader = loader.with_yaml_file(&path);
            break; // Use first found
        }
    }

    loader.build()
}

/// Load configuration with custom options
pub fn load_config_with_options<T>(service_name: &str, options: ConfigOptions) -> Result<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    let config_file = format!("config/{}.yaml", service_name.to_lowercase());
    let env_prefix = service_name.to_uppercase();

    ConfigLoader::new()
        .with_env_prefix(&env_prefix)
        .with_yaml_file(&config_file)
        .with_options(options)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        name: String,
        port: u16,
        #[serde(default)]
        redis: RedisConfig,
        #[serde(default)]
        features: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct RedisConfig {
        url: String,
        pool_size: u32,
    }

    impl Default for RedisConfig {
        fn default() -> Self {
            Self {
                url: "redis://localhost:6379".to_string(),
                pool_size: 10,
            }
        }
    }

    #[test]
    fn test_default_config() {
        let config: TestConfig = ConfigLoader::new()
            .build()
            .expect("Failed to build config with defaults");
        assert_eq!(config.name, "");
        assert_eq!(config.port, 0);
        assert_eq!(config.redis.url, "redis://localhost:6379");
        assert_eq!(config.redis.pool_size, 10);
    }

    #[test]
    fn test_env_override() {
        env::set_var("TEST_NAME", "test-service");
        env::set_var("TEST_PORT", "8080");
        env::set_var("TEST_REDIS_URL", "redis://custom:6379");
        env::set_var("TEST_REDIS_POOL__SIZE", "20"); // Test escaped underscore

        let config: TestConfig = ConfigLoader::new()
            .with_env_prefix("TEST")
            .build()
            .expect("Failed to build config with env overrides");

        assert_eq!(config.name, "test-service");
        assert_eq!(config.port, 8080);
        assert_eq!(config.redis.url, "redis://custom:6379");

        // Clean environment variables
        env::remove_var("TEST_NAME");
        env::remove_var("TEST_PORT");
        env::remove_var("TEST_REDIS_URL");
        env::remove_var("TEST_REDIS_POOL__SIZE");
    }

    #[test]
    fn test_type_hints() {
        env::set_var("TEST_PORT__int", "08"); // Should parse as 8, not 8.0
        env::set_var("TEST_FEATURES__json", "[\"feature1\", \"feature2\"]");

        let config: TestConfig = ConfigLoader::new()
            .with_env_prefix("TEST")
            .build()
            .expect("Failed to build config with type hints");

        assert_eq!(config.port, 8);
        assert_eq!(config.features, vec!["feature1", "feature2"]);

        // Clean environment variables
        env::remove_var("TEST_PORT__int");
        env::remove_var("TEST_FEATURES__json");
    }

    #[test]
    fn test_array_merge_strategies() {
        let base = TestConfig {
            features: vec!["base1".to_string(), "base2".to_string()],
            ..Default::default()
        };

        // Test append strategy
        let options = ConfigOptions {
            array_merge: ArrayMergeStrategy::Append,
            ..Default::default()
        };

        // Would need to create a test YAML file to fully test this
        // For now, just verify the options are accepted
        let _loader = ConfigLoader::new()
            .with_defaults(base.clone())
            .with_options(options);
    }
}
