//! Utility functions for monarch CLI

use anyhow::Result;
use sqlx::SqlitePool;
use std::path::Path;
use tracing::debug;

/// Database status information
#[derive(Debug)]
#[allow(dead_code)] // Fields accessed via Debug trait for logging
pub struct DatabaseStatus {
    pub exists: bool,
    pub initialized: bool,
    pub last_sync: Option<String>,
    pub item_count: Option<usize>,
    pub schema_version: Option<String>,
}

/// Check database status
pub async fn check_database_status(db_path: &Path) -> Result<DatabaseStatus> {
    debug!("Checking database status: {:?}", db_path);

    // Check if database file exists
    if !db_path.exists() {
        return Ok(DatabaseStatus {
            exists: false,
            initialized: false,
            last_sync: None,
            item_count: None,
            schema_version: None,
        });
    }

    // Connect to database in read-only mode
    let connection_string = format!("sqlite://{}?mode=ro", db_path.display());
    let pool = SqlitePool::connect(&connection_string).await?;

    // Check if service_config table exists
    let table_exists: bool = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='service_config'",
    )
    .fetch_optional(&pool)
    .await?
    .unwrap_or(false);

    if !table_exists {
        return Ok(DatabaseStatus {
            exists: true,
            initialized: false,
            last_sync: None,
            item_count: None,
            schema_version: None,
        });
    }

    // Get last sync timestamp
    let last_sync: Option<String> =
        sqlx::query_scalar("SELECT value FROM service_config WHERE key = '_sync_timestamp'")
            .fetch_optional(&pool)
            .await?;

    // Get item count
    let item_count: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM service_config")
        .fetch_optional(&pool)
        .await?;

    // Get schema version if available
    let schema_version: Option<String> =
        sqlx::query_scalar("SELECT value FROM service_config WHERE key = '_schema_version'")
            .fetch_optional(&pool)
            .await?;

    Ok(DatabaseStatus {
        exists: true,
        initialized: true,
        last_sync,
        item_count: item_count.map(|c| c as usize),
        schema_version,
    })
}
