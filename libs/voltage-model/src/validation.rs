//! Validation Utilities
//!
//! Pure validation logic for model entities.
//! No database or IO dependencies.

use crate::error::{ModelError, Result};

/// Validate instance name format
///
/// Rules:
/// - Length: 1-64 characters
/// - Characters: alphanumeric, underscore (_), hyphen (-)
/// - Cannot start with a number
/// - Cannot contain spaces or special characters
///
/// # Arguments
/// * `name` - Instance name to validate
///
/// # Returns
/// * `Ok(())` - Name is valid
/// * `Err(ModelError)` - Validation error with description
///
/// # Examples
/// ```
/// use voltage_model::validate_instance_name;
///
/// assert!(validate_instance_name("pv_inverter_01").is_ok());
/// assert!(validate_instance_name("battery-system").is_ok());
/// assert!(validate_instance_name("_underscore_start").is_ok());
/// assert!(validate_instance_name("123_invalid").is_err());
/// assert!(validate_instance_name("bad name!").is_err());
/// assert!(validate_instance_name("").is_err());
/// ```
pub fn validate_instance_name(name: &str) -> Result<()> {
    // Length check
    if name.is_empty() {
        return Err(ModelError::InvalidInstanceName(
            "Instance name cannot be empty".to_string(),
        ));
    }
    if name.len() > 64 {
        return Err(ModelError::InvalidInstanceName(format!(
            "Instance name too long ({} characters). Maximum length is 64 characters.",
            name.len()
        )));
    }

    // Character validation: only alphanumeric, underscore, and hyphen allowed
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ModelError::InvalidInstanceName(format!(
            "Instance name can only contain letters (a-z, A-Z), numbers (0-9), underscores (_), and hyphens (-). Invalid name: '{}'",
            name
        )));
    }

    // Cannot start with a number (helps distinguish from IDs)
    if name.chars().next().is_some_and(|c| c.is_numeric()) {
        return Err(ModelError::InvalidInstanceName(
            "Instance name cannot start with a number. Please start with a letter or underscore."
                .to_string(),
        ));
    }

    Ok(())
}

/// Validate product name format
///
/// Rules:
/// - Length: 1-64 characters
/// - Characters: alphanumeric, underscore (_), hyphen (-)
/// - Cannot start with a number
/// - Cannot contain path traversal characters (/, \, ..)
///
/// # Arguments
/// * `name` - Product name to validate
///
/// # Returns
/// * `Ok(())` - Name is valid
/// * `Err(ModelError)` - Validation error with description
pub fn validate_product_name(name: &str) -> Result<()> {
    // Length check
    if name.is_empty() {
        return Err(ModelError::Validation(
            "Product name cannot be empty".to_string(),
        ));
    }
    if name.len() > 64 {
        return Err(ModelError::Validation(format!(
            "Product name too long ({} characters). Maximum length is 64 characters.",
            name.len()
        )));
    }

    // Path traversal prevention
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(ModelError::Validation(
            "Product name contains path traversal characters".to_string(),
        ));
    }

    // Character validation
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ModelError::Validation(format!(
            "Product name can only contain letters, numbers, underscores, and hyphens. Invalid name: '{}'",
            name
        )));
    }

    // Cannot start with a number
    if name.chars().next().is_some_and(|c| c.is_numeric()) {
        return Err(ModelError::Validation(
            "Product name cannot start with a number".to_string(),
        ));
    }

    Ok(())
}

/// Validate calculation ID format
///
/// Rules:
/// - Length: 1-128 characters
/// - Characters: alphanumeric, underscore (_), hyphen (-), dot (.)
/// - Cannot be empty
pub fn validate_calculation_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(ModelError::Validation(
            "Calculation ID cannot be empty".to_string(),
        ));
    }
    if id.len() > 128 {
        return Err(ModelError::Validation(format!(
            "Calculation ID too long ({} characters). Maximum length is 128 characters.",
            id.len()
        )));
    }

    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(ModelError::Validation(format!(
            "Calculation ID can only contain letters, numbers, underscores, hyphens, and dots. Invalid ID: '{}'",
            id
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_instance_names() {
        assert!(validate_instance_name("pv_inverter_01").is_ok());
        assert!(validate_instance_name("battery-system").is_ok());
        assert!(validate_instance_name("_underscore_start").is_ok());
        assert!(validate_instance_name("A").is_ok());
        assert!(validate_instance_name("test123").is_ok());
    }

    #[test]
    fn test_invalid_instance_names() {
        // Empty name
        assert!(validate_instance_name("").is_err());

        // Starts with number
        assert!(validate_instance_name("123_invalid").is_err());
        assert!(validate_instance_name("1test").is_err());

        // Invalid characters
        assert!(validate_instance_name("bad name!").is_err());
        assert!(validate_instance_name("test@email").is_err());
        assert!(validate_instance_name("test/path").is_err());

        // Too long (65 characters)
        let long_name = "a".repeat(65);
        assert!(validate_instance_name(&long_name).is_err());
    }

    #[test]
    fn test_valid_product_names() {
        assert!(validate_product_name("pv_inverter").is_ok());
        assert!(validate_product_name("battery-system").is_ok());
        assert!(validate_product_name("TestProduct").is_ok());
    }

    #[test]
    fn test_invalid_product_names() {
        // Path traversal
        assert!(validate_product_name("../etc/passwd").is_err());
        assert!(validate_product_name("test/subdir").is_err());
        assert!(validate_product_name("test\\subdir").is_err());

        // Empty
        assert!(validate_product_name("").is_err());

        // Starts with number
        assert!(validate_product_name("1product").is_err());
    }

    #[test]
    fn test_valid_calculation_ids() {
        assert!(validate_calculation_id("calc_001").is_ok());
        assert!(validate_calculation_id("power.balance").is_ok());
        assert!(validate_calculation_id("inst-1.soc").is_ok());
    }

    #[test]
    fn test_invalid_calculation_ids() {
        assert!(validate_calculation_id("").is_err());
        assert!(validate_calculation_id("invalid id").is_err());
    }
}
