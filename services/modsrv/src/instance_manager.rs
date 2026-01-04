//! Instance Manager - Core Lifecycle Operations
//!
//! This module provides the core instance lifecycle management.
//! Extended functionality is provided in separate modules:
//! - `instance_routing.rs` - Routing CRUD operations
//! - `instance_redis_sync.rs` - Redis synchronization
//! - `instance_data.rs` - Data loading and querying

use crate::config::InstanceRedisKeys;
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
    /// @output `Result<Instance>` - Created instance with all point routings
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

        // 2. Verify product exists
        // Note: Name uniqueness is enforced by database UNIQUE constraint.
        // We rely on the constraint rather than check-then-act to avoid race conditions.
        let product = self.product_loader.get_product(&req.product_name).await?;

        // 3. Begin transaction for atomic creation
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

        // 4. Create instance in SQLite within transaction
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

        // 5. Create point routings for measurement and action points within transaction
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

        // 6. Commit transaction first (ensure database persistence)
        if let Err(e) = tx.commit().await {
            error!(
                "Failed to commit transaction for instance {}: {}",
                instance_name, e
            );
            return Err(anyhow!("Database transaction commit failed: {}", e));
        }

        // 7. Best effort register instance in Redis (after commit, allow failure)
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
            let properties: HashMap<String, serde_json::Value> = match properties_json {
                Some(json) => serde_json::from_str(&json).map_err(|e| {
                    anyhow!(
                        "Invalid properties JSON for instance {}: {}",
                        instance_id,
                        e
                    )
                })?,
                None => HashMap::new(),
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
            let properties: HashMap<String, serde_json::Value> = match properties_json {
                Some(json) => serde_json::from_str(&json).map_err(|e| {
                    anyhow!(
                        "Invalid properties JSON for instance {}: {}",
                        instance_id,
                        e
                    )
                })?,
                None => HashMap::new(),
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
            let properties: HashMap<String, serde_json::Value> = match properties_json {
                Some(json) => serde_json::from_str(&json).map_err(|e| {
                    anyhow!(
                        "Invalid properties JSON for instance {}: {}",
                        instance_id,
                        e
                    )
                })?,
                None => HashMap::new(),
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
        let properties: HashMap<String, serde_json::Value> = match properties_json {
            Some(json) => serde_json::from_str(&json).map_err(|e| {
                anyhow!(
                    "Invalid properties JSON for instance {}: {}",
                    instance_id,
                    e
                )
            })?,
            None => HashMap::new(),
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
#[path = "instance_manager_tests.rs"]
mod tests;
