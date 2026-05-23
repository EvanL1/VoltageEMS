//! Shared test scaffolding and utilities
//!
//! Provides reusable test fixtures, helper functions, and sample data builders

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable
#![allow(dead_code)] // Test scaffolding - not all helpers used in every test file

use anyhow::Result;
use common::redis::RedisClient;
use modsrv::config::ModsrvConfig;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// Test environment context containing all required resources
pub struct TestEnv {
    pub pool: SqlitePool,
    pub redis_client: Arc<RedisClient>,
    pub temp_dir: TempDir,
    pub config: ModsrvConfig,
}

impl TestEnv {
    /// Create a fully provisioned test environment
    ///
    /// Includes:
    /// - Temporary SQLite database (full schema)
    /// - Mock Redis client
    /// - Temporary configuration directory
    /// - Default configuration
    pub async fn create() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("test_voltage.db");

        // Create SQLite connection pool
        let pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path.display())).await?;

        // Initialize database schema
        init_test_schema(&pool).await?;

        // Create mock Redis client
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let redis_client = Arc::new(RedisClient::new(&redis_url).await?);

        // Create testing configuration
        let config = create_test_config()?;

        Ok(Self {
            pool,
            redis_client,
            temp_dir,
            config,
        })
    }

    /// Clean up the test environment
    pub async fn cleanup(self) -> Result<()> {
        // Clean up Redis test data
        // IMPORTANT: This only clears database 0 (default test database)
        // Production services should use different databases
        if let Err(e) = self.redis_client.flushdb().await {
            eprintln!("Warning: Failed to flush Redis test database: {}", e);
        }

        // Close database connections
        self.pool.close().await;

        // Temporary directory is cleaned up when Drop runs
        Ok(())
    }

    /// Borrow the database connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Borrow the Redis client
    pub fn redis(&self) -> &Arc<RedisClient> {
        &self.redis_client
    }

    /// Return the temporary directory path
    pub fn temp_dir(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }
}

/// Initialize the test database schema
async fn init_test_schema(pool: &SqlitePool) -> Result<()> {
    common::test_utils::schema::init_modsrv_schema(pool).await?;

    // Calculations table (test-specific feature)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS calculations (
            calculation_id INTEGER PRIMARY KEY AUTOINCREMENT,
            calculation_name TEXT NOT NULL UNIQUE,
            product_name TEXT NOT NULL,
            result_point_id INTEGER NOT NULL,
            expression TEXT NOT NULL,
            description TEXT,
            enabled BOOLEAN DEFAULT TRUE,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Create a test configuration
fn create_test_config() -> Result<ModsrvConfig> {
    use common::{ApiConfig, BaseServiceConfig, RedisConfig};

    let config = ModsrvConfig {
        service: BaseServiceConfig {
            name: "modsrv_test".to_string(),
            ..Default::default()
        },
        api: ApiConfig {
            host: "0.0.0.0".to_string(),
            port: 6001,
        },
        redis: RedisConfig {
            url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    Ok(config)
}

/// Test data builders
pub mod fixtures {
    use super::*;
    use serde_json::json;

    /// Create test instance properties
    pub fn create_test_instance_properties() -> HashMap<String, serde_json::Value> {
        let mut props = HashMap::new();
        props.insert("Max Capacity".to_string(), json!(500));
        props.insert("Min SOC".to_string(), json!(10));
        props.insert("Max SOC".to_string(), json!(95));
        props
    }
}

/// Test helper functions
pub mod helpers {
    use super::*;

    /// Verify that an instance exists
    pub async fn assert_instance_exists(pool: &SqlitePool, instance_id: u16) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_id = ?)",
        )
        .bind(instance_id as i64)
        .fetch_one(pool)
        .await?;

        Ok(exists)
    }

    /// Verify the existence of a Redis key
    pub async fn assert_redis_key_exists(redis: &RedisClient, key: &str) -> Result<bool> {
        // Attempt to fetch the key to confirm presence
        match redis.get::<String>(key).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Clean up test data
    pub async fn cleanup_test_data(pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM measurement_routing")
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM action_routing")
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM instances").execute(pool).await?;
        Ok(())
    }
}

/// M2C Routing test helpers
///
/// Provides setup and assertion functions for M2C (Model to Channel) routing tests.
pub mod routing {
    use super::*;
    use bytes::Bytes;
    use voltage_routing::RoutingCache;
    use voltage_rtdb::MemoryRtdb;
    use voltage_rtdb::Rtdb;

    /// Create test environment with M2C routing configuration
    ///
    /// # Arguments
    /// * `m2c_routes` - M2C routing mappings: [("23:A:1", "1001:A:1"), ...]
    /// * `instance_mappings` - Instance name to ID mappings: [("inverter_01", 23), ...]
    ///
    /// # Returns
    /// * `(Arc<MemoryRtdb>, Arc<RoutingCache>)` - RTDB and routing cache instances
    ///
    /// # Example
    /// ```no_run
    /// use common::routing::*;
    ///
    /// #[tokio::test]
    /// async fn test_m2c() {
    ///     let (rtdb, routing_cache) = setup_m2c_routing(
    ///         vec![("23:A:1", "1001:A:1")],
    ///         vec![("inverter_01", 23)],
    ///     ).await;
    ///     // Use rtdb and routing_cache in tests
    /// }
    /// ```
    pub async fn setup_m2c_routing(
        m2c_routes: Vec<(&str, &str)>,
        instance_mappings: Vec<(&str, u32)>,
    ) -> (Arc<MemoryRtdb>, Arc<RoutingCache>) {
        use voltage_rtdb::MemoryRtdb;

        let rtdb = Arc::new(MemoryRtdb::new());

        // Step 1: Setup instance name index (inst:name:index Hash)
        for (name, id) in instance_mappings {
            rtdb.hash_set("inst:name:index", name, Bytes::from(id.to_string()))
                .await
                .expect("Failed to set instance name mapping");
        }

        // Step 2: Configure M2C routing table
        let mut m2c_map = HashMap::new();
        for (source, target) in m2c_routes {
            m2c_map.insert(source.to_string(), target.to_string());
        }

        let routing_cache = Arc::new(RoutingCache::from_maps(
            HashMap::new(), // C2M routing (empty)
            m2c_map,        // M2C routing
            HashMap::new(), // C2C routing (empty)
        ));

        (rtdb, routing_cache)
    }

    /// Verify instance action value
    ///
    /// # Arguments
    /// * `rtdb` - RTDB instance
    /// * `instance_id` - Instance ID
    /// * `point_id` - Point ID
    /// * `expected_value` - Expected value
    ///
    /// # Example
    /// ```no_run
    /// use common::routing::*;
    ///
    /// #[tokio::test]
    /// async fn test_instance_action() {
    ///     let rtdb = create_test_rtdb();
    ///     // ... write data ...
    ///     assert_instance_action(&rtdb, 23, 1, 12.3).await;
    /// }
    /// ```
    pub async fn assert_instance_action<R: Rtdb>(
        rtdb: &Arc<R>,
        instance_id: u32,
        point_id: u32,
        expected_value: f64,
    ) {
        use voltage_rtdb::KeySpaceConfig;

        let config = KeySpaceConfig::production();
        let inst_key = config.instance_action_key(instance_id);

        let value = rtdb
            .hash_get(&inst_key, &point_id.to_string())
            .await
            .expect("Failed to read instance action")
            .expect("Instance action should exist");

        let actual_value: f64 = String::from_utf8(value.to_vec())
            .expect("Value should be valid UTF-8")
            .parse()
            .expect("Value should be valid f64");

        assert_eq!(
            actual_value, expected_value,
            "Instance {} action point {} value mismatch",
            instance_id, point_id
        );
    }
}
