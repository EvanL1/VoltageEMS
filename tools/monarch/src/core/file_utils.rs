//! Utility functions for configuration loading and processing

use anyhow::{Context, Result};
use csv::Reader;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;
use voltage_config::validation::{CsvFields, CsvHeaderValidator};

/// Error that occurred while parsing a specific CSV row
#[derive(Debug, Clone)]
pub struct CsvRowError {
    /// Row number (1-indexed, excluding header)
    pub row_number: usize,
    /// Error message
    pub error: String,
}

/// Type alias for CSV loading result with error recovery
pub type CsvResult<T> = Result<(Vec<T>, Vec<CsvRowError>)>;

/// Load CSV file and return as vector of hashmaps
pub fn load_csv<P: AsRef<Path>>(path: P) -> Result<Vec<HashMap<String, String>>> {
    let path = path.as_ref();
    debug!("Loading CSV file: {:?}", path);

    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open CSV file: {:?}", path))?;

    let mut reader = Reader::from_reader(file);
    let headers = reader
        .headers()
        .with_context(|| format!("Failed to read CSV headers: {:?}", path))?
        .clone();

    let mut records = Vec::new();
    for result in reader.records() {
        let record = result.with_context(|| format!("Failed to read CSV record: {:?}", path))?;

        let mut row = HashMap::new();
        for (i, field) in record.iter().enumerate() {
            if let Some(header) = headers.get(i) {
                row.insert(header.to_string(), field.to_string());
            }
        }
        records.push(row);
    }

    debug!("Loaded {} records from CSV: {:?}", records.len(), path);
    Ok(records)
}

/// Load CSV file and deserialize directly into typed structs
pub fn load_csv_typed<T, P>(path: P) -> Result<Vec<T>>
where
    T: DeserializeOwned,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    debug!("Loading CSV file with typed deserialization: {:?}", path);

    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open CSV file: {:?}", path))?;

    let mut reader = Reader::from_reader(file);
    let mut records = Vec::new();

    for (line_number, result) in reader.deserialize().enumerate() {
        let record: T = match result {
            Ok(rec) => rec,
            Err(e) => {
                // Print detailed CSV error before propagating
                eprintln!(
                    "CSV deserialization error at line {}: {:#?}",
                    line_number + 2,
                    e
                );
                return Err(anyhow::Error::new(e)).with_context(|| {
                    format!(
                        "Failed to deserialize CSV record from: {:?} (line {})",
                        path,
                        line_number + 2
                    )
                });
            },
        };
        records.push(record);
    }

    debug!(
        "Loaded and deserialized {} typed records from CSV: {:?}",
        records.len(),
        path
    );
    Ok(records)
}

/// Load CSV file with error recovery - returns successful rows and errors separately
///
/// This version does not fail on individual row errors, instead collecting them
/// for reporting while continuing to process valid rows.
pub fn load_csv_with_errors<P: AsRef<Path>>(path: P) -> CsvResult<HashMap<String, String>> {
    let path = path.as_ref();
    debug!("Loading CSV file with error recovery: {:?}", path);

    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open CSV file: {:?}", path))?;

    let mut reader = Reader::from_reader(file);
    let headers = reader
        .headers()
        .with_context(|| format!("Failed to read CSV headers: {:?}", path))?
        .clone();

    let mut records = Vec::new();
    let mut errors = Vec::new();

    for (row_number, result) in reader.records().enumerate() {
        let row_number = row_number + 1; // 1-indexed

        match result {
            Ok(record) => {
                let mut row = HashMap::new();
                for (i, field) in record.iter().enumerate() {
                    if let Some(header) = headers.get(i) {
                        row.insert(header.to_string(), field.to_string());
                    }
                }
                records.push(row);
            },
            Err(e) => {
                errors.push(CsvRowError {
                    row_number,
                    error: e.to_string(),
                });
            },
        }
    }

    debug!(
        "Loaded {} valid records and encountered {} errors from CSV: {:?}",
        records.len(),
        errors.len(),
        path
    );
    Ok((records, errors))
}

/// Load CSV file and deserialize with error recovery
///
/// This version does not fail on individual row deserialization errors,
/// instead collecting them for reporting while continuing to process valid rows.
///
/// Additionally validates CSV header against expected fields defined by CsvFields trait.
pub fn load_csv_typed_with_errors<T, P>(path: P) -> CsvResult<T>
where
    T: DeserializeOwned + CsvFields,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    debug!(
        "Loading CSV file with typed deserialization and error recovery: {:?}",
        path
    );

    let mut errors = Vec::new();

    // Step 1: Validate CSV header before processing
    match CsvHeaderValidator::validate_csv_header::<T>(path) {
        Ok(validation_result) => {
            // Add validation errors as CSV row errors
            for error in validation_result.errors {
                errors.push(CsvRowError {
                    row_number: 0, // 0 indicates header error
                    error,
                });
            }

            // Log warnings but don't fail
            for warning in validation_result.warnings {
                tracing::warn!("CSV header warning: {}", warning);
            }

            // If header validation failed, we still try to parse (best effort)
            // but the errors are already recorded
        },
        Err(e) => {
            // If we can't even read the file for validation, add as error
            errors.push(CsvRowError {
                row_number: 0,
                error: format!("Header validation failed: {}", e),
            });
        },
    }

    // Step 2: Attempt to load and deserialize records
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open CSV file: {:?}", path))?;

    let mut reader = Reader::from_reader(file);
    let mut records = Vec::new();

    for (row_number, result) in reader.deserialize().enumerate() {
        let row_number = row_number + 1; // 1-indexed

        match result {
            Ok(record) => {
                records.push(record);
            },
            Err(e) => {
                errors.push(CsvRowError {
                    row_number,
                    error: e.to_string(),
                });
            },
        }
    }

    debug!(
        "Loaded and deserialized {} valid typed records and encountered {} errors from CSV: {:?}",
        records.len(),
        errors.len(),
        path
    );
    Ok((records, errors))
}

/// Flatten nested JSON object into key-value pairs
pub fn flatten_json(value: &JsonValue, prefix: Option<String>) -> HashMap<String, JsonValue> {
    let mut result = HashMap::new();

    match value {
        JsonValue::Object(map) => {
            for (key, val) in map {
                let new_key = match &prefix {
                    Some(p) => format!("{}.{}", p, key),
                    None => key.clone(),
                };

                match val {
                    JsonValue::Object(_) => {
                        // Recursively flatten nested objects
                        let nested = flatten_json(val, Some(new_key));
                        result.extend(nested);
                    },
                    _ => {
                        // Store leaf values directly
                        result.insert(new_key, val.clone());
                    },
                }
            }
        },
        _ => {
            // If not an object, store the value with the given prefix
            if let Some(p) = prefix {
                result.insert(p, value.clone());
            }
        },
    }

    result
}

/// Database status information
#[allow(dead_code)]
pub struct DatabaseStatus {
    pub exists: bool,
    pub initialized: bool,
    pub last_sync: Option<String>,
    pub item_count: Option<usize>,
    pub schema_version: Option<String>,
}

/// Create parent directories for a path if they don't exist
#[allow(dead_code)]
pub fn ensure_parent_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;
    }
    Ok(())
}

/// Check if a path exists and is a directory
#[allow(dead_code)]
pub fn dir_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().exists() && path.as_ref().is_dir()
}

/// Parse boolean from string (handles various formats)
#[allow(dead_code)]
pub fn parse_bool(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "true" | "yes" | "1" | "on" | "enabled"
    )
}

/// Parse optional integer from string
#[allow(dead_code)]
pub fn parse_optional_int(s: Option<&str>) -> Option<i32> {
    s.and_then(|v| v.parse::<i32>().ok())
}

/// Parse optional float from string
#[allow(dead_code)]
pub fn parse_optional_float(s: Option<&str>) -> Option<f64> {
    s.and_then(|v| v.parse::<f64>().ok())
}

/// Set database file permissions for Docker compatibility
/// Sets permissions to 664 (rw-rw-r--) to allow owner and group access
/// while preventing world write access for security
pub fn set_database_permissions<P: AsRef<Path>>(path: P) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = path.as_ref();
        if path.exists() {
            let mut perms = std::fs::metadata(path)?.permissions();
            // Set permissions to 664 (rw-rw-r--) - owner and group can read/write, others read-only
            perms.set_mode(0o664);
            std::fs::set_permissions(path, perms)
                .with_context(|| format!("Failed to set permissions for {:?}", path))?;
            debug!("Set permissions to 664 for {:?}", path);
        }
    }
    Ok(())
}
