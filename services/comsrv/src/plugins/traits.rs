//! Plugin system interface definitions
//!
//! Contains protocol plugin trait definitions, configuration template system and related type definitions

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;

use crate::core::combase::{ComClient, ComServer};
use crate::core::config::types::ChannelConfig;
use crate::utils::error::{ComSrvError, Result};
use std::sync::Arc;

// ============================================================================
// Protocol plugin interface
// ============================================================================

/// Protocol plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMetadata {
    /// Unique protocol identifier (e.g., "modbus_tcp", "iec60870")
    pub id: String,
    /// Human-readable protocol name
    pub name: String,
    /// Protocol version
    pub version: String,
    /// Protocol description
    pub description: String,
    /// Author information
    pub author: String,
    /// License information
    pub license: String,
    /// Supported features
    pub features: Vec<String>,
    /// Dependencies
    pub dependencies: HashMap<String, String>,
}

/// Protocol plugin trait
///
/// All protocol plugins must implement this trait to be compatible with the plugin system
#[async_trait]
pub trait ProtocolPlugin: Send + Sync + Any {
    /// Get protocol metadata
    fn metadata(&self) -> ProtocolMetadata;

    /// Get configuration template
    fn config_template(&self) -> Vec<ConfigTemplate>;

    /// Validate configuration
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()>;

    /// Create protocol client instance
    /// Default implementation returns NotSupported error
    async fn create_client(
        &self,
        _channel_config: Arc<ChannelConfig>,
    ) -> Result<Box<dyn ComClient>> {
        Err(ComSrvError::NotSupported(
            "Client mode not supported by this plugin".to_string(),
        ))
    }

    /// Create protocol server instance
    /// Default implementation returns NotSupported error
    async fn create_server(
        &self,
        _channel_config: Arc<ChannelConfig>,
    ) -> Result<Box<dyn ComServer>> {
        Err(ComSrvError::NotSupported(
            "Server mode not supported by this plugin".to_string(),
        ))
    }

    // Removed deprecated create_instance method since trait upcasting is not stable
    // Plugins should directly implement create_client and/or create_server

    /// Get protocol-specific CLI commands
    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![]
    }

    /// Get protocol documentation
    fn documentation(&self) -> &'static str {
        "No documentation available"
    }

    /// Generate example configuration
    fn generate_example_config(&self) -> HashMap<String, Value> {
        let mut config = HashMap::new();
        for template in self.config_template() {
            if let Some(default) = template.default_value {
                config.insert(template.name, default);
            }
        }
        config
    }
}

/// Plugin factory function type
pub type PluginFactory = fn() -> Box<dyn ProtocolPlugin>;

/// Helper function to create plugin instance
pub fn create_plugin_instance<T: ProtocolPlugin + Default + 'static>() -> Box<dyn ProtocolPlugin> {
    Box::new(T::default())
}

// ============================================================================
// Configuration template system
// ============================================================================

/// Configuration template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigTemplate {
    /// Parameter name
    pub name: String,
    /// Parameter description
    pub description: String,
    /// Parameter type
    pub param_type: String,
    /// Whether required
    pub required: bool,
    /// Default value
    pub default_value: Option<Value>,
    /// Validation rule
    pub validation: Option<ValidationRule>,
}

/// Validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    /// Minimum value (for numeric types)
    pub min: Option<f64>,
    /// Maximum value (for numeric types)
    pub max: Option<f64>,
    /// Regular expression pattern (for string types)
    pub pattern: Option<String>,
    /// Allowed values (for enum types)
    pub allowed_values: Option<Vec<String>>,
}

/// Configuration schema (for UI generation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    /// Schema version
    pub version: String,
    /// Protocol ID
    pub protocol_id: String,
    /// Configuration sections
    pub sections: Vec<ConfigSection>,
}

/// Configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSection {
    /// Section name
    pub name: String,
    /// Section description
    pub description: String,
    /// Parameters in this section
    pub parameters: Vec<ConfigParameter>,
}

/// Configuration parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigParameter {
    /// Parameter key
    pub key: String,
    /// Display name
    pub display_name: String,
    /// Parameter description
    pub description: String,
    /// Parameter type
    pub param_type: ParameterType,
    /// Whether required
    pub required: bool,
    /// Default value
    pub default: Option<Value>,
    /// Validation rule
    pub validation: Option<ParameterValidation>,
    /// Whether this is an advanced parameter
    pub advanced: bool,
    /// Parameter dependency
    pub depends_on: Option<ParameterDependency>,
}

/// Parameter type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ParameterType {
    String,
    Integer {
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        min: Option<f64>,
        max: Option<f64>,
    },
    Boolean,
    Enum {
        values: Vec<EnumValue>,
    },
    Array {
        item_type: Box<ParameterType>,
    },
    Object {
        properties: HashMap<String, ParameterType>,
    },
    Duration,
    IpAddress,
    Port,
    FilePath,
    Secret,
}

/// Enum value definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

/// Parameter validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterValidation {
    /// Regular expression pattern
    pub pattern: Option<String>,
    /// Custom validation function name
    pub custom_validator: Option<String>,
    /// Validation error message
    pub error_message: Option<String>,
}

/// Parameter dependency definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDependency {
    /// Dependent parameter key
    pub key: String,
    /// Dependency condition
    pub condition: DependencyCondition,
}

/// Dependency condition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DependencyCondition {
    Equals { value: Value },
    NotEquals { value: Value },
    GreaterThan { value: f64 },
    LessThan { value: f64 },
    In { values: Vec<Value> },
    NotIn { values: Vec<Value> },
}

// ============================================================================
// CLI command definitions
// ============================================================================

/// CLI command definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliCommand {
    /// Command name
    pub name: String,
    /// Command description
    pub description: String,
    /// Subcommands
    pub subcommands: Vec<CliSubcommand>,
}

/// CLI subcommand
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliSubcommand {
    /// Subcommand name
    pub name: String,
    /// Subcommand description
    pub description: String,
    /// Parameter list
    pub arguments: Vec<CliArgument>,
    /// Handler function name
    pub handler: String,
}

/// CLI parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliArgument {
    /// Parameter name
    pub name: String,
    /// Parameter description
    pub description: String,
    /// Whether required
    pub required: bool,
    /// Parameter type
    pub arg_type: String,
    /// Default value
    pub default: Option<String>,
}

// ============================================================================
// Configuration validation and generation
// ============================================================================

/// Configuration validator trait
pub trait ConfigValidator {
    /// Validate configuration value
    fn validate(&self, value: &Value) -> ValidationResult;
}

/// Validation result
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            valid: false,
            errors: vec![message],
            warnings: vec![],
        }
    }
}

/// Configuration generator trait
pub trait ConfigGenerator {
    /// Generate default configuration
    fn generate_default(&self) -> Value;

    /// Generate example configuration
    fn generate_example(&self) -> Value;

    /// Generate configuration from template
    fn generate_from_template(&self, template: &ConfigSchema) -> Value;
}

// ============================================================================
// Plugin registration macros
// ============================================================================

/// Macro for registering plugins
#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty, $plugin_id:expr) => {
        paste::paste! {
            #[no_mangle]
            pub extern "C" fn [<register_ $plugin_id>]() {
                let registry = $crate::plugins::PluginRegistry::global();
                let mut reg = registry.write().unwrap();
                let plugin = Box::new(<$plugin_type>::new());
                if let Err(e) = reg.register_plugin(plugin) {
                    tracing::error!("Failed to register plugin {}: {}", $plugin_id, e);
                }
            }
        }
    };
}

/// Protocol plugin macro to simplify plugin definition
#[macro_export]
macro_rules! protocol_plugin {
    (
        metadata: {
            id: $id:expr,
            name: $name:expr,
            version: $version:expr,
            author: $author:expr,
            description: $description:expr,
        },
        config: [
            $($config:tt)*
        ]
    ) => {
        pub fn plugin_metadata() -> $crate::plugins::ProtocolMetadata {
            $crate::plugins::ProtocolMetadata {
                id: $id.to_string(),
                name: $name.to_string(),
                version: $version.to_string(),
                author: $author.to_string(),
                description: $description.to_string(),
                license: "Apache-2.0".to_string(),
                features: vec![],
                dependencies: std::collections::HashMap::new(),
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        let success = ValidationResult::success();
        assert!(success.valid);
        assert!(success.errors.is_empty());

        let error = ValidationResult::error("Test error".to_string());
        assert!(!error.valid);
        assert_eq!(error.errors.len(), 1);
    }

    #[test]
    fn test_parameter_type_serialization() {
        let int_type = ParameterType::Integer {
            min: Some(0),
            max: Some(100),
        };

        let json = serde_json::to_string(&int_type).unwrap();
        assert!(json.contains("\"type\":\"Integer\""));

        let enum_type = ParameterType::Enum {
            values: vec![EnumValue {
                value: "tcp".to_string(),
                label: "TCP".to_string(),
                description: Some("TCP transport".to_string()),
            }],
        };

        let json = serde_json::to_string(&enum_type).unwrap();
        assert!(json.contains("\"type\":\"Enum\""));
    }
}
