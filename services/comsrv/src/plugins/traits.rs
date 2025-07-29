//! 插件系统接口定义
//!
//! 包含协议插件trait定义、配置模板系统和相关的类型定义

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;

use crate::core::combase::ComBase;
use crate::core::config::types::ChannelConfig;
use crate::utils::error::Result;

// ============================================================================
// 协议插件接口
// ============================================================================

/// 协议插件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMetadata {
    /// 唯一协议标识符 (如 "`modbus_tcp`", "iec60870")
    pub id: String,
    /// 人类可读的协议名称
    pub name: String,
    /// 协议版本
    pub version: String,
    /// 协议描述
    pub description: String,
    /// 作者信息
    pub author: String,
    /// 许可证信息
    pub license: String,
    /// 支持的特性
    pub features: Vec<String>,
    /// 依赖项
    pub dependencies: HashMap<String, String>,
}

/// 协议插件trait
///
/// 所有协议插件必须实现此trait才能与插件系统兼容
#[async_trait]
pub trait ProtocolPlugin: Send + Sync + Any {
    /// 获取协议元数据
    fn metadata(&self) -> ProtocolMetadata;

    /// 获取配置模板
    fn config_template(&self) -> Vec<ConfigTemplate>;

    /// 验证配置
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()>;

    /// 创建协议实例
    async fn create_instance(&self, channel_config: ChannelConfig) -> Result<Box<dyn ComBase>>;

    /// 获取协议特定的CLI命令
    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![]
    }

    /// 获取协议文档
    fn documentation(&self) -> &'static str {
        "No documentation available"
    }

    /// 生成示例配置
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

/// 插件工厂函数类型
pub type PluginFactory = fn() -> Box<dyn ProtocolPlugin>;

/// 创建插件实例的辅助函数
pub fn create_plugin_instance<T: ProtocolPlugin + Default + 'static>() -> Box<dyn ProtocolPlugin> {
    Box::new(T::default())
}

// ============================================================================
// 配置模板系统
// ============================================================================

/// 配置模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigTemplate {
    /// 参数名称
    pub name: String,
    /// 参数描述
    pub description: String,
    /// 参数类型
    pub param_type: String,
    /// 是否必需
    pub required: bool,
    /// 默认值
    pub default_value: Option<Value>,
    /// 验证规则
    pub validation: Option<ValidationRule>,
}

/// 验证规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    /// 最小值（数值类型）
    pub min: Option<f64>,
    /// 最大值（数值类型）
    pub max: Option<f64>,
    /// 正则表达式模式（字符串类型）
    pub pattern: Option<String>,
    /// 允许的值（枚举类型）
    pub allowed_values: Option<Vec<String>>,
}

/// 配置模式（用于UI生成）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    /// 模式版本
    pub version: String,
    /// 协议ID
    pub protocol_id: String,
    /// 配置分组
    pub sections: Vec<ConfigSection>,
}

/// 配置分组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSection {
    /// 分组名称
    pub name: String,
    /// 分组描述
    pub description: String,
    /// 该分组的参数
    pub parameters: Vec<ConfigParameter>,
}

/// 配置参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigParameter {
    /// 参数键
    pub key: String,
    /// 显示名称
    pub display_name: String,
    /// 参数描述
    pub description: String,
    /// 参数类型
    pub param_type: ParameterType,
    /// 是否必需
    pub required: bool,
    /// 默认值
    pub default: Option<Value>,
    /// 验证规则
    pub validation: Option<ParameterValidation>,
    /// 是否为高级参数
    pub advanced: bool,
    /// 参数依赖
    pub depends_on: Option<ParameterDependency>,
}

/// 参数类型
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

/// 枚举值定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

/// 参数验证规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterValidation {
    /// 正则表达式模式
    pub pattern: Option<String>,
    /// 自定义验证函数名
    pub custom_validator: Option<String>,
    /// 验证错误消息
    pub error_message: Option<String>,
}

/// 参数依赖定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDependency {
    /// 依赖的参数键
    pub key: String,
    /// 依赖条件
    pub condition: DependencyCondition,
}

/// 依赖条件
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
// CLI命令定义
// ============================================================================

/// CLI命令定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliCommand {
    /// 命令名称
    pub name: String,
    /// 命令描述
    pub description: String,
    /// 子命令
    pub subcommands: Vec<CliSubcommand>,
}

/// CLI子命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliSubcommand {
    /// 子命令名称
    pub name: String,
    /// 子命令描述
    pub description: String,
    /// 参数列表
    pub arguments: Vec<CliArgument>,
    /// 处理函数名
    pub handler: String,
}

/// CLI参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliArgument {
    /// 参数名称
    pub name: String,
    /// 参数描述
    pub description: String,
    /// 是否必需
    pub required: bool,
    /// 参数类型
    pub arg_type: String,
    /// 默认值
    pub default: Option<String>,
}

// ============================================================================
// 配置验证和生成
// ============================================================================

/// 配置验证器trait
pub trait ConfigValidator {
    /// 验证配置值
    fn validate(&self, value: &Value) -> ValidationResult;
}

/// 验证结果
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

/// 配置生成器trait
pub trait ConfigGenerator {
    /// 生成默认配置
    fn generate_default(&self) -> Value;

    /// 生成示例配置
    fn generate_example(&self) -> Value;

    /// 从模板生成配置
    fn generate_from_template(&self, template: &ConfigSchema) -> Value;
}

// ============================================================================
// 插件注册宏
// ============================================================================

/// 注册插件的宏
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

/// 协议插件宏，简化插件定义
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
