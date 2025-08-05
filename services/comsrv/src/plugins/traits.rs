//! pluginsysteminterfacedefinition
//!
//! package含protocolplugintraitdefinition、configuring模板system和相off的typedefinition

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;

use crate::core::combase::ComBase;
use crate::core::config::types::ChannelConfig;
use crate::utils::error::Result;

// ============================================================================
// protocolplugininterface
// ============================================================================

/// protocolpluginmetadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMetadata {
    /// uniqueprotocolidentifier符 (如 "`modbus_tcp`", "iec60870")
    pub id: String,
    /// 人class可读的protocolname
    pub name: String,
    /// protocolversion
    pub version: String,
    /// protocoldescription
    pub description: String,
    /// 作者info
    pub author: String,
    /// 许可证info
    pub license: String,
    /// supporting的feature
    pub features: Vec<String>,
    /// dependency项
    pub dependencies: HashMap<String, String>,
}

/// protocolplugintrait
///
/// allprotocolplugin必须implement此trait才能与pluginsystem兼容
#[async_trait]
pub trait ProtocolPlugin: Send + Sync + Any {
    /// Getprotocolmetadata
    fn metadata(&self) -> ProtocolMetadata;

    /// Getconfiguring模板
    fn config_template(&self) -> Vec<ConfigTemplate>;

    /// Validateconfiguring
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()>;

    /// Createprotocolinstance
    async fn create_instance(&self, channel_config: ChannelConfig) -> Result<Box<dyn ComBase>>;

    /// Getprotocol特定的CLI命令
    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![]
    }

    /// Getprotocoldocumentation
    fn documentation(&self) -> &'static str {
        "No documentation available"
    }

    /// 生成exampleconfiguring
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

/// plugin工厂functiontype
pub type PluginFactory = fn() -> Box<dyn ProtocolPlugin>;

/// Createplugininstance的辅助function
pub fn create_plugin_instance<T: ProtocolPlugin + Default + 'static>() -> Box<dyn ProtocolPlugin> {
    Box::new(T::default())
}

// ============================================================================
// configuring模板system
// ============================================================================

/// Configuration模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigTemplate {
    /// Parametername
    pub name: String,
    /// Parameterdescription
    pub description: String,
    /// Parametertype
    pub param_type: String,
    /// yesno必需
    pub required: bool,
    /// defaultvalue
    pub default_value: Option<Value>,
    /// Validaterule
    pub validation: Option<ValidationRule>,
}

/// Validaterule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    /// minvalue（数valuetype）
    pub min: Option<f64>,
    /// maxvalue（数valuetype）
    pub max: Option<f64>,
    /// 正则expressionpattern（字符串type）
    pub pattern: Option<String>,
    /// allowing的value（enumtype）
    pub allowed_values: Option<Vec<String>>,
}

/// Configurationpattern（用于UI生成）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    /// patternversion
    pub version: String,
    /// protocolID
    pub protocol_id: String,
    /// Configurationgrouping
    pub sections: Vec<ConfigSection>,
}

/// Configurationgrouping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSection {
    /// groupingname
    pub name: String,
    /// groupingdescription
    pub description: String,
    /// 该grouping的parameter
    pub parameters: Vec<ConfigParameter>,
}

/// Configurationparameterdefinition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigParameter {
    /// Parameterkey
    pub key: String,
    /// 显示name
    pub display_name: String,
    /// Parameterdescription
    pub description: String,
    /// Parametertype
    pub param_type: ParameterType,
    /// yesno必需
    pub required: bool,
    /// defaultvalue
    pub default: Option<Value>,
    /// Validaterule
    pub validation: Option<ParameterValidation>,
    /// yesno为advancedparameter
    pub advanced: bool,
    /// Parameterdependency
    pub depends_on: Option<ParameterDependency>,
}

/// Parametertype
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

/// enumvaluedefinition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

/// Parametervalidationrule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterValidation {
    /// 正则expressionpattern
    pub pattern: Option<String>,
    /// customvalidationfunction名
    pub custom_validator: Option<String>,
    /// Validateerrormessage
    pub error_message: Option<String>,
}

/// Parameterdependencydefinition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDependency {
    /// dependency的parameterkey
    pub key: String,
    /// dependencycondition
    pub condition: DependencyCondition,
}

/// dependencycondition
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
// CLI命令definition
// ============================================================================

/// CLI命令definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliCommand {
    /// 命令name
    pub name: String,
    /// 命令description
    pub description: String,
    /// 子命令
    pub subcommands: Vec<CliSubcommand>,
}

/// CLI子命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliSubcommand {
    /// 子命令name
    pub name: String,
    /// 子命令description
    pub description: String,
    /// Parameterlist
    pub arguments: Vec<CliArgument>,
    /// Processfunction名
    pub handler: String,
}

/// CLIparameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliArgument {
    /// Parametername
    pub name: String,
    /// Parameterdescription
    pub description: String,
    /// yesno必需
    pub required: bool,
    /// Parametertype
    pub arg_type: String,
    /// defaultvalue
    pub default: Option<String>,
}

// ============================================================================
// configuringvalidation和生成
// ============================================================================

/// Configurationvalidation器trait
pub trait ConfigValidator {
    /// Validateconfiguringvalue
    fn validate(&self, value: &Value) -> ValidationResult;
}

/// Validateresult
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

/// Configuration生成器trait
pub trait ConfigGenerator {
    /// 生成defaultconfiguring
    fn generate_default(&self) -> Value;

    /// 生成exampleconfiguring
    fn generate_example(&self) -> Value;

    /// slave模板生成configuring
    fn generate_from_template(&self, template: &ConfigSchema) -> Value;
}

// ============================================================================
// pluginregisteringmacro
// ============================================================================

/// registeringplugin的macro
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

/// protocolpluginmacro，简化plugindefinition
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
