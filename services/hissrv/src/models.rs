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

/// 服务运行参数配置（`/hisApi/config`）
///
/// 控制数据采集频率、写入批量、查询限制和 Redis 监听规则。
/// 存储后端连接参数请通过 `/hisApi/storage` 单独管理。
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
    /// 采集间隔（秒）
    ///
    /// 每隔多少秒从 Redis 扫描一次数据并写入内存缓冲区。
    /// 值越小数据越实时，但 Redis 压力越大，建议范围 10 ~ 300。
    #[schema(example = 30, minimum = 1)]
    pub collection_interval_secs: u64,

    /// 刷盘间隔（秒）
    ///
    /// 每隔多少秒将内存缓冲区的数据批量写入数据库。
    /// 建议不低于 `collection_interval_secs`，范围 30 ~ 600。
    #[schema(example = 60, minimum = 1)]
    pub flush_interval_secs: u64,

    /// 单批写入上限（条）
    ///
    /// 每次刷盘最多写入多少条记录。超出部分留到下次刷盘。
    /// 建议范围 100 ~ 5000，过大会增加单次数据库事务耗时。
    #[schema(example = 1000, minimum = 1)]
    pub batch_size: usize,

    /// 是否启用历史数据清理
    ///
    /// 开启后，每天凌晨 02:00 UTC 自动删除超过保留期限的旧数据。
    #[schema(example = true)]
    pub cleanup_enabled: bool,

    /// 数据保留天数
    ///
    /// 清理任务会删除距今超过此天数的所有历史记录。
    /// 仅在 `cleanup_enabled = true` 时生效，建议范围 7 ~ 3650。
    #[schema(example = 30, minimum = 1)]
    pub cleanup_older_than_days: i32,

    /// 默认分页大小（条/页）
    ///
    /// 查询历史数据时，不传 `page_size` 参数时使用此值。
    #[schema(example = 100, minimum = 1)]
    pub default_page_size: i64,

    /// 最大分页大小（条/页）
    ///
    /// 前端传入的 `page_size` 超过此值时会被截断，防止单次查询过大。
    #[schema(example = 1000, minimum = 1)]
    pub max_page_size: i64,

    /// 最大查询时间跨度（天）
    ///
    /// 单次查询的时间范围（`start_time` 到 `end_time`）不能超过此值，
    /// 超出则返回错误。建议范围 1 ~ 3650。
    #[schema(example = 365, minimum = 1)]
    pub max_time_range_days: i64,

    /// Redis Key 订阅模式列表（**glob 语法**，与 Redis SCAN 相同）
    ///
    /// 采集器只扫描匹配这些模式的 Redis Key。使用 glob 语法：
    /// - `*` 匹配任意长度的任意字符
    /// - `?` 匹配单个任意字符
    ///
    /// 示例：`inst:*:M` 匹配所有通道的遥测数据，`inst:*:A` 匹配遥信数据。
    #[schema(example = json!(["inst:*:M", "inst:*:A"]))]
    pub subscribe_patterns: Vec<String>,

    /// 排除模式列表（**正则表达式语法**，与 subscribe_patterns 的 glob 语法不同）
    ///
    /// 命中任意一条正则的 Redis Key 将被跳过，不采集。留空表示不排除任何 Key。
    ///
    /// 正则常用符号：
    /// - `.`  匹配任意单个字符
    /// - `.*` 匹配任意长度的任意字符串（相当于 glob 的 `*`）
    /// - `^`  匹配开头，`$` 匹配结尾
    ///
    /// 示例：
    /// - `["^inst:0:"]` 排除通道 0 的所有数据
    /// - `["^inst:0:.*", "^inst:1:.*"]` 同时排除通道 0 和通道 1
    ///
    /// ⚠️ 注意：此处 **不能** 用 glob 语法，`inst:0:*` 在正则中含义错误。
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

/// 存储后端连接参数（`PUT /hisApi/storage`）
/// 连通性测试请求（`POST /hisApi/storage/test`）
///
/// 探测时**不写入任何数据、不修改任何运行状态**。  
/// 对 PostgreSQL / TimescaleDB：连接内置 `postgres` 维护库并执行 `SELECT 1`，
/// 因此**业务数据库不需要提前存在**即可通过测试。
#[derive(Debug, Deserialize, ToSchema)]
pub struct StorageTestRequest {
    /// 数据库类型
    ///
    /// - `postgres` — 标准 PostgreSQL
    /// - `timescaledb` — PostgreSQL + TimescaleDB 扩展（连接参数与 postgres 相同）
    /// - `influxdb` — InfluxDB（预留，暂未实现）
    #[schema(example = "timescaledb")]
    pub backend: String,

    /// 数据库主机地址（IP 或域名）
    #[schema(example = "192.168.20.21")]
    pub host: String,

    /// 数据库端口
    ///
    /// PostgreSQL / TimescaleDB 默认 `5432`；InfluxDB 默认 `8086`。
    #[schema(example = 5432, minimum = 1, maximum = 65535)]
    pub port: Option<u16>,

    /// 数据库用户名（PostgreSQL / TimescaleDB）
    #[schema(example = "postgres")]
    pub username: String,

    /// 数据库密码（PostgreSQL / TimescaleDB）
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

/// PUT /hisApi/storage 请求体
///
/// 此接口**只保存参数**，不会立即建立数据库连接。
/// 保存后通过 `POST /hisApi/storage/reconnect` 应用并连接，
/// 或通过 `POST /hisApi/storage/test` 提前验证连通性。
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
    /// 是否启用历史存储
    ///
    /// `true`：服务启动或重连后开始采集并写入历史数据。
    /// `false`：停止写入（已存数据不受影响）。
    #[schema(example = true)]
    pub enabled: bool,

    /// 数据库类型
    ///
    /// - `postgres`：标准 PostgreSQL，适合普通历史数据存储。
    /// - `timescaledb`：PostgreSQL + TimescaleDB 扩展，针对时序数据优化，
    ///   查询大量历史数据时性能更好，**推荐生产环境使用**。
    #[schema(example = "timescaledb")]
    pub backend: String,

    /// 数据库主机地址
    ///
    /// IP 地址或域名，例如 `192.168.20.21` 或 `db.example.com`。
    #[schema(example = "192.168.20.21")]
    pub host: String,

    /// 数据库端口，默认 `5432`
    #[schema(example = 5432, minimum = 1, maximum = 65535)]
    pub port: Option<u16>,

    /// 数据库名称
    ///
    /// 历史数据将写入此数据库，**数据库须已存在**，首次连接时会自动建表。
    #[schema(example = "hissrv")]
    pub database: String,

    /// 数据库用户名
    #[schema(example = "postgres")]
    pub username: String,

    /// 数据库密码
    ///
    /// 密码中包含特殊字符（如 `@` `#` `:`）时无需手动转义，后端会自动处理。
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
