//! 极简的Redis存储类型定义

use serde::{Deserialize, Serialize};

/// 点位类型缩写
pub const TYPE_MEASUREMENT: &str = "m"; // 遥测 YC
pub const TYPE_SIGNAL: &str = "s"; // 遥信 YX
pub const TYPE_CONTROL: &str = "c"; // 遥控 YK
pub const TYPE_ADJUSTMENT: &str = "a"; // 遥调 YT

/// 点位值（只保留必要信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointValue {
    pub value: f64,
    pub timestamp: i64,
}

impl PointValue {
    /// 创建新的点位值
    pub fn new(value: f64) -> Self {
        Self {
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// 从Redis字符串解析
    pub fn from_redis(data: &str) -> Option<Self> {
        let parts: Vec<&str> = data.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(value), Ok(timestamp)) = (parts[0].parse::<f64>(), parts[1].parse::<i64>()) {
                return Some(Self { value, timestamp });
            }
        }
        None
    }

    /// 转换为Redis字符串
    pub fn to_redis(&self) -> String {
        format!("{}:{}", self.value, self.timestamp)
    }
}

/// 点位配置（极简版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointConfig {
    pub name: String,
    pub unit: String,
    pub scale: f64,
    pub offset: f64,
}

impl Default for PointConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            unit: String::new(),
            scale: 1.0,
            offset: 0.0,
        }
    }
}

/// 批量更新项
#[derive(Debug, Clone)]
pub struct PointUpdate {
    pub channel_id: u16,
    pub point_type: &'static str,
    pub point_id: u32,
    pub value: f64,
}

/// 批量查询键
#[derive(Debug, Clone)]
pub struct PointKey {
    pub channel_id: u16,
    pub point_type: &'static str,
    pub point_id: u32,
}

/// 生成Redis键
pub fn make_key(channel_id: u16, point_type: &str, point_id: u32) -> String {
    format!("{}:{}:{}", channel_id, point_type, point_id)
}

/// 生成配置键
pub fn make_config_key(channel_id: u16, point_type: &str, point_id: u32) -> String {
    format!("cfg:{}:{}:{}", channel_id, point_type, point_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_value() {
        let pv = PointValue::new(25.6);
        assert_eq!(pv.value, 25.6);

        let redis_str = pv.to_redis();
        assert!(redis_str.contains("25.6"));

        let parsed = PointValue::from_redis(&redis_str).unwrap();
        assert_eq!(parsed.value, 25.6);
        assert_eq!(parsed.timestamp, pv.timestamp);
    }

    #[test]
    fn test_make_key() {
        assert_eq!(make_key(1001, TYPE_MEASUREMENT, 10001), "1001:m:10001");
        assert_eq!(make_key(1001, TYPE_SIGNAL, 20001), "1001:s:20001");
        assert_eq!(
            make_config_key(1001, TYPE_MEASUREMENT, 10001),
            "cfg:1001:m:10001"
        );
    }
}
