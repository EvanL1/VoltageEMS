//! 共享的基础typedefinition

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// 测点IDtype
pub type PointId = u32;

/// timerange
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

/// standard化的浮点数value，强制using6位小数精度
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StandardFloat(f64);

impl StandardFloat {
    /// Create新的standard化浮点数
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    /// Getprimalvalue
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Convert为Redisstorage格式（固定6位小数）
    pub fn to_redis(&self) -> String {
        format!("{:.6}", self.0)
    }

    /// slaveRedis格式parse
    pub fn from_redis(data: &str) -> Result<Self, String> {
        let value = data
            .parse::<f64>()
            .map_err(|e| format!("Failed to parse StandardFloat: {e}"))?;
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
        Self::new(f64::from(value))
    }
}

impl From<StandardFloat> for f64 {
    fn from(value: StandardFloat) -> Self {
        value.0
    }
}

/// data点value，用于不同serving的统一datatable示
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    /// 点位value（standard化6位小数）
    pub value: StandardFloat,
    /// Timestamp（毫秒）
    pub timestamp: i64,
}

impl PointData {
    /// Create新的点位data
    pub fn new(value: f64) -> Self {
        Self {
            value: StandardFloat::new(value),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// 带time戳create
    pub fn with_timestamp(value: f64, timestamp: i64) -> Self {
        Self {
            value: StandardFloat::new(value),
            timestamp,
        }
    }

    /// Convert为Redisstorage格式（仅value）
    pub fn to_redis_value(&self) -> String {
        self.value.to_redis()
    }

    /// Convert为Redisstorage格式（value:time戳）
    pub fn to_redis_with_timestamp(&self) -> String {
        format!("{}:{}", self.value.to_redis(), self.timestamp)
    }

    /// slaveRedis格式parse（仅value）
    pub fn from_redis_value(data: &str) -> Result<Self, String> {
        let value = StandardFloat::from_redis(data)?;
        Ok(Self {
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// slaveRedis格式parse（value:time戳）
    pub fn from_redis_with_timestamp(data: &str) -> Result<Self, String> {
        let parts: Vec<&str> = data.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid format, expected 'value:timestamp'".to_string());
        }

        let value = StandardFloat::from_redis(parts[0])?;
        let timestamp = parts[1]
            .parse::<i64>()
            .map_err(|e| format!("Failed to parse timestamp: {e}"))?;

        Ok(Self { value, timestamp })
    }
}
