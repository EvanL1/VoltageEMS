//! storagedatatypedefinition

use voltage_libs::types::PointData as LibPointData;

/// 点位data（usinglibrarymedium的standard化version）
pub type PointData = LibPointData;

/// 点位update
#[derive(Debug, Clone)]
pub struct PointUpdate {
    /// channelID
    pub channel_id: u16,
    /// 点位type
    pub point_type: String,
    /// 点位ID
    pub point_id: u32,
    /// 点位data
    pub data: PointData,
    /// primalvalue（可选）
    pub raw_value: Option<f64>,
}

impl PointUpdate {
    /// Create新的点位update
    pub fn new(channel_id: u16, point_type: String, point_id: u32, value: f64) -> Self {
        Self {
            channel_id,
            point_type,
            point_id,
            data: PointData::new(value),
            raw_value: None,
        }
    }

    /// Create带primalvalue的点位update
    pub fn with_raw_value(mut self, raw_value: f64) -> Self {
        self.raw_value = Some(raw_value);
        self
    }

    /// 生成 Redis Hashkey
    pub fn hash_key(&self) -> String {
        format!("comsrv:{}:{}", self.channel_id, self.point_type)
    }

    /// 生成 Redis key（旧格式，reserving用于兼容）
    pub fn key(&self) -> String {
        format!(
            "comsrv:{}:{}:{}",
            self.channel_id, self.point_type, self.point_id
        )
    }
}
