#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use anyhow::{anyhow, Result};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use voltage_config::{
    common::{ValidationLevel, ValidationResult},
    modsrv::{InstanceRedisKeys, ModsrvQueries},
};
use voltage_model::validate_instance_name;
use voltage_rtdb::Rtdb;

use crate::product_loader::{CreateInstanceRequest, Instance, ProductLoader};
use crate::redis_state;
use crate::routing_loader::{
    ActionRouting, ActionRoutingRow, MeasurementRouting, MeasurementRoutingRow,
};

/// Instance Manager handles runtime instance lifecycle
pub struct InstanceManager<R: Rtdb> {
    pub pool: SqlitePool,
    pub rtdb: Arc<R>,
    routing_cache: Arc<voltage_config::RoutingCache>,
    product_loader: Arc<ProductLoader>,
}

impl<R: Rtdb + 'static> InstanceManager<R> {
    pub fn new(
        pool: SqlitePool,
        rtdb: Arc<R>,
        routing_cache: Arc<voltage_config::RoutingCache>,
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
    pub fn routing_cache(&self) -> &Arc<voltage_config::RoutingCache> {
        &self.routing_cache
    }

    /// Create a new instance based on a product template
    ///
    /// @input req: CreateInstanceRequest - Instance configuration
    /// @output Result<Instance> - Created instance with all point routings
    /// @throws anyhow::Error - Instance exists, product not found, database error
    /// @side-effects Creates instance in SQLite, initializes Redis keys
    /// @redis-write modsrv:{instance}:status - Instance status
    /// @redis-write modsrv:{instance}:config - Instance configuration
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
            core: voltage_config::modsrv::InstanceCore {
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
    ///
    /// @input product_name: Option<&str> - Optional filter by product type
    /// @output Result<Vec<Instance>> - List of instances with properties
    /// @throws anyhow::Error - Database query error
    /// @redis-read modsrv:{instance}:status - Instance runtime status
    /// @side-effects None (read-only operation)
    pub async fn list_instances(&self, product_name: Option<&str>) -> Result<Vec<Instance>> {
        let query = if let Some(pname) = product_name {
            sqlx::query_as::<_, (i32, String, String, Option<String>, String)>(
                r#"
                SELECT instance_id, instance_name, product_name, properties, created_at
                FROM instances
                WHERE product_name = ?
                ORDER BY instance_id ASC
                "#,
            )
            .bind(pname)
        } else {
            sqlx::query_as::<_, (i32, String, String, Option<String>, String)>(
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

            // Validate instance_id is within u16 range
            if instance_id < 0 || instance_id > u16::MAX as i32 {
                warn!(
                    "Instance ID {} is out of u16 range, skipping instance {}",
                    instance_id, instance_name
                );
                continue;
            }

            instances.push(Instance {
                core: voltage_config::modsrv::InstanceCore {
                    instance_id: instance_id as u16,
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
    ///
    /// @input product_name: Option<&str> - Optional filter by product type
    /// @input page: u32 - Page number (1-indexed)
    /// @input page_size: u32 - Items per page
    /// @output Result<(u32, Vec<Instance>)> - (Total count, instances for current page)
    /// @throws anyhow::Error - Database query error
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
            sqlx::query_as::<_, (i32, String, String, Option<String>, String)>(
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
            sqlx::query_as::<_, (i32, String, String, Option<String>, String)>(
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

            // Validate instance_id is within u16 range
            if instance_id < 0 || instance_id > u16::MAX as i32 {
                warn!(
                    "Instance ID {} is out of u16 range, skipping instance {}",
                    instance_id, instance_name
                );
                continue;
            }

            instances.push(Instance {
                core: voltage_config::modsrv::InstanceCore {
                    instance_id: instance_id as u16,
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
    ///
    /// Performs a LIKE query on instance_name with optional product filter.
    ///
    /// @input keyword: &str - Search keyword for fuzzy matching
    /// @input product_name: Option<&str> - Optional filter by product type
    /// @input page: u32 - Page number (1-indexed)
    /// @input page_size: u32 - Items per page
    /// @output Result<(u32, Vec<Instance>)> - (Total count, instances for current page)
    /// @throws anyhow::Error - Database query error
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
        let rows: Vec<(i32, String, String, Option<String>, String)> =
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

            if instance_id < 0 || instance_id > u16::MAX as i32 {
                warn!(
                    "Instance ID {} is out of u16 range, skipping instance {}",
                    instance_id, instance_name
                );
                continue;
            }

            instances.push(Instance {
                core: voltage_config::modsrv::InstanceCore {
                    instance_id: instance_id as u16,
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
    ///
    /// Updates instance_name in SQLite (instances, measurement_routing, action_routing tables).
    /// Caller is responsible for updating Redis after this method succeeds.
    ///
    /// @input instance_id: u16 - Instance ID to rename
    /// @input new_name: &str - New instance name (must be unique)
    /// @output Result<()> - Success or error
    /// @throws anyhow::Error - Database error or name conflict
    pub async fn rename_instance(&self, instance_id: u16, new_name: &str) -> Result<()> {
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
    ///
    /// Queries the database for the maximum existing instance_id and returns the next sequential ID.
    /// Returns 1 if no instances exist yet.
    pub async fn get_next_instance_id(&self) -> Result<u16> {
        let row = sqlx::query_as::<_, (Option<i32>,)>("SELECT MAX(instance_id) FROM instances")
            .fetch_one(&self.pool)
            .await?;

        match row.0 {
            Some(max_id) => {
                let next_id = max_id + 1;
                if next_id > u16::MAX as i32 {
                    anyhow::bail!("Instance ID overflow: exceeded u16::MAX");
                }
                Ok(next_id as u16)
            },
            None => Ok(1), // First instance
        }
    }

    /// Get instance by ID
    pub async fn get_instance(&self, instance_id: u16) -> Result<Instance> {
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
        let measurement_points = sqlx::query_as::<_, (i32,)>(
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
            let redis_key = InstanceRedisKeys::measurement(instance_id, point_id as u32);
            measurement_point_routings.insert(point_id as u32, redis_key);
        }

        // Query action routing
        let action_points = sqlx::query_as::<_, (i32,)>(
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
            let redis_key = InstanceRedisKeys::action(instance_id, point_id as u32);
            action_point_routings.insert(point_id as u32, redis_key);
        }

        Ok(Instance {
            core: voltage_config::modsrv::InstanceCore {
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
    pub async fn delete_instance(&self, instance_id: u16) -> Result<()> {
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

    /// Sync all instances from SQLite to Redis (called on startup)
    pub async fn sync_instances_to_redis(&self) -> Result<()> {
        info!("Syncing instances from SQLite to Redis...");

        let instances = self.list_instances(None).await?;
        let total = instances.len();

        if total == 0 {
            info!("No instances to sync");
            return Ok(());
        }

        // Collect all instance data for batch sync
        struct InstanceRedisPayload {
            instance_id: u16,
            instance_name: String,
            product_name: String,
            properties: HashMap<String, serde_json::Value>,
            // Point routing mappings - Maps point IDs to Redis keys
            measurement_point_routings: HashMap<u32, String>,
            action_point_routings: HashMap<u32, String>,
            measurement_points: Vec<crate::product_loader::MeasurementPoint>,
            action_points: Vec<crate::product_loader::ActionPoint>,
        }

        let mut batch_data: Vec<InstanceRedisPayload> = Vec::new();
        let mut failed_count = 0;

        for instance in instances {
            // Get product details
            let product = match self
                .product_loader
                .get_product(instance.product_name())
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        "Product {} not found for instance {}: {}",
                        instance.product_name(),
                        instance.instance_id(),
                        e
                    );
                    failed_count += 1;
                    continue;
                },
            };

            // Get point routings for this instance
            let full_instance = match self.get_instance(instance.instance_id()).await {
                Ok(inst) => inst,
                Err(e) => {
                    warn!(
                        "Failed to get point routings for instance {} ({}): {}",
                        instance.instance_id(),
                        instance.instance_name(),
                        e
                    );
                    failed_count += 1;
                    continue;
                },
            };

            let measurement_point_routings = full_instance.measurement_mappings.unwrap_or_default();
            let action_point_routings = full_instance.action_mappings.unwrap_or_default();

            batch_data.push(InstanceRedisPayload {
                instance_id: instance.instance_id(),
                instance_name: instance.instance_name().to_string(),
                product_name: instance.product_name().to_string(),
                properties: instance.core.properties.clone(),
                measurement_point_routings: measurement_point_routings.clone(),
                action_point_routings: action_point_routings.clone(),
                measurement_points: product.measurements.clone(),
                action_points: product.actions.clone(),
            });
        }

        // Perform batch sync to Redis if we have data
        if !batch_data.is_empty() {
            info!("Batch syncing {} instances to Redis", batch_data.len());

            // Process in chunks to avoid overwhelming Redis
            const BATCH_SIZE: usize = 50;
            for chunk in batch_data.chunks(BATCH_SIZE) {
                for payload in chunk {
                    if let Err(e) = redis_state::register_instance(
                        self.rtdb.as_ref(),
                        payload.instance_id,
                        &payload.instance_name,
                        &payload.product_name,
                        &payload.properties,
                        &payload.measurement_point_routings,
                        &payload.action_point_routings,
                        &payload.measurement_points,
                        &payload.action_points,
                        None,
                    )
                    .await
                    {
                        warn!(
                            "Failed to sync instance {} to Redis: {}",
                            payload.instance_name, e
                        );
                        failed_count += 1;
                    }
                }

                // Small delay between chunks to avoid overwhelming Redis
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }

        if failed_count > 0 {
            warn!(
                "Instance sync completed with {} failures out of {}",
                failed_count, total
            );
        } else {
            info!("All {} instances synced successfully", total);
        }

        Ok(()) // Always return Ok to allow service to start
    }

    /// Sync a single instance to Redis (for hot reload)
    ///
    /// @input instance: &Instance - Instance configuration to sync
    /// @output Result<()> - Success or error
    /// @redis-write inst:{instance_id}:* - Instance data and routing
    pub async fn sync_single_instance_to_redis(&self, instance: &Instance) -> Result<()> {
        // Get product details
        let product = self
            .product_loader
            .get_product(instance.product_name())
            .await?;

        // Get point routings for this instance
        let full_instance = self.get_instance(instance.instance_id()).await?;

        let measurement_point_routings = full_instance.measurement_mappings.unwrap_or_default();
        let action_point_routings = full_instance.action_mappings.unwrap_or_default();

        // Register instance in Redis
        redis_state::register_instance(
            self.rtdb.as_ref(),
            instance.instance_id(),
            instance.instance_name(),
            instance.product_name(),
            &instance.core.properties,
            &measurement_point_routings,
            &action_point_routings,
            &product.measurements,
            &product.actions,
            None,
        )
        .await?;

        Ok(())
    }

    /// Sync a single instance to Redis using pending transaction data.
    ///
    /// Reads instance metadata and point routings using the provided transaction
    /// so the uncommitted state becomes visible to Redis when we upsert.
    pub async fn sync_instance_to_redis_with_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        instance_name: &str,
        properties: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Fetch instance metadata within the transaction
        let (instance_id, product_name): (i32, String) = sqlx::query_as(
            r#"
            SELECT instance_id, product_name
            FROM instances
            WHERE instance_name = ?
            "#,
        )
        .bind(instance_name)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|e| {
            anyhow!(
                "Failed to load instance {} within transaction: {}",
                instance_name,
                e
            )
        })?;

        // Fetch point routings from routing tables and generate Redis keys dynamically
        let mut measurement_point_routings: HashMap<u32, String> = HashMap::new();
        let mut action_point_routings: HashMap<u32, String> = HashMap::new();

        // Query measurement routing within transaction
        let measurement_points: Vec<(i32,)> = sqlx::query_as(
            r#"
            SELECT measurement_id
            FROM measurement_routing
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| {
            anyhow!(
                "Failed to load measurement routing for {}: {}",
                instance_name,
                e
            )
        })?;

        for (point_id,) in measurement_points {
            let redis_key = InstanceRedisKeys::measurement(instance_id as u16, point_id as u32);
            measurement_point_routings.insert(point_id as u32, redis_key);
        }

        // Query action routing within transaction
        let action_points: Vec<(i32,)> = sqlx::query_as(
            r#"
            SELECT action_id
            FROM action_routing
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| anyhow!("Failed to load action routing for {}: {}", instance_name, e))?;

        for (point_id,) in action_points {
            let redis_key = InstanceRedisKeys::action(instance_id as u16, point_id as u32);
            action_point_routings.insert(point_id as u32, redis_key);
        }

        // Load product definition (cached) to include point metadata
        let product = self.product_loader.get_product(&product_name).await?;

        self.register_instance_in_redis(
            instance_id as u16,
            instance_name,
            &product_name,
            properties,
            &product.measurements,
            &product.actions,
            &measurement_point_routings,
            &action_point_routings,
        )
        .await
    }

    /// Sync instance to Redis without transaction (for error recovery)
    ///
    /// Reads from committed database data and syncs to Redis.
    /// Used when commit fails and we need to revert Redis to the old (committed) state.
    pub async fn sync_instance_to_redis_internal(
        &self,
        instance_name: &str,
        properties: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Fetch instance metadata from committed data (no transaction)
        let (instance_id, product_name): (i32, String) = sqlx::query_as(
            r#"
            SELECT instance_id, product_name
            FROM instances
            WHERE instance_name = ?
            "#,
        )
        .bind(instance_name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            anyhow!(
                "Failed to load instance {} from database: {}",
                instance_name,
                e
            )
        })?;

        // Fetch point routings from routing tables and generate Redis keys dynamically
        let mut measurement_point_routings: HashMap<u32, String> = HashMap::new();
        let mut action_point_routings: HashMap<u32, String> = HashMap::new();

        // Query measurement routing (no transaction)
        let measurement_points: Vec<(i32,)> = sqlx::query_as(
            r#"
            SELECT measurement_id
            FROM measurement_routing
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            anyhow!(
                "Failed to load measurement routing for {}: {}",
                instance_name,
                e
            )
        })?;

        for (point_id,) in measurement_points {
            let redis_key = InstanceRedisKeys::measurement(instance_id as u16, point_id as u32);
            measurement_point_routings.insert(point_id as u32, redis_key);
        }

        // Query action routing (no transaction)
        let action_points: Vec<(i32,)> = sqlx::query_as(
            r#"
            SELECT action_id
            FROM action_routing
            WHERE instance_id = ?
            "#,
        )
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to load action routing for {}: {}", instance_name, e))?;

        for (point_id,) in action_points {
            let redis_key = InstanceRedisKeys::action(instance_id as u16, point_id as u32);
            action_point_routings.insert(point_id as u32, redis_key);
        }

        // Load product definition (cached) to include point metadata
        let product = self.product_loader.get_product(&product_name).await?;

        self.register_instance_in_redis(
            instance_id as u16,
            instance_name,
            &product_name,
            properties,
            &product.measurements,
            &product.actions,
            &measurement_point_routings,
            &action_point_routings,
        )
        .await
    }

    /// Register instance metadata and point routing mappings to Redis
    #[allow(clippy::too_many_arguments)]
    async fn register_instance_in_redis(
        &self,
        instance_id: u16,
        instance_name: &str,
        product_name: &str,
        properties: &HashMap<String, serde_json::Value>,
        measurements: &[crate::product_loader::MeasurementPoint],
        actions: &[crate::product_loader::ActionPoint],
        measurement_point_routings: &HashMap<u32, String>,
        action_point_routings: &HashMap<u32, String>,
    ) -> Result<()> {
        redis_state::register_instance(
            self.rtdb.as_ref(),
            instance_id,
            instance_name,
            product_name,
            properties,
            measurement_point_routings,
            action_point_routings,
            measurements,
            actions,
            None,
        )
        .await?;

        info!("Registered instance {} in Redis", instance_name);
        Ok(())
    }

    /// Unregister instance from Redis
    async fn unregister_instance_from_redis(
        &self,
        instance_id: u16,
        instance_name: &str,
    ) -> Result<()> {
        redis_state::unregister_instance(self.rtdb.as_ref(), instance_id, instance_name).await?;

        debug!("Unregistered instance {} from Redis", instance_name);
        Ok(())
    }

    /// Get instance real-time data from Redis
    pub async fn get_instance_data(
        &self,
        instance_id: u16,
        data_type: Option<&str>,
    ) -> Result<serde_json::Value> {
        let data =
            redis_state::get_instance_data(self.rtdb.as_ref(), instance_id, data_type).await?;
        Ok(data)
    }

    /// Get instance point definitions from Redis (metadata, not real-time values)
    /// Load instance points with routing configuration (runtime merge)
    ///
    /// This method performs a JOIN query to combine:
    /// - Product point templates (from measurement_points/action_points tables)
    /// - Instance-specific routing (from measurement_routing/action_routing tables)
    ///
    /// @input instance_id: u16 - Instance identifier
    /// @output Result<(Vec<InstanceMeasurementPoint>, Vec<InstanceActionPoint>)> - Points with routing
    /// @throws anyhow::Error - Instance not found, database error
    /// @performance O(n) where n = number of points, single JOIN query per point type
    pub async fn load_instance_points(
        &self,
        instance_id: u16,
    ) -> Result<(
        Vec<crate::dto::InstanceMeasurementPoint>,
        Vec<crate::dto::InstanceActionPoint>,
    )> {
        use crate::dto::{InstanceActionPoint, InstanceMeasurementPoint, PointRouting};

        // 1. Get product_name from instance
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // 2. JOIN query for measurement points (Product template + Instance routing)
        // Also JOIN channels and point tables to get display names
        let measurements = sqlx::query_as::<
            _,
            (
                u32,
                String,
                Option<String>,
                Option<String>, // Point fields: measurement_id, name, unit, description
                Option<i32>,
                Option<String>,
                Option<u32>,
                Option<bool>, // Routing fields: channel_id, channel_type, channel_point_id, enabled
                Option<String>, // channel_name (from channels table)
                Option<String>, // channel_point_name (from telemetry_points/signal_points)
            ),
        >(
            r#"
            SELECT
                mp.measurement_id,
                mp.name,
                mp.unit,
                mp.description,
                mr.channel_id,
                mr.channel_type,
                mr.channel_point_id,
                mr.enabled,
                c.name AS channel_name,
                COALESCE(tp.signal_name, sp.signal_name) AS channel_point_name
            FROM measurement_points mp
            LEFT JOIN measurement_routing mr
                ON mr.instance_id = ? AND mr.measurement_id = mp.measurement_id
            LEFT JOIN channels c ON c.channel_id = mr.channel_id
            LEFT JOIN telemetry_points tp
                ON tp.channel_id = mr.channel_id
                AND tp.point_id = mr.channel_point_id
                AND mr.channel_type = 'T'
            LEFT JOIN signal_points sp
                ON sp.channel_id = mr.channel_id
                AND sp.point_id = mr.channel_point_id
                AND mr.channel_type = 'S'
            WHERE mp.product_name = ?
            ORDER BY mp.measurement_id
            "#,
        )
        .bind(instance_id as i32)
        .bind(&product_name)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(
            |(idx, name, unit, desc, cid, ctype, cpid, enabled, cname, cpname)| {
                InstanceMeasurementPoint {
                    measurement_id: idx,
                    name,
                    unit,
                    description: desc,
                    routing: match (ctype, enabled) {
                        (Some(t), Some(e)) => Some(PointRouting {
                            channel_id: cid,
                            channel_type: Some(t),
                            channel_point_id: cpid,
                            enabled: e,
                            channel_name: cname,
                            channel_point_name: cpname,
                        }),
                        _ => None,
                    },
                }
            },
        )
        .collect();

        // 3. JOIN query for action points (Product template + Instance routing)
        // Also JOIN channels and point tables to get display names
        let actions = sqlx::query_as::<
            _,
            (
                u32,
                String,
                Option<String>,
                Option<String>, // Point fields: action_id, name, unit, description
                Option<i32>,
                Option<String>,
                Option<u32>,
                Option<bool>, // Routing fields: channel_id, channel_type, channel_point_id, enabled
                Option<String>, // channel_name (from channels table)
                Option<String>, // channel_point_name (from control_points/adjustment_points)
            ),
        >(
            r#"
            SELECT
                ap.action_id,
                ap.name,
                ap.unit,
                ap.description,
                ar.channel_id,
                ar.channel_type,
                ar.channel_point_id,
                ar.enabled,
                c.name AS channel_name,
                COALESCE(cp.signal_name, ajp.signal_name) AS channel_point_name
            FROM action_points ap
            LEFT JOIN action_routing ar
                ON ar.instance_id = ? AND ar.action_id = ap.action_id
            LEFT JOIN channels c ON c.channel_id = ar.channel_id
            LEFT JOIN control_points cp
                ON cp.channel_id = ar.channel_id
                AND cp.point_id = ar.channel_point_id
                AND ar.channel_type = 'C'
            LEFT JOIN adjustment_points ajp
                ON ajp.channel_id = ar.channel_id
                AND ajp.point_id = ar.channel_point_id
                AND ar.channel_type = 'A'
            WHERE ap.product_name = ?
            ORDER BY ap.action_id
            "#,
        )
        .bind(instance_id as i32)
        .bind(&product_name)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(
            |(idx, name, unit, desc, cid, ctype, cpid, enabled, cname, cpname)| {
                InstanceActionPoint {
                    action_id: idx,
                    name,
                    unit,
                    description: desc,
                    routing: match (ctype, enabled) {
                        (Some(t), Some(e)) => Some(PointRouting {
                            channel_id: cid,
                            channel_type: Some(t),
                            channel_point_id: cpid,
                            enabled: e,
                            channel_name: cname,
                            channel_point_name: cpname,
                        }),
                        _ => None,
                    },
                }
            },
        )
        .collect();

        Ok((measurements, actions))
    }

    pub async fn get_instance_points(
        &self,
        instance_id: u16,
        data_type: Option<&str>,
    ) -> Result<serde_json::Value> {
        // ========================================================================
        // SQLite = Single source of truth (Redis = real-time data only)
        // Query point definitions directly from SQLite instead of Redis cache
        // ========================================================================

        // Get instance metadata (product_name, properties)
        let instance_row: (String, String) =
            sqlx::query_as("SELECT product_name, properties FROM instances WHERE instance_id = ?")
                .bind(instance_id as i32)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| anyhow!("Instance {} not found", instance_id))?;

        let (product_name, properties_json) = instance_row;

        match data_type {
            Some("measurement") => {
                // Query measurement points from SQLite
                let measurements: Vec<(String, String, f64, f64, String)> = sqlx::query_as(
                    "SELECT signal_name, data_type, scale, offset, unit
                     FROM measurement_points WHERE product_name = ?",
                )
                .bind(&product_name)
                .fetch_all(&self.pool)
                .await?;

                let mut result = serde_json::Map::new();
                for (signal_name, data_type, scale, offset, unit) in measurements {
                    let point = serde_json::json!({
                        "signal_name": signal_name,
                        "data_type": data_type,
                        "scale": scale,
                        "offset": offset,
                        "unit": unit
                    });
                    result.insert(signal_name.clone(), point);
                }

                Ok(serde_json::Value::Object(result))
            },
            Some("action") => {
                // Query action points from SQLite
                let actions: Vec<(String, String, f64, f64, String)> = sqlx::query_as(
                    "SELECT signal_name, data_type, scale, offset, unit
                     FROM action_points WHERE product_name = ?",
                )
                .bind(&product_name)
                .fetch_all(&self.pool)
                .await?;

                let mut result = serde_json::Map::new();
                for (signal_name, data_type, scale, offset, unit) in actions {
                    let point = serde_json::json!({
                        "signal_name": signal_name,
                        "data_type": data_type,
                        "scale": scale,
                        "offset": offset,
                        "unit": unit
                    });
                    result.insert(signal_name.clone(), point);
                }

                Ok(serde_json::Value::Object(result))
            },
            Some("property") => {
                // Return instance properties (stored as JSON in instances table)
                let properties: serde_json::Value =
                    serde_json::from_str(&properties_json).unwrap_or(serde_json::json!({}));
                Ok(properties)
            },
            None => {
                // Return all three: measurements, actions, properties
                let measurements: Vec<(String, String, f64, f64, String)> = sqlx::query_as(
                    "SELECT signal_name, data_type, scale, offset, unit
                     FROM measurement_points WHERE product_name = ?",
                )
                .bind(&product_name)
                .fetch_all(&self.pool)
                .await?;

                let actions: Vec<(String, String, f64, f64, String)> = sqlx::query_as(
                    "SELECT signal_name, data_type, scale, offset, unit
                     FROM action_points WHERE product_name = ?",
                )
                .bind(&product_name)
                .fetch_all(&self.pool)
                .await?;

                let mut m_map = serde_json::Map::new();
                for (signal_name, data_type, scale, offset, unit) in measurements {
                    let point = serde_json::json!({
                        "signal_name": signal_name,
                        "data_type": data_type,
                        "scale": scale,
                        "offset": offset,
                        "unit": unit
                    });
                    m_map.insert(signal_name.clone(), point);
                }

                let mut a_map = serde_json::Map::new();
                for (signal_name, data_type, scale, offset, unit) in actions {
                    let point = serde_json::json!({
                        "signal_name": signal_name,
                        "data_type": data_type,
                        "scale": scale,
                        "offset": offset,
                        "unit": unit
                    });
                    a_map.insert(signal_name.clone(), point);
                }

                let properties: serde_json::Value =
                    serde_json::from_str(&properties_json).unwrap_or(serde_json::json!({}));

                Ok(serde_json::json!({
                    "measurements": m_map,
                    "actions": a_map,
                    "properties": properties
                }))
            },
            Some(other) => Err(anyhow!(
                "Unknown data type '{}'; use 'measurement', 'action', 'property', or omit for all",
                other
            )),
        }
    }

    /// Sync measurement data to instance
    pub async fn sync_measurement(
        &self,
        instance_id: u16,
        data: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        redis_state::sync_measurement(self.rtdb.as_ref(), instance_id, &data).await?;

        debug!("Synced measurement data for instance {}", instance_id);
        Ok(())
    }

    /// Execute action on instance
    pub async fn execute_action(
        &self,
        instance_id: u16,
        action_id: &str,
        value: f64,
    ) -> Result<()> {
        // Query instance_name for voltage-routing library compatibility
        // voltage-routing uses instance_name for Redis Hash key lookups (inst:name:index)
        let instance_name: String =
            sqlx::query_scalar("SELECT instance_name FROM instances WHERE instance_id = ?")
                .bind(instance_id as i32)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // Use application-layer routing with cache
        let outcome = voltage_routing::set_action_point(
            self.rtdb.as_ref(),
            &self.routing_cache,
            &instance_name,
            action_id,
            value,
        )
        .await?;

        if outcome.routed {
            debug!(
                "Action {} routed to channel {} for instance {} ({})",
                action_id,
                outcome
                    .route_result
                    .as_deref()
                    .unwrap_or("<unknown_channel>"),
                instance_id,
                instance_name
            );
        } else {
            debug!(
                "Action {} stored but not routed for instance {} ({}) - {}",
                action_id,
                instance_id,
                instance_name,
                outcome
                    .route_result
                    .as_deref()
                    .unwrap_or("<no_route_reason>")
            );
        }

        Ok(())
    }

    /// Load a single measurement point with routing configuration
    pub async fn load_single_measurement_point(
        &self,
        instance_id: u16,
        point_id: u32,
    ) -> Result<crate::dto::InstanceMeasurementPoint> {
        use crate::dto::{InstanceMeasurementPoint, PointRouting};

        // 1. Get product_name
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // 2. JOIN query for the specific measurement point (with channel and point names)
        let point = sqlx::query_as::<
            _,
            (
                u32,
                String,
                Option<String>,
                Option<String>, // Point fields
                Option<i32>,
                Option<String>,
                Option<u32>,
                Option<bool>,   // Routing fields
                Option<String>, // channel_name
                Option<String>, // channel_point_name
            ),
        >(
            r#"
            SELECT
                mp.measurement_id,
                mp.name,
                mp.unit,
                mp.description,
                mr.channel_id,
                mr.channel_type,
                mr.channel_point_id,
                mr.enabled,
                c.name AS channel_name,
                COALESCE(tp.signal_name, sp.signal_name) AS channel_point_name
            FROM measurement_points mp
            LEFT JOIN measurement_routing mr
                ON mr.instance_id = ? AND mr.measurement_id = mp.measurement_id
            LEFT JOIN channels c ON c.channel_id = mr.channel_id
            LEFT JOIN telemetry_points tp
                ON tp.channel_id = mr.channel_id
                AND tp.point_id = mr.channel_point_id
                AND mr.channel_type = 'T'
            LEFT JOIN signal_points sp
                ON sp.channel_id = mr.channel_id
                AND sp.point_id = mr.channel_point_id
                AND mr.channel_type = 'S'
            WHERE mp.product_name = ? AND mp.measurement_id = ?
            "#,
        )
        .bind(instance_id as i32)
        .bind(&product_name)
        .bind(point_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            anyhow!(
                "Measurement point {} not found for instance {}: {}",
                point_id,
                instance_id,
                e
            )
        })?;

        let (idx, name, unit, desc, cid, ctype, cpid, enabled, cname, cpname) = point;

        Ok(InstanceMeasurementPoint {
            measurement_id: idx,
            name,
            unit,
            description: desc,
            routing: match (ctype, enabled) {
                (Some(t), Some(e)) => Some(PointRouting {
                    channel_id: cid,
                    channel_type: Some(t),
                    channel_point_id: cpid,
                    enabled: e,
                    channel_name: cname,
                    channel_point_name: cpname,
                }),
                _ => None,
            },
        })
    }

    /// Load a single action point with routing configuration
    pub async fn load_single_action_point(
        &self,
        instance_id: u16,
        point_id: u32,
    ) -> Result<crate::dto::InstanceActionPoint> {
        use crate::dto::{InstanceActionPoint, PointRouting};

        // 1. Get product_name
        let product_name = sqlx::query_scalar::<_, String>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // 2. JOIN query for the specific action point (with channel and point names)
        let point = sqlx::query_as::<
            _,
            (
                u32,
                String,
                Option<String>,
                Option<String>, // Point fields
                Option<i32>,
                Option<String>,
                Option<u32>,
                Option<bool>,   // Routing fields
                Option<String>, // channel_name
                Option<String>, // channel_point_name
            ),
        >(
            r#"
            SELECT
                ap.action_id,
                ap.name,
                ap.unit,
                ap.description,
                ar.channel_id,
                ar.channel_type,
                ar.channel_point_id,
                ar.enabled,
                c.name AS channel_name,
                COALESCE(cp.signal_name, ajp.signal_name) AS channel_point_name
            FROM action_points ap
            LEFT JOIN action_routing ar
                ON ar.instance_id = ? AND ar.action_id = ap.action_id
            LEFT JOIN channels c ON c.channel_id = ar.channel_id
            LEFT JOIN control_points cp
                ON cp.channel_id = ar.channel_id
                AND cp.point_id = ar.channel_point_id
                AND ar.channel_type = 'C'
            LEFT JOIN adjustment_points ajp
                ON ajp.channel_id = ar.channel_id
                AND ajp.point_id = ar.channel_point_id
                AND ar.channel_type = 'A'
            WHERE ap.product_name = ? AND ap.action_id = ?
            "#,
        )
        .bind(instance_id as i32)
        .bind(&product_name)
        .bind(point_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            anyhow!(
                "Action point {} not found for instance {}: {}",
                point_id,
                instance_id,
                e
            )
        })?;

        let (idx, name, unit, desc, cid, ctype, cpid, enabled, cname, cpname) = point;

        Ok(InstanceActionPoint {
            action_id: idx,
            name,
            unit,
            description: desc,
            routing: match (ctype, enabled) {
                (Some(t), Some(e)) => Some(PointRouting {
                    channel_id: cid,
                    channel_type: Some(t),
                    channel_point_id: cpid,
                    enabled: e,
                    channel_name: cname,
                    channel_point_name: cpname,
                }),
                _ => None,
            },
        })
    }

    /// Create or update routing for a single measurement point (UPSERT)
    pub async fn upsert_measurement_routing(
        &self,
        instance_id: u16,
        point_id: u32,
        request: crate::dto::SinglePointRoutingRequest,
    ) -> Result<()> {
        // Validate channel_type (must be T or S for measurement, skip if None - unbound)
        if let Some(ref fr) = request.four_remote {
            if !fr.is_input() {
                return Err(anyhow!(
                    "Invalid channel_type '{}' for measurement routing (must be T or S)",
                    fr
                ));
            }
        }

        // Get instance_name for routing table denormalization
        let instance_name = sqlx::query_scalar::<_, String>(
            "SELECT instance_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // UPSERT into measurement_routing
        sqlx::query(
            r#"
            INSERT INTO measurement_routing
            (instance_id, instance_name, channel_id, channel_type, channel_point_id,
             measurement_id, enabled)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(instance_id, measurement_id)
            DO UPDATE SET
                channel_id = excluded.channel_id,
                channel_type = excluded.channel_type,
                channel_point_id = excluded.channel_point_id,
                enabled = excluded.enabled,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(instance_id as i32)
        .bind(instance_name)
        .bind(request.channel_id)
        .bind(request.four_remote.map(|fr| fr.as_str()))
        .bind(request.channel_point_id)
        .bind(point_id)
        .bind(request.enabled)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Create or update routing for a single action point (UPSERT)
    pub async fn upsert_action_routing(
        &self,
        instance_id: u16,
        point_id: u32,
        request: crate::dto::SinglePointRoutingRequest,
    ) -> Result<()> {
        // Validate channel_type (must be C or A for action, skip if None - unbound)
        if let Some(ref fr) = request.four_remote {
            if !fr.is_output() {
                return Err(anyhow!(
                    "Invalid channel_type '{}' for action routing (must be C or A)",
                    fr
                ));
            }
        }

        // Get instance_name for routing table denormalization
        let instance_name = sqlx::query_scalar::<_, String>(
            "SELECT instance_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!("Instance {} not found: {}", instance_id, e))?;

        // UPSERT into action_routing
        sqlx::query(
            r#"
            INSERT INTO action_routing
            (instance_id, instance_name, action_id, channel_id, channel_type,
             channel_point_id, enabled)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(instance_id, action_id)
            DO UPDATE SET
                channel_id = excluded.channel_id,
                channel_type = excluded.channel_type,
                channel_point_id = excluded.channel_point_id,
                enabled = excluded.enabled,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(instance_id as i32)
        .bind(instance_name)
        .bind(point_id)
        .bind(request.channel_id)
        .bind(request.four_remote.map(|fr| fr.as_str()))
        .bind(request.channel_point_id)
        .bind(request.enabled)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete routing for a single measurement point
    pub async fn delete_measurement_routing(&self, instance_id: u16, point_id: u32) -> Result<u64> {
        // Delete from measurement_routing
        let result = sqlx::query(
            "DELETE FROM measurement_routing WHERE instance_id = ? AND measurement_id = ?",
        )
        .bind(instance_id as i32)
        .bind(point_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Delete routing for a single action point
    pub async fn delete_action_routing(&self, instance_id: u16, point_id: u32) -> Result<u64> {
        // Delete from action_routing
        let result =
            sqlx::query("DELETE FROM action_routing WHERE instance_id = ? AND action_id = ?")
                .bind(instance_id as i32)
                .bind(point_id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected())
    }

    /// Toggle enabled state for a single measurement point routing
    pub async fn toggle_measurement_routing(
        &self,
        instance_id: u16,
        point_id: u32,
        enabled: bool,
    ) -> Result<u64> {
        // Update enabled field
        let result = sqlx::query(
            r#"
            UPDATE measurement_routing
            SET enabled = ?, updated_at = CURRENT_TIMESTAMP
            WHERE instance_id = ? AND measurement_id = ?
            "#,
        )
        .bind(enabled)
        .bind(instance_id as i32)
        .bind(point_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Toggle enabled state for a single action point routing
    pub async fn toggle_action_routing(
        &self,
        instance_id: u16,
        point_id: u32,
        enabled: bool,
    ) -> Result<u64> {
        // Update enabled field
        let result = sqlx::query(
            r#"
            UPDATE action_routing
            SET enabled = ?, updated_at = CURRENT_TIMESTAMP
            WHERE instance_id = ? AND action_id = ?
            "#,
        )
        .bind(enabled)
        .bind(instance_id as i32)
        .bind(point_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get all measurement routing for an instance
    ///
    /// Retrieves all enabled measurement routing entries for the specified instance.
    ///
    /// @input instance_id: u16 - Instance ID
    /// @output Result<Vec<MeasurementRouting>> - List of measurement routing entries
    /// @throws anyhow::Error - Database query error
    pub async fn get_measurement_routing(
        &self,
        instance_id: u16,
    ) -> Result<Vec<MeasurementRouting>> {
        let routing = sqlx::query_as::<_, MeasurementRouting>(
            r#"
            SELECT * FROM measurement_routing
            WHERE instance_id = ? AND enabled = TRUE
            ORDER BY channel_id, channel_type, channel_point_id
            "#,
        )
        .bind(instance_id as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(routing)
    }

    /// Get all action routing for an instance
    ///
    /// Retrieves all enabled action routing entries for the specified instance.
    ///
    /// @input instance_id: u16 - Instance ID
    /// @output Result<Vec<ActionRouting>> - List of action routing entries
    /// @throws anyhow::Error - Database query error
    pub async fn get_action_routing(&self, instance_id: u16) -> Result<Vec<ActionRouting>> {
        let routing = sqlx::query_as::<_, ActionRouting>(
            r#"
            SELECT * FROM action_routing
            WHERE instance_id = ? AND enabled = TRUE
            ORDER BY action_id
            "#,
        )
        .bind(instance_id as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(routing)
    }

    /// Validate a measurement routing entry
    ///
    /// Checks if a measurement routing configuration is valid by verifying:
    /// - Instance exists
    /// - Channel type is input (T or S)
    /// - Measurement point exists for the instance's product
    ///
    /// @input routing: &MeasurementRoutingRow - Routing configuration to validate
    /// @input instance_name: &str - Instance name
    /// @output Result<ValidationResult> - Validation result with errors if any
    /// @throws anyhow::Error - Database query error
    pub async fn validate_measurement_routing(
        &self,
        routing: &MeasurementRoutingRow,
        instance_name: &str,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Validate instance exists
        let instance_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_name = ?)",
        )
        .bind(instance_name)
        .fetch_one(&self.pool)
        .await?;

        if !instance_exists {
            errors.push(format!("Instance {} does not exist", instance_name));
        }

        // Validate channel_type (skip if None - unbound routing is valid)
        if let Some(ref ct) = routing.channel_type {
            if !ct.is_input() {
                errors.push(format!(
                    "Invalid channel_type for measurement: {}. Must be T or S",
                    ct
                ));
            }
        }

        // Validate measurement point exists
        let point_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM measurement_points mp
                JOIN instances i ON i.product_name = mp.product_name
                WHERE i.instance_name = ? AND mp.measurement_id = ?
            )
            "#,
        )
        .bind(instance_name)
        .bind(routing.measurement_id)
        .fetch_one(&self.pool)
        .await?;

        if !point_exists {
            errors.push(format!(
                "Measurement point {} not found for instance {}",
                routing.measurement_id, instance_name
            ));
        }

        let mut result = ValidationResult::new(ValidationLevel::Business);
        for error in errors {
            result.add_error(error);
        }
        Ok(result)
    }

    /// Validate an action routing entry
    ///
    /// Checks if an action routing configuration is valid by verifying:
    /// - Instance exists
    /// - Channel type is output (C or A)
    /// - Action point exists for the instance's product
    ///
    /// @input routing: &ActionRoutingRow - Routing configuration to validate
    /// @input instance_name: &str - Instance name
    /// @output Result<ValidationResult> - Validation result with errors if any
    /// @throws anyhow::Error - Database query error
    pub async fn validate_action_routing(
        &self,
        routing: &ActionRoutingRow,
        instance_name: &str,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Validate instance exists
        let instance_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_name = ?)",
        )
        .bind(instance_name)
        .fetch_one(&self.pool)
        .await?;

        if !instance_exists {
            errors.push(format!("Instance {} does not exist", instance_name));
        }

        // Validate channel_type (skip if None - unbound routing is valid)
        if let Some(ref ct) = routing.channel_type {
            if !ct.is_output() {
                errors.push(format!(
                    "Invalid channel_type for action: {}. Must be C or A",
                    ct
                ));
            }
        }

        // Validate action point exists
        let point_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_points ap
                JOIN instances i ON i.product_name = ap.product_name
                WHERE i.instance_name = ? AND ap.action_id = ?
            )
            "#,
        )
        .bind(instance_name)
        .bind(routing.action_id)
        .fetch_one(&self.pool)
        .await?;

        if !point_exists {
            errors.push(format!(
                "Action point {} not found for instance {}",
                routing.action_id, instance_name
            ));
        }

        let mut result = ValidationResult::new(ValidationLevel::Business);
        for error in errors {
            result.add_error(error);
        }
        Ok(result)
    }

    /// Delete all routing for an instance
    ///
    /// Removes all measurement and action routing entries for the specified instance.
    ///
    /// @input instance_id: u16 - Instance ID
    /// @output Result<(u64, u64)> - Tuple of (measurement_deleted, action_deleted) counts
    /// @throws anyhow::Error - Database query error
    pub async fn delete_all_routing(&self, instance_id: u16) -> Result<(u64, u64)> {
        let measurement_result =
            sqlx::query("DELETE FROM measurement_routing WHERE instance_id = ?")
                .bind(instance_id as i32)
                .execute(&self.pool)
                .await?;

        let action_result = sqlx::query("DELETE FROM action_routing WHERE instance_id = ?")
            .bind(instance_id as i32)
            .execute(&self.pool)
            .await?;

        Ok((
            measurement_result.rows_affected(),
            action_result.rows_affected(),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;
    // use voltage_config::modsrv::{ActionPoint, MeasurementPoint, PointType};

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
        Arc::new(crate::product_loader::ProductLoader::new(
            "config/modsrv/products",
            pool,
        ))
    }

    // Helper: Create in-memory RTDB for testing (no Redis dependency)
    fn create_test_rtdb() -> Arc<voltage_rtdb::MemoryRtdb> {
        Arc::new(voltage_rtdb::MemoryRtdb::new())
    }

    // ==================== Phase 1: CRUD Core Tests ====================

    #[tokio::test]
    async fn test_instance_manager_new() {
        let (_temp_dir, pool) = create_test_database().await;
        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();

        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
        let _manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        // Test passes if InstanceManager::new() doesn't panic
    }

    #[tokio::test]
    async fn test_create_instance_success() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        instance_id: u16,
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
        let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

        // Try to execute action on non-existent instance
        let result = manager.execute_action(9999, "1", 100.0).await;

        assert!(
            result.is_err(),
            "execute_action should fail for non-existent instance"
        );
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_execute_action_no_route_stores_locally() {
        let (_temp_dir, pool) = create_test_database().await;
        create_test_product(&pool, "test_product").await;

        let product_loader = create_test_product_loader(pool.clone());
        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let config = voltage_config::KeySpaceConfig::production();
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::from_maps(
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
        let config = voltage_config::KeySpaceConfig::production();
        let action_key = config.instance_action_key(1001);
        let stored = rtdb.hash_get(&action_key, "1").await.unwrap();
        assert!(stored.is_some(), "Instance action should be stored");
        assert_eq!(stored.unwrap().as_ref(), b"75");

        // Verify channel hash was updated
        use voltage_config::protocols::PointType;
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::from_maps(
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
        let config = voltage_config::KeySpaceConfig::production();
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let config = voltage_config::KeySpaceConfig::production();
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
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());
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
        let config = voltage_config::KeySpaceConfig::production();
        let action_key = config.instance_action_key(1001);
        let stored = rtdb.hash_get(&action_key, "1").await.unwrap().unwrap();
        assert_eq!(stored.as_ref(), b"-50.5");
    }
}
