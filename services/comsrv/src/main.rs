//! Communication Service (`ComsrvRust`)
//!
//! A high-performance, async-first industrial communication service written in Rust.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::serve;
use clap::Parser;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;

use common::service_bootstrap::ServiceInfo;
use voltage_config::comsrv::DEFAULT_PORT;
use voltage_config::error::VoltageResult;

use comsrv::core::bootstrap::{self, Args};
use comsrv::core::combase::ChannelManager;
use comsrv::core::config::ConfigManager;
use comsrv::ComSrvError;

#[tokio::main]
async fn main() -> VoltageResult<()> {
    // Parse arguments and initialize
    let args = Args::parse();
    let service_args = args.clone().into();

    let service_info = ServiceInfo::new(
        "comsrv",
        "Industrial Communication Service - Multi-Protocol Support",
        DEFAULT_PORT,
    );

    // Bootstrap: logging (API logging enabled by default), banner, system checks
    bootstrap::initialize_logging(&service_args, &service_info)?;
    // Enable SIGHUP-triggered log reopen
    common::logging::enable_sighup_log_reopen();
    if !args.no_color {
        common::service_bootstrap::print_startup_banner(&service_info);
    }
    bootstrap::check_system_requirements()?;

    // Validation mode: validate and exit
    if args.validate {
        bootstrap::validate_configuration().await?;
        info!("Validation completed successfully");
        return Ok(());
    }

    // Load configuration from unified database
    let db_path =
        std::env::var("VOLTAGE_DB_PATH").unwrap_or_else(|_| "data/voltage.db".to_string());
    info!(
        "Loading configuration from unified SQLite database: {}",
        db_path
    );
    let config_manager = Arc::new(ConfigManager::load().await?);
    let app_config = config_manager.config();

    // Create SQLite pool for API endpoints
    let sqlite_pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path))
        .await
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to create SQLite pool: {}", e)))?;

    // Calculate dynamic Redis connection pool size based on channel count
    let channel_count = app_config.channels.len();
    let max_connections = (channel_count * 2 + 30).max(50); // Minimum 50 connections
    info!(
        "Dynamic connection pool sizing: {} channels → {} max connections",
        channel_count, max_connections
    );

    // Setup Redis connection with custom pool configuration
    let mut redis_config = common::redis::RedisConfig::from_url(&app_config.redis.url);
    redis_config.max_connections = max_connections as u32;

    let (redis_url, redis_client) = common::bootstrap_database::setup_redis_with_config(
        Some(app_config.redis.url.clone()),
        redis_config,
    )
    .await?;

    // ============ Phase 1: Create initial rtdb for cleanup ============
    let mut redis_rtdb = voltage_rtdb::RedisRtdb::new(&redis_url).await?;

    // Perform Redis cleanup first (before loading routing)
    info!("Performing Redis cleanup based on database configuration...");
    let cleanup_provider =
        comsrv::cleanup_provider::ComsrvCleanupProvider::new(sqlite_pool.clone());
    match voltage_rtdb::cleanup_invalid_keys(&cleanup_provider, &redis_rtdb).await {
        Ok(deleted) => {
            if deleted > 0 {
                info!("Redis cleanup completed: {} invalid keys removed", deleted);
            } else {
                info!("Redis cleanup completed: no invalid keys found");
            }
        },
        Err(e) => {
            error!("Redis cleanup failed (continuing anyway): {}", e);
        },
    }

    // ============ Phase 2: Load routing configuration from unified database ============
    info!("Loading routing cache from unified database...");
    let routing_cache = {
        // Load routing maps directly from the same SQLite pool
        let (c2m_data, m2c_data, c2c_data) =
            comsrv::core::bootstrap::load_routing_maps_from_sqlite(&sqlite_pool)
                .await
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to load routing: {}", e)))?;

        let c2m_len = c2m_data.len();
        let m2c_len = m2c_data.len();
        let c2c_len = c2c_data.len();
        let cache = Arc::new(voltage_config::RoutingCache::from_maps(
            c2m_data, m2c_data, c2c_data,
        ));

        info!(
            "Loaded routing cache: {} C2M routes, {} M2C routes, {} C2C routes",
            c2m_len, m2c_len, c2c_len
        );

        cache
    };

    // ============ Phase 3: Inject routing into rtdb ============
    redis_rtdb.set_routing_cache(routing_cache.clone()).await;

    // Initialize services
    let shutdown_token = CancellationToken::new();

    // Wrap RedisRtdb as trait object for ChannelManager
    let rtdb: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(redis_rtdb);

    // Create channel manager (mutable state, needs lock)
    let channel_manager = Arc::new(RwLock::new(ChannelManager::with_sqlite_pool(
        rtdb,
        routing_cache,
        sqlite_pool.clone(),
    )));

    // Determine bind address and start server
    let bind_address = bootstrap::determine_bind_address(
        args.bind_address,
        &app_config.api.host,
        app_config.api.port,
    );
    let addr: SocketAddr = bind_address.parse().map_err(|e| {
        ComSrvError::ConfigError(format!("Invalid bind address '{}': {}", bind_address, e))
    })?;

    info!("Starting {} service", app_config.service.name);
    if app_config.redis.enabled {
        info!("Redis storage enabled at: {}", app_config.redis.url);
    }

    // Start communication channels
    let configured_count = comsrv::runtime::start_communication_service(
        config_manager.clone(),
        Arc::clone(&channel_manager),
    )
    .await?;
    let (cleanup_handle, cleanup_token) =
        comsrv::runtime::start_cleanup_task(Arc::clone(&channel_manager), configured_count);
    let warning_token = shutdown_token.clone();
    let warning_handle = tokio::spawn(async move {
        if let Err(e) =
            common::warning_monitor::start_warning_monitor(redis_url, warning_token).await
        {
            tracing::error!("Warning monitor error: {}", e);
        }
    });

    // Start API server
    comsrv::api::routes::set_service_start_time(chrono::Utc::now());
    let app = comsrv::api::routes::create_api_routes(
        Arc::clone(&channel_manager),
        redis_client,
        sqlite_pool,
    );

    #[cfg(feature = "swagger-ui")]
    let app = {
        info!("Swagger UI feature ENABLED - initializing at /docs");
        let openapi = comsrv::api::routes::ComsrvApiDoc::openapi();
        let merged = app.merge(SwaggerUi::new("/docs").url("/openapi.json", openapi));
        info!("Swagger UI configured successfully");
        merged
    };

    #[cfg(not(feature = "swagger-ui"))]
    info!("Swagger UI feature DISABLED");

    // Note: HTTP request logging middleware is applied in create_api_routes()

    let socket = tokio::net::TcpSocket::new_v4()
        .map_err(|e| ComSrvError::NetworkError(format!("Failed to create socket: {}", e)))?;
    socket
        .set_reuseaddr(true)
        .map_err(|e| ComSrvError::NetworkError(format!("Failed to set SO_REUSEADDR: {}", e)))?;
    socket
        .bind(addr)
        .map_err(|e| ComSrvError::NetworkError(format!("Failed to bind to {}: {}", addr, e)))?;
    let listener = socket
        .listen(1024)
        .map_err(|e| ComSrvError::NetworkError(format!("Failed to listen: {}", e)))?;

    info!("API server listening on http://{}", addr);
    info!("Health check: http://{}/health", addr);

    let server = serve(listener, app);
    let server_token = shutdown_token.clone();
    let server_handle = tokio::spawn(async move {
        let shutdown = async move { server_token.cancelled().await };
        if let Err(e) = server.with_graceful_shutdown(shutdown).await {
            error!("Server error: {}", e);
        }
    });

    // Wait for shutdown and cleanup
    comsrv::wait_for_shutdown().await;
    comsrv::shutdown_services(
        channel_manager,
        shutdown_token,
        cleanup_token,
        cleanup_handle,
        server_handle,
        warning_handle,
    )
    .await;

    Ok(())
}
