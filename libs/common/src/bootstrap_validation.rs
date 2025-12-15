//! Configuration validation utilities
//!
//! Provides common functions for validating service configurations
//! and environment settings

use errors::{VoltageError, VoltageResult};
use std::collections::HashSet;
use std::path::Path;
use tracing::{debug, error, warn};

/// Validation result with detailed information
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether validation passed
    pub is_valid: bool,
    /// Warning messages (non-fatal issues)
    pub warnings: Vec<String>,
    /// Error messages (fatal issues)
    pub errors: Vec<String>,
    /// Informational messages
    pub info: Vec<String>,
}

impl ValidationResult {
    /// Create a new validation result
    pub fn new() -> Self {
        Self {
            is_valid: true,
            warnings: Vec::new(),
            errors: Vec::new(),
            info: Vec::new(),
        }
    }

    /// Add an error (marks validation as failed)
    pub fn add_error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
        self.is_valid = false;
    }

    /// Add a warning (doesn't fail validation)
    pub fn add_warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    /// Add an info message
    pub fn add_info(&mut self, message: impl Into<String>) {
        self.info.push(message.into());
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Merge another validation result into this one
    pub fn merge(&mut self, other: ValidationResult) {
        self.warnings.extend(other.warnings);
        self.errors.extend(other.errors);
        self.info.extend(other.info);
        self.is_valid = self.is_valid && other.is_valid;
    }

    /// Print validation summary
    pub fn print_summary(&self) {
        for msg in &self.info {
            debug!("{}", msg);
        }

        for msg in &self.warnings {
            warn!("{}", msg);
        }

        for msg in &self.errors {
            error!("{}", msg);
        }

        if self.is_valid {
            debug!("Config valid");
        } else {
            error!("Config invalid");
        }
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate port configuration
pub fn validate_port(port: u16, service_name: &str) -> ValidationResult {
    let mut result = ValidationResult::new();

    if port == 0 {
        result.add_error(format!("{}: Port cannot be 0", service_name));
    } else if port < 1024 {
        result.add_warning(format!(
            "{}: Port {} is in privileged range (< 1024)",
            service_name, port
        ));
    } else {
        result.add_info(format!("{}: Port {} is valid", service_name, port));
    }

    result
}

/// Validate file path exists and is accessible
pub fn validate_file_path(path: &str, description: &str) -> ValidationResult {
    let mut result = ValidationResult::new();
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        result.add_error(format!("{} does not exist: {}", description, path));
    } else if !path_obj.is_file() {
        result.add_error(format!("{} is not a file: {}", description, path));
    } else {
        // Check if readable
        if let Err(e) = std::fs::File::open(path) {
            result.add_error(format!("{} cannot be read: {} - {}", description, path, e));
        } else {
            result.add_info(format!("{} is accessible: {}", description, path));
        }
    }

    result
}

/// Validate directory path exists and is accessible
pub fn validate_directory_path(path: &str, description: &str) -> ValidationResult {
    let mut result = ValidationResult::new();
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        result.add_warning(format!(
            "{} does not exist: {} (will be created)",
            description, path
        ));
    } else if !path_obj.is_dir() {
        result.add_error(format!("{} is not a directory: {}", description, path));
    } else {
        result.add_info(format!("{} directory exists: {}", description, path));
    }

    result
}

/// Validate URL format
pub fn validate_url(url: &str, description: &str) -> ValidationResult {
    let mut result = ValidationResult::new();

    if url.is_empty() {
        result.add_error(format!("{} URL is empty", description));
        return result;
    }

    // Basic URL validation
    if !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.starts_with("redis://")
        && !url.starts_with("postgres://")
        && !url.starts_with("sqlite://")
    {
        result.add_warning(format!("{} URL has unusual scheme: {}", description, url));
    }

    // Check for common issues
    if url.contains(' ') {
        result.add_error(format!("{} URL contains spaces: {}", description, url));
    }

    if url.ends_with('/') && url.len() > 8 {
        result.add_warning(format!("{} URL has trailing slash: {}", description, url));
    }

    if result.is_valid {
        result.add_info(format!("{} URL format is valid: {}", description, url));
    }

    result
}

/// Validate environment variables
pub fn validate_environment_variables(required: &[&str]) -> ValidationResult {
    let mut result = ValidationResult::new();

    for var in required {
        match std::env::var(var) {
            Ok(value) if value.is_empty() => {
                result.add_warning(format!("Environment variable {} is set but empty", var));
            },
            Ok(_) => {
                result.add_info(format!("Environment variable {} is set", var));
            },
            Err(_) => {
                result.add_error(format!("Required environment variable {} is not set", var));
            },
        }
    }

    result
}

/// Validate service dependencies
pub fn validate_service_dependencies(services: &[(&str, u16)]) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (service, port) in services {
        // Try to check if port is open (simplified check)
        match std::net::TcpStream::connect(format!("127.0.0.1:{}", port)) {
            Ok(_) => {
                result.add_info(format!("Service {} is available on port {}", service, port));
            },
            Err(_) => {
                result.add_warning(format!(
                    "Service {} may not be running on port {}",
                    service, port
                ));
            },
        }
    }

    result
}

/// Validate configuration completeness
pub fn validate_config_completeness(
    config_fields: &[(&str, bool)], // (field_name, is_present)
) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (field, present) in config_fields {
        if *present {
            debug!("Field ok: {}", field);
        } else {
            result.add_error(format!(
                "Required configuration field '{}' is missing",
                field
            ));
        }
    }

    result
}

/// Validate no port conflicts
pub fn validate_port_conflicts(ports: &[(&str, u16)]) -> ValidationResult {
    let mut result = ValidationResult::new();
    let mut seen = HashSet::new();

    for (service, port) in ports {
        if !seen.insert(port) {
            result.add_error(format!(
                "Port conflict: {} wants to use port {} which is already assigned",
                service, port
            ));
        }
    }

    if result.is_valid {
        result.add_info("No port conflicts detected");
    }

    result
}

/// Run all basic validations
pub async fn run_basic_validations(
    service_name: &str,
    port: u16,
    db_path: Option<&str>,
    redis_url: Option<&str>,
) -> VoltageResult<ValidationResult> {
    let mut result = ValidationResult::new();

    debug!("Validating {}", service_name);

    // Validate port
    result.merge(validate_port(port, service_name));

    // Validate database path if provided
    if let Some(path) = db_path {
        result.merge(validate_file_path(path, "Database"));
    }

    // Validate Redis URL if provided
    if let Some(url) = redis_url {
        result.merge(validate_url(url, "Redis"));
    }

    // Check common directories
    result.merge(validate_directory_path("logs", "Logs"));
    result.merge(validate_directory_path("data", "Data"));

    result.print_summary();

    if result.is_valid {
        Ok(result)
    } else {
        Err(VoltageError::Configuration(
            "Configuration validation failed".to_string(),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::new();
        assert!(result.is_valid);

        result.add_warning("This is a warning");
        assert!(result.is_valid);
        assert!(result.has_warnings());

        result.add_error("This is an error");
        assert!(!result.is_valid);
        assert!(result.has_errors());
    }

    #[test]
    fn test_validate_port() {
        let result = validate_port(0, "test");
        assert!(!result.is_valid);

        let result = validate_port(80, "test");
        assert!(result.is_valid);
        assert!(result.has_warnings()); // Privileged port

        let result = validate_port(8080, "test");
        assert!(result.is_valid);
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_validate_url() {
        let result = validate_url("", "test");
        assert!(!result.is_valid);

        let result = validate_url("redis://localhost:6379", "test");
        assert!(result.is_valid);

        let result = validate_url("http://example com", "test");
        assert!(!result.is_valid); // Contains space
    }

    #[test]
    fn test_merge_results() {
        let mut result1 = ValidationResult::new();
        result1.add_warning("Warning 1");

        let mut result2 = ValidationResult::new();
        result2.add_error("Error 1");

        result1.merge(result2);
        assert!(!result1.is_valid);
        assert_eq!(result1.warnings.len(), 1);
        assert_eq!(result1.errors.len(), 1);
    }
}
