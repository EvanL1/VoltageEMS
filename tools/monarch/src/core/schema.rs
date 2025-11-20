//! Database schema initialization for each service

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::Path;
use tracing::info;
use voltage_config::{comsrv, modsrv, rulesrv};

use super::file_utils;

/// Initialize database schema for a specific service
///
/// @input service: &str - Service name ("comsrv", "modsrv", "rulesrv")
/// @input db_path: impl AsRef<Path> - Path to SQLite database file
/// @output Result<()> - Success or initialization error
/// @throws anyhow::Error - Unknown service or database connection failure
/// @side-effects Creates database file if not exists, creates all tables
/// @transaction Atomic schema creation
pub async fn init_service_schema(service: &str, db_path: impl AsRef<Path>) -> Result<()> {
    let db_path = db_path.as_ref();

    // Ensure data directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Connect to database (will create if not exists)
    // SQLite connection string with create mode
    let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await
        .with_context(|| format!("Failed to connect to {} database", service))?;

    // Set file permissions for Docker compatibility
    file_utils::set_database_permissions(db_path)?;

    match service {
        "comsrv" => init_comsrv_schema(&pool).await?,
        "modsrv" => init_modsrv_schema(&pool).await?,
        "rulesrv" => init_rulesrv_schema(&pool).await?,
        _ => return Err(anyhow::anyhow!("Unknown service: {}", service)),
    }

    info!("Initialized schema for {} database", service);
    Ok(())
}

/// Initialize comsrv database schema
///
/// @input pool: &SqlitePool - Database connection pool
/// @output Result<()> - Success or schema creation error
/// @throws sqlx::Error - Table creation or index creation failure
/// @side-effects Creates 10 tables and indexes
/// @tables service_config, channels, four point tables, *_mappings (deprecated), sync_metadata
pub async fn init_comsrv_schema(pool: &SqlitePool) -> Result<()> {
    // Service configuration table
    sqlx::query(comsrv::SERVICE_CONFIG_TABLE)
        .execute(pool)
        .await?;

    // Channels table
    sqlx::query(comsrv::CHANNELS_TABLE).execute(pool).await?;

    // Four separate point tables with embedded protocol mappings
    sqlx::query(comsrv::TELEMETRY_POINTS_TABLE)
        .execute(pool)
        .await?;
    sqlx::query(comsrv::SIGNAL_POINTS_TABLE)
        .execute(pool)
        .await?;
    sqlx::query(comsrv::CONTROL_POINTS_TABLE)
        .execute(pool)
        .await?;
    sqlx::query(comsrv::ADJUSTMENT_POINTS_TABLE)
        .execute(pool)
        .await?;

    // Sync metadata table
    sqlx::query(comsrv::SYNC_METADATA_TABLE)
        .execute(pool)
        .await?;

    // Create indexes for the four point tables
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_telemetry_points_channel ON telemetry_points(channel_id)",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_signal_points_channel ON signal_points(channel_id)",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_control_points_channel ON control_points(channel_id)",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_adjustment_points_channel ON adjustment_points(channel_id)",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Initialize modsrv database schema
///
/// @input pool: &SqlitePool - Database connection pool
/// @output Result<()> - Success or schema creation error
/// @throws sqlx::Error - Table creation or index creation failure
/// @side-effects Creates products, instances, point definitions, routing tables
/// @tables products, instances, measurement_points, action_points, property_templates, measurement_routing, action_routing
pub async fn init_modsrv_schema(pool: &SqlitePool) -> Result<()> {
    // Service configuration table
    sqlx::query(modsrv::SERVICE_CONFIG_TABLE)
        .execute(pool)
        .await?;

    // Products table
    sqlx::query(modsrv::PRODUCTS_TABLE).execute(pool).await?;

    // Measurement points table
    sqlx::query(modsrv::MEASUREMENT_POINTS_TABLE)
        .execute(pool)
        .await?;

    // Action points table
    sqlx::query(modsrv::ACTION_POINTS_TABLE)
        .execute(pool)
        .await?;

    // Property templates table
    sqlx::query(modsrv::PROPERTY_TEMPLATES_TABLE)
        .execute(pool)
        .await?;

    // Instances table
    sqlx::query(modsrv::INSTANCES_TABLE).execute(pool).await?;

    // Two separate routing tables (replaces instance_mappings)
    sqlx::query(modsrv::MEASUREMENT_ROUTING_TABLE)
        .execute(pool)
        .await?;
    sqlx::query(modsrv::ACTION_ROUTING_TABLE)
        .execute(pool)
        .await?;

    // Sync metadata table
    sqlx::query(modsrv::SYNC_METADATA_TABLE)
        .execute(pool)
        .await?;

    // Create indexes (using modsrv's actual table names)
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_measurement_points_product ON measurement_points(product_name)")
        .execute(pool)
        .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_action_points_product ON action_points(product_name)",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_measurement_routing_instance ON measurement_routing(instance_id)",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_action_routing_instance ON action_routing(instance_id)",
    )
    .execute(pool)
    .await?;

    // Create triggers for automatic routing cleanup on point deletion
    // When a telemetry point is deleted, remove corresponding measurement_routing records
    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS cleanup_routing_on_telemetry_delete
         AFTER DELETE ON telemetry_points
         FOR EACH ROW
         BEGIN
             DELETE FROM measurement_routing
             WHERE channel_id = OLD.channel_id
               AND channel_type = 'T'
               AND channel_point_id = OLD.point_id;
         END",
    )
    .execute(pool)
    .await?;

    // When a signal point is deleted, remove corresponding measurement_routing records
    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS cleanup_routing_on_signal_delete
         AFTER DELETE ON signal_points
         FOR EACH ROW
         BEGIN
             DELETE FROM measurement_routing
             WHERE channel_id = OLD.channel_id
               AND channel_type = 'S'
               AND channel_point_id = OLD.point_id;
         END",
    )
    .execute(pool)
    .await?;

    // When a control point is deleted, remove corresponding action_routing records
    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS cleanup_routing_on_control_delete
         AFTER DELETE ON control_points
         FOR EACH ROW
         BEGIN
             DELETE FROM action_routing
             WHERE channel_id = OLD.channel_id
               AND channel_type = 'C'
               AND channel_point_id = OLD.point_id;
         END",
    )
    .execute(pool)
    .await?;

    // When an adjustment point is deleted, remove corresponding action_routing records
    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS cleanup_routing_on_adjustment_delete
         AFTER DELETE ON adjustment_points
         FOR EACH ROW
         BEGIN
             DELETE FROM action_routing
             WHERE channel_id = OLD.channel_id
               AND channel_type = 'A'
               AND channel_point_id = OLD.point_id;
         END",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Initialize rulesrv database schema
///
/// @input pool: &SqlitePool - Database connection pool
/// @output Result<()> - Success or schema creation error
/// @throws sqlx::Error - Table creation failure
/// @side-effects Creates rule tables and history tracking
/// @tables service_config, rules, rule_history, sync_metadata
pub async fn init_rulesrv_schema(pool: &SqlitePool) -> Result<()> {
    // Service configuration table
    sqlx::query(rulesrv::SERVICE_CONFIG_TABLE)
        .execute(pool)
        .await?;

    // Rules table
    sqlx::query(rulesrv::RULES_TABLE).execute(pool).await?;

    // Rule history table
    sqlx::query(rulesrv::RULE_HISTORY_TABLE)
        .execute(pool)
        .await?;

    // Sync metadata table
    sqlx::query(rulesrv::SYNC_METADATA_TABLE)
        .execute(pool)
        .await?;

    // Create indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rules_enabled ON rules(enabled)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rule_history_rule ON rule_history(rule_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rule_history_time ON rule_history(triggered_at)")
        .execute(pool)
        .await?;

    Ok(())
}
