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
use voltage_rtdb::{is_shm_available, ChannelToSlotIndex, SharedConfig, SharedVecRtdbWriter};

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

    // ============ Phase 2.5: Initialize shared memory (optional) ============
    // SharedVecRtdbWriter provides zero-copy cross-process data sharing via tmpfs
    // Load SharedConfig from global config (SQLite key-value table)
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
        if is_shm_available(&config) {
            match SharedVecRtdbWriter::open(&config) {
                Ok(mut writer) => {
                    // Register all instances with both Measurement and Action points
                    // Measurement points from C2M routing (Channel → Instance)
                    let mut measurements: std::collections::HashMap<u32, Vec<u32>> =
                        std::collections::HashMap::new();
                    for ((_, _, _), target) in routing_cache.c2m_iter() {
                        measurements
                            .entry(target.instance_id)
                            .or_default()
                            .push(target.point_id);
                    }

                    // Action points from M2C routing (Instance → Channel)
                    let mut actions: std::collections::HashMap<u32, Vec<u32>> =
                        std::collections::HashMap::new();
                    for ((instance_id, _, point_id), _) in routing_cache.m2c_iter() {
                        actions.entry(instance_id).or_default().push(point_id);
                    }

                    // Merge all instance IDs and register with both point types
                    let all_instances: std::collections::HashSet<u32> =
                        measurements.keys().chain(actions.keys()).copied().collect();
                    let mut registered_count = 0;
                    let mut total_m_points = 0;
                    let mut total_a_points = 0;
                    for instance_id in all_instances {
                        let m_points = measurements
                            .get(&instance_id)
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]);
                        let a_points = actions
                            .get(&instance_id)
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]);
                        if let Err(e) = writer.register_instance(instance_id, m_points, a_points) {
                            tracing::warn!("Failed to register instance {}: {}", instance_id, e);
                        } else {
                            registered_count += 1;
                            total_m_points += m_points.len();
                            total_a_points += a_points.len();
                        }
                    }
                    info!(
                        "SharedMemory: registered {} instances ({} M + {} A points)",
                        registered_count, total_m_points, total_a_points
                    );

                    // Register all channels with T/S/C/A points
                    // Collect channel points from routing
                    let mut ch_telemetry: std::collections::HashMap<u32, Vec<u32>> =
                        std::collections::HashMap::new();
                    let mut ch_signal: std::collections::HashMap<u32, Vec<u32>> =
                        std::collections::HashMap::new();
                    let mut ch_control: std::collections::HashMap<u32, Vec<u32>> =
                        std::collections::HashMap::new();
                    let mut ch_adjustment: std::collections::HashMap<u32, Vec<u32>> =
                        std::collections::HashMap::new();

                    // C2M routing gives us T/S points (uplink: Channel → Instance)
                    for ((channel_id, point_type, point_id), _) in routing_cache.c2m_iter() {
                        match point_type {
                            voltage_model::PointType::Telemetry => {
                                ch_telemetry.entry(channel_id).or_default().push(point_id);
                            },
                            voltage_model::PointType::Signal => {
                                ch_signal.entry(channel_id).or_default().push(point_id);
                            },
                            _ => {}, // C2M should only have T/S
                        }
                    }

                    // M2C routing gives us C/A points (downlink: Instance → Channel)
                    for (_, target) in routing_cache.m2c_iter() {
                        match target.point_type {
                            voltage_model::PointType::Control => {
                                ch_control
                                    .entry(target.channel_id)
                                    .or_default()
                                    .push(target.point_id);
                            },
                            voltage_model::PointType::Adjustment => {
                                ch_adjustment
                                    .entry(target.channel_id)
                                    .or_default()
                                    .push(target.point_id);
                            },
                            _ => {}, // M2C should only have C/A
                        }
                    }

                    // Register all channels
                    let all_channels: std::collections::HashSet<u32> = ch_telemetry
                        .keys()
                        .chain(ch_signal.keys())
                        .chain(ch_control.keys())
                        .chain(ch_adjustment.keys())
                        .copied()
                        .collect();

                    let mut ch_count = 0;
                    let mut ch_points = 0;
                    for channel_id in all_channels {
                        let t = ch_telemetry
                            .get(&channel_id)
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]);
                        let s = ch_signal
                            .get(&channel_id)
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]);
                        let c = ch_control
                            .get(&channel_id)
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]);
                        let a = ch_adjustment
                            .get(&channel_id)
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]);

                        if let Err(e) = writer.register_channel(channel_id, t, s, c, a) {
                            tracing::warn!("Failed to register channel {}: {}", channel_id, e);
                        } else {
                            ch_count += 1;
                            ch_points += t.len() + s.len() + c.len() + a.len();
                        }
                    }
                    info!(
                        "SharedMemory: registered {} channels ({} T/S/C/A points)",
                        ch_count, ch_points
                    );

                    // Build pre-computed channel → slot direct mapping
                    let index = ChannelToSlotIndex::build(&routing_cache, &writer);
                    info!("SharedMemory initialized: {} channel mappings", index.len());
                    (Some(Arc::new(writer)), Some(Arc::new(index)))
                },
                Err(e) => {
                    tracing::warn!("SharedMemory not available: {}", e);
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
