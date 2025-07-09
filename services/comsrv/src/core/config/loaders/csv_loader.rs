//! CSV Configuration Loader
//!
//! This module provides utilities for loading various types of CSV configuration files,
//! including point mappings, channel configurations, and protocol-specific data.

use crate::utils::error::{ComSrvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// Cache entry for CSV data
#[derive(Debug, Clone)]
struct CsvCacheEntry<T> {
    data: Vec<T>,
    modified_time: SystemTime,
}

/// Cache statistics for CSV loader
#[derive(Debug, Clone, Default)]
pub struct CsvCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub reloads: u64,
}

impl CsvCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Cached CSV loader that avoids re-reading unchanged files
#[derive(Debug)]
pub struct CachedCsvLoader {
    cache: Arc<RwLock<HashMap<PathBuf, CsvCacheEntry<serde_json::Value>>>>,
    stats: Arc<RwLock<CsvCacheStats>>,
}

impl Default for CachedCsvLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl CachedCsvLoader {
    /// Create a new cached CSV loader
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CsvCacheStats::default())),
        }
    }

    /// Load CSV with caching
    pub async fn load_csv_cached<T>(&self, file_path: &Path) -> Result<Vec<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize + Clone,
    {
        let path = file_path.to_path_buf();

        // Get file modified time
        let metadata = std::fs::metadata(&path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to get file metadata: {e}")))?;
        let current_modified = metadata
            .modified()
            .map_err(|e| ComSrvError::IoError(format!("Failed to get modified time: {e}")))?;

        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&path) {
                if entry.modified_time == current_modified {
                    // Cache hit
                    self.stats.write().await.hits += 1;
                    trace!("CSV cache hit for: {}", path.display());

                    // Deserialize from cached JSON values
                    let result: Result<Vec<T>> = entry
                        .data
                        .iter()
                        .map(|v| {
                            serde_json::from_value(v.clone()).map_err(|e| {
                                ComSrvError::ConfigError(format!(
                                    "Failed to deserialize cached data: {e}"
                                ))
                            })
                        })
                        .collect();

                    return result;
                }
            }
        }

        // Cache miss or stale - load from file
        self.stats.write().await.misses += 1;
        debug!("CSV cache miss for: {}, loading from file", path.display());

        let data = CsvLoader::load_csv::<T>(&path)?;

        // Convert to JSON values for type-erased storage
        let json_values: Vec<serde_json::Value> = data
            .iter()
            .map(|item| serde_json::to_value(item).unwrap())
            .collect();

        // Update cache
        let entry = CsvCacheEntry {
            data: json_values,
            modified_time: current_modified,
        };

        self.cache.write().await.insert(path.clone(), entry);
        self.stats.write().await.reloads += 1;

        Ok(data)
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CsvCacheStats {
        self.stats.read().await.clone()
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
        debug!("CSV cache cleared");
    }

    /// Get cache size
    pub async fn cache_size(&self) -> usize {
        self.cache.read().await.len()
    }
}

/// Generic CSV loader for different point types
#[derive(Debug)]
pub struct CsvLoader;

impl CsvLoader {
    /// Load CSV file and parse into the specified type
    pub fn load_csv<T>(file_path: &Path) -> Result<Vec<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut reader = csv::Reader::from_path(file_path).map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to open CSV file {}: {}",
                file_path.display(),
                e
            ))
        })?;

        let mut records = Vec::new();
        for (line_num, result) in reader.deserialize().enumerate() {
            let record: T = result.map_err(|e| {
                ComSrvError::ConfigError(format!(
                    "Failed to parse CSV record at line {} in {}: {}",
                    line_num + 2,
                    file_path.display(),
                    e // +2 because line 1 is header
                ))
            })?;
            records.push(record);
        }

        Ok(records)
    }

    /// Load and validate CSV file with custom validation
    pub fn load_csv_with_validation<T, F>(file_path: &Path, validator: F) -> Result<Vec<T>>
    where
        T: for<'de> Deserialize<'de>,
        F: Fn(&T) -> Result<()>,
    {
        let records = Self::load_csv(file_path)?;

        for record in &records {
            validator(record)?;
        }

        Ok(records)
    }

    /// Check if CSV file exists and has correct headers
    pub fn validate_csv_format(file_path: &Path, expected_headers: &[&str]) -> Result<()> {
        if !file_path.exists() {
            return Err(ComSrvError::ConfigError(format!(
                "CSV file not found: {}",
                file_path.display()
            )));
        }

        let mut reader = csv::Reader::from_path(file_path).map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to open CSV file {}: {}",
                file_path.display(),
                e
            ))
        })?;

        let headers = reader.headers().map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to read headers from {}: {}",
                file_path.display(),
                e
            ))
        })?;

        for expected_header in expected_headers {
            if !headers.iter().any(|h| h == *expected_header) {
                return Err(ComSrvError::ConfigError(format!(
                    "Missing required header '{}' in CSV file {}",
                    expected_header,
                    file_path.display()
                )));
            }
        }

        Ok(())
    }
}

/// Four Telemetry CSV record structure (YC, YX, YT, YK)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryRecord {
    pub point_id: u32,
    pub signal_name: String,
    pub chinese_name: Option<String>,
    pub data_type: String,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    pub reverse: Option<bool>, // For signal/control types
    pub unit: Option<String>,
    pub description: Option<String>,
    pub group: Option<String>,
}

/// Modbus protocol mapping CSV record structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMappingRecord {
    pub point_id: u32,
    pub signal_name: String,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_format: String,
    pub number_of_bytes: Option<u8>, // Made optional
    pub bit_position: Option<u8>,
    pub byte_order: Option<String>,
    pub register_count: Option<u16>, // This is the primary field
    pub description: Option<String>,
}

/// Specialized loaders for different CSV types
impl CsvLoader {
    /// Load four telemetry CSV (YC, YX, YT, YK)
    pub fn load_four_telemetry_csv(file_path: &Path) -> Result<Vec<FourTelemetryRecord>> {
        Self::validate_csv_format(file_path, &["point_id", "signal_name", "data_type"])?;

        Self::load_csv_with_validation(file_path, |record: &FourTelemetryRecord| {
            if record.signal_name.is_empty() {
                return Err(ComSrvError::ConfigError(
                    "Signal name cannot be empty".to_string(),
                ));
            }
            Ok(())
        })
    }

    /// Load Modbus protocol mapping CSV
    pub fn load_modbus_mapping_csv(file_path: &Path) -> Result<Vec<ModbusMappingRecord>> {
        Self::validate_csv_format(
            file_path,
            &[
                "point_id",
                "signal_name",
                "slave_id",
                "function_code",
                "register_address",
            ],
        )?;

        Self::load_csv_with_validation(file_path, |record: &ModbusMappingRecord| {
            if record.slave_id == 0 || record.slave_id > 247 {
                return Err(ComSrvError::ConfigError(format!(
                    "Invalid Modbus slave ID: {}. Must be 1-247",
                    record.slave_id
                )));
            }

            if record.signal_name.is_empty() {
                return Err(ComSrvError::ConfigError(
                    "Signal name cannot be empty".to_string(),
                ));
            }

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_csv_loader() {
        // Create a temporary CSV file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "point_id,signal_name,data_type").unwrap();
        writeln!(temp_file, "1,Test Signal,float32").unwrap();
        writeln!(temp_file, "2,Another Signal,uint16").unwrap();

        let records: Vec<FourTelemetryRecord> = CsvLoader::load_csv(temp_file.path()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].point_id, 1);
        assert_eq!(records[0].signal_name, "Test Signal");
    }

    #[test]
    fn test_csv_validation() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "point_id,signal_name,data_type").unwrap();
        writeln!(temp_file, "1,Test Signal,float32").unwrap();

        let result = CsvLoader::validate_csv_format(
            temp_file.path(),
            &["point_id", "signal_name", "data_type"],
        );
        assert!(result.is_ok());

        let result =
            CsvLoader::validate_csv_format(temp_file.path(), &["point_id", "missing_header"]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cached_csv_loader() {
        // Create a temporary CSV file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let mut file = std::fs::File::create(&path).unwrap();
            writeln!(file, "point_id,signal_name,data_type").unwrap();
            writeln!(file, "1,Test Signal,float32").unwrap();
            writeln!(file, "2,Another Signal,uint16").unwrap();
        }

        let loader = CachedCsvLoader::new();

        // First load - cache miss
        let records1: Vec<FourTelemetryRecord> = loader.load_csv_cached(&path).await.unwrap();
        assert_eq!(records1.len(), 2);

        let stats = loader.stats().await;
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.reloads, 1);

        // Second load - cache hit
        let records2: Vec<FourTelemetryRecord> = loader.load_csv_cached(&path).await.unwrap();
        assert_eq!(records2.len(), 2);
        assert_eq!(records1[0].point_id, records2[0].point_id);

        let stats = loader.stats().await;
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.reloads, 1);
        assert!(stats.hit_rate() > 0.0);

        // Modify file
        std::thread::sleep(std::time::Duration::from_millis(10));
        {
            let mut file = std::fs::File::create(&path).unwrap();
            writeln!(file, "point_id,signal_name,data_type").unwrap();
            writeln!(file, "3,New Signal,float64").unwrap();
        }

        // Third load - cache miss due to file change
        let records3: Vec<FourTelemetryRecord> = loader.load_csv_cached(&path).await.unwrap();
        assert_eq!(records3.len(), 1);
        assert_eq!(records3[0].point_id, 3);

        let stats = loader.stats().await;
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.reloads, 2);
    }
}
