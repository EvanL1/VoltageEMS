use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

// ── Core data types ───────────────────────────────────────────────────────────

/// One measurement point ready to be written to storage.
#[derive(Debug, Clone)]
pub struct DataPoint {
    pub time: DateTime<Utc>,
    /// Redis key, e.g. `inst:1:M`
    pub redis_key: String,
    /// Field name inside the hash, e.g. `"42"`
    pub point_id: String,
    pub value: Option<f64>,
    pub string_value: Option<String>,
}

/// One row returned from a historical query.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HistoryRecord {
    pub timestamp: String,
    pub redis_key: String,
    pub point_id: String,
    pub value: Option<f64>,
    /// Source prefix, derived from the first segment of redis_key (e.g. `inst`)
    pub source: String,
}

// ── Query models ──────────────────────────────────────────────────────────────

/// Query string parameters for `GET /hisApi/data/query`.
#[derive(Debug, Deserialize, IntoParams)]
pub struct QueryRangeParams {
    pub redis_key: String,
    pub point_id: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// Query string parameters for `GET /hisApi/data/latest`.
#[derive(Debug, Deserialize, IntoParams)]
pub struct LatestParams {
    pub redis_key: String,
    pub point_id: String,
}

/// Paginated query result.
#[allow(dead_code)]
#[derive(Debug, Serialize, ToSchema)]
pub struct QueryResult {
    pub status: String,
    pub message: String,
    pub data: Vec<HistoryRecord>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub has_more: bool,
}

/// Response for `GET /hisApi/data/range`.
#[derive(Debug, Serialize, ToSchema)]
pub struct DataStats {
    pub earliest_timestamp: Option<String>,
    pub latest_timestamp: Option<String>,
    pub total_points: i64,
    pub channels: Vec<String>,
    pub data_types: Vec<String>,
}

// ── Dynamic service configuration ────────────────────────────────────────────

/// Service runtime configuration (`/hisApi/config`).
///
/// Controls collection frequency, write batch size, query limits, and
/// Redis subscription patterns. Storage backend connection parameters are
/// managed separately via `/hisApi/storage`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "collection_interval_secs": 30,
    "flush_interval_secs": 60,
    "batch_size": 1000,
    "cleanup_enabled": true,
    "cleanup_older_than_days": 30,
    "default_page_size": 100,
    "max_page_size": 1000,
    "max_time_range_days": 365,
    "subscribe_patterns": ["inst:*:M", "inst:*:A"],
    "exclude_patterns": []
}))]
pub struct ServiceConfig {
    /// Collection interval in seconds.
    ///
    /// How often the collector scans Redis and writes data into the in-memory
    /// buffer. Shorter intervals increase data freshness but add Redis load.
    /// Recommended range: 10–300.
    #[schema(example = 30, minimum = 1)]
    pub collection_interval_secs: u64,

    /// Flush interval in seconds.
    ///
    /// How often the in-memory buffer is batch-written to the database.
    /// Should not be shorter than `collection_interval_secs`.
    /// Recommended range: 30–600.
    #[schema(example = 60, minimum = 1)]
    pub flush_interval_secs: u64,

    /// Maximum records per flush batch.
    ///
    /// Records beyond this limit are deferred to the next flush cycle.
    /// Larger values increase single-transaction latency.
    /// Recommended range: 100–5000.
    #[schema(example = 1000, minimum = 1)]
    pub batch_size: usize,

    /// Enable automatic data retention cleanup.
    ///
    /// When enabled, a daily job at 02:00 UTC deletes records older than
    /// `cleanup_older_than_days`.
    #[schema(example = true)]
    pub cleanup_enabled: bool,

    /// Data retention period in days.
    ///
    /// The cleanup job removes all records older than this value.
    /// Only effective when `cleanup_enabled = true`.
    /// Recommended range: 7–3650.
    #[schema(example = 30, minimum = 1)]
    pub cleanup_older_than_days: i32,

    /// Default page size (records per page).
    ///
    /// Used when the caller omits the `page_size` query parameter.
    #[schema(example = 100, minimum = 1)]
    pub default_page_size: i64,

    /// Maximum allowed page size (records per page).
    ///
    /// Client-supplied `page_size` values exceeding this limit are clamped
    /// to prevent oversized single queries.
    #[schema(example = 1000, minimum = 1)]
    pub max_page_size: i64,

    /// Maximum query time span in days.
    ///
    /// A single query's `start_time`-to-`end_time` range may not exceed this
    /// value; requests exceeding it are rejected. Recommended range: 1–3650.
    #[schema(example = 365, minimum = 1)]
    pub max_time_range_days: i64,

    /// Redis key subscription patterns (**glob syntax**, same as Redis SCAN).
    ///
    /// The collector only scans Redis keys matching at least one of these
    /// patterns. Glob syntax:
    /// - `*` — matches any sequence of characters
    /// - `?` — matches any single character
    ///
    /// Example: `inst:*:M` matches telemetry for all channels;
    /// `inst:*:A` matches status data.
    #[schema(example = json!(["inst:*:M", "inst:*:A"]))]
    pub subscribe_patterns: Vec<String>,

    /// Exclusion patterns (**regex syntax** — distinct from the glob syntax
    /// used in `subscribe_patterns`).
    ///
    /// Any Redis key matching at least one regex is skipped. Leave empty to
    /// collect all matched keys.
    ///
    /// Common regex constructs:
    /// - `.`  — any single character
    /// - `.*` — any sequence of characters (equivalent to glob `*`)
    /// - `^`  — start of string; `$` — end of string
    ///
    /// Examples:
    /// - `["^inst:0:"]` — exclude all data for channel 0
    /// - `["^inst:0:.*", "^inst:1:.*"]` — exclude channels 0 and 1
    ///
    /// Note: glob syntax is **not** valid here — `inst:0:*` has incorrect
    /// meaning as a regex.
    #[schema(example = json!([]))]
    pub exclude_patterns: Vec<String>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            collection_interval_secs: 30,
            flush_interval_secs: 60,
            batch_size: 1000,
            cleanup_enabled: true,
            cleanup_older_than_days: 30,
            default_page_size: 100,
            max_page_size: 1000,
            max_time_range_days: 365,
            subscribe_patterns: vec!["inst:*:M".to_string(), "inst:*:A".to_string()],
            exclude_patterns: vec![],
        }
    }
}

impl ServiceConfig {
    pub fn normalize(&mut self) {
        self.collection_interval_secs = self.collection_interval_secs.max(1);
        self.flush_interval_secs = self.flush_interval_secs.max(1);
        self.batch_size = self.batch_size.max(1);
        self.cleanup_older_than_days = self.cleanup_older_than_days.max(1);
        self.default_page_size = self.default_page_size.max(1);
        self.max_page_size = self.max_page_size.max(1);
        self.max_time_range_days = self.max_time_range_days.max(1);
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn normalize_clamps_zero_runtime_values() {
        let mut cfg = ServiceConfig {
            collection_interval_secs: 0,
            flush_interval_secs: 0,
            batch_size: 0,
            cleanup_older_than_days: 0,
            default_page_size: 0,
            max_page_size: 0,
            max_time_range_days: 0,
            ..ServiceConfig::default()
        };

        cfg.normalize();

        assert_eq!(cfg.collection_interval_secs, 1);
        assert_eq!(cfg.flush_interval_secs, 1);
        assert_eq!(cfg.batch_size, 1);
        assert_eq!(cfg.cleanup_older_than_days, 1);
        assert_eq!(cfg.default_page_size, 1);
        assert_eq!(cfg.max_page_size, 1);
        assert_eq!(cfg.max_time_range_days, 1);
    }
}

// ── Internal storage connection settings ─────────────────────────────────────

/// Storage backend connection settings.  Persisted in the same `hissrv_config`
/// table but **only** accessible via `/hisApi/storage` – never mixed into the
/// general service config API.
#[derive(Debug, Clone, Default)]
pub struct StorageSettings {
    pub enabled: bool,
    /// "postgres" | "timescaledb"
    pub backend: String,
    /// Full PostgreSQL DSN assembled by the backend from the user-supplied
    /// host/port/database/username/password fields.
    pub url: String,
}

// ── Storage configuration request ────────────────────────────────────────────

/// Connectivity test request body (`POST /hisApi/storage/test`).
///
/// The probe **does not write any data or modify any runtime state**.
/// For PostgreSQL / TimescaleDB it connects to the built-in `postgres`
/// maintenance database and executes `SELECT 1`, so **the target business
/// database does not need to exist** for the test to pass.
#[derive(Debug, Deserialize, ToSchema)]
pub struct StorageTestRequest {
    /// Database backend type.
    ///
    /// - `postgres` — standard PostgreSQL
    /// - `timescaledb` — PostgreSQL + TimescaleDB extension (same connection params as postgres)
    /// - `influxdb` — InfluxDB (reserved; not yet implemented)
    #[schema(example = "timescaledb")]
    pub backend: String,

    /// Database host address (IP or hostname).
    #[schema(example = "192.168.20.21")]
    pub host: String,

    /// Database port.
    ///
    /// Default: `5432` for PostgreSQL / TimescaleDB; `8086` for InfluxDB.
    #[schema(example = 5432, minimum = 1, maximum = 65535)]
    pub port: Option<u16>,

    /// Database username (PostgreSQL / TimescaleDB).
    #[schema(example = "postgres")]
    pub username: String,

    /// Database password (PostgreSQL / TimescaleDB).
    #[schema(example = "secret")]
    pub password: String,
}

impl StorageTestRequest {
    /// Friendly `host:port` string for log / response messages.
    pub fn addr(&self) -> String {
        let default_port = match self.backend.as_str() {
            "influxdb" => 8086,
            _ => 5432,
        };
        format!("{}:{}", self.host, self.port.unwrap_or(default_port))
    }

    /// Build a PostgreSQL DSN pointing at the always-present `postgres`
    /// maintenance database (used for postgres / timescaledb probing).
    pub fn pg_probe_dsn(&self) -> String {
        build_dsn(
            &self.host,
            self.port,
            "postgres",
            &self.username,
            &self.password,
        )
    }
}

/// Request body for `PUT /hisApi/storage`.
///
/// This endpoint **only persists parameters**; it does not establish a
/// database connection immediately. After saving, apply and connect via
/// `POST /hisApi/storage/reconnect`, or verify connectivity first via
/// `POST /hisApi/storage/test`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[schema(example = json!({
    "enabled": true,
    "backend": "timescaledb",
    "host": "192.168.20.21",
    "port": 5432,
    "database": "hissrv",
    "username": "postgres",
    "password": "postgres"
}))]
pub struct StorageConfigRequest {
    /// Enable historical storage.
    ///
    /// `true`: collection and writes begin after service startup or reconnect.
    /// `false`: writes are stopped (existing data is unaffected).
    #[schema(example = true)]
    pub enabled: bool,

    /// Database backend type.
    ///
    /// - `postgres`: standard PostgreSQL, suitable for general historical storage.
    /// - `timescaledb`: PostgreSQL + TimescaleDB extension, optimised for
    ///   time-series data; recommended for production.
    #[schema(example = "timescaledb")]
    pub backend: String,

    /// Database host address.
    ///
    /// IP address or hostname, e.g. `192.168.20.21` or `db.example.com`.
    #[schema(example = "192.168.20.21")]
    pub host: String,

    /// Database port (default `5432`).
    #[schema(example = 5432, minimum = 1, maximum = 65535)]
    pub port: Option<u16>,

    /// Database name.
    ///
    /// Historical data is written to this database. The database is created
    /// automatically on first connect; tables are initialised on first use.
    #[schema(example = "hissrv")]
    pub database: String,

    /// Database username.
    #[schema(example = "postgres")]
    pub username: String,

    /// Database password.
    ///
    /// Special characters (`@`, `#`, `:`, etc.) do not need to be
    /// percent-encoded — the backend handles URL-encoding automatically.
    #[schema(example = "postgres")]
    pub password: String,
}

impl StorageConfigRequest {
    pub fn to_dsn(&self) -> String {
        build_dsn(
            &self.host,
            self.port,
            &self.database,
            &self.username,
            &self.password,
        )
    }
}

// ── Shared DSN builder ────────────────────────────────────────────────────────

pub fn build_dsn(
    host: &str,
    port: Option<u16>,
    database: &str,
    username: &str,
    password: &str,
) -> String {
    let port = port.unwrap_or(5432);
    let user = urlencoding::encode(username);
    let pass = urlencoding::encode(password);
    format!(
        "postgres://{}:{}@{}:{}/{}",
        user, pass, host, port, database
    )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a `DateTime<Utc>` to the ISO-8601 string format used in responses.
pub fn fmt_ts(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Derive the `source` prefix from a Redis key (first `:` segment).
pub fn source_from_key(key: &str) -> String {
    key.split(':').next().unwrap_or(key).to_string()
}

/// Parse various time string formats into `DateTime<Utc>`.
pub fn parse_time(s: &str) -> anyhow::Result<DateTime<Utc>> {
    use chrono::NaiveDateTime;

    // Try RFC 3339 / ISO 8601 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // `2025-08-21 23:59:59`
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(dt.and_utc());
    }

    // `2025-08-21T23:59:59`
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt.and_utc());
    }

    // Date only: `2025-08-21`
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        && let Some(dt) = d.and_hms_opt(0, 0, 0)
    {
        return Ok(dt.and_utc());
    }

    // Unix timestamp (integer)
    if let Ok(ts) = s.parse::<i64>()
        && let Some(dt) = DateTime::from_timestamp(ts, 0)
    {
        return Ok(dt);
    }

    anyhow::bail!("Unsupported time format: {}", s)
}
