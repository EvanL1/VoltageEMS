//! comsrv real-timedatalibrarymodular
//!
//! implementflat化的keyvaluestorage，专为real-timedataoptimization的real-timedatalibrary(RTDB)

mod rtdb_impl;
mod types;

pub use rtdb_impl::{RetryConfig, RtdbStorage};
pub use types::{PointData, PointUpdate};

use crate::utils::error::Result;
use async_trait::async_trait;

/// 点位storage trait
#[async_trait]
pub trait PointStorage: Send + Sync {
    /// write单个点位
    async fn write_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// write点位（带metadata）
    async fn write_point_with_metadata(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
        raw_value: Option<f64>,
    ) -> Result<()>;

    /// batchwrite
    async fn write_batch(&self, updates: Vec<PointUpdate>) -> Result<()>;

    /// read单个点位
    async fn read_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<PointData>>;

    /// read多个点位
    async fn read_points(&self, keys: Vec<String>) -> Result<Vec<Option<PointData>>>;

    /// Get指定channel和type的all点位
    async fn get_channel_points(
        &self,
        channel_id: u16,
        point_type: &str,
    ) -> Result<Vec<(u32, PointData)>>;
}
