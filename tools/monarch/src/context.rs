//! Service Context Management
//!
//! Provides unified context management for all VoltageEMS services,
//! enabling direct library calls instead of HTTP API.

use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "lib-mode")]
use {
    common::redis::RedisClient,
    common::PointType,
    comsrv::core::channels::ChannelManager,
    modsrv::{InstanceManager, ProductLoader},
    sqlx::SqlitePool,
    voltage_rtdb::RedisRtdb,
};

/// Configuration for service initialization
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Base path for database files (e.g., "data" or "/opt/MonarchEdge/data")
    pub db_path: PathBuf,

    /// Base path for configuration files (e.g., "config" or "/opt/MonarchEdge/config")
    pub config_path: PathBuf,

    /// Redis URL (e.g., "redis://localhost:6379")
    pub redis_url: String,
}

impl ServiceConfig {
    /// Get unified database path (voltage.db)
    ///
    /// All services now use a single unified database for configuration
    pub fn unified_db_path(&self) -> PathBuf {
        self.db_path.join("voltage.db")
    }

    /// Auto-detect paths from environment or defaults
    pub fn auto_detect() -> Self {
        // Detect database path
        let db_path = if Path::new("/opt/MonarchEdge/data").exists() {
            PathBuf::from("/opt/MonarchEdge/data")
        } else {
            PathBuf::from("data")
        };

        // Detect config path
        let config_path = if Path::new("/opt/MonarchEdge/config").exists() {
            PathBuf::from("/opt/MonarchEdge/config")
        } else {
            PathBuf::from("config")
        };

        // Get Redis URL from environment or default
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());

        Self {
            db_path,
            config_path,
            redis_url,
        }
    }
}

/// Unified service context providing access to all service capabilities
pub struct ServiceContext {
    config: ServiceConfig,

    #[cfg(feature = "lib-mode")]
    comsrv: Option<ComsrvContext>,

    #[cfg(feature = "lib-mode")]
    modsrv: Option<ModsrvContext>,
    // rules have been merged into modsrv - rules functionality via ModsrvContext
}

impl ServiceContext {
    /// Create a new service context (all services uninitialized)
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            config,
            #[cfg(feature = "lib-mode")]
            comsrv: None,
            #[cfg(feature = "lib-mode")]
            modsrv: None,
        }
    }

    /// Initialize modsrv context (public API for lib-mode users)
    #[cfg(feature = "lib-mode")]
    pub async fn init_modsrv(&mut self) -> Result<()> {
        if self.modsrv.is_some() {
            return Ok(()); // Already initialized
        }

        let modsrv = ModsrvContext::new(&self.config).await?;
        self.modsrv = Some(modsrv);
        Ok(())
    }

    // init_rules() removed - rules merged into modsrv

    /// Get comsrv context
    #[cfg(feature = "lib-mode")]
    pub fn comsrv(&self) -> Result<&ComsrvContext> {
        self.comsrv
            .as_ref()
            .context("Comsrv not initialized. Call init_comsrv() first.")
    }

    /// Get modsrv context
    #[cfg(feature = "lib-mode")]
    pub fn modsrv(&self) -> Result<&ModsrvContext> {
        self.modsrv
            .as_ref()
            .context("Modsrv not initialized. Call init_modsrv() first.")
    }

    // rules() removed - rules merged into modsrv, use modsrv() instead
}

#[cfg(feature = "lib-mode")]
/// Comsrv service context
pub struct ComsrvContext {
    pub channel_manager: Arc<RwLock<ChannelManager<RedisRtdb>>>,
    pub sqlite_pool: SqlitePool,
    pub rtdb: Arc<RedisRtdb>,
}

#[cfg(feature = "lib-mode")]
impl ComsrvContext {
    #[allow(dead_code)] // Reserved for future channel offline support
    async fn new(config: &ServiceConfig) -> Result<Self> {
        // Initialize SQLite connection (unified database)
        let db_path = config.unified_db_path();
        let sqlite_pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
            .await
            .with_context(|| format!("Failed to connect to comsrv database at {:?}", db_path))?;

        // Initialize Redis connection via RedisClient
        let redis_client = Arc::new(
            RedisClient::new(&config.redis_url)
                .await
                .with_context(|| format!("Failed to connect to Redis at {}", config.redis_url))?,
        );

        // Create RedisRtdb from RedisClient (using concrete type for static dispatch)
        let rtdb: Arc<RedisRtdb> = Arc::new(RedisRtdb::from_client(redis_client.clone()));

        // Create empty routing cache (Monarch doesn't use routing)
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());

        // Create channel manager (no longer needs protocol factory)
        let channel_manager = Arc::new(RwLock::new(ChannelManager::with_sqlite_pool(
            rtdb.clone(),
            routing_cache,
            sqlite_pool.clone(),
        )));

        Ok(Self {
            channel_manager,
            sqlite_pool,
            rtdb,
        })
    }
}

#[cfg(feature = "lib-mode")]
/// Modsrv service context
pub struct ModsrvContext {
    pub instance_manager: Arc<InstanceManager<RedisRtdb>>,
    pub product_loader: Arc<ProductLoader>,
    pub sqlite_pool: SqlitePool,
    pub rtdb: Arc<RedisRtdb>,
}

#[cfg(feature = "lib-mode")]
impl ModsrvContext {
    async fn new(config: &ServiceConfig) -> Result<Self> {
        // Initialize SQLite connection (unified database)
        let db_path = config.unified_db_path();
        let sqlite_pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
            .await
            .with_context(|| format!("Failed to connect to modsrv database at {:?}", db_path))?;

        // Initialize Redis connection
        let redis_client = Arc::new(
            RedisClient::new(&config.redis_url)
                .await
                .with_context(|| format!("Failed to connect to Redis at {}", config.redis_url))?,
        );

        let rtdb = Arc::new(RedisRtdb::from_client(redis_client.clone()));

        // Create product loader (products are now loaded from code definitions)
        let product_loader = Arc::new(ProductLoader::new(sqlite_pool.clone()));

        // Load routing cache from SQLite (enables direct library calls)
        let (c2m_map, m2c_map) = load_routing_maps_from_sqlite(&sqlite_pool).await?;
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::from_maps(
            c2m_map,
            m2c_map,
            std::collections::HashMap::new(), // C2C routing not yet implemented
        ));

        // Create instance manager
        let instance_manager = Arc::new(InstanceManager::new(
            sqlite_pool.clone(),
            rtdb.clone(),
            routing_cache,
            product_loader.clone(),
        ));

        Ok(Self {
            instance_manager,
            product_loader,
            sqlite_pool,
            rtdb,
        })
    }
}

// RulesContext has been removed - rules functionality is now in modsrv
// Use ModsrvContext for rule operations (same sqlite_pool can be used)

/// Load routing maps from SQLite database
///
/// This function loads C2M (Channel to Model) and M2C (Model to Channel) routing maps
/// from the SQLite database, enabling monarch to perform routing operations directly
/// without requiring services to be running.
///
/// # Returns
/// * `Ok((c2m_map, m2c_map))` - HashMaps containing routing mappings
/// * `Err(anyhow::Error)` - Database query error
async fn load_routing_maps_from_sqlite(
    sqlite_pool: &SqlitePool,
) -> Result<(
    std::collections::HashMap<String, String>,
    std::collections::HashMap<String, String>,
)> {
    tracing::info!("Loading routing maps from SQLite for monarch");

    let mut c2m_map = std::collections::HashMap::new();
    let mut m2c_map = std::collections::HashMap::new();

    // Fetch all enabled measurement routing (C2M - uplink)
    let measurement_routing = sqlx::query_as::<_, (u32, String, u32, String, u32, u32)>(
        r#"
        SELECT
            instance_id, instance_name, channel_id, channel_type, channel_point_id,
            measurement_id
        FROM measurement_routing
        WHERE enabled = TRUE
        "#,
    )
    .fetch_all(sqlite_pool)
    .await?;

    for (instance_id, _instance_name, channel_id, channel_type, channel_point_id, measurement_id) in
        measurement_routing
    {
        // Parse channel type
        let point_type = PointType::from_str(&channel_type)
            .ok_or_else(|| anyhow::anyhow!("Invalid channel type: {}", channel_type))?;

        // Build routing keys directly (avoids triple allocation from to_string + c2m_route_key + to_string)
        // From: channel_id:type:point_id → To: instance_id:M:point_id
        let from_key = format!(
            "{}:{}:{}",
            channel_id,
            point_type.as_str(),
            channel_point_id
        );
        // Note: Target uses "M" (Measurement role), not a PointType enum
        let to_key = format!("{}:M:{}", instance_id, measurement_id);

        c2m_map.insert(from_key, to_key);
    }

    // Fetch all enabled action routing (M2C - downlink)
    let action_routing = sqlx::query_as::<_, (u32, String, u32, u32, String, u32)>(
        r#"
        SELECT
            instance_id, instance_name, action_id, channel_id, channel_type,
            channel_point_id
        FROM action_routing
        WHERE enabled = TRUE
        "#,
    )
    .fetch_all(sqlite_pool)
    .await?;

    for (instance_id, _instance_name, action_id, channel_id, channel_type, channel_point_id) in
        action_routing
    {
        // Parse channel type (C or A)
        let point_type = PointType::from_str(&channel_type)
            .ok_or_else(|| anyhow::anyhow!("Invalid channel type: {}", channel_type))?;

        // Build routing keys directly (avoids triple allocation)
        // From: instance_id:A:point_id → To: channel_id:type:point_id
        let from_key = format!("{}:A:{}", instance_id, action_id);
        let to_key = format!(
            "{}:{}:{}",
            channel_id,
            point_type.as_str(),
            channel_point_id
        );

        m2c_map.insert(from_key, to_key);
    }

    tracing::info!(
        "Loaded routing cache for monarch: {} C2M routes, {} M2C routes",
        c2m_map.len(),
        m2c_map.len()
    );

    Ok((c2m_map, m2c_map))
}
