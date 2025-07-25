//! 共享的基础类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// 时间范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    pub fn duration(&self) -> chrono::Duration {
        self.end - self.start
    }
}

/// 标准化的浮点数值，强制使用6位小数精度
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StandardFloat(f64);

impl StandardFloat {
    /// 创建新的标准化浮点数
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    /// 获取原始值
    pub fn value(&self) -> f64 {
        self.0
    }

    /// 转换为Redis存储格式（固定6位小数）
    pub fn to_redis(&self) -> String {
        format!("{:.6}", self.0)
    }

    /// 从Redis格式解析
    pub fn from_redis(data: &str) -> Result<Self, String> {
        let value = data
            .parse::<f64>()
            .map_err(|e| format!("Failed to parse StandardFloat: {}", e))?;
        Ok(Self::new(value))
    }
}

impl fmt::Display for StandardFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6}", self.0)
    }
}

impl From<f64> for StandardFloat {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

impl From<f32> for StandardFloat {
    fn from(value: f32) -> Self {
        Self::new(value as f64)
    }
}

impl From<StandardFloat> for f64 {
    fn from(value: StandardFloat) -> Self {
        value.0
    }
}

/// 数据点值，用于不同服务的统一数据表示
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    /// 点位值（标准化6位小数）
    pub value: StandardFloat,
    /// 时间戳（毫秒）
    pub timestamp: i64,
}

impl PointData {
    /// 创建新的点位数据
    pub fn new(value: f64) -> Self {
        Self {
            value: StandardFloat::new(value),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// 带时间戳创建
    pub fn with_timestamp(value: f64, timestamp: i64) -> Self {
        Self {
            value: StandardFloat::new(value),
            timestamp,
        }
    }

    /// 转换为Redis存储格式（仅值）
    pub fn to_redis_value(&self) -> String {
        self.value.to_redis()
    }

    /// 转换为Redis存储格式（值:时间戳）
    pub fn to_redis_with_timestamp(&self) -> String {
        format!("{}:{}", self.value.to_redis(), self.timestamp)
    }

    /// 从Redis格式解析（仅值）
    pub fn from_redis_value(data: &str) -> Result<Self, String> {
        let value = StandardFloat::from_redis(data)?;
        Ok(Self {
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// 从Redis格式解析（值:时间戳）
    pub fn from_redis_with_timestamp(data: &str) -> Result<Self, String> {
        let parts: Vec<&str> = data.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid format, expected 'value:timestamp'".to_string());
        }

        let value = StandardFloat::from_redis(parts[0])?;
        let timestamp = parts[1]
            .parse::<i64>()
            .map_err(|e| format!("Failed to parse timestamp: {}", e))?;

        Ok(Self { value, timestamp })
    }
}
