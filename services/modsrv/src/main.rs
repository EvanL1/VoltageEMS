//! Model Service (ModSrv)
//!
//! Model management service supporting measurement/action separation architecture
//! Now includes integrated Rule Engine on port 6003.

use std::{net::SocketAddr, sync::Arc};

use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;

// modsrv imports
use modsrv::{
    bootstrap, routes,
    rule_routes::{create_rule_routes, RuleEngineState},
    Result, RuleScheduler,
};

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
        let openapi = routes::ModsrvApiDoc::openapi();
        let merged = app.merge(SwaggerUi::new("/docs").url("/openapi.json", openapi));
        info!("Swagger UI configured successfully");
        merged
    };

    #[cfg(not(feature = "swagger-ui"))]
    info!("Swagger UI feature DISABLED");

    // ============================================================================
    // Initialize Rule Engine (port 6003)
    // ============================================================================
    let rule_engine_port = 6003u16;
    let sqlite_pool = state.instance_manager.pool.clone();
    let rtdb = state.instance_manager.rtdb.clone();
    let routing_cache = state.instance_manager.routing_cache().clone();

    // Create rule scheduler
    let scheduler = Arc::new(RuleScheduler::new(rtdb, routing_cache, sqlite_pool.clone()));

    // Load rules into scheduler
    match scheduler.load_rules().await {
        Ok(count) => info!("Rule Engine: loaded {} rules", count),
        Err(e) => warn!("Rule Engine: failed to load rules: {}", e),
    }

    // Create rule engine state and routes
    let rule_state = Arc::new(RuleEngineState::new(sqlite_pool, Arc::clone(&scheduler)));
    let rule_app = create_rule_routes(rule_state);

    // Start HTTP service (model API - port 6002)
    let addr = SocketAddr::from(([0, 0, 0, 0], state.config.api.port));

    // Rule engine address (port 6003)
    let rule_addr = SocketAddr::from(([0, 0, 0, 0], rule_engine_port));

    // Create socket for model API (port 6002)
    let socket = tokio::net::TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.bind(addr)?;
    let listener = socket.listen(1024)?;

    // Create socket for rule engine API (port 6003)
    let rule_socket = tokio::net::TcpSocket::new_v4()?;
    rule_socket.set_reuseaddr(true)?;
    rule_socket.bind(rule_addr)?;
    let rule_listener = rule_socket.listen(1024)?;

    info!("Model Service started on {}", addr);
    info!("Rule Engine started on {}", rule_addr);
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
    info!("Rule Engine API endpoints (port {}):", rule_engine_port);
    info!("  GET /health - Rule engine health check");
    info!("  GET/POST /api/rules - Rule management");
    info!("  GET/PUT/DELETE /api/rules/:id - Single rule operations");
    info!("  POST /api/rules/:id/execute - Execute rule manually");
    info!("  GET /api/scheduler/status - Scheduler status");
    info!("  POST /api/scheduler/reload - Reload rules");

    // Prepare graceful shutdown for model server
    let model_cancel_token = shutdown_token.clone();
    let model_shutdown_signal = async move {
        model_cancel_token.cancelled().await;
        info!("Shutdown signal received, stopping model service...");
    };

    // Prepare graceful shutdown for rule server
    let rule_cancel_token = shutdown_token.clone();
    let rule_shutdown_signal = async move {
        rule_cancel_token.cancelled().await;
        info!("Shutdown signal received, stopping rule engine...");
    };

    // Spawn model server task
    let model_server_task = async move {
        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(model_shutdown_signal)
            .await
        {
            error!("Model server error: {}", e);
        }
    };

    // Spawn rule server task
    let rule_server_task = async move {
        if let Err(e) = axum::serve(rule_listener, rule_app)
            .with_graceful_shutdown(rule_shutdown_signal)
            .await
        {
            error!("Rule server error: {}", e);
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

    // Spawn server tasks
    let model_server_handle = tokio::spawn(model_server_task);
    let rule_server_handle = tokio::spawn(rule_server_task);
    info!("Model server started (port {})", state.config.api.port);
    info!("Rule server started (port {})", rule_engine_port);

    // Start rule scheduler in background
    let scheduler_handle = {
        let scheduler = Arc::clone(&scheduler);
        tokio::spawn(async move {
            scheduler.start().await;
        })
    };
    info!("Rule scheduler started");

    // Wait for shutdown signal (Ctrl+C or SIGTERM)
    wait_for_shutdown().await;
    info!("Initiating graceful shutdown...");

    // Signal all tasks to shutdown
    shutdown_token.cancel();

    // Stop scheduler
    scheduler.stop();

    // Wait for tasks to complete with timeout
    let shutdown_timeout = tokio::time::Duration::from_secs(30);

    // Wait for model server task
    match tokio::time::timeout(shutdown_timeout, model_server_handle).await {
        Ok(Ok(())) => info!("Model server shut down gracefully"),
        Ok(Err(e)) => error!("Model server task failed: {}", e),
        Err(_) => {
            error!("Model server shutdown timed out");
        },
    }

    // Wait for rule server task
    match tokio::time::timeout(shutdown_timeout, rule_server_handle).await {
        Ok(Ok(())) => info!("Rule server shut down gracefully"),
        Ok(Err(e)) => error!("Rule server task failed: {}", e),
        Err(_) => {
            error!("Rule server shutdown timed out");
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

// Unified shutdown signal: handle Ctrl+C and (on Unix) SIGTERM
async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        use tracing::warn;

        let term_signal = match signal(SignalKind::terminate()) {
            Ok(sig) => Some(sig),
            Err(e) => {
                warn!(
                    "Failed to install SIGTERM handler: {}. Service will only respond to Ctrl+C",
                    e
                );
                None
            },
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = async {
                if let Some(mut sig) = term_signal {
                    sig.recv().await;
                } else {
                    // If SIGTERM handler failed, wait forever (only Ctrl+C will work)
                    std::future::pending::<()>().await
                }
            } => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
