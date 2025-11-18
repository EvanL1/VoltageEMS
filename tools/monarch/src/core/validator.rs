//! Configuration validation module
//!
//! This module provides validation functionality for service configurations
//! using the shared voltage-config validation framework.

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use voltage_config::{
    ComsrvValidator, ConfigValidator as VoltageConfigValidator, ModsrvValidator, RulesrvValidator,
    ValidationLevel, ValidationResult as VoltageResult,
};

/// Result of a validation operation (compatibility wrapper)
#[derive(Debug, Default)]
pub struct ValidationResult {
    /// Whether the configuration is valid
    pub valid: bool,
    /// List of validation errors
    pub errors: Vec<String>,
    /// List of validation warnings
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Check if validation passed (no errors)
    pub fn is_valid(&self) -> bool {
        self.valid && self.errors.is_empty()
    }

    /// Convert from voltage-config ValidationResult
    fn from_voltage_result(result: VoltageResult) -> Self {
        Self {
            valid: result.is_valid,
            errors: result.errors,
            warnings: result.warnings,
        }
    }
}

/// Configuration validator using shared validation framework
pub struct ConfigValidator {
    config_path: PathBuf,
    validation_level: ValidationLevel,
}

impl ConfigValidator {
    /// Create a new validator
    pub fn new(config_path: impl AsRef<Path>) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
            // For Monarch, validate up to Business level (not Runtime)
            validation_level: ValidationLevel::Business,
        }
    }

    /// Validate configuration for a specific service
    pub async fn validate_service(&self, service: &str) -> Result<ValidationResult> {
        info!("Validating configuration for service: {}", service);

        // Special handling for global configuration (no subdirectory)
        if service == "global" {
            return self.validate_global().await;
        }

        // Check if service configuration exists
        let service_config_path = self.config_path.join(service);
        if !service_config_path.exists() {
            return Ok(ValidationResult {
                valid: false,
                errors: vec![format!(
                    "Service configuration directory not found: {:?}",
                    service_config_path
                )],
                warnings: vec![],
            });
        }

        // Use shared validation framework
        let result = match service {
            "comsrv" => self.validate_comsrv().await?,
            "modsrv" => self.validate_modsrv().await?,
            "rulesrv" => self.validate_rulesrv().await?,
            _ => {
                return Ok(ValidationResult {
                    valid: false,
                    errors: vec![format!("Unknown service: {}", service)],
                    warnings: vec![],
                })
            },
        };

        if result.valid {
            debug!("Validation passed for service: {}", service);
        } else {
            warn!("Validation failed for service: {}", service);
            for error in &result.errors {
                warn!("  Error: {}", error);
            }
        }

        Ok(result)
    }

    /// Validate comsrv configuration
    async fn validate_comsrv(&self) -> Result<ValidationResult> {
        let yaml_path = self.config_path.join("comsrv").join("comsrv.yaml");

        // Check if file exists
        if !yaml_path.exists() {
            return Ok(ValidationResult {
                valid: false,
                errors: vec![format!("Missing required file: {:?}", yaml_path)],
                warnings: vec![],
            });
        }

        // Load and validate using shared framework
        // Note: Errors from from_file already include file path + line number + reason
        let validator = ComsrvValidator::from_file(&yaml_path)?;
        let result = validator.validate(self.validation_level)?;

        Ok(ValidationResult::from_voltage_result(result))
    }

    /// Validate modsrv configuration
    async fn validate_modsrv(&self) -> Result<ValidationResult> {
        let yaml_path = self.config_path.join("modsrv").join("modsrv.yaml");

        // Check if file exists
        if !yaml_path.exists() {
            return Ok(ValidationResult {
                valid: false,
                errors: vec![format!("Missing required file: {:?}", yaml_path)],
                warnings: vec![],
            });
        }

        // Load and validate using shared framework
        // Note: Errors from from_file already include file path + line number + reason
        let validator = ModsrvValidator::from_file(&yaml_path)?;
        let result = validator.validate(self.validation_level)?;

        Ok(ValidationResult::from_voltage_result(result))
    }

    /// Validate rulesrv configuration
    async fn validate_rulesrv(&self) -> Result<ValidationResult> {
        let yaml_path = self.config_path.join("rulesrv").join("rulesrv.yaml");

        // Check if file exists
        if !yaml_path.exists() {
            return Ok(ValidationResult {
                valid: false,
                errors: vec![format!("Missing required file: {:?}", yaml_path)],
                warnings: vec![],
            });
        }

        // Load and validate using shared framework
        // Note: Errors from from_file already include file path + line number + reason
        let validator = RulesrvValidator::from_file(&yaml_path)?;
        let result = validator.validate(self.validation_level)?;

        Ok(ValidationResult::from_voltage_result(result))
    }

    /// Validate global configuration
    async fn validate_global(&self) -> Result<ValidationResult> {
        let yaml_path = self.config_path.join("global.yaml");

        // Check if file exists
        if !yaml_path.exists() {
            return Ok(ValidationResult {
                valid: false,
                errors: vec![format!(
                    "Missing global configuration file: {:?}",
                    yaml_path
                )],
                warnings: vec![],
            });
        }

        // Load YAML and perform basic validation
        let yaml_content = std::fs::read_to_string(&yaml_path)?;
        match serde_yaml::from_str::<serde_yaml::Value>(&yaml_content) {
            Ok(_) => {
                // Global config is valid YAML
                Ok(ValidationResult {
                    valid: true,
                    errors: vec![],
                    warnings: vec![],
                })
            },
            Err(e) => {
                // YAML parsing failed
                Ok(ValidationResult {
                    valid: false,
                    errors: vec![format!("Invalid YAML in {:?}: {}", yaml_path, e)],
                    warnings: vec![],
                })
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_validator_with_shared_framework() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path();
        let comsrv_dir = config_path.join("comsrv");
        fs::create_dir_all(&comsrv_dir).unwrap();

        // Create test config
        let config_content = r#"
service:
  name: comsrv
  description: Test Service
  port: 6000
channels: []
"#;
        fs::write(comsrv_dir.join("comsrv.yaml"), config_content).unwrap();

        // Test validation
        let validator = ConfigValidator::new(config_path);
        let result = validator.validate_service("comsrv").await.unwrap();

        // Should have error about no channels
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("at least one channel")));
    }
}
