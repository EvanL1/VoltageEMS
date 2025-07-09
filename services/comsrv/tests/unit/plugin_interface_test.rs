//! 插件接口单元测试
//!
//! 测试协议插件接口的基本功能，包括元数据、配置验证、实例创建等

use comsrv::core::plugins::{ProtocolPlugin, ProtocolMetadata, ConfigTemplate, ValidationRule};
use std::collections::HashMap;
use serde_json::{json, Value};
use async_trait::async_trait;

/// 测试用的模拟协议插件
struct MockProtocolPlugin {
    id: String,
}

#[async_trait]
impl ProtocolPlugin for MockProtocolPlugin {
    fn metadata(&self) -> ProtocolMetadata {
        ProtocolMetadata {
            id: self.id.clone(),
            name: "Mock Protocol".to_string(),
            version: "1.0.0".to_string(),
            description: "A mock protocol for testing".to_string(),
            author: "Test Author".to_string(),
            license: "MIT".to_string(),
            features: vec!["telemetry".to_string(), "control".to_string()],
            dependencies: HashMap::new(),
        }
    }
    
    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![
            ConfigTemplate {
                name: "host".to_string(),
                description: "Server host address".to_string(),
                param_type: "string".to_string(),
                required: true,
                default_value: Some(json!("127.0.0.1")),
                validation: None,
            },
            ConfigTemplate {
                name: "port".to_string(),
                description: "Server port".to_string(),
                param_type: "int".to_string(),
                required: true,
                default_value: Some(json!(502)),
                validation: Some(ValidationRule {
                    min: Some(1.0),
                    max: Some(65535.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "protocol_version".to_string(),
                description: "Protocol version".to_string(),
                param_type: "string".to_string(),
                required: false,
                default_value: Some(json!("v1")),
                validation: Some(ValidationRule {
                    min: None,
                    max: None,
                    pattern: None,
                    allowed_values: Some(vec!["v1".to_string(), "v2".to_string()]),
                }),
            },
        ]
    }
    
    fn validate_config(&self, config: &HashMap<String, Value>) -> comsrv::utils::Result<()> {
        // 验证必需参数
        if !config.contains_key("host") {
            return Err(comsrv::utils::Error::Config("Missing required parameter: host".into()));
        }
        
        if let Some(port) = config.get("port") {
            if let Some(port_num) = port.as_i64() {
                if port_num < 1 || port_num > 65535 {
                    return Err(comsrv::utils::Error::Config("Port must be between 1 and 65535".into()));
                }
            }
        }
        
        if let Some(version) = config.get("protocol_version") {
            if let Some(ver_str) = version.as_str() {
                if !["v1", "v2"].contains(&ver_str) {
                    return Err(comsrv::utils::Error::Config("Invalid protocol version".into()));
                }
            }
        }
        
        Ok(())
    }
    
    async fn create_instance(
        &self,
        _channel_config: comsrv::core::config::types::channel::ChannelConfig,
    ) -> comsrv::utils::Result<Box<dyn comsrv::core::protocols::common::traits::ComBase>> {
        // 返回模拟实例
        unimplemented!("Mock plugin does not create real instances")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_metadata() {
        let plugin = MockProtocolPlugin {
            id: "mock_protocol".to_string(),
        };
        
        let metadata = plugin.metadata();
        assert_eq!(metadata.id, "mock_protocol");
        assert_eq!(metadata.name, "Mock Protocol");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.features.len(), 2);
        assert!(metadata.features.contains(&"telemetry".to_string()));
        assert!(metadata.features.contains(&"control".to_string()));
    }
    
    #[test]
    fn test_config_template() {
        let plugin = MockProtocolPlugin {
            id: "mock_protocol".to_string(),
        };
        
        let template = plugin.config_template();
        assert_eq!(template.len(), 3);
        
        // 测试host参数
        let host_param = &template[0];
        assert_eq!(host_param.name, "host");
        assert!(host_param.required);
        assert_eq!(host_param.default_value, Some(json!("127.0.0.1")));
        
        // 测试port参数
        let port_param = &template[1];
        assert_eq!(port_param.name, "port");
        assert!(port_param.required);
        assert_eq!(port_param.default_value, Some(json!(502)));
        
        // 测试validation规则
        let validation = port_param.validation.as_ref().unwrap();
        assert_eq!(validation.min, Some(1.0));
        assert_eq!(validation.max, Some(65535.0));
        
        // 测试protocol_version参数
        let version_param = &template[2];
        assert_eq!(version_param.name, "protocol_version");
        assert!(!version_param.required);
        
        let validation = version_param.validation.as_ref().unwrap();
        assert_eq!(validation.allowed_values, Some(vec!["v1".to_string(), "v2".to_string()]));
    }
    
    #[test]
    fn test_config_validation_success() {
        let plugin = MockProtocolPlugin {
            id: "mock_protocol".to_string(),
        };
        
        let mut config = HashMap::new();
        config.insert("host".to_string(), json!("192.168.1.100"));
        config.insert("port".to_string(), json!(502));
        config.insert("protocol_version".to_string(), json!("v2"));
        
        assert!(plugin.validate_config(&config).is_ok());
    }
    
    #[test]
    fn test_config_validation_missing_required() {
        let plugin = MockProtocolPlugin {
            id: "mock_protocol".to_string(),
        };
        
        let config = HashMap::new();
        let result = plugin.validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing required parameter"));
    }
    
    #[test]
    fn test_config_validation_invalid_port() {
        let plugin = MockProtocolPlugin {
            id: "mock_protocol".to_string(),
        };
        
        let mut config = HashMap::new();
        config.insert("host".to_string(), json!("192.168.1.100"));
        config.insert("port".to_string(), json!(70000)); // 超出范围
        
        let result = plugin.validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Port must be between"));
    }
    
    #[test]
    fn test_config_validation_invalid_version() {
        let plugin = MockProtocolPlugin {
            id: "mock_protocol".to_string(),
        };
        
        let mut config = HashMap::new();
        config.insert("host".to_string(), json!("192.168.1.100"));
        config.insert("port".to_string(), json!(502));
        config.insert("protocol_version".to_string(), json!("v3")); // 无效版本
        
        let result = plugin.validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid protocol version"));
    }
    
    #[test]
    fn test_generate_example_config() {
        let plugin = MockProtocolPlugin {
            id: "mock_protocol".to_string(),
        };
        
        let example_config = plugin.generate_example_config();
        
        assert_eq!(example_config.get("host"), Some(&json!("127.0.0.1")));
        assert_eq!(example_config.get("port"), Some(&json!(502)));
        assert_eq!(example_config.get("protocol_version"), Some(&json!("v1")));
    }
}

/// 测试插件宏的功能
#[cfg(test)]
mod macro_tests {
    use comsrv::protocol_plugin;
    
    // 使用宏定义测试插件
    protocol_plugin! {
        id: "test_protocol",
        name: "Test Protocol",
        version: "1.0.0",
        description: "Protocol defined using macro",
        author: "Test",
        license: "MIT",
        features: ["telemetry", "control"],
        config: [
            {
                name: "address",
                description: "Device address",
                param_type: "string",
                required: true,
                default: "0.0.0.0"
            },
            {
                name: "timeout",
                description: "Connection timeout in seconds",
                param_type: "int",
                required: false,
                default: 30,
                validation: {
                    min: 1,
                    max: 300
                }
            }
        ]
    }
    
    #[test]
    fn test_macro_metadata() {
        let metadata = PluginMetadataImpl::metadata();
        assert_eq!(metadata.id, "test_protocol");
        assert_eq!(metadata.name, "Test Protocol");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.features, vec!["telemetry", "control"]);
    }
    
    #[test]
    fn test_macro_config_template() {
        let template = PluginMetadataImpl::config_template();
        assert_eq!(template.len(), 2);
        
        let address_param = &template[0];
        assert_eq!(address_param.name, "address");
        assert!(address_param.required);
        assert_eq!(address_param.default_value, Some(serde_json::json!("0.0.0.0")));
        
        let timeout_param = &template[1];
        assert_eq!(timeout_param.name, "timeout");
        assert!(!timeout_param.required);
        assert_eq!(timeout_param.default_value, Some(serde_json::json!(30)));
        
        let validation = timeout_param.validation.as_ref().unwrap();
        assert_eq!(validation.min, Some(1.0));
        assert_eq!(validation.max, Some(300.0));
    }
}