//! Model Service (ModSrv)
//!
//! Model management service supporting measurement/action separation architecture.
//! Rule Engine API is integrated on the same port (6002).

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

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
use voltage_rtdb::{is_shm_available, SharedConfig, SharedVecRtdbReader, VecRtdb};

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

    // Create VecRtdb for read-through cache (Round 113)
    // The cache will be populated lazily as rule variables are read
    let vec_rtdb = Arc::new(VecRtdb::new());
    info!("VecRtdb initialized (read-through cache for rule engine)");

    // Initialize SharedVecRtdbReader for cross-process zero-copy reads (Round 127)
    // This enables direct mmap access to comsrv's shared memory via Docker tmpfs volume
    let shared_reader = {
        let config = SharedConfig::default();
        if is_shm_available(&config) {
            match SharedVecRtdbReader::open(&config) {
                Ok(reader) => {
                    let stats = reader.stats();
                    info!(
                        "SharedVecRtdbReader opened: {} instances, {} points",
                        stats.instance_count, stats.total_points
                    );
                    Some(Arc::new(reader))
                },
                Err(e) => {
                    warn!("SharedVecRtdbReader unavailable: {}", e);
                    None
                },
            }
        } else {
            info!("SharedMemory path not found, skipping (non-Docker environment)");
            None
        }
    };

    // Create rule scheduler with three-tier priority (SharedMemory > VecRtdb > Redis)
    let rule_log_root = PathBuf::from("logs/modsrv");
    let scheduler = Arc::new(RuleScheduler::with_shared_reader(
        rtdb,
        routing_cache,
        sqlite_pool.clone(),
        tick_ms,
        rule_log_root,
        vec_rtdb,
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
