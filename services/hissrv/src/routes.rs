use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use tracing::{error, info};
use utoipa::OpenApi;
use utoipa_swagger_ui::{Config, SwaggerUi};

use crate::backend_null::NullBackend;
use crate::db_config;
use crate::models::{
    DataStats, HistoryRecord, LatestParams, QueryRangeParams, QueryResult, ServiceConfig,
    StorageConfigRequest, StorageSettings,
};
use crate::state::AppState;
use crate::storage::StorageBackend;

// ============================================================================
// Internal: lightweight connectivity probe (no schema init, no data write)
// ============================================================================

/// Try to open a connection and run `SELECT 1`.  Returns `Ok(())` on success.
async fn probe_connection(url: &str) -> anyhow::Result<()> {
    use sqlx::postgres::PgPoolOptions;
    use sqlx::Executor;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(url)
        .await
        .map_err(|e| anyhow::anyhow!("连接失败: {}", e))?;

    pool.execute("SELECT 1")
        .await
        .map_err(|e| anyhow::anyhow!("探测查询失败: {}", e))?;
    pool.close().await;
    Ok(())
}

// ============================================================================
// Router
// ============================================================================

pub fn build_router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        // Health
        .route("/", get(root))
        .route("/ping", get(ping))
        .route("/hisApi/health", get(health))
        // Data queries
        .route("/hisApi/data/query", get(query_range))
        .route("/hisApi/data/latest", get(query_latest))
        .route("/hisApi/data/range", get(data_range))
        // Metadata
        .route("/hisApi/channels", get(list_channels))
        .route("/hisApi/metrics", get(metrics))
        // General service config (intervals, patterns, etc.)
        .route("/hisApi/config", get(get_config).put(update_config))
        // Storage backend config & control
        .route("/hisApi/storage", get(get_storage).put(update_storage))
        .route("/hisApi/storage/test", get(test_storage))
        .route("/hisApi/storage/reconnect", axum::routing::post(reconnect_storage))
        .with_state(state);

    api.merge(
        SwaggerUi::new("/docs")
            .url("/openapi.json", ApiDoc::openapi())
            .config(
                Config::default()
                    .default_model_rendering("model")
                    .default_models_expand_depth(1),
            ),
    )
}

// ============================================================================
// OpenAPI document
// ============================================================================

#[derive(OpenApi)]
#[openapi(
    paths(
        root,
        ping,
        health,
        query_range,
        query_latest,
        data_range,
        list_channels,
        metrics,
        get_config,
        update_config,
        get_storage,
        update_storage,
        test_storage,
        reconnect_storage,
    ),
    components(schemas(
        HistoryRecord,
        QueryResult,
        DataStats,
        ServiceConfig,
        StorageConfigRequest,
    )),
    tags(
        (name = "Data",    description = "历史数据查询"),
        (name = "Meta",    description = "元数据与指标"),
        (name = "Config",  description = "服务配置"),
        (name = "Storage", description = "存储后端配置与控制"),
        (name = "Health",  description = "健康检查"),
    ),
    info(
        title = "VoltageEMS History Service",
        version = "1.0.0",
        description = "历史数据采集、存储与查询（支持 PostgreSQL / TimescaleDB 后端）"
    )
)]
pub struct ApiDoc;

// ============================================================================
// Public helper – build and initialise a storage backend by type + URL.
// Used both in main.rs (startup restore) and the PUT /hisApi/storage handler.
// ============================================================================

pub async fn connect_storage_backend(
    backend: &str,
    url: &str,
) -> anyhow::Result<Arc<dyn StorageBackend>> {
    use sqlx::postgres::PgPoolOptions;

    // Extract the target database name from the DSN.
    let target_db = url::Url::parse(url)
        .ok()
        .map(|u| u.path().trim_start_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "hissrv".to_string());

    // Step 1: Connect to the `postgres` maintenance database and auto-create
    // the target database if it does not already exist.
    let maintenance_url = replace_db_in_dsn(url, "postgres");
    let maint_pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&maintenance_url)
        .await
        .map_err(|e| anyhow::anyhow!("无法连接数据库服务器: {}", e))?;

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&target_db)
            .fetch_one(&maint_pool)
            .await
            .unwrap_or(false);

    if !exists {
        // Database names cannot be parameterized in DDL; safe here because
        // the name comes from our own saved config, not raw user input.
        let create_sql = format!(r#"CREATE DATABASE "{}""#, target_db.replace('"', ""));
        sqlx::query(&create_sql)
            .execute(&maint_pool)
            .await
            .map_err(|e| anyhow::anyhow!("自动创建数据库 '{}' 失败: {}", target_db, e))?;
        info!("Database '{}' created automatically", target_db);
    }
    maint_pool.close().await;

    // Step 2: Connect to the target database and initialise the schema.
    let pg_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(url)
        .await
        .map_err(|e| anyhow::anyhow!("连接数据库 '{}' 失败: {}", target_db, e))?;

    let storage: Arc<dyn StorageBackend> = match backend {
        "timescaledb" => {
            let b = Arc::new(crate::backend_tsdb::TimescaleDbBackend::new(pg_pool));
            b.init_schema().await?;
            b
        },
        "influxdb" => {
            let b = Arc::new(crate::backend_influx::InfluxDbBackend);
            b.init_schema().await?;
            b
        },
        _ => {
            let b = Arc::new(crate::backend_pg::PostgresBackend::new(pg_pool));
            b.init_schema().await?;
            b
        },
    };

    Ok(storage)
}

// ============================================================================
// Root / ping
// ============================================================================

#[utoipa::path(get, path = "/", tag = "Health",
    responses((status = 200, description = "服务基本信息")))]
async fn root() -> Json<Value> {
    Json(json!({
        "service": "hissrv",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running"
    }))
}

#[utoipa::path(get, path = "/ping", tag = "Health",
    responses((status = 200, description = "pong")))]
async fn ping() -> &'static str {
    "pong"
}

// ============================================================================
// Health
// ============================================================================

#[utoipa::path(get, path = "/hisApi/health", tag = "Health",
    responses((status = 200, description = "存储后端健康状态")))]
async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let backend = state.storage.read().await.clone();
    let storage_ok = backend.health_check().await;
    let buf_len = state.buffer.lock().await.len();
    let storage_enabled = state.storage_settings.read().await.enabled;

    Json(json!({
        "status": if storage_ok { "healthy" } else { "degraded" },
        "backend": backend.name(),
        "storage_enabled": storage_enabled,
        "storage_healthy": storage_ok,
        "buffer_size": buf_len,
    }))
}

// ============================================================================
// Data queries
// ============================================================================

#[utoipa::path(get, path = "/hisApi/data/query", tag = "Data",
    params(QueryRangeParams),
    responses(
        (status = 200, description = "分页历史数据", body = QueryResult),
        (status = 500, description = "查询失败"),
    ))]
async fn query_range(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueryRangeParams>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (default_page, max_page, max_days) = {
        let cfg = state.config.read().await;
        (
            cfg.default_page_size,
            cfg.max_page_size,
            cfg.max_time_range_days,
        )
    };

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params
        .page_size
        .unwrap_or(default_page)
        .min(max_page)
        .max(1);

    let backend = state.storage.read().await.clone();
    match backend
        .query_range(&params, default_page, max_page, max_days)
        .await
    {
        Ok((data, total)) => {
            let has_more = (page * page_size) < total;
            Ok(Json(json!({
                "status": "success",
                "message": format!("成功查询到 {} 条数据", data.len()),
                "data": data,
                "total": total,
                "page": page,
                "page_size": page_size,
                "has_more": has_more,
            })))
        },
        Err(e) => {
            error!("query_range error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "error", "message": e.to_string()})),
            ))
        },
    }
}

#[utoipa::path(get, path = "/hisApi/data/latest", tag = "Data",
    params(LatestParams),
    responses(
        (status = 200, description = "该点位最新一条历史记录", body = HistoryRecord),
        (status = 404, description = "暂无数据"),
        (status = 500, description = "查询失败"),
    ))]
async fn query_latest(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LatestParams>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let backend = state.storage.read().await.clone();
    match backend
        .query_latest(&params.redis_key, &params.point_id)
        .await
    {
        Ok(Some(record)) => Ok(Json(json!({
            "status": "success",
            "data": record,
        }))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"status": "not_found", "message": "No data found"})),
        )),
        Err(e) => {
            error!("query_latest error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "error", "message": e.to_string()})),
            ))
        },
    }
}

#[utoipa::path(get, path = "/hisApi/data/range", tag = "Data",
    responses(
        (status = 200, description = "数据时间范围与整体统计", body = DataStats),
        (status = 500, description = "查询失败"),
    ))]
async fn data_range(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let backend = state.storage.read().await.clone();
    match backend.get_stats().await {
        Ok(stats) => Ok(Json(json!({
            "earliest_timestamp": stats.earliest_timestamp,
            "latest_timestamp":   stats.latest_timestamp,
            "total_points":       stats.total_points,
            "channels":           stats.channels,
            "data_types":         stats.data_types,
        }))),
        Err(e) => {
            error!("data_range error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "error", "message": e.to_string()})),
            ))
        },
    }
}

// ============================================================================
// Metadata
// ============================================================================

#[utoipa::path(get, path = "/hisApi/channels", tag = "Meta",
    responses(
        (status = 200, description = "已存储数据的通道列表"),
        (status = 500, description = "查询失败"),
    ))]
async fn list_channels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let backend = state.storage.read().await.clone();
    match backend.list_channels().await {
        Ok(channels) => {
            let count = channels.len();
            Ok(Json(json!({
                "status": "success",
                "channels": channels,
                "count": count,
            })))
        },
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "error", "message": e.to_string()})),
        )),
    }
}

#[utoipa::path(get, path = "/hisApi/metrics", tag = "Meta",
    responses((status = 200, description = "服务运行指标（总点数、通道数、缓冲区大小等）")))]
async fn metrics(State(state): State<Arc<AppState>>) -> Json<Value> {
    let backend = state.storage.read().await.clone();
    let stats = backend.get_stats().await.unwrap_or_else(|_| DataStats {
        earliest_timestamp: None,
        latest_timestamp: None,
        total_points: 0,
        channels: vec![],
        data_types: vec![],
    });

    Json(json!({
        "total_points": stats.total_points,
        "channel_count": stats.channels.len(),
        "backend": backend.name(),
        "buffer_size": state.buffer.lock().await.len(),
    }))
}

// ============================================================================
// General service config CRUD (intervals, patterns, etc.)
// ============================================================================

#[utoipa::path(get, path = "/hisApi/config", tag = "Config",
    responses((status = 200, description = "当前服务配置", body = ServiceConfig)))]
async fn get_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cfg = state.config.read().await.clone();
    Json(json!({ "status": "success", "data": cfg }))
}

#[utoipa::path(put, path = "/hisApi/config", tag = "Config",
    request_body = ServiceConfig,
    responses(
        (status = 200, description = "配置已更新"),
        (status = 500, description = "保存失败"),
    ))]
async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(new_cfg): Json<ServiceConfig>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Err(e) = db_config::save_config(&state.sqlite, &new_cfg).await {
        error!("Failed to save config: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "error", "message": e.to_string()})),
        ));
    }

    *state.config.write().await = new_cfg;

    Ok(Json(
        json!({ "status": "success", "message": "配置已更新" }),
    ))
}

// ============================================================================
// Storage backend config & control
// ============================================================================

/// GET /hisApi/storage – return current storage config and connection status.
#[utoipa::path(get, path = "/hisApi/storage", tag = "Storage",
    responses((status = 200, description = "当前存储后端配置与连接状态")))]
async fn get_storage(State(state): State<Arc<AppState>>) -> Json<Value> {
    let ss = state.storage_settings.read().await.clone();
    let backend = state.storage.read().await.clone();
    let healthy = backend.health_check().await;

    // Parse stored DSN back into friendly fields for the frontend.
    let (host, port, database, username) = parse_dsn_fields(&ss.url);

    Json(json!({
        "status": "success",
        "data": {
            "enabled":        ss.enabled,
            "backend":        ss.backend,
            "host":           host,
            "port":           port,
            "database":       database,
            "username":       username,
            "active_backend": backend.name(),
            "connected":      healthy,
        }
    }))
}

/// PUT /hisApi/storage – 保存存储后端连接参数（**不会立即建立连接**）
///
/// 此接口只负责持久化配置，不尝试连接数据库，不影响当前运行状态。
/// 保存后可通过以下接口继续操作：
/// - `POST /hisApi/storage/test` — 验证连通性
/// - `POST /hisApi/storage/reconnect` — 应用新配置并正式建立连接
#[utoipa::path(put, path = "/hisApi/storage", tag = "Storage",
    request_body = StorageConfigRequest,
    responses(
        (status = 200, description = "参数已保存"),
        (status = 400, description = "参数错误（缺少必填字段）"),
        (status = 500, description = "保存失败"),
    ))]
async fn update_storage(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StorageConfigRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if req.host.is_empty() || req.database.is_empty() || req.username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"status": "error", "message": "host、database、username 为必填项"})),
        ));
    }

    // Always assemble DSN from the new params so GET always reflects what was saved.
    let dsn = req.to_dsn();

    let new_ss = StorageSettings {
        enabled: req.enabled,
        backend: req.backend.clone(),
        url: dsn,
    };

    if let Err(e) = db_config::save_storage(&state.sqlite, &new_ss).await {
        error!("Failed to persist storage config: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "error", "message": e.to_string()})),
        ));
    }

    // If disabling, immediately swap in NullBackend so writes stop.
    if !req.enabled {
        *state.storage.write().await = Arc::new(NullBackend);
        info!("Storage disabled, writes stopped");
    }

    *state.storage_settings.write().await = new_ss;

    Ok(Json(json!({
        "status": "success",
        "message": "参数已保存，如需连接请调用 POST /hisApi/storage/reconnect"
    })))
}

/// GET /hisApi/storage/test – 用已保存的参数测试数据库连通性
///
/// 无需请求体，直接使用 `PUT /hisApi/storage` 保存的 host/port/username/password。
/// 探测时连接到 PostgreSQL 内置的 `postgres` 维护库，该库在任何 PG 服务器上
/// 都存在，因此**业务数据库不需要提前创建**即可测试通过。
/// 不修改任何运行状态，不写入任何数据。
#[utoipa::path(get, path = "/hisApi/storage/test", tag = "Storage",
    responses(
        (status = 200, description = "连接测试成功"),
        (status = 400, description = "连接参数尚未配置，请先调用 PUT /hisApi/storage"),
        (status = 500, description = "连接失败，返回具体错误信息"),
    ))]
async fn test_storage(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let ss = state.storage_settings.read().await.clone();

    if ss.url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"status": "error", "message": "连接参数尚未配置，请先调用 PUT /hisApi/storage"}),
            ),
        ));
    }

    // Replace the database segment with "postgres" (the always-present
    // maintenance DB) so the test succeeds even before the target DB is created.
    let probe_dsn = replace_db_in_dsn(&ss.url, "postgres");

    // Parse host:port for a friendly success message.
    let addr = url::Url::parse(&ss.url)
        .map(|u| {
            format!(
                "{}:{}",
                u.host_str().unwrap_or("?"),
                u.port().unwrap_or(5432)
            )
        })
        .unwrap_or_else(|_| "?".to_string());

    match probe_connection(&probe_dsn).await {
        Ok(()) => Ok(Json(json!({
            "status": "success",
            "message": format!("成功连接到 {} (用已保存的账号验证通过)", addr)
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "error", "message": e.to_string()})),
        )),
    }
}

/// POST /hisApi/storage/reconnect – 使用已保存的参数正式建立连接
///
/// 连接成功后立即开始采集并写入历史数据。
/// 若当前 `enabled = false`，调用此接口也不会建立连接（需先通过 PUT 将 enabled 设为 true）。
#[utoipa::path(post, path = "/hisApi/storage/reconnect", tag = "Storage",
    responses(
        (status = 200, description = "重连成功"),
        (status = 400, description = "存储未配置或未启用"),
        (status = 500, description = "重连失败"),
    ))]
async fn reconnect_storage(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (enabled, backend_type, dsn) = {
        let ss = state.storage_settings.read().await;
        (ss.enabled, ss.backend.clone(), ss.url.clone())
    };

    if !enabled {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"status": "error", "message": "存储未启用，请先通过 PUT /hisApi/storage 将 enabled 设为 true"}),
            ),
        ));
    }

    if dsn.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"status": "error", "message": "连接参数未配置，请先调用 PUT /hisApi/storage"}),
            ),
        ));
    }

    match connect_storage_backend(&backend_type, &dsn).await {
        Ok(b) => {
            info!("Storage backend '{}' reconnected", backend_type);
            *state.storage.write().await = b;
            Ok(Json(json!({
                "status": "success",
                "message": format!("已成功连接至 '{}' 后端，开始采集历史数据", backend_type)
            })))
        },
        Err(e) => {
            error!("Storage reconnect failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "error", "message": e.to_string()})),
            ))
        },
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Parse a PostgreSQL DSN back into (host, port, database, username) for the
/// GET /hisApi/storage response.  Returns empty strings on parse failure.
fn parse_dsn_fields(dsn: &str) -> (String, u16, String, String) {
    if let Ok(url) = url::Url::parse(dsn) {
        let host = url.host_str().unwrap_or("").to_string();
        let port = url.port().unwrap_or(5432);
        let database = url.path().trim_start_matches('/').to_string();
        let username = url.username().to_string();
        return (host, port, database, username);
    }
    (String::new(), 5432, String::new(), String::new())
}

/// Replace the database segment in a PostgreSQL DSN with `new_db`.
/// Used by the test endpoint to probe against the always-present `postgres` DB.
fn replace_db_in_dsn(dsn: &str, new_db: &str) -> String {
    if let Ok(mut url) = url::Url::parse(dsn) {
        url.set_path(&format!("/{}", new_db));
        return url.to_string();
    }
    dsn.to_string()
}
