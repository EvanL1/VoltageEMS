//! hissrv – Historical data service (Rust)
//!
//! Collects real-time data from Redis, persists it to a pluggable SQL backend
//! (TimescaleDB or PostgreSQL), and exposes a REST API for historical queries.
//!
//! Storage backend is configured at **runtime** via `PUT /hisApi/storage`.
//! The service starts with storage disabled; no database connection is required
//! at startup – only `VOLTAGE_DB_PATH` (SQLite) and `REDIS_URL` are needed.

use std::net::SocketAddr;
use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

mod backend_influx;
mod backend_null;
mod backend_pg;
mod backend_tsdb;
mod collector;
mod config;
mod db_config;
mod models;
mod routes;
mod scheduler;
mod state;
mod storage;

use crate::backend_null::NullBackend;
use crate::config::EnvConfig;
use crate::state::AppState;
use crate::storage::StorageBackend;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Env config ────────────────────────────────────────────────────────────
    let env = Arc::new(EnvConfig::default());

    // ── Logging ───────────────────────────────────────────────────────────────
    let service_info = common::service_bootstrap::ServiceInfo::new(
        "hissrv",
        "Historical data service",
        env.api_port,
    );
    common::service_bootstrap::init_logging(&service_info, None)
        .map_err(|e| anyhow::anyhow!("Failed to init logging: {}", e))?;
    common::logging::enable_sighup_log_reopen();
    common::service_bootstrap::print_startup_banner(&service_info);

    info!("hissrv starting");

    // ── Shared SQLite – config table ──────────────────────────────────────────
    if let Some(dir) = std::path::Path::new(&env.db_path).parent() {
        std::fs::create_dir_all(dir)?;
    }
    let sqlite = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(common::bootstrap_database::sqlite_connect_options(
            &env.db_path,
        ))
        .await
        .map_err(|e| anyhow::anyhow!("SQLite connect failed: {} path={}", e, env.db_path))?;

    db_config::create_config_table(&sqlite).await?;
    let service_cfg = db_config::load_config(&sqlite).await?;
    let storage_cfg = db_config::load_storage(&sqlite).await?;

    // ── Storage backend – lazy / runtime-configurable ─────────────────────────
    // Start with the null backend.  If the saved config has storage enabled,
    // attempt to reconnect immediately so a service restart preserves the setting.
    let initial_storage: Arc<dyn StorageBackend> =
        if storage_cfg.enabled && !storage_cfg.url.is_empty() {
            match routes::connect_storage_backend(&storage_cfg.backend, &storage_cfg.url).await {
                Ok(b) => {
                    info!(
                        "Storage backend '{}' connected at startup",
                        storage_cfg.backend
                    );
                    b
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect storage at startup ({}), starting as disabled: {}",
                        storage_cfg.backend,
                        e
                    );
                    Arc::new(NullBackend)
                },
            }
        } else {
            info!("Storage disabled – configure via PUT /hisApi/storage");
            Arc::new(NullBackend)
        };

    // ── Redis ─────────────────────────────────────────────────────────────────
    let (_, redis_client) =
        common::bootstrap_database::setup_redis_connection(Some(env.redis_url.clone()))
            .await
            .map_err(|e| anyhow::anyhow!("Redis connect failed: {}", e))?;

    let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client));

    // ── App State ─────────────────────────────────────────────────────────────
    let state = Arc::new(AppState {
        storage: Arc::new(RwLock::new(initial_storage)),
        sqlite,
        rtdb,
        env: Arc::clone(&env),
        config: Arc::new(RwLock::new(service_cfg)),
        storage_settings: Arc::new(RwLock::new(storage_cfg)),
        buffer: Arc::new(Mutex::new(Vec::new())),
    });

    // ── Background tasks ──────────────────────────────────────────────────────
    let shutdown = CancellationToken::new();
    scheduler::spawn_all(Arc::clone(&state), shutdown.clone());

    // ── HTTP server ───────────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::build_router(Arc::clone(&state))
        .layer(axum::middleware::from_fn(
            common::logging::http_request_logger,
        ))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(cors);

    let addr: SocketAddr = format!("{}:{}", env.api_host, env.api_port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid bind address: {}", e))?;

    info!("hissrv listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            common::shutdown::wait_for_shutdown().await;
            info!("Shutdown signal received");
            shutdown.cancel();
        })
        .await?;

    common::logging::shutdown_logging_tasks().await;
    Ok(())
}
