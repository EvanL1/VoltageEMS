//! Test database schema utilities
//!
//! Provides helper functions to initialize test databases with standard schemas
//! defined in voltage-config. This eliminates the need for duplicate CREATE TABLE
//! statements across test files.
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
use voltage_config::comsrv;
use voltage_config::modsrv;
use voltage_config::rules;

/// Initialize comsrv standard schema for testing
///
/// Creates all comsrv-related tables using schema definitions from voltage-config.
/// This includes:
/// - service_config
/// - sync_metadata
/// - channels
/// - telemetry_points, signal_points, control_points, adjustment_points
pub async fn init_comsrv_schema(pool: &SqlitePool) -> Result<()> {
    // Service metadata tables
    sqlx::query(comsrv::SERVICE_CONFIG_TABLE)
        .execute(pool)
        .await?;

    sqlx::query(comsrv::SYNC_METADATA_TABLE)
        .execute(pool)
        .await?;

    // Core channel table
    sqlx::query(comsrv::CHANNELS_TABLE).execute(pool).await?;

    // Point tables
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

    Ok(())
}

/// Initialize modsrv standard schema for testing
///
/// Creates all modsrv-related tables using schema definitions from voltage-config.
/// This includes:
/// - service_config
/// - sync_metadata
/// - channels (required by routing table foreign keys)
/// - products
/// - instances
/// - measurement_points, action_points, property_templates
/// - measurement_routing, action_routing
pub async fn init_modsrv_schema(pool: &SqlitePool) -> Result<()> {
    // Service metadata tables
    sqlx::query(modsrv::SERVICE_CONFIG_TABLE)
        .execute(pool)
        .await?;

    sqlx::query(modsrv::SYNC_METADATA_TABLE)
        .execute(pool)
        .await?;

    // Channels table (required by routing table foreign keys in unified database architecture)
    sqlx::query(comsrv::CHANNELS_TABLE).execute(pool).await?;

    // Product and instance tables
    sqlx::query(modsrv::PRODUCTS_TABLE).execute(pool).await?;

    sqlx::query(modsrv::INSTANCES_TABLE).execute(pool).await?;

    // Point definition tables
    sqlx::query(modsrv::MEASUREMENT_POINTS_TABLE)
        .execute(pool)
        .await?;

    sqlx::query(modsrv::ACTION_POINTS_TABLE)
        .execute(pool)
        .await?;

    sqlx::query(modsrv::PROPERTY_TEMPLATES_TABLE)
        .execute(pool)
        .await?;

    // Routing tables
    sqlx::query(modsrv::MEASUREMENT_ROUTING_TABLE)
        .execute(pool)
        .await?;

    sqlx::query(modsrv::ACTION_ROUTING_TABLE)
        .execute(pool)
        .await?;

    Ok(())
}

/// Initialize rules standard schema for testing
///
/// Creates all rules-related tables using schema definitions from voltage-config.
/// This includes:
/// - service_config
/// - sync_metadata
/// - rules (Vue Flow rule chains)
/// - rule_history
pub async fn init_rules_schema(pool: &SqlitePool) -> Result<()> {
    // Service metadata tables
    sqlx::query(rules::SERVICE_CONFIG_TABLE)
        .execute(pool)
        .await?;

    sqlx::query(rules::SYNC_METADATA_TABLE)
        .execute(pool)
        .await?;

    // Rule chains table (Vue Flow format)
    sqlx::query(rules::RULE_CHAINS_TABLE).execute(pool).await?;

    sqlx::query(rules::RULE_HISTORY_TABLE).execute(pool).await?;

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

        // Should have 10 tables: service_config, sync_metadata, channels, products, instances,
        // 3 point tables, 2 routing tables
        assert!(
            result.0 >= 10,
            "Expected at least 10 tables, found {}",
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
