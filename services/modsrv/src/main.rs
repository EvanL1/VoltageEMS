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
    DEFAULT_TICK_MS, Result, RuleScheduler, bootstrap, routes,
    rule_routes::{RuleEngineState, create_rule_routes},
};
use voltage_calc::RtdbStateStore;
use voltage_rtdb_shm::{SharedConfig, UnifiedReader, is_shm_available};

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
    let sqlite_pool = state.instance_manager.pool().clone();
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

    // Initialize SharedConfig for shared memory access
    // Load SharedConfig parameters from database
    let shm_config = {
        let mut cfg = SharedConfig::default();

        // Helper to load usize value from service_config
        async fn load_usize(pool: &sqlx::SqlitePool, key: &str) -> Option<usize> {
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM service_config WHERE service_name = 'global' AND key = ?",
            )
            .bind(key)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
        }

        if let Some(v) = load_usize(&sqlite_pool, "shared_memory.max_slots").await {
            cfg = cfg.with_max_slots(v);
        }

        debug!("SharedConfig: max_slots={:?}", cfg.max_slots());
        cfg
    };

    // Load channel point counts for SHM layout (routing-independent)
    let channel_points = voltage_rtdb_shm::ChannelPointCounts::load_from_db(&sqlite_pool)
        .await
        .unwrap_or_else(|e| {
            warn!("Failed to load channel points: {}, using empty layout", e);
            voltage_rtdb_shm::ChannelPointCounts::new()
        });

    // Initialize UnifiedReader for cross-process zero-copy reads
    // Simplified: Header + PointSlots only, indexes built from channel points
    // Added retry mechanism for cold start race condition
    let shared_reader = {
        const MAX_RETRIES: u32 = 10;
        const BASE_DELAY_MS: u64 = 1000;
        const MAX_DELAY_MS: u64 = 15000;
        let mut retry_count = 0;

        loop {
            if is_shm_available(&shm_config) {
                // Open reader with RoutingCache (builds indexes from routing)
                match UnifiedReader::open(&shm_config, &channel_points) {
                    Ok(reader) => {
                        info!(
                            "UnifiedReader opened: {} slots, {} instances, {} channels",
                            reader.slot_count(),
                            reader.instance_ids(&routing_cache).len(),
                            reader.channel_ids().len()
                        );
                        break Some(Arc::new(reader));
                    },
                    Err(e) if retry_count < MAX_RETRIES => {
                        let delay_ms = (BASE_DELAY_MS * 2u64.pow(retry_count)).min(MAX_DELAY_MS);
                        info!(
                            "SharedMemory not ready (retry {}/{}, next in {}ms): {}",
                            retry_count + 1,
                            MAX_RETRIES,
                            delay_ms,
                            e
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        retry_count += 1;
                    },
                    Err(e) => {
                        warn!(
                            "UnifiedReader unavailable after {} retries: {}",
                            MAX_RETRIES, e
                        );
                        break None;
                    },
                }
            } else if retry_count < MAX_RETRIES {
                let delay_ms = (BASE_DELAY_MS * 2u64.pow(retry_count)).min(MAX_DELAY_MS);
                info!(
                    "SharedMemory path not found (retry {}/{}, next in {}ms), waiting for comsrv...",
                    retry_count + 1,
                    MAX_RETRIES,
                    delay_ms
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                retry_count += 1;
            } else {
                warn!(
                    "SharedMemory unavailable after {} retries; M2C dispatch will be unavailable (SHM writer not configured)",
                    MAX_RETRIES
                );
                break None;
            }
        }
    };

    // Initialize UnifiedWriter for M2C actions (Control/Adjustment via SHM)
    // Only open if reader succeeded (SHM file exists)
    let shm_action_writer = if shared_reader.is_some() {
        match voltage_rtdb_shm::UnifiedWriter::open_for_actions(&shm_config, &channel_points) {
            Ok(writer) => {
                info!("UnifiedWriter (actions) opened for M2C via SHM");
                Some(Arc::new(writer))
            },
            Err(e) => {
                warn!("UnifiedWriter (actions) unavailable: {}", e);
                None
            },
        }
    } else {
        None
    };

    // Configure ShmDispatch with SHM components for M2C via shared memory
    // ShmDispatch uses ArcSwapOption/OnceLock for delayed initialization
    // ShmNotifier is shared between ShmDispatch and RuleScheduler for unified M2C dispatch
    let shm_notifier: Option<Arc<tokio::sync::Mutex<voltage_rtdb_shm::ShmNotifier>>> =
        if let Some(ref writer) = shm_action_writer {
            // Set SHM action writer for direct M2C writes (+ store config for rebuild)
            state
                .shm_dispatch
                .set_writer(Arc::clone(writer), shm_config.clone());
            info!("ShmDispatch: SHM action writer configured");

            // Connect ShmNotifier for event-driven M2C dispatch (~1-2ms latency).
            // connect_default() never returns Err (connect failure yields a degraded Ok),
            // but handle the Err arm defensively: always call set_notifier so that the
            // auto-reconnect logic inside ShmNotifier can activate on future notify() calls.
            match voltage_rtdb_shm::ShmNotifier::connect_default().await {
                Ok(notifier) => {
                    let notifier = Arc::new(tokio::sync::Mutex::new(notifier));
                    if state.shm_dispatch.set_notifier(Arc::clone(&notifier)) {
                        info!("ShmDispatch: ShmNotifier configured for event-driven dispatch");
                    }
                    Some(notifier)
                },
                Err(e) => {
                    // Defensive path: create a degraded notifier so set_notifier is always
                    // called and auto-reconnect can activate when comsrv UDS becomes ready.
                    info!(
                        "ShmNotifier unavailable (UDS listener not ready), will auto-reconnect: {}",
                        e
                    );
                    let notifier = Arc::new(tokio::sync::Mutex::new(
                        voltage_rtdb_shm::ShmNotifier::disabled(),
                    ));
                    state.shm_dispatch.set_notifier(Arc::clone(&notifier));
                    Some(notifier)
                },
            }
        } else {
            None
        };

    // Spawn SHM writer auto-rebuild task.
    // When dispatch() detects a generation mismatch (comsrv restarted), it fires
    // rebuild_trigger. This task re-opens the writer with exponential backoff,
    // restoring M2C dispatch without a modsrv restart.
    {
        let rebuild_notify = state.shm_dispatch.rebuild_trigger();
        let rebuild_dispatch = Arc::clone(&state.shm_dispatch);
        let rebuild_pool = sqlite_pool.clone();
        let rebuild_shm_config = shm_config.clone();
        let rebuild_token = shutdown_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = rebuild_notify.notified() => {},
                    _ = rebuild_token.cancelled() => break,
                }
                info!("SHM rebuild triggered — attempting to reopen writer...");
                const MAX_RETRIES: u32 = 10;
                const BASE_DELAY_MS: u64 = 1000;
                const MAX_DELAY_MS: u64 = 15000;
                let mut retry_count = 0u32;
                let ok = loop {
                    // Reload channel points (layout may have changed)
                    let cp = voltage_rtdb_shm::ChannelPointCounts::load_from_db(&rebuild_pool)
                        .await
                        .unwrap_or_else(|e| {
                            warn!("SHM rebuild: failed to load channel points: {}", e);
                            voltage_rtdb_shm::ChannelPointCounts::new()
                        });
                    match voltage_rtdb_shm::UnifiedWriter::open_for_actions(
                        &rebuild_shm_config,
                        &cp,
                    ) {
                        Ok(writer) => {
                            let writer = Arc::new(writer);
                            rebuild_dispatch
                                .set_writer(Arc::clone(&writer), rebuild_shm_config.clone());
                            info!("SHM rebuild: writer restored successfully");
                            break true;
                        },
                        Err(e) if retry_count < MAX_RETRIES => {
                            let delay = (BASE_DELAY_MS * 2u64.pow(retry_count)).min(MAX_DELAY_MS);
                            info!(
                                "SHM rebuild retry {}/{} in {}ms: {}",
                                retry_count + 1,
                                MAX_RETRIES,
                                delay,
                                e
                            );
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                            retry_count += 1;
                        },
                        Err(e) => {
                            warn!(
                                "SHM rebuild failed after {} retries: {}. \
                                 Will retry on next generation mismatch.",
                                MAX_RETRIES, e
                            );
                            break false;
                        },
                    }
                };
                if ok {
                    info!("SHM auto-rebuild complete — M2C dispatch restored");
                }
            }
        });
    }

    // Load max_concurrency from global config (SQLite key-value table)
    let max_concurrency: usize = sqlx::query_scalar::<_, String>(
        "SELECT value FROM service_config WHERE service_name = 'global' AND key = 'rules.max_concurrency'",
    )
    .fetch_optional(&sqlite_pool)
    .await
    .ok()
    .flatten()
    .and_then(|s| s.parse().ok())
    .unwrap_or(4);

    // Create rule scheduler with two-tier priority (SharedMemory > Redis)
    // SHM writer enables M2C actions via shared memory (primary path)
    // ShmNotifier enables UDS event notification for immediate dispatch
    // RtdbStateStore ensures stateful functions (period_delta, integrate, etc.) persist across restarts
    let rule_log_root = PathBuf::from("logs/modsrv");
    let state_store = Arc::new(RtdbStateStore::new(Arc::clone(&rtdb)));
    let mut scheduler = RuleScheduler::with_state_store(
        rtdb,
        routing_cache,
        sqlite_pool.clone(),
        tick_ms,
        rule_log_root,
        state_store,
        shared_reader,
        shm_action_writer,
        shm_notifier,
    );
    scheduler.set_max_concurrency(max_concurrency);
    let scheduler = Arc::new(scheduler);

    info!(
        "Rule scheduler: tick_ms={}, max_concurrency={}",
        tick_ms, max_concurrency
    );

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

    info!("Model Service (with Rule Engine) shutdown complete");
    Ok(())
}
