//! 存储数据类型定义

use voltage_libs::types::PointData as LibPointData;

/// 点位数据（使用库中的标准化版本）
pub type PointData = LibPointData;

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

    /// 生成 Redis Hash键
    pub fn hash_key(&self) -> String {
        format!("comsrv:{}:{}", self.channel_id, self.point_type)
    }

    /// 生成 Redis 键（旧格式，保留用于兼容）
    pub fn key(&self) -> String {
        format!(
            "comsrv:{}:{}:{}",
            self.channel_id, self.point_type, self.point_id
        )
    }
}
