//! Database connection and setup utilities
//!
//! Provides common functions for setting up Redis and SQLite connections
//! with retry logic and validation

use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use voltage_config::error::{VoltageError, VoltageResult};

use crate::config_loader::{build_redis_candidates, connect_redis_with_retry};
use crate::redis::RedisClient;

/// Database connection configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// SQLite database path
    pub sqlite_path: String,
    /// Redis URL (optional)
    pub redis_url: Option<String>,
    /// Maximum SQLite connections
    pub sqlite_max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            sqlite_path: "data/service.db".to_string(),
            redis_url: None,
            sqlite_max_connections: 5,
            connection_timeout: 10,
        }
    }
}

/// Setup SQLite database connection pool
pub async fn setup_sqlite_pool(db_path: &str) -> VoltageResult<SqlitePool> {
    // Check if database file exists
    if !Path::new(db_path).exists() {
        error!("DB not found: {}", db_path);
        return Err(VoltageError::DatabaseNotFound {
            path: db_path.to_string(),
            service: "unknown".to_string(),
        });
    }

    info!("SQLite: {}", db_path);

    // Create connection pool with configuration
    let pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path))
        .await
        .map_err(|e| {
            VoltageError::Database(format!("Failed to connect to SQLite database: {}", e))
        })?;

    // Test the connection
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|e| VoltageError::Database(format!("Failed to test SQLite connection: {}", e)))?;

    debug!("SQLite pool ready");
    Ok(pool)
}

/// Setup SQLite with custom configuration
pub async fn setup_sqlite_with_config(config: &DatabaseConfig) -> VoltageResult<SqlitePool> {
    let pool_options = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(config.sqlite_max_connections)
        .acquire_timeout(std::time::Duration::from_secs(config.connection_timeout));

    // Check if database file exists
    if !Path::new(&config.sqlite_path).exists() {
        error!("DB not found: {}", config.sqlite_path);
        return Err(VoltageError::DatabaseNotFound {
            path: config.sqlite_path.clone(),
            service: "unknown".to_string(),
        });
    }

    info!("SQLite: {}", config.sqlite_path);

    let pool = pool_options
        .connect(&format!("sqlite:{}?mode=rwc", config.sqlite_path))
        .await
        .map_err(|e| VoltageError::Database(format!("Failed to connect to SQLite: {}", e)))?;

    Ok(pool)
}

/// Setup Redis connection with retry logic
pub async fn setup_redis_connection(
    redis_url: Option<String>,
) -> VoltageResult<(String, Arc<RedisClient>)> {
    setup_redis_with_timeout(redis_url, tokio::time::Duration::from_secs(5)).await
}

/// Setup Redis with custom timeout
pub async fn setup_redis_with_timeout(
    redis_url: Option<String>,
    timeout: tokio::time::Duration,
) -> VoltageResult<(String, Arc<RedisClient>)> {
    // Build connection candidates with priority
    let candidates = build_redis_candidates(redis_url, "redis://127.0.0.1:6379");

    info!("Redis: {} candidates", candidates.len());

    // Connect with retry logic
    let (url, client) = connect_redis_with_retry(candidates, timeout).await;

    info!("Redis connected: {}", url);
    Ok((url, Arc::new(client)))
}

/// Setup Redis with custom configuration (including dynamic connection pool)
///
/// This function allows fine-grained control over Redis connection pool settings,
/// particularly useful for dynamically adjusting pool size based on workload.
///
/// # Arguments
/// * `redis_url` - Optional Redis URL (falls back to environment or default)
/// * `redis_config` - Custom Redis configuration with pool settings
///
/// # Example
/// ```no_run
/// use common::bootstrap_database::setup_redis_with_config;
/// use common::redis::RedisConfig;
///
/// let channel_count = 50;
/// let max_connections = channel_count * 2 + 30; // Dynamic calculation
///
/// let mut redis_config = RedisConfig::from_url("redis://localhost:6379");
/// redis_config.max_connections = max_connections;
///
/// let (url, client) = setup_redis_with_config(None, redis_config).await?;
/// ```
pub async fn setup_redis_with_config(
    redis_url: Option<String>,
    redis_config: crate::redis::RedisPoolConfig,
) -> VoltageResult<(String, Arc<RedisClient>)> {
    // Build connection candidates with priority
    let candidates = build_redis_candidates(redis_url, "redis://127.0.0.1:6379");

    info!(
        "Redis: {} candidates (pool:{})",
        candidates.len(),
        redis_config.max_connections
    );

    // Connect with retry logic using custom config
    let timeout = tokio::time::Duration::from_secs(redis_config.connection_timeout);
    let (url, _) = connect_redis_with_retry(candidates.clone(), timeout).await;

    // Create client with custom configuration
    let mut final_config = redis_config;
    final_config.url = url.clone();
    let pool_size = final_config.max_connections;

    let client = RedisClient::with_config(final_config)
        .await
        .map_err(|e| VoltageError::Internal(format!("Failed to create Redis client: {}", e)))?;

    info!("Redis connected: {} (pool:{})", url, pool_size);
    Ok((url, Arc::new(client)))
}

/// Validate database exists and has required tables
pub async fn validate_sqlite_schema(
    pool: &SqlitePool,
    required_tables: &[&str],
) -> VoltageResult<()> {
    debug!("Validating schema");

    for table in required_tables {
        let query = format!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='{}'",
            table
        );

        let result: Option<(String,)> =
            sqlx::query_as(&query)
                .fetch_optional(pool)
                .await
                .map_err(|e| {
                    VoltageError::Database(format!("Failed to check table {}: {}", table, e))
                })?;

        if result.is_none() {
            error!("Missing table: {}", table);
            return Err(VoltageError::Configuration(format!(
                "Missing required table: {}. Please run: monarch init",
                table
            )));
        }

        debug!("Table ok: {}", table);
    }

    debug!("Schema valid");
    Ok(())
}

/// Check database file permissions
pub fn check_database_permissions(db_path: &str) -> VoltageResult<()> {
    let path = Path::new(db_path);

    // Check if file exists
    if !path.exists() {
        return Err(VoltageError::DatabaseNotFound {
            path: db_path.to_string(),
            service: "unknown".to_string(),
        });
    }

    // Check if we can read the file
    if !path.is_file() {
        return Err(VoltageError::Configuration(format!(
            "{} is not a file",
            db_path
        )));
    }

    // Check parent directory for write permissions (for WAL files)
    if let Some(parent) = path.parent() {
        let metadata = parent.metadata().map_err(|e| {
            VoltageError::Configuration(format!("Cannot access database directory: {}", e))
        })?;

        if metadata.permissions().readonly() {
            warn!("Read-only dir: {}", parent.display());
        }
    }

    Ok(())
}

/// Initialize database with retry logic
pub async fn initialize_database_with_retry(
    db_path: &str,
    max_retries: u32,
) -> VoltageResult<SqlitePool> {
    let mut last_error = None;

    for attempt in 1..=max_retries {
        debug!("DB retry {}/{}", attempt, max_retries);

        match setup_sqlite_pool(db_path).await {
            Ok(pool) => {
                debug!("DB connected");
                return Ok(pool);
            },
            Err(e) => {
                warn!("DB retry {} failed: {}", attempt, e);
                last_error = Some(e);

                if attempt < max_retries {
                    let delay = std::time::Duration::from_secs(attempt as u64);
                    tokio::time::sleep(delay).await;
                }
            },
        }
    }

    Err(last_error.unwrap_or_else(|| {
        VoltageError::Database("Failed to connect to database after all retries".to_string())
    }))
}

/// Test Redis connection and basic operations
pub async fn test_redis_connection(client: &RedisClient) -> VoltageResult<()> {
    debug!("Testing Redis");

    // Test PING
    let pong: String = client
        .ping()
        .await
        .map_err(|e| VoltageError::Communication(format!("Redis PING failed: {}", e)))?;

    if pong != "PONG" {
        return Err(VoltageError::Communication(format!(
            "Unexpected PING response: {}",
            pong
        )));
    }

    // Test SET/GET
    let test_key = "voltage:test:connection";
    let test_value = "ok";

    client
        .set(test_key, test_value)
        .await
        .map_err(|e| VoltageError::Communication(format!("Redis SET failed: {}", e)))?;

    let retrieved: Option<String> = client
        .get(test_key)
        .await
        .map_err(|e| VoltageError::Communication(format!("Redis GET failed: {}", e)))?;

    if retrieved != Some(test_value.to_string()) {
        return Err(VoltageError::Communication(
            "Redis GET returned unexpected value".to_string(),
        ));
    }

    // Clean up test key
    let _: u32 = client
        .del(&[test_key])
        .await
        .map_err(|e| VoltageError::Communication(format!("Redis DEL failed: {}", e)))?;

    debug!("Redis ok");
    Ok(())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_default_database_config() {
        let config = DatabaseConfig::default();
        assert_eq!(config.sqlite_path, "data/service.db");
        assert_eq!(config.sqlite_max_connections, 5);
        assert_eq!(config.connection_timeout, 10);
    }

    #[tokio::test]
    async fn test_check_database_permissions() {
        // Test with non-existent file
        let result = check_database_permissions("/non/existent/path.db");
        assert!(result.is_err());

        // Test with existing file (use temp file in real tests)
        // This would require creating a temp file for proper testing
    }
}
