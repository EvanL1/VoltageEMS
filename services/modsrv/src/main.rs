//! Model Service (ModSrv)
//!
//! Model management service supporting measurement/action separation architecture

use modsrv::Result;
use std::{net::SocketAddr, sync::Arc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;

// Import from modsrv library instead of declaring modules
use modsrv::{bootstrap, routes};

// Private modules not exported from lib
mod instance_logger;
mod time_series;
mod virtual_calc;

#[tokio::main]
async fn main() -> Result<()> {
    // Create service info
    let service_info = bootstrap::create_service_info();

    // Initialize cancellation token for graceful shutdown
    let shutdown_token = CancellationToken::new();
    debug!("Shutdown token initialized");

    // Create application state with all initialized components
    let state = bootstrap::create_app_state(&service_info).await?;

    // Start real-time virtual point calculation if enabled
    let _virtual_calc_handle = if std::env::var("ENABLE_REALTIME_VIRTUAL_CALC")
        .unwrap_or_else(|_| "true".to_string())
        == "true"
    {
        let polling_interval = std::env::var("VIRTUAL_CALC_INTERVAL_MS")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u64>()
            .unwrap_or(1000);

        info!(
            "Starting real-time virtual point calculation with {}ms interval",
            polling_interval
        );

        // Create dedicated connections for virtual calculation task
        let redis_url = state.config.redis.url.clone();
        // Try to build Redis client; if it fails, disable the task but keep service running.
        let redis_client = match common::redis::RedisClient::new(&redis_url).await {
            Ok(client) => Some(Arc::new(client)),
            Err(e) => {
                error!(
                    "Failed to create Redis client for virtual calculation: {}",
                    e
                );
                info!("Disabling real-time virtual point calculation task due to Redis error");
                None
            },
        };

        if let Some(redis_client) = redis_client {
            let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client));

            let db_path =
                std::env::var("VOLTAGE_DB_PATH").unwrap_or_else(|_| "data/voltage.db".to_string());
            match sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path)).await {
                Ok(pool) => Some(virtual_calc::start_realtime_virtual_calculation(
                    rtdb,
                    pool,
                    polling_interval,
                )),
                Err(e) => {
                    error!("Failed to connect to SQLite for virtual calculation: {}", e);
                    info!("Disabling real-time virtual point calculation task due to SQLite error");
                    None
                },
            }
        } else {
            None
        }
    } else {
        info!("Virtual point calculation is disabled (ENABLE_REALTIME_VIRTUAL_CALC=false)");
        None
    };

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

    // Start HTTP service
    let addr = SocketAddr::from(([0, 0, 0, 0], state.config.api.port));

    // Create socket with SO_REUSEADDR to allow quick restart
    let socket = tokio::net::TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?; // Allow reuse of address
    socket.bind(addr)?;
    let listener = socket.listen(1024)?;

    info!("Model Service started on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET/POST /api/instances - Instance management");
    info!("  GET /api/products - Product management");
    info!("  GET /api/instances/:id/data - Get instance data");
    info!("  POST /api/instances/:id/sync - Sync measurement");
    info!("  POST /api/instances/:id/action - Execute action");
    info!("  POST /api/instances/sync/all - Sync all instances");

    // Prepare graceful shutdown
    let cancel_token = shutdown_token.clone();
    let shutdown_signal = async move {
        cancel_token.cancelled().await;
        info!("Shutdown signal received, stopping model service...");
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

    let server_handle = tokio::spawn(server_task);
    info!("Main server started");

    // Wait for shutdown signal (Ctrl+C or SIGTERM)
    wait_for_shutdown().await;
    info!("Initiating graceful shutdown...");

    // Signal all tasks to shutdown
    shutdown_token.cancel();

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

    // Abort warning monitor if still running
    warning_handle.abort();
    let _ = warning_handle.await; // Ignore abort error

    info!("Model Service shutdown complete");
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
