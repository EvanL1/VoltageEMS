//! 通用configuringloading器
//!
//! 提供统一的configuringloadingpriority：
//! 1. defaultvalue（最low）
//! 2. cycle境variable（medium）
//! 3. YAMLfile（最high）

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use tracing::{debug, info};

/// Configurationloadingerror
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("YAMLparseerror: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("JSONparseerror: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IOerror: {0}")]
    IoError(#[from] std::io::Error),

    #[error("configuringmergeerror: {0}")]
    MergeError(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// 通用configuringloading器
pub struct ConfigLoader<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    /// defaultconfiguring
    defaults: T,
    /// cycle境variable前缀
    env_prefix: Option<String>,
    /// YAMLfilepath
    yaml_path: Option<String>,
    /// yesnoallowingcycle境variable覆盖
    allow_env_override: bool,
}

impl<T> Default for ConfigLoader<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    fn default() -> Self {
        Self {
            defaults: T::default(),
            env_prefix: None,
            yaml_path: None,
            allow_env_override: true,
        }
    }
}

impl<T> ConfigLoader<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    /// Create新的configuringloading器
    pub fn new() -> Self {
        Self::default()
    }

    /// Setdefaultconfiguring
    pub fn with_defaults(mut self, defaults: T) -> Self {
        self.defaults = defaults;
        self
    }

    /// Setcycle境variable前缀
    pub fn with_env_prefix(mut self, prefix: &str) -> Self {
        self.env_prefix = Some(prefix.to_string());
        self.allow_env_override = true;
        self
    }

    /// SetYAMLconfiguringfilepath
    pub fn with_yaml_file(mut self, path: &str) -> Self {
        self.yaml_path = Some(path.to_string());
        self
    }

    /// Build最终configuring
    pub fn build(self) -> Result<T> {
        // 1. slavedefaultvaluestart
        let mut config_json = serde_json::to_value(&self.defaults)?;
        debug!("Starting from default configuration");

        // 2. 应用cycle境variable（mediumpriority）
        if let Some(prefix) = &self.env_prefix {
            if self.allow_env_override {
                debug!("Applying environment variables, prefix: {}", prefix);
                self.apply_env_vars(&mut config_json, prefix)?;
            }
        }

        // 3. 应用YAMLfile（最highpriority）
        if let Some(yaml_path) = &self.yaml_path {
            if Path::new(yaml_path).exists() {
                info!("Loading YAML config file: {}", yaml_path);
                let yaml_content = std::fs::read_to_string(yaml_path)?;
                let yaml_value: YamlValue = serde_yaml::from_str(&yaml_content)?;

                // 将YAMLvaluemerge到configuringmedium
                self.merge_yaml_into_json(&mut config_json, &yaml_value)?;
            } else {
                debug!("YAML config file not found, skipping: {}", yaml_path);
            }
        }

        // 4. 反serializing为最终configuring
        let config: T = serde_json::from_value(config_json)?;
        Ok(config)
    }

    /// 应用cycle境variable到configuring
    fn apply_env_vars(&self, config: &mut JsonValue, prefix: &str) -> Result<()> {
        // 收集all以指定前缀on头的cycle境variable
        let env_vars: HashMap<String, String> =
            env::vars().filter(|(k, _)| k.starts_with(prefix)).collect();

        for (key, value) in env_vars {
            // 移除前缀，converting为configuringpath
            let path = key
                .strip_prefix(prefix)
                .unwrap_or(&key)
                .trim_start_matches('_')
                .to_lowercase()
                .replace('_', ".");

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

    /// mergeYAMLvalue到JSONconfiguringmedium
    fn merge_yaml_into_json(&self, json: &mut JsonValue, yaml: &YamlValue) -> Result<()> {
        // convertingYAMLvalue为JSONvalue
        let yaml_as_json = self.yaml_to_json(yaml)?;

        // recursivemerge
        Self::merge_json_values(json, &yaml_as_json);

        Ok(())
    }

    /// 将YAMLvalueconverting为JSONvalue
    fn yaml_to_json(&self, yaml: &YamlValue) -> Result<JsonValue> {
        // 先serializing为字符串，再parse为JSON
        let yaml_str = serde_yaml::to_string(yaml)?;
        let json_value: JsonValue = serde_yaml::from_str(&yaml_str)?;
        Ok(json_value)
    }

    /// recursivemerge两个JSONvalue
    fn merge_json_values(base: &mut JsonValue, overlay: &JsonValue) {
        match (base, overlay) {
            (JsonValue::Object(base_map), JsonValue::Object(overlay_map)) => {
                // mergepair象
                for (key, overlay_value) in overlay_map {
                    match base_map.get_mut(key) {
                        Some(base_value) => {
                            // recursivemerge
                            Self::merge_json_values(base_value, overlay_value);
                        },
                        None => {
                            // 新key，直接insert
                            base_map.insert(key.clone(), overlay_value.clone());
                        },
                    }
                }
            },
            (base, overlay) => {
                // othertype直接替换
                *base = overlay.clone();
            },
        }
    }

    /// 根据pathsettingvalue
    fn set_value_by_path(&self, config: &mut JsonValue, path: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = config;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // 最后一个partial，settingvalue
                if let JsonValue::Object(map) = current {
                    // 尝试parsevalue的type
                    let parsed_value = self.parse_env_value(value);
                    map.insert(part.to_string(), parsed_value);
                }
            } else {
                // medium间partial，确保yespair象
                if let JsonValue::Object(map) = current {
                    current = map
                        .entry(part.to_string())
                        .or_insert_with(|| JsonValue::Object(serde_json::Map::new()));
                }
            }
        }

        Ok(())
    }

    /// Parsecycle境variablevalue的type
    fn parse_env_value(&self, value: &str) -> JsonValue {
        // 尝试parse为布尔value
        if let Ok(bool_val) = value.parse::<bool>() {
            return JsonValue::Bool(bool_val);
        }

        // 尝试parse为数字
        if let Ok(int_val) = value.parse::<i64>() {
            return JsonValue::Number(serde_json::Number::from(int_val));
        }

        if let Ok(float_val) = value.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(float_val) {
                return JsonValue::Number(num);
            }
        }

        // default作为字符串
        JsonValue::String(value.to_string())
    }
}

/// Configurationloading辅助function
pub fn load_config<T>(service_name: &str) -> Result<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    let config_file = format!("config/{}.yaml", service_name.to_lowercase());
    let env_prefix = service_name.to_uppercase();

    ConfigLoader::new()
        .with_env_prefix(&env_prefix)
        .with_yaml_file(&config_file)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        name: String,
        port: u16,
        #[serde(default)]
        redis: RedisConfig,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
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

        let config: TestConfig = ConfigLoader::new()
            .with_env_prefix("TEST")
            .build()
            .expect("Failed to build config with env overrides");

        assert_eq!(config.name, "test-service");
        assert_eq!(config.port, 8080);
        assert_eq!(config.redis.url, "redis://custom:6379");

        // cleaningcycle境variable
        env::remove_var("TEST_NAME");
        env::remove_var("TEST_PORT");
        env::remove_var("TEST_REDIS_URL");
    }
}
