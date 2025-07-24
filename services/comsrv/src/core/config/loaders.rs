//! 配置加载器模块
//!
//! 整合CSV加载、点位映射和协议映射功能

use super::types::{CombinedPoint, ScalingInfo, UnifiedPointMapping};
use crate::utils::error::{ComSrvError, Result};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{debug, trace, warn};

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
        data.into_iter()
            .map(|v| {
                serde_json::from_value(v).map_err(|e| {
                    ComSrvError::ConfigError(format!("Failed to deserialize CSV data: {e}"))
                })
            })
            .collect()
    }

    /// 加载CSV文件
    fn load_csv_file(path: &Path) -> Result<Vec<serde_json::Value>> {
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {e}")))?;

        let headers = reader
            .headers()
            .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV headers: {e}")))?
            .clone();

        let mut records = Vec::new();

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
pub struct FourTelemetryRecord {
    pub point_id: u32,
    pub signal_name: String,
    pub data_type: String,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    pub reverse: Option<bool>,
    pub unit: Option<String>,
    pub description: Option<String>,
}

/// Modbus映射记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMappingRecord {
    pub point_id: u32,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_format: String,
    pub number_of_bytes: Option<u8>,
    pub bit_position: Option<u8>,
    pub byte_order: Option<String>,
    pub register_count: Option<u16>,
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
        telemetry_points: Vec<FourTelemetryRecord>,
        modbus_mappings: Vec<ModbusMappingRecord>,
        telemetry_type: &str,
    ) -> Result<Vec<CombinedPoint>> {
        // 创建映射查找表
        let mut mapping_lookup: HashMap<u32, ModbusMappingRecord> = HashMap::new();
        for mapping in modbus_mappings {
            mapping_lookup.insert(mapping.point_id, mapping);
        }

        let mut combined_points = Vec::new();

        for telemetry in telemetry_points {
            if let Some(mapping) = mapping_lookup.remove(&telemetry.point_id) {
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

                if let Some(bit_pos) = mapping.bit_position {
                    protocol_params.insert("bit_position".to_string(), bit_pos.to_string());
                }
                if let Some(byte_order) = mapping.byte_order {
                    protocol_params.insert("byte_order".to_string(), byte_order);
                }
                if let Some(reg_count) = mapping.register_count {
                    protocol_params.insert("register_count".to_string(), reg_count.to_string());
                }

                let combined = CombinedPoint {
                    point_id: telemetry.point_id,
                    signal_name: telemetry.signal_name,
                    telemetry_type: telemetry_type.to_string(),
                    data_type: telemetry.data_type,
                    protocol_params,
                    scaling: if telemetry.scale.is_some() || telemetry.offset.is_some() {
                        Some(ScalingInfo {
                            scale: telemetry.scale.unwrap_or(1.0),
                            offset: telemetry.offset.unwrap_or(0.0),
                            unit: telemetry.unit,
                            reverse: telemetry.reverse,
                        })
                    } else {
                        None
                    },
                };

                combined_points.push(combined);
            } else {
                warn!(
                    "No protocol mapping found for point_id: {}",
                    telemetry.point_id
                );
            }
        }

        // 检查未映射的协议映射
        for (point_id, _) in mapping_lookup {
            warn!(
                "Protocol mapping for point_id {} has no corresponding telemetry point",
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

    /// 转换为统一点位映射
    pub fn to_unified_mappings(points: Vec<CombinedPoint>) -> Vec<UnifiedPointMapping> {
        points
            .into_iter()
            .map(|point| UnifiedPointMapping {
                point_id: point.point_id,
                signal_name: point.signal_name,
                telemetry_type: point.telemetry_type,
                data_type: point.data_type,
                protocol_params: point.protocol_params,
                scaling: point.scaling.map(|s| super::types::ScalingParams {
                    scale: s.scale,
                    offset: s.offset,
                    unit: s.unit,
                    reverse: None,
                }),
            })
            .collect()
    }
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

/// Modbus协议映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_format: String,
    pub number_of_bytes: Option<u8>,
    pub bit_position: Option<u8>,
    pub byte_order: Option<String>,
    pub register_count: Option<u16>,
    pub description: Option<String>,
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

        if let Some(bit_pos) = self.bit_position {
            params.insert("bit_position".to_string(), bit_pos.to_string());
        }
        if let Some(byte_order) = &self.byte_order {
            params.insert("byte_order".to_string(), byte_order.clone());
        }
        if let Some(reg_count) = self.register_count {
            params.insert("register_count".to_string(), reg_count.to_string());
        }

        params
    }

    fn data_format(&self) -> &str {
        &self.data_format
    }

    fn data_size(&self) -> u8 {
        self.register_count.unwrap_or(1) as u8
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
            number_of_bytes: None,
            bit_position: None,
            byte_order: Some("big".to_string()),
            register_count: Some(2),
            description: None,
        };

        let params = mapping.to_protocol_params();
        assert_eq!(params.get("slave_id").unwrap(), "1");
        assert_eq!(params.get("function_code").unwrap(), "3");
        assert_eq!(params.get("register_address").unwrap(), "1000");
        assert_eq!(params.get("byte_order").unwrap(), "big");
        assert_eq!(mapping.data_size(), 2);
    }
}
