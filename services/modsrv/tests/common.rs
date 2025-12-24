//! Shared test scaffolding and utilities
//!
//! Provides reusable test fixtures, helper functions, and sample data builders

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable

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
    #[allow(dead_code)]
    pub redis_client: Arc<RedisClient>,
    #[allow(dead_code)]
    pub temp_dir: TempDir,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn redis(&self) -> &Arc<RedisClient> {
        &self.redis_client
    }

    /// Return the temporary directory path
    #[allow(dead_code)]
    pub fn temp_dir(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }
}

/// Initialize the test database schema
async fn init_test_schema(pool: &SqlitePool) -> Result<()> {
    common::test_utils::schema::init_modsrv_schema(pool).await?;

    // Test-specific tables (not in standard schema)
    // Instance point routings table (deprecated, may be removed in future)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS instance_point_routings (
            routing_id INTEGER PRIMARY KEY AUTOINCREMENT,
            instance_id INTEGER NOT NULL,
            point_type TEXT NOT NULL CHECK(point_type IN ('M', 'V', 'A')),
            point_id INTEGER NOT NULL,
            redis_key TEXT NOT NULL,
            FOREIGN KEY (instance_id) REFERENCES instances(instance_id),
            UNIQUE(instance_id, point_type, point_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

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
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (product_name) REFERENCES products(product_name)
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

    /// Create a test product record
    #[allow(dead_code)]
    pub async fn create_test_product(pool: &SqlitePool, product_name: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO products (product_name, parent_name)
            VALUES (?, NULL)
            "#,
        )
        .bind(product_name)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Create test product points
    #[allow(dead_code)]
    pub async fn create_test_product_points(pool: &SqlitePool, product_name: &str) -> Result<()> {
        // Measurement points (M)
        for i in 1..=3 {
            sqlx::query(
                r#"
                INSERT INTO measurement_points (product_name, measurement_id, name, unit)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(product_name)
            .bind(i)
            .bind(format!("M{}", i))
            .bind("kW")
            .execute(pool)
            .await?;
        }

        // Action points (A)
        for i in 1..=2 {
            sqlx::query(
                r#"
                INSERT INTO action_points (product_name, action_id, name, unit)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(product_name)
            .bind(i)
            .bind(format!("A{}", i))
            .bind("kW")
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Create test instance properties
    #[allow(dead_code)]
    pub fn create_test_instance_properties() -> HashMap<String, serde_json::Value> {
        let mut props = HashMap::new();
        props.insert("capacity".to_string(), json!(100));
        props.insert("location".to_string(), json!("test_location"));
        props
    }

    /// Create a complete test product with measurements, actions, and properties
    #[allow(dead_code)]
    pub async fn create_complete_test_product(
        pool: &SqlitePool,
        product_name: &str,
        parent_name: Option<&str>,
        num_measurements: u32,
        num_actions: u32,
        num_properties: i32,
    ) -> Result<()> {
        // Insert product
        sqlx::query(
            r#"
            INSERT INTO products (product_name, parent_name)
            VALUES (?, ?)
            "#,
        )
        .bind(product_name)
        .bind(parent_name)
        .execute(pool)
        .await?;

        // Insert measurements
        for i in 1..=num_measurements {
            sqlx::query(
                r#"
                INSERT INTO measurement_points (product_name, measurement_id, name, unit, description)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(product_name)
            .bind(i as i32)
            .bind(format!("Measurement_{}", i))
            .bind("kW")
            .bind(format!("Test measurement {}", i))
            .execute(pool)
            .await?;
        }

        // Insert actions
        for i in 1..=num_actions {
            sqlx::query(
                r#"
                INSERT INTO action_points (product_name, action_id, name, unit, description)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(product_name)
            .bind(i as i32)
            .bind(format!("Action_{}", i))
            .bind(None::<String>)
            .bind(format!("Test action {}", i))
            .execute(pool)
            .await?;
        }

        // Insert properties
        for i in 1..=num_properties {
            sqlx::query(
                r#"
                INSERT INTO property_templates (product_name, property_id, name, unit, description)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(product_name)
            .bind(i)
            .bind(format!("Property_{}", i))
            .bind("unit")
            .bind(format!("Test property {}", i))
            .execute(pool)
            .await?;
        }

        Ok(())
    }
}

/// Test helper functions
pub mod helpers {
    use super::*;

    /// Verify that an instance exists
    #[allow(dead_code)]
    pub async fn assert_instance_exists(pool: &SqlitePool, instance_id: u16) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_id = ?)",
        )
        .bind(instance_id as i32)
        .fetch_one(pool)
        .await?;

        Ok(exists)
    }

    /// Verify the existence of a Redis key
    #[allow(dead_code)]
    pub async fn assert_redis_key_exists(redis: &RedisClient, key: &str) -> Result<bool> {
        // Attempt to fetch the key to confirm presence
        match redis.get::<String>(key).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Clean up test data
    #[allow(dead_code)]
    pub async fn cleanup_test_data(pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM instance_point_routings")
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM instances").execute(pool).await?;
        sqlx::query("DELETE FROM measurement_points")
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM action_points")
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM property_templates")
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM products").execute(pool).await?;
        sqlx::query("DELETE FROM measurement_routing")
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM action_routing")
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// M2C Routing test helpers
///
/// Provides setup and assertion functions for M2C (Model to Channel) routing tests.
pub mod routing {
    use super::*;
    use bytes::Bytes;
    use voltage_rtdb::MemoryRtdb;
    use voltage_rtdb::RoutingCache;
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

    /// Verify TODO queue has trigger messages
    ///
    /// # Arguments
    /// * `rtdb` - RTDB instance
    /// * `queue_key` - TODO queue key (e.g., "comsrv:1001:A:TODO")
    ///
    /// # Example
    /// ```no_run
    /// use common::routing::*;
    ///
    /// #[tokio::test]
    /// async fn test_todo_queue() {
    ///     let rtdb = create_test_rtdb();
    ///     // ... trigger action ...
    ///     assert_todo_queue_triggered(&rtdb, "comsrv:1001:A:TODO").await;
    /// }
    /// ```
    #[allow(dead_code)]
    pub async fn assert_todo_queue_triggered<R: Rtdb>(rtdb: &Arc<R>, queue_key: &str) {
        let messages = rtdb
            .list_range(queue_key, 0, -1)
            .await
            .expect("Failed to read TODO queue");

        assert!(
            !messages.is_empty(),
            "TODO queue '{}' should have messages",
            queue_key
        );
    }

    /// Verify TODO queue is empty
    ///
    /// # Arguments
    /// * `rtdb` - RTDB instance
    /// * `queue_key` - TODO queue key (e.g., "comsrv:1001:A:TODO")
    ///
    /// # Example
    /// ```no_run
    /// use common::routing::*;
    ///
    /// #[tokio::test]
    /// async fn test_no_routing() {
    ///     let rtdb = create_test_rtdb();
    ///     // ... operation that should NOT trigger ...
    ///     assert_todo_queue_empty(&rtdb, "comsrv:1001:A:TODO").await;
    /// }
    /// ```
    #[allow(dead_code)]
    pub async fn assert_todo_queue_empty<R: Rtdb>(rtdb: &Arc<R>, queue_key: &str) {
        let messages = rtdb
            .list_range(queue_key, 0, -1)
            .await
            .expect("Failed to read TODO queue");

        assert!(
            messages.is_empty(),
            "TODO queue '{}' should be empty, but has {} messages",
            queue_key,
            messages.len()
        );
    }

    /// Parse TODO queue trigger message
    ///
    /// # Arguments
    /// * `rtdb` - RTDB instance
    /// * `queue_key` - TODO queue key (e.g., "comsrv:1001:A:TODO")
    ///
    /// # Returns
    /// * Parsed JSON value from the first message in the queue
    ///
    /// # Example
    /// ```no_run
    /// use common::routing::*;
    ///
    /// #[tokio::test]
    /// async fn test_message_format() {
    ///     let rtdb = create_test_rtdb();
    ///     // ... trigger action ...
    ///     let msg = parse_todo_message(&rtdb, "comsrv:1001:A:TODO").await;
    ///     assert_eq!(msg["point_id"], 1);
    ///     assert_eq!(msg["value"], 12.3);
    /// }
    /// ```
    #[allow(dead_code)]
    pub async fn parse_todo_message<R: Rtdb>(rtdb: &Arc<R>, queue_key: &str) -> serde_json::Value {
        let messages = rtdb
            .list_range(queue_key, 0, -1)
            .await
            .expect("Failed to read TODO queue");

        assert!(
            !messages.is_empty(),
            "TODO queue '{}' should have messages",
            queue_key
        );

        let message_bytes = &messages[0];
        let message_str =
            String::from_utf8(message_bytes.to_vec()).expect("Message should be valid UTF-8");

        serde_json::from_str(&message_str).expect("Message should be valid JSON")
    }
}
