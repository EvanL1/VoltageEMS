//! 配置类型定义
//!
//! 包含所有配置相关的类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

// ============================================================================
// 应用配置
// ============================================================================

/// 应用配置根结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 服务配置
    pub service: ServiceConfig,

    /// 通道配置列表
    pub channels: Vec<ChannelConfig>,
}

/// 服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// 服务名称
    pub name: String,

    /// 服务版本
    pub version: Option<String>,

    /// 服务描述
    pub description: Option<String>,

    /// API配置
    #[serde(default)]
    pub api: ApiConfig,

    /// Redis配置
    #[serde(default)]
    pub redis: RedisConfig,

    /// 日志配置
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// API配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// 监听地址
    #[serde(default = "default_api_host")]
    pub host: String,

    /// 监听端口
    #[serde(default = "default_api_port")]
    pub port: u16,

    /// 工作线程数
    #[serde(default = "default_workers")]
    pub workers: usize,
}

/// Redis配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// 连接池大小
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,

    /// 连接超时（毫秒）
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub level: String,

    /// 日志格式
    #[serde(default = "default_log_format")]
    pub format: String,

    /// 日志文件路径
    pub file: Option<PathBuf>,

    /// 是否输出到控制台
    #[serde(default = "default_true")]
    pub console: bool,

    /// 日志轮转配置
    #[serde(default)]
    pub rotation: LogRotationConfig,
}

/// 日志轮转配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    /// 轮转策略
    #[serde(default = "default_rotation_strategy")]
    pub strategy: String,

    /// 最大文件大小（MB）
    #[serde(default = "default_max_size")]
    pub max_size_mb: u64,

    /// 保留文件数
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

// ============================================================================
// 通道配置
// ============================================================================

/// 通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// 通道ID
    pub id: u16,

    /// 通道名称
    pub name: String,

    /// 描述
    pub description: Option<String>,

    /// 协议类型
    pub protocol: String,

    /// 协议参数（通用HashMap存储）
    #[serde(default)]
    pub parameters: HashMap<String, serde_yaml::Value>,

    /// 通道日志配置
    #[serde(default)]
    pub logging: ChannelLoggingConfig,

    /// 表配置
    pub table_config: Option<TableConfig>,

    /// 解析后的点位映射
    #[serde(skip)]
    pub points: Vec<UnifiedPointMapping>,

    /// 四遥点位映射 - 分别存储四种遥测类型
    #[serde(skip)]
    pub measurement_points: HashMap<u32, CombinedPoint>,
    #[serde(skip)]
    pub signal_points: HashMap<u32, CombinedPoint>,
    #[serde(skip)]
    pub control_points: HashMap<u32, CombinedPoint>,
    #[serde(skip)]
    pub adjustment_points: HashMap<u32, CombinedPoint>,
}

/// 通道日志配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelLoggingConfig {
    /// 是否启用
    #[serde(default)]
    pub enabled: bool,

    /// 日志级别
    pub level: Option<String>,

    /// 日志文件
    pub file: Option<String>,

    /// 是否包含协议细节
    #[serde(default)]
    pub protocol_details: bool,
}

/// 表配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableConfig {
    /// 四遥路径
    pub four_telemetry_route: String,

    /// 四遥文件
    pub four_telemetry_files: FourTelemetryFiles,

    /// 协议映射路径
    pub protocol_mapping_route: String,

    /// 协议映射文件
    pub protocol_mapping_file: ProtocolMappingFiles,
}

/// 四遥文件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryFiles {
    /// 遥测文件
    pub telemetry_file: String,

    /// 遥信文件
    pub signal_file: String,

    /// 遥调文件
    pub adjustment_file: String,

    /// 遥控文件
    pub control_file: String,
}

/// 协议映射文件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMappingFiles {
    /// 遥测映射文件
    pub telemetry_mapping: String,

    /// 遥信映射文件
    pub signal_mapping: String,

    /// 遥调映射文件
    pub adjustment_mapping: String,

    /// 遥控映射文件
    pub control_mapping: String,
}

/// 合并的点位
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedPoint {
    pub point_id: u32,
    pub signal_name: String,
    pub telemetry_type: String,
    pub data_type: String,
    pub protocol_params: HashMap<String, String>,
    pub scaling: Option<ScalingInfo>,
}

/// 缩放信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingInfo {
    pub scale: f64,
    pub offset: f64,
    pub unit: Option<String>,
    pub reverse: Option<bool>,
}

// ============================================================================
// 协议配置
// ============================================================================

/// 统一点位映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPointMapping {
    /// 点位ID
    pub point_id: u32,

    /// 信号名称
    pub signal_name: String,

    /// 遥测类型
    pub telemetry_type: String,

    /// 数据类型
    pub data_type: String,

    /// 协议特定参数
    pub protocol_params: HashMap<String, String>,

    /// 缩放信息
    pub scaling: Option<ScalingParams>,
}

/// 缩放参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingParams {
    pub scale: f64,
    pub offset: f64,
    pub unit: Option<String>,
    pub reverse: Option<bool>,
}

/// 协议映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub protocol_params: HashMap<String, String>,
}

/// 遥测类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TelemetryType {
    /// 遥测 (YC)
    #[serde(rename = "m")]
    Telemetry,
    /// 遥信 (YX)
    #[serde(rename = "s")]
    Signal,
    /// 遥控 (YK)
    #[serde(rename = "c")]
    Control,
    /// 遥调 (YT)
    #[serde(rename = "a")]
    Adjustment,
}

impl std::str::FromStr for TelemetryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "m" | "telemetry" | "Telemetry" => Ok(TelemetryType::Telemetry),
            "s" | "signal" | "Signal" => Ok(TelemetryType::Signal),
            "c" | "control" | "Control" => Ok(TelemetryType::Control),
            "a" | "adjustment" | "Adjustment" => Ok(TelemetryType::Adjustment),
            _ => Err(format!("Invalid telemetry type: {}", s)),
        }
    }
}

/// 协议类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolType {
    ModbusTcp,
    ModbusRtu,
    Can,
    Iec60870,
    Virtual,
}

impl std::str::FromStr for ProtocolType {
    type Err = crate::utils::error::ComSrvError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 使用normalize_protocol_name函数来实现大小写不敏感的匹配
        let normalized = crate::utils::normalize_protocol_name(s);
        match normalized.as_str() {
            "modbus_tcp" => Ok(ProtocolType::ModbusTcp),
            "modbus_rtu" => Ok(ProtocolType::ModbusRtu),
            "can" => Ok(ProtocolType::Can),
            "iec60870" => Ok(ProtocolType::Iec60870),
            "virtual" => Ok(ProtocolType::Virtual),
            _ => Err(crate::utils::error::ComSrvError::ConfigError(format!(
                "Unknown protocol type: {}",
                s
            ))),
        }
    }
}

// ============================================================================
// 默认值函数
// ============================================================================

fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_api_port() -> u16 {
    3000
}

fn default_workers() -> usize {
    num_cpus::get()
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

fn default_pool_size() -> u32 {
    10
}

fn default_timeout() -> u64 {
    5000
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "pretty".to_string()
}

fn default_rotation_strategy() -> String {
    "daily".to_string()
}

fn default_max_size() -> u64 {
    100
}

fn default_max_files() -> u32 {
    7
}

// ============================================================================
// 实现
// ============================================================================

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_api_port(),
            workers: default_workers(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            pool_size: default_pool_size(),
            timeout_ms: default_timeout(),
            enabled: default_true(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            file: None,
            console: default_true(),
            rotation: LogRotationConfig::default(),
        }
    }
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            strategy: default_rotation_strategy(),
            max_size_mb: default_max_size(),
            max_files: default_max_files(),
        }
    }
}

impl ChannelConfig {
    /// 获取参数值
    pub fn get_parameter(&self, key: &str) -> Option<&serde_yaml::Value> {
        self.parameters.get(key)
    }

    /// 获取字符串参数
    pub fn get_string_parameter(&self, key: &str) -> Option<String> {
        self.parameters
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// 获取整数参数
    pub fn get_int_parameter(&self, key: &str) -> Option<i64> {
        self.parameters.get(key).and_then(|v| v.as_i64())
    }

    /// 获取布尔参数
    pub fn get_bool_parameter(&self, key: &str) -> Option<bool> {
        self.parameters.get(key).and_then(|v| v.as_bool())
    }

    /// 根据遥测类型获取点位
    pub fn get_point(
        &self,
        telemetry_type: TelemetryType,
        point_id: u32,
    ) -> Option<&CombinedPoint> {
        match telemetry_type {
            TelemetryType::Telemetry => self.measurement_points.get(&point_id),
            TelemetryType::Signal => self.signal_points.get(&point_id),
            TelemetryType::Control => self.control_points.get(&point_id),
            TelemetryType::Adjustment => self.adjustment_points.get(&point_id),
        }
    }

    /// 添加点位到对应的HashMap
    pub fn add_point(&mut self, point: CombinedPoint) -> Result<(), String> {
        let telemetry_type = TelemetryType::from_str(&point.telemetry_type)
            .map_err(|e| format!("Invalid telemetry type: {}", e))?;

        let target_hashmap = match telemetry_type {
            TelemetryType::Telemetry => &mut self.measurement_points,
            TelemetryType::Signal => &mut self.signal_points,
            TelemetryType::Control => &mut self.control_points,
            TelemetryType::Adjustment => &mut self.adjustment_points,
        };

        target_hashmap.insert(point.point_id, point);
        Ok(())
    }

    /// 获取所有点位数量
    pub fn get_total_points_count(&self) -> usize {
        self.measurement_points.len()
            + self.signal_points.len()
            + self.control_points.len()
            + self.adjustment_points.len()
    }

    /// 获取指定类型的所有点位
    pub fn get_points_by_type(
        &self,
        telemetry_type: TelemetryType,
    ) -> &HashMap<u32, CombinedPoint> {
        match telemetry_type {
            TelemetryType::Telemetry => &self.measurement_points,
            TelemetryType::Signal => &self.signal_points,
            TelemetryType::Control => &self.control_points,
            TelemetryType::Adjustment => &self.adjustment_points,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let api = ApiConfig::default();
        assert_eq!(api.host, "0.0.0.0");
        assert_eq!(api.port, 3000);

        let redis = RedisConfig::default();
        assert_eq!(redis.url, "redis://127.0.0.1:6379");
        assert!(redis.enabled);

        let logging = LoggingConfig::default();
        assert_eq!(logging.level, "info");
        assert!(logging.console);
    }

    #[test]
    fn test_channel_config_parameters() {
        let mut channel = ChannelConfig {
            id: 1,
            name: "Test".to_string(),
            description: None,
            protocol: "modbus".to_string(),
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
            table_config: None,
            points: vec![],
            measurement_points: HashMap::new(),
            signal_points: HashMap::new(),
            control_points: HashMap::new(),
            adjustment_points: HashMap::new(),
        };

        // 添加参数
        channel.parameters.insert(
            "host".to_string(),
            serde_yaml::Value::String("192.168.1.1".to_string()),
        );
        channel.parameters.insert(
            "port".to_string(),
            serde_yaml::Value::Number(serde_yaml::Number::from(502)),
        );
        channel
            .parameters
            .insert("enabled".to_string(), serde_yaml::Value::Bool(true));

        // 测试获取参数
        assert_eq!(
            channel.get_string_parameter("host"),
            Some("192.168.1.1".to_string())
        );
        assert_eq!(channel.get_int_parameter("port"), Some(502));
        assert_eq!(channel.get_bool_parameter("enabled"), Some(true));
        assert_eq!(channel.get_string_parameter("missing"), None);
    }
}
