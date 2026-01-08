//! Model Service (ModSrv)
//!
//! Model management service supporting measurement/action separation architecture.
//! Rule Engine API is integrated on the same port (6002).

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;

// modsrv imports
#[cfg(feature = "swagger-ui")]
use modsrv::rule_routes::RuleApiDoc;
use modsrv::{
    bootstrap, routes,
    rule_routes::{create_rule_routes, RuleEngineState},
    Result, RuleScheduler, DEFAULT_TICK_MS,
};
use voltage_rtdb::{is_shm_available, SharedConfig, SharedVecRtdbReader};

#[tokio::main]
async fn main() -> Result<()> {
    // Create service info
    let service_info = bootstrap::create_service_info();

    // Initialize cancellation token for graceful shutdown
    let shutdown_token = CancellationToken::new();
    debug!("Shutdown token initialized");

    // Create application state with all initialized components
    let state = bootstrap::create_app_state(&service_info).await?;

    // Create API routes using the routes module
    let app = routes::create_routes(Arc::clone(&state));

    #[cfg(feature = "swagger-ui")]
    let app = {
        info!("Swagger UI feature ENABLED - initializing at /docs");
        // Merge ModsrvApiDoc with RuleApiDoc for complete OpenAPI documentation
        let openapi = routes::ModsrvApiDoc::openapi().nest("", RuleApiDoc::openapi());
        let merged = app.merge(SwaggerUi::new("/docs").url("/openapi.json", openapi));
        info!("Swagger UI configured successfully (including Rule Engine API)");
        merged
    };

    #[cfg(not(feature = "swagger-ui"))]
    info!("Swagger UI feature DISABLED");

    // ============================================================================
    // Initialize Rule Engine (integrated on port 6002)
    // ============================================================================
    let sqlite_pool = state.instance_manager.pool.clone();
    let rtdb = state.instance_manager.rtdb.clone();
    let routing_cache = state.instance_manager.routing_cache().clone();

    // Load tick_ms from global config (SQLite key-value table)
    let tick_ms: u64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM service_config WHERE service_name = 'global' AND key = 'rules.tick_ms'",
    )
    .fetch_optional(&sqlite_pool)
    .await
    .ok()
    .flatten()
    .and_then(|s| s.parse().ok())
    .unwrap_or(DEFAULT_TICK_MS);

    debug!("Rule scheduler tick_ms: {}", tick_ms);

    // Initialize SharedVecRtdbReader for cross-process zero-copy reads
    // Uses smart path selection - works on any filesystem
    // Added retry mechanism for cold start race condition
    // Load SharedConfig from global config (SQLite key-value table)
    // This enables direct mmap access to comsrv's shared memory
    let shared_reader = {
        // Load SharedConfig parameters from database
        let config = {
            let mut cfg = SharedConfig::default();

            // Helper to load usize value from service_config
            async fn load_usize(pool: &sqlx::SqlitePool, key: &str) -> Option<usize> {
                sqlx::query_scalar::<_, String>(&format!(
                    "SELECT value FROM service_config WHERE service_name = 'global' AND key = '{}'",
                    key
                ))
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
            }

            if let Some(v) = load_usize(&sqlite_pool, "shared_memory.max_instances").await {
                cfg = cfg.with_max_instances(v);
            }
            if let Some(v) = load_usize(&sqlite_pool, "shared_memory.max_points_per_instance").await
            {
                cfg = cfg.with_max_points_per_instance(v);
            }
            if let Some(v) = load_usize(&sqlite_pool, "shared_memory.max_channels").await {
                cfg = cfg.with_max_channels(v);
            }
            if let Some(v) = load_usize(&sqlite_pool, "shared_memory.max_points_per_channel").await
            {
                cfg = cfg.with_max_points_per_channel(v);
            }

            debug!(
                "SharedConfig: max_instances={}, max_channels={}, points_per_inst={}, points_per_ch={}",
                cfg.max_instances, cfg.max_channels, cfg.max_points_per_instance, cfg.max_points_per_channel
            );
            cfg
        };
        const MAX_RETRIES: u32 = 3;
        const RETRY_DELAY: Duration = Duration::from_secs(2);
        let mut retry_count = 0;

        loop {
            if is_shm_available(&config) {
                match SharedVecRtdbReader::open(&config) {
                    Ok(reader) => {
                        let stats = reader.stats();
                        info!(
                            "SharedVecRtdbReader opened: {} instances, {} points",
                            stats.instance_count, stats.total_points
                        );
                        break Some(Arc::new(reader));
                    },
                    Err(e) if retry_count < MAX_RETRIES => {
                        info!(
                            "SharedMemory not ready (retry {}/{}): {}",
                            retry_count + 1,
                            MAX_RETRIES,
                            e
                        );
                        tokio::time::sleep(RETRY_DELAY).await;
                        retry_count += 1;
                    },
                    Err(e) => {
                        warn!(
                            "SharedVecRtdbReader unavailable after {} retries: {}",
                            MAX_RETRIES, e
                        );
                        break None;
                    },
                }
            } else if retry_count < MAX_RETRIES {
                info!(
                    "SharedMemory path not found (retry {}/{}), waiting for comsrv...",
                    retry_count + 1,
                    MAX_RETRIES
                );
                tokio::time::sleep(RETRY_DELAY).await;
                retry_count += 1;
            } else {
                info!(
                    "SharedMemory path not found after {} retries, using Redis fallback",
                    MAX_RETRIES
                );
                break None;
            }
        }
    };

    // Create rule scheduler with two-tier priority (SharedMemory > Redis)
    // Removed VecRtdb - using SharedMemory + Redis two-tier architecture
    let rule_log_root = PathBuf::from("logs/modsrv");
    let scheduler = Arc::new(RuleScheduler::with_shared_reader(
        rtdb,
        routing_cache,
        sqlite_pool.clone(),
        tick_ms,
        rule_log_root,
        shared_reader,
    ));

    // Load rules into scheduler
    match scheduler.load_rules().await {
        Ok(count) => info!("Rule Engine: loaded {} rules", count),
        Err(e) => warn!("Rule Engine: failed to load rules: {}", e),
    }

    // Create rule engine state and routes
    let rule_state = Arc::new(RuleEngineState::new(sqlite_pool, Arc::clone(&scheduler)));
    let rule_routes = create_rule_routes(rule_state);

    // Merge rule routes into the main app (both on port 6002)
    let app = app.merge(rule_routes);

    // Start HTTP service (model API + rule engine - port 6002)
    let addr = SocketAddr::from(([0, 0, 0, 0], state.config.api.port));

    // Create socket for unified API (port 6002)
    let socket = tokio::net::TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.bind(addr)?;
    let listener = socket.listen(1024)?;

    info!("Model Service (with Rule Engine) started on {}", addr);
    info!("");
    info!("Model API endpoints (port {}):", state.config.api.port);
    info!("  GET /health - Health check");
    info!("  GET/POST /api/instances - Instance management");
    info!("  GET /api/products - Product management");
    info!("  GET /api/instances/:id/data - Get instance data");
    info!("  POST /api/instances/:id/sync - Sync measurement");
    info!("  POST /api/instances/:id/action - Execute action");
    info!("  POST /api/instances/sync/all - Sync all instances");
    info!("");
    info!(
        "Rule Engine API endpoints (port {}):",
        state.config.api.port
    );
    info!("  GET/POST /api/rules - Rule management");
    info!("  GET/PUT/DELETE /api/rules/:id - Single rule operations");
    info!("  POST /api/rules/:id/execute - Execute rule manually");
    info!("  GET /api/scheduler/status - Scheduler status");
    info!("  POST /api/scheduler/reload - Reload rules");

    // Prepare graceful shutdown
    let cancel_token = shutdown_token.clone();
    let shutdown_signal = async move {
        cancel_token.cancelled().await;
        info!("Shutdown signal received, stopping service...");
    };

    // Spawn server task
    let server_task = async move {
        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
        {
            error!("Server error: {}", e);
        }
    };

    // Start warning monitor for real-time alerts
    let warning_redis_url = state.config.redis.url.clone();
    let warning_token = shutdown_token.clone();
    let warning_handle = tokio::spawn(async move {
        if let Err(e) =
            common::warning_monitor::start_warning_monitor(warning_redis_url, warning_token).await
        {
            error!("Warning monitor error: {}", e);
        }
    });
    info!("Warning monitor started");

    // Spawn server task
    let server_handle = tokio::spawn(server_task);
    info!("Server started (port {})", state.config.api.port);

    // Start rule scheduler in background
    let scheduler_handle = {
        let scheduler = Arc::clone(&scheduler);
        tokio::spawn(async move {
            scheduler.start().await;
        })
    };
    info!("Rule scheduler started");

    // Wait for shutdown signal (Ctrl+C or SIGTERM)
    common::shutdown::wait_for_shutdown().await;
    info!("Initiating graceful shutdown...");

    // Signal all tasks to shutdown
    shutdown_token.cancel();

    // Stop scheduler
    scheduler.stop();

    // Wait for tasks to complete with timeout
    let shutdown_timeout = tokio::time::Duration::from_secs(30);

    // Wait for server task
    match tokio::time::timeout(shutdown_timeout, server_handle).await {
        Ok(Ok(())) => info!("Server shut down gracefully"),
        Ok(Err(e)) => error!("Server task failed: {}", e),
        Err(_) => {
            error!("Server shutdown timed out");
        },
    }

    // Wait for scheduler to stop
    match tokio::time::timeout(shutdown_timeout, scheduler_handle).await {
        Ok(Ok(())) => info!("Scheduler shut down gracefully"),
        Ok(Err(e)) => error!("Scheduler task failed: {}", e),
        Err(_) => {
            error!("Scheduler shutdown timed out");
        },
    }

    // Abort warning monitor if still running
    warning_handle.abort();
    let _ = warning_handle.await; // Ignore abort error

    info!("Model Service (with Rule Engine) shutdown complete");
    Ok(())
}
