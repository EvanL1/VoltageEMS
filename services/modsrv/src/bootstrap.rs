//! Service Bootstrap and Initialization
//!
//! Handles all service initialization including logging, configuration,
//! database connections, and component setup.

use crate::config::ModsrvConfig;
use common::bootstrap_args::ServiceArgs;
use common::bootstrap_database::{setup_redis_connection, setup_sqlite_pool};
use common::bootstrap_system::{SystemRequirements, check_system_requirements_with};
use common::redis::RedisClient;
use common::service_bootstrap::{ServiceInfo, get_service_port};
use common::sqlite::ServiceConfigLoader;
use common::{ApiConfig, BaseServiceConfig, DEFAULT_API_HOST, DEFAULT_REDIS_URL, RedisConfig};
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use voltage_model::KeySpaceConfig;

// Import from error module directly (works in both lib and bin context)
use super::error::{ModSrvError, Result};

use crate::app_state::AppState;
use crate::instance_manager::InstanceManager;
use crate::product_loader::ProductLoader;

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
///
/// If `config.products_path` is set and the directory exists, external product
/// JSON files override built-in products. Otherwise, built-in products are used.
pub async fn load_products<R>(
    config: &ModsrvConfig,
    sqlite_pool: &SqlitePool,
    _rtdb: &Arc<R>,
) -> Result<Arc<ProductLoader>>
where
    R: voltage_rtdb::Rtdb,
{
    use std::sync::Arc as StdArc;
    use voltage_model::product_lib::ProductLibrary;

    // Load product library with optional external overrides
    let products_dir = config
        .products_path
        .as_ref()
        .map(std::path::Path::new)
        .filter(|p| p.is_dir());

    let library = ProductLibrary::load(products_dir).map_err(|e| {
        super::error::ModSrvError::ConfigError(format!("Failed to load product library: {}", e))
    })?;
    let product_count = library.len();

    let product_loader = ProductLoader::with_library(sqlite_pool.clone(), StdArc::new(library));

    // Initialize instance schema
    product_loader.init_schema().await?;

    // Ensure rules tables exist (normally created by `monarch init`,
    // but needed for standalone startup)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS rules (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            enabled BOOLEAN DEFAULT TRUE,
            priority INTEGER DEFAULT 0,
            cooldown_ms INTEGER DEFAULT 0,
            trigger_config TEXT,
            nodes_json TEXT NOT NULL,
            flow_json TEXT,
            format TEXT DEFAULT 'vue-flow',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(sqlite_pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS rule_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_id INTEGER NOT NULL,
            triggered_at TIMESTAMP NOT NULL,
            execution_result TEXT,
            error TEXT,
            FOREIGN KEY (rule_id) REFERENCES rules(id)
        )",
    )
    .execute(sqlite_pool)
    .await?;

    if products_dir.is_some() {
        info!(
            "{} products loaded (with external overrides)",
            product_count
        );
    } else {
        info!("{} built-in products available", product_count);
    }

    Ok(Arc::new(product_loader))
}

/// Setup instance manager and sync instances
#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
pub async fn setup_instance_manager(
    sqlite_pool: &SqlitePool,
    _rtdb: Arc<voltage_rtdb::RedisRtdb>,
    routing_cache: Arc<voltage_routing::RoutingCache>,
    product_loader: Arc<ProductLoader>,
    dispatch: Arc<dyn crate::infra::shm_dispatch::ActionDispatch>,
) -> Result<Arc<InstanceManager<voltage_rtdb::MemoryRtdb>>> {
    // Create MemoryRtdb for testing (ignore the injected RedisRtdb)
    let rtdb = Arc::new(voltage_rtdb::MemoryRtdb::new());

    // Create instance manager with RTDB, routing cache, and dispatch
    let instance_manager = Arc::new(InstanceManager::new(
        sqlite_pool.clone(),
        rtdb,
        routing_cache,
        product_loader,
        dispatch,
    ));

    Ok(instance_manager)
}

#[cfg(not(test))]
pub async fn setup_instance_manager(
    sqlite_pool: &SqlitePool,
    rtdb: Arc<voltage_rtdb::RedisRtdb>,
    routing_cache: Arc<voltage_routing::RoutingCache>,
    product_loader: Arc<ProductLoader>,
    dispatch: Arc<dyn crate::infra::shm_dispatch::ActionDispatch>,
) -> Result<Arc<InstanceManager<voltage_rtdb::RedisRtdb>>> {
    // RTDB is a pure storage abstraction
    // M2C routing is handled externally by voltage-routing library

    // Create instance manager with RTDB, routing cache, and dispatch
    let instance_manager = Arc::new(InstanceManager::new(
        sqlite_pool.clone(),
        rtdb,
        routing_cache,
        product_loader,
        dispatch,
    ));

    // Instances loaded by monarch (may be empty on first startup)
    let instance_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instances")
        .fetch_one(sqlite_pool)
        .await
        .unwrap_or(0);

    if instance_count == 0 {
        warn!("No instances in DB — run `monarch sync` to load instance config");
    } else {
        info!("{} instances loaded", instance_count);
    }

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
    routing_cache: &Arc<voltage_routing::RoutingCache>,
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

/// Create application state with all initialized components
pub async fn create_app_state(service_info: &ServiceInfo) -> Result<Arc<AppState>> {
    // Initialize environment
    init_environment(service_info)?;

    // Wait for comsrv to be healthy before opening SHM (comsrv must create the SHM file first)
    let comsrv_base = common::comsrv_url();
    let comsrv_health = format!("{comsrv_base}/health");
    if let Err(e) = common::dependency::wait_for_dependency(
        "comsrv",
        &comsrv_health,
        std::time::Duration::from_secs(30),
    )
    .await
    {
        warn!("comsrv health check failed: {e}. Continuing startup (SHM may be unavailable).");
    }

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

        let cache = Arc::new(voltage_routing::RoutingCache::from_maps(
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

    // Create ShmDispatch (initially unconfigured, configured later in main.rs)
    let shm_dispatch = Arc::new(crate::infra::shm_dispatch::ShmDispatch::new());
    let dispatch: Arc<dyn crate::infra::shm_dispatch::ActionDispatch> =
        Arc::clone(&shm_dispatch) as Arc<dyn crate::infra::shm_dispatch::ActionDispatch>;

    // Setup instance manager (routing handled externally by voltage-routing library)
    let instance_manager = setup_instance_manager(
        &sqlite_pool,
        rtdb,
        routing_cache.clone(),
        Arc::clone(&product_loader),
        dispatch,
    )
    .await?;

    // Create application state
    Ok(Arc::new(AppState::new(
        config,
        instance_manager,
        shm_dispatch,
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

    // Build index fields for batch write (into_iter() moves ownership, avoiding clone)
    let fields: Vec<(String, Bytes)> = instances
        .into_iter()
        .map(|(id, name)| (name, Bytes::from(id.to_string())))
        .collect();

    // Save count before moving fields
    let count = fields.len();

    // Write all mappings in one batch operation
    let keyspace = KeySpaceConfig::production_cached();
    rtdb.hash_mset(&keyspace.instance_name_index_key(), fields)
        .await
        .map_err(|e| {
            ModSrvError::InternalError(format!("Failed to rebuild instance name index: {}", e))
        })?;

    info!("Name index: {} instances", count);

    Ok(count)
}
