//! Communication Service (`ComsrvRust`)
//!
//! A high-performance, async-first industrial communication service written in Rust.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::serve;
use clap::Parser;
#[cfg(feature = "swagger-ui")]
use comsrv::api::routes::ComsrvApiDoc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;

use common::service_bootstrap::ServiceInfo;
use comsrv::core::config::DEFAULT_PORT;
use errors::VoltageResult;

// comsrv imports
use comsrv::{
    api::{
        command_cache::CommandTxCache,
        routes::{create_api_routes, set_service_start_time},
    },
    cleanup_provider::ComsrvCleanupProvider,
    core::{
        bootstrap::{self, Args},
        channels::ChannelManager,
        config::ConfigManager,
    },
    error::ComSrvError,
    runtime::{start_cleanup_task, start_communication_service},
    shutdown_services, wait_for_shutdown,
};
use voltage_routing::load_routing_maps;
use voltage_rtdb::{is_shm_available, ChannelToSlotIndex, SharedConfig, UnifiedWriter};

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
    // Note: Config not loaded yet, use VOLTAGE_LOG_DIR env or default
    bootstrap::initialize_logging(&service_args, &service_info, None)?;
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
    let db_path = service_args.get_db_path("comsrv");
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
    let mut redis_config = common::redis::RedisPoolConfig::from_url(&app_config.redis.url);
    redis_config.max_connections = max_connections as u32;

    let (redis_url, redis_client) = common::bootstrap_database::setup_redis_with_config(
        Some(app_config.redis.url.clone()),
        redis_config,
    )
    .await?;

    // ============ Phase 1: Create initial rtdb for cleanup ============
    // Reuse the existing connection pool instead of creating a new one
    let redis_rtdb = voltage_rtdb::RedisRtdb::from_client(redis_client.clone());

    // Perform Redis cleanup first (before loading routing)
    info!("Performing Redis cleanup based on database configuration...");
    let cleanup_provider = ComsrvCleanupProvider::new(sqlite_pool.clone());
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
        // Load routing maps from shared library
        let maps = load_routing_maps(&sqlite_pool)
            .await
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to load routing: {}", e)))?;

        info!("Loaded routing cache: {} total routes", maps.total_routes());

        Arc::new(voltage_rtdb::RoutingCache::from_maps(
            maps.c2m, maps.m2c, maps.c2c,
        ))
    };

    // RTDB is a pure storage abstraction
    // Routing is handled by ChannelManager using routing_cache

    // ============ Phase 2.5: Initialize UnifiedWriter (shared memory) ============
    // UnifiedWriter: creates shared memory with indexes from RoutingCache
    // Simplified: no SlotMeta, indexes are Vec in process memory
    let (shared_writer, channel_index) = {
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

            if let Some(v) = load_usize(&sqlite_pool, "shared_memory.max_slots").await {
                cfg = cfg.with_max_slots(v);
            }

            debug!("SharedConfig: max_slots={:?}", cfg.max_slots());
            cfg
        };

        // Create UnifiedWriter from RoutingCache (automatic slot allocation)
        // is_shm_available checks if parent directory exists (Docker mount point)
        if is_shm_available(&config) {
            match UnifiedWriter::create(&config, &routing_cache) {
                Ok(writer) => {
                    info!(
                        "UnifiedWriter: created with {} slots (Header + PointSlots only)",
                        writer.slot_count()
                    );

                    // Build channel → slot index from writer's layouts
                    let index = ChannelToSlotIndex::from_unified_writer(&writer);
                    info!("ChannelToSlotIndex: {} mappings", index.len());

                    (Some(Arc::new(writer)), Some(Arc::new(index)))
                },
                Err(e) => {
                    tracing::warn!("UnifiedWriter not available: {}", e);
                    (None, None)
                },
            }
        } else {
            info!("SharedMemory path not found, skipping (non-Docker environment)");
            (None, None)
        }
    };

    // CommandTxCache for O(1) hot path access
    // Bypasses ChannelManager RwLock for Control/Adjustment writes
    let command_tx_cache = Arc::new(CommandTxCache::new());
    info!("CommandTxCache initialized (O(1) hot path for Control/Adjustment)");

    // Initialize services
    let shutdown_token = CancellationToken::new();

    // Use concrete type (native AFIT requires static dispatch)
    let rtdb: Arc<voltage_rtdb::RedisRtdb> = Arc::new(redis_rtdb);

    // Create channel manager with optional shared memory and CommandTxCache support
    // Lock-free architecture - no RwLock wrapper needed
    // Removed VecRtdb - SharedMemory + Redis two-tier architecture
    let channel_manager = Arc::new(ChannelManager::with_shared_memory(
        rtdb,
        routing_cache,
        sqlite_pool.clone(),
        shared_writer,
        channel_index,
        Some(Arc::clone(&command_tx_cache)),
    ));

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
    let configured_count =
        start_communication_service(config_manager.clone(), Arc::clone(&channel_manager)).await?;
    let (cleanup_handle, cleanup_token) =
        start_cleanup_task(Arc::clone(&channel_manager), configured_count);
    let warning_token = shutdown_token.clone();
    let warning_handle = tokio::spawn(async move {
        if let Err(e) =
            common::warning_monitor::start_warning_monitor(redis_url, warning_token).await
        {
            tracing::error!("Warning monitor error: {}", e);
        }
    });

    // Start API server
    set_service_start_time(chrono::Utc::now());
    let app = create_api_routes(
        Arc::clone(&channel_manager),
        redis_client,
        sqlite_pool,
        Arc::clone(&command_tx_cache),
    );

    #[cfg(feature = "swagger-ui")]
    let app = {
        info!("Swagger UI feature ENABLED - initializing at /docs");
        let openapi = ComsrvApiDoc::openapi();
        let merged = app.merge(SwaggerUi::new("/docs").url("/openapi.json", openapi));
        info!("Swagger UI configured successfully");
        merged
    };

    #[cfg(not(feature = "swagger-ui"))]
    info!("Swagger UI feature DISABLED");

    // Note: HTTP request logging middleware is applied in create_api_routes()

    let socket = tokio::net::TcpSocket::new_v4()
        .map_err(|e| ComSrvError::ConnectionError(format!("Failed to create socket: {}", e)))?;
    socket
        .set_reuseaddr(true)
        .map_err(|e| ComSrvError::ConnectionError(format!("Failed to set SO_REUSEADDR: {}", e)))?;
    socket
        .bind(addr)
        .map_err(|e| ComSrvError::ConnectionError(format!("Failed to bind to {}: {}", addr, e)))?;
    let listener = socket
        .listen(1024)
        .map_err(|e| ComSrvError::ConnectionError(format!("Failed to listen: {}", e)))?;

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
    wait_for_shutdown().await;
    shutdown_services(
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
