//! CSV Validation Module
//!
//! Provides automatic CSV header validation by comparing actual CSV files
//! against expected field names from Rust struct definitions.

use std::collections::HashSet;
use std::path::Path;

use crate::common::{ValidationLevel, ValidationResult};

/// Trait for types that can be deserialized from CSV files
///
/// Implementing this trait provides automatic CSV header validation
/// by defining the expected field names for CSV deserialization.
pub trait CsvFields {
    /// Returns the expected CSV header field names in order
    fn field_names() -> Vec<String>;

    /// Returns the required fields (cannot be empty/null)
    fn required_fields() -> Vec<String> {
        // By default, all fields are required
        Self::field_names()
    }

    /// Returns optional fields (can be empty/null)
    fn optional_fields() -> Vec<String> {
        vec![]
    }
}

/// CSV Header Validator
pub struct CsvHeaderValidator;

impl CsvHeaderValidator {
    /// Validate CSV file header against expected fields
    ///
    /// # Arguments
    /// * `csv_path` - Path to the CSV file to validate
    ///
    /// # Returns
    /// * `Ok(ValidationResult)` with validation status and detailed errors/warnings
    pub fn validate_csv_header<T>(csv_path: &Path) -> anyhow::Result<ValidationResult>
    where
        T: CsvFields,
    {
        // Read CSV file
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(csv_path)?;

        // Get actual headers from CSV
        let headers = reader.headers()?;
        let actual_headers: Vec<String> = headers.iter().map(|s| s.to_string()).collect();

        // Get expected headers from trait
        let expected_headers = T::field_names();

        // Validate
        Self::validate_headers(&actual_headers, &expected_headers, csv_path)
    }

    /// Validate headers with detailed error reporting
    fn validate_headers(
        actual: &[String],
        expected: &[String],
        csv_path: &Path,
    ) -> anyhow::Result<ValidationResult> {
        let actual_set: HashSet<_> = actual.iter().collect();
        let expected_set: HashSet<_> = expected.iter().collect();

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check for missing required fields
        let missing: Vec<_> = expected_set.difference(&actual_set).collect();
        if !missing.is_empty() {
            let missing_str = missing
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            errors.push(format!(
                "Missing required fields in {}: [{}]",
                csv_path.display(),
                missing_str
            ));
        }

        // Check for extra fields (warnings only)
        let extra: Vec<_> = actual_set.difference(&expected_set).collect();
        if !extra.is_empty() {
            let extra_str = extra
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            warnings.push(format!(
                "Extra fields found in {} (will be ignored): [{}]",
                csv_path.display(),
                extra_str
            ));
        }

        // Check field order (warning only - CSV allows any order)
        if actual != expected && missing.is_empty() && extra.is_empty() {
            warnings.push(format!(
                "Field order in {} differs from expected (this is OK, just informational)",
                csv_path.display()
            ));
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            level: ValidationLevel::Schema,
            errors,
            warnings,
        })
    }

    /// Validate multiple CSV files at once
    ///
    /// Returns aggregated validation result
    pub fn validate_multiple<T>(csv_paths: &[&Path]) -> anyhow::Result<ValidationResult>
    where
        T: CsvFields,
    {
        let mut aggregated = ValidationResult {
            is_valid: true,
            level: ValidationLevel::Schema,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        for path in csv_paths {
            let result = Self::validate_csv_header::<T>(path)?;
            aggregated.errors.extend(result.errors);
            aggregated.warnings.extend(result.warnings);

            if !result.is_valid {
                aggregated.is_valid = false;
            }
        }

        Ok(aggregated)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    struct TestPoint;

    impl CsvFields for TestPoint {
        fn field_names() -> Vec<String> {
            vec![
                "point_id".to_string(),
                "signal_name".to_string(),
                "unit".to_string(),
            ]
        }
    }

    #[test]
    fn test_header_validation_exact_match() {
        let actual = vec![
            "point_id".to_string(),
            "signal_name".to_string(),
            "unit".to_string(),
        ];
        let expected = TestPoint::field_names();

        let result =
            CsvHeaderValidator::validate_headers(&actual, &expected, Path::new("test.csv"))
                .unwrap();

        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_header_validation_missing_field() {
        let actual = vec!["point_id".to_string(), "signal_name".to_string()];
        let expected = TestPoint::field_names();

        let result =
            CsvHeaderValidator::validate_headers(&actual, &expected, Path::new("test.csv"))
                .unwrap();

        assert!(!result.is_valid);
        assert!(result.errors.len() == 1);
        assert!(result.errors[0].contains("Missing required fields"));
        assert!(result.errors[0].contains("unit"));
    }

    #[test]
    fn test_header_validation_extra_field() {
        let actual = vec![
            "point_id".to_string(),
            "signal_name".to_string(),
            "unit".to_string(),
            "extra_field".to_string(),
        ];
        let expected = TestPoint::field_names();

        let result =
            CsvHeaderValidator::validate_headers(&actual, &expected, Path::new("test.csv"))
                .unwrap();

        assert!(result.is_valid); // Extra fields are warnings, not errors
        assert!(result.warnings.len() == 1);
        assert!(result.warnings[0].contains("Extra fields"));
        assert!(result.warnings[0].contains("extra_field"));
    }

    #[test]
    fn test_header_validation_different_order() {
        let actual = vec![
            "signal_name".to_string(),
            "point_id".to_string(),
            "unit".to_string(),
        ];
        let expected = TestPoint::field_names();

        let result =
            CsvHeaderValidator::validate_headers(&actual, &expected, Path::new("test.csv"))
                .unwrap();

        assert!(result.is_valid); // Different order is OK
        assert!(result.warnings.len() == 1);
        assert!(result.warnings[0].contains("Field order"));
    }
}
