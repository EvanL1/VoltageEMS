//! Application state and initialization logic
//!
//! Configuration Strategy:
//! - Rules are loaded from SQLite database (synced via Monarch tool)
//! - YAML files are source of truth for version control only
//! - No runtime YAML loading to maintain architectural consistency

use crate::rule_engine::{ExecutionResult, Rule, RuleConfig};
use common::service_bootstrap::ServiceInfo;
use common::sqlite::{ServiceConfigLoader, SqliteClient};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use voltage_config::{
    common::{ApiConfig, BaseServiceConfig, RedisConfig, DEFAULT_API_HOST, DEFAULT_REDIS_URL},
    error::{VoltageError, VoltageResult},
    rulesrv::{ExecutionConfig, RulesrvConfig, DEFAULT_PORT},
};

// Type alias for compatibility with existing code
pub type Config = RulesrvConfig;

/// Application state shared across all handlers
pub struct AppState {
    /// RTDB for all Redis operations (data + messaging)
    pub rtdb: Arc<dyn voltage_rtdb::Rtdb>,
    /// Routing cache for M2C (Model to Channel) routing
    pub routing_cache: Arc<voltage_config::RoutingCache>,
    pub config: Arc<Config>,
    pub sqlite_client: Option<Arc<SqliteClient>>,
    pub rules_cache: Arc<RwLock<Arc<Vec<Rule>>>>,
    pub rule_config: Arc<RwLock<Option<RuleConfig>>>,
    pub execution_history: Arc<RwLock<Vec<ExecutionResult>>>,
}

/// Create application state from service info
pub async fn create_app_state(_service_info: &ServiceInfo) -> VoltageResult<Arc<AppState>> {
    // Load configuration from SQLite database (required)
    let db_path = if let Ok(dir) = std::env::var("DATABASE_DIR") {
        format!("{}/voltage.db", dir)
    } else {
        std::env::var("VOLTAGE_DB_PATH").unwrap_or_else(|_| "data/voltage.db".to_string())
    };

    if !std::path::Path::new(&db_path).exists() {
        error!("Configuration database not found at: {}", db_path);
        error!("Please run: monarch init all && monarch sync all");
        return Err(VoltageError::DatabaseNotFound {
            path: db_path.clone(),
            service: "rulesrv".to_string(),
        });
    }

    debug!("Loading configuration from SQLite database: {}", db_path);
    let service_config = ServiceConfigLoader::new(&db_path, "rulesrv")
        .await?
        .load_config()
        .await?;

    // Convert to voltage-config structure
    let mut config = Config {
        service: BaseServiceConfig {
            name: service_config.service_name,
            ..Default::default()
        },
        api: ApiConfig {
            host: DEFAULT_API_HOST.to_string(),
            port: service_config.port,
            workers: None,
        },
        redis: RedisConfig {
            url: service_config.redis_url,
            ..Default::default()
        },
        execution: ExecutionConfig::default(),
    };

    // Apply configuration priority: DB > ENV > Default
    let is_port_default = config.api.port == 0 || config.api.port == DEFAULT_PORT;
    config.api.port = common::config_loader::get_config_value(
        Some(config.api.port),
        is_port_default,
        "SERVICE_PORT",
        DEFAULT_PORT,
    );

    // Perform runtime validation before starting services
    info!("Performing runtime validation...");
    let skip_full_check = std::env::var("SKIP_VALIDATION").is_ok();
    if !skip_full_check {
        // Basic runtime validation
        if config.api.port == 0 {
            error!("Invalid port configuration: 0");
            return Err(VoltageError::InvalidConfig {
                field: "api.port".to_string(),
                reason: "Port cannot be 0".to_string(),
            });
        }
        if config.redis.url.is_empty() {
            error!("Redis URL not configured");
            return Err(VoltageError::MissingConfig("redis.url".to_string()));
        }
        info!("Basic configuration validation passed");
    }
    info!("Runtime validation completed successfully");

    // Build Redis connection candidates and connect with retry
    let redis_url_from_db = if !config.redis.url.is_empty() && config.redis.url != DEFAULT_REDIS_URL
    {
        Some(config.redis.url.clone())
    } else {
        None
    };

    let redis_candidates =
        common::config_loader::build_redis_candidates(redis_url_from_db, DEFAULT_REDIS_URL);

    let (redis_url, redis_client) = common::config_loader::connect_redis_with_retry(
        redis_candidates,
        tokio::time::Duration::from_secs(5),
    )
    .await;

    // Update config with connected Redis URL
    config.redis.url = redis_url.clone();
    debug!("Connected to Redis at: {}", redis_url);

    // Create RTDB trait implementation from Redis client (all Redis operations go through this)
    let rtdb: Arc<dyn voltage_rtdb::Rtdb> =
        Arc::new(voltage_rtdb::RedisRtdb::from_client(Arc::new(redis_client)));
    debug!("Created RedisRtdb trait implementation for all Redis operations");

    // Try to connect to SQLite database for rule configurations
    // Note: This is separate from service config - it's for storing rules
    let sqlite_client = match std::env::var("RULESRV_RULES_DB_PATH")
        .or_else(|_| std::env::var("VOLTAGE_DB_PATH"))
    {
        Ok(db_path) => match SqliteClient::new(&db_path).await {
            Ok(client) => {
                debug!(
                    "Connected to SQLite database for rule configurations: {}",
                    db_path
                );
                Some(Arc::new(client))
            },
            Err(e) => {
                warn!(
                    "Failed to open SQLite database for rules ({}), continuing without DB: {}",
                    db_path, e
                );
                None
            },
        },
        Err(_) => {
            debug!("RULESRV_RULES_DB_PATH/VOLTAGE_DB_PATH not set, rule management DB disabled");
            None
        },
    };

    // Load routing cache from Redis
    info!("Loading routing cache from Redis");
    let routing_cache = load_routing_cache(&rtdb).await?;
    info!("Routing cache loaded successfully");

    // Initialize rule caches (rules loaded from SQLite via API)
    let rules_cache = Arc::new(RwLock::new(Arc::new(Vec::new())));
    let rule_config = Arc::new(RwLock::new(None));
    let execution_history = Arc::new(RwLock::new(Vec::new()));

    // Create application state
    Ok(Arc::new(AppState {
        rtdb,
        routing_cache,
        config: Arc::new(config),
        sqlite_client,
        rules_cache,
        rule_config,
        execution_history,
    }))
}

/// Load routing cache from Redis
async fn load_routing_cache(
    rtdb: &Arc<dyn voltage_rtdb::Rtdb>,
) -> VoltageResult<Arc<voltage_config::RoutingCache>> {
    use std::collections::HashMap;
    use voltage_config::common::RedisRoutingKeys;

    // Load C2M routing (Channel to Model)
    let c2m_bytes = rtdb
        .hash_get_all(RedisRoutingKeys::CHANNEL_TO_MODEL)
        .await
        .unwrap_or_default();
    let c2m_data: HashMap<String, String> = c2m_bytes
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
        .collect();

    // Load M2C routing (Model to Channel)
    let m2c_bytes = rtdb
        .hash_get_all(RedisRoutingKeys::MODEL_TO_CHANNEL)
        .await
        .unwrap_or_default();
    let m2c_data: HashMap<String, String> = m2c_bytes
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
        .collect();

    Ok(Arc::new(voltage_config::RoutingCache::from_maps(
        c2m_data,
        m2c_data,
        std::collections::HashMap::new(), // C2C routing (not used in rulesrv)
    )))
}
