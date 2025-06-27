//! Serialization Utilities
//!
//! This module provides common serialization and deserialization utilities
//! for consistent data handling across the communication service.
//!
//! # Features
//!
//! - JSON serialization/deserialization with error handling
//! - YAML serialization/deserialization with error handling
//! - Pretty printing utilities
//! - Safe serialization with fallbacks
//!
//! # Examples
//!
//! ```rust
//! use comsrv::utils::serialization::{to_json_string, from_json_string, to_json_pretty};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct Config {
//!     name: String,
//!     value: i32,
//! }
//!
//! let config = Config { name: "test".to_string(), value: 42 };
//!
//! // Serialize to JSON
//! let json = to_json_string(&config)?;
//!
//! // Deserialize from JSON
//! let parsed: Config = from_json_string(&json)?;
//!
//! // Pretty print JSON
//! let pretty = to_json_pretty(&config)?;
//! ```

use crate::utils::error::{ComSrvError, Result};
use serde::{Deserialize, Serialize};

/// Serialize a value to JSON string
///
/// Provides a consistent way to serialize values to JSON with proper error handling.
///
/// # Arguments
///
/// * `value` - Value to serialize
///
/// # Returns
///
/// Result containing JSON string or serialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::to_json_string;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { value: i32 }
///
/// let data = Data { value: 42 };
/// let json = to_json_string(&data)?;
/// ```
pub fn to_json_string<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| {
        ComSrvError::SerializationError(format!("JSON serialization failed: {}", e))
    })
}

/// Serialize a value to pretty-printed JSON string
///
/// # Arguments
///
/// * `value` - Value to serialize
///
/// # Returns
///
/// Result containing pretty-printed JSON string or serialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::to_json_pretty;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { value: i32 }
///
/// let data = Data { value: 42 };
/// let pretty_json = to_json_pretty(&data)?;
/// ```
pub fn to_json_pretty<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(|e| {
        ComSrvError::SerializationError(format!("JSON pretty serialization failed: {}", e))
    })
}

/// Deserialize a value from JSON string
///
/// # Arguments
///
/// * `json` - JSON string to deserialize
///
/// # Returns
///
/// Result containing deserialized value or deserialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::from_json_string;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Data { value: i32 }
///
/// let json = r#"{"value": 42}"#;
/// let data: Data = from_json_string(json)?;
/// ```
pub fn from_json_string<T: for<'de> Deserialize<'de>>(json: &str) -> Result<T> {
    serde_json::from_str(json).map_err(|e| {
        ComSrvError::SerializationError(format!("JSON deserialization failed: {}", e))
    })
}

/// Serialize a value to YAML string
///
/// # Arguments
///
/// * `value` - Value to serialize
///
/// # Returns
///
/// Result containing YAML string or serialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::to_yaml_string;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { value: i32 }
///
/// let data = Data { value: 42 };
/// let yaml = to_yaml_string(&data)?;
/// ```
pub fn to_yaml_string<T: Serialize>(value: &T) -> Result<String> {
    serde_yaml::to_string(value).map_err(|e| {
        ComSrvError::SerializationError(format!("YAML serialization failed: {}", e))
    })
}

/// Deserialize a value from YAML string
///
/// # Arguments
///
/// * `yaml` - YAML string to deserialize
///
/// # Returns
///
/// Result containing deserialized value or deserialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::from_yaml_string;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Data { value: i32 }
///
/// let yaml = "value: 42";
/// let data: Data = from_yaml_string(yaml)?;
/// ```
pub fn from_yaml_string<T: for<'de> Deserialize<'de>>(yaml: &str) -> Result<T> {
    serde_yaml::from_str(yaml).map_err(|e| {
        ComSrvError::SerializationError(format!("YAML deserialization failed: {}", e))
    })
}

/// Safe JSON serialization with fallback
///
/// Attempts to serialize to JSON, but returns a fallback string if serialization fails.
///
/// # Arguments
///
/// * `value` - Value to serialize
/// * `fallback` - Fallback string to use if serialization fails
///
/// # Returns
///
/// JSON string or fallback string
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::to_json_safe;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { value: i32 }
///
/// let data = Data { value: 42 };
/// let json = to_json_safe(&data, "{}");
/// ```
pub fn to_json_safe<T: Serialize>(value: &T, fallback: &str) -> String {
    to_json_string(value).unwrap_or_else(|_| fallback.to_string())
}

/// Load and deserialize JSON from file
///
/// # Arguments
///
/// * `path` - File path to read from
///
/// # Returns
///
/// Result containing deserialized value or I/O/deserialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::load_json_from_file;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Config { name: String }
///
/// let config: Config = load_json_from_file("config.json")?;
/// ```
pub fn load_json_from_file<T: for<'de> Deserialize<'de>, P: AsRef<std::path::Path>>(
    path: P,
) -> Result<T> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        ComSrvError::IoError(format!("Failed to read file: {}", e))
    })?;
    from_json_string(&content)
}

/// Save serialized JSON to file
///
/// # Arguments
///
/// * `value` - Value to serialize and save
/// * `path` - File path to write to
///
/// # Returns
///
/// Result indicating success or I/O/serialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::save_json_to_file;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Config { name: String }
///
/// let config = Config { name: "test".to_string() };
/// save_json_to_file(&config, "config.json")?;
/// ```
pub fn save_json_to_file<T: Serialize, P: AsRef<std::path::Path>>(
    value: &T,
    path: P,
) -> Result<()> {
    let json = to_json_pretty(value)?;
    std::fs::write(path, json).map_err(|e| {
        ComSrvError::IoError(format!("Failed to write file: {}", e))
    })
}

/// Load and deserialize YAML from file
///
/// # Arguments
///
/// * `path` - File path to read from
///
/// # Returns
///
/// Result containing deserialized value or I/O/deserialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::load_yaml_from_file;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Config { name: String }
///
/// let config: Config = load_yaml_from_file("config.yaml")?;
/// ```
pub fn load_yaml_from_file<T: for<'de> Deserialize<'de>, P: AsRef<std::path::Path>>(
    path: P,
) -> Result<T> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        ComSrvError::IoError(format!("Failed to read file: {}", e))
    })?;
    from_yaml_string(&content)
}

/// Save serialized YAML to file
///
/// # Arguments
///
/// * `value` - Value to serialize and save
/// * `path` - File path to write to
///
/// # Returns
///
/// Result indicating success or I/O/serialization error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::serialization::save_yaml_to_file;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Config { name: String }
///
/// let config = Config { name: "test".to_string() };
/// save_yaml_to_file(&config, "config.yaml")?;
/// ```
pub fn save_yaml_to_file<T: Serialize, P: AsRef<std::path::Path>>(
    value: &T,
    path: P,
) -> Result<()> {
    let yaml = to_yaml_string(value)?;
    std::fs::write(path, yaml).map_err(|e| {
        ComSrvError::IoError(format!("Failed to write file: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::NamedTempFile;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
        enabled: bool,
    }

    impl TestData {
        fn sample() -> Self {
            Self {
                name: "test".to_string(),
                value: 42,
                enabled: true,
            }
        }
    }

    #[test]
    fn test_json_serialization() {
        let data = TestData::sample();
        let json = to_json_string(&data).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("42"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_json_pretty_serialization() {
        let data = TestData::sample();
        let pretty_json = to_json_pretty(&data).unwrap();
        assert!(pretty_json.contains("test"));
        assert!(pretty_json.contains('\n')); // Should have newlines for pretty printing
    }

    #[test]
    fn test_json_deserialization() {
        let json = r#"{"name":"test","value":42,"enabled":true}"#;
        let data: TestData = from_json_string(json).unwrap();
        assert_eq!(data, TestData::sample());
    }

    #[test]
    fn test_yaml_serialization() {
        let data = TestData::sample();
        let yaml = to_yaml_string(&data).unwrap();
        assert!(yaml.contains("test"));
        assert!(yaml.contains("42"));
        assert!(yaml.contains("true"));
    }

    #[test]
    fn test_yaml_deserialization() {
        let yaml = "name: test\nvalue: 42\nenabled: true\n";
        let data: TestData = from_yaml_string(yaml).unwrap();
        assert_eq!(data, TestData::sample());
    }

    #[test]
    fn test_json_safe_fallback() {
        let data = TestData::sample();
        let json = to_json_safe(&data, "fallback");
        assert!(json.contains("test")); // Should succeed

        // Test with a type that can't be serialized (this is a bit contrived)
        let fallback = to_json_safe(&std::f64::NAN, "fallback");
        assert!(fallback.contains("null") || fallback.contains("fallback"));
    }

    #[test]
    fn test_file_operations() {
        let data = TestData::sample();

        // Test JSON file operations
        let json_file = NamedTempFile::new().unwrap();
        save_json_to_file(&data, json_file.path()).unwrap();
        let loaded_json: TestData = load_json_from_file(json_file.path()).unwrap();
        assert_eq!(loaded_json, data);

        // Test YAML file operations
        let yaml_file = NamedTempFile::new().unwrap();
        save_yaml_to_file(&data, yaml_file.path()).unwrap();
        let loaded_yaml: TestData = load_yaml_from_file(yaml_file.path()).unwrap();
        assert_eq!(loaded_yaml, data);
    }

    #[test]
    fn test_error_handling() {
        // Test invalid JSON
        let invalid_json = r#"{"invalid": json"#;
        let result: Result<TestData> = from_json_string(invalid_json);
        assert!(result.is_err());

        // Test invalid YAML
        let invalid_yaml = "invalid: yaml: [unclosed";
        let result: Result<TestData> = from_yaml_string(invalid_yaml);
        assert!(result.is_err());
    }
} 