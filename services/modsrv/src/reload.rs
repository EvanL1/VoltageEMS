//! Hot reload implementation for modsrv
//!
//! This module implements the unified `ReloadableService` trait for modsrv,
//! enabling incremental synchronization of instance configurations from SQLite to Redis.

use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info, warn};
use voltage_config::{InstanceReloadResult, ReloadableService};
use voltage_rtdb::Rtdb;

use crate::instance_manager::InstanceManager;
use crate::product_loader::Instance;
use crate::redis_state;

/// Instance change severity classification
///
/// For modsrv, all changes are treated as configuration updates since there are
/// no active connections to restart (unlike comsrv's protocol clients).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstanceChangeType {
    /// Configuration update (properties, product_name)
    ConfigUpdate = 0,
}

impl<R: Rtdb + 'static> ReloadableService for InstanceManager<R> {
    type ChangeType = InstanceChangeType;
    type Config = Instance;
    type ReloadResult = InstanceReloadResult;

    /// Reload instances from SQLite database with incremental sync
    async fn reload_from_database(
        &self,
        _pool: &sqlx::SqlitePool,
    ) -> anyhow::Result<Self::ReloadResult> {
        let start_time = std::time::Instant::now();
        info!("Starting incremental instance reload from SQLite");

        // 1. Load all instances from SQLite
        let db_instances = self.list_instances(None).await?;

        let db_ids: std::collections::HashSet<u16> =
            db_instances.iter().map(|inst| inst.instance_id()).collect();

        // 2. Get all instance IDs from Redis by scanning inst:*:name keys
        let redis_ids = Self::get_redis_instance_ids(&self.rtdb).await?;

        // 3. Determine changes
        let to_add: Vec<u16> = db_ids.difference(&redis_ids).copied().collect();
        let to_remove: Vec<u16> = redis_ids.difference(&db_ids).copied().collect();
        let to_update: Vec<u16> = db_ids.intersection(&redis_ids).copied().collect();

        let mut added = Vec::new();
        let mut updated = Vec::new();
        let mut removed = Vec::new();
        let mut errors = Vec::new();

        // 4. Remove instances that are no longer in SQLite
        for id in &to_remove {
            // Find instance name in Redis for logging
            let instance_name = self
                .rtdb
                .get(&format!("inst:{}:name", id))
                .await
                .ok()
                .flatten()
                .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
                .unwrap_or_else(|| format!("instance_{}", id));

            match redis_state::unregister_instance(self.rtdb.as_ref(), *id, &instance_name).await {
                Ok(_) => {
                    removed.push(*id);
                    info!("Removed instance {} (id: {}) from Redis", instance_name, id);
                },
                Err(e) => {
                    errors.push(format!("Failed to remove instance {}: {}", id, e));
                    error!("Failed to remove instance {}: {}", id, e);
                },
            }
        }

        // 5. Add new instances to Redis
        for id in &to_add {
            if let Some(instance) = db_instances.iter().find(|i| i.instance_id() == *id) {
                match self.sync_single_instance_to_redis(instance).await {
                    Ok(_) => {
                        added.push(*id);
                        info!(
                            "Added instance {} (id: {}) to Redis",
                            instance.instance_name(),
                            id
                        );
                    },
                    Err(e) => {
                        errors.push(format!(
                            "Failed to add instance {}: {}",
                            instance.instance_name(),
                            e
                        ));
                        error!("Failed to add instance {}: {}", instance.instance_name(), e);
                    },
                }
            }
        }

        // 6. Update existing instances (re-sync to Redis)
        for id in &to_update {
            if let Some(instance) = db_instances.iter().find(|i| i.instance_id() == *id) {
                match self.sync_single_instance_to_redis(instance).await {
                    Ok(_) => {
                        updated.push(*id);
                        info!(
                            "Updated instance {} (id: {}) in Redis",
                            instance.instance_name(),
                            id
                        );
                    },
                    Err(e) => {
                        errors.push(format!(
                            "Failed to update instance {}: {}",
                            instance.instance_name(),
                            e
                        ));
                        error!(
                            "Failed to update instance {}: {}",
                            instance.instance_name(),
                            e
                        );
                    },
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let total_count = db_instances.len();

        info!(
            "Instance reload completed: {} added, {} updated, {} removed, {} errors ({}ms)",
            added.len(),
            updated.len(),
            removed.len(),
            errors.len(),
            duration_ms
        );

        Ok(InstanceReloadResult {
            total_count,
            added,
            updated,
            removed,
            errors,
            duration_ms,
        })
    }

    /// Analyze changes between old and new configuration
    fn analyze_changes(
        &self,
        _old_config: &Self::Config,
        _new_config: &Self::Config,
    ) -> Self::ChangeType {
        // For modsrv, all changes are config updates
        InstanceChangeType::ConfigUpdate
    }

    /// Perform hot reload of an instance (sync to Redis)
    async fn perform_hot_reload(&self, config: Self::Config) -> anyhow::Result<String> {
        info!(
            "Performing hot reload for instance {} (id: {})",
            config.instance_name(),
            config.instance_id()
        );

        self.sync_single_instance_to_redis(&config).await?;
        Ok("synced".to_string())
    }

    /// Rollback to previous configuration
    async fn rollback(&self, previous_config: Self::Config) -> anyhow::Result<String> {
        warn!(
            "Rolling back instance {} to previous configuration",
            previous_config.instance_name()
        );

        self.sync_single_instance_to_redis(&previous_config).await?;
        Ok("restored".to_string())
    }
}

impl<R: Rtdb + 'static> InstanceManager<R> {
    /// Get all instance IDs from Redis by scanning inst:*:name keys
    async fn get_redis_instance_ids(rtdb: &Arc<R>) -> Result<std::collections::HashSet<u16>> {
        // Scan for all inst:*:name keys
        let pattern = "inst:*:name";
        let keys = rtdb.scan_match(pattern).await?;

        let mut instance_ids = std::collections::HashSet::new();

        for key in keys {
            // Extract instance ID from key format: inst:{id}:name
            if let Some(id_str) = key
                .strip_prefix("inst:")
                .and_then(|s| s.strip_suffix(":name"))
            {
                if let Ok(id) = id_str.parse::<u16>() {
                    instance_ids.insert(id);
                } else {
                    warn!("Invalid instance ID in Redis key: {}", key);
                }
            }
        }

        info!("Found {} instances in Redis", instance_ids.len());
        Ok(instance_ids)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_instance_change_type() {
        // For modsrv, all changes are classified as ConfigUpdate
        let change_type = InstanceChangeType::ConfigUpdate;
        assert_eq!(change_type, InstanceChangeType::ConfigUpdate);

        // Test ordering
        assert!(InstanceChangeType::ConfigUpdate == InstanceChangeType::ConfigUpdate);
    }

    #[test]
    fn test_instance_creation() {
        // Test instance creation works correctly
        let instance = create_test_instance(1, "pv_inverter_01", "pv_inverter");
        assert_eq!(instance.instance_id(), 1);
        assert_eq!(instance.instance_name(), "pv_inverter_01");
        assert_eq!(instance.product_name(), "pv_inverter");
    }

    fn create_test_instance(id: u16, name: &str, product: &str) -> Instance {
        Instance {
            core: voltage_config::modsrv::InstanceCore {
                instance_id: id,
                instance_name: name.to_string(),
                product_name: product.to_string(),
                properties: std::collections::HashMap::new(),
            },
            measurement_mappings: None,
            action_mappings: None,
            created_at: None,
        }
    }
}
