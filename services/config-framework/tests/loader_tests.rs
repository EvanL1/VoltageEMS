use serde::{Deserialize, Serialize};
use std::any::Any;
use tempfile::TempDir;
use voltage_config::prelude::*;
use voltage_config::ConfigFormat;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestConfig {
    name: String,
    value: i32,
    nested: NestedConfig,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct NestedConfig {
    enabled: bool,
    items: Vec<String>,
}

impl Configurable for TestConfig {
    fn validate(&self) -> Result<()> {
        if self.value < 0 {
            return Err(ConfigError::Validation("Value must be non-negative".into()));
        }
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn test_load_defaults() {
    let loader = ConfigLoaderBuilder::new()
        .defaults(serde_json::json!({
            "name": "test",
            "value": 42,
            "nested": {
                "enabled": true,
                "items": ["a", "b", "c"]
            }
        }))
        .unwrap()
        .build()
        .unwrap();
    
    let config: TestConfig = loader.load().unwrap();
    
    assert_eq!(config.name, "test");
    assert_eq!(config.value, 42);
    assert!(config.nested.enabled);
    assert_eq!(config.nested.items, vec!["a", "b", "c"]);
}

#[test]
fn test_load_yaml() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yml");
    
    std::fs::write(&config_path, r#"
name: yaml_test
value: 100
nested:
  enabled: false
  items:
    - x
    - y
    - z
"#).unwrap();
    
    let loader = ConfigLoaderBuilder::new()
        .add_file(&config_path)
        .build()
        .unwrap();
    
    let config: TestConfig = loader.load().unwrap();
    
    assert_eq!(config.name, "yaml_test");
    assert_eq!(config.value, 100);
    assert!(!config.nested.enabled);
    assert_eq!(config.nested.items, vec!["x", "y", "z"]);
}

#[test]
fn test_load_toml() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.toml");
    
    std::fs::write(&config_path, r#"
name = "toml_test"
value = 200

[nested]
enabled = true
items = ["foo", "bar"]
"#).unwrap();
    
    let loader = ConfigLoaderBuilder::new()
        .add_file_with_format(&config_path, ConfigFormat::Toml)
        .build()
        .unwrap();
    
    let config: TestConfig = loader.load().unwrap();
    
    assert_eq!(config.name, "toml_test");
    assert_eq!(config.value, 200);
    assert!(config.nested.enabled);
    assert_eq!(config.nested.items, vec!["foo", "bar"]);
}

#[test]
fn test_load_json() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.json");
    
    std::fs::write(&config_path, r#"{
  "name": "json_test",
  "value": 300,
  "nested": {
    "enabled": false,
    "items": ["one", "two", "three"]
  }
}"#).unwrap();
    
    let loader = ConfigLoaderBuilder::new()
        .add_file_with_format(&config_path, ConfigFormat::Json)
        .build()
        .unwrap();
    
    let config: TestConfig = loader.load().unwrap();
    
    assert_eq!(config.name, "json_test");
    assert_eq!(config.value, 300);
    assert!(!config.nested.enabled);
    assert_eq!(config.nested.items, vec!["one", "two", "three"]);
}

#[test]
#[ignore = "Environment variable tests can be flaky in parallel test execution"]
fn test_env_override() {
    // Use a unique prefix to avoid test conflicts
    std::env::set_var("VTEST_NAME", "env_override");
    std::env::set_var("VTEST_VALUE", "999");
    std::env::set_var("VTEST_NESTED_ENABLED", "true");
    
    let loader = ConfigLoaderBuilder::new()
        .defaults(serde_json::json!({
            "name": "default",
            "value": 1,
            "nested": {
                "enabled": false,
                "items": []
            }
        }))
        .unwrap()
        .env_prefix("VTEST")
        .build()
        .unwrap();
    
    let config: TestConfig = loader.load().unwrap();
    
    assert_eq!(config.name, "env_override");
    assert_eq!(config.value, 999);
    assert!(config.nested.enabled);
    
    std::env::remove_var("VTEST_NAME");
    std::env::remove_var("VTEST_VALUE");
    std::env::remove_var("VTEST_NESTED_ENABLED");
}

#[test]
fn test_validation_error() {
    let loader = ConfigLoaderBuilder::new()
        .defaults(serde_json::json!({
            "name": "invalid",
            "value": -10,
            "nested": {
                "enabled": true,
                "items": []
            }
        }))
        .unwrap()
        .build()
        .unwrap();
    
    let result: Result<TestConfig> = loader.load();
    
    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigError::Validation(msg) => {
            assert_eq!(msg, "Value must be non-negative");
        }
        _ => panic!("Expected validation error"),
    }
}

#[test]
fn test_merge_configs() {
    let temp_dir = TempDir::new().unwrap();
    
    let base_config = temp_dir.path().join("base.yml");
    std::fs::write(&base_config, r#"
name: base
value: 50
nested:
  enabled: true
  items: [a, b]
"#).unwrap();
    
    let override_config = temp_dir.path().join("override.yml");
    std::fs::write(&override_config, r#"
value: 100
nested:
  items: [x, y, z]
"#).unwrap();
    
    let loader = ConfigLoaderBuilder::new()
        .add_file(&base_config)
        .add_file(&override_config)
        .build()
        .unwrap();
    
    let config: TestConfig = loader.load().unwrap();
    
    assert_eq!(config.name, "base");
    assert_eq!(config.value, 100);
    assert!(config.nested.enabled);
    assert_eq!(config.nested.items, vec!["x", "y", "z"]);
}