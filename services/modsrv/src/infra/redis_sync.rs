//! Instance Redis Synchronization Infrastructure
//!
//! Provides the trait and implementation for syncing instance state
//! between SQLite and Redis. Redis serves as the real-time data store
//! while SQLite is the source of truth.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};
use voltage_model::KeySpaceConfig;
use voltage_rtdb::Rtdb;

use crate::product_loader::{ActionPoint, Instance, MeasurementPoint, ProductLoader};
use crate::redis_state;

/// Trait for Redis instance synchronization
///
/// Abstracts Redis sync operations so they can be mocked in tests.
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait InstanceRedisSync: Send + Sync {
    /// Register instance metadata and point routing mappings in Redis
    async fn register_instance(
        &self,
        instance_id: u32,
        instance_name: &str,
        product_name: &str,
        properties: &HashMap<String, serde_json::Value>,
        measurements: &[MeasurementPoint],
        actions: &[ActionPoint],
        measurement_point_routings: &HashMap<u32, String>,
        action_point_routings: &HashMap<u32, String>,
    ) -> Result<()>;

    /// Remove instance from Redis
    async fn unregister_instance(&self, instance_id: u32, instance_name: &str) -> Result<()>;

    /// Sync all instances from SQLite to Redis (startup batch)
    async fn sync_all(
        &self,
        pool: &SqlitePool,
        product_loader: &ProductLoader,
        instances: Vec<Instance>,
        all_measurement_routings: HashMap<u32, HashMap<u32, String>>,
        all_action_routings: HashMap<u32, HashMap<u32, String>>,
    ) -> Result<()>;
}

/// Production Redis sync implementation using RedisRtdb
pub struct RedisInstanceSync<R: Rtdb> {
    rtdb: Arc<R>,
}

impl<R: Rtdb> RedisInstanceSync<R> {
    pub fn new(rtdb: Arc<R>) -> Self {
        Self { rtdb }
    }
}

#[async_trait]
impl<R: Rtdb + 'static> InstanceRedisSync for RedisInstanceSync<R> {
    async fn register_instance(
        &self,
        instance_id: u32,
        instance_name: &str,
        product_name: &str,
        properties: &HashMap<String, serde_json::Value>,
        measurements: &[MeasurementPoint],
        actions: &[ActionPoint],
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

    async fn unregister_instance(&self, instance_id: u32, instance_name: &str) -> Result<()> {
        redis_state::unregister_instance(self.rtdb.as_ref(), instance_id, instance_name).await?;
        debug!("Unregistered instance {} from Redis", instance_name);
        Ok(())
    }

    async fn sync_all(
        &self,
        _pool: &SqlitePool,
        product_loader: &ProductLoader,
        instances: Vec<Instance>,
        all_measurement_routings: HashMap<u32, HashMap<u32, String>>,
        all_action_routings: HashMap<u32, HashMap<u32, String>>,
    ) -> Result<()> {
        let total = instances.len();
        if total == 0 {
            info!("No instances to sync");
            return Ok(());
        }

        info!("Batch syncing {} instances to Redis", total);

        // Cache products by name to avoid repeated loads
        let mut product_cache: HashMap<String, Arc<crate::product_loader::Product>> =
            HashMap::new();
        let mut failed_count = 0;

        struct Payload {
            instance_id: u32,
            instance_name: String,
            product_name: String,
            properties: HashMap<String, serde_json::Value>,
            measurement_point_routings: HashMap<u32, String>,
            action_point_routings: HashMap<u32, String>,
            product: Arc<crate::product_loader::Product>,
        }

        let mut batch_data: Vec<Payload> = Vec::new();

        for instance in instances {
            let product = match product_cache.get(instance.product_name()) {
                Some(cached) => Arc::clone(cached),
                None => match product_loader.get_product(instance.product_name()) {
                    Ok(p) => {
                        let arc_product = Arc::new(p);
                        product_cache.insert(
                            instance.product_name().to_string(),
                            Arc::clone(&arc_product),
                        );
                        arc_product
                    },
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
                },
            };

            let measurement_point_routings = all_measurement_routings
                .get(&instance.instance_id())
                .cloned()
                .unwrap_or_default();
            let action_point_routings = all_action_routings
                .get(&instance.instance_id())
                .cloned()
                .unwrap_or_default();

            batch_data.push(Payload {
                instance_id: instance.instance_id(),
                instance_name: instance.instance_name().to_string(),
                product_name: instance.product_name().to_string(),
                properties: instance.core.properties.clone(),
                measurement_point_routings,
                action_point_routings,
                product,
            });
        }

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
                    &payload.product.measurements,
                    &payload.product.actions,
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
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        if failed_count > 0 {
            warn!(
                "Instance sync completed with {} failures out of {}",
                failed_count, total
            );
        } else {
            info!("All {} instances synced successfully", total);
        }

        Ok(())
    }
}

/// Fetch measurement and action routing maps for a single instance,
/// generating Redis keys for each point.
///
/// This is a utility function used during per-instance Redis sync.
pub async fn fetch_routing_maps(
    conn: &mut sqlx::SqliteConnection,
    instance_id: u32,
    instance_name: &str,
) -> Result<(HashMap<u32, String>, HashMap<u32, String>)> {
    let ks = KeySpaceConfig::production_cached();

    let measurement_points: Vec<(u32,)> =
        sqlx::query_as("SELECT measurement_id FROM measurement_routing WHERE instance_id = ?")
            .bind(instance_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to load measurement routing for {}: {}",
                    instance_name,
                    e
                )
            })?;

    let measurement_map: HashMap<u32, String> = measurement_points
        .into_iter()
        .map(|(id,)| {
            let key = ks.instance_measurement_point_key(instance_id, &id.to_string());
            (id, key)
        })
        .collect();

    let action_points: Vec<(u32,)> =
        sqlx::query_as("SELECT action_id FROM action_routing WHERE instance_id = ?")
            .bind(instance_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| anyhow!("Failed to load action routing for {}: {}", instance_name, e))?;

    let action_map: HashMap<u32, String> = action_points
        .into_iter()
        .map(|(id,)| {
            let key = ks.instance_action_point_key(instance_id, &id.to_string());
            (id, key)
        })
        .collect();

    Ok((measurement_map, action_map))
}
