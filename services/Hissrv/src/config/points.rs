use crate::error::{HisSrvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// 策略类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyType {
    /// 允许所有点位（默认保存）
    AllowAll,
    /// 拒绝所有点位（默认不保存）
    DenyAll,
}

impl Default for PolicyType {
    fn default() -> Self {
        PolicyType::AllowAll
    }
}

/// 过滤器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FilterRule {
    /// 值范围过滤
    ValueRange {
        point_types: Vec<String>,
        min_value: Option<f64>,
        max_value: Option<f64>,
    },
    /// 时间间隔过滤
    TimeInterval {
        point_types: Option<Vec<String>>,
        min_interval_seconds: u64,
    },
    /// 质量过滤
    Quality {
        point_types: Option<Vec<String>>,
        min_quality: Option<u8>,
    },
}

/// 通道级别规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRule {
    /// 通道ID
    pub channel_id: u32,
    /// 是否启用保存
    pub enabled: bool,
    /// 允许的点位类型 (m=测量, s=信号, c=控制, a=调节)
    pub point_types: Option<Vec<String>>,
    /// 通道名称描述
    pub name: Option<String>,
}

/// 点位级别规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointRule {
    /// 通道ID
    pub channel_id: u32,
    /// 点位ID
    pub point_id: u32,
    /// 点位类型
    pub point_type: String,
    /// 是否启用保存
    pub enabled: bool,
    /// 点位名称描述
    pub name: Option<String>,
}

/// 存储规则
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageRules {
    /// 通道级别规则
    #[serde(default)]
    pub channels: Vec<ChannelRule>,
    /// 点位级别规则
    #[serde(default)]
    pub points: Vec<PointRule>,
}

/// 点位存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointStorageConfig {
    /// 全局启用/禁用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 默认策略
    #[serde(default)]
    pub default_policy: PolicyType,
    /// 存储规则
    #[serde(default)]
    pub rules: StorageRules,
    /// 过滤器
    #[serde(default)]
    pub filters: Vec<FilterRule>,
}

fn default_enabled() -> bool {
    true
}

impl Default for PointStorageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_policy: PolicyType::AllowAll,
            rules: StorageRules::default(),
            filters: Vec::new(),
        }
    }
}

impl PointStorageConfig {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        if !path.as_ref().exists() {
            tracing::info!("点位配置文件不存在，使用默认配置");
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| HisSrvError::config(format!("读取配置文件失败: {}", e)))?;

        let config: Self = serde_yaml::from_str(&content)
            .map_err(|e| HisSrvError::config(format!("解析配置文件失败: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    /// 验证配置有效性
    pub fn validate(&self) -> Result<()> {
        // 验证通道规则
        for rule in &self.rules.channels {
            if rule.channel_id == 0 {
                return Err(HisSrvError::config("通道ID不能为0"));
            }
            
            if let Some(ref point_types) = rule.point_types {
                for point_type in point_types {
                    if !Self::is_valid_point_type(point_type) {
                        return Err(HisSrvError::config(format!("无效的点位类型: {}", point_type)));
                    }
                }
            }
        }

        // 验证点位规则
        for rule in &self.rules.points {
            if rule.channel_id == 0 {
                return Err(HisSrvError::config("通道ID不能为0"));
            }
            if rule.point_id == 0 {
                return Err(HisSrvError::config("点位ID不能为0"));
            }
            if !Self::is_valid_point_type(&rule.point_type) {
                return Err(HisSrvError::config(format!("无效的点位类型: {}", rule.point_type)));
            }
        }

        // 验证过滤器
        for filter in &self.filters {
            match filter {
                FilterRule::ValueRange { point_types, min_value, max_value } => {
                    for point_type in point_types {
                        if !Self::is_valid_point_type(point_type) {
                            return Err(HisSrvError::config(format!("过滤器中无效的点位类型: {}", point_type)));
                        }
                    }
                    if let (Some(min), Some(max)) = (min_value, max_value) {
                        if min >= max {
                            return Err(HisSrvError::config("值范围过滤器的最小值必须小于最大值"));
                        }
                    }
                }
                FilterRule::TimeInterval { point_types, min_interval_seconds } => {
                    if *min_interval_seconds == 0 {
                        return Err(HisSrvError::config("时间间隔过滤器的最小间隔必须大于0"));
                    }
                    if let Some(ref point_types) = point_types {
                        for point_type in point_types {
                            if !Self::is_valid_point_type(point_type) {
                                return Err(HisSrvError::config(format!("过滤器中无效的点位类型: {}", point_type)));
                            }
                        }
                    }
                }
                FilterRule::Quality { point_types, min_quality: _ } => {
                    if let Some(ref point_types) = point_types {
                        for point_type in point_types {
                            if !Self::is_valid_point_type(point_type) {
                                return Err(HisSrvError::config(format!("过滤器中无效的点位类型: {}", point_type)));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 验证点位类型是否有效
    fn is_valid_point_type(point_type: &str) -> bool {
        matches!(point_type, "m" | "s" | "c" | "a")
    }

    /// 判断点位是否应该保存
    pub fn should_save_point(&self, channel_id: u32, point_id: u32, point_type: &str) -> bool {
        // 如果全局禁用，直接返回false
        if !self.enabled {
            return false;
        }

        // 1. 首先检查具体点位规则（优先级最高）
        for rule in &self.rules.points {
            if rule.channel_id == channel_id && rule.point_id == point_id && rule.point_type == point_type {
                return rule.enabled;
            }
        }

        // 2. 检查通道级别规则
        for rule in &self.rules.channels {
            if rule.channel_id == channel_id {
                // 如果通道禁用，直接返回false
                if !rule.enabled {
                    return false;
                }
                
                // 检查点位类型是否在允许列表中
                if let Some(ref allowed_types) = rule.point_types {
                    if !allowed_types.contains(&point_type.to_string()) {
                        return false;
                    }
                }
                
                // 通道允许该点位类型
                return true;
            }
        }

        // 3. 使用默认策略
        match self.default_policy {
            PolicyType::AllowAll => true,
            PolicyType::DenyAll => false,
        }
    }

    /// 获取所有配置的通道ID
    pub fn get_configured_channels(&self) -> Vec<u32> {
        let mut channels = Vec::new();
        
        for rule in &self.rules.channels {
            if !channels.contains(&rule.channel_id) {
                channels.push(rule.channel_id);
            }
        }
        
        for rule in &self.rules.points {
            if !channels.contains(&rule.channel_id) {
                channels.push(rule.channel_id);
            }
        }
        
        channels.sort();
        channels
    }

    /// 获取配置统计信息
    pub fn get_stats(&self) -> PointConfigStats {
        PointConfigStats {
            enabled: self.enabled,
            default_policy: self.default_policy.clone(),
            channel_rules_count: self.rules.channels.len(),
            point_rules_count: self.rules.points.len(),
            filter_rules_count: self.filters.len(),
            configured_channels: self.get_configured_channels(),
        }
    }
}

/// 配置统计信息
#[derive(Debug, Clone, Serialize)]
pub struct PointConfigStats {
    pub enabled: bool,
    pub default_policy: PolicyType,
    pub channel_rules_count: usize,
    pub point_rules_count: usize,
    pub filter_rules_count: usize,
    pub configured_channels: Vec<u32>,
}

/// 过滤器状态管理
#[derive(Debug)]
pub struct FilterState {
    /// 时间间隔过滤器的最后更新时间
    last_update_times: HashMap<String, chrono::DateTime<chrono::Utc>>,
}

impl FilterState {
    pub fn new() -> Self {
        Self {
            last_update_times: HashMap::new(),
        }
    }

    /// 检查时间间隔过滤器
    pub fn check_time_interval(&mut self, key: &str, min_interval: u64) -> bool {
        let now = chrono::Utc::now();
        let key = key.to_string();
        
        if let Some(last_time) = self.last_update_times.get(&key) {
            let duration = now.signed_duration_since(*last_time);
            if duration.num_seconds() < min_interval as i64 {
                return false; // 间隔时间不足
            }
        }
        
        self.last_update_times.insert(key, now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = PointStorageConfig::default();
        assert!(config.enabled);
        assert_eq!(config.default_policy, PolicyType::AllowAll);
        assert!(config.rules.channels.is_empty());
        assert!(config.rules.points.is_empty());
        assert!(config.filters.is_empty());
    }

    #[test]
    fn test_should_save_point_with_default_policy() {
        let config = PointStorageConfig::default();
        
        // 默认策略是 AllowAll
        assert!(config.should_save_point(1001, 10001, "m"));
        assert!(config.should_save_point(1002, 10002, "s"));
        
        // 测试 DenyAll 策略
        let mut config = config;
        config.default_policy = PolicyType::DenyAll;
        assert!(!config.should_save_point(1001, 10001, "m"));
    }

    #[test]
    fn test_should_save_point_with_channel_rules() {
        let mut config = PointStorageConfig::default();
        config.rules.channels.push(ChannelRule {
            channel_id: 1001,
            enabled: true,
            point_types: Some(vec!["m".to_string()]),
            name: None,
        });
        config.rules.channels.push(ChannelRule {
            channel_id: 1002,
            enabled: false,
            point_types: None,
            name: None,
        });

        // 通道1001允许测量数据
        assert!(config.should_save_point(1001, 10001, "m"));
        assert!(!config.should_save_point(1001, 10001, "s"));
        
        // 通道1002完全禁用
        assert!(!config.should_save_point(1002, 10001, "m"));
        
        // 未配置的通道使用默认策略
        assert!(config.should_save_point(1003, 10001, "m"));
    }

    #[test]
    fn test_should_save_point_with_point_rules() {
        let mut config = PointStorageConfig::default();
        config.rules.points.push(PointRule {
            channel_id: 1001,
            point_id: 10001,
            point_type: "m".to_string(),
            enabled: false,
            name: None,
        });

        // 特定点位被禁用
        assert!(!config.should_save_point(1001, 10001, "m"));
        
        // 其他点位使用默认策略
        assert!(config.should_save_point(1001, 10002, "m"));
    }

    #[test]
    fn test_config_validation() {
        let mut config = PointStorageConfig::default();
        
        // 测试无效的通道ID
        config.rules.channels.push(ChannelRule {
            channel_id: 0,
            enabled: true,
            point_types: None,
            name: None,
        });
        assert!(config.validate().is_err());
        
        // 测试无效的点位类型
        config.rules.channels.clear();
        config.rules.channels.push(ChannelRule {
            channel_id: 1001,
            enabled: true,
            point_types: Some(vec!["invalid".to_string()]),
            name: None,
        });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_load_from_yaml() {
        let yaml_content = r#"
enabled: true
default_policy: "allow_all"
rules:
  channels:
    - channel_id: 1001
      enabled: true
      point_types: ["m", "s"]
      name: "测试通道1"
  points:
    - channel_id: 1001
      point_id: 10001
      point_type: "m"
      enabled: false
      name: "测试点位1"
filters:
  - type: "value_range"
    point_types: ["m"]
    min_value: 0
    max_value: 100
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", yaml_content).unwrap();

        let config = PointStorageConfig::from_file(temp_file.path()).unwrap();
        assert!(config.enabled);
        assert_eq!(config.default_policy, PolicyType::AllowAll);
        assert_eq!(config.rules.channels.len(), 1);
        assert_eq!(config.rules.points.len(), 1);
        assert_eq!(config.filters.len(), 1);
    }

    #[test]
    fn test_filter_state() {
        let mut state = FilterState::new();
        
        // 第一次调用应该返回true
        assert!(state.check_time_interval("test_key", 5));
        
        // 立即再次调用应该返回false（间隔不足）
        assert!(!state.check_time_interval("test_key", 5));
    }
}