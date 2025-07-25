//! 配置管理核心模块
//!
//! 整合配置中心、配置管理器和统一加载器的功能

use super::types::{AppConfig, ChannelConfig, CombinedPoint, ServiceConfig, TableConfig};
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use csv::ReaderBuilder;
use figment::{
    providers::{Env, Format, Json, Toml, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

// ============================================================================
// 配置中心集成
// ============================================================================

/// 配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub version: String,
    pub checksum: String,
    pub last_modified: String,
    pub content: serde_json::Value,
}

/// 配置项响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItemResponse {
    pub key: String,
    pub value: serde_json::Value,
    #[serde(rename = "type")]
    pub value_type: String,
}

/// 配置变更事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub event: String,
    pub service: String,
    pub keys: Vec<String>,
    pub version: String,
}

/// 配置源trait
#[async_trait]
pub trait ConfigSource: Send + Sync {
    /// 获取完整配置
    async fn fetch_config(&self, service_name: &str) -> Result<ConfigResponse>;

    /// 获取特定配置项
    async fn fetch_item(&self, service_name: &str, key: &str) -> Result<ConfigItemResponse>;

    /// 获取源名称
    fn name(&self) -> &str;
}

/// 配置中心客户端
#[derive(Debug, Clone)]
pub struct ConfigCenterClient {
    pub service_name: String,
    pub config_center_url: Option<String>,
    pub fallback_path: Option<String>,
    pub cache_duration: u64,
}

impl ConfigCenterClient {
    /// 从环境变量创建客户端
    pub fn from_env(service_name: String) -> Self {
        let config_center_url = std::env::var("CONFIG_CENTER_URL").ok();
        let cache_duration = std::env::var("CONFIG_CACHE_DURATION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300); // 5 minutes default

        Self {
            service_name,
            config_center_url,
            fallback_path: None,
            cache_duration,
        }
    }

    /// 设置后备配置路径
    pub fn with_fallback(mut self, path: String) -> Self {
        self.fallback_path = Some(path);
        self
    }

    /// 获取配置（带缓存和后备）
    pub async fn get_config(&self) -> Result<Option<serde_json::Value>> {
        if let Some(url) = &self.config_center_url {
            debug!("Fetching config from config center: {}", url);
            // TODO: 实际的配置中心实现
            // 这里返回None表示从配置中心获取失败，使用本地配置
            Ok(None)
        } else {
            Ok(None)
        }
    }
}

// ============================================================================
// 配置管理器
// ============================================================================

/// 配置管理器
#[derive(Debug)]
pub struct ConfigManager {
    /// 加载的应用配置
    config: AppConfig,
    /// Figment实例用于重新加载
    #[allow(dead_code)]
    figment: Figment,
    /// 配置中心客户端
    #[allow(dead_code)]
    config_center: Option<ConfigCenterClient>,
}

impl ConfigManager {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let service_name = "comsrv";

        // 初始化配置中心客户端
        let config_center = if std::env::var("CONFIG_CENTER_URL").is_ok() {
            info!("Config center URL detected, initializing client");
            Some(
                ConfigCenterClient::from_env(service_name.to_string())
                    .with_fallback(path.to_string_lossy().to_string()),
            )
        } else {
            None
        };

        // 尝试从配置中心加载
        let mut figment = Figment::new();
        let mut from_config_center = false;

        if let Some(ref cc_client) = config_center {
            let runtime = tokio::runtime::Handle::try_current()
                .unwrap_or_else(|_| tokio::runtime::Runtime::new().unwrap().handle().clone());

            if let Ok(Some(remote_config)) = runtime.block_on(cc_client.get_config()) {
                info!("Successfully loaded configuration from config center");
                figment = figment.merge(Json::string(&remote_config.to_string()));
                from_config_center = true;
            }
        }

        // 如果没有从配置中心加载，则从文件加载
        if !from_config_center {
            let extension = path
                .extension()
                .and_then(|s| s.to_str())
                .ok_or_else(|| ComSrvError::ConfigError("Invalid file extension".to_string()))?;

            figment = match extension {
                "json" => figment.merge(Json::file(path)),
                "toml" => figment.merge(Toml::file(path)),
                "yaml" | "yml" => figment.merge(Yaml::file(path)),
                _ => {
                    return Err(ComSrvError::ConfigError(format!(
                        "Unsupported config format: {}",
                        extension
                    )))
                }
            };
        }

        // 合并环境变量
        figment = figment.merge(Env::prefixed("COMSRV_"));

        // 解析配置
        let mut config: AppConfig = figment
            .extract()
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse config: {}", e)))?;

        // 加载CSV配置
        let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
        Self::load_csv_configs(&mut config, config_dir)?;

        Ok(Self {
            config,
            figment,
            config_center,
        })
    }

    /// 加载CSV配置
    fn load_csv_configs(config: &mut AppConfig, config_dir: &Path) -> Result<()> {
        for channel in &mut config.channels {
            if let Some(ref table_config) = channel.table_config {
                debug!("Loading CSV for channel {}", channel.id);
                match CsvLoader::load_channel_tables(table_config, config_dir) {
                    Ok(points) => {
                        info!(
                            "Loaded {} four remote points for channel {}",
                            points.len(),
                            channel.id
                        );
                        // 将点位分别添加到对应的HashMap
                        for point in points {
                            if let Err(e) = channel.add_point(point) {
                                warn!("Failed to add point: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to load CSV for channel {}: {}", channel.id, e);
                    }
                }
            }
        }
        Ok(())
    }

    /// 获取服务配置
    pub fn service_config(&self) -> &ServiceConfig {
        &self.config.service
    }

    /// 获取所有通道配置
    pub fn channels(&self) -> &[ChannelConfig] {
        &self.config.channels
    }

    /// 根据ID获取通道配置
    pub fn get_channel(&self, channel_id: u16) -> Option<&ChannelConfig> {
        self.config.channels.iter().find(|c| c.id == channel_id)
    }

    /// 获取通道数量
    pub fn channel_count(&self) -> usize {
        self.config.channels.len()
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 检查通道ID唯一性
        let mut channel_ids = std::collections::HashSet::new();
        for channel in &self.config.channels {
            if !channel_ids.insert(channel.id) {
                return Err(ComSrvError::ConfigError(format!(
                    "Duplicate channel ID: {}",
                    channel.id
                )));
            }
        }

        Ok(())
    }

    // 四遥分离架构下，不再需要统一映射方法
}

// ============================================================================
// CSV加载器
// ============================================================================

/// 统一的CSV加载器
#[derive(Debug)]
pub struct CsvLoader;

/// 四遥点位
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourRemotePoint {
    pub point_id: u32,
    pub signal_name: String,
    pub telemetry_type: String,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    pub unit: Option<String>,
    pub reverse: Option<bool>,
    pub data_type: String,
}

/// Modbus映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub bit_position: Option<u8>,
    pub data_format: Option<String>,
    pub register_count: Option<u16>,
}

impl CsvLoader {
    /// 加载通道的所有CSV表
    pub fn load_channel_tables(
        table_config: &TableConfig,
        config_dir: &Path,
    ) -> Result<Vec<CombinedPoint>> {
        info!("Loading CSV tables for channel");

        // 按四遥类型分别存储点位
        let mut measurement_points = HashMap::new();
        let mut signal_points = HashMap::new();
        let mut control_points = HashMap::new();
        let mut adjustment_points = HashMap::new();

        // 检查环境变量覆盖
        let base_dir = std::env::var("COMSRV_CSV_BASE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| config_dir.to_path_buf());

        debug!("Using CSV base directory: {}", base_dir.display());

        // 加载遥测文件
        let base_path = base_dir.join(&table_config.four_remote_route);

        // 遥测
        if let Some(measurement_data) = Self::load_measurement_file(
            &base_path.join(&table_config.four_remote_files.measurement_file),
            "Measurement",
        )? {
            for point in measurement_data {
                measurement_points.insert(point.point_id, point);
            }
        }

        // 遥信
        if let Some(signal_data) = Self::load_signal_file(
            &base_path.join(&table_config.four_remote_files.signal_file),
            "Signal",
        )? {
            for point in signal_data {
                signal_points.insert(point.point_id, point);
            }
        }

        // 遥调
        if let Some(adjustment_data) = Self::load_measurement_file(
            &base_path.join(&table_config.four_remote_files.adjustment_file),
            "Adjustment",
        )? {
            for point in adjustment_data {
                adjustment_points.insert(point.point_id, point);
            }
        }

        // 遥控
        if let Some(control_data) = Self::load_signal_file(
            &base_path.join(&table_config.four_remote_files.control_file),
            "Control",
        )? {
            for point in control_data {
                control_points.insert(point.point_id, point);
            }
        }

        // 加载协议映射
        let protocol_path = base_dir.join(&table_config.protocol_mapping_route);

        // 为每种遥测类型分别加载对应的映射文件，避免点位ID冲突
        let mut combined = Vec::new();

        // 合并遥测点位
        if let Ok(measurement_mappings) = Self::load_protocol_mappings(
            &protocol_path.join(&table_config.protocol_mapping_file.measurement_mapping),
        ) {
            debug!("Loaded {} measurement mappings", measurement_mappings.len());
            let measurement_combined = Self::combine_points_by_type(
                measurement_points,
                &measurement_mappings,
                "Measurement",
            )?;
            combined.extend(measurement_combined);
        }

        // 合并遥信点位
        if let Ok(signal_mappings) = Self::load_protocol_mappings(
            &protocol_path.join(&table_config.protocol_mapping_file.signal_mapping),
        ) {
            debug!("Loaded {} signal mappings", signal_mappings.len());
            let signal_combined =
                Self::combine_points_by_type(signal_points, &signal_mappings, "Signal")?;
            combined.extend(signal_combined);
        }

        // 合并遥调点位
        if let Ok(adjustment_mappings) = Self::load_protocol_mappings(
            &protocol_path.join(&table_config.protocol_mapping_file.adjustment_mapping),
        ) {
            debug!("Loaded {} adjustment mappings", adjustment_mappings.len());
            let adjustment_combined = Self::combine_points_by_type(
                adjustment_points,
                &adjustment_mappings,
                "Adjustment",
            )?;
            combined.extend(adjustment_combined);
        }

        // 合并遥控点位
        if let Ok(control_mappings) = Self::load_protocol_mappings(
            &protocol_path.join(&table_config.protocol_mapping_file.control_mapping),
        ) {
            debug!("Loaded {} control mappings", control_mappings.len());
            let control_combined =
                Self::combine_points_by_type(control_points, &control_mappings, "Control")?;
            combined.extend(control_combined);
        }

        Ok(combined)
    }

    /// 加载遥测文件（带缩放参数）
    fn load_measurement_file(
        path: &Path,
        telemetry_type: &str,
    ) -> Result<Option<Vec<FourRemotePoint>>> {
        if !path.exists() {
            debug!("File not found: {}, skipping", path.display());
            return Ok(None);
        }

        debug!("Loading {} file: {}", telemetry_type, path.display());

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {}", e)))?;

        let mut points = Vec::new();

        for result in reader.records() {
            let record = result
                .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV record: {}", e)))?;

            let point = FourRemotePoint {
                point_id: record
                    .get(0)
                    .ok_or_else(|| ComSrvError::ConfigError("Missing point_id".to_string()))?
                    .parse()
                    .map_err(|_| ComSrvError::ConfigError("Invalid point_id".to_string()))?,
                signal_name: record.get(1).unwrap_or("Unknown").to_string(),
                telemetry_type: telemetry_type.to_string(),
                scale: record.get(2).and_then(|s| s.parse().ok()),
                offset: record.get(3).and_then(|s| s.parse().ok()),
                unit: record.get(4).map(|s| s.to_string()),
                reverse: record.get(5).and_then(|s| s.parse().ok()),
                data_type: record.get(6).unwrap_or("float").to_string(),
            };

            points.push(point);
        }

        debug!("Loaded {} {} points", points.len(), telemetry_type);
        Ok(Some(points))
    }

    /// 加载信号文件（不带缩放参数）
    fn load_signal_file(path: &Path, telemetry_type: &str) -> Result<Option<Vec<FourRemotePoint>>> {
        if !path.exists() {
            debug!("File not found: {}, skipping", path.display());
            return Ok(None);
        }

        debug!("Loading {} file: {}", telemetry_type, path.display());

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {}", e)))?;

        let mut points = Vec::new();

        for result in reader.records() {
            let record = result
                .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV record: {}", e)))?;

            let point = FourRemotePoint {
                point_id: record
                    .get(0)
                    .ok_or_else(|| ComSrvError::ConfigError("Missing point_id".to_string()))?
                    .parse()
                    .map_err(|_| ComSrvError::ConfigError("Invalid point_id".to_string()))?,
                signal_name: record.get(1).unwrap_or("Unknown").to_string(),
                telemetry_type: telemetry_type.to_string(),
                scale: None,
                offset: None,
                unit: None,
                reverse: record.get(2).and_then(|s| s.parse().ok()),
                data_type: "bool".to_string(),
            };

            points.push(point);
        }

        debug!("Loaded {} {} points", points.len(), telemetry_type);
        Ok(Some(points))
    }

    /// 加载协议映射
    fn load_protocol_mappings(path: &Path) -> Result<HashMap<u32, HashMap<String, String>>> {
        if !path.exists() {
            return Err(ComSrvError::ConfigError(format!(
                "Protocol mapping file not found: {}",
                path.display()
            )));
        }

        debug!("Loading protocol mappings: {}", path.display());

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {}", e)))?;

        let mut mappings = HashMap::new();

        for result in reader.records() {
            let record = result
                .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV record: {}", e)))?;

            let point_id: u32 = record
                .get(0)
                .ok_or_else(|| ComSrvError::ConfigError("Missing point_id".to_string()))?
                .parse()
                .map_err(|_| ComSrvError::ConfigError("Invalid point_id".to_string()))?;

            let mut params = HashMap::new();

            // Modbus参数
            if let (Some(slave_id), Some(function_code), Some(register_address)) =
                (record.get(1), record.get(2), record.get(3))
            {
                params.insert("slave_id".to_string(), slave_id.to_string());
                params.insert("function_code".to_string(), function_code.to_string());
                params.insert("register_address".to_string(), register_address.to_string());

                // 可选参数
                if let Some(bit_position) = record.get(4) {
                    if !bit_position.is_empty() {
                        params.insert("bit_position".to_string(), bit_position.to_string());
                    }
                }
                if let Some(data_format) = record.get(5) {
                    if !data_format.is_empty() {
                        params.insert("data_format".to_string(), data_format.to_string());
                    }
                }
                if let Some(register_count) = record.get(6) {
                    if !register_count.is_empty() {
                        params.insert("register_count".to_string(), register_count.to_string());
                    }
                }
            }

            mappings.insert(point_id, params);
        }

        debug!("Loaded {} protocol mappings", mappings.len());
        Ok(mappings)
    }

    /// 合并点位信息
    fn combine_points(
        measurement: HashMap<u32, FourRemotePoint>,
        protocol_mappings: HashMap<u32, HashMap<String, String>>,
    ) -> Result<Vec<CombinedPoint>> {
        let mut combined = Vec::new();

        for (point_id, measurement_point) in measurement {
            if let Some(protocol_params) = protocol_mappings.get(&point_id) {
                let point = CombinedPoint {
                    point_id,
                    signal_name: measurement_point.signal_name,
                    telemetry_type: measurement_point.telemetry_type,
                    data_type: measurement_point.data_type,
                    protocol_params: protocol_params.clone(),
                    scaling: if measurement_point.scale.is_some()
                        || measurement_point.offset.is_some()
                    {
                        Some(super::types::ScalingInfo {
                            scale: measurement_point.scale.unwrap_or(1.0),
                            offset: measurement_point.offset.unwrap_or(0.0),
                            unit: measurement_point.unit,
                            reverse: None, // TODO: Load from CSV if needed
                        })
                    } else {
                        None
                    },
                };
                combined.push(point);
            } else {
                warn!("No protocol mapping found for point_id: {}", point_id);
            }
        }

        info!("Combined {} points with protocol mappings", combined.len());
        Ok(combined)
    }

    /// 按类型合并点位信息，保持四遥分离
    fn combine_points_by_type(
        measurement_points: HashMap<u32, FourRemotePoint>,
        protocol_mappings: &HashMap<u32, HashMap<String, String>>,
        telemetry_type: &str,
    ) -> Result<Vec<CombinedPoint>> {
        let mut combined = Vec::new();

        for (point_id, measurement_point) in measurement_points {
            if let Some(protocol_params) = protocol_mappings.get(&point_id) {
                let point = CombinedPoint {
                    point_id,
                    signal_name: measurement_point.signal_name,
                    telemetry_type: measurement_point.telemetry_type,
                    data_type: measurement_point.data_type,
                    protocol_params: protocol_params.clone(),
                    scaling: if measurement_point.scale.is_some()
                        || measurement_point.offset.is_some()
                        || measurement_point.reverse.is_some()
                    {
                        Some(super::types::ScalingInfo {
                            scale: measurement_point.scale.unwrap_or(1.0),
                            offset: measurement_point.offset.unwrap_or(0.0),
                            unit: measurement_point.unit,
                            reverse: measurement_point.reverse,
                        })
                    } else {
                        None
                    },
                };
                combined.push(point);
            } else {
                debug!(
                    "No protocol mapping found for {} point_id: {}",
                    telemetry_type, point_id
                );
            }
        }

        debug!(
            "Combined {} {} points with protocol mappings",
            combined.len(),
            telemetry_type
        );
        Ok(combined)
    }
}

// ============================================================================
// 文件系统配置源实现
// ============================================================================

/// 文件系统配置源
pub struct FileSystemSource {
    base_path: String,
}

impl FileSystemSource {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }
}

#[async_trait]
impl ConfigSource for FileSystemSource {
    async fn fetch_config(&self, service_name: &str) -> Result<ConfigResponse> {
        let path = Path::new(&self.base_path).join(format!("{}.yml", service_name));

        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| ComSrvError::IoError(format!("Failed to read config file: {}", e)))?;

        let value: serde_json::Value = serde_yaml::from_str(&content)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse YAML: {}", e)))?;

        Ok(ConfigResponse {
            version: "1.0.0".to_string(),
            checksum: format!("{:x}", md5::compute(&content)),
            last_modified: std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "Unknown".to_string()),
            content: value,
        })
    }

    async fn fetch_item(&self, service_name: &str, key: &str) -> Result<ConfigItemResponse> {
        let config = self.fetch_config(service_name).await?;

        let value = config
            .content
            .get(key)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Key '{}' not found", key)))?
            .clone();

        Ok(ConfigItemResponse {
            key: key.to_string(),
            value_type: match &value {
                serde_json::Value::Bool(_) => "bool",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::Object(_) => "object",
                serde_json::Value::Null => "null",
            }
            .to_string(),
            value,
        })
    }

    fn name(&self) -> &str {
        "filesystem"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_center_client_creation() {
        let client = ConfigCenterClient::from_env("test_service".to_string());
        assert_eq!(client.service_name, "test_service");
        assert_eq!(client.cache_duration, 300);
    }

    #[tokio::test]
    async fn test_filesystem_source() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().unwrap();
        let config_path = dir.path().join("test.yml");

        let config_content = r#"
service:
  name: test
  version: 1.0.0
"#;

        fs::write(&config_path, config_content).await.unwrap();

        let source = FileSystemSource::new(dir.path().to_string_lossy().to_string());
        let config = source.fetch_config("test").await.unwrap();

        assert_eq!(config.version, "1.0.0");
        assert!(config.content.get("service").is_some());
    }
}
