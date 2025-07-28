//! 配置加载器模块
//!
//! 整合CSV加载、点位映射和协议映射功能

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
// 自定义反序列化函数
// ============================================================================

/// 从字符串反序列化bool，支持"0"/"1"/"true"/"false"
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
            _ => Err(D::Error::custom(format!("Invalid boolean value: {}", s))),
        },
    }
}

/// 从字符串反序列化f64，支持空字符串
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
            .map_err(|_| D::Error::custom(format!("Invalid float value: {}", s))),
    }
}

/// 从字符串反序列化u32
fn deserialize_u32_from_str<'de, D>(deserializer: D) -> std::result::Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    s.parse::<u32>()
        .map_err(|_| D::Error::custom(format!("Invalid u32 value: {}", s)))
}

/// 从字符串反序列化u8
fn deserialize_u8_from_str<'de, D>(deserializer: D) -> std::result::Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    s.parse::<u8>()
        .map_err(|_| D::Error::custom(format!("Invalid u8 value: {}", s)))
}

/// 从字符串反序列化u16
fn deserialize_u16_from_str<'de, D>(deserializer: D) -> std::result::Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    s.parse::<u16>()
        .map_err(|_| D::Error::custom(format!("Invalid u16 value: {}", s)))
}

/// 从字符串反序列化可选u8
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
            .map_err(|_| D::Error::custom(format!("Invalid u8 value: {}", s))),
    }
}

/// 从字符串反序列化可选u32
fn deserialize_optional_u32_from_str<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(ref s) if s.is_empty() => Ok(None),
        Some(s) => s
            .parse::<u32>()
            .map(Some)
            .map_err(|_| D::Error::custom(format!("Invalid u32 value: {}", s))),
    }
}

// ============================================================================
// CSV缓存
// ============================================================================

/// CSV缓存条目
#[derive(Debug, Clone)]
struct CsvCacheEntry<T> {
    data: Vec<T>,
    modified_time: SystemTime,
}

/// CSV缓存统计
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

/// 带缓存的CSV加载器
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
    /// 创建新的缓存CSV加载器
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CsvCacheStats::default())),
        }
    }

    /// 带缓存加载CSV
    pub async fn load_csv_cached<T>(&self, file_path: &Path) -> Result<Vec<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize + Clone,
    {
        let path = file_path.to_path_buf();

        // 获取文件修改时间
        let metadata = std::fs::metadata(&path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to get file metadata: {e}")))?;
        let current_modified = metadata
            .modified()
            .map_err(|e| ComSrvError::IoError(format!("Failed to get modified time: {e}")))?;

        // 检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&path) {
                if entry.modified_time == current_modified {
                    // 缓存命中
                    self.stats.write().await.hits += 1;
                    trace!("CSV cache hit for: {}", path.display());

                    // 从缓存的JSON值反序列化
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

        // 缓存未命中，加载文件
        self.stats.write().await.misses += 1;
        debug!("Loading CSV file: {}", path.display());

        let data = Self::load_csv_file(&path)?;

        // 更新缓存
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

        // 反序列化为目标类型
        let result: Result<Vec<T>> = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                serde_json::from_value(v.clone()).map_err(|e| {
                    eprintln!("DEBUG: Failed to deserialize record {} - Error: {}", i, e);
                    eprintln!("DEBUG: Record content: {:?}", v);
                    ComSrvError::ConfigError(format!(
                        "Failed to deserialize CSV data at row {}: {e}",
                        i
                    ))
                })
            })
            .collect();

        eprintln!(
            "DEBUG: Deserialization result: {} records converted",
            result.as_ref().map(|v| v.len()).unwrap_or(0)
        );
        result
    }

    /// 加载CSV文件
    fn load_csv_file(path: &Path) -> Result<Vec<serde_json::Value>> {
        eprintln!("DEBUG: load_csv_file called for: {}", path.display());
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {e}")))?;

        let headers = reader
            .headers()
            .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV headers: {e}")))?
            .clone();

        let mut records = Vec::new();
        eprintln!("DEBUG: Headers: {:?}", headers);

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

        eprintln!("DEBUG: Read {} records from CSV", records.len());
        if !records.is_empty() {
            eprintln!("DEBUG: First record: {:?}", records[0]);
        }
        Ok(records)
    }

    /// 获取缓存统计
    pub async fn get_stats(&self) -> CsvCacheStats {
        self.stats.read().await.clone()
    }

    /// 清空缓存
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
    }
}

// ============================================================================
// CSV记录类型
// ============================================================================

/// 四遥记录
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

/// Modbus映射记录 - 简化版本
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
    #[serde(rename = "data_type")]
    pub data_format: String,
    #[serde(default)]
    pub byte_order: Option<String>,
    // 可选字段，仅在特殊情况下使用（如信号位操作）
    #[serde(default, deserialize_with = "deserialize_opt_u8_from_str")]
    pub bit_position: Option<u8>,
}

impl ModbusMappingRecord {
    /// 根据数据类型推断寄存器数量
    pub fn register_count(&self) -> u16 {
        match self.data_format.as_str() {
            "bool" | "int8" | "uint8" => 1,
            "int16" | "uint16" => 1,
            "int32" | "uint32" | "float32" => 2,
            "int64" | "uint64" | "float64" => 4,
            _ => {
                tracing::warn!("未知数据类型: {}, 默认使用1个寄存器", self.data_format);
                1
            }
        }
    }

    /// 根据数据类型推断字节数
    pub fn byte_count(&self) -> u8 {
        match self.data_format.as_str() {
            "bool" | "int8" | "uint8" => 1,
            "int16" | "uint16" => 2,
            "int32" | "uint32" | "float32" => 4,
            "int64" | "uint64" | "float64" => 8,
            _ => {
                tracing::warn!("未知数据类型: {}, 默认使用2字节", self.data_format);
                2
            }
        }
    }

    /// 获取默认字节序（如果未指定）
    pub fn effective_byte_order(&self) -> String {
        self.byte_order
            .clone()
            .unwrap_or_else(|| match self.data_format.as_str() {
                "int16" | "uint16" => "AB".to_string(),
                "int32" | "uint32" | "float32" => "ABCD".to_string(),
                "int64" | "uint64" | "float64" => "ABCDEFGH".to_string(),
                _ => "AB".to_string(),
            })
    }

    /// 获取有效位位置（如果未指定，默认为0）
    pub fn effective_bit_position(&self) -> u8 {
        self.bit_position.unwrap_or(0)
    }
}

// ============================================================================
// 点位映射
// ============================================================================

/// 点位映射器
#[derive(Debug)]
pub struct PointMapper;

impl PointMapper {
    /// 合并Modbus点位
    pub fn combine_modbus_points(
        remote_points: Vec<FourRemoteRecord>,
        modbus_mappings: Vec<ModbusMappingRecord>,
        telemetry_type: &str,
    ) -> Result<Vec<CombinedPoint>> {
        // 创建映射查找表
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
                protocol_params.insert("data_format".to_string(), mapping.data_format.clone());

                // 使用自动推断的值
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

        // 检查未映射的协议映射
        for (point_id, _) in mapping_lookup {
            warn!(
                "Protocol mapping for point_id {} has no corresponding remote point",
                point_id
            );
        }

        Ok(combined_points)
    }

    /// 验证点位映射
    pub fn validate_mappings(points: &[CombinedPoint]) -> Result<()> {
        let mut seen_ids = HashMap::new();

        for point in points {
            if let Some(existing) = seen_ids.insert(point.point_id, &point.signal_name) {
                return Err(ComSrvError::ConfigError(format!(
                    "Duplicate point_id {} found: '{}' and '{}'",
                    point.point_id, existing, point.signal_name
                )));
            }

            // 验证协议参数
            if !point.protocol_params.contains_key("slave_id") {
                return Err(ComSrvError::ConfigError(format!(
                    "Missing slave_id for point_id {}",
                    point.point_id
                )));
            }
        }

        Ok(())
    }

    // 四遥分离架构下，不再需要to_unified_mappings方法
}

// ============================================================================
// 协议映射
// ============================================================================

/// 协议映射trait
pub trait ProtocolMapping: Send + Sync {
    /// 获取点位ID
    fn point_id(&self) -> u32;

    /// 获取信号名称
    fn signal_name(&self) -> &str;

    /// 转换为协议参数
    fn to_protocol_params(&self) -> HashMap<String, String>;

    /// 获取数据格式
    fn data_format(&self) -> &str;

    /// 获取数据大小
    fn data_size(&self) -> u8;
}

/// Modbus协议映射 - 简化版本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_format: String,
    pub byte_order: Option<String>,
    pub bit_position: Option<u8>,
    pub description: Option<String>,
}

impl ModbusMapping {
    /// 根据数据类型推断寄存器数量
    pub fn register_count(&self) -> u16 {
        match self.data_format.as_str() {
            "bool" | "int8" | "uint8" => 1,
            "int16" | "uint16" => 1,
            "int32" | "uint32" | "float32" => 2,
            "int64" | "uint64" | "float64" => 4,
            _ => 1,
        }
    }

    /// 根据数据类型推断字节数
    pub fn byte_count(&self) -> u8 {
        match self.data_format.as_str() {
            "bool" | "int8" | "uint8" => 1,
            "int16" | "uint16" => 2,
            "int32" | "uint32" | "float32" => 4,
            "int64" | "uint64" | "float64" => 8,
            _ => 2,
        }
    }

    /// 获取有效字节序
    pub fn effective_byte_order(&self) -> String {
        self.byte_order
            .clone()
            .unwrap_or_else(|| match self.data_format.as_str() {
                "int16" | "uint16" => "AB".to_string(),
                "int32" | "uint32" | "float32" => "ABCD".to_string(),
                "int64" | "uint64" | "float64" => "ABCDEFGH".to_string(),
                _ => "AB".to_string(),
            })
    }

    /// 获取有效位位置（如果未指定，默认为0）
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

        // 使用自动推断的值
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
        &self.data_format
    }

    fn data_size(&self) -> u8 {
        self.register_count() as u8
    }
}

/// CAN协议映射
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
        (self.bit_length + 7) / 8
    }
}

/// IEC60870协议映射
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
            1..=14 => 1,
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
                telemetry_type: "Measurement".to_string(),
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
                telemetry_type: "Measurement".to_string(),
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
            .unwrap_err()
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
            data_format: "float32".to_string(),
            bit_position: None,
            byte_order: Some("DCBA".to_string()),
            description: None,
        };

        let params = mapping.to_protocol_params();
        assert_eq!(params.get("slave_id").unwrap(), "1");
        assert_eq!(params.get("function_code").unwrap(), "3");
        assert_eq!(params.get("register_address").unwrap(), "1000");
        assert_eq!(params.get("byte_order").unwrap(), "DCBA");
        assert_eq!(params.get("register_count").unwrap(), "2");
        assert_eq!(params.get("byte_count").unwrap(), "4");
        assert_eq!(mapping.data_size(), 2);

        // 测试自动推断
        let mapping_auto = ModbusMapping {
            point_id: 101,
            signal_name: "Test Auto".to_string(),
            slave_id: 1,
            function_code: 3,
            register_address: 1002,
            data_format: "int16".to_string(),
            bit_position: None,
            byte_order: None, // 未指定，应该自动推断
            description: None,
        };

        let params_auto = mapping_auto.to_protocol_params();
        assert_eq!(params_auto.get("byte_order").unwrap(), "AB");
        assert_eq!(params_auto.get("register_count").unwrap(), "1");
        assert_eq!(params_auto.get("byte_count").unwrap(), "2");
        assert_eq!(params_auto.get("bit_position").unwrap(), "0"); // 默认为0
    }
}
