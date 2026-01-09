//! Test database schema utilities
//!
//! Provides helper functions to initialize test databases with standard schemas.
//! This eliminates the need for duplicate CREATE TABLE statements across test files.
//!
//! # Usage
//!
//! ```rust,ignore
//! use common::test_utils::schema;
//! use sqlx::SqlitePool;
//!
//! #[tokio::test]
//! async fn test_something() {
//!     let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
//!     schema::init_comsrv_schema(&pool).await.unwrap();
//!
//!     // Now use the pool with standard comsrv tables
//! }
//! ```

use anyhow::Result;
use sqlx::SqlitePool;

// Re-export common table constants
pub use crate::{SERVICE_CONFIG_TABLE, SYNC_METADATA_TABLE};

// ============================================================================
// Comsrv Table DDL
// ============================================================================

/// Channels table DDL (matches comsrv::core::config::ChannelRecord)
pub const CHANNELS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS channels (
        channel_id INTEGER NOT NULL PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        protocol TEXT,
        enabled INTEGER NOT NULL DEFAULT 1,
        config TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    )
"#;

/// Telemetry points table DDL (matches comsrv::core::config::TelemetryPointRecord)
pub const TELEMETRY_POINTS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS telemetry_points (
        point_id INTEGER NOT NULL,
        channel_id INTEGER NOT NULL REFERENCES channels(channel_id),
        signal_name TEXT NOT NULL,
        scale REAL DEFAULT 1.0,
        offset REAL DEFAULT 0.0,
        unit TEXT,
        reverse INTEGER DEFAULT 0,
        data_type TEXT,
        description TEXT,
        protocol_mappings TEXT,
        PRIMARY KEY (channel_id, point_id)
    )
"#;

/// Signal points table DDL (matches comsrv::core::config::SignalPointRecord)
pub const SIGNAL_POINTS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS signal_points (
        point_id INTEGER NOT NULL,
        channel_id INTEGER NOT NULL REFERENCES channels(channel_id),
        signal_name TEXT NOT NULL,
        scale REAL DEFAULT 1.0,
        offset REAL DEFAULT 0.0,
        unit TEXT,
        reverse INTEGER DEFAULT 0,
        normal_state INTEGER DEFAULT 0,
        data_type TEXT,
        description TEXT,
        protocol_mappings TEXT,
        PRIMARY KEY (channel_id, point_id)
    )
"#;

/// Control points table DDL (matches comsrv::core::config::ControlPointRecord)
pub const CONTROL_POINTS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS control_points (
        point_id INTEGER NOT NULL,
        channel_id INTEGER NOT NULL REFERENCES channels(channel_id),
        signal_name TEXT NOT NULL,
        scale REAL DEFAULT 1.0,
        offset REAL DEFAULT 0.0,
        unit TEXT,
        reverse INTEGER DEFAULT 0,
        data_type TEXT,
        description TEXT,
        protocol_mappings TEXT,
        PRIMARY KEY (channel_id, point_id)
    )
"#;

/// Adjustment points table DDL (matches comsrv::core::config::AdjustmentPointRecord)
pub const ADJUSTMENT_POINTS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS adjustment_points (
        point_id INTEGER NOT NULL,
        channel_id INTEGER NOT NULL REFERENCES channels(channel_id),
        signal_name TEXT NOT NULL,
        scale REAL DEFAULT 1.0,
        offset REAL DEFAULT 0.0,
        unit TEXT,
        reverse INTEGER DEFAULT 0,
        data_type TEXT,
        description TEXT,
        protocol_mappings TEXT,
        PRIMARY KEY (channel_id, point_id)
    )
"#;

// ============================================================================
// Modsrv Table DDL (matches modsrv::config schemas)
// ============================================================================

// Note: Products table has been removed.
// Products are now compile-time built-in constants from voltage-model crate.
// Use built-in product names like "Battery", "PCS", "ESS", "Station", etc.

/// Instances table DDL (matches modsrv::config::InstanceRecord)
/// Note: No foreign key to products table - products are compile-time constants
pub const INSTANCES_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS instances (
        instance_id INTEGER NOT NULL PRIMARY KEY,
        instance_name TEXT NOT NULL UNIQUE,
        product_name TEXT NOT NULL,
        parent_id INTEGER,
        properties TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (parent_id) REFERENCES instances(instance_id) ON DELETE SET NULL
    )
"#;

/// Measurement routing table DDL (matches modsrv::config::MeasurementRoutingRecord)
pub const MEASUREMENT_ROUTING_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS measurement_routing (
        routing_id INTEGER PRIMARY KEY AUTOINCREMENT,
        instance_id INTEGER NOT NULL REFERENCES instances(instance_id) ON DELETE CASCADE,
        instance_name TEXT NOT NULL,
        channel_id INTEGER REFERENCES channels(channel_id) ON DELETE SET NULL,
        channel_type TEXT,
        channel_point_id INTEGER,
        measurement_id INTEGER NOT NULL,
        description TEXT,
        enabled INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        UNIQUE(instance_id, measurement_id),
        CHECK(channel_type IN ('T','S'))
    )
"#;

/// Action routing table DDL (matches modsrv::config::ActionRoutingRecord)
pub const ACTION_ROUTING_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS action_routing (
        routing_id INTEGER PRIMARY KEY AUTOINCREMENT,
        instance_id INTEGER NOT NULL REFERENCES instances(instance_id) ON DELETE CASCADE,
        instance_name TEXT NOT NULL,
        action_id INTEGER NOT NULL,
        channel_id INTEGER REFERENCES channels(channel_id) ON DELETE SET NULL,
        channel_type TEXT,
        channel_point_id INTEGER,
        description TEXT,
        enabled INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        UNIQUE(instance_id, action_id),
        CHECK(channel_type IN ('C','A'))
    )
"#;

// ============================================================================
// Rules Table DDL
// ============================================================================

/// Rule chains table DDL (Vue Flow format)
pub const RULE_CHAINS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS rules (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        enabled INTEGER DEFAULT 1,
        priority INTEGER DEFAULT 0,
        cooldown_ms INTEGER DEFAULT 0,
        nodes_json TEXT NOT NULL,
        flow_json TEXT,
        format TEXT DEFAULT 'vue-flow',
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT DEFAULT CURRENT_TIMESTAMP
    )
"#;

/// Rule history table DDL
pub const RULE_HISTORY_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS rule_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        rule_id INTEGER NOT NULL REFERENCES rules(id),
        triggered_at TEXT NOT NULL,
        execution_result TEXT,
        error TEXT
    )
"#;

// ============================================================================
// Schema Initialization Functions
// ============================================================================

/// Initialize comsrv standard schema for testing
///
/// Creates all comsrv-related tables.
/// This includes:
/// - service_config
/// - sync_metadata
/// - channels
/// - telemetry_points, signal_points, control_points, adjustment_points
pub async fn init_comsrv_schema(pool: &SqlitePool) -> Result<()> {
    // Service metadata tables
    sqlx::query(SERVICE_CONFIG_TABLE).execute(pool).await?;
    sqlx::query(SYNC_METADATA_TABLE).execute(pool).await?;

    // Core channel table
    sqlx::query(CHANNELS_TABLE).execute(pool).await?;

    // Point tables
    sqlx::query(TELEMETRY_POINTS_TABLE).execute(pool).await?;
    sqlx::query(SIGNAL_POINTS_TABLE).execute(pool).await?;
    sqlx::query(CONTROL_POINTS_TABLE).execute(pool).await?;
    sqlx::query(ADJUSTMENT_POINTS_TABLE).execute(pool).await?;

    Ok(())
}

/// Initialize modsrv standard schema for testing
///
/// Creates all modsrv-related tables.
/// This includes:
/// - service_config
/// - sync_metadata
/// - channels (required by routing table foreign keys)
/// - instances
/// - measurement_routing, action_routing
///
/// Note: Products are now compile-time built-in constants from voltage-model crate.
/// No products table is created. Use built-in product names like "Battery", "PCS", etc.
pub async fn init_modsrv_schema(pool: &SqlitePool) -> Result<()> {
    // Service metadata tables
    sqlx::query(SERVICE_CONFIG_TABLE).execute(pool).await?;
    sqlx::query(SYNC_METADATA_TABLE).execute(pool).await?;

    // Channels table (required by routing table foreign keys in unified database architecture)
    sqlx::query(CHANNELS_TABLE).execute(pool).await?;

    // Instance table (no longer references products table)
    sqlx::query(INSTANCES_TABLE).execute(pool).await?;

    // Routing tables
    sqlx::query(MEASUREMENT_ROUTING_TABLE).execute(pool).await?;
    sqlx::query(ACTION_ROUTING_TABLE).execute(pool).await?;

    Ok(())
}

/// Initialize rules standard schema for testing
///
/// Creates all rules-related tables.
/// This includes:
/// - service_config
/// - sync_metadata
/// - rules (Vue Flow rule chains)
/// - rule_history
pub async fn init_rules_schema(pool: &SqlitePool) -> Result<()> {
    // Service metadata tables
    sqlx::query(SERVICE_CONFIG_TABLE).execute(pool).await?;
    sqlx::query(SYNC_METADATA_TABLE).execute(pool).await?;

    // Rule chains table (Vue Flow format)
    sqlx::query(RULE_CHAINS_TABLE).execute(pool).await?;
    sqlx::query(RULE_HISTORY_TABLE).execute(pool).await?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_comsrv_schema() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_comsrv_schema(&pool).await.unwrap();

        // Verify tables exist by querying them
        let result: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table'")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Should have 7 tables: service_config, sync_metadata, channels, 4 point tables
        assert!(
            result.0 >= 7,
            "Expected at least 7 tables, found {}",
            result.0
        );
    }

    #[tokio::test]
    async fn test_init_modsrv_schema() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_modsrv_schema(&pool).await.unwrap();

        // Verify tables exist
        let result: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table'")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Should have 6 tables: service_config, sync_metadata, channels, instances,
        // measurement_routing, action_routing
        // Note: Products and point definition tables removed (products are compile-time constants)
        assert!(
            result.0 >= 6,
            "Expected at least 6 tables, found {}",
            result.0
        );
    }

    #[tokio::test]
    async fn test_init_rules_schema() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_rules_schema(&pool).await.unwrap();

        // Verify tables exist
        let result: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table'")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Should have 4 tables: service_config, sync_metadata, rules, rule_history
        assert!(
            result.0 >= 4,
            "Expected at least 4 tables, found {}",
            result.0
        );
    }
}
