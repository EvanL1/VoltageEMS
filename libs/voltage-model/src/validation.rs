//! Validation Utilities
//!
//! Pure validation logic for model entities.
//! No database or IO dependencies.
//!
//! ## Numeric Value Validation
//!
//! `ValidationConfig` provides configurable validation for numeric values:
//! - Reject NaN and Infinity (default enabled)
//! - Optional absolute value limits
//! - Zero-overhead when disabled

use crate::error::{ModelError, Result};

// ============================================================================
// Numeric Value Validation
// ============================================================================

/// Configuration for numeric value validation
///
/// Provides safe defaults that reject obviously invalid values (NaN, Infinity)
/// while allowing flexibility for specific use cases.
///
/// ## Default Configuration
///
/// ```rust
/// use voltage_model::validation::ValidationConfig;
///
/// let config = ValidationConfig::default();
/// assert!(config.reject_nan);
/// assert!(config.reject_infinity);
/// ```
///
/// ## Custom Configuration
///
/// ```rust
/// use voltage_model::validation::ValidationConfig;
///
/// let config = ValidationConfig {
///     reject_nan: true,
///     reject_infinity: true,
///     max_abs_value: Some(1e15),  // Limit to reasonable range
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ValidationConfig {
    /// Reject NaN values (default: true)
    pub reject_nan: bool,
    /// Reject infinite values (default: true)
    pub reject_infinity: bool,
    /// Maximum absolute value (None = no limit)
    pub max_abs_value: Option<f64>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            reject_nan: true,
            reject_infinity: true,
            max_abs_value: None, // No default limit - let users configure if needed
        }
    }
}

impl ValidationConfig {
    /// Create a strict validation config with range limits
    ///
    /// Useful for industrial control values where extreme values
    /// indicate sensor errors or configuration mistakes.
    pub fn strict() -> Self {
        Self {
            reject_nan: true,
            reject_infinity: true,
            max_abs_value: Some(1e15), // Reasonable limit for most use cases
        }
    }

    /// Create a permissive config that only rejects NaN/Infinity
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Create a config that allows all values (including NaN/Infinity)
    ///
    /// Use with caution - only for specific cases where IEEE 754 special
    /// values have defined semantics in the application.
    pub fn allow_all() -> Self {
        Self {
            reject_nan: false,
            reject_infinity: false,
            max_abs_value: None,
        }
    }
}

/// Error type for value validation
#[derive(Debug, Clone, PartialEq)]
pub enum ValueValidationError {
    /// Value is NaN (Not a Number)
    NaN,
    /// Value is infinite (+∞ or -∞)
    Infinity,
    /// Value exceeds maximum absolute value
    OutOfRange { value: f64, max: f64 },
}

impl std::fmt::Display for ValueValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NaN => write!(f, "Value is NaN (Not a Number)"),
            Self::Infinity => write!(f, "Value is infinite"),
            Self::OutOfRange { value, max } => {
                write!(f, "Value {} exceeds max absolute value {}", value, max)
            },
        }
    }
}

impl std::error::Error for ValueValidationError {}

/// Validate a numeric value according to the given configuration
///
/// Returns the value unchanged if valid, or an error describing the problem.
///
/// ## Examples
///
/// ```rust
/// use voltage_model::validation::{validate_value, ValidationConfig, ValueValidationError};
///
/// let config = ValidationConfig::default();
///
/// // Valid values pass through
/// assert_eq!(validate_value(42.0, &config), Ok(42.0));
///
/// // NaN is rejected
/// assert!(matches!(validate_value(f64::NAN, &config), Err(ValueValidationError::NaN)));
///
/// // Infinity is rejected
/// assert!(matches!(validate_value(f64::INFINITY, &config), Err(ValueValidationError::Infinity)));
/// ```
#[inline]
pub fn validate_value(
    value: f64,
    config: &ValidationConfig,
) -> std::result::Result<f64, ValueValidationError> {
    if config.reject_nan && value.is_nan() {
        return Err(ValueValidationError::NaN);
    }
    if config.reject_infinity && value.is_infinite() {
        return Err(ValueValidationError::Infinity);
    }
    if let Some(max) = config.max_abs_value {
        if value.abs() > max {
            return Err(ValueValidationError::OutOfRange { value, max });
        }
    }
    Ok(value)
}

/// Check if a value is valid according to the given configuration
///
/// Convenience function that returns a boolean instead of Result.
#[inline]
pub fn is_value_valid(value: f64, config: &ValidationConfig) -> bool {
    validate_value(value, config).is_ok()
}

/// Sanitize a value by replacing invalid values with a default
///
/// If the value is invalid (NaN, Infinity, or out of range),
/// returns the `default` value instead.
///
/// ## Example
///
/// ```rust
/// use voltage_model::validation::{sanitize_value, ValidationConfig};
///
/// let config = ValidationConfig::default();
///
/// assert_eq!(sanitize_value(42.0, 0.0, &config), 42.0);
/// assert_eq!(sanitize_value(f64::NAN, 0.0, &config), 0.0);
/// ```
#[inline]
pub fn sanitize_value(value: f64, default: f64, config: &ValidationConfig) -> f64 {
    validate_value(value, config).unwrap_or(default)
}

// ============================================================================
// Name Validation (existing functionality)
// ============================================================================

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

    // ========== Numeric Value Validation Tests ==========

    #[test]
    fn test_validate_value_normal() {
        let config = ValidationConfig::default();

        // Normal values pass
        assert!(validate_value(0.0, &config).is_ok());
        assert!(validate_value(42.0, &config).is_ok());
        assert!(validate_value(-100.0, &config).is_ok());
        assert!(validate_value(1e10, &config).is_ok());
    }

    #[test]
    fn test_validate_value_nan() {
        let config = ValidationConfig::default();

        // NaN is rejected by default
        assert!(matches!(
            validate_value(f64::NAN, &config),
            Err(ValueValidationError::NaN)
        ));

        // NaN can be allowed
        let permissive = ValidationConfig::allow_all();
        assert!(validate_value(f64::NAN, &permissive).is_ok());
    }

    #[test]
    fn test_validate_value_infinity() {
        let config = ValidationConfig::default();

        // Infinity is rejected by default
        assert!(matches!(
            validate_value(f64::INFINITY, &config),
            Err(ValueValidationError::Infinity)
        ));
        assert!(matches!(
            validate_value(f64::NEG_INFINITY, &config),
            Err(ValueValidationError::Infinity)
        ));

        // Infinity can be allowed
        let permissive = ValidationConfig::allow_all();
        assert!(validate_value(f64::INFINITY, &permissive).is_ok());
    }

    #[test]
    fn test_validate_value_range() {
        let config = ValidationConfig::strict();

        // Within range
        assert!(validate_value(1e14, &config).is_ok());

        // Out of range
        assert!(matches!(
            validate_value(1e16, &config),
            Err(ValueValidationError::OutOfRange { .. })
        ));
        assert!(matches!(
            validate_value(-1e16, &config),
            Err(ValueValidationError::OutOfRange { .. })
        ));
    }

    #[test]
    fn test_sanitize_value() {
        let config = ValidationConfig::default();

        // Valid values pass through
        assert_eq!(sanitize_value(42.0, 0.0, &config), 42.0);

        // Invalid values get replaced with default
        assert_eq!(sanitize_value(f64::NAN, 0.0, &config), 0.0);
        assert_eq!(sanitize_value(f64::INFINITY, -1.0, &config), -1.0);
    }

    #[test]
    fn test_is_value_valid() {
        let config = ValidationConfig::default();

        assert!(is_value_valid(42.0, &config));
        assert!(!is_value_valid(f64::NAN, &config));
        assert!(!is_value_valid(f64::INFINITY, &config));
    }

    #[test]
    fn test_validation_config_presets() {
        // Default: reject NaN/Infinity, no range limit
        let default = ValidationConfig::default();
        assert!(default.reject_nan);
        assert!(default.reject_infinity);
        assert!(default.max_abs_value.is_none());

        // Strict: reject NaN/Infinity, with range limit
        let strict = ValidationConfig::strict();
        assert!(strict.reject_nan);
        assert!(strict.reject_infinity);
        assert!(strict.max_abs_value.is_some());

        // Allow all: permit everything
        let allow_all = ValidationConfig::allow_all();
        assert!(!allow_all.reject_nan);
        assert!(!allow_all.reject_infinity);
        assert!(allow_all.max_abs_value.is_none());
    }
}
