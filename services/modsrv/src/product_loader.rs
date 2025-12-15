//! Product Configuration Loader for ModSrv
//!
//! Products are loaded from database at runtime (via cloud sync API or Monarch import).
//! This loader provides database schema initialization and product query methods.

#![allow(dead_code)]

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use tracing::{debug, info, warn};

// Re-export types from local config for other modules
pub use crate::config::{
    ActionPoint, CreateInstanceRequest, Instance, MeasurementPoint, Product, ProductHierarchy,
    PropertyTemplate,
};
pub use voltage_model::PointRole;

/// Product loader that populates database from code-defined product types
#[derive(Clone)]
pub struct ProductLoader {
    pool: SqlitePool,
}

impl ProductLoader {
    /// Create a new ProductLoader with database connection
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Legacy constructor for backward compatibility
    /// The products_dir parameter is ignored since we load from code
    #[deprecated(note = "Use new(pool) instead - products are now loaded from code definitions")]
    pub fn with_dir(_products_dir: impl Into<std::path::PathBuf>, pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize database tables with separate tables for each point type
    pub async fn init_database(&self) -> Result<()> {
        debug!("Init product tables");

        // Products table (just hierarchy)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS products (
                product_name TEXT PRIMARY KEY,
                parent_name TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Measurement points table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS measurement_points (
                product_name TEXT NOT NULL,
                measurement_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                unit TEXT,
                description TEXT,
                PRIMARY KEY (product_name, measurement_id),
                FOREIGN KEY (product_name) REFERENCES products(product_name) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Action points table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS action_points (
                product_name TEXT NOT NULL,
                action_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                unit TEXT,
                description TEXT,
                PRIMARY KEY (product_name, action_id),
                FOREIGN KEY (product_name) REFERENCES products(product_name) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Property templates table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS property_templates (
                product_name TEXT NOT NULL,
                property_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                unit TEXT,
                description TEXT,
                PRIMARY KEY (product_name, property_id),
                FOREIGN KEY (product_name) REFERENCES products(product_name) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create instances table with parent_id for topology hierarchy
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS instances (
                instance_id INTEGER PRIMARY KEY,
                instance_name TEXT UNIQUE NOT NULL,
                product_name TEXT NOT NULL,
                parent_id INTEGER,
                properties TEXT,  -- JSON format
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (product_name) REFERENCES products(product_name),
                FOREIGN KEY (parent_id) REFERENCES instances(instance_id) ON DELETE SET NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create instance mappings table for point-to-redis-key mappings
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS instance_mappings (
                instance_id INTEGER,
                point_type TEXT,  -- 'M' or 'A'
                point_id INTEGER,
                redis_key TEXT,
                PRIMARY KEY (instance_id, point_type, point_id),
                FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create point mappings table for channel-instance point routing
        // UNIQUE constraint ensures each instance point has only one data source
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS point_mappings (
                mapping_id INTEGER PRIMARY KEY AUTOINCREMENT,
                instance_id INTEGER NOT NULL,
                channel_id INTEGER NOT NULL,
                channel_type TEXT NOT NULL CHECK(channel_type IN ('T','S','C','A')),
                channel_point_id INTEGER NOT NULL,
                instance_type TEXT NOT NULL CHECK(instance_type IN ('M','A')),
                instance_point_id INTEGER NOT NULL,
                description TEXT,
                enabled BOOLEAN DEFAULT TRUE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(instance_id, instance_type, instance_point_id),
                FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_mapping_channel ON point_mappings(channel_id, channel_type)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_mapping_instance ON point_mappings(instance_id)",
        )
        .execute(&self.pool)
        .await?;

        // Calculations table for virtual/computed points
        // Stores calculation definitions loaded from YAML via Monarch
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS calculations (
                calculation_id INTEGER PRIMARY KEY AUTOINCREMENT,
                calculation_name TEXT NOT NULL UNIQUE,
                description TEXT,
                calculation_type TEXT NOT NULL,  -- JSON serialized CalculationType
                output_inst INTEGER NOT NULL,    -- Output instance ID
                output_type TEXT NOT NULL CHECK(output_type IN ('M', 'A')),  -- M=Measurement, A=Action
                output_id INTEGER NOT NULL,      -- Output point ID
                enabled BOOLEAN DEFAULT TRUE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Index for efficient lookup by output point
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_calc_output ON calculations(output_inst, output_type, output_id)",
        )
        .execute(&self.pool)
        .await?;

        // Product library metadata for cloud sync versioning
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS product_library_meta (
                version TEXT PRIMARY KEY
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        debug!("Product tables ready");
        Ok(())
    }

    /// Verify products exist in database
    ///
    /// Products are now loaded from database at runtime (via cloud sync API or Monarch import).
    /// This method verifies the database has product definitions.
    pub async fn load_all(&self) -> Result<()> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&self.pool)
            .await?;

        if count == 0 {
            warn!("No products in database. Use /api/products/sync or monarch to import.");
        } else {
            info!("Database has {} products", count);
        }
        Ok(())
    }

    /// Clear all product data from database
    async fn clear_all_products(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM property_templates")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM action_points")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM measurement_points")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM products")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Get a complete product with nested structure
    ///
    /// @input product_name: &str - Product identifier to retrieve
    /// @output Result<Product> - Product with all point definitions
    /// @throws anyhow::Error - Product not found or database error
    /// @loads measurement_points, action_points, property_templates tables
    /// @side-effects None (read-only)
    pub async fn get_product(&self, product_name: &str) -> Result<Product> {
        // Get product basic info
        let (product_name_db, parent_name_db) = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT product_name, parent_name FROM products WHERE product_name = ?",
        )
        .bind(product_name)
        .fetch_one(&self.pool)
        .await
        .context(format!("Product not found: {}", product_name))?;

        // Get measurement points
        let measurements = sqlx::query_as::<_, (i32, String, Option<String>, Option<String>)>(
            r#"
            SELECT measurement_id, name, unit, description
            FROM measurement_points
            WHERE product_name = ?
            ORDER BY measurement_id
            "#,
        )
        .bind(product_name)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(
            |(measurement_id, name, unit, description)| MeasurementPoint {
                measurement_id: measurement_id as u32,
                name,
                unit,
                description,
            },
        )
        .collect();

        // Get action points
        let actions = sqlx::query_as::<_, (i32, String, Option<String>, Option<String>)>(
            r#"
            SELECT action_id, name, unit, description
            FROM action_points
            WHERE product_name = ?
            ORDER BY action_id
            "#,
        )
        .bind(product_name)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(action_id, name, unit, description)| ActionPoint {
            action_id: action_id as u32,
            name,
            unit,
            description,
        })
        .collect();

        // Get property templates
        let properties = sqlx::query_as::<_, (i32, String, Option<String>, Option<String>)>(
            r#"
            SELECT property_id, name, unit, description
            FROM property_templates
            WHERE product_name = ?
            ORDER BY property_id
            "#,
        )
        .bind(product_name)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(property_id, name, unit, description)| PropertyTemplate {
            property_id,
            name,
            unit,
            description,
        })
        .collect();

        Ok(Product {
            product_name: product_name_db,
            parent_name: parent_name_db,
            measurements,
            actions,
            properties,
        })
    }

    /// Get all products
    ///
    /// @input None
    /// @output Result<Vec<Product>> - All products with complete definitions
    /// @throws anyhow::Error - Database query error
    /// @side-effects None (read-only)
    /// @performance O(n*m) where n=products, m=points per product
    pub async fn get_all_products(&self) -> Result<Vec<Product>> {
        let product_names = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM products ORDER BY product_name",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut products = Vec::new();
        for name in product_names {
            products.push(self.get_product(&name).await?);
        }

        Ok(products)
    }

    /// Get product hierarchy
    ///
    /// @input None
    /// @output Result<ProductHierarchy> - Vec<(product_name, parent_name)> tuples
    /// @throws anyhow::Error - Database query error
    /// @side-effects None (read-only)
    /// @returns Flat list of parent-child relationships
    pub async fn get_product_hierarchy(&self) -> Result<ProductHierarchy> {
        let rows = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT product_name, parent_name FROM products ORDER BY product_name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get all product names without loading point details
    ///
    /// This is a lightweight query that returns only product basic information
    /// (product_name and parent_name) without loading measurements, actions, or properties.
    /// Ideal for frontend dropdown lists or selection interfaces.
    ///
    /// @input None
    /// @output Result<Vec<(String, Option<String>)>> - Vec of (product_name, parent_name) tuples
    /// @throws anyhow::Error - Database query error
    /// @side-effects None (read-only)
    /// @performance O(n) where n=product count - much faster than get_all_products()
    pub async fn get_all_product_names(&self) -> Result<Vec<(String, Option<String>)>> {
        let rows = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT product_name, parent_name FROM products ORDER BY product_name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Reload product definitions from database
    ///
    /// Called after cloud sync to refresh any cached state.
    /// Currently ProductLoader queries database directly without caching,
    /// so this method just logs the reload event for auditing.
    ///
    /// @output Result<()> - Success or error
    pub async fn reload(&self) -> Result<()> {
        // Verify database connectivity by counting products
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&self.pool)
            .await?;

        info!("Product library reloaded: {} products", count);
        Ok(())
    }

    /// Add a measurement point to a product
    pub async fn add_measurement(
        &self,
        product_name: &str,
        point: &MeasurementPoint,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO measurement_points
            (product_name, measurement_id, name, unit, description)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(product_name, measurement_id) DO UPDATE SET
                name = excluded.name,
                unit = excluded.unit,
                description = excluded.description
            "#,
        )
        .bind(product_name)
        .bind(point.measurement_id)
        .bind(&point.name)
        .bind(&point.unit)
        .bind(&point.description)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Generate Redis key for a point
    pub fn get_redis_key(instance: &str, point_role: PointRole, id: i32) -> String {
        let type_prefix = point_role.as_str();
        format!("modsrv:{}:{}:{}", instance, type_prefix, id)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use sqlx::Row;

    async fn setup_test_env() -> Result<ProductLoader> {
        let db_path = ":memory:";
        let sqlite_url = format!("sqlite:{db_path}?mode=memory&cache=shared");
        let pool = SqlitePool::connect(&sqlite_url).await?;

        // Use standard modsrv schema from common test utils
        common::test_utils::schema::init_modsrv_schema(&pool).await?;

        let loader = ProductLoader::new(pool);
        Ok(loader)
    }

    /// Insert a test product fixture into database
    async fn insert_test_product(
        loader: &ProductLoader,
        name: &str,
        parent: Option<&str>,
    ) -> Result<()> {
        sqlx::query("INSERT INTO products (product_name, parent_name) VALUES (?, ?)")
            .bind(name)
            .bind(parent)
            .execute(&loader.pool)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_init_database() {
        let loader = setup_test_env().await.expect("Failed to setup test env");

        // Verify tables were created
        let result = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&loader.pool)
            .await
            .expect("Failed to query tables");

        let table_names: Vec<String> = result
            .iter()
            .map(|row| row.try_get::<String, _>("name").unwrap())
            .collect();

        assert!(table_names.contains(&"products".to_string()));
        assert!(table_names.contains(&"measurement_points".to_string()));
        assert!(table_names.contains(&"action_points".to_string()));
        assert!(table_names.contains(&"property_templates".to_string()));
    }

    #[tokio::test]
    async fn test_load_all_empty_database() -> Result<()> {
        let loader = setup_test_env().await.expect("Failed to setup test env");

        // load_all should succeed even with empty database (just logs warning)
        loader.load_all().await.expect("load_all should not fail");

        // Verify no products
        let products = loader.get_all_products().await?;
        assert!(products.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_get_product_with_fixture() -> Result<()> {
        let loader = setup_test_env().await.expect("Failed to setup test env");

        // Insert test product
        insert_test_product(&loader, "test_inverter", Some("station")).await?;

        // Add a measurement point
        let point = MeasurementPoint {
            measurement_id: 1,
            name: "Power".to_string(),
            unit: Some("kW".to_string()),
            description: Some("Active power output".to_string()),
        };
        loader.add_measurement("test_inverter", &point).await?;

        // Get product
        let product = loader.get_product("test_inverter").await?;

        assert_eq!(product.product_name, "test_inverter");
        assert_eq!(product.parent_name, Some("station".to_string()));
        assert_eq!(product.measurements.len(), 1);
        assert_eq!(product.measurements[0].name, "Power");

        Ok(())
    }

    #[tokio::test]
    async fn test_add_measurement() -> Result<()> {
        let loader = setup_test_env().await.expect("Failed to setup test env");

        // Insert test product
        insert_test_product(&loader, "test_product", None).await?;

        // Add measurement
        let new_point = MeasurementPoint {
            measurement_id: 999,
            name: "Custom Measurement".to_string(),
            unit: Some("kW".to_string()),
            description: Some("Custom calculated power".to_string()),
        };

        loader.add_measurement("test_product", &new_point).await?;

        // Get product and verify point was added
        let product = loader.get_product("test_product").await?;

        let added_point = product
            .measurements
            .iter()
            .find(|m| m.measurement_id == 999);
        assert!(added_point.is_some());

        let ap = added_point.unwrap();
        assert_eq!(ap.name, "Custom Measurement");
        assert_eq!(ap.description, Some("Custom calculated power".to_string()));

        Ok(())
    }

    #[test]
    fn test_redis_key_generation() {
        let key = ProductLoader::get_redis_key("pv_inv_001", PointRole::Measurement, 1);
        assert_eq!(key, "modsrv:pv_inv_001:M:1");

        let key = ProductLoader::get_redis_key("pv_inv_001", PointRole::Action, 1);
        assert_eq!(key, "modsrv:pv_inv_001:A:1");
    }

    #[tokio::test]
    async fn test_get_all_products() -> Result<()> {
        let loader = setup_test_env().await.expect("Failed to setup test env");

        // Insert test products
        insert_test_product(&loader, "station", None).await?;
        insert_test_product(&loader, "inverter", Some("station")).await?;
        insert_test_product(&loader, "battery", Some("station")).await?;

        // Get all products
        let products = loader.get_all_products().await?;
        assert_eq!(products.len(), 3);

        let product_names: Vec<&str> = products.iter().map(|p| p.product_name.as_str()).collect();
        assert!(product_names.contains(&"station"));
        assert!(product_names.contains(&"inverter"));
        assert!(product_names.contains(&"battery"));

        Ok(())
    }

    #[tokio::test]
    async fn test_product_hierarchy() -> Result<()> {
        let loader = setup_test_env().await.expect("Failed to setup test env");

        // Insert hierarchical products
        insert_test_product(&loader, "station", None).await?;
        insert_test_product(&loader, "battery_stack", Some("station")).await?;
        insert_test_product(&loader, "battery_cluster", Some("battery_stack")).await?;

        // Verify hierarchy
        let stack = loader.get_product("battery_stack").await?;
        assert_eq!(stack.parent_name, Some("station".to_string()));

        let cluster = loader.get_product("battery_cluster").await?;
        assert_eq!(cluster.parent_name, Some("battery_stack".to_string()));

        Ok(())
    }
}
