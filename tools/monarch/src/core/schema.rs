//! Database schema initialization
//!
//! Provides unified database initialization for all VoltageEMS tables.
//! All tables are created in a single `voltage.db` file.

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::Path;
use tracing::info;
use voltage_config::{comsrv, modsrv, rulesrv};

use super::file_utils;

/// Initialize all database tables in voltage.db
///
/// Creates all tables, indexes, and triggers needed by VoltageEMS services.
/// This is a unified initialization that replaces the old per-service approach.
///
/// @input db_path: impl AsRef<Path> - Path to SQLite database file
/// @output Result<()> - Success or initialization error
/// @throws anyhow::Error - Database connection or schema creation failure
/// @side-effects Creates database file if not exists, creates all tables/indexes/triggers
pub async fn init_database(db_path: impl AsRef<Path>) -> Result<()> {
    let db_path = db_path.as_ref();

    // Ensure data directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Connect to database (will create if not exists)
    let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await
        .with_context(|| "Failed to connect to database")?;

    // Set file permissions for Docker compatibility
    file_utils::set_database_permissions(db_path)?;

    // === Shared tables ===
    sqlx::query(comsrv::SERVICE_CONFIG_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv::SYNC_METADATA_TABLE)
        .execute(&pool)
        .await?;

    // === Channel & Point tables (comsrv) ===
    sqlx::query(comsrv::CHANNELS_TABLE).execute(&pool).await?;
    sqlx::query(comsrv::TELEMETRY_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv::SIGNAL_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv::CONTROL_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv::ADJUSTMENT_POINTS_TABLE)
        .execute(&pool)
        .await?;

    // === Product & Instance tables (modsrv) ===
    sqlx::query(modsrv::PRODUCTS_TABLE).execute(&pool).await?;
    sqlx::query(modsrv::MEASUREMENT_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv::ACTION_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv::PROPERTY_TEMPLATES_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv::INSTANCES_TABLE).execute(&pool).await?;
    sqlx::query(modsrv::MEASUREMENT_ROUTING_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv::ACTION_ROUTING_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv::CALCULATIONS_TABLE)
        .execute(&pool)
        .await?;

    // === Rule tables (rulesrv) ===
    sqlx::query(rulesrv::RULE_CHAINS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(rulesrv::RULE_HISTORY_TABLE)
        .execute(&pool)
        .await?;

    // === Indexes ===
    create_indexes(&pool).await?;

    // === Triggers ===
    create_triggers(&pool).await?;

    info!("Database initialized: {}", db_path.display());
    Ok(())
}

/// Create all database indexes
async fn create_indexes(pool: &SqlitePool) -> Result<()> {
    // Point tables indexes
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

    // Product/instance indexes
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_measurement_points_product ON measurement_points(product_name)",
    )
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

    // Rule indexes
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

/// Create all database triggers for automatic cleanup
async fn create_triggers(pool: &SqlitePool) -> Result<()> {
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
