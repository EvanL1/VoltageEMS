//! Configuration loader module
//!
//! Integrates CSV loading, point mapping and protocol mapping functionality

use super::types::{CombinedPoint, ScalingInfo};
use crate::utils::error::{ComSrvError, Result};
use csv::ReaderBuilder;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{debug, trace, warn};

// ============================================================================
// Custom deserialization functions
// ============================================================================

/// Deserialize bool from string, supports "0"/"1"/"true"/"false"
fn deserialize_bool_from_str<'de, D>(deserializer: D) -> std::result::Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(ref s) if s.is_empty() => Ok(None),
        Some(s) => match s.as_str() {
            "0" | "false" | "False" | "FALSE" => Ok(Some(false)),
            "1" | "true" | "True" | "TRUE" => Ok(Some(true)),
            _ => Err(D::Error::custom(format!("Invalid boolean value: {s}"))),
        },
    }
}

/// Deserialize f64 from string, supports empty string
fn deserialize_f64_from_str<'de, D>(deserializer: D) -> std::result::Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(ref s) if s.is_empty() => Ok(None),
        Some(s) => s
            .parse::<f64>()
            .map(Some)
            .map_err(|_| D::Error::custom(format!("Invalid float value: {s}"))),
    }
}

/// Deserialize u32 from string
fn deserialize_u32_from_str<'de, D>(deserializer: D) -> std::result::Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    s.parse::<u32>()
        .map_err(|_| D::Error::custom(format!("Invalid u32 value: {s}")))
}

/// Deserialize u8 from string
fn deserialize_u8_from_str<'de, D>(deserializer: D) -> std::result::Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    s.parse::<u8>()
        .map_err(|_| D::Error::custom(format!("Invalid u8 value: {s}")))
}

/// Deserialize u16 from string
fn deserialize_u16_from_str<'de, D>(deserializer: D) -> std::result::Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    s.parse::<u16>()
        .map_err(|_| D::Error::custom(format!("Invalid u16 value: {s}")))
}

/// Deserialize optional u8 from string
fn deserialize_opt_u8_from_str<'de, D>(deserializer: D) -> std::result::Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(ref s) if s.is_empty() => Ok(None),
        Some(s) => s
            .parse::<u8>()
            .map(Some)
            .map_err(|_| D::Error::custom(format!("Invalid u8 value: {s}"))),
    }
}

// ============================================================================
// CSV cache
// ============================================================================

/// CSV cache entry
#[derive(Debug, Clone)]
struct CsvCacheEntry<T> {
    data: Vec<T>,
    modified_time: SystemTime,
}

/// CSV cache statistics
#[derive(Debug, Clone, Default)]
pub struct CsvCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub reloads: u64,
}

impl CsvCacheStats {
    #[allow(clippy::cast_precision_loss)]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// CSV loader with cache
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
    /// Create new cached CSV loader
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CsvCacheStats::default())),
        }
    }

    /// Load CSV with cache
    pub async fn load_csv_cached<T>(&self, file_path: &Path) -> Result<Vec<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize + Clone,
    {
        let path = file_path.to_path_buf();

        // Get file modification time
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

        // Cache miss, load file
        self.stats.write().await.misses += 1;
        debug!("Loading CSV file: {}", path.display());

        let data = Self::load_csv_file(&path)?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                path.clone(),
                CsvCacheEntry {
                    data: data.clone(),
                    modified_time: current_modified,
                },
            );
            self.stats.write().await.reloads += 1;
        }

        // Deserialize to target type
        let result: Result<Vec<T>> = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                serde_json::from_value(v.clone()).map_err(|e| {
                    debug!("Failed to deserialize record {i} - Error: {e}");
                    debug!("Record content: {v:?}");
                    ComSrvError::ConfigError(format!(
                        "Failed to deserialize CSV data at row {i}: {e}"
                    ))
                })
            })
            .collect();

        debug!(
            "Deserialization result: {} records converted",
            result.as_ref().map(std::vec::Vec::len).unwrap_or(0)
        );
        result
    }

    /// Load CSV file
    fn load_csv_file(path: &Path) -> Result<Vec<serde_json::Value>> {
        debug!("load_csv_file called for: {}", path.display());
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {e}")))?;

        let headers = reader
            .headers()
            .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV headers: {e}")))?
            .clone();

        let mut records = Vec::new();
        debug!("Headers: {headers:?}");

        for result in reader.records() {
            let record = result
                .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV record: {e}")))?;

            let mut map = serde_json::Map::new();
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = headers.get(i) {
                    map.insert(
                        header.to_string(),
                        serde_json::Value::String(field.to_string()),
                    );
                }
            }

            records.push(serde_json::Value::Object(map));
        }

        debug!("Read {} records from CSV", records.len());
        if !records.is_empty() {
            debug!("First record: {:?}", records[0]);
        }
        Ok(records)
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CsvCacheStats {
        self.stats.read().await.clone()
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
    }
}

// ============================================================================
// CSV record types
// ============================================================================

/// Four-telemetry record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourRemoteRecord {
    #[serde(deserialize_with = "deserialize_u32_from_str")]
    pub point_id: u32,
    pub signal_name: String,
    pub data_type: String,
    #[serde(default, deserialize_with = "deserialize_f64_from_str")]
    pub scale: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_f64_from_str")]
    pub offset: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_bool_from_str")]
    pub reverse: Option<bool>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Modbus mapping record - simplified version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMappingRecord {
    #[serde(deserialize_with = "deserialize_u32_from_str")]
    pub point_id: u32,
    #[serde(deserialize_with = "deserialize_u8_from_str")]
    pub slave_id: u8,
    #[serde(deserialize_with = "deserialize_u8_from_str")]
    pub function_code: u8,
    #[serde(deserialize_with = "deserialize_u16_from_str")]
    pub register_address: u16,
    pub data_type: String,
    #[serde(default)]
    pub byte_order: Option<String>,
    // Optional field, only used in special cases (e.g., signal bit operations)
    #[serde(default, deserialize_with = "deserialize_opt_u8_from_str")]
    pub bit_position: Option<u8>,
}

impl ModbusMappingRecord {
    /// Infer register count based on data type
    pub fn register_count(&self) -> u16 {
        match self.data_type.as_str() {
            "bool" | "int8" | "uint8" | "int16" | "uint16" => 1,
            "int32" | "uint32" | "float32" => 2,
            "int64" | "uint64" | "float64" => 4,
            _ => {
                tracing::warn!(
                    "Unknown data type: {}, defaulting to 1 register",
                    self.data_type
                );
                1
            },
        }
    }

    /// Infer byte count based on data type
    pub fn byte_count(&self) -> u8 {
        match self.data_type.as_str() {
            "bool" | "int8" | "uint8" => 1,
            "int16" | "uint16" => 2,
            "int32" | "uint32" | "float32" => 4,
            "int64" | "uint64" | "float64" => 8,
            _ => {
                tracing::warn!(
                    "Unknown data type: {}, defaulting to 2 bytes",
                    self.data_type
                );
                2
            },
        }
    }

    /// Get default byte order (if not specified)
    pub fn effective_byte_order(&self) -> String {
        self.byte_order
            .as_ref()
            .cloned()
            .unwrap_or_else(|| match self.data_type.as_str() {
                "int32" | "uint32" | "float32" => "ABCD".to_string(),
                "int64" | "uint64" | "float64" => "ABCDEFGH".to_string(),
                _ => "AB".to_string(),
            })
    }

    /// Get effective bit position (defaults to 0 if not specified)
    pub fn effective_bit_position(&self) -> u8 {
        self.bit_position.unwrap_or(0)
    }
}

// ============================================================================
// Point mapping
// ============================================================================

/// Point mapper
#[derive(Debug)]
pub struct PointMapper;

impl PointMapper {
    /// Merge Modbus points
    pub fn combine_modbus_points(
        remote_points: Vec<FourRemoteRecord>,
        modbus_mappings: Vec<ModbusMappingRecord>,
        telemetry_type: &str,
    ) -> Result<Vec<CombinedPoint>> {
        // Create mapping lookup table
        let mut mapping_lookup: HashMap<u32, ModbusMappingRecord> = HashMap::new();
        for mapping in modbus_mappings {
            mapping_lookup.insert(mapping.point_id, mapping);
        }

        let mut combined_points = Vec::new();

        for remote in remote_points {
            if let Some(mapping) = mapping_lookup.remove(&remote.point_id) {
                let mut protocol_params = HashMap::new();
                protocol_params.insert("slave_id".to_string(), mapping.slave_id.to_string());
                protocol_params.insert(
                    "function_code".to_string(),
                    mapping.function_code.to_string(),
                );
                protocol_params.insert(
                    "register_address".to_string(),
                    mapping.register_address.to_string(),
                );
                protocol_params.insert("data_type".to_string(), mapping.data_type.clone());

                // Use automatically inferred values
                protocol_params.insert(
                    "register_count".to_string(),
                    mapping.register_count().to_string(),
                );
                protocol_params.insert("byte_count".to_string(), mapping.byte_count().to_string());
                protocol_params.insert("byte_order".to_string(), mapping.effective_byte_order());
                protocol_params.insert(
                    "bit_position".to_string(),
                    mapping.effective_bit_position().to_string(),
                );

                let combined = CombinedPoint {
                    point_id: remote.point_id,
                    signal_name: remote.signal_name,
                    telemetry_type: telemetry_type.to_string(),
                    data_type: remote.data_type,
                    protocol_params,
                    scaling: if remote.scale.is_some()
                        || remote.offset.is_some()
                        || remote.reverse.is_some()
                    {
                        Some(ScalingInfo {
                            scale: remote.scale.unwrap_or(1.0),
                            offset: remote.offset.unwrap_or(0.0),
                            unit: remote.unit,
                            reverse: remote.reverse,
                        })
                    } else {
                        None
                    },
                };

                combined_points.push(combined);
            } else {
                warn!(
                    "No protocol mapping found for point_id: {}",
                    remote.point_id
                );
            }
        }

        // Check unmapped protocol mappings
        for (point_id, _) in mapping_lookup {
            warn!(
                "Protocol mapping for point_id {} has no corresponding remote point",
                point_id
            );
        }

        Ok(combined_points)
    }

    /// Validate point mappings
    pub fn validate_mappings(points: &[CombinedPoint]) -> Result<()> {
        let mut seen_ids = HashMap::new();

        for point in points {
            if let Some(existing) = seen_ids.insert(point.point_id, &point.signal_name) {
                return Err(ComSrvError::ConfigError(format!(
                    "Duplicate point_id {} found: '{}' and '{}'",
                    point.point_id, existing, point.signal_name
                )));
            }

            // Validate protocol parameters
            if !point.protocol_params.contains_key("slave_id") {
                return Err(ComSrvError::ConfigError(format!(
                    "Missing slave_id for point_id {}",
                    point.point_id
                )));
            }
        }

        Ok(())
    }

    // Under four-telemetry separation architecture, to_unified_mappings method is no longer needed
}

// ============================================================================
// Protocol mapping
// ============================================================================

/// Protocol mapping trait
pub trait ProtocolMapping: Send + Sync {
    /// Get point ID
    fn point_id(&self) -> u32;

    /// Get signal name
    fn signal_name(&self) -> &str;

    /// Convert to protocol parameters
    fn to_protocol_params(&self) -> HashMap<String, String>;

    /// Get data format
    fn data_format(&self) -> &str;

    /// Get data size
    fn data_size(&self) -> u8;
}

/// Modbus protocol mapping - simplified version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_type: String,
    pub byte_order: Option<String>,
    pub bit_position: Option<u8>,
    pub description: Option<String>,
}

impl ModbusMapping {
    /// Infer register count based on data type
    pub fn register_count(&self) -> u16 {
        match self.data_type.as_str() {
            "int32" | "uint32" | "float32" => 2,
            "int64" | "uint64" | "float64" => 4,
            _ => 1,
        }
    }

    /// Infer byte count based on data type
    pub fn byte_count(&self) -> u8 {
        match self.data_type.as_str() {
            "bool" | "int8" | "uint8" => 1,
            "int32" | "uint32" | "float32" => 4,
            "int64" | "uint64" | "float64" => 8,
            _ => 2,
        }
    }

    /// Get effective byte order
    pub fn effective_byte_order(&self) -> String {
        self.byte_order
            .as_ref()
            .cloned()
            .unwrap_or_else(|| match self.data_type.as_str() {
                "int32" | "uint32" | "float32" => "ABCD".to_string(),
                "int64" | "uint64" | "float64" => "ABCDEFGH".to_string(),
                _ => "AB".to_string(),
            })
    }

    /// Get effective bit position (defaults to 0 if not specified)
    pub fn effective_bit_position(&self) -> u8 {
        self.bit_position.unwrap_or(0)
    }
}

impl ProtocolMapping for ModbusMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn to_protocol_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("slave_id".to_string(), self.slave_id.to_string());
        params.insert("function_code".to_string(), self.function_code.to_string());
        params.insert(
            "register_address".to_string(),
            self.register_address.to_string(),
        );

        // using自动push断的value
        params.insert(
            "register_count".to_string(),
            self.register_count().to_string(),
        );
        params.insert("byte_count".to_string(), self.byte_count().to_string());
        params.insert("byte_order".to_string(), self.effective_byte_order());
        params.insert(
            "bit_position".to_string(),
            self.effective_bit_position().to_string(),
        );

        params
    }

    fn data_format(&self) -> &str {
        &self.data_type
    }

    fn data_size(&self) -> u8 {
        self.byte_count()
    }
}

/// CAN protocol mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub can_id: u32,
    pub start_bit: u8,
    pub bit_length: u8,
    pub byte_order: String,
    pub data_type: String,
}

impl ProtocolMapping for CanMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn to_protocol_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("can_id".to_string(), self.can_id.to_string());
        params.insert("start_bit".to_string(), self.start_bit.to_string());
        params.insert("bit_length".to_string(), self.bit_length.to_string());
        params.insert("byte_order".to_string(), self.byte_order.clone());
        params.insert("data_type".to_string(), self.data_type.clone());
        params
    }

    fn data_format(&self) -> &str {
        &self.data_type
    }

    fn data_size(&self) -> u8 {
        self.bit_length.div_ceil(8)
    }
}

/// IEC60870 protocol mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iec60870Mapping {
    pub point_id: u32,
    pub signal_name: String,
    pub ioa: u32, // Information Object Address
    pub type_id: u8,
    pub common_address: u16,
}

impl ProtocolMapping for Iec60870Mapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn to_protocol_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("ioa".to_string(), self.ioa.to_string());
        params.insert("type_id".to_string(), self.type_id.to_string());
        params.insert(
            "common_address".to_string(),
            self.common_address.to_string(),
        );
        params
    }

    fn data_format(&self) -> &str {
        match self.type_id {
            1..=14 => "bool",
            15..=40 => "float",
            _ => "unknown",
        }
    }

    fn data_size(&self) -> u8 {
        match self.type_id {
            15..=40 => 4,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_stats() {
        let mut stats = CsvCacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0);

        stats.hits = 75;
        stats.misses = 25;
        assert_eq!(stats.hit_rate(), 0.75);
    }

    #[test]
    fn test_point_mapper_validation() {
        let points = vec![
            CombinedPoint {
                point_id: 1,
                signal_name: "Test1".to_string(),
                telemetry_type: "Telemetry".to_string(),
                data_type: "float".to_string(),
                protocol_params: {
                    let mut params = HashMap::new();
                    params.insert("slave_id".to_string(), "1".to_string());
                    params
                },
                scaling: None,
            },
            CombinedPoint {
                point_id: 1, // Duplicate ID
                signal_name: "Test2".to_string(),
                telemetry_type: "Telemetry".to_string(),
                data_type: "float".to_string(),
                protocol_params: {
                    let mut params = HashMap::new();
                    params.insert("slave_id".to_string(), "1".to_string());
                    params
                },
                scaling: None,
            },
        ];

        let result = PointMapper::validate_mappings(&points);
        assert!(result.is_err());
        assert!(result
            .expect_err("validation should fail for duplicate point_id")
            .to_string()
            .contains("Duplicate point_id"));
    }

    #[test]
    fn test_modbus_mapping_trait() {
        let mapping = ModbusMapping {
            point_id: 100,
            signal_name: "Test".to_string(),
            slave_id: 1,
            function_code: 3,
            register_address: 1000,
            data_type: "float32".to_string(),
            bit_position: None,
            byte_order: Some("DCBA".to_string()),
            description: None,
        };

        let params = mapping.to_protocol_params();
        assert_eq!(params.get("slave_id").expect("slave_id should exist"), "1");
        assert_eq!(
            params
                .get("function_code")
                .expect("function_code should exist"),
            "3"
        );
        assert_eq!(
            params
                .get("register_address")
                .expect("register_address should exist"),
            "1000"
        );
        assert_eq!(
            params.get("byte_order").expect("byte_order should exist"),
            "DCBA"
        );
        assert_eq!(
            params
                .get("register_count")
                .expect("register_count should exist"),
            "2"
        );
        assert_eq!(
            params.get("byte_count").expect("byte_count should exist"),
            "4"
        );
        assert_eq!(mapping.data_size(), 2);

        // Test automatic inference
        let mapping_auto = ModbusMapping {
            point_id: 101,
            signal_name: "Test Auto".to_string(),
            slave_id: 1,
            function_code: 3,
            register_address: 1002,
            data_type: "int16".to_string(),
            bit_position: None,
            byte_order: None, // Not specified, should be automatically inferred
            description: None,
        };

        let params_auto = mapping_auto.to_protocol_params();
        assert_eq!(
            params_auto
                .get("byte_order")
                .expect("byte_order should be auto-inferred"),
            "AB"
        );
        assert_eq!(
            params_auto
                .get("register_count")
                .expect("register_count should be auto-inferred"),
            "1"
        );
        assert_eq!(
            params_auto
                .get("byte_count")
                .expect("byte_count should be auto-inferred"),
            "2"
        );
        assert_eq!(
            params_auto
                .get("bit_position")
                .expect("bit_position should default to 0"),
            "0"
        ); // Defaults to 0
    }
}
