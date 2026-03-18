/// TimescaleDB storage backend.
///
/// Delegates all read/write operations to `PostgresBackend`.  The only
/// difference is `init_schema`: after creating the regular `history` table it
/// attempts to convert it into a TimescaleDB *hypertable* partitioned by
/// `time`.  If the TimescaleDB extension is absent the call is silently skipped
/// and the table behaves like plain PostgreSQL.
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::{info, warn};

use crate::backend_pg::PostgresBackend;
use crate::models::{DataPoint, DataStats, HistoryRecord, QueryRangeParams};
use crate::storage::StorageBackend;

pub struct TimescaleDbBackend {
    inner: PostgresBackend,
}

impl TimescaleDbBackend {
    pub fn new(pool: PgPool) -> Self {
        Self {
            inner: PostgresBackend::new(pool.clone()),
        }
    }
}

#[async_trait]
impl StorageBackend for TimescaleDbBackend {
    fn name(&self) -> &str {
        "timescaledb"
    }

    async fn init_schema(&self) -> anyhow::Result<()> {
        // Reuse the plain-PG schema creation (table + indexes).
        self.inner.init_schema().await?;

        // Attempt hypertable conversion.  This is a no-op if TimescaleDB is
        // not installed; we log a warning and continue.
        match sqlx::query(
            "SELECT create_hypertable('history', 'time', if_not_exists => TRUE)",
        )
        .execute(&self.inner.pool)
        .await
        {
            Ok(_) => info!("TimescaleDB hypertable created (or already existed)"),
            Err(e) => {
                warn!(
                    "create_hypertable failed – is the TimescaleDB extension installed? ({}). \
                     Falling back to plain PostgreSQL behaviour.",
                    e
                );
            }
        }

        Ok(())
    }

    // All remaining methods delegate to the shared PostgreSQL implementation.

    async fn write_batch(&self, points: Vec<DataPoint>) -> anyhow::Result<usize> {
        self.inner.write_batch(points).await
    }

    async fn query_range(
        &self,
        params: &QueryRangeParams,
        default_page_size: i64,
        max_page_size: i64,
        max_time_range_days: i64,
    ) -> anyhow::Result<(Vec<HistoryRecord>, i64)> {
        self.inner
            .query_range(params, default_page_size, max_page_size, max_time_range_days)
            .await
    }

    async fn query_latest(
        &self,
        redis_key: &str,
        point_id: &str,
    ) -> anyhow::Result<Option<HistoryRecord>> {
        self.inner.query_latest(redis_key, point_id).await
    }

    async fn get_stats(&self) -> anyhow::Result<DataStats> {
        self.inner.get_stats().await
    }

    async fn list_channels(&self) -> anyhow::Result<Vec<String>> {
        self.inner.list_channels().await
    }

    async fn cleanup_old_data(&self, older_than_days: i32) -> anyhow::Result<u64> {
        self.inner.cleanup_old_data(older_than_days).await
    }

    async fn health_check(&self) -> bool {
        self.inner.health_check().await
    }
}
