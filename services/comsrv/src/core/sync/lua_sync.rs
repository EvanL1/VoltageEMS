//! Redis Functions data synchronization manager
//!
//! Manages bidirectional data synchronization between ComsRv and other services using the new Redis Functions architecture

use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};
use voltage_libs::redis::RedisClient;

/// Redis Functions synchronization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaSyncConfig {
    /// Whether synchronization is enabled
    pub enabled: bool,
    /// Batch synchronization size
    pub batch_size: usize,
    /// Synchronization retry count
    pub retry_count: u32,
    /// Whether to synchronize asynchronously (non-blocking main flow)
    pub async_sync: bool,
    /// Whether to enable alarm triggering
    pub trigger_alarms: bool,
}

impl Default for LuaSyncConfig {
    fn default() -> Self {
        Self {
            enabled: true, // 同步是核心功能，应该默认启用
            batch_size: 100,
            retry_count: 3,
            async_sync: true,
            trigger_alarms: true,
        }
    }
}

/// Synchronization statistics
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    pub total_synced: u64,
    pub sync_success: u64,
    pub sync_failed: u64,
    pub no_mapping: u64,
    pub last_sync_error: Option<String>,
}

/// Synchronization update data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncUpdate {
    pub point_id: u32,
    pub value: f64,
}

/// Redis Functions synchronization manager
pub struct LuaSyncManager {
    config: LuaSyncConfig,
    redis_client: Arc<RwLock<RedisClient>>,
    stats: Arc<Mutex<SyncStats>>,
}

impl std::fmt::Debug for LuaSyncManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaSyncManager")
            .field("config", &self.config)
            .field("redis_client", &"<RedisClient>")
            .field("stats", &"<Arc<Mutex<SyncStats>>")
            .finish()
    }
}

impl LuaSyncManager {
    /// Create new synchronization manager
    pub async fn new(config: LuaSyncConfig, redis_client: RedisClient) -> Result<Self> {
        let redis_client = Arc::new(RwLock::new(redis_client));

        let manager = Self {
            config,
            redis_client: redis_client.clone(),
            stats: Arc::new(Mutex::new(SyncStats::default())),
        };

        if manager.config.enabled {
            info!("Redis Functions sync manager initialized");
        }

        Ok(manager)
    }

    /// Check if Redis Functions are available
    pub async fn check_functions_available(&self) -> Result<bool> {
        // Here we can check if Redis Functions are loaded
        // But in the new architecture, we assume they have been loaded correctly
        Ok(true)
    }

    /// Synchronize telemetry data
    pub async fn sync_telemetry(
        &self,
        channel_id: u16,
        telemetry_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Use batch synchronization for single data point
        let updates = vec![SyncUpdate { point_id, value }];

        self.sync_channel_data(channel_id, telemetry_type, updates)
            .await
    }

    /// Batch synchronize data
    pub async fn batch_sync_telemetries(
        &self,
        updates: Vec<(u16, String, u32, f64)>, // (channel_id, telemetry_type, point_id, value)
    ) -> Result<()> {
        if !self.config.enabled || updates.is_empty() {
            return Ok(());
        }

        // Group by channel and type
        let mut grouped_updates: std::collections::HashMap<(u16, String), Vec<SyncUpdate>> =
            std::collections::HashMap::new();

        for (channel_id, telemetry_type, point_id, value) in updates {
            let key = (channel_id, telemetry_type);
            grouped_updates
                .entry(key)
                .or_default()
                .push(SyncUpdate { point_id, value });
        }

        // Synchronize each group
        for ((channel_id, telemetry_type), updates) in grouped_updates {
            if self.config.async_sync {
                let manager_clone = Self {
                    config: self.config.clone(),
                    redis_client: self.redis_client.clone(),
                    stats: self.stats.clone(),
                };

                tokio::spawn(async move {
                    let _ = manager_clone
                        .sync_channel_data(channel_id, &telemetry_type, updates)
                        .await;
                });
            } else {
                self.sync_channel_data(channel_id, &telemetry_type, updates)
                    .await?;
            }
        }

        Ok(())
    }

    /// Synchronize channel data (using Redis Functions)
    async fn sync_channel_data(
        &self,
        channel_id: u16,
        point_type: &str,
        updates: Vec<SyncUpdate>,
    ) -> Result<()> {
        let mut stats = self.stats.lock().await;
        stats.total_synced += 1;

        let updates_json = serde_json::to_string(&updates)
            .map_err(|e| ComSrvError::ConfigError(format!("JSON serialization error: {}", e)))?;

        let timestamp = chrono::Utc::now().to_rfc3339();
        let trigger_alarms = if self.config.trigger_alarms {
            "true"
        } else {
            "false"
        };

        // Use Redis Functions call
        let keys = [channel_id.to_string()];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args = vec![point_type, &updates_json, trigger_alarms, &timestamp];

        // Get Redis connection and call function
        let mut conn = self.redis_client.write().await;

        let result: String = conn
            .fcall("sync_channel_data", &key_refs, &args)
            .await
            .map_err(|e| ComSrvError::RedisError(format!("Redis function call failed: {}", e)))?;

        // Parse result: [update_count, alarm_count]
        if let Ok(counts) = serde_json::from_str::<Vec<i64>>(&result) {
            if counts.len() >= 2 {
                let update_count = counts[0];
                let alarm_count = counts[1];

                stats.sync_success += 1;
                debug!(
                    "Channel {} sync successful: {} updates, {} alarms triggered",
                    channel_id, update_count, alarm_count
                );
            } else {
                stats.sync_success += 1;
                debug!("Channel {} sync successful", channel_id);
            }
        } else {
            stats.sync_success += 1;
            debug!("Channel {} sync completed: {}", channel_id, result);
        }

        Ok(())
    }

    /// Get synchronization statistics
    pub async fn get_stats(&self) -> SyncStats {
        self.stats.lock().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.lock().await;
        *stats = SyncStats::default();
    }

    /// Check if Redis Functions are available
    pub async fn is_functions_available(&self) -> bool {
        // In the new architecture, we assume functions are always available
        // In actual environment, we can check by calling FUNCTION LIST
        true
    }

    /// Enable/disable synchronization
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Get configuration
    pub fn config(&self) -> &LuaSyncConfig {
        &self.config
    }
}

/// Synchronization trait (for other modules to use)
#[async_trait]
pub trait DataSync: Send + Sync {
    /// Synchronize single telemetry point
    async fn sync_telemetry(
        &self,
        channel_id: u16,
        telemetry_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// Batch synchronization
    async fn batch_sync(&self, updates: Vec<(u16, String, u32, f64)>) -> Result<()>;
}

#[async_trait]
impl DataSync for LuaSyncManager {
    async fn sync_telemetry(
        &self,
        channel_id: u16,
        telemetry_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        self.sync_telemetry(channel_id, telemetry_type, point_id, value)
            .await
    }

    async fn batch_sync(&self, updates: Vec<(u16, String, u32, f64)>) -> Result<()> {
        self.batch_sync_telemetries(updates).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_default() {
        let config = LuaSyncConfig::default();
        assert!(config.enabled); // 同步应该默认启用
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.retry_count, 3);
        assert!(config.async_sync);
        assert!(config.trigger_alarms);
    }

    #[test]
    fn test_sync_stats_default() {
        let stats = SyncStats::default();
        assert_eq!(stats.total_synced, 0);
        assert_eq!(stats.sync_success, 0);
        assert_eq!(stats.sync_failed, 0);
        assert_eq!(stats.no_mapping, 0);
        assert!(stats.last_sync_error.is_none());
    }
}
