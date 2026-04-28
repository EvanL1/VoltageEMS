/// Plain PostgreSQL storage backend.
///
/// Uses a regular `history` table with B-tree indexes. No TimescaleDB
/// extension required. For deployments with TimescaleDB installed, prefer
/// `backend_tsdb.rs` which converts the table to a hypertable.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use tracing::{error, info};

use crate::models::{
    DataPoint, DataStats, HistoryRecord, QueryRangeParams, fmt_ts, parse_time, source_from_key,
};
use crate::storage::StorageBackend;

pub struct PostgresBackend {
    pub pool: PgPool,
}

impl PostgresBackend {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
    fn name(&self) -> &str {
        "postgres"
    }

    async fn init_schema(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS history (
                time         TIMESTAMPTZ NOT NULL,
                redis_key    TEXT NOT NULL,
                point_id     TEXT NOT NULL,
                value        DOUBLE PRECISION,
                string_value TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_history_key_point_time
             ON history (redis_key, point_id, time DESC)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_history_time
             ON history (time DESC)",
        )
        .execute(&self.pool)
        .await?;

        info!("PostgreSQL backend schema initialized");
        Ok(())
    }

    async fn write_batch(&self, points: Vec<DataPoint>) -> anyhow::Result<usize> {
        if points.is_empty() {
            return Ok(0);
        }
        let len = points.len();

        let times: Vec<DateTime<Utc>> = points.iter().map(|p| p.time).collect();
        let keys: Vec<&str> = points.iter().map(|p| p.redis_key.as_str()).collect();
        let pids: Vec<&str> = points.iter().map(|p| p.point_id.as_str()).collect();
        let values: Vec<Option<f64>> = points.iter().map(|p| p.value).collect();
        let svalues: Vec<Option<&str>> = points.iter().map(|p| p.string_value.as_deref()).collect();

        sqlx::query(
            "INSERT INTO history (time, redis_key, point_id, value, string_value)
             SELECT * FROM UNNEST(
                 $1::TIMESTAMPTZ[],
                 $2::TEXT[],
                 $3::TEXT[],
                 $4::FLOAT8[],
                 $5::TEXT[]
             )",
        )
        .bind(times)
        .bind(keys)
        .bind(pids)
        .bind(values)
        .bind(svalues)
        .execute(&self.pool)
        .await?;

        Ok(len)
    }

    async fn query_range(
        &self,
        params: &QueryRangeParams,
        default_page_size: i64,
        max_page_size: i64,
        max_time_range_days: i64,
    ) -> anyhow::Result<(Vec<HistoryRecord>, i64)> {
        let page = params.page.unwrap_or(1).max(1);
        let page_size = params
            .page_size
            .unwrap_or(default_page_size)
            .min(max_page_size)
            .max(1);
        let offset = (page - 1) * page_size;

        let end = params
            .end_time
            .as_deref()
            .map(parse_time)
            .transpose()?
            .unwrap_or_else(Utc::now);

        let start = params
            .start_time
            .as_deref()
            .map(parse_time)
            .transpose()?
            .unwrap_or_else(|| end - Duration::hours(24));

        let min_allowed = end - Duration::days(max_time_range_days);
        let start = if start < min_allowed {
            min_allowed
        } else {
            start
        };

        struct Row {
            time: DateTime<Utc>,
            redis_key: String,
            point_id: String,
            value: Option<f64>,
        }

        let rows = sqlx::query_as::<_, (DateTime<Utc>, String, String, Option<f64>)>(
            "SELECT time, redis_key, point_id, value
             FROM history
             WHERE redis_key = $1 AND point_id = $2
               AND time >= $3 AND time <= $4
             ORDER BY time DESC
             LIMIT $5 OFFSET $6",
        )
        .bind(&params.redis_key)
        .bind(&params.point_id)
        .bind(start)
        .bind(end)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(time, redis_key, point_id, value)| Row {
            time,
            redis_key,
            point_id,
            value,
        })
        .collect::<Vec<_>>();

        let total: i64 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*)
             FROM history
             WHERE redis_key = $1 AND point_id = $2
               AND time >= $3 AND time <= $4",
        )
        .bind(&params.redis_key)
        .bind(&params.point_id)
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map(|(n,)| n)
        .unwrap_or(0);

        let records = rows
            .into_iter()
            .map(|r| HistoryRecord {
                timestamp: fmt_ts(&r.time),
                source: source_from_key(&r.redis_key),
                redis_key: r.redis_key,
                point_id: r.point_id,
                value: r.value,
            })
            .collect();

        Ok((records, total))
    }

    async fn query_latest(
        &self,
        redis_key: &str,
        point_id: &str,
    ) -> anyhow::Result<Option<HistoryRecord>> {
        let row = sqlx::query_as::<_, (DateTime<Utc>, String, String, Option<f64>)>(
            "SELECT time, redis_key, point_id, value
             FROM history
             WHERE redis_key = $1 AND point_id = $2
             ORDER BY time DESC
             LIMIT 1",
        )
        .bind(redis_key)
        .bind(point_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(time, rk, pid, value)| HistoryRecord {
            timestamp: fmt_ts(&time),
            source: source_from_key(&rk),
            redis_key: rk,
            point_id: pid,
            value,
        }))
    }

    async fn get_stats(&self) -> anyhow::Result<DataStats> {
        let (earliest, latest, total): (Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<i64>) =
            sqlx::query_as(
                "SELECT MIN(time), MAX(time), COUNT(*)
                 FROM history",
            )
            .fetch_one(&self.pool)
            .await?;

        let channels = self.list_channels().await.unwrap_or_default();
        let data_types: Vec<String> = channels
            .iter()
            .filter_map(|k| k.split(':').next().map(|s| s.to_string()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(DataStats {
            earliest_timestamp: earliest.as_ref().map(fmt_ts),
            latest_timestamp: latest.as_ref().map(fmt_ts),
            total_points: total.unwrap_or(0),
            channels,
            data_types,
        })
    }

    async fn list_channels(&self) -> anyhow::Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT redis_key FROM history ORDER BY redis_key")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(k,)| k).collect())
    }

    async fn cleanup_old_data(&self, older_than_days: i32) -> anyhow::Result<u64> {
        let cutoff = Utc::now() - Duration::days(older_than_days as i64);
        let result = sqlx::query("DELETE FROM history WHERE time < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        let deleted = result.rows_affected();
        info!(
            "Cleanup: deleted {} rows older than {} days",
            deleted, older_than_days
        );
        Ok(deleted)
    }

    async fn health_check(&self) -> bool {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| true)
            .unwrap_or_else(|e| {
                error!("PostgreSQL health check failed: {}", e);
                false
            })
    }
}
