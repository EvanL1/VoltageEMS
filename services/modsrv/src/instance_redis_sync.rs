//! Instance Redis Synchronization
//!
//! This module provides Redis sync operations for instances.
//! Extracted from instance_manager.rs for better code organization.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::config::InstanceRedisKeys;
use crate::redis_state;

use super::instance_manager::InstanceManager;
use voltage_rtdb::Rtdb;

impl<R: Rtdb + 'static> InstanceManager<R> {
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
            instance_id: u32,
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
    pub async fn sync_single_instance_to_redis(
        &self,
        instance: &crate::product_loader::Instance,
    ) -> Result<()> {
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
        let (instance_id, product_name): (u32, String) = sqlx::query_as(
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
        let measurement_points: Vec<(u32,)> = sqlx::query_as(
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
            let redis_key = InstanceRedisKeys::measurement(instance_id, point_id);
            measurement_point_routings.insert(point_id, redis_key);
        }

        // Query action routing within transaction
        let action_points: Vec<(u32,)> = sqlx::query_as(
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
            let redis_key = InstanceRedisKeys::action(instance_id, point_id);
            action_point_routings.insert(point_id, redis_key);
        }

        // Load product definition (cached) to include point metadata
        let product = self.product_loader.get_product(&product_name).await?;

        self.register_instance_in_redis(
            instance_id,
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
        let (instance_id, product_name): (u32, String) = sqlx::query_as(
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
        let measurement_points: Vec<(u32,)> = sqlx::query_as(
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
            let redis_key = InstanceRedisKeys::measurement(instance_id, point_id);
            measurement_point_routings.insert(point_id, redis_key);
        }

        // Query action routing (no transaction)
        let action_points: Vec<(u32,)> = sqlx::query_as(
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
            let redis_key = InstanceRedisKeys::action(instance_id, point_id);
            action_point_routings.insert(point_id, redis_key);
        }

        // Load product definition (cached) to include point metadata
        let product = self.product_loader.get_product(&product_name).await?;

        self.register_instance_in_redis(
            instance_id,
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
    pub(crate) async fn register_instance_in_redis(
        &self,
        instance_id: u32,
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
    pub(crate) async fn unregister_instance_from_redis(
        &self,
        instance_id: u32,
        instance_name: &str,
    ) -> Result<()> {
        redis_state::unregister_instance(self.rtdb.as_ref(), instance_id, instance_name).await?;

        debug!("Unregistered instance {} from Redis", instance_name);
        Ok(())
    }
}
