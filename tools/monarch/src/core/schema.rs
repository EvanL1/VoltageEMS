//! Database schema initialization
//!
//! Provides unified database initialization for all VoltageEMS tables.
//! All tables are created in a single `voltage.db` file.

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::Path;
use tracing::info;

// Import DDL constants from services (lib mode)
use comsrv::core::config as comsrv_schema;
use modsrv::config as modsrv_schema;

use super::file_utils;

// ============================================================================
// Rules DDL (defined locally since rules are managed by monarch)
// ============================================================================

/// Rules table SQL
const RULE_CHAINS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS rules (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        enabled BOOLEAN DEFAULT TRUE,
        priority INTEGER DEFAULT 0,
        cooldown_ms INTEGER DEFAULT 0,
        nodes_json TEXT NOT NULL,
        flow_json TEXT,
        format TEXT DEFAULT 'vue-flow',
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
"#;

/// Rule history table SQL
const RULE_HISTORY_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS rule_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        rule_id INTEGER NOT NULL,
        triggered_at TIMESTAMP NOT NULL,
        execution_result TEXT,
        error TEXT,
        FOREIGN KEY (rule_id) REFERENCES rules(id)
    )
"#;

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
    sqlx::query(comsrv_schema::SERVICE_CONFIG_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv_schema::SYNC_METADATA_TABLE)
        .execute(&pool)
        .await?;

    // === Channel & Point tables (comsrv) ===
    sqlx::query(comsrv_schema::CHANNELS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv_schema::TELEMETRY_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv_schema::SIGNAL_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv_schema::CONTROL_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(comsrv_schema::ADJUSTMENT_POINTS_TABLE)
        .execute(&pool)
        .await?;

    // === Product & Instance tables (modsrv) ===
    sqlx::query(modsrv_schema::PRODUCTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv_schema::MEASUREMENT_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv_schema::ACTION_POINTS_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv_schema::PROPERTY_TEMPLATES_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv_schema::INSTANCES_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv_schema::MEASUREMENT_ROUTING_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(modsrv_schema::ACTION_ROUTING_TABLE)
        .execute(&pool)
        .await?;

    // === Rule tables (rules engine) ===
    sqlx::query(RULE_CHAINS_TABLE).execute(&pool).await?;
    sqlx::query(RULE_HISTORY_TABLE).execute(&pool).await?;

    // === Indexes ===
    create_indexes(&pool).await?;

    // === Triggers ===
    create_triggers(&pool).await?;

    info!("DB init: {}", db_path.display());
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
