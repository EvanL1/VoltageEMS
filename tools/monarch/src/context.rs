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
    comsrv::core::combase::ChannelManager,
    modsrv::{InstanceManager, ProductLoader, RoutingLoader},
    rulesrv::rule_engine::Rule,
    sqlx::SqlitePool,
    voltage_rtdb::{RedisRtdb, Rtdb},
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
    /// Create configuration from paths (public API)
    #[allow(dead_code)]
    pub fn new(
        db_path: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
        redis_url: String,
    ) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
            config_path: config_path.as_ref().to_path_buf(),
            redis_url,
        }
    }

    /// Get unified database path (voltage.db)
    ///
    /// All services now use a single unified database for configuration
    pub fn unified_db_path(&self) -> PathBuf {
        self.db_path.join("voltage.db")
    }

    /// Get database path for a specific service (DEPRECATED)
    ///
    /// This method is kept for backward compatibility but all services
    /// should migrate to use `unified_db_path()` instead.
    #[deprecated(note = "Use unified_db_path() instead - all services now share voltage.db")]
    #[allow(dead_code)]
    pub fn service_db_path(&self, service: &str) -> PathBuf {
        self.db_path.join(format!("{}.db", service))
    }

    /// Get configuration path for a specific service
    pub fn service_config_path(&self, service: &str) -> PathBuf {
        self.config_path.join(service)
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

    #[cfg(feature = "lib-mode")]
    rulesrv: Option<RulesrvContext>,
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
            #[cfg(feature = "lib-mode")]
            rulesrv: None,
        }
    }

    /// Initialize comsrv context
    #[cfg(feature = "lib-mode")]
    #[allow(dead_code)]
    pub async fn init_comsrv(&mut self) -> Result<()> {
        if self.comsrv.is_some() {
            return Ok(()); // Already initialized
        }

        let comsrv = ComsrvContext::new(&self.config).await?;
        self.comsrv = Some(comsrv);
        Ok(())
    }

    /// Initialize modsrv context (public API for lib-mode users)
    #[cfg(feature = "lib-mode")]
    #[allow(dead_code)]
    pub async fn init_modsrv(&mut self) -> Result<()> {
        if self.modsrv.is_some() {
            return Ok(()); // Already initialized
        }

        let modsrv = ModsrvContext::new(&self.config).await?;
        self.modsrv = Some(modsrv);
        Ok(())
    }

    /// Initialize rulesrv context (public API for lib-mode users)
    #[cfg(feature = "lib-mode")]
    #[allow(dead_code)]
    pub async fn init_rulesrv(&mut self) -> Result<()> {
        if self.rulesrv.is_some() {
            return Ok(()); // Already initialized
        }

        let rulesrv = RulesrvContext::new(&self.config).await?;
        self.rulesrv = Some(rulesrv);
        Ok(())
    }

    /// Initialize all services
    #[cfg(feature = "lib-mode")]
    pub async fn init_all(&mut self) -> Result<()> {
        // Parallel initialization for faster startup
        let (comsrv_result, modsrv_result, rulesrv_result) = tokio::join!(
            ComsrvContext::new(&self.config),
            ModsrvContext::new(&self.config),
            RulesrvContext::new(&self.config),
        );

        self.comsrv = Some(comsrv_result?);
        self.modsrv = Some(modsrv_result?);
        self.rulesrv = Some(rulesrv_result?);

        Ok(())
    }

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

    /// Get rulesrv context
    #[cfg(feature = "lib-mode")]
    pub fn rulesrv(&self) -> Result<&RulesrvContext> {
        self.rulesrv
            .as_ref()
            .context("Rulesrv not initialized. Call init_rulesrv() first.")
    }

    /// Get configuration (public API)
    #[allow(dead_code)]
    pub fn config(&self) -> &ServiceConfig {
        &self.config
    }
}

#[cfg(feature = "lib-mode")]
/// Comsrv service context
pub struct ComsrvContext {
    pub channel_manager: Arc<RwLock<ChannelManager>>,
    pub sqlite_pool: SqlitePool,
    pub rtdb: Arc<dyn Rtdb>,
}

#[cfg(feature = "lib-mode")]
impl ComsrvContext {
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

        // Create RedisRtdb from RedisClient and cast to trait object
        let rtdb: Arc<dyn Rtdb> = Arc::new(RedisRtdb::from_client(redis_client.clone()));

        // Create empty routing cache (Monarch doesn't use routing)
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());

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
    /// Routing loader (used for routing operations)
    #[allow(dead_code)]
    pub routing_loader: Arc<RoutingLoader>,
    pub sqlite_pool: SqlitePool,
    /// RTDB (used by lib_api data methods)
    #[allow(dead_code)]
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

        // Get products directory from config
        let products_dir = config.service_config_path("modsrv").join("products");

        // Create product loader
        let product_loader = Arc::new(ProductLoader::new(products_dir, sqlite_pool.clone()));

        // Get instances directory from config
        let instances_dir = config.service_config_path("modsrv").join("instances");

        // Create routing loader
        let routing_loader = Arc::new(RoutingLoader::new(instances_dir, sqlite_pool.clone()));

        // Create empty routing cache (Monarch doesn't use routing)
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());

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
            routing_loader,
            sqlite_pool,
            rtdb,
        })
    }
}

#[cfg(feature = "lib-mode")]
/// Rulesrv service context
pub struct RulesrvContext {
    pub sqlite_pool: SqlitePool,
    /// RTDB (used for rule execution)
    #[allow(dead_code)]
    pub rtdb: Arc<dyn Rtdb>,
    /// Pre-loaded rules cache for performance
    #[allow(dead_code)]
    pub rules_cache: Arc<RwLock<Vec<Rule>>>,
}

#[cfg(feature = "lib-mode")]
impl RulesrvContext {
    async fn new(config: &ServiceConfig) -> Result<Self> {
        // Initialize SQLite connection (unified database)
        let db_path = config.unified_db_path();
        let sqlite_pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
            .await
            .with_context(|| format!("Failed to connect to rulesrv database at {:?}", db_path))?;

        // Initialize Redis connection
        let redis_client = Arc::new(
            RedisClient::new(&config.redis_url)
                .await
                .with_context(|| format!("Failed to connect to Redis at {}", config.redis_url))?,
        );

        let rtdb: Arc<dyn Rtdb> = Arc::new(RedisRtdb::from_client(redis_client.clone()));

        // Load rules from database
        let rules = Self::load_rules_from_db(&sqlite_pool)
            .await
            .context("Failed to load rules from database")?;

        Ok(Self {
            sqlite_pool,
            rtdb,
            rules_cache: Arc::new(RwLock::new(rules)),
        })
    }

    /// Load all rules from SQLite database
    async fn load_rules_from_db(pool: &SqlitePool) -> Result<Vec<Rule>> {
        let db_rules: Vec<(String, String, Option<String>)> =
            sqlx::query_as("SELECT id, name, flow_json FROM rules ORDER BY priority DESC, id")
                .fetch_all(pool)
                .await
                .context("Failed to query rules from database")?;

        let mut rules = Vec::new();
        for (id, name, flow_json_opt) in db_rules {
            let flow_json = flow_json_opt.unwrap_or_else(|| "{}".to_string());

            // Parse flow_json as complete Rule object
            let mut rule: Rule = serde_json::from_str(&flow_json)
                .with_context(|| format!("Failed to parse rule for rule '{}'", id))?;

            // Override id and name from database columns (they take precedence)
            rule.id = id;
            rule.name = name;

            rules.push(rule);
        }

        Ok(rules)
    }
}
