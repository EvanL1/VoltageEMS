/// 转发计算配置管理 - Forward Calculation Configuration Management
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::utils::error::{ComSrvError, Result};
use crate::core::protocols::common::combase::TelemetryType;

/// 四遥点位标识符 - Four-Telemetry Point Identifier
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct TelemetryPointId {
    /// 四遥类型
    pub telemetry_type: TelemetryType,
    /// 点位ID
    pub point_id: u32,
}

impl TelemetryPointId {
    /// 创建新的四遥点位标识符
    pub fn new(telemetry_type: TelemetryType, point_id: u32) -> Self {
        Self {
            telemetry_type,
            point_id,
        }
    }

    /// 转换为字符串表示
    pub fn to_string(&self) -> String {
        format!("{}:{}", self.telemetry_type.english_name(), self.point_id)
    }

    /// 从字符串解析四遥点位标识符
    pub fn from_string(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(ComSrvError::ConfigError(
                "Invalid point ID format, expected 'type:id'".to_string(),
            ));
        }

        let telemetry_type = match parts[0].to_lowercase().as_str() {
            "telemetry" => TelemetryType::Telemetry,
            "signaling" => TelemetryType::Signaling,
            "control" => TelemetryType::Control,
            "setpoint" => TelemetryType::Setpoint,
            _ => return Err(ComSrvError::ConfigError(
                format!("Unknown telemetry type: {}", parts[0])
            )),
        };

        let point_id = parts[1].parse::<u32>()
            .map_err(|_| ComSrvError::ConfigError(
                format!("Invalid point ID: {}", parts[1])
            ))?;

        Ok(Self {
            telemetry_type,
            point_id,
        })
    }
}

/// 计算值类型 - 支持数值和布尔值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CalculationValue {
    /// 数值（用于遥测、遥调）
    Numeric(f64),
    /// 布尔值（用于遥信、遥控）
    Boolean(bool),
}

impl CalculationValue {
    /// 转换为数值
    pub fn as_numeric(&self) -> Option<f64> {
        match self {
            CalculationValue::Numeric(value) => Some(*value),
            CalculationValue::Boolean(value) => Some(if *value { 1.0 } else { 0.0 }),
        }
    }

    /// 转换为布尔值
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            CalculationValue::Boolean(value) => Some(*value),
            CalculationValue::Numeric(value) => Some(*value != 0.0),
        }
    }
}

/// 转发计算规则配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardCalculationRule {
    /// 规则ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 描述信息
    pub description: Option<String>,
    /// 是否启用
    pub enabled: bool,
    
    /// 目标点位
    pub target: TelemetryPointId,
    /// 目标点位名称（可选）
    pub target_name: Option<String>,
    /// 工程单位
    pub unit: Option<String>,
    
    /// 计算表达式
    pub expression: String,
    /// 源点位映射（变量名 -> 四遥点位）
    pub sources: HashMap<String, TelemetryPointId>,
    
    /// 计算优先级（越小优先级越高）
    pub priority: u32,
    /// 执行间隔（毫秒，覆盖全局设置）
    pub execution_interval_ms: Option<u64>,
    /// 规则组别
    pub group: Option<String>,
    /// 标签
    pub tags: Option<Vec<String>>,
}

impl ForwardCalculationRule {
    /// 验证规则配置
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(ComSrvError::ConfigError("Rule ID cannot be empty".to_string()));
        }
        if self.expression.is_empty() {
            return Err(ComSrvError::ConfigError("Expression cannot be empty".to_string()));
        }
        if self.sources.is_empty() {
            return Err(ComSrvError::ConfigError("Sources cannot be empty".to_string()));
        }
        Ok(())
    }
}

/// 虚拟通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualChannelConfig {
    /// 通道ID
    pub channel_id: String,
    /// 通道名称
    pub name: String,
    /// 描述信息
    pub description: Option<String>,
    /// 是否启用
    pub enabled: bool,
    
    /// 全局执行间隔（毫秒）
    pub global_execution_interval_ms: u64,
    /// 最大并发计算数
    pub max_concurrent_calculations: u32,
    /// 计算超时时间（毫秒）
    pub calculation_timeout_ms: u64,
    
    /// 转发计算规则列表
    pub rules: Vec<ForwardCalculationRule>,
    
    /// 创建时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    /// 更新时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

impl VirtualChannelConfig {
    /// 创建新的虚拟通道
    pub fn new(channel_id: String, name: String) -> Self {
        let now = Utc::now();
        Self {
            channel_id,
            name,
            description: None,
            enabled: true,
            global_execution_interval_ms: 1000,
            max_concurrent_calculations: 10,
            calculation_timeout_ms: 5000,
            rules: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        if self.channel_id.is_empty() {
            return Err(ComSrvError::ConfigError("Channel ID cannot be empty".to_string()));
        }
        
        // 验证所有规则
        for rule in &self.rules {
            rule.validate()?;
        }
        
        // 检查规则ID唯一性
        let mut rule_ids = std::collections::HashSet::new();
        for rule in &self.rules {
            if !rule_ids.insert(&rule.id) {
                return Err(ComSrvError::ConfigError(
                    format!("Duplicate rule ID: {}", rule.id)
                ));
            }
        }
        
        Ok(())
    }

    /// 获取启用的规则
    pub fn get_enabled_rules(&self) -> Vec<&ForwardCalculationRule> {
        self.rules.iter().filter(|r| r.enabled).collect()
    }
}

/// 转发计算配置文件的根结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardCalculationConfig {
    /// 配置文件版本
    pub version: String,
    /// 配置描述
    pub description: Option<String>,
    /// 配置作者
    pub author: Option<String>,
    
    /// 全局设置
    pub global: GlobalConfig,
    /// 虚拟通道列表
    pub virtual_channels: Vec<VirtualChannelConfig>,
    
    /// 配置创建时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    /// 配置更新时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

/// 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// 默认执行间隔（毫秒）
    pub default_execution_interval_ms: u64,
    /// 默认计算超时（毫秒）
    pub default_calculation_timeout_ms: u64,
    /// 启用调试模式
    pub debug_mode: bool,
    /// 日志级别
    pub log_level: String,
    /// 计算结果历史保留数量
    pub max_result_history: usize,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_execution_interval_ms: 1000,
            default_calculation_timeout_ms: 5000,
            debug_mode: false,
            log_level: "info".to_string(),
            max_result_history: 1000,
        }
    }
}

impl ForwardCalculationConfig {
    /// 创建新的配置
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            version: "1.0.0".to_string(),
            description: Some("VoltageEMS Forward Calculation Configuration".to_string()),
            author: None,
            global: GlobalConfig::default(),
            virtual_channels: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 从YAML文件加载
    pub fn load_from_yaml<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to read config file: {}", e)))?;
        
        let config: Self = serde_yaml::from_str(&content)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse YAML: {}", e)))?;
        
        config.validate()?;
        Ok(config)
    }

    /// 保存到YAML文件
    pub fn save_to_yaml<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        self.validate()?;
        
        let content = serde_yaml::to_string(self)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize YAML: {}", e)))?;
        
        std::fs::write(path, content)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }

    /// 验证整个配置
    pub fn validate(&self) -> Result<()> {
        for channel in &self.virtual_channels {
            channel.validate()?;
        }
        
        // 检查虚拟通道ID唯一性
        let mut channel_ids = std::collections::HashSet::new();
        for channel in &self.virtual_channels {
            if !channel_ids.insert(&channel.channel_id) {
                return Err(ComSrvError::ConfigError(
                    format!("Duplicate virtual channel ID: {}", channel.channel_id)
                ));
            }
        }
        
        Ok(())
    }

    /// 添加虚拟通道
    pub fn add_virtual_channel(&mut self, channel: VirtualChannelConfig) -> Result<()> {
        // 检查ID是否重复
        if self.virtual_channels.iter().any(|c| c.channel_id == channel.channel_id) {
            return Err(ComSrvError::ConfigError(
                format!("Virtual channel ID '{}' already exists", channel.channel_id)
            ));
        }
        
        channel.validate()?;
        self.virtual_channels.push(channel);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 获取虚拟通道
    pub fn get_virtual_channel(&self, channel_id: &str) -> Option<&VirtualChannelConfig> {
        self.virtual_channels.iter().find(|c| c.channel_id == channel_id)
    }

    /// 获取启用的虚拟通道
    pub fn get_enabled_virtual_channels(&self) -> Vec<&VirtualChannelConfig> {
        self.virtual_channels.iter().filter(|c| c.enabled).collect()
    }
}

impl Default for ForwardCalculationConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_point_id() {
        let point_id = TelemetryPointId::new(TelemetryType::Telemetry, 1001);
        assert_eq!(point_id.to_string(), "telemetry:1001");
        
        let parsed = TelemetryPointId::from_string("signaling:2002").unwrap();
        assert_eq!(parsed.telemetry_type, TelemetryType::Signaling);
        assert_eq!(parsed.point_id, 2002);
    }

    #[test]
    fn test_config_validation() {
        let mut config = ForwardCalculationConfig::new();
        assert!(config.validate().is_ok());
        
        // 测试重复的通道ID
        let channel1 = VirtualChannelConfig::new("test".to_string(), "Test 1".to_string());
        let channel2 = VirtualChannelConfig::new("test".to_string(), "Test 2".to_string());
        
        assert!(config.add_virtual_channel(channel1).is_ok());
        assert!(config.add_virtual_channel(channel2).is_err());
    }
} 