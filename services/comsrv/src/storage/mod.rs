//! comsrv 实时数据库模块
//!
//! 实现扁平化的键值存储，专为实时数据优化的实时数据库(RTDB)

mod rtdb_impl;
mod types;

pub use rtdb_impl::{RetryConfig, RtdbStorage};
pub use types::{PointData, PointUpdate};

use crate::utils::error::Result;
use async_trait::async_trait;

/// 点位存储 trait
#[async_trait]
pub trait PointStorage: Send + Sync {
    /// 写入单个点位
    async fn write_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// 写入点位（带元数据）
    async fn write_point_with_metadata(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
        raw_value: Option<f64>,
    ) -> Result<()>;

    /// 批量写入
    async fn write_batch(&self, updates: Vec<PointUpdate>) -> Result<()>;

    /// 读取单个点位
    async fn read_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<PointData>>;

    /// 读取多个点位
    async fn read_points(&self, keys: Vec<String>) -> Result<Vec<Option<PointData>>>;

    /// 获取指定通道和类型的所有点位
    async fn get_channel_points(
        &self,
        channel_id: u16,
        point_type: &str,
    ) -> Result<Vec<(u32, PointData)>>;
}
