//! Service Bootstrap and Initialization
//!
//! Handles all service initialization including logging, configuration,
//! database connections, and component setup.

use crate::config::{ModsrvConfig, ModsrvQueries};
use common::bootstrap_args::ServiceArgs;
use common::bootstrap_database::{setup_redis_connection, setup_sqlite_pool};
use common::bootstrap_system::{check_system_requirements_with, SystemRequirements};
use common::redis::RedisClient;
use common::service_bootstrap::{get_service_port, ServiceInfo};
use common::sqlite::{ServiceConfigLoader, SqliteClient};
use common::{ApiConfig, BaseServiceConfig, RedisConfig, DEFAULT_API_HOST, DEFAULT_REDIS_URL};
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

// Import from error module directly (works in both lib and bin context)
use super::error::{ModSrvError, Result};

use crate::app_state::AppState;
use crate::instance_manager::InstanceManager;
use crate::product_loader::ProductLoader;
use crate::redis_state;

/// Initialize service info for unified bootstrap
pub fn create_service_info() -> ServiceInfo {
    ServiceInfo::new(
        "modsrv",
        "Model Service - Instance & Routing Management",
        6002,
    )
}

/// Initialize logging and environment
pub fn init_environment(service_info: &ServiceInfo) -> Result<()> {
    // Load environment variables from .env file
    common::service_bootstrap::load_development_env();

    // Initialize logging using service_bootstrap (config not loaded yet, use env/default)
    common::service_bootstrap::init_logging(service_info, None)
        .map_err(|e| ModSrvError::ConfigError(format!("Failed to initialize logging: {}", e)))?;

    // Print startup banner using service_bootstrap
    common::service_bootstrap::print_startup_banner(service_info);

    // Enable SIGHUP-triggered log reopen for long-running processes
    common::logging::enable_sighup_log_reopen();

    info!("ModSrv starting");

    Ok(())
}

/// Load configuration from SQLite database
pub async fn load_configuration(service_info: &ServiceInfo) -> Result<ModsrvConfig> {
    let db_path = ServiceArgs::default().get_db_path("modsrv");

    if !std::path::Path::new(&db_path).exists() {
        error!("DB not found: {}", db_path);
        return Err(ModSrvError::DatabaseError(format!(
            "Database not found: {}",
            db_path
        )));
    }

    info!("Loading config: {}", db_path);
    let service_config = ServiceConfigLoader::new(&db_path, "modsrv")
        .await
        .map_err(|e| {
            ModSrvError::ConfigError(format!("Failed to initialize config loader: {}", e))
        })?
        .load_config()
        .await
        .map_err(|e| ModSrvError::ConfigError(format!("Failed to load configuration: {}", e)))?;

    // Convert ServiceConfig to ModsrvConfig (following Rules pattern)
    let mut config = ModsrvConfig {
        service: BaseServiceConfig {
            name: service_config.service_name,
            description: service_config
                .extra_config
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            version: service_config
                .extra_config
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        },
        api: ApiConfig {
            host: DEFAULT_API_HOST.to_string(),
            port: service_config.port,
        },
        redis: RedisConfig {
            url: service_config.redis_url,
            enabled: true,
        },
        products_path: service_config
            .extra_config
            .get("products_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        instances_path: service_config
            .extra_config
            .get("instances_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        auto_load_instances: service_config
            .extra_config
            .get("auto_load_instances")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
    };

    debug!("Config loaded");

    // Apply configuration priority: DB > ENV > Default
    config.api.port = get_service_port(config.api.port, service_info);

    // Perform runtime validation
    validate_configuration(&config)?;

    Ok(config)
}

/// Validate configuration
fn validate_configuration(config: &ModsrvConfig) -> Result<()> {
    debug!("Validating config");

    let skip_full_check = std::env::var("SKIP_VALIDATION").is_ok();
    if !skip_full_check {
        // Basic runtime validation
        if config.api.port == 0 {
            error!("Invalid port: 0");
            return Err(ModSrvError::InvalidConfig(
                "api.port: Port cannot be 0".to_string(),
            ));
        }
        if config.redis.url.is_empty() {
            error!("Redis URL missing");
            return Err(ModSrvError::MissingConfig("redis.url".to_string()));
        }
        debug!("Config valid");
    }

    debug!("Validation done");
    Ok(())
}

/// Wrapper for Redis setup with ModsrvConfig
async fn setup_redis_with_config(config: &ModsrvConfig) -> Result<(String, Arc<RedisClient>)> {
    // Redis URL configuration - handled separately with retry logic
    let redis_url_from_db = if !config.redis.url.is_empty() && config.redis.url != DEFAULT_REDIS_URL
    {
        Some(config.redis.url.clone())
    } else {
        None
    };

    setup_redis_connection(redis_url_from_db)
        .await
        .map_err(Into::into)
}

/// Wrapper for SQLite setup with modsrv defaults
async fn setup_sqlite() -> Result<SqlitePool> {
    let db_path = ServiceArgs::default().get_db_path("modsrv");
    info!("SQLite: {}", db_path);
    setup_sqlite_pool(&db_path).await.map_err(Into::into)
}

/// Load and sync products to Redis
pub async fn load_products<R>(
    _config: &ModsrvConfig,
    sqlite_pool: &SqlitePool,
    _rtdb: &Arc<R>,
) -> Result<Arc<ProductLoader>>
where
    R: voltage_rtdb::Rtdb,
{
    // Products are now loaded from code definitions (no config directory needed)
    let product_loader = ProductLoader::new(sqlite_pool.clone());

    // Initialize product database tables
    product_loader.init_database().await?;

    // Products must be in database (loaded by monarch)
    let product_count: i64 = sqlx::query_scalar(ModsrvQueries::COUNT_PRODUCTS)
        .fetch_one(sqlite_pool)
        .await
        .unwrap_or(0);

    if product_count == 0 {
        let allow_empty = std::env::var("MODSRV_ALLOW_EMPTY")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        if allow_empty {
            warn!("No products (ALLOW_EMPTY)");
        } else {
            error!("No products in DB");
            return Err(ModSrvError::DatabaseError(
                "No products/instances found in voltage.db".to_string(),
            ));
        }
    }

    info!("{} products loaded", product_count);
    Ok(Arc::new(product_loader))
}

/// Setup instance manager and sync instances
#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
pub async fn setup_instance_manager(
    sqlite_pool: &SqlitePool,
    _rtdb: Arc<voltage_rtdb::RedisRtdb>,
    routing_cache: Arc<voltage_rtdb::RoutingCache>,
    product_loader: Arc<ProductLoader>,
) -> Result<Arc<InstanceManager<voltage_rtdb::MemoryRtdb>>> {
    // Create MemoryRtdb for testing (ignore the injected RedisRtdb)
    let rtdb = Arc::new(voltage_rtdb::MemoryRtdb::new());

    // Create instance manager with RTDB and routing cache
    let instance_manager = Arc::new(InstanceManager::new(
        sqlite_pool.clone(),
        rtdb,
        routing_cache,
        product_loader,
    ));

    Ok(instance_manager)
}

#[cfg(not(test))]
pub async fn setup_instance_manager(
    sqlite_pool: &SqlitePool,
    rtdb: Arc<voltage_rtdb::RedisRtdb>,
    routing_cache: Arc<voltage_rtdb::RoutingCache>,
    product_loader: Arc<ProductLoader>,
) -> Result<Arc<InstanceManager<voltage_rtdb::RedisRtdb>>> {
    // RTDB is a pure storage abstraction
    // M2C routing is handled externally by voltage-routing library

    // Create instance manager with RTDB and routing cache
    let instance_manager = Arc::new(InstanceManager::new(
        sqlite_pool.clone(),
        rtdb,
        routing_cache,
        product_loader,
    ));

    // Instances must be in database (loaded by monarch)
    let instance_count: i64 = sqlx::query_scalar(ModsrvQueries::COUNT_INSTANCES)
        .fetch_one(sqlite_pool)
        .await
        .unwrap_or(0);

    if instance_count == 0 {
        let allow_empty = std::env::var("MODSRV_ALLOW_EMPTY")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        if allow_empty {
            warn!("No instances (ALLOW_EMPTY)");
        } else {
            error!("No instances in DB");
            return Err(ModSrvError::DatabaseError(
                "No products/instances found in voltage.db".to_string(),
            ));
        }
    }

    info!("{} instances loaded", instance_count);

    // Initialize real-time data structures in Redis (M/A Hash + name mappings)
    // Note: This does NOT sync metadata - only creates empty Hash structures for real-time data
    if let Err(e) = instance_manager.sync_instances_to_redis().await {
        error!("Redis init failed: {}", e);
        // Service continues anyway (structures will be created on-demand)
    }

    Ok(instance_manager)
}

/// Validate routing integrity and check for orphan records
///
/// This function validates that all routing table entries point to existing
/// channel points. It's called during service startup to ensure data integrity.
///
/// # Arguments
/// * `sqlite_pool` - SQLite connection pool
///
/// # Returns
/// * `Ok(())` - Validation passed or orphans found but service can continue
/// * `Err(ModSrvError)` - Critical validation failure
///
/// # Behavior
/// - Reports orphan measurement_routing records (T/S points not found)
/// - Reports orphan action_routing records (C/A points not found)
/// - Logs warnings but allows service to start
/// - Suggests running migration script if orphans found
pub async fn validate_routing_integrity(sqlite_pool: &SqlitePool) -> Result<()> {
    debug!("Validating routing");

    // Check measurement_routing for orphan T/S points
    let orphan_telemetry: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM measurement_routing
        WHERE channel_type = 'T'
          AND NOT EXISTS (
              SELECT 1 FROM telemetry_points
              WHERE telemetry_points.channel_id = measurement_routing.channel_id
                AND telemetry_points.point_id = measurement_routing.channel_point_id
          )
        "#,
    )
    .fetch_one(sqlite_pool)
    .await
    .unwrap_or(0);

    let orphan_signal: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM measurement_routing
        WHERE channel_type = 'S'
          AND NOT EXISTS (
              SELECT 1 FROM signal_points
              WHERE signal_points.channel_id = measurement_routing.channel_id
                AND signal_points.point_id = measurement_routing.channel_point_id
          )
        "#,
    )
    .fetch_one(sqlite_pool)
    .await
    .unwrap_or(0);

    // Check action_routing for orphan C/A points
    let orphan_control: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM action_routing
        WHERE channel_type = 'C'
          AND NOT EXISTS (
              SELECT 1 FROM control_points
              WHERE control_points.channel_id = action_routing.channel_id
                AND control_points.point_id = action_routing.channel_point_id
          )
        "#,
    )
    .fetch_one(sqlite_pool)
    .await
    .unwrap_or(0);

    let orphan_adjustment: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM action_routing
        WHERE channel_type = 'A'
          AND NOT EXISTS (
              SELECT 1 FROM adjustment_points
              WHERE adjustment_points.channel_id = action_routing.channel_id
                AND adjustment_points.point_id = action_routing.channel_point_id
          )
        "#,
    )
    .fetch_one(sqlite_pool)
    .await
    .unwrap_or(0);

    let total_orphans = orphan_telemetry + orphan_signal + orphan_control + orphan_adjustment;

    if total_orphans > 0 {
        warn!(
            "Orphan routes: T={}, S={}, C={}, A={}",
            orphan_telemetry, orphan_signal, orphan_control, orphan_adjustment
        );
    } else {
        debug!("Routing valid");
    }

    Ok(())
}

/// Refresh routing cache from SQLite database
///
/// This function reloads routing data from SQLite and updates the in-memory
/// routing cache. It's called after routing management operations (create/update/delete)
/// to ensure the cache stays synchronized with the database.
///
/// # Arguments
/// * `sqlite_pool` - SQLite connection pool
/// * `routing_cache` - Shared routing cache to refresh
///
/// # Returns
/// * `Ok(usize)` - Number of routes loaded (c2m + m2c)
/// * `Err(anyhow::Error)` - Database or parsing errors
pub async fn refresh_routing_cache(
    sqlite_pool: &SqlitePool,
    routing_cache: &Arc<voltage_rtdb::RoutingCache>,
) -> anyhow::Result<usize> {
    debug!("Refreshing routes");

    // Load fresh routing data from database via shared library
    let maps = voltage_routing::load_routing_maps(sqlite_pool).await?;

    let total_routes = maps.c2m.len() + maps.m2c.len();

    // Update cache atomically (clears old data and loads new)
    routing_cache.update(maps.c2m, maps.m2c, maps.c2c);

    info!("Routes refreshed: {}", total_routes);

    Ok(total_routes)
}

/// Load routing cache from Redis (legacy method, for compatibility)
///
/// NOTE: This method is kept for backward compatibility but should be
/// avoided during service initialization. Use `load_routing_maps_from_sqlite`
/// instead for better performance.
pub async fn load_routing_cache<R>(rtdb: &Arc<R>) -> Result<Arc<voltage_rtdb::RoutingCache>>
where
    R: voltage_rtdb::Rtdb,
{
    debug!("Loading routes from Redis");

    // Load C2M routing (Channel to Model)
    let c2m_data = redis_state::get_routing(
        rtdb.as_ref(),
        redis_state::RoutingDirection::ChannelToModel,
        None,
    )
    .await
    .unwrap_or_else(|e| {
        warn!("C2M load failed: {}", e);
        std::collections::HashMap::new()
    });

    // Load M2C routing (Model to Channel)
    let m2c_data = redis_state::get_routing(
        rtdb.as_ref(),
        redis_state::RoutingDirection::ModelToChannel,
        None,
    )
    .await
    .unwrap_or_else(|e| {
        warn!("M2C load failed: {}", e);
        std::collections::HashMap::new()
    });

    info!("Routes: {} C2M, {} M2C", c2m_data.len(), m2c_data.len());

    Ok(Arc::new(voltage_rtdb::RoutingCache::from_maps(
        c2m_data,
        m2c_data,
        std::collections::HashMap::new(), // C2C routing not yet implemented
    )))
}

/// Create application state with all initialized components
pub async fn create_app_state(service_info: &ServiceInfo) -> Result<Arc<AppState>> {
    // Initialize environment
    init_environment(service_info)?;

    // Check system requirements
    let requirements = SystemRequirements {
        min_cpu_cores: 2,
        min_memory_mb: 512,
        recommended_cpu_cores: 4,
        recommended_memory_mb: 1024,
    };
    check_system_requirements_with(requirements)?;

    // Load configuration
    let mut config = load_configuration(service_info).await?;

    // Setup Redis using common function
    let (redis_url, redis_client) = setup_redis_with_config(&config).await?;
    config.redis.url = redis_url;
    let config = Arc::new(config);

    // Setup SQLite using common function
    let sqlite_pool = setup_sqlite().await?;
    let sqlite_client = Some(Arc::new(SqliteClient::from_pool(sqlite_pool.clone())));

    // ============ Phase 1: Load routing configuration from unified database ============
    debug!("Loading routing config");

    // Validate routing integrity before loading (check for orphan records)
    validate_routing_integrity(&sqlite_pool).await?;

    let routing_cache = {
        // Load routing maps directly from the same SQLite pool via shared library
        let maps = voltage_routing::load_routing_maps(&sqlite_pool).await?;

        // Save lengths before moving maps
        let c2m_len = maps.c2m.len();
        let m2c_len = maps.m2c.len();

        let cache = Arc::new(voltage_rtdb::RoutingCache::from_maps(
            maps.c2m, maps.m2c, maps.c2c,
        ));

        info!("Routes: {} C2M, {} M2C", c2m_len, m2c_len);

        cache
    };

    // ============ Phase 2: Create RTDB ============
    debug!("Creating RedisRtdb");
    let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client.clone()));

    // ============ Phase 3: Use the single rtdb for all operations ============
    // Perform Redis cleanup (uses basic methods, no routing triggered)
    debug!("Redis cleanup");
    let cleanup_provider = crate::cleanup_provider::ModsrvCleanupProvider::new(sqlite_pool.clone());
    match voltage_rtdb::cleanup::cleanup_invalid_keys(&cleanup_provider, rtdb.as_ref()).await {
        Ok(deleted) => {
            if deleted > 0 {
                info!("Redis cleanup: {} keys removed", deleted);
            }
        },
        Err(e) => {
            warn!("Redis cleanup failed: {}", e);
        },
    }

    // Rebuild instance name index for O(1) name→ID lookups
    if let Err(e) = rebuild_instance_name_index(&sqlite_pool, rtdb.as_ref()).await {
        warn!("Name index rebuild failed: {}", e);
    }

    // Load products (uses basic methods, no routing triggered)
    let product_loader = load_products(&config, &sqlite_pool, &rtdb).await?;

    // Setup instance manager (routing handled externally by voltage-routing library)
    let instance_manager = setup_instance_manager(
        &sqlite_pool,
        rtdb,
        routing_cache.clone(),
        Arc::clone(&product_loader),
    )
    .await?;

    // Create application state
    Ok(Arc::new(AppState::new(
        config,
        sqlite_client,
        product_loader,
        instance_manager,
    )))
}

/// Rebuild instance name index from SQLite database
///
/// This function scans all instances in SQLite and rebuilds the reverse index
/// (inst:name:index Hash) for O(1) instance name lookups. This is needed for:
/// - Migration from old deployment without the index
/// - Recovery after index corruption
///
/// @input pool: SQLite connection pool
/// @input rtdb: Redis RTDB instance
/// @output `Result<usize>` - Number of instances indexed
async fn rebuild_instance_name_index<R: voltage_rtdb::Rtdb>(
    pool: &SqlitePool,
    rtdb: &R,
) -> Result<usize> {
    use bytes::Bytes;

    debug!("Rebuilding name index");

    // Query all instances from SQLite
    let instances: Vec<(i32, String)> =
        sqlx::query_as("SELECT instance_id, instance_name FROM instances ORDER BY instance_id")
            .fetch_all(pool)
            .await
            .map_err(|e| ModSrvError::DatabaseError(format!("Failed to query instances: {}", e)))?;

    if instances.is_empty() {
        debug!("No instances for index");
        return Ok(0);
    }

    // Build index fields for batch write
    let fields: Vec<(String, Bytes)> = instances
        .iter()
        .map(|(id, name)| (name.clone(), Bytes::from(id.to_string())))
        .collect();

    // Write all mappings in one batch operation
    rtdb.hash_mset("inst:name:index", fields)
        .await
        .map_err(|e| {
            ModSrvError::InternalError(format!("Failed to rebuild instance name index: {}", e))
        })?;

    let count = instances.len();
    info!("Name index: {} instances", count);

    Ok(count)
}
