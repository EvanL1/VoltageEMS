//! Protocol Plugin System
//!
//! This module provides the core trait and macro definitions for protocol plugins,
//! enabling dynamic protocol loading and standardized protocol implementation.

use async_trait::async_trait;
use std::collections::HashMap;
use std::any::Any;
use serde::{Serialize, Deserialize};
use serde_json::Value;

use crate::core::protocols::common::traits::ComBase;
use crate::core::config::types::channel::ChannelConfig;
use crate::utils::Result;

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
    /// Required dependencies
    pub dependencies: HashMap<String, String>,
}

/// Configuration template for protocol parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigTemplate {
    /// Parameter name
    pub name: String,
    /// Parameter description
    pub description: String,
    /// Parameter type (string, int, bool, etc.)
    pub param_type: String,
    /// Whether the parameter is required
    pub required: bool,
    /// Default value if any
    pub default_value: Option<Value>,
    /// Validation rules
    pub validation: Option<ValidationRule>,
}

/// Validation rules for configuration parameters
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

/// Protocol plugin trait
///
/// All protocol plugins must implement this trait to be compatible
/// with the plugin system.
#[async_trait]
pub trait ProtocolPlugin: Send + Sync + Any {
    /// Get protocol metadata
    fn metadata(&self) -> ProtocolMetadata;
    
    /// Get configuration template
    fn config_template(&self) -> Vec<ConfigTemplate>;
    
    /// Validate configuration
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()>;
    
    /// Create a new protocol instance
    async fn create_instance(
        &self,
        channel_config: ChannelConfig,
    ) -> Result<Box<dyn ComBase>>;
    
    /// Get protocol-specific CLI commands
    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![]
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
    
    /// Get protocol documentation
    fn documentation(&self) -> &str {
        ""
    }
}

/// CLI command definition for protocol-specific tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliCommand {
    /// Command name
    pub name: String,
    /// Command description
    pub description: String,
    /// Command arguments
    pub args: Vec<CliArgument>,
}

/// CLI argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliArgument {
    /// Argument name
    pub name: String,
    /// Argument description
    pub description: String,
    /// Whether the argument is required
    pub required: bool,
    /// Default value
    pub default: Option<String>,
}

/// Plugin factory function type
pub type PluginFactory = fn() -> Box<dyn ProtocolPlugin>;

/// Macro to simplify protocol plugin implementation
///
/// Example usage:
/// ```
/// protocol_plugin! {
///     id: "modbus_tcp",
///     name: "Modbus TCP Protocol",
///     version: "1.0.0",
///     description: "Modbus TCP protocol implementation",
///     author: "VoltageEMS Team",
///     license: "MIT",
///     features: ["telemetry", "control", "adjustment", "signal"],
///     config: [
///         {
///             name: "host",
///             description: "Modbus server host address",
///             param_type: "string",
///             required: true,
///         },
///         {
///             name: "port",
///             description: "Modbus server port",
///             param_type: "int",
///             required: false,
///             default: 502,
///             validation: {
///                 min: 1,
///                 max: 65535,
///             }
///         }
///     ]
/// }
/// ```
#[macro_export]
macro_rules! protocol_plugin {
    (
        id: $id:expr,
        name: $name:expr,
        version: $version:expr,
        description: $description:expr,
        author: $author:expr,
        license: $license:expr,
        features: [$($feature:expr),*],
        config: [$($config_item:tt),*]
    ) => {
        pub struct PluginMetadataImpl;
        
        impl PluginMetadataImpl {
            pub fn metadata() -> $crate::core::plugins::protocol_plugin::ProtocolMetadata {
                $crate::core::plugins::protocol_plugin::ProtocolMetadata {
                    id: $id.to_string(),
                    name: $name.to_string(),
                    version: $version.to_string(),
                    description: $description.to_string(),
                    author: $author.to_string(),
                    license: $license.to_string(),
                    features: vec![$($feature.to_string()),*],
                    dependencies: std::collections::HashMap::new(),
                }
            }
            
            pub fn config_template() -> Vec<$crate::core::plugins::protocol_plugin::ConfigTemplate> {
                vec![
                    $(protocol_plugin!(@config_item $config_item)),*
                ]
            }
        }
    };
    
    (@config_item {
        name: $name:expr,
        description: $description:expr,
        param_type: $param_type:expr,
        required: $required:expr
        $(, default: $default:expr)?
        $(, validation: $validation:tt)?
    }) => {
        $crate::core::plugins::protocol_plugin::ConfigTemplate {
            name: $name.to_string(),
            description: $description.to_string(),
            param_type: $param_type.to_string(),
            required: $required,
            default_value: protocol_plugin!(@default $($default)?),
            validation: protocol_plugin!(@validation $($validation)?),
        }
    };
    
    (@default) => { None };
    (@default $default:expr) => { Some(serde_json::json!($default)) };
    
    (@validation) => { None };
    (@validation { $($field:ident: $value:expr),* }) => {
        Some($crate::core::plugins::protocol_plugin::ValidationRule {
            $(
                $field: protocol_plugin!(@validation_field $field, $value),
            )*
            ..Default::default()
        })
    };
    
    (@validation_field min, $value:expr) => { Some($value as f64) };
    (@validation_field max, $value:expr) => { Some($value as f64) };
    (@validation_field pattern, $value:expr) => { Some($value.to_string()) };
    (@validation_field allowed_values, [$($v:expr),*]) => { Some(vec![$($v.to_string()),*]) };
}

/// Derive macro attributes for automatic validation
pub trait Validatable {
    fn validate(&self) -> Result<()>;
}

/// Helper function to create a plugin instance
pub fn create_plugin_instance<T: ProtocolPlugin + Default + 'static>() -> Box<dyn ProtocolPlugin> {
    Box::new(T::default())
}

// Re-export for convenience
pub use protocol_plugin;