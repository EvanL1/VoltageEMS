//! 通用配置加载器
//!
//! 提供统一的配置加载优先级：
//! 1. 默认值（最低）
//! 2. 环境变量（中）
//! 3. YAML文件（最高）

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use tracing::{debug, info};

/// 配置加载错误
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("YAML解析错误: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("JSON解析错误: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),

    #[error("配置合并错误: {0}")]
    MergeError(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// 通用配置加载器
pub struct ConfigLoader<T>
where
    T: Default + DeserializeOwned + Serialize,
{
    /// 默认配置
    defaults: T,
    /// 环境变量前缀
    env_prefix: Option<String>,
    /// YAML文件路径
    yaml_path: Option<String>,
    /// 是否允许环境变量覆盖
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
    /// 创建新的配置加载器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置默认配置
    pub fn with_defaults(mut self, defaults: T) -> Self {
        self.defaults = defaults;
        self
    }

    /// 设置环境变量前缀
    pub fn with_env_prefix(mut self, prefix: &str) -> Self {
        self.env_prefix = Some(prefix.to_string());
        self.allow_env_override = true;
        self
    }

    /// 设置YAML配置文件路径
    pub fn with_yaml_file(mut self, path: &str) -> Self {
        self.yaml_path = Some(path.to_string());
        self
    }

    /// 构建最终配置
    pub fn build(self) -> Result<T> {
        // 1. 从默认值开始
        let mut config_json = serde_json::to_value(&self.defaults)?;
        debug!("从默认配置开始构建");

        // 2. 应用环境变量（中优先级）
        if let Some(prefix) = &self.env_prefix {
            if self.allow_env_override {
                debug!("应用环境变量，前缀: {}", prefix);
                self.apply_env_vars(&mut config_json, prefix)?;
            }
        }

        // 3. 应用YAML文件（最高优先级）
        if let Some(yaml_path) = &self.yaml_path {
            if Path::new(yaml_path).exists() {
                info!("加载YAML配置文件: {}", yaml_path);
                let yaml_content = std::fs::read_to_string(yaml_path)?;
                let yaml_value: YamlValue = serde_yaml::from_str(&yaml_content)?;

                // 将YAML值合并到配置中
                self.merge_yaml_into_json(&mut config_json, &yaml_value)?;
            } else {
                debug!("YAML配置文件不存在，跳过: {}", yaml_path);
            }
        }

        // 4. 反序列化为最终配置
        let config: T = serde_json::from_value(config_json)?;
        Ok(config)
    }

    /// 应用环境变量到配置
    fn apply_env_vars(&self, config: &mut JsonValue, prefix: &str) -> Result<()> {
        // 收集所有以指定前缀开头的环境变量
        let env_vars: HashMap<String, String> =
            env::vars().filter(|(k, _)| k.starts_with(prefix)).collect();

        for (key, value) in env_vars {
            // 移除前缀，转换为配置路径
            let path = key
                .strip_prefix(prefix)
                .unwrap_or(&key)
                .trim_start_matches('_')
                .to_lowercase()
                .replace('_', ".");

            if !path.is_empty() {
                debug!("应用环境变量 {} = {} 到路径 {}", key, value, path);
                self.set_value_by_path(config, &path, &value)?;
            }
        }

        Ok(())
    }

    /// 合并YAML值到JSON配置中
    fn merge_yaml_into_json(&self, json: &mut JsonValue, yaml: &YamlValue) -> Result<()> {
        // 转换YAML值为JSON值
        let yaml_as_json = self.yaml_to_json(yaml)?;

        // 递归合并
        Self::merge_json_values(json, &yaml_as_json);

        Ok(())
    }

    /// 将YAML值转换为JSON值
    fn yaml_to_json(&self, yaml: &YamlValue) -> Result<JsonValue> {
        // 先序列化为字符串，再解析为JSON
        let yaml_str = serde_yaml::to_string(yaml)?;
        let json_value: JsonValue = serde_yaml::from_str(&yaml_str)?;
        Ok(json_value)
    }

    /// 递归合并两个JSON值
    fn merge_json_values(base: &mut JsonValue, overlay: &JsonValue) {
        match (base, overlay) {
            (JsonValue::Object(base_map), JsonValue::Object(overlay_map)) => {
                // 合并对象
                for (key, overlay_value) in overlay_map {
                    match base_map.get_mut(key) {
                        Some(base_value) => {
                            // 递归合并
                            Self::merge_json_values(base_value, overlay_value);
                        },
                        None => {
                            // 新键，直接插入
                            base_map.insert(key.clone(), overlay_value.clone());
                        },
                    }
                }
            },
            (base, overlay) => {
                // 其他类型直接替换
                *base = overlay.clone();
            },
        }
    }

    /// 根据路径设置值
    fn set_value_by_path(&self, config: &mut JsonValue, path: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = config;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // 最后一个部分，设置值
                if let JsonValue::Object(map) = current {
                    // 尝试解析值的类型
                    let parsed_value = self.parse_env_value(value);
                    map.insert(part.to_string(), parsed_value);
                }
            } else {
                // 中间部分，确保是对象
                if let JsonValue::Object(map) = current {
                    current = map
                        .entry(part.to_string())
                        .or_insert_with(|| JsonValue::Object(serde_json::Map::new()));
                }
            }
        }

        Ok(())
    }

    /// 解析环境变量值的类型
    fn parse_env_value(&self, value: &str) -> JsonValue {
        // 尝试解析为布尔值
        if let Ok(bool_val) = value.parse::<bool>() {
            return JsonValue::Bool(bool_val);
        }

        // 尝试解析为数字
        if let Ok(int_val) = value.parse::<i64>() {
            return JsonValue::Number(serde_json::Number::from(int_val));
        }

        if let Ok(float_val) = value.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(float_val) {
                return JsonValue::Number(num);
            }
        }

        // 默认作为字符串
        JsonValue::String(value.to_string())
    }
}

/// 配置加载辅助函数
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

        // 清理环境变量
        env::remove_var("TEST_NAME");
        env::remove_var("TEST_PORT");
        env::remove_var("TEST_REDIS_URL");
    }
}
