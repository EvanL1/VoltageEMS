//! Instance Manager - Core Lifecycle Operations
//!
//! This module provides the core instance lifecycle management.
//! Extended functionality is provided in separate modules:
//! - `instance_routing.rs` - Routing CRUD operations
//! - `instance_redis_sync.rs` - Redis synchronization
//! - `instance_data.rs` - Data loading and querying

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::config::{InstanceRedisKeys, ModsrvQueries};
use anyhow::{anyhow, Result};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};
use voltage_model::validate_instance_name;
use voltage_rtdb::Rtdb;

use crate::product_loader::{CreateInstanceRequest, Instance, ProductLoader};

/// Instance Manager handles runtime instance lifecycle
pub struct InstanceManager<R: Rtdb> {
    pub pool: SqlitePool,
    pub rtdb: Arc<R>,
    pub(crate) routing_cache: Arc<voltage_rtdb::RoutingCache>,
    pub(crate) product_loader: Arc<ProductLoader>,
}

impl<R: Rtdb + 'static> InstanceManager<R> {
    pub fn new(
        pool: SqlitePool,
        rtdb: Arc<R>,
        routing_cache: Arc<voltage_rtdb::RoutingCache>,
        product_loader: Arc<ProductLoader>,
    ) -> Self {
        Self {
            pool,
            rtdb,
            routing_cache,
            product_loader,
        }
    }

    /// Get the routing cache reference
    ///
    /// Returns a reference to the shared routing cache for use in API handlers
    /// that need to refresh the cache after routing management operations.
    pub fn routing_cache(&self) -> &Arc<voltage_rtdb::RoutingCache> {
        &self.routing_cache
    }

    /// Create a new instance based on a product template
    ///
    /// @input req: CreateInstanceRequest - Instance configuration
    /// @output Result<Instance> - Created instance with all point routings
    /// @throws anyhow::Error - Instance exists, product not found, database error
    /// @side-effects Creates instance in SQLite, initializes Redis keys
    /// @transaction Full creation is atomic
    pub async fn create_instance(&self, req: CreateInstanceRequest) -> Result<Instance> {
        // Use user-provided instance ID and name
        let instance_id = req.instance_id;
        let instance_name = req.instance_name.clone();

        info!(
            "Creating instance: {} (id: {}) for product: {}",
            instance_name, instance_id, req.product_name
        );

        // 1. Validate instance name format
        if let Err(e) = validate_instance_name(&instance_name) {
            return Err(anyhow!("Invalid instance name: {}", e));
        }

        // 2. Check if instance name already exists (enforces uniqueness)
        let exists = sqlx::query_scalar::<_, bool>(ModsrvQueries::CHECK_INSTANCE_NAME_EXISTS)
            .bind(&instance_name)
            .fetch_one(&self.pool)
            .await?;

        if exists {
            return Err(anyhow!(
                "Instance with name '{}' already exists. Please choose a different name.",
                instance_name
            ));
        }

        // 3. Verify product exists
        let product = self.product_loader.get_product(&req.product_name).await?;

        // 4. Begin transaction for atomic creation
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!(
                    "Failed to begin transaction for instance {}: {}",
                    instance_name, e
                );
                return Err(anyhow!("Database transaction failed: {}", e));
            },
        };

        // 5. Create instance in SQLite within transaction
        let properties_json = serde_json::to_string(&req.properties)?;

        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO instances (instance_id, instance_name, product_name, properties)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(instance_id as i32)
        .bind(&instance_name)
        .bind(&req.product_name)
        .bind(&properties_json)
        .execute(&mut *tx)
        .await
        {
            error!("Failed to insert instance {}: {}", instance_name, e);
            let _ = tx.rollback().await;
            return Err(anyhow!("Failed to create instance: {}", e));
        }

        // 6. Create point routings for measurement and action points within transaction
        // Measurement point routing - maps point IDs to Redis keys
        let mut measurement_point_routings = HashMap::new();
        // Action point routing - maps point IDs to Redis keys
        let mut action_point_routings = HashMap::new();

        // Build point routing maps for Redis registration (generate Redis keys)
        // Note: Routing configuration is managed by routing_loader.rs, not stored here
        for point in &product.measurements {
            let redis_key = InstanceRedisKeys::measurement(instance_id, point.measurement_id);
            measurement_point_routings.insert(point.measurement_id, redis_key);
        }

        for point in &product.actions {
            let redis_key = InstanceRedisKeys::action(instance_id, point.action_id);
            action_point_routings.insert(point.action_id, redis_key);
        }

        // 7. Commit transaction first (ensure database persistence)
        if let Err(e) = tx.commit().await {
            error!(
                "Failed to commit transaction for instance {}: {}",
                instance_name, e
            );
            return Err(anyhow!("Database transaction commit failed: {}", e));
        }

        // 8. Best effort register instance in Redis (after commit, allow failure)
        info!("Registering instance {} in Redis", instance_name);
        if let Err(e) = self
            .register_instance_in_redis(
                req.instance_id,
                &instance_name,
                &req.product_name,
                &req.properties,
                &product.measurements,
                &product.actions,
                &measurement_point_routings,
                &action_point_routings,
            )
            .await
        {
            warn!(
                "Instance {} created in SQLite but Redis registration failed: {}. Will register on next reload.",
                instance_name, e
            );
        } else {
            info!(
                "Successfully registered instance {} in Redis after creation",
                instance_name
            );
        }

        info!("Successfully created instance {}", instance_name);

        // 8. Return created instance
        Ok(Instance {
            core: crate::config::InstanceCore {
                instance_id,
                instance_name,
                product_name: req.product_name,
                properties: req.properties,
            },
            measurement_mappings: Some(measurement_point_routings),
            action_mappings: Some(action_point_routings),
            created_at: Some(chrono::Utc::now()),
        })
    }

    /// List all instances, optionally filtered by product_name
    pub async fn list_instances(&self, product_name: Option<&str>) -> Result<Vec<Instance>> {
        let query = if let Some(pname) = product_name {
            sqlx::query_as::<_, (u32, String, String, Option<String>, String)>(
                r#"
                SELECT instance_id, instance_name, product_name, properties, created_at
                FROM instances
                WHERE product_name = ?
                ORDER BY instance_id ASC
                "#,
            )
            .bind(pname)
        } else {
            sqlx::query_as::<_, (u32, String, String, Option<String>, String)>(
                r#"
                SELECT instance_id, instance_name, product_name, properties, created_at
                FROM instances
                ORDER BY instance_id ASC
                "#,
            )
        };

        let rows = query.fetch_all(&self.pool).await?;

        let mut instances = Vec::new();
        for (instance_id, instance_name, product_name, properties_json, _created_at) in rows {
            let properties: HashMap<String, serde_json::Value> = if let Some(json) = properties_json
            {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                HashMap::new()
            };

            instances.push(Instance {
                core: crate::config::InstanceCore {
                    instance_id,
                    instance_name,
                    product_name,
                    properties,
                },
                measurement_mappings: None,
                action_mappings: None,
                created_at: None,
            });
        }

        Ok(instances)
    }

    /// List instances with pagination
    pub async fn list_instances_paginated(
        &self,
        product_name: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(u32, Vec<Instance>)> {
        // Calculate offset
        let offset = (page - 1) * page_size;

        // Get total count
        let count_query = if let Some(pname) = product_name {
            sqlx::query_as::<_, (i64,)>(
                r#"
                SELECT COUNT(*) FROM instances WHERE product_name = ?
                "#,
            )
            .bind(pname)
        } else {
            sqlx::query_as::<_, (i64,)>(
                r#"
                SELECT COUNT(*) FROM instances
                "#,
            )
        };

        let (total,) = count_query.fetch_one(&self.pool).await?;

        // Get paginated data
        let data_query = if let Some(pname) = product_name {
            sqlx::query_as::<_, (u32, String, String, Option<String>, String)>(
                r#"
                SELECT instance_id, instance_name, product_name, properties, created_at
                FROM instances
                WHERE product_name = ?
                ORDER BY instance_id ASC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(pname)
            .bind(page_size as i64)
            .bind(offset as i64)
        } else {
            sqlx::query_as::<_, (u32, String, String, Option<String>, String)>(
                r#"
                SELECT instance_id, instance_name, product_name, properties, created_at
                FROM instances
                ORDER BY instance_id ASC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(page_size as i64)
            .bind(offset as i64)
        };

        let rows = data_query.fetch_all(&self.pool).await?;

        let mut instances = Vec::new();
        for (instance_id, instance_name, product_name, properties_json, _created_at) in rows {
            let properties: HashMap<String, serde_json::Value> = if let Some(json) = properties_json
            {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                HashMap::new()
            };

            instances.push(Instance {
                core: crate::config::InstanceCore {
                    instance_id,
                    instance_name,
                    product_name,
                    properties,
                },
                measurement_mappings: None,
                action_mappings: None,
                created_at: None,
            });
        }

        Ok((total as u32, instances))
    }

    /// Search instances by name with fuzzy matching
    pub async fn search_instances(
        &self,
        keyword: &str,
        product_name: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(u32, Vec<Instance>)> {
        let offset = (page - 1) * page_size;
        let like_pattern = format!("%{}%", keyword);

        // Get total count
        let (total,): (i64,) = if let Some(pname) = product_name {
            sqlx::query_as(
                r#"
                SELECT COUNT(*) FROM instances
                WHERE instance_name LIKE ? AND product_name = ?
                "#,
            )
            .bind(&like_pattern)
            .bind(pname)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT COUNT(*) FROM instances
                WHERE instance_name LIKE ?
                "#,
            )
            .bind(&like_pattern)
            .fetch_one(&self.pool)
            .await?
        };

        // Get paginated data
        let rows: Vec<(u32, String, String, Option<String>, String)> =
            if let Some(pname) = product_name {
                sqlx::query_as(
                    r#"
                SELECT instance_id, instance_name, product_name, properties, created_at
                FROM instances
                WHERE instance_name LIKE ? AND product_name = ?
                ORDER BY instance_id ASC
                LIMIT ? OFFSET ?
                "#,
                )
                .bind(&like_pattern)
                .bind(pname)
                .bind(page_size as i64)
                .bind(offset as i64)
                .fetch_all(&self.pool)
                .await?
            } else {
                sqlx::query_as(
                    r#"
                SELECT instance_id, instance_name, product_name, properties, created_at
                FROM instances
                WHERE instance_name LIKE ?
                ORDER BY instance_id ASC
                LIMIT ? OFFSET ?
                "#,
                )
                .bind(&like_pattern)
                .bind(page_size as i64)
                .bind(offset as i64)
                .fetch_all(&self.pool)
                .await?
            };

        let mut instances = Vec::new();
        for (instance_id, instance_name, product_name, properties_json, _created_at) in rows {
            let properties: HashMap<String, serde_json::Value> = if let Some(json) = properties_json
            {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                HashMap::new()
            };

            instances.push(Instance {
                core: crate::config::InstanceCore {
                    instance_id,
                    instance_name,
                    product_name,
                    properties,
                },
                measurement_mappings: None,
                action_mappings: None,
                created_at: None,
            });
        }

        Ok((total as u32, instances))
    }

    /// Rename an instance
    pub async fn rename_instance(&self, instance_id: u32, new_name: &str) -> Result<()> {
        // Check if new name already exists
        let (count,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM instances WHERE instance_name = ? AND instance_id != ?"#,
        )
        .bind(new_name)
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await?;

        if count > 0 {
            anyhow::bail!("Instance name '{}' already exists, cannot rename", new_name);
        }

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Update instances table
        sqlx::query(
            r#"UPDATE instances SET instance_name = ?, updated_at = CURRENT_TIMESTAMP WHERE instance_id = ?"#,
        )
        .bind(new_name)
        .bind(instance_id as i32)
        .execute(&mut *tx)
        .await?;

        // Update measurement_routing table (redundant field)
        sqlx::query(r#"UPDATE measurement_routing SET instance_name = ? WHERE instance_id = ?"#)
            .bind(new_name)
            .bind(instance_id as i32)
            .execute(&mut *tx)
            .await?;

        // Update action_routing table (redundant field)
        sqlx::query(r#"UPDATE action_routing SET instance_name = ? WHERE instance_id = ?"#)
            .bind(new_name)
            .bind(instance_id as i32)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        info!(
            "Instance {} renamed to '{}' in SQLite",
            instance_id, new_name
        );
        Ok(())
    }

    /// Get next available instance ID
    pub async fn get_next_instance_id(&self) -> Result<u32> {
        let row = sqlx::query_as::<_, (Option<i32>,)>("SELECT MAX(instance_id) FROM instances")
            .fetch_one(&self.pool)
            .await?;

        match row.0 {
            Some(max_id) => {
                let next_id = max_id + 1;
                if next_id < 0 {
                    anyhow::bail!("Instance ID overflow: negative value");
                }
                Ok(next_id as u32)
            },
            None => Ok(1), // First instance
        }
    }

    /// Get instance by ID
    pub async fn get_instance(&self, instance_id: u32) -> Result<Instance> {
        let row = sqlx::query_as::<_, (String, String, Option<String>, String)>(
            r#"
            SELECT instance_name, product_name, properties, created_at
            FROM instances
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id as i32)
        .fetch_optional(&self.pool)
        .await?;

        let row = row.ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        let (instance_name, product_name, properties_json, _created_at) = row;
        let properties: HashMap<String, serde_json::Value> = if let Some(json) = properties_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            HashMap::new()
        };

        // Load point routings from routing tables and generate Redis keys dynamically
        let mut measurement_point_routings = HashMap::new();
        let mut action_point_routings = HashMap::new();

        // Query measurement routing
        let measurement_points = sqlx::query_as::<_, (u32,)>(
            r#"
            SELECT measurement_id
            FROM measurement_routing
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id as i32)
        .fetch_all(&self.pool)
        .await?;

        for (point_id,) in measurement_points {
            let redis_key = InstanceRedisKeys::measurement(instance_id, point_id);
            measurement_point_routings.insert(point_id, redis_key);
        }

        // Query action routing
        let action_points = sqlx::query_as::<_, (u32,)>(
            r#"
            SELECT action_id
            FROM action_routing
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id as i32)
        .fetch_all(&self.pool)
        .await?;

        for (point_id,) in action_points {
            let redis_key = InstanceRedisKeys::action(instance_id, point_id);
            action_point_routings.insert(point_id, redis_key);
        }

        Ok(Instance {
            core: crate::config::InstanceCore {
                instance_id,
                instance_name,
                product_name,
                properties,
            },
            measurement_mappings: Some(measurement_point_routings),
            action_mappings: Some(action_point_routings),
            created_at: None,
        })
    }

    /// Delete an instance by ID
    pub async fn delete_instance(&self, instance_id: u32) -> Result<()> {
        // 1. Query instance_name before deletion (needed for Redis cleanup and logging)
        let instance_name: String =
            sqlx::query_scalar("SELECT instance_name FROM instances WHERE instance_id = ?")
                .bind(instance_id as i32)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // 2. Begin transaction for atomic deletion
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!(
                    "Failed to begin transaction for deleting instance {} ({}): {}",
                    instance_id, instance_name, e
                );
                return Err(anyhow!("Database transaction failed: {}", e));
            },
        };

        // 3. Delete from SQLite within transaction (cascade will handle point routings)
        let result = sqlx::query("DELETE FROM instances WHERE instance_id = ?")
            .bind(instance_id as i32)
            .execute(&mut *tx)
            .await;

        match result {
            Ok(res) => {
                if res.rows_affected() == 0 {
                    // Rollback transaction
                    let _ = tx.rollback().await;
                    return Err(anyhow!("Instance not found: {}", instance_id));
                }
            },
            Err(e) => {
                error!(
                    "Failed to delete instance {} ({}) from SQLite: {}",
                    instance_id, instance_name, e
                );
                let _ = tx.rollback().await;
                return Err(anyhow!("Failed to delete instance: {}", e));
            },
        }

        // 4. Commit transaction first (ensure database persistence)
        if let Err(e) = tx.commit().await {
            error!(
                "Failed to commit transaction for deleting instance {} ({}): {}",
                instance_id, instance_name, e
            );
            return Err(anyhow!("Database transaction commit failed: {}", e));
        }

        // 5. Best effort remove from Redis (after commit, allow failure)
        if let Err(e) = self
            .unregister_instance_from_redis(instance_id, &instance_name)
            .await
        {
            warn!(
                "Instance {} ({}) deleted from SQLite but Redis cleanup failed: {}. Will be cleaned up on next reload.",
                instance_id, instance_name, e
            );
        } else {
            info!(
                "Successfully unregistered instance {} ({}) from Redis after deletion",
                instance_id, instance_name
            );
        }

        info!(
            "Successfully deleted instance: {} ({})",
            instance_id, instance_name
        );
        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    // Helper: Create test database with all required tables
    async fn create_test_database() -> (TempDir, SqlitePool) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_instance_manager.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let pool = SqlitePool::connect(&db_url).await.unwrap();

        // Use standard modsrv schema from common test utils
        common::test_utils::schema::init_modsrv_schema(&pool)
            .await
            .unwrap();

        (temp_dir, pool)
    }

    // Helper: Create test product with points
    async fn create_test_product(pool: &SqlitePool, product_name: &str) {
        // Insert product
        sqlx::query("INSERT INTO products (product_name, parent_name) VALUES (?, NULL)")
            .bind(product_name)
            .execute(pool)
            .await
            .unwrap();

        // Insert measurement points
        sqlx::query(
            "INSERT INTO measurement_points (product_name, measurement_id, name, unit) VALUES (?, ?, ?, ?)"
        )
        .bind(product_name)
        .bind(1)
        .bind("Temperature")
        .bind("°C")
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO measurement_points (product_name, measurement_id, name, unit) VALUES (?, ?, ?, ?)"
        )
        .bind(product_name)
        .bind(2)
        .bind("Voltage")
        .bind("V")
        .execute(pool)
        .await
        .unwrap();

        // Insert action point
        sqlx::query("INSERT INTO action_points (product_name, action_id, name) VALUES (?, ?, ?)")
            .bind(product_name)
            .bind(1)
            .bind("SetPower")
            .execute(pool)
            .await
            .unwrap();
    }

    // Helper: Create test ProductLoader
    fn create_test_product_loader(pool: SqlitePool) -> Arc<ProductLoader> {
        Arc::new(crate::product_loader::ProductLoader::new(pool))
    }

    use voltage_rtdb::helpers::create_test_memory_rtdb as create_test_rtdb;

    // ==================== Phase 1: CRUD Core Tests ====================

    #[tokio::test]
    async fn test_instance_manager_new() {
        let (_temp_dir, pool) = create_test_database().await;
        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();

        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let _manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        // Test passes if InstanceManager::new() doesn't panic
    }

    #[tokio::test]
    async fn test_create_instance_success() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        let req = CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "test_instance_01".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        };

        let result = manager.create_instance(req).await;

        assert!(
            result.is_ok(),
            "Instance creation should succeed: {:?}",
            result.as_ref().err()
        );
        let instance = result.unwrap();
        assert_eq!(instance.instance_id(), 1001);
        assert_eq!(instance.instance_name(), "test_instance_01");
        assert_eq!(instance.product_name(), "test_product");
    }

    #[tokio::test]
    async fn test_create_instance_with_properties() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "solar_panel").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        let mut properties = HashMap::new();
        properties.insert("location".to_string(), serde_json::json!("Roof A"));
        properties.insert("capacity".to_string(), serde_json::json!(5000));

        let req = CreateInstanceRequest {
            instance_id: 2001,
            instance_name: "solar_panel_01".to_string(),
            product_name: "solar_panel".to_string(),
            properties,
        };

        let result = manager.create_instance(req).await;

        assert!(result.is_ok());
        let instance = result.unwrap();
        assert_eq!(instance.core.properties.len(), 2);
        assert_eq!(
            instance.core.properties.get("location").unwrap(),
            &serde_json::json!("Roof A")
        );
        assert_eq!(
            instance.core.properties.get("capacity").unwrap(),
            &serde_json::json!(5000)
        );
    }

    #[tokio::test]
    async fn test_create_instance_already_exists() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        let req = CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "duplicate_instance".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        };

        // First creation should succeed
        let result1 = manager.create_instance(req.clone()).await;
        assert!(result1.is_ok());

        // Second creation with same ID should fail
        let result2 = manager.create_instance(req).await;
        assert!(result2.is_err());
        assert!(result2.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_create_instance_product_not_found() {
        let (_temp_dir, pool) = create_test_database().await;
        // Don't create the product

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        let req = CreateInstanceRequest {
            instance_id: 3001,
            instance_name: "orphan_instance".to_string(),
            product_name: "nonexistent_product".to_string(),
            properties: HashMap::new(),
        };

        let result = manager.create_instance(req).await;

        assert!(result.is_err(), "Should fail when product doesn't exist");
    }

    #[tokio::test]
    async fn test_list_instances_all() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "product_a").await;
        create_test_product(&pool, "product_b").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        // Create multiple instances
        let req1 = CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "instance_01".to_string(),
            product_name: "product_a".to_string(),
            properties: HashMap::new(),
        };
        manager.create_instance(req1).await.unwrap();

        let req2 = CreateInstanceRequest {
            instance_id: 1002,
            instance_name: "instance_02".to_string(),
            product_name: "product_b".to_string(),
            properties: HashMap::new(),
        };
        manager.create_instance(req2).await.unwrap();

        // List all instances
        let instances = manager.list_instances(None).await.unwrap();

        assert_eq!(instances.len(), 2);
    }

    #[tokio::test]
    async fn test_list_instances_by_product() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "product_a").await;
        create_test_product(&pool, "product_b").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        // Create instances for different products
        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1001,
                instance_name: "product_a_instance_01".to_string(),
                product_name: "product_a".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1002,
                instance_name: "product_a_instance_02".to_string(),
                product_name: "product_a".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 2001,
                instance_name: "product_b_instance_01".to_string(),
                product_name: "product_b".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        // List instances for product_a only
        let instances = manager.list_instances(Some("product_a")).await.unwrap();

        assert_eq!(instances.len(), 2);
        assert!(instances.iter().all(|i| i.product_name() == "product_a"));
    }

    #[tokio::test]
    async fn test_get_instance_success() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        // Create instance
        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1001,
                instance_name: "get_test_instance".to_string(),
                product_name: "test_product".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        // Get instance by ID
        let result = manager.get_instance(1001).await;

        assert!(
            result.is_ok(),
            "Failed to get instance: {:?}",
            result.as_ref().err()
        );
        let instance = result.unwrap();
        assert_eq!(instance.instance_id(), 1001);
        assert_eq!(instance.instance_name(), "get_test_instance");
    }

    #[tokio::test]
    async fn test_get_instance_not_found() {
        let (_temp_dir, pool) = create_test_database().await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        let result = manager.get_instance(9999).await;

        assert!(result.is_err(), "Should fail when instance doesn't exist");
    }

    #[tokio::test]
    async fn test_delete_instance() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        // Create instance
        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1001,
                instance_name: "delete_test".to_string(),
                product_name: "test_product".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        // Delete instance by ID
        let result = manager.delete_instance(1001).await;
        assert!(result.is_ok());

        // Verify it's deleted
        let get_result = manager.get_instance(1001).await;
        assert!(get_result.is_err());
    }

    // ==================== Phase 2: M2C Routing Tests (execute_action) ====================

    /// Helper: Setup instance name index in RTDB (required for voltage-routing)
    async fn setup_instance_name_index(
        rtdb: &voltage_rtdb::MemoryRtdb,
        instance_id: u32,
        instance_name: &str,
    ) {
        use bytes::Bytes;
        use voltage_rtdb::Rtdb;
        rtdb.hash_set(
            "inst:name:index",
            instance_name,
            Bytes::from(instance_id.to_string()),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_execute_action_instance_not_found() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        // Execute action on non-existent instance - this now succeeds
        // but doesn't route (no route configured for instance 9999)
        // The value is stored locally to inst:9999:A hash
        let result = manager.execute_action(9999, "1", 100.0).await;

        // Current behavior: succeeds and stores locally (not routed)
        assert!(
            result.is_ok(),
            "execute_action should succeed even for unconfigured instance (stores locally)"
        );
    }

    #[tokio::test]
    async fn test_execute_action_no_route_stores_locally() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager =
            InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

        // Create instance
        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1001,
                instance_name: "action_test_instance".to_string(),
                product_name: "test_product".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        // Setup instance name index
        setup_instance_name_index(&rtdb, 1001, "action_test_instance").await;

        // Execute action (no M2C route configured, should store locally)
        let result = manager.execute_action(1001, "1", 50.0).await;

        assert!(
            result.is_ok(),
            "execute_action should succeed even without route: {:?}",
            result.as_ref().err()
        );

        // Verify value was stored in instance action key
        use voltage_rtdb::Rtdb;
        let config = voltage_rtdb::KeySpaceConfig::production();
        let action_key = config.instance_action_key(1001);
        let stored = rtdb.hash_get(&action_key, "1").await.unwrap();
        assert!(stored.is_some(), "Action value should be stored");
        assert_eq!(stored.unwrap().as_ref(), b"50");
    }

    #[tokio::test]
    async fn test_execute_action_with_route_triggers_downstream() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();

        // Configure M2C route: instance 1001, action point "1" -> channel 2, A, point 5
        let mut m2c_data = HashMap::new();
        m2c_data.insert("1001:A:1".to_string(), "2:A:5".to_string());
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::from_maps(
            HashMap::new(),
            m2c_data,
            HashMap::new(),
        ));

        let manager =
            InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

        // Create instance
        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1001,
                instance_name: "routed_action_instance".to_string(),
                product_name: "test_product".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        // Setup instance name index
        setup_instance_name_index(&rtdb, 1001, "routed_action_instance").await;

        // Execute action (M2C route configured)
        let result = manager.execute_action(1001, "1", 75.0).await;

        assert!(
            result.is_ok(),
            "execute_action should succeed with route: {:?}",
            result.as_ref().err()
        );

        // Verify instance action hash was updated
        use voltage_rtdb::Rtdb;
        let config = voltage_rtdb::KeySpaceConfig::production();
        let action_key = config.instance_action_key(1001);
        let stored = rtdb.hash_get(&action_key, "1").await.unwrap();
        assert!(stored.is_some(), "Instance action should be stored");
        assert_eq!(stored.unwrap().as_ref(), b"75");

        // Verify channel hash was updated
        use voltage_model::PointType;
        let channel_key = config.channel_key(2, PointType::Adjustment);
        let channel_value = rtdb.hash_get(&channel_key, "5").await.unwrap();
        assert!(channel_value.is_some(), "Channel point should be updated");

        // Verify TODO queue was triggered (check if there are any entries)
        let todo_key = config.todo_queue_key(2, PointType::Adjustment);
        let queue_entries = rtdb.list_range(&todo_key, 0, -1).await.unwrap();
        assert!(!queue_entries.is_empty(), "TODO queue should have entry");
    }

    #[tokio::test]
    async fn test_execute_action_multiple_points() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();

        // Configure multiple routes
        let mut m2c_data = HashMap::new();
        m2c_data.insert("1001:A:1".to_string(), "10:A:1".to_string());
        m2c_data.insert("1001:A:2".to_string(), "10:A:2".to_string());
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::from_maps(
            HashMap::new(),
            m2c_data,
            HashMap::new(),
        ));

        let manager =
            InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1001,
                instance_name: "multi_action_instance".to_string(),
                product_name: "test_product".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        setup_instance_name_index(&rtdb, 1001, "multi_action_instance").await;

        // Execute multiple actions
        let result1 = manager.execute_action(1001, "1", 10.0).await;
        let result2 = manager.execute_action(1001, "2", 20.0).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Verify both actions were stored
        use voltage_rtdb::Rtdb;
        let config = voltage_rtdb::KeySpaceConfig::production();
        let action_key = config.instance_action_key(1001);
        let v1 = rtdb.hash_get(&action_key, "1").await.unwrap().unwrap();
        let v2 = rtdb.hash_get(&action_key, "2").await.unwrap().unwrap();
        assert_eq!(v1.as_ref(), b"10");
        assert_eq!(v2.as_ref(), b"20");
    }

    #[tokio::test]
    async fn test_execute_action_value_overwrite() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager =
            InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1001,
                instance_name: "overwrite_test".to_string(),
                product_name: "test_product".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        setup_instance_name_index(&rtdb, 1001, "overwrite_test").await;

        // Execute action twice with different values
        manager.execute_action(1001, "1", 100.0).await.unwrap();
        manager.execute_action(1001, "1", 200.0).await.unwrap();

        // Verify latest value wins
        use voltage_rtdb::Rtdb;
        let config = voltage_rtdb::KeySpaceConfig::production();
        let action_key = config.instance_action_key(1001);
        let stored = rtdb.hash_get(&action_key, "1").await.unwrap().unwrap();
        assert_eq!(stored.as_ref(), b"200", "Latest value should overwrite");
    }

    #[tokio::test]
    async fn test_execute_action_negative_values() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let manager =
            InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

        manager
            .create_instance(CreateInstanceRequest {
                instance_id: 1001,
                instance_name: "negative_test".to_string(),
                product_name: "test_product".to_string(),
                properties: HashMap::new(),
            })
            .await
            .unwrap();

        setup_instance_name_index(&rtdb, 1001, "negative_test").await;

        // Execute action with negative value
        let result = manager.execute_action(1001, "1", -50.5).await;
        assert!(result.is_ok());

        // Verify negative value was stored correctly
        use voltage_rtdb::Rtdb;
        let config = voltage_rtdb::KeySpaceConfig::production();
        let action_key = config.instance_action_key(1001);
        let stored = rtdb.hash_get(&action_key, "1").await.unwrap().unwrap();
        assert_eq!(stored.as_ref(), b"-50.5");
    }
}
