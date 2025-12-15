//! Channel lifecycle management module
//!
//! Handles channel creation, removal, and lifecycle operations

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::core::channels::point_config::RuntimeConfigProvider;
use crate::core::channels::sync::TelemetrySync;
use crate::core::channels::traits::{ComClient, TelemetryBatch};
use crate::core::channels::trigger::CommandTrigger;
use crate::core::config::{ChannelConfig, RuntimeChannelConfig};
use crate::error::{ComSrvError, Result};

// ============================================================================
// Channel Types (merged from channel.rs)
// ============================================================================

/// Channel metadata
#[derive(Debug, Clone)]
pub struct ChannelMetadata {
    pub name: Arc<str>,
    pub protocol_type: String,
    pub created_at: Instant,
    pub last_accessed: Arc<RwLock<Instant>>,
}

/// Channel entry, combining channel and metadata
#[derive(Clone)]
pub struct ChannelEntry {
    pub channel: Arc<RwLock<Box<dyn ComClient>>>,
    pub metadata: ChannelMetadata,
    pub command_trigger: Option<Arc<RwLock<CommandTrigger>>>,
    pub channel_config: Arc<ChannelConfig>,
}

impl std::fmt::Debug for ChannelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelEntry")
            .field("metadata", &self.metadata)
            .finish_non_exhaustive()
    }
}

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel_id: u32,
    pub name: String,
    pub protocol_type: String,
    pub is_connected: bool,
    pub created_at: Instant,
    pub last_accessed: Instant,
}

impl ChannelEntry {
    /// Create new channel entry
    pub fn new(
        channel: Arc<RwLock<Box<dyn ComClient>>>,
        channel_config: Arc<ChannelConfig>,
        protocol_type: String,
        command_trigger: Option<Arc<RwLock<CommandTrigger>>>,
    ) -> Self {
        let metadata = ChannelMetadata {
            name: Arc::from(channel_config.name()),
            protocol_type,
            created_at: Instant::now(),
            last_accessed: Arc::new(RwLock::new(Instant::now())),
        };

        Self {
            channel,
            metadata,
            command_trigger,
            channel_config,
        }
    }

    /// Get channel statistics
    pub async fn get_stats(&self, channel_id: u32) -> ChannelStats {
        let channel = self.channel.read().await;
        let last_accessed = *self.metadata.last_accessed.read().await;

        ChannelStats {
            channel_id,
            name: self.metadata.name.to_string(),
            protocol_type: self.metadata.protocol_type.clone(),
            is_connected: channel.is_connected(),
            created_at: self.metadata.created_at,
            last_accessed,
        }
    }

    /// Update last accessed time
    pub async fn touch(&self) {
        let mut last_accessed = self.metadata.last_accessed.write().await;
        *last_accessed = Instant::now();
    }
}

// ============================================================================
// Protocol Client Factory
// ============================================================================

/// Create protocol client based on protocol name
async fn create_protocol_client(
    protocol_name: &str,
    runtime_config: &RuntimeChannelConfig,
) -> Result<Box<dyn ComClient>> {
    match protocol_name {
        #[cfg(feature = "modbus")]
        "modbus_tcp" | "modbus_rtu" => {
            use crate::protocols::modbus::ModbusProtocol;
            Ok(Box::new(ModbusProtocol::from_runtime_config(
                runtime_config,
            )?))
        },
        "virtual" => {
            use crate::protocols::virt::VirtualProtocol;
            Ok(Box::new(VirtualProtocol::from_runtime_config(
                runtime_config,
            )?))
        },
        _ => Err(ComSrvError::ProtocolError(format!(
            "Unknown protocol: {}",
            protocol_name
        ))),
    }
}

/// Dynamic communication client type
pub type DynComClient = Arc<RwLock<Box<dyn ComClient>>>;

/// Channel manager - responsible for channel lifecycle management
pub struct ChannelManager {
    /// Store created channels
    channels: DashMap<u32, ChannelEntry, ahash::RandomState>,
    /// Shared RTDB (Redis or Memory for testing)
    rtdb: Arc<dyn voltage_rtdb::Rtdb>,
    /// Routing cache for C2M/M2C routing (public for reload operations)
    pub routing_cache: Arc<voltage_rtdb::RoutingCache>,
    /// Telemetry sync manager
    telemetry_sync: TelemetrySync,
    /// SQLite connection pool for configuration loading
    sqlite_pool: Option<sqlx::SqlitePool>,
    /// Point configuration provider for data transformation
    config_provider: Arc<RuntimeConfigProvider>,
}

impl std::fmt::Debug for ChannelManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelManager")
            .field("channels", &self.channels.len())
            .finish()
    }
}

impl ChannelManager {
    /// Create new channel manager
    pub fn new(
        rtdb: Arc<dyn voltage_rtdb::Rtdb>,
        routing_cache: Arc<voltage_rtdb::RoutingCache>,
    ) -> Self {
        // Create point configuration provider for data transformation
        let config_provider = Arc::new(RuntimeConfigProvider::new());

        let telemetry_sync =
            TelemetrySync::new(rtdb.clone(), routing_cache.clone(), config_provider.clone());

        Self {
            channels: DashMap::with_hasher(ahash::RandomState::default()),
            rtdb,
            routing_cache,
            telemetry_sync,
            sqlite_pool: None,
            config_provider,
        }
    }

    /// Create channel manager with SQLite pool
    pub fn with_sqlite_pool(
        rtdb: Arc<dyn voltage_rtdb::Rtdb>,
        routing_cache: Arc<voltage_rtdb::RoutingCache>,
        sqlite_pool: sqlx::SqlitePool,
    ) -> Self {
        // Create point configuration provider for data transformation
        let config_provider = Arc::new(RuntimeConfigProvider::new());

        let telemetry_sync =
            TelemetrySync::new(rtdb.clone(), routing_cache.clone(), config_provider.clone());

        Self {
            channels: DashMap::with_hasher(ahash::RandomState::default()),
            rtdb,
            routing_cache,
            telemetry_sync,
            sqlite_pool: Some(sqlite_pool),
            config_provider,
        }
    }

    /// Create channel
    pub async fn create_channel(
        &self,
        channel_config: Arc<ChannelConfig>,
    ) -> Result<Arc<RwLock<Box<dyn ComClient>>>> {
        let channel_id = channel_config.id();

        // Validate channel doesn't exist
        if self.channels.contains_key(&channel_id) {
            return Err(ComSrvError::ChannelExists(channel_id));
        }

        // Convert to RuntimeChannelConfig and load configuration from SQLite
        let mut runtime_config = RuntimeChannelConfig::from_base((*channel_config).clone());
        self.load_channel_configuration(&mut runtime_config).await?;
        let runtime_config = Arc::new(runtime_config);

        info!(
            "Ch{}: T={} S={} C={} A={} pts",
            channel_id,
            runtime_config.telemetry_points.len(),
            runtime_config.signal_points.len(),
            runtime_config.control_points.len(),
            runtime_config.adjustment_points.len()
        );

        // Load point transformers into config provider for data transformation
        self.config_provider
            .load_channel_config(&runtime_config)
            .await;
        debug!("Ch{} transformers loaded", channel_id);

        // Get protocol using normalized name
        let protocol_name = crate::utils::normalize_protocol_name(runtime_config.protocol());

        // Create client based on protocol name
        let mut client = create_protocol_client(&protocol_name, &runtime_config).await?;
        let base_config = Arc::clone(&runtime_config.base);

        // Setup Redis storage with runtime config containing actual point IDs
        self.initialize_channel_redis_storage(&runtime_config)
            .await?;

        // Initialize protocol with runtime config containing point data
        client.initialize(runtime_config.clone()).await?;

        // Setup telemetry data channel for protocol to send data
        let telemetry_sender = self.get_telemetry_sender();
        client.set_data_channel(telemetry_sender);

        // Start telemetry sync task (only once - protected by internal guard)
        if self.telemetry_sync.get_handle().read().await.is_none() {
            self.start_telemetry_sync_task().await?;
            info!("Telemetry sync started");
        }

        // Setup command trigger
        let (command_trigger, rx) = self.create_command_trigger(channel_id).await?;
        client.set_command_receiver(rx);

        // Create channel entry
        let channel_arc = Arc::new(RwLock::new(client));
        let entry = ChannelEntry::new(
            channel_arc.clone(),
            base_config,
            protocol_name.clone(),
            command_trigger,
        );

        self.channels.insert(channel_id, entry);

        info!("Ch{} created ({})", channel_id, protocol_name);
        Ok(channel_arc)
    }

    /// Load channel configuration from SQLite
    async fn load_channel_configuration(
        &self,
        runtime_config: &mut RuntimeChannelConfig,
    ) -> Result<()> {
        use crate::core::config::sqlite_loader::ComsrvSqliteLoader;

        // Use existing pool if available, otherwise create new connection
        if let Some(ref pool) = self.sqlite_pool {
            // Use existing pool (preferred for performance)
            let loader = ComsrvSqliteLoader::with_pool(pool.clone());
            loader.load_runtime_channel_points(runtime_config).await?;
        } else {
            // Fallback to creating new connection (less efficient)
            let db_path =
                std::env::var("VOLTAGE_DB_PATH").unwrap_or_else(|_| "data/voltage.db".to_string());
            let loader = ComsrvSqliteLoader::new(&db_path).await?;
            loader.load_runtime_channel_points(runtime_config).await?;
        }
        Ok(())
    }

    /// Remove channel
    pub async fn remove_channel(&self, channel_id: u32) -> Result<()> {
        if let Some((_, entry)) = self.channels.remove(&channel_id) {
            // Disconnect channel
            {
                let mut channel = entry.channel.write().await;
                let _ = channel.disconnect().await;
            }

            // Stop command trigger if exists
            if let Some(trigger_arc) = entry.command_trigger {
                let mut trigger = trigger_arc.write().await;
                let _ = trigger.stop().await;
            }

            info!("Ch{} removed", channel_id);
            Ok(())
        } else {
            Err(ComSrvError::ChannelNotFound(format!(
                "Channel {}",
                channel_id
            )))
        }
    }

    /// Get channel
    pub fn get_channel(&self, channel_id: u32) -> Option<Arc<RwLock<Box<dyn ComClient>>>> {
        self.channels
            .get(&channel_id)
            .map(|entry| entry.channel.clone())
    }

    /// Get channel IDs
    pub fn get_channel_ids(&self) -> Vec<u32> {
        self.channels.iter().map(|entry| *entry.key()).collect()
    }

    /// Get channel count
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Get running channel count
    pub async fn running_channel_count(&self) -> usize {
        let mut count = 0;
        for entry in self.channels.iter() {
            let channel = entry.channel.read().await;
            if channel.is_connected() {
                count += 1;
            }
        }
        count
    }

    /// Get channel metadata
    pub fn get_channel_metadata(&self, channel_id: u32) -> Option<(String, String)> {
        self.channels.get(&channel_id).map(|entry| {
            (
                entry.metadata.name.to_string(),
                format!("{:?}", entry.metadata.protocol_type),
            )
        })
    }

    /// Get channel stats
    pub async fn get_channel_stats(&self, channel_id: u32) -> Option<ChannelStats> {
        if let Some(entry) = self.channels.get(&channel_id) {
            Some(entry.get_stats(channel_id).await)
        } else {
            None
        }
    }

    /// Get all channel stats
    pub async fn get_all_channel_stats(&self) -> Vec<ChannelStats> {
        let mut stats = Vec::new();
        for entry in self.channels.iter() {
            let channel_id = *entry.key();
            stats.push(entry.value().get_stats(channel_id).await);
        }
        stats
    }

    /// Connect all channels
    pub async fn connect_all_channels(&self) -> Result<()> {
        let mut connect_tasks = Vec::new();

        for entry in self.channels.iter() {
            let channel_id = *entry.key();
            let channel_arc = entry.channel.clone();

            let task = tokio::spawn(async move {
                let mut channel = channel_arc.write().await;
                match channel.connect().await {
                    Ok(_) => {
                        info!("Ch{} connected", channel_id);
                        Ok(())
                    },
                    Err(e) => {
                        error!("Ch{} connect err: {}", channel_id, e);
                        Err(e)
                    },
                }
            });

            connect_tasks.push(task);
        }

        // Wait for all connections
        let mut failed_channels = Vec::new();
        for task in connect_tasks {
            if let Ok(Err(e)) = task.await {
                failed_channels.push(e);
            }
        }

        if failed_channels.is_empty() {
            Ok(())
        } else {
            Err(ComSrvError::BatchOperationFailed(format!(
                "Failed to connect {} channels",
                failed_channels.len()
            )))
        }
    }

    /// Start telemetry sync task
    pub async fn start_telemetry_sync_task(&self) -> Result<()> {
        self.telemetry_sync.start_telemetry_sync_task().await
    }

    /// Stop telemetry sync task
    pub async fn stop_telemetry_sync_task(&self) -> Result<()> {
        self.telemetry_sync.stop_telemetry_sync_task().await
    }

    /// Get telemetry data sender
    pub fn get_telemetry_sender(&self) -> tokio::sync::mpsc::Sender<TelemetryBatch> {
        self.telemetry_sync.get_sender()
    }

    /// Cleanup all resources
    pub async fn cleanup(&self) -> Result<()> {
        info!("Cleanup started");

        // Stop telemetry sync task
        let _ = self.stop_telemetry_sync_task().await;

        // Remove all channels
        let channel_ids: Vec<u32> = self.get_channel_ids();
        for channel_id in channel_ids {
            let _ = self.remove_channel(channel_id).await;
        }

        info!("Cleanup done");
        Ok(())
    }

    // ============================================================================
    // Private helper methods
    // ============================================================================

    /// Initialize channel points to Redis (replaces storage_manager.setup_redis_storage)
    async fn initialize_channel_redis_storage(
        &self,
        runtime_config: &RuntimeChannelConfig,
    ) -> Result<()> {
        use tracing::{debug, warn};

        let channel_id = runtime_config.base.id();
        debug!("Ch{} init Redis points", channel_id);

        // Use shared RTDB directly
        let rtdb = self.rtdb.clone();

        // Extract actual point IDs from RuntimeChannelConfig
        let telemetry_ids: Vec<u32> = runtime_config
            .telemetry_points
            .iter()
            .map(|p| p.base.point_id)
            .collect();
        let signal_ids: Vec<u32> = runtime_config
            .signal_points
            .iter()
            .map(|p| p.base.point_id)
            .collect();
        let control_ids: Vec<u32> = runtime_config
            .control_points
            .iter()
            .map(|p| p.base.point_id)
            .collect();
        let adjustment_ids: Vec<u32> = runtime_config
            .adjustment_points
            .iter()
            .map(|p| p.base.point_id)
            .collect();

        let telemetry_types: Vec<(&str, crate::core::config::FourRemote, Vec<u32>)> = vec![
            (
                "telemetry",
                crate::core::config::FourRemote::Telemetry,
                telemetry_ids,
            ),
            (
                "signal",
                crate::core::config::FourRemote::Signal,
                signal_ids,
            ),
            (
                "control",
                crate::core::config::FourRemote::Control,
                control_ids,
            ),
            (
                "adjustment",
                crate::core::config::FourRemote::Adjustment,
                adjustment_ids,
            ),
        ];

        for (telemetry_name, four_remote, point_ids) in telemetry_types {
            if point_ids.is_empty() {
                debug!("Ch{} no {} pts", channel_id, telemetry_name);
                continue;
            }

            debug!(
                "Ch{} {}: {} pts",
                channel_id,
                telemetry_name,
                point_ids.len()
            );

            // Get existing point IDs from Redis
            let config = voltage_rtdb::KeySpaceConfig::production();
            // Convert FourRemote to PointType via string (both have same T/S/C/A representation)
            let point_type = voltage_model::PointType::from_str(four_remote.as_str())
                .expect("FourRemote and PointType have matching string representations");
            let channel_key = config.channel_key(channel_id, point_type);

            // Check if Redis key exists (defensive verification)
            let key_exists = rtdb.exists(&channel_key).await.unwrap_or(false);
            debug!(
                "Channel {} {} - Redis key '{}' exists: {}",
                channel_id, telemetry_name, channel_key, key_exists
            );

            // Fetch existing fields in Redis Hash
            let existing_hash = rtdb.hash_get_all(&channel_key).await.unwrap_or_else(|e| {
                warn!("Fetch err {}: {}", channel_key, e);
                std::collections::HashMap::new()
            });
            let existing_fields: Vec<String> = existing_hash.keys().cloned().collect();

            debug!(
                "Ch{} {}: {} fields in Redis",
                channel_id,
                telemetry_name,
                existing_fields.len()
            );

            // Filter out timestamp suffix fields (e.g., "10:ts") and convert to point IDs
            let existing_point_ids: std::collections::HashSet<u32> = existing_fields
                .iter()
                .filter(|field| !field.ends_with(":ts") && !field.ends_with(":raw"))
                .filter_map(|field| field.parse::<u32>().ok())
                .collect();

            debug!(
                "Ch{} {}: {} existing pts",
                channel_id,
                telemetry_name,
                existing_point_ids.len()
            );

            // Calculate missing points that need initialization
            let missing_point_ids: Vec<u32> = point_ids
                .iter()
                .filter(|id| !existing_point_ids.contains(id))
                .copied()
                .collect();

            debug!(
                "Ch{} {}: {} missing pts",
                channel_id,
                telemetry_name,
                missing_point_ids.len()
            );

            if missing_point_ids.is_empty() {
                debug!(
                    "Ch{} {}: {} pts exist, skip",
                    channel_id,
                    telemetry_name,
                    point_ids.len()
                );
                continue;
            }

            info!(
                "Ch{} {} init: {} new (+{} exist)",
                channel_id,
                telemetry_name,
                missing_point_ids.len(),
                existing_point_ids.len()
            );

            // Build PointUpdate vector for missing points (using application-layer storage)
            let updates: Vec<crate::storage::PointUpdate> = missing_point_ids
                .iter()
                .map(|point_id| crate::storage::PointUpdate {
                    channel_id,
                    point_type: four_remote,
                    point_id: *point_id,
                    value: 0.0,           // Initialize with 0
                    raw_value: Some(0.0), // Initialize with 0
                    cascade_depth: 0,     // Initial depth for direct writes
                })
                .collect();

            // Call application-layer batch write
            crate::storage::write_batch(rtdb.as_ref(), &self.routing_cache, updates)
                .await
                .map_err(|e| {
                    ComSrvError::RedisError(format!(
                        "Failed to initialize {} points via application layer: {}",
                        telemetry_name, e
                    ))
                })?;

            debug!(
                "Ch{} {} init: {} pts",
                channel_id,
                telemetry_name,
                missing_point_ids.len()
            );
        }

        Ok(())
    }

    /// Create and start CommandTrigger (replaces storage_manager.setup_command_trigger)
    async fn create_command_trigger(
        &self,
        channel_id: u32,
    ) -> Result<(
        Option<Arc<RwLock<crate::core::channels::trigger::CommandTrigger>>>,
        tokio::sync::mpsc::Receiver<crate::core::channels::traits::ChannelCommand>,
    )> {
        use crate::core::channels::trigger::{CommandTrigger, CommandTriggerConfig};

        debug!("Ch{} trigger creating", channel_id);

        let config = CommandTriggerConfig {
            channel_id,
            ..Default::default()
        };

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Pass RTDB directly to trigger (works with both RedisRtdb and MemoryRtdb)
        let mut trigger = CommandTrigger::new(config, tx, self.rtdb.clone()).await?;
        trigger.start().await?;

        debug!("Ch{} trigger created", channel_id);

        Ok((Some(Arc::new(RwLock::new(trigger))), rx))
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    use voltage_rtdb::helpers::create_test_rtdb;

    /// Create test routing cache for unit tests
    fn create_test_routing_cache() -> Arc<voltage_rtdb::RoutingCache> {
        Arc::new(voltage_rtdb::RoutingCache::new())
    }

    #[tokio::test]
    async fn test_channel_manager_creation() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();
        let manager = ChannelManager::new(rtdb, routing_cache);

        assert_eq!(manager.channel_count(), 0);
        assert_eq!(manager.get_channel_ids().len(), 0);
    }

    #[tokio::test]
    async fn test_channel_manager_running_count() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();
        let manager = ChannelManager::new(rtdb, routing_cache);

        let count = manager.running_channel_count().await;
        assert_eq!(count, 0);
    }
}
