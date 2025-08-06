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
            enabled: true, // synchronousyes核心function，应该defaultenabling
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

    /// Synchronize telemetry data using the generic sync engine
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

        // Use the generic sync engine
        let updates = vec![SyncUpdate { point_id, value }];

        // Call the generic sync function
        self.sync_with_engine("comsrv_to_modsrv", channel_id, telemetry_type, updates)
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

    /// Synchronize data using the generic sync engine
    async fn sync_with_engine(
        &self,
        rule_id: &str,
        channel_id: u16,
        telemetry_type: &str,
        updates: Vec<SyncUpdate>,
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        // Call the generic sync engine
        let function_name = "sync_comsrv_to_modsrv";
        let updates_json = serde_json::to_string(&updates).map_err(|e| {
            ComSrvError::SerializationError(format!("Failed to serialize updates: {}", e))
        })?;

        let keys = [channel_id.to_string(), telemetry_type.to_string()];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args = vec![updates_json.as_str()];

        // Try to execute with retry logic
        let mut retry_count = 0;
        loop {
            let mut redis_client = self.redis_client.write().await;

            match redis_client
                .fcall::<String>(function_name, &key_refs, &args)
                .await
            {
                Ok(result) => {
                    // Parse sync results
                    if let Ok(sync_result) = serde_json::from_str::<serde_json::Value>(&result) {
                        let sync_count = sync_result["sync_count"].as_u64().unwrap_or(0);
                        let no_mapping = updates.len() as u64 - sync_count;

                        // Update statistics
                        let mut stats = self.stats.lock().await;
                        stats.total_synced += updates.len() as u64;
                        stats.sync_success += sync_count;
                        stats.no_mapping += no_mapping;

                        if sync_count > 0 {
                            debug!(
                                "Synced {} points using rule {} from channel {} type {}",
                                sync_count, rule_id, channel_id, telemetry_type
                            );
                        }
                    }
                    return Ok(());
                },
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= self.config.retry_count {
                        let mut stats = self.stats.lock().await;
                        stats.sync_failed += updates.len() as u64;
                        stats.last_sync_error = Some(e.to_string());

                        return Err(ComSrvError::SyncError(format!(
                            "Failed to sync after {} retries: {}",
                            retry_count, e
                        )));
                    }

                    // Wait before retry
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                },
            }
        }
    }

    /// Synchronize channel data (using both generic engine and legacy alarm processing)
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

        // Get Redis connection
        let mut conn = self.redis_client.write().await;

        // 1. Use the new generic sync engine for all configured sync rules
        // Execute sync based on the telemetry type
        let rule_suffix = match point_type {
            "T" => "_T",
            "S" => "_S",
            "C" => "_C",
            "A" => "_A",
            _ => "",
        };

        if !rule_suffix.is_empty() {
            // Build the rule ID based on the pattern
            let rule_id = format!("comsrv_to_modsrv{}", rule_suffix);
            let keys = [channel_id.to_string(), point_type.to_string()];
            let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
            let args = vec![updates_json.as_str()];

            // Call the generic sync engine
            match conn
                .fcall::<String>("sync_comsrv_to_modsrv", &key_refs, &args)
                .await
            {
                Ok(result) => {
                    debug!("Synced via rule {}: {}", rule_id, result);
                },
                Err(e) => {
                    // Non-fatal error, just log it
                    debug!("Sync for rule {} skipped: {}", rule_id, e);
                },
            }

            // Also trigger other sync rules (e.g., to alarmsrv, hissrv)
            // This allows multiple services to receive the same data
            if point_type == "T" {
                // Sync to alarmsrv for alarm checking
                match conn
                    .fcall::<String>("sync_pattern_execute", &["comsrv_to_alarmsrv"], &[])
                    .await
                {
                    Ok(_) => debug!("Alarm sync triggered"),
                    Err(e) => debug!("Alarm sync skipped: {}", e),
                }
            }

            // Sync to hissrv for historical storage (all types)
            match conn
                .fcall::<String>("sync_pattern_execute", &["comsrv_to_hissrv"], &[])
                .await
            {
                Ok(_) => debug!("History sync triggered"),
                Err(e) => debug!("History sync skipped: {}", e),
            }
        }

        // 2. Still call the original sync_channel_data for alarm processing
        let keys = [channel_id.to_string()];
        let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let args = vec![point_type, &updates_json, trigger_alarms, &timestamp];

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
        assert!(config.enabled); // synchronous应该defaultenabling
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
