//! Framework storage module
//!
//! `Integrates ComBase storage interface and optimized batch synchronization functionality`

use super::core::RedisValue;
use crate::core::sync::{DataSync, LuaSyncManager};
use crate::plugins::core::{telemetry_type_to_redis, PluginPointUpdate, PluginStorage};
use crate::utils::error::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};

// ============================================================================
// ComBase unified storage interface
// ============================================================================

/// ComBase layer unified storage trait
#[async_trait]
pub trait ComBaseStorage: Send + Sync {
    /// Batch update and publish data
    async fn batch_update_and_publish(
        &mut self,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()>;

    /// Single point update and publish
    async fn update_and_publish(
        &mut self,
        channel_id: u16,
        point_id: u32,
        value: RedisValue,
        telemetry_type: &str,
    ) -> Result<()>;

    /// Get storage statistics
    async fn get_stats(&self) -> StorageStats;
}

/// Storage statistics
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    pub total_updates: u64,
    pub batch_updates: u64,
    pub single_updates: u64,
    pub publish_success: u64,
    pub publish_failed: u64,
    pub storage_errors: u64,
}

/// `Default ComBase storage implementation`
pub struct DefaultComBaseStorage {
    storage: Arc<Mutex<Box<dyn PluginStorage>>>,
    stats: Mutex<StorageStats>,
    sync_manager: Option<Arc<LuaSyncManager>>,
}

impl DefaultComBaseStorage {
    /// Create new instance
    pub fn new(storage: Box<dyn PluginStorage>) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
            stats: Mutex::new(StorageStats::default()),
            sync_manager: None,
        }
    }

    /// Set synchronization manager
    pub fn set_sync_manager(&mut self, sync_manager: Arc<LuaSyncManager>) {
        self.sync_manager = Some(sync_manager);
    }

    /// Internal batch update method
    async fn internal_batch_update(
        &self,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()> {
        let storage = self.storage.lock().await;

        // Execute batch update
        storage.write_points(updates.clone()).await?;

        // If Lua synchronization is enabled, asynchronously synchronize data
        if let Some(sync_manager) = &self.sync_manager {
            let sync_updates: Vec<(u16, String, u32, f64)> = updates
                .into_iter()
                .map(|update| {
                    (
                        channel_id,
                        telemetry_type_to_redis(&update.telemetry_type).to_string(),
                        update.point_id,
                        update.value,
                    )
                })
                .collect();

            // Asynchronous synchronization, non-blocking main flow
            if !sync_updates.is_empty() {
                let update_count = sync_updates.len();
                match sync_manager.batch_sync(sync_updates).await {
                    Ok(()) => debug!("Batch sync initiated for {} points", update_count),
                    Err(e) => warn!("Batch sync failed (non-blocking): {}", e),
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl ComBaseStorage for DefaultComBaseStorage {
    async fn batch_update_and_publish(
        &mut self,
        channel_id: u16,
        updates: Vec<PluginPointUpdate>,
    ) -> Result<()> {
        let update_count = updates.len();

        match self.internal_batch_update(channel_id, updates).await {
            Ok(()) => {
                let mut stats = self.stats.lock().await;
                stats.total_updates += update_count as u64;
                stats.batch_updates += 1;
                Ok(())
            },
            Err(e) => {
                let mut stats = self.stats.lock().await;
                stats.storage_errors += 1;
                Err(e)
            },
        }
    }

    async fn update_and_publish(
        &mut self,
        channel_id: u16,
        point_id: u32,
        value: RedisValue,
        telemetry_type: &str,
    ) -> Result<()> {
        let float_value = match value {
            RedisValue::Float(f) => f,
            #[allow(clippy::cast_precision_loss)]
            RedisValue::Integer(i) => i as f64,
            RedisValue::Bool(b) => {
                if b {
                    1.0
                } else {
                    0.0
                }
            },
            RedisValue::String(s) => s.parse::<f64>().unwrap_or(0.0),
            RedisValue::Null => 0.0,
        };

        let update = PluginPointUpdate {
            channel_id,
            point_id,
            value: float_value,
            timestamp: i64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should not be before UNIX epoch")
                    .as_secs(),
            )
            .unwrap_or(i64::MAX),
            telemetry_type: telemetry_type.parse().map_err(|_| {
                crate::utils::error::ComSrvError::ParsingError(format!(
                    "Invalid telemetry type: {}",
                    telemetry_type
                ))
            })?, // Need to convert from string to enum
            raw_value: None,
        };

        match self
            .internal_batch_update(channel_id, vec![update.clone()])
            .await
        {
            Ok(()) => {
                let mut stats = self.stats.lock().await;
                stats.total_updates += 1;
                stats.single_updates += 1;

                // Single point synchronization (if enabled)
                if let Some(sync_manager) = &self.sync_manager {
                    match sync_manager
                        .sync_telemetry(channel_id, telemetry_type, point_id, float_value)
                        .await
                    {
                        Ok(()) => debug!("Single point sync initiated"),
                        Err(e) => warn!("Single point sync failed (non-blocking): {}", e),
                    }
                }

                Ok(())
            },
            Err(e) => {
                let mut stats = self.stats.lock().await;
                stats.storage_errors += 1;
                Err(e)
            },
        }
    }

    async fn get_stats(&self) -> StorageStats {
        self.stats.lock().await.clone()
    }
}

impl std::fmt::Debug for DefaultComBaseStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultComBaseStorage")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Create ComBase storage instance with storage
pub fn create_combase_storage(storage: Box<dyn PluginStorage>) -> Box<dyn ComBaseStorage> {
    Box::new(DefaultComBaseStorage::new(storage))
}

// ============================================================================
// Test module
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::core::DefaultPluginStorage;

    #[tokio::test]
    async fn test_combase_storage() {
        // Use default storage for testing
        if let Ok(default_storage) = DefaultPluginStorage::from_env().await {
            let plugin_storage = Box::new(default_storage) as Box<dyn PluginStorage>;
            let mut storage = DefaultComBaseStorage::new(plugin_storage);

            // Test single point update
            let result = storage
                .update_and_publish(1, 100, RedisValue::Float(42.0), "m")
                .await;
            assert!(result.is_ok());

            // Get statistics
            let stats = storage.get_stats().await;
            assert_eq!(stats.total_updates, 1);
            assert_eq!(stats.single_updates, 1);
        }
    }
}
