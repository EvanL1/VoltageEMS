//! Database schema initialization
//!
//! Provides unified database initialization for all VoltageEMS tables.
//! All tables are created in a single `voltage.db` file.

use anyhow::{Context, Result};
use sqlx::{Row, Sqlite, SqlitePool};
use std::path::Path;
use tracing::{info, warn};

// Import DDL constants from common (shared schema definitions)
use common::test_utils::schema::{
    ACTION_ROUTING_TABLE, ADJUSTMENT_POINTS_TABLE, CHANNEL_TEMPLATES_TABLE, CHANNELS_TABLE,
    CONTROL_POINTS_TABLE, INSTANCE_PROPERTIES_TABLE, INSTANCES_TABLE, MEASUREMENT_ROUTING_TABLE,
    SERVICE_CONFIG_TABLE, SIGNAL_POINTS_TABLE, SYNC_METADATA_TABLE, TELEMETRY_POINTS_TABLE,
};

use super::file_utils;

// ============================================================================
// JSON Point Mappings DDL (MQTT/HTTP protocol support)
// ============================================================================

/// JSON point mappings table DDL for MQTT/HTTP protocols
///
/// This table enables configuration-driven device integration:
/// - MQTT devices publish JSON payloads with vendor-specific formats
/// - HTTP devices return JSON responses with custom schemas
/// - JSONPath expressions extract values without code changes
const JSON_POINT_MAPPINGS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS json_point_mappings (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        channel_id INTEGER NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
        point_id INTEGER NOT NULL,
        point_type TEXT NOT NULL,
        json_path TEXT NOT NULL,
        data_type TEXT DEFAULT 'float',
        scale REAL DEFAULT 1.0,
        offset REAL DEFAULT 0.0,
        description TEXT,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
        UNIQUE(channel_id, point_id, point_type)
    )
"#;

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
        trigger_config TEXT,
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

// ============================================================================
// Schema Version Migration
// ============================================================================
//
// Uses SQLite's built-in PRAGMA user_version to track schema structure version.
// Each breaking schema change gets a new version with a migration function.
//
// To add a new migration:
//   1. Increment SCHEMA_VERSION
//   2. Add `migrate_vN()` function
//   3. Add `if current < N { migrate_vN(&mut conn).await?; }` in run_migrations()

/// Current schema structure version — increment when adding migrations
const SCHEMA_VERSION: i32 = 5;

/// Run pending schema migrations based on `PRAGMA user_version`
///
/// Reads the database's current version, executes any outstanding migrations
/// sequentially, then stamps the new version. All migration queries run on
/// a single connection to keep `PRAGMA foreign_keys` state consistent.
async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    let current: i32 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await?;

    if current >= SCHEMA_VERSION {
        return Ok(());
    }

    info!("Schema migration: v{current} -> v{SCHEMA_VERSION}",);

    // Acquire a single connection — PRAGMA foreign_keys is per-connection
    let mut conn = pool.acquire().await?;

    if current < 1 {
        migrate_v1(&mut conn).await.context("Migration v1 failed")?;
    }

    if current < 2 {
        migrate_v2(&mut conn).await.context("Migration v2 failed")?;
    }

    if current < 3 {
        migrate_v3(&mut conn).await.context("Migration v3 failed")?;
    }

    if current < 4 {
        migrate_v4(&mut conn).await.context("Migration v4 failed")?;
    }

    if current < 5 {
        migrate_v5(&mut conn).await.context("Migration v5 failed")?;
    }

    sqlx::query(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
        .execute(&mut *conn)
        .await?;

    info!("Schema migration complete: v{SCHEMA_VERSION}");
    Ok(())
}

/// v1: Remove `products` foreign key from `instances` table, drop obsolete tables
///
/// Old schema had `REFERENCES products(product_name)` on instances.product_name.
/// Products are now compile-time constants — the FK and table are no longer needed.
async fn migrate_v1(conn: &mut sqlx::pool::PoolConnection<Sqlite>) -> Result<()> {
    let has_products: bool =
        sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type='table' AND name='products'")
            .fetch_optional(&mut **conn)
            .await?
            .unwrap_or(false);

    if !has_products {
        info!("Migration v1: skipped (products table not found)");
        return Ok(());
    }

    info!("Migration v1: rebuilding instances table, removing products FK");

    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut **conn)
        .await?;

    // Rebuild instances without products FK (matches INSTANCES_TABLE DDL)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS instances_new (
            instance_id INTEGER NOT NULL PRIMARY KEY,
            instance_name TEXT NOT NULL UNIQUE,
            product_name TEXT NOT NULL,
            parent_id INTEGER,
            properties TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (parent_id) REFERENCES instances(instance_id) ON DELETE SET NULL
        )",
    )
    .execute(&mut **conn)
    .await?;

    // Copy data — old table has no parent_id, defaults to NULL
    sqlx::query(
        "INSERT INTO instances_new
            (instance_id, instance_name, product_name, properties, created_at, updated_at)
         SELECT instance_id, instance_name, product_name, properties, created_at, updated_at
         FROM instances",
    )
    .execute(&mut **conn)
    .await?;

    sqlx::query("DROP TABLE instances")
        .execute(&mut **conn)
        .await?;
    sqlx::query("ALTER TABLE instances_new RENAME TO instances")
        .execute(&mut **conn)
        .await?;

    // Drop obsolete product-related tables
    for table in [
        "products",
        "measurement_points",
        "action_points",
        "property_templates",
        "product_library_meta",
    ] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(&mut **conn)
            .await?;
    }

    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&mut **conn)
        .await?;

    info!("Migration v1: complete");
    Ok(())
}

/// v2: Rename legacy snake_case product names to match new built-in product library
///
/// Old config templates used `battery_pack`, `pcs`, etc. The compile-time product
/// library uses `Battery`, `PCS`, `PVInverter`, `Diesel`. This migration updates
/// existing rows so `get_builtin_product()` can find them by exact name match.
async fn migrate_v2(conn: &mut sqlx::pool::PoolConnection<Sqlite>) -> Result<()> {
    // On a fresh database, instances table doesn't exist yet (created after migrations).
    // Only run the rename if the table is already present from a previous schema version.
    let has_instances: bool =
        sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type='table' AND name='instances'")
            .fetch_optional(&mut **conn)
            .await?
            .unwrap_or(false);

    if !has_instances {
        info!("Migration v2: skipped (instances table not yet created)");
        return Ok(());
    }

    info!("Migration v2: renaming legacy product names");

    let mappings = [
        ("battery_pack", "Battery"),
        ("pcs", "PCS"),
        ("diesel_generator", "Diesel"),
        ("pv_inverter", "PVInverter"),
    ];

    for (old, new) in mappings {
        let result = sqlx::query("UPDATE instances SET product_name = ? WHERE product_name = ?")
            .bind(new)
            .bind(old)
            .execute(&mut **conn)
            .await?;

        if result.rows_affected() > 0 {
            info!(
                "Migration v2: renamed '{}' -> '{}' ({} rows)",
                old,
                new,
                result.rows_affected()
            );
        }
    }

    info!("Migration v2: complete");
    Ok(())
}

/// v3: Add `channel_templates` table for protocol point-table template management
///
/// Stores JSON snapshots of channel point definitions and protocol mappings,
/// enabling "save once → apply many" workflows for identically-configured devices.
async fn migrate_v3(conn: &mut sqlx::pool::PoolConnection<Sqlite>) -> Result<()> {
    info!("Migration v3: creating channel_templates table");

    sqlx::query(CHANNEL_TEMPLATES_TABLE)
        .execute(&mut **conn)
        .await?;

    info!("Migration v3: complete");
    Ok(())
}

/// v4: Add `trigger_config` column to `rules` table
///
/// The column was added to DDL definitions but never migrated for existing databases.
/// Without it, `repository.rs::hydrate_rule()` fails with "no such column: trigger_config",
/// causing the rule engine to load zero rules.
async fn migrate_v4(conn: &mut sqlx::pool::PoolConnection<Sqlite>) -> Result<()> {
    info!("Migration v4: adding trigger_config column to rules table");

    // If the rules table doesn't exist yet, skip — it will be created fresh
    // with trigger_config included when monarch init runs the full DDL.
    let table_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='rules'",
    )
    .fetch_one(&mut **conn)
    .await?;

    if !table_exists {
        info!("Migration v4: rules table not yet created, skipping ALTER TABLE");
        return Ok(());
    }

    // Check if column already exists (idempotent)
    let has_column: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('rules') WHERE name = 'trigger_config'",
    )
    .fetch_one(&mut **conn)
    .await?;

    if !has_column {
        sqlx::query("ALTER TABLE rules ADD COLUMN trigger_config TEXT")
            .execute(&mut **conn)
            .await?;
        info!("Migration v4: added trigger_config column");
    } else {
        info!("Migration v4: trigger_config column already exists (skipped)");
    }

    Ok(())
}

/// v5: Move per-instance property values out of `instances.properties` JSON
/// column into a dedicated `instance_properties` table, then drop the column.
///
/// Old shape: each instance row had a `properties TEXT` column holding a
/// `{name: value}` JSON map. That made single-property writes require a
/// read-modify-write of the whole map (last-write-wins on concurrent edits)
/// and left no schema-level constraint on which keys are valid.
///
/// New shape: one row per (instance_id, property_id) in `instance_properties`
/// — mirrors `measurement_routing` / `action_routing`. `property_id` is
/// resolved by looking up each JSON key against the instance's product's
/// PropertyTemplate (compile-time constants in the `voltage-model` crate).
/// Keys that don't match any template are dropped with a warning — they
/// were unreachable from the existing `/api/instances/{id}/points` response
/// anyway, since that endpoint only emits points declared by the product.
async fn migrate_v5(conn: &mut sqlx::pool::PoolConnection<Sqlite>) -> Result<()> {
    info!("Migration v5: instance properties JSON column -> instance_properties table");

    // 1) Create the new table (idempotent — `init_database` also creates it
    //    on fresh installs, but a partial v4→v5 upgrade hits this first).
    sqlx::query(INSTANCE_PROPERTIES_TABLE)
        .execute(&mut **conn)
        .await?;

    // 2) Bail early if `instances` doesn't exist yet (very fresh DB before
    //    any DDL ran). Nothing to migrate.
    let has_instances: bool =
        sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type='table' AND name='instances'")
            .fetch_optional(&mut **conn)
            .await?
            .unwrap_or(false);

    if !has_instances {
        info!("Migration v5: instances table missing, skipping data migration");
        return Ok(());
    }

    // 3) Check if the legacy `properties` column actually exists. Re-running
    //    migrate_v5 on a v5+ database (column already dropped) should no-op.
    let has_properties_col: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('instances') WHERE name = 'properties'",
    )
    .fetch_one(&mut **conn)
    .await?;

    if !has_properties_col {
        info!("Migration v5: properties column already dropped, nothing to migrate");
        return Ok(());
    }

    // 4) Read every (instance_id, product_name, properties_json) and translate
    //    each JSON entry to a row in instance_properties. We do this BEFORE
    //    dropping the column so failure mid-migration leaves data recoverable.
    let rows: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT instance_id, product_name, properties FROM instances")
            .fetch_all(&mut **conn)
            .await?;

    let mut migrated_values = 0_usize;
    let mut dropped_keys = 0_usize;

    for (instance_id, product_name, properties_json) in rows {
        let Some(json_str) = properties_json else {
            continue;
        };
        let trimmed = json_str.trim();
        if trimmed.is_empty() || trimmed == "{}" {
            continue;
        }

        let map: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(trimmed) {
            Ok(serde_json::Value::Object(m)) => m,
            Ok(_other) => {
                warn!(
                    "Migration v5: instance {} properties JSON is not an object, skipping",
                    instance_id
                );
                continue;
            },
            Err(e) => {
                warn!(
                    "Migration v5: instance {} properties JSON unparseable ({}), skipping",
                    instance_id, e
                );
                continue;
            },
        };

        let Some(product) = voltage_model::product_lib::get_builtin_product(&product_name) else {
            warn!(
                "Migration v5: instance {} references unknown product '{}', dropping {} properties",
                instance_id,
                product_name,
                map.len()
            );
            dropped_keys += map.len();
            continue;
        };

        for (name, value) in map {
            let Some(tpl) = product.properties.iter().find(|p| p.name == name) else {
                warn!(
                    "Migration v5: instance {} key '{}' not in product '{}' PropertyTemplate, dropping",
                    instance_id, name, product_name
                );
                dropped_keys += 1;
                continue;
            };
            let value_json =
                serde_json::to_string(&value).context("encode property value to JSON")?;
            sqlx::query(
                "INSERT OR REPLACE INTO instance_properties \
                 (instance_id, property_id, value_json) VALUES (?, ?, ?)",
            )
            .bind(instance_id)
            .bind(tpl.id as i64)
            .bind(value_json)
            .execute(&mut **conn)
            .await?;
            migrated_values += 1;
        }
    }

    info!(
        "Migration v5: migrated {} property values, dropped {} unrecognized keys",
        migrated_values, dropped_keys
    );

    // 5) Rebuild `instances` without the `properties` column. SQLite < 3.35
    //    cannot DROP COLUMN, and even on newer versions table rebuild keeps
    //    behaviour consistent across deployments.
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut **conn)
        .await?;

    sqlx::query(
        "CREATE TABLE instances_new (
            instance_id INTEGER NOT NULL PRIMARY KEY,
            instance_name TEXT NOT NULL UNIQUE,
            product_name TEXT NOT NULL,
            parent_id INTEGER,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (parent_id) REFERENCES instances(instance_id) ON DELETE SET NULL
        )",
    )
    .execute(&mut **conn)
    .await?;

    sqlx::query(
        "INSERT INTO instances_new \
            (instance_id, instance_name, product_name, parent_id, created_at, updated_at) \
         SELECT instance_id, instance_name, product_name, parent_id, created_at, updated_at \
         FROM instances",
    )
    .execute(&mut **conn)
    .await?;

    sqlx::query("DROP TABLE instances")
        .execute(&mut **conn)
        .await?;
    sqlx::query("ALTER TABLE instances_new RENAME TO instances")
        .execute(&mut **conn)
        .await?;

    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&mut **conn)
        .await?;

    info!("Migration v5: complete (properties column dropped)");
    Ok(())
}

// ============================================================================
// Legacy Ad-hoc Migrations (kept for backward compatibility)
// ============================================================================

/// Migrate legacy rules table if needed
///
/// Checks if the rules table uses the old schema (id TEXT) and rebuilds it
/// with the correct schema (id INTEGER). This handles upgrades from older versions.
async fn migrate_rules_table_if_needed(pool: &SqlitePool) -> Result<()> {
    // Check if rules table exists and get id column type
    let row = sqlx::query("SELECT type FROM pragma_table_info('rules') WHERE name = 'id'")
        .fetch_optional(pool)
        .await?;

    if let Some(row) = row {
        let col_type: String = row.try_get("type")?;
        // Optimization: eq_ignore_ascii_case avoids to_uppercase() allocation
        if col_type.eq_ignore_ascii_case("TEXT") {
            warn!("Detected legacy rules table (id TEXT), rebuilding with INTEGER schema...");

            // Drop old tables (will be recreated with correct schema)
            sqlx::query("DROP TABLE IF EXISTS rule_history")
                .execute(pool)
                .await?;
            sqlx::query("DROP TABLE IF EXISTS rules")
                .execute(pool)
                .await?;

            info!("Legacy rules table dropped, will be recreated with correct schema");
        }
    }

    Ok(())
}

/// Initialize all database tables in voltage.db
///
/// Creates all tables, indexes, and triggers needed by VoltageEMS services.
/// This is a unified initialization that replaces the old per-service approach.
///
/// @input db_path: `impl AsRef<Path>` - Path to SQLite database file
/// @output `Result<()>` - Success or initialization error
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

    // Run versioned schema migrations (PRAGMA user_version based)
    run_migrations(&pool).await?;

    // Run legacy ad-hoc migrations (detection-based, kept for backward compatibility)
    migrate_rules_table_if_needed(&pool).await?;

    // === Shared tables ===
    sqlx::query(SERVICE_CONFIG_TABLE).execute(&pool).await?;
    sqlx::query(SYNC_METADATA_TABLE).execute(&pool).await?;

    // === Channel & Point tables ===
    sqlx::query(CHANNELS_TABLE).execute(&pool).await?;
    sqlx::query(TELEMETRY_POINTS_TABLE).execute(&pool).await?;
    sqlx::query(SIGNAL_POINTS_TABLE).execute(&pool).await?;
    sqlx::query(CONTROL_POINTS_TABLE).execute(&pool).await?;
    sqlx::query(ADJUSTMENT_POINTS_TABLE).execute(&pool).await?;

    // === JSON point mappings table (MQTT/HTTP protocols) ===
    sqlx::query(JSON_POINT_MAPPINGS_TABLE)
        .execute(&pool)
        .await?;

    // === Channel templates table ===
    sqlx::query(CHANNEL_TEMPLATES_TABLE).execute(&pool).await?;

    // === Instance tables ===
    sqlx::query(INSTANCES_TABLE).execute(&pool).await?;
    sqlx::query(MEASUREMENT_ROUTING_TABLE)
        .execute(&pool)
        .await?;
    sqlx::query(ACTION_ROUTING_TABLE).execute(&pool).await?;
    sqlx::query(INSTANCE_PROPERTIES_TABLE)
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

    // JSON point mappings indexes (for MQTT/HTTP protocols)
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_json_point_mappings_channel ON json_point_mappings(channel_id)",
    )
    .execute(pool)
    .await?;

    // Instance routing indexes
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
