//! 存储数据类型定义

use serde::{Deserialize, Serialize};

/// 点位数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    /// 点位值
    pub value: f64,
    /// 时间戳（毫秒）
    pub timestamp: i64,
    /// 数据质量
    pub quality: u8,
}

impl PointData {
    /// 创建新的点位数据
    pub fn new(value: f64) -> Self {
        Self {
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
            quality: 192, // Good quality
        }
    }

    /// 转换为 Redis 存储格式（固定精度）
    pub fn to_redis(&self) -> String {
        format!("{:.6}", self.value)
    }

    /// 从 Redis 格式解析
    pub fn from_redis(data: &str) -> Result<Self, String> {
        let value = data
            .parse::<f64>()
            .map_err(|e| format!("Failed to parse value: {}", e))?;
        Ok(Self::new(value))
    }
}

/// 点位更新
#[derive(Debug, Clone)]
pub struct PointUpdate {
    /// 通道ID
    pub channel_id: u16,
    /// 点位类型
    pub point_type: String,
    /// 点位ID
    pub point_id: u32,
    /// 点位数据
    pub data: PointData,
    /// 原始值（可选）
    pub raw_value: Option<f64>,
}

impl PointUpdate {
    /// 创建新的点位更新
    pub fn new(channel_id: u16, point_type: String, point_id: u32, value: f64) -> Self {
        Self {
            channel_id,
            point_type,
            point_id,
            data: PointData::new(value),
            raw_value: None,
        }
    }

    /// 创建带原始值的点位更新
    pub fn with_raw_value(mut self, raw_value: f64) -> Self {
        self.raw_value = Some(raw_value);
        self
    }

    /// 生成 Redis 键
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.channel_id, self.point_type, self.point_id)
    }
}
