//! 配置验证单元测试
//!
//! 测试配置参数的验证逻辑，包括类型检查、范围验证、模式匹配等

use comsrv::core::plugins::{ConfigTemplate, ValidationRule};
use serde_json::{json, Value};
use std::collections::HashMap;

/// 配置验证器
struct ConfigValidator;

impl ConfigValidator {
    /// 验证单个参数
    fn validate_parameter(template: &ConfigTemplate, value: Option<&Value>) -> Result<(), String> {
        // 检查必需参数
        if template.required && value.is_none() {
            return Err(format!("Required parameter '{}' is missing", template.name));
        }
        
        if let Some(val) = value {
            // 类型检查
            match template.param_type.as_str() {
                "string" => {
                    if !val.is_string() {
                        return Err(format!("Parameter '{}' must be a string", template.name));
                    }
                }
                "int" | "integer" => {
                    if !val.is_i64() && !val.is_u64() {
                        return Err(format!("Parameter '{}' must be an integer", template.name));
                    }
                }
                "float" | "number" => {
                    if !val.is_number() {
                        return Err(format!("Parameter '{}' must be a number", template.name));
                    }
                }
                "bool" | "boolean" => {
                    if !val.is_boolean() {
                        return Err(format!("Parameter '{}' must be a boolean", template.name));
                    }
                }
                "array" => {
                    if !val.is_array() {
                        return Err(format!("Parameter '{}' must be an array", template.name));
                    }
                }
                "object" => {
                    if !val.is_object() {
                        return Err(format!("Parameter '{}' must be an object", template.name));
                    }
                }
                _ => {}
            }
            
            // 验证规则
            if let Some(rule) = &template.validation {
                Self::validate_rule(template.name.as_str(), val, rule)?;
            }
        }
        
        Ok(())
    }
    
    /// 验证规则
    fn validate_rule(name: &str, value: &Value, rule: &ValidationRule) -> Result<(), String> {
        // 数值范围验证
        if let Some(num) = value.as_f64() {
            if let Some(min) = rule.min {
                if num < min {
                    return Err(format!("Parameter '{}' value {} is less than minimum {}", name, num, min));
                }
            }
            if let Some(max) = rule.max {
                if num > max {
                    return Err(format!("Parameter '{}' value {} is greater than maximum {}", name, num, max));
                }
            }
        }
        
        // 字符串模式验证
        if let Some(pattern) = &rule.pattern {
            if let Some(str_val) = value.as_str() {
                let regex = regex::Regex::new(pattern)
                    .map_err(|_| format!("Invalid regex pattern for parameter '{}'", name))?;
                if !regex.is_match(str_val) {
                    return Err(format!("Parameter '{}' value '{}' does not match pattern '{}'", name, str_val, pattern));
                }
            }
        }
        
        // 枚举值验证
        if let Some(allowed) = &rule.allowed_values {
            if let Some(str_val) = value.as_str() {
                if !allowed.contains(&str_val.to_string()) {
                    return Err(format!("Parameter '{}' value '{}' is not in allowed values: {:?}", name, str_val, allowed));
                }
            }
        }
        
        Ok(())
    }
    
    /// 验证完整配置
    fn validate_config(templates: &[ConfigTemplate], config: &HashMap<String, Value>) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        for template in templates {
            let value = config.get(&template.name);
            if let Err(e) = Self::validate_parameter(template, value) {
                errors.push(e);
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_required_parameter_validation() {
        let template = ConfigTemplate {
            name: "host".to_string(),
            description: "Host address".to_string(),
            param_type: "string".to_string(),
            required: true,
            default_value: None,
            validation: None,
        };
        
        // 缺少必需参数
        assert!(ConfigValidator::validate_parameter(&template, None).is_err());
        
        // 提供必需参数
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("127.0.0.1"))).is_ok());
    }
    
    #[test]
    fn test_type_validation() {
        // 字符串类型
        let string_template = ConfigTemplate {
            name: "name".to_string(),
            description: "Name".to_string(),
            param_type: "string".to_string(),
            required: true,
            default_value: None,
            validation: None,
        };
        
        assert!(ConfigValidator::validate_parameter(&string_template, Some(&json!("test"))).is_ok());
        assert!(ConfigValidator::validate_parameter(&string_template, Some(&json!(123))).is_err());
        
        // 整数类型
        let int_template = ConfigTemplate {
            name: "port".to_string(),
            description: "Port".to_string(),
            param_type: "int".to_string(),
            required: true,
            default_value: None,
            validation: None,
        };
        
        assert!(ConfigValidator::validate_parameter(&int_template, Some(&json!(8080))).is_ok());
        assert!(ConfigValidator::validate_parameter(&int_template, Some(&json!("8080"))).is_err());
        
        // 布尔类型
        let bool_template = ConfigTemplate {
            name: "enabled".to_string(),
            description: "Enabled".to_string(),
            param_type: "bool".to_string(),
            required: true,
            default_value: None,
            validation: None,
        };
        
        assert!(ConfigValidator::validate_parameter(&bool_template, Some(&json!(true))).is_ok());
        assert!(ConfigValidator::validate_parameter(&bool_template, Some(&json!("true"))).is_err());
    }
    
    #[test]
    fn test_range_validation() {
        let template = ConfigTemplate {
            name: "timeout".to_string(),
            description: "Timeout in seconds".to_string(),
            param_type: "int".to_string(),
            required: true,
            default_value: None,
            validation: Some(ValidationRule {
                min: Some(1.0),
                max: Some(300.0),
                pattern: None,
                allowed_values: None,
            }),
        };
        
        // 范围内
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!(60))).is_ok());
        
        // 小于最小值
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!(0))).is_err());
        
        // 大于最大值
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!(400))).is_err());
    }
    
    #[test]
    fn test_pattern_validation() {
        let template = ConfigTemplate {
            name: "ip_address".to_string(),
            description: "IP Address".to_string(),
            param_type: "string".to_string(),
            required: true,
            default_value: None,
            validation: Some(ValidationRule {
                min: None,
                max: None,
                pattern: Some(r"^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$".to_string()),
                allowed_values: None,
            }),
        };
        
        // 有效IP
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("192.168.1.1"))).is_ok());
        
        // 无效IP
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("192.168.1"))).is_err());
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("not-an-ip"))).is_err());
    }
    
    #[test]
    fn test_allowed_values_validation() {
        let template = ConfigTemplate {
            name: "protocol_version".to_string(),
            description: "Protocol version".to_string(),
            param_type: "string".to_string(),
            required: true,
            default_value: None,
            validation: Some(ValidationRule {
                min: None,
                max: None,
                pattern: None,
                allowed_values: Some(vec!["v1".to_string(), "v2".to_string(), "v3".to_string()]),
            }),
        };
        
        // 允许的值
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("v1"))).is_ok());
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("v2"))).is_ok());
        
        // 不允许的值
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("v4"))).is_err());
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("invalid"))).is_err());
    }
    
    #[test]
    fn test_complete_config_validation() {
        let templates = vec![
            ConfigTemplate {
                name: "host".to_string(),
                description: "Host".to_string(),
                param_type: "string".to_string(),
                required: true,
                default_value: None,
                validation: None,
            },
            ConfigTemplate {
                name: "port".to_string(),
                description: "Port".to_string(),
                param_type: "int".to_string(),
                required: true,
                default_value: None,
                validation: Some(ValidationRule {
                    min: Some(1.0),
                    max: Some(65535.0),
                    pattern: None,
                    allowed_values: None,
                }),
            },
            ConfigTemplate {
                name: "timeout".to_string(),
                description: "Timeout".to_string(),
                param_type: "int".to_string(),
                required: false,
                default_value: Some(json!(30)),
                validation: None,
            },
        ];
        
        // 有效配置
        let mut valid_config = HashMap::new();
        valid_config.insert("host".to_string(), json!("192.168.1.1"));
        valid_config.insert("port".to_string(), json!(502));
        
        assert!(ConfigValidator::validate_config(&templates, &valid_config).is_ok());
        
        // 无效配置（缺少必需参数）
        let mut invalid_config = HashMap::new();
        invalid_config.insert("host".to_string(), json!("192.168.1.1"));
        
        let result = ConfigValidator::validate_config(&templates, &invalid_config);
        assert!(result.is_err());
        if let Err(errors) = result {
            assert_eq!(errors.len(), 1);
            assert!(errors[0].contains("port"));
        }
        
        // 无效配置（端口超出范围）
        let mut invalid_config2 = HashMap::new();
        invalid_config2.insert("host".to_string(), json!("192.168.1.1"));
        invalid_config2.insert("port".to_string(), json!(70000));
        
        let result = ConfigValidator::validate_config(&templates, &invalid_config2);
        assert!(result.is_err());
        if let Err(errors) = result {
            assert_eq!(errors.len(), 1);
            assert!(errors[0].contains("maximum"));
        }
    }
    
    #[test]
    fn test_complex_object_validation() {
        let template = ConfigTemplate {
            name: "advanced_settings".to_string(),
            description: "Advanced settings".to_string(),
            param_type: "object".to_string(),
            required: false,
            default_value: None,
            validation: None,
        };
        
        // 有效对象
        let obj = json!({
            "retry_count": 3,
            "retry_delay": 1000,
            "buffer_size": 4096
        });
        assert!(ConfigValidator::validate_parameter(&template, Some(&obj)).is_ok());
        
        // 非对象类型
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!("not an object"))).is_err());
    }
    
    #[test]
    fn test_array_validation() {
        let template = ConfigTemplate {
            name: "slave_ids".to_string(),
            description: "Slave IDs".to_string(),
            param_type: "array".to_string(),
            required: false,
            default_value: None,
            validation: None,
        };
        
        // 有效数组
        let arr = json!([1, 2, 3, 4, 5]);
        assert!(ConfigValidator::validate_parameter(&template, Some(&arr)).is_ok());
        
        // 空数组
        let empty_arr = json!([]);
        assert!(ConfigValidator::validate_parameter(&template, Some(&empty_arr)).is_ok());
        
        // 非数组类型
        assert!(ConfigValidator::validate_parameter(&template, Some(&json!(123))).is_err());
    }
}