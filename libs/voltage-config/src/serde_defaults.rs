//! Serde default value functions and custom deserializers for configuration structs
//!
//! This module provides centralized default value functions used by serde's
//! `#[serde(default = "...")]` attribute across voltage-config modules.
//!
//! By centralizing these functions, we avoid code duplication and ensure
//! consistent default values throughout the codebase.

use serde::de::{self, Deserializer};
use serde::Deserialize;

// ============================================================================
// Default Value Functions
// ============================================================================

/// Default value: true
///
/// Used for boolean fields that should default to enabled/true.
pub fn bool_true() -> bool {
    true
}

/// Default value: false
///
/// Used for boolean fields that should default to disabled/false.
pub fn bool_false() -> bool {
    false
}

/// Default page size for pagination: 20
///
/// Used for API pagination parameters.
pub fn page_size() -> usize {
    20
}

// ============================================================================
// Custom Deserializers
// ============================================================================

/// Custom deserializer for boolean fields that supports multiple input formats
///
/// Supports native JSON booleans, integers, and string values:
/// - JSON boolean: true, false
/// - JSON integer: 0 (false), 1 (true)
/// - CSV string: "1"/"0", "true"/"false", "yes"/"no" (case-insensitive)
pub fn deserialize_bool_flexible<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrStringOrInt {
        Bool(bool),
        Int(i64),
        String(String),
    }

    match BoolOrStringOrInt::deserialize(deserializer)? {
        BoolOrStringOrInt::Bool(b) => Ok(b),
        BoolOrStringOrInt::Int(i) => match i {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(de::Error::custom(format!(
                "Invalid integer value {}, expected 0 or 1",
                i
            ))),
        },
        BoolOrStringOrInt::String(s) => match s.to_lowercase().trim() {
            "1" | "true" | "yes" => Ok(true),
            "0" | "false" | "no" | "" => Ok(false),
            other => Err(de::Error::custom(format!(
                "Invalid boolean value '{}', expected: 1/0, true/false, yes/no, or boolean",
                other
            ))),
        },
    }
}

/// Custom deserializer for u8 fields that treats empty strings as 0
///
/// Allows CSV files to have empty bit_position values which default to 0
/// Supports: numeric strings or empty string
pub fn deserialize_u8_default_zero<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        Ok(0)
    } else {
        trimmed.parse::<u8>().map_err(de::Error::custom)
    }
}
