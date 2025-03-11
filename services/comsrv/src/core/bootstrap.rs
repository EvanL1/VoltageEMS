//! Service Bootstrap and Initialization
//!
//! This module handles service initialization including:
//! - Logging configuration
//! - Configuration validation
//! - Redis connection setup
//!
//! Uses common bootstrap utilities for shared functionality

use clap::Parser;
use tracing::{debug, info};

use common::service_bootstrap::ServiceInfo;
use voltage_config::common::DEFAULT_API_HOST;
use voltage_config::comsrv::DEFAULT_PORT;
use voltage_config::error::{VoltageError, VoltageResult};

use crate::core::config::ConfigManager;

// Re-export common bootstrap functionality
pub use common::bootstrap_args::ServiceArgs;
pub use common::bootstrap_database::setup_redis_connection;
pub use common::bootstrap_system::check_system_requirements;

/// Command-line arguments for comsrv
#[derive(Parser, Clone)]
#[command(
    name = "comsrv",
    version = env!("CARGO_PKG_VERSION"),
    about = "Industrial Communication Service",
    long_about = None
)]
pub struct Args {
    /// Log level (trace, debug, info, warn, error)
    #[arg(short = 'l', long, default_value = "info")]
    pub log_level: String,

    /// Bind address for API server
    #[arg(short = 'b', long)]
    pub bind_address: Option<String>,

    /// Enable debug mode
    #[arg(short = 'd', long)]
    pub debug: bool,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    /// Validation mode - only validate configuration without starting service
    #[arg(long)]
    pub validate: bool,
}

impl From<Args> for ServiceArgs {
    fn from(args: Args) -> Self {
        ServiceArgs {
            log_level: args.log_level,
            bind_address: args.bind_address,
            debug: args.debug,
            no_color: args.no_color,
            validate: args.validate,
            watch: false,
            db_path: None,
            redis_url: None,
        }
    }
}

/// Initialize logging system with command-line arguments
/// Wraps common functionality with service-specific configuration
pub fn initialize_logging(args: &ServiceArgs, service_info: &ServiceInfo) -> VoltageResult<()> {
    // Load environment variables from .env file in development mode
    common::service_bootstrap::load_development_env();

    // Use common arg parsing
    let console_level = args.parse_log_level();

    // Check LOG_DIR environment variable for custom log directory
    // Format: /path/to/logs (without service subdirectory)
    // Defaults to "logs/{service_name}" if not set
    let log_dir = std::env::var("LOG_DIR")
        .ok()
        .map(|base| std::path::PathBuf::from(base).join(&service_info.name))
        .unwrap_or_else(|| std::path::PathBuf::from(format!("logs/{}", service_info.name)));

    let log_config = common::logging::LogConfig {
        service_name: service_info.name.clone(),
        log_dir,
        console_level,
        file_level: tracing::Level::DEBUG,
        enable_json: false,
        max_log_files: 365,
        enable_api_log: true,
        api_log_level: tracing::Level::INFO,
    };

    common::logging::init_with_config(log_config)
        .map_err(|e| VoltageError::Configuration(format!("Failed to init logging: {}", e)))?;
    Ok(())
}

/// Validate configuration from SQLite database
pub async fn validate_configuration() -> VoltageResult<()> {
    debug!("Validating configuration from SQLite database");

    // Load and validate configuration
    let config_manager = ConfigManager::load().await?;
    debug!("Configuration loaded successfully");

    // Validate service configuration
    let service_config = config_manager.service_config();
    info!("Service: {}", service_config.name);
    if let Some(desc) = &service_config.description {
        info!("Description: {}", desc);
    }

    // Validate channels
    let channels = config_manager.channels();
    info!("Found {} channel(s)", channels.len());

    for channel in channels {
        info!(
            "  Channel {}: {} (protocol: {})",
            channel.id(),
            channel.name(),
            channel.protocol()
        );

        // Note: Point counts will be loaded at runtime from SQLite
        info!("    Points will be loaded from SQLite at runtime");
    }

    info!("Configuration validation completed successfully");
    Ok(())
}

/// Determine bind address from multiple sources
/// Priority: CLI > Config > ENV > Default
pub fn determine_bind_address(
    cli_arg: Option<String>,
    config_host: &str,
    config_port: u16,
) -> String {
    if let Some(addr) = cli_arg {
        info!("Using bind address from command line: {}", addr);
        return addr;
    }

    // Check if configuration specifies port (non-default)
    let is_config_default = config_port == DEFAULT_PORT || config_port == 0;

    if !is_config_default {
        let config_addr = format!("{}:{}", config_host, config_port);
        info!("Using bind address from configuration: {}", config_addr);
        return config_addr;
    }

    // Try environment variables
    let port = common::config_loader::get_config_value(
        Some(config_port),
        is_config_default,
        "SERVICE_PORT",
        DEFAULT_PORT,
    );

    let host = common::config_loader::get_string_config(
        Some(config_host.to_string()),
        config_host.is_empty(),
        "SERVICE_HOST",
        DEFAULT_API_HOST.to_string(),
    );

    format!("{}:{}", host, port)
}

// ============================================================================
// Routing Configuration Loading (from route.db)
// ============================================================================

/// Load routing maps directly from SQLite route.db
///
/// This function reads routing configuration from the dedicated route.db
/// and builds C2M (Channel to Model), M2C (Model to Channel) and C2C (Channel to Channel) mappings.
///
/// # Arguments
/// * `sqlite_pool` - SQLite connection pool to route.db
///
/// # Returns
/// Tuple of (c2m_map, m2c_map, c2c_map) where keys are routing identifiers
///
/// # Example
/// ```ignore
/// let route_pool = SqlitePool::connect("sqlite:data/route.db?mode=ro").await?;
/// let (c2m, m2c, c2c) = load_routing_maps_from_sqlite(&route_pool).await?;
/// ```
pub async fn load_routing_maps_from_sqlite(
    sqlite_pool: &sqlx::SqlitePool,
) -> anyhow::Result<(
    std::collections::HashMap<String, String>,
    std::collections::HashMap<String, String>,
    std::collections::HashMap<String, String>,
)> {
    use voltage_config::KeySpaceConfig;

    info!("Loading routing maps from route.db");

    let keyspace = KeySpaceConfig::production();
    let mut c2m_map = std::collections::HashMap::new();
    let mut m2c_map = std::collections::HashMap::new();
    let mut c2c_map = std::collections::HashMap::new();

    // Fetch all enabled measurement routing (C2M - uplink)
    let measurement_routing = sqlx::query_as::<_, (u16, String, i32, String, u32, u32)>(
        r#"
        SELECT instance_id, instance_name, channel_id, channel_type, channel_point_id,
               measurement_id
        FROM measurement_routing
        WHERE enabled = TRUE
        "#,
    )
    .fetch_all(sqlite_pool)
    .await?;

    for (instance_id, _, channel_id, channel_type, channel_point_id, measurement_id) in
        measurement_routing
    {
        // Parse channel type
        let point_type = voltage_config::protocols::PointType::from_str(&channel_type)
            .ok_or_else(|| anyhow::anyhow!("Invalid channel type: {}", channel_type))?;

        // Build routing keys (no prefix for hash fields)
        // From: channel_id:type:point_id → To: instance_id:M:point_id
        let from_key =
            keyspace.c2m_route_key(channel_id as u16, point_type, &channel_point_id.to_string());
        // Note: Target uses "M" (Measurement role), not a PointType enum
        let to_key = format!("{}:M:{}", instance_id, measurement_id);

        c2m_map.insert(from_key.to_string(), to_key);
    }

    // Fetch all enabled action routing (M2C - downlink)
    let action_routing = sqlx::query_as::<_, (u16, String, u32, i32, String, u32)>(
        r#"
        SELECT instance_id, instance_name, action_id, channel_id, channel_type,
               channel_point_id
        FROM action_routing
        WHERE enabled = TRUE
        "#,
    )
    .fetch_all(sqlite_pool)
    .await?;

    for (instance_id, _, action_id, channel_id, channel_type, channel_point_id) in action_routing {
        // Parse channel type (A or C)
        let point_type = voltage_config::protocols::PointType::from_str(&channel_type)
            .ok_or_else(|| anyhow::anyhow!("Invalid channel type: {}", channel_type))?;

        // Build routing keys (no prefix for hash fields)
        // From: instance_id:A:point_id → To: channel_id:type:point_id
        // Note: Source uses "A" (Action role), not a PointType enum
        let from_key = format!("{}:A:{}", instance_id, action_id);
        let to_key =
            keyspace.c2m_route_key(channel_id as u16, point_type, &channel_point_id.to_string());

        m2c_map.insert(from_key, to_key.to_string());
    }

    // Fetch all enabled C2C routing (Channel to Channel)
    let c2c_routing = sqlx::query_as::<_, (u16, String, u32, u16, String, u32, f64, f64)>(
        r#"
        SELECT source_channel_id, source_type, source_point_id,
               target_channel_id, target_type, target_point_id,
               scale, offset
        FROM channel_routing
        WHERE enabled = TRUE
        "#,
    )
    .fetch_all(sqlite_pool)
    .await
    .unwrap_or_else(|e| {
        // Table might not exist yet in older databases
        debug!(
            "channel_routing table not found, C2C routing disabled: {}",
            e
        );
        vec![]
    });

    for (
        source_channel_id,
        source_type,
        source_point_id,
        target_channel_id,
        target_type,
        target_point_id,
        _scale,
        _offset,
    ) in c2c_routing
    {
        // Parse source and target types
        let source_point_type = voltage_config::protocols::PointType::from_str(&source_type)
            .ok_or_else(|| anyhow::anyhow!("Invalid source type: {}", source_type))?;
        let target_point_type = voltage_config::protocols::PointType::from_str(&target_type)
            .ok_or_else(|| anyhow::anyhow!("Invalid target type: {}", target_type))?;

        // Build C2C routing key
        // From: source_channel_id:type:point_id → To: target_channel_id:type:point_id
        let from_key = keyspace.c2m_route_key(
            source_channel_id,
            source_point_type,
            &source_point_id.to_string(),
        );
        let to_key = keyspace.c2m_route_key(
            target_channel_id,
            target_point_type,
            &target_point_id.to_string(),
        );

        c2c_map.insert(from_key.to_string(), to_key.to_string());
    }

    info!(
        "Loaded routing maps from route.db: {} C2M routes, {} M2C routes, {} C2C routes",
        c2m_map.len(),
        m2c_map.len(),
        c2c_map.len()
    );

    Ok((c2m_map, m2c_map, c2c_map))
}
