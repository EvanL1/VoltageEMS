//! Configuration Template System
//!
//! This module provides a flexible configuration template system for protocol plugins,
//! including validation, generation, and documentation capabilities.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::utils::error::{ComSrvError as Error, Result};

/// Configuration schema for protocol plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    /// Schema version
    pub version: String,
    /// Protocol ID this schema applies to
    pub protocol_id: String,
    /// Schema sections
    pub sections: Vec<ConfigSection>,
}

/// Configuration section grouping related parameters
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
    /// Whether the parameter is required
    pub required: bool,
    /// Default value
    pub default: Option<Value>,
    /// Validation rules
    pub validation: Option<ParameterValidation>,
    /// Whether this parameter is advanced/expert-only
    pub advanced: bool,
    /// Dependencies on other parameters
    pub depends_on: Option<ParameterDependency>,
}

/// Parameter types
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
    Duration, // In seconds
    IpAddress,
    Port,
    FilePath,
    Secret, // For passwords, tokens, etc.
}

/// Enum value definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

/// Parameter validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterValidation {
    /// Regular expression pattern
    pub pattern: Option<String>,
    /// Minimum length (for strings/arrays)
    pub min_length: Option<usize>,
    /// Maximum length (for strings/arrays)
    pub max_length: Option<usize>,
    /// Custom validation function name
    pub custom_validator: Option<String>,
    /// Validation error message
    pub error_message: Option<String>,
}

/// Parameter dependency definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDependency {
    /// Parameter this depends on
    pub parameter: String,
    /// Condition for this parameter to be active
    pub condition: DependencyCondition,
}

/// Dependency condition types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DependencyCondition {
    Equals { value: Value },
    NotEquals { value: Value },
    In { values: Vec<Value> },
    GreaterThan { value: Value },
    LessThan { value: Value },
}

/// Configuration template builder
#[derive(Debug)]
pub struct ConfigTemplateBuilder {
    schema: ConfigSchema,
    current_section: Option<usize>,
}

impl ConfigTemplateBuilder {
    /// Create a new configuration template builder
    pub fn new(protocol_id: &str) -> Self {
        Self {
            schema: ConfigSchema {
                version: "1.0".to_string(),
                protocol_id: protocol_id.to_string(),
                sections: Vec::new(),
            },
            current_section: None,
        }
    }

    /// Add a new section
    pub fn add_section(mut self, name: &str, description: &str) -> Self {
        self.schema.sections.push(ConfigSection {
            name: name.to_string(),
            description: description.to_string(),
            parameters: Vec::new(),
        });
        self.current_section = Some(self.schema.sections.len() - 1);
        self
    }

    /// Add a parameter to the current section
    pub fn add_parameter(mut self, param: ConfigParameter) -> Self {
        if let Some(idx) = self.current_section {
            self.schema.sections[idx].parameters.push(param);
        }
        self
    }

    /// Build the configuration schema
    pub fn build(self) -> ConfigSchema {
        self.schema
    }
}

/// Configuration validator
#[derive(Debug)]
pub struct ConfigValidator {
    schema: ConfigSchema,
}

impl ConfigValidator {
    /// Create a new validator with the given schema
    pub fn new(schema: ConfigSchema) -> Self {
        Self { schema }
    }

    /// Validate a configuration against the schema
    pub fn validate(&self, config: &HashMap<String, Value>) -> Result<ValidationResult> {
        let mut result = ValidationResult::new();

        // Check all sections and parameters
        for section in &self.schema.sections {
            for param in &section.parameters {
                self.validate_parameter(param, config, &mut result)?;
            }
        }

        Ok(result)
    }

    /// Validate a single parameter
    fn validate_parameter(
        &self,
        param: &ConfigParameter,
        config: &HashMap<String, Value>,
        result: &mut ValidationResult,
    ) -> Result<()> {
        let value = config.get(&param.key);

        // Check required parameters
        if param.required && value.is_none() {
            result.add_error(ValidationError {
                parameter: param.key.clone(),
                message: format!("Required parameter '{}' is missing", param.key),
                severity: ErrorSeverity::Error,
            });
            return Ok(());
        }

        // Skip validation if not present and not required
        if value.is_none() {
            return Ok(());
        }

        let value = value.unwrap();

        // Check dependencies
        if let Some(dep) = &param.depends_on {
            if !self.check_dependency(dep, config) {
                result.add_warning(ValidationWarning {
                    parameter: param.key.clone(),
                    message: format!(
                        "Parameter '{}' depends on '{}' which doesn't meet the condition",
                        param.key, dep.parameter
                    ),
                });
                return Ok(());
            }
        }

        // Validate type
        self.validate_type(&param.key, value, &param.param_type, result)?;

        // Apply validation rules
        if let Some(validation) = &param.validation {
            self.apply_validation_rules(&param.key, value, validation, result)?;
        }

        Ok(())
    }

    /// Validate parameter type
    fn validate_type(
        &self,
        key: &str,
        value: &Value,
        param_type: &ParameterType,
        result: &mut ValidationResult,
    ) -> Result<()> {
        match param_type {
            ParameterType::String => {
                if !value.is_string() {
                    result.add_error(ValidationError {
                        parameter: key.to_string(),
                        message: format!("Parameter '{}' must be a string", key),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
            ParameterType::Integer { min, max } => {
                if let Some(n) = value.as_i64() {
                    if let Some(min_val) = min {
                        if n < *min_val {
                            result.add_error(ValidationError {
                                parameter: key.to_string(),
                                message: format!("Value {} is less than minimum {min_val}", n),
                                severity: ErrorSeverity::Error,
                            });
                        }
                    }
                    if let Some(max_val) = max {
                        if n > *max_val {
                            result.add_error(ValidationError {
                                parameter: key.to_string(),
                                message: format!("Value {} is greater than maximum {max_val}", n),
                                severity: ErrorSeverity::Error,
                            });
                        }
                    }
                } else {
                    result.add_error(ValidationError {
                        parameter: key.to_string(),
                        message: format!("Parameter '{}' must be an integer", key),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
            ParameterType::Port => {
                if let Some(n) = value.as_u64() {
                    if n == 0 || n > 65535 {
                        result.add_error(ValidationError {
                            parameter: key.to_string(),
                            message: format!("Port {} is not valid (must be 1-65535)", n),
                            severity: ErrorSeverity::Error,
                        });
                    }
                } else {
                    result.add_error(ValidationError {
                        parameter: key.to_string(),
                        message: format!("Parameter '{}' must be a valid port number", key),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
            ParameterType::IpAddress => {
                if let Some(s) = value.as_str() {
                    // Simple IP validation
                    let ip_regex = Regex::new(r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$").unwrap();
                    if !ip_regex.is_match(s) && s != "localhost" {
                        result.add_error(ValidationError {
                            parameter: key.to_string(),
                            message: format!("'{}' is not a valid IP address", s),
                            severity: ErrorSeverity::Error,
                        });
                    }
                }
            }
            // TODO: Implement other type validations
            _ => {}
        }

        Ok(())
    }

    /// Apply custom validation rules
    fn apply_validation_rules(
        &self,
        key: &str,
        value: &Value,
        validation: &ParameterValidation,
        result: &mut ValidationResult,
    ) -> Result<()> {
        // Pattern validation
        if let Some(pattern) = &validation.pattern {
            if let Some(s) = value.as_str() {
                let regex = Regex::new(pattern)
                    .map_err(|e| Error::ConfigError(format!("Invalid regex pattern: {e}")))?;
                if !regex.is_match(s) {
                    result.add_error(ValidationError {
                        parameter: key.to_string(),
                        message: validation.error_message.clone().unwrap_or_else(|| {
                            format!("Value '{}' doesn't match pattern '{}'", s, pattern)
                        }),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
        }

        // Length validation
        if let Some(s) = value.as_str() {
            if let Some(min_len) = validation.min_length {
                if s.len() < min_len {
                    result.add_error(ValidationError {
                        parameter: key.to_string(),
                        message: format!(
                            "Value length {} is less than minimum {}",
                            s.len(),
                            min_len
                        ),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
            if let Some(max_len) = validation.max_length {
                if s.len() > max_len {
                    result.add_error(ValidationError {
                        parameter: key.to_string(),
                        message: format!("Value length {} exceeds maximum {max_len}", s.len()),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
        }

        Ok(())
    }

    /// Check parameter dependency
    fn check_dependency(&self, dep: &ParameterDependency, config: &HashMap<String, Value>) -> bool {
        if let Some(dep_value) = config.get(&dep.parameter) {
            match &dep.condition {
                DependencyCondition::Equals { value } => dep_value == value,
                DependencyCondition::NotEquals { value } => dep_value != value,
                DependencyCondition::In { values } => values.contains(dep_value),
                _ => true, // TODO: Implement other conditions
            }
        } else {
            false
        }
    }
}

/// Validation result
#[derive(Debug, Default)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    /// Create a new validation result
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if validation passed (no errors)
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Add an error
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }
}

/// Validation error
#[derive(Debug)]
pub struct ValidationError {
    pub parameter: String,
    pub message: String,
    pub severity: ErrorSeverity,
}

/// Validation warning
#[derive(Debug)]
pub struct ValidationWarning {
    pub parameter: String,
    pub message: String,
}

/// Error severity levels
#[derive(Debug)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}

/// Configuration generator
#[derive(Debug)]
pub struct ConfigGenerator {
    schema: ConfigSchema,
}

impl ConfigGenerator {
    /// Create a new configuration generator
    pub fn new(schema: ConfigSchema) -> Self {
        Self { schema }
    }

    /// Generate a default configuration
    pub fn generate_default(&self) -> HashMap<String, Value> {
        let mut config = HashMap::new();

        for section in &self.schema.sections {
            for param in &section.parameters {
                if let Some(default) = &param.default {
                    config.insert(param.key.clone(), default.clone());
                } else if param.required {
                    // Generate sensible defaults for required fields
                    config.insert(
                        param.key.clone(),
                        self.generate_default_value(&param.param_type),
                    );
                }
            }
        }

        config
    }

    /// Generate a default value for a parameter type
    fn generate_default_value(&self, param_type: &ParameterType) -> Value {
        match param_type {
            ParameterType::String => Value::String("".to_string()),
            ParameterType::Integer { min, .. } => Value::Number(min.unwrap_or(0).into()),
            ParameterType::Float { min, .. } => Value::from(min.unwrap_or(0.0)),
            ParameterType::Boolean => Value::Bool(false),
            ParameterType::Port => Value::Number(8080.into()),
            ParameterType::IpAddress => Value::String("127.0.0.1".to_string()),
            ParameterType::Duration => Value::Number(60.into()),
            ParameterType::Array { .. } => Value::Array(vec![]),
            ParameterType::Object { .. } => Value::Object(serde_json::Map::new()),
            ParameterType::Enum { values } => {
                if let Some(first) = values.first() {
                    Value::String(first.value.clone())
                } else {
                    Value::Null
                }
            }
            _ => Value::Null,
        }
    }

    /// Generate example configuration with comments
    pub fn generate_example_yaml(&self) -> String {
        let mut yaml = String::new();
        yaml.push_str(&format!(
            "# Configuration for {} protocol\n",
            self.schema.protocol_id
        ));
        yaml.push_str(&format!("# Schema version: {}\n\n", self.schema.version));

        for section in &self.schema.sections {
            yaml.push_str(&format!("# {}\n", section.description));
            yaml.push_str(&format!("# {}\n", "=".repeat(section.description.len())));

            for param in &section.parameters {
                yaml.push_str(&format!("\n# {}\n", param.description));
                if param.required {
                    yaml.push_str("# (Required)\n");
                }

                // Add type information
                yaml.push_str(&format!("# Type: {:?}\n", param.param_type));

                // Add validation info if present
                if let Some(validation) = &param.validation {
                    if let Some(pattern) = &validation.pattern {
                        yaml.push_str(&format!("# Pattern: {}\n", pattern));
                    }
                }

                // Add the parameter with default or example value
                let value = if let Some(default) = &param.default {
                    default.clone()
                } else {
                    self.generate_default_value(&param.param_type)
                };

                yaml.push_str(&format!(
                    "{}: {}\n",
                    param.key,
                    serde_yaml::to_string(&value).unwrap().trim()
                ));
            }

            yaml.push('\n');
        }

        yaml
    }
}
