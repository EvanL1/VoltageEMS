use async_trait::async_trait;

use crate::models::{DataPoint, DataStats, HistoryRecord, QueryRangeParams};

/// Uniform interface for all historical-data storage backends.
///
/// Implement this trait to add a new backend (TimescaleDB, PostgreSQL,
/// InfluxDB, etc.) without touching the rest of the service.
#[async_trait]
pub trait StorageBackend: Send + Sync + 'static {
    /// Short identifier string, e.g. `"timescaledb"` or `"postgres"`.
    fn name(&self) -> &str;

    /// Initialize the schema (create tables, hypertables, indexes).
    /// Called once at startup.
    async fn init_schema(&self) -> anyhow::Result<()>;

    /// Persist a batch of data points. Returns the number of rows written.
    async fn write_batch(&self, points: Vec<DataPoint>) -> anyhow::Result<usize>;

    /// Paginated range query. Returns `(records, total_count)`.
    async fn query_range(
        &self,
        params: &QueryRangeParams,
        default_page_size: i64,
        max_page_size: i64,
        max_time_range_days: i64,
    ) -> anyhow::Result<(Vec<HistoryRecord>, i64)>;

    /// Fetch the single most-recent record for a key/point pair.
    async fn query_latest(
        &self,
        redis_key: &str,
        point_id: &str,
    ) -> anyhow::Result<Option<HistoryRecord>>;

    /// Global stats (row count, channel list, time range).
    async fn get_stats(&self) -> anyhow::Result<DataStats>;

    /// Return distinct Redis keys that have data in storage.
    async fn list_channels(&self) -> anyhow::Result<Vec<String>>;

    /// Delete rows older than `older_than_days`. Returns deleted row count.
    async fn cleanup_old_data(&self, older_than_days: i32) -> anyhow::Result<u64>;

    /// Lightweight connectivity check. Returns `true` if healthy.
    async fn health_check(&self) -> bool;
}
