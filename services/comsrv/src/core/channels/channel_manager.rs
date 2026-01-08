//! Channel lifecycle management module
//!
//! Handles channel creation, removal, and lifecycle operations

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use arc_swap::ArcSwapOption;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Maximum number of channel slots (pre-allocated for O(1) access)
/// Channel IDs must be < MAX_CHANNELS
const MAX_CHANNELS: usize = 10000;

use crate::core::channels::igw_bridge::{
    convert_to_igw_point_configs, convert_to_modbus_point_configs, create_modbus_channel,
    create_modbus_rtu_channel, create_virtual_channel, ChannelImpl, IgwChannelWrapper,
};

#[cfg(all(target_os = "linux", feature = "gpio"))]
use crate::core::channels::igw_bridge::create_gpio_channel;
#[cfg(all(feature = "can", target_os = "linux"))]
use crate::core::channels::igw_bridge::{
    convert_can_to_igw_point_configs, convert_to_can_point_configs, create_can_channel,
};
use crate::core::channels::trigger::CommandTrigger;
use crate::core::config::{ChannelConfig, RuntimeChannelConfig};
use crate::error::{ComSrvError, Result};
use crate::store::RedisDataStore;
use voltage_rtdb::{ChannelToSlotIndex, Rtdb, SharedVecRtdbWriter};

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
pub struct ChannelEntry<R: Rtdb> {
    /// Dual-mode channel implementation (Legacy ComClient or IGW ProtocolClient)
    pub channel: ChannelImpl<R>,
    pub metadata: ChannelMetadata,
    pub command_trigger: Option<Arc<RwLock<CommandTrigger<R>>>>,
    pub channel_config: Arc<ChannelConfig>,
    /// Direct command sender for bypassing TODO queue
    pub command_tx:
        Option<tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>>,
}

impl<R: Rtdb> std::fmt::Debug for ChannelEntry<R> {
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

impl<R: Rtdb> ChannelEntry<R> {
    /// Create new channel entry
    pub fn new(
        channel: ChannelImpl<R>,
        channel_config: Arc<ChannelConfig>,
        protocol_type: String,
        command_trigger: Option<Arc<RwLock<CommandTrigger<R>>>>,
        command_tx: Option<
            tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>,
        >,
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
            command_tx,
        }
    }

    /// Get channel statistics
    pub async fn get_stats(&self, channel_id: u32) -> ChannelStats {
        let last_accessed = *self.metadata.last_accessed.read().await;

        ChannelStats {
            channel_id,
            name: self.metadata.name.to_string(),
            protocol_type: self.metadata.protocol_type.clone(),
            is_connected: self.channel.read().await.is_connected().await,
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
// Channel Manager
// ============================================================================

/// Channel manager - responsible for channel lifecycle management
///
/// # arc-swap + Vec Architecture
/// Uses pre-allocated `Vec<ArcSwapOption<ChannelEntry>>` for O(1) lock-free access.
/// - Read latency: ~5ns (was ~50μs with RwLock+DashMap)
/// - Write latency: ~50ns (atomic swap)
/// - Memory: ~160KB for 10000 slots (16 bytes per ArcSwapOption)
pub struct ChannelManager<R: Rtdb> {
    /// Pre-allocated channel slots for O(1) direct index access
    /// Index = channel_id, value = Option<Arc<ChannelEntry>>
    channels: Vec<ArcSwapOption<ChannelEntry<R>>>,
    /// Shared RTDB (Redis or Memory for testing)
    rtdb: Arc<R>,
    /// Routing cache for C2M/M2C routing (public for reload operations)
    pub routing_cache: Arc<voltage_rtdb::RoutingCache>,
    /// SQLite connection pool for configuration loading
    sqlite_pool: Option<sqlx::SqlitePool>,
    /// Shared memory writer for zero-copy cross-process data sharing (optional)
    shared_writer: Option<Arc<SharedVecRtdbWriter>>,
    /// Pre-computed channel → slot mapping for O(1) shared memory writes (optional)
    channel_index: Option<Arc<ChannelToSlotIndex>>,
    /// Command TX cache for O(1) hot path access
    /// Shared with AppState for direct API access bypassing RwLock
    command_tx_cache: Option<Arc<crate::api::command_cache::CommandTxCache>>,
}

impl<R: Rtdb> std::fmt::Debug for ChannelManager<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelManager")
            .field("channels", &self.channel_count())
            .finish()
    }
}

impl<R: Rtdb + 'static> ChannelManager<R> {
    /// Pre-allocate channel slots for O(1) access
    #[inline]
    fn create_channel_slots() -> Vec<ArcSwapOption<ChannelEntry<R>>> {
        (0..MAX_CHANNELS).map(|_| ArcSwapOption::empty()).collect()
    }

    /// Create new channel manager
    pub fn new(rtdb: Arc<R>, routing_cache: Arc<voltage_rtdb::RoutingCache>) -> Self {
        Self {
            channels: Self::create_channel_slots(),
            rtdb,
            routing_cache,
            sqlite_pool: None,
            shared_writer: None,
            channel_index: None,
            command_tx_cache: None,
        }
    }

    /// Create channel manager with SQLite pool
    pub fn with_sqlite_pool(
        rtdb: Arc<R>,
        routing_cache: Arc<voltage_rtdb::RoutingCache>,
        sqlite_pool: sqlx::SqlitePool,
    ) -> Self {
        Self {
            channels: Self::create_channel_slots(),
            rtdb,
            routing_cache,
            sqlite_pool: Some(sqlite_pool),
            shared_writer: None,
            channel_index: None,
            command_tx_cache: None,
        }
    }

    /// Create channel manager with shared memory support
    ///
    /// # Arguments
    /// * `rtdb` - Shared RTDB (Redis or Memory)
    /// * `routing_cache` - C2M/M2C routing cache
    /// * `sqlite_pool` - SQLite connection pool for configuration
    /// * `shared_writer` - Optional shared memory writer for zero-copy writes
    /// * `channel_index` - Optional pre-computed channel → slot mapping
    /// * `command_tx_cache` - Optional CommandTxCache for O(1) hot path
    ///
    /// Note: Removed VecRtdb - using SharedMemory + Redis two-tier architecture
    pub fn with_shared_memory(
        rtdb: Arc<R>,
        routing_cache: Arc<voltage_rtdb::RoutingCache>,
        sqlite_pool: sqlx::SqlitePool,
        shared_writer: Option<Arc<SharedVecRtdbWriter>>,
        channel_index: Option<Arc<ChannelToSlotIndex>>,
        command_tx_cache: Option<Arc<crate::api::command_cache::CommandTxCache>>,
    ) -> Self {
        Self {
            channels: Self::create_channel_slots(),
            rtdb,
            routing_cache,
            sqlite_pool: Some(sqlite_pool),
            shared_writer,
            channel_index,
            command_tx_cache,
        }
    }

    /// Create channel
    pub async fn create_channel(
        &self,
        channel_config: Arc<ChannelConfig>,
    ) -> Result<ChannelImpl<R>> {
        let channel_id = channel_config.id();

        // Bounds check for pre-allocated Vec
        let slot = self
            .channels
            .get(channel_id as usize)
            .ok_or_else(|| ComSrvError::invalid_channel_id(channel_id))?;

        // Validate channel doesn't exist (O(1) atomic load)
        if slot.load().is_some() {
            return Err(ComSrvError::channel_exists(channel_id));
        }

        // Convert to RuntimeChannelConfig and load configuration from SQLite
        let mut runtime_config = RuntimeChannelConfig::from_base_arc(Arc::clone(&channel_config));
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

        // Get protocol using normalized name
        let protocol_name = crate::utils::normalize_protocol_name(runtime_config.protocol());
        let base_config = Arc::clone(&runtime_config.base);

        // Setup Redis storage with runtime config containing actual point IDs
        self.initialize_channel_redis_storage(&runtime_config)
            .await?;

        // Branch based on protocol type: IGW path for virtual/modbus, Legacy for others
        let (channel_impl, command_trigger, command_tx) = match protocol_name.as_str() {
            "virtual" => {
                // IGW path: Use igw::VirtualChannel with RedisDataStore
                self.create_igw_virtual_channel(channel_id, &runtime_config)
                    .await?
            },
            "modbus_tcp" => {
                // IGW path: Use igw::ModbusChannel (TCP) with RedisDataStore
                self.create_igw_modbus_channel(channel_id, &runtime_config)
                    .await?
            },
            "modbus_rtu" => {
                // IGW path: Use igw::ModbusChannel (RTU/serial) with RedisDataStore
                self.create_igw_modbus_rtu_channel(channel_id, &runtime_config)
                    .await?
            },
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            "gpio" | "di_do" | "dido" => {
                // IGW path: Use igw::GpioChannel for DI/DO
                self.create_igw_gpio_channel(channel_id, &runtime_config)
                    .await?
            },
            #[cfg(all(feature = "can", target_os = "linux"))]
            "can" => {
                // IGW path: Use igw::CanClient with RedisDataStore
                self.create_igw_can_channel(channel_id, &runtime_config)
                    .await?
            },
            _ => {
                // All protocols now use IGW - unsupported protocols should error
                // Base protocols available on all platforms
                #[allow(unused_mut)] // mut needed for cfg-conditional push_str on Linux
                let mut supported = String::from("virtual, modbus_tcp, modbus_rtu");

                #[cfg(all(target_os = "linux", feature = "gpio"))]
                supported.push_str(", gpio/di_do");

                #[cfg(all(feature = "can", target_os = "linux"))]
                supported.push_str(", can");

                return Err(anyhow::anyhow!(
                    "Unsupported protocol '{}' for channel {}. Supported: {}",
                    protocol_name,
                    channel_id,
                    supported
                )
                .into());
            },
        };

        // Register command_tx with cache for O(1) hot path access
        // Clone tx before moving into ChannelEntry to avoid ownership issues
        if let (Some(ref cache), Some(ref tx)) = (&self.command_tx_cache, &command_tx) {
            cache.register(channel_id, tx.clone());
        }

        let entry = ChannelEntry::new(
            channel_impl.clone(),
            base_config,
            protocol_name.clone(),
            command_trigger,
            command_tx, // Direct command sender for bypassing TODO queue
        );

        // O(1) atomic store (slot already validated above)
        slot.store(Some(Arc::new(entry)));

        info!("Ch{} created ({})", channel_id, protocol_name);
        Ok(channel_impl)
    }

    /// Create IGW-based virtual channel.
    ///
    /// Uses igw::VirtualChannel with RedisDataStore for data persistence.
    async fn create_igw_virtual_channel(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
    ) -> Result<(
        ChannelImpl<R>,
        Option<Arc<RwLock<CommandTrigger<R>>>>,
        Option<tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>>,
    )> {
        debug!("Ch{} creating via IGW path", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert point configs to IGW format and register with store
        let point_configs = convert_to_igw_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Create VirtualChannel (no store - storage handled by IgwChannelWrapper)
        let protocol = create_virtual_channel(channel_id, runtime_config.name(), point_configs);

        // 5. Setup command trigger for M2C control
        let (command_trigger, rx, command_tx) = self.create_command_trigger(channel_id).await?;

        // 6. Create IgwChannelWrapper with command processing and storage
        // Virtual channel uses default 1000ms polling
        let poll_interval_ms = runtime_config
            .base
            .parameters
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        // Point types are encoded in internal_id by igw_bridge - no registration needed
        let wrapper = IgwChannelWrapper::new(protocol, channel_id, store, rx, poll_interval_ms);
        let channel_impl: ChannelImpl<R> = Arc::new(RwLock::new(wrapper));

        info!("Ch{} created via IGW (virtual)", channel_id);
        Ok((channel_impl, command_trigger, command_tx))
    }

    /// Create IGW-based Modbus TCP channel.
    ///
    /// Uses igw::ModbusChannel with RedisDataStore for data persistence.
    /// Includes batch read optimization, auto-reconnect, and zero-data detection.
    async fn create_igw_modbus_channel(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
    ) -> Result<(
        ChannelImpl<R>,
        Option<Arc<RwLock<CommandTrigger<R>>>>,
        Option<tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>>,
    )> {
        debug!("Ch{} creating via IGW Modbus path", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert Modbus point configs to IGW format
        let point_configs = convert_to_modbus_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Extract host/port from runtime config parameters
        let params = &runtime_config.base.parameters;
        let host = params
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1");
        let port = params
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|n| n as u16)
            .unwrap_or(502);

        // 5. Create ModbusChannel via igw_bridge (no store - storage handled by IgwChannelWrapper)
        let protocol = create_modbus_channel(channel_id, host, port, point_configs);

        // 6. Setup command trigger for M2C control
        let (command_trigger, rx, command_tx) = self.create_command_trigger(channel_id).await?;

        // 7. Create IgwChannelWrapper with command processing and storage
        // Modbus uses internal polling, external polling as backup (default 1000ms)
        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        // Point types are encoded in internal_id by igw_bridge - no registration needed
        let wrapper = IgwChannelWrapper::new(protocol, channel_id, store, rx, poll_interval_ms);
        let channel_impl: ChannelImpl<R> = Arc::new(RwLock::new(wrapper));

        info!("Ch{} created via IGW (modbus_tcp)", channel_id);
        Ok((channel_impl, command_trigger, command_tx))
    }

    /// Create IGW-based Modbus RTU (serial) channel.
    ///
    /// Uses igw::ModbusChannel in RTU mode with RedisDataStore for data persistence.
    /// Includes batch read optimization, auto-reconnect, and zero-data detection.
    async fn create_igw_modbus_rtu_channel(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
    ) -> Result<(
        ChannelImpl<R>,
        Option<Arc<RwLock<CommandTrigger<R>>>>,
        Option<tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>>,
    )> {
        debug!("Ch{} creating via IGW Modbus RTU path", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert Modbus point configs to IGW format
        let point_configs = convert_to_modbus_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Extract device/baud_rate from runtime config parameters
        let params = &runtime_config.base.parameters;
        let device = params
            .get("device")
            .and_then(|v| v.as_str())
            .unwrap_or("/dev/ttyUSB0");
        let baud_rate = params
            .get("baud_rate")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or(9600);

        // 5. Create ModbusChannel (RTU) via igw_bridge
        let protocol = create_modbus_rtu_channel(channel_id, device, baud_rate, point_configs);

        // 6. Setup command trigger for M2C control
        let (command_trigger, rx, command_tx) = self.create_command_trigger(channel_id).await?;

        // 7. Create IgwChannelWrapper with command processing and storage
        // Modbus uses internal polling, external polling as backup (default 1000ms)
        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        // Point types are encoded in internal_id by igw_bridge - no registration needed
        let wrapper = IgwChannelWrapper::new(protocol, channel_id, store, rx, poll_interval_ms);
        let channel_impl: ChannelImpl<R> = Arc::new(RwLock::new(wrapper));

        info!("Ch{} created via IGW (modbus_rtu)", channel_id);
        Ok((channel_impl, command_trigger, command_tx))
    }

    /// Create IGW-based GPIO channel for DI/DO.
    ///
    /// GPIO channels support:
    /// - Signal points (DI): Digital input reading
    /// - Control points (DO): Digital output control
    #[cfg(all(target_os = "linux", feature = "gpio"))]
    async fn create_igw_gpio_channel(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
    ) -> Result<(
        ChannelImpl<R>,
        Option<Arc<RwLock<CommandTrigger<R>>>>,
        Option<tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>>,
    )> {
        debug!("Ch{} creating via IGW GPIO path", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert point configs to IGW format (for signal/control points)
        let point_configs = convert_to_igw_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs);

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Create GpioChannel via igw_bridge
        let protocol = create_gpio_channel(channel_id, runtime_config);

        // 5. Setup command trigger for M2C control (DO commands)
        let (command_trigger, rx, command_tx) = self.create_command_trigger(channel_id).await?;

        // 6. Create IgwChannelWrapper
        // GPIO needs faster polling (default 200ms for responsive DI detection)
        let poll_interval_ms = runtime_config
            .base
            .parameters
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(200);

        // Point types are encoded in internal_id by igw_bridge - no registration needed
        let wrapper = IgwChannelWrapper::new(protocol, channel_id, store, rx, poll_interval_ms);
        let channel_impl: ChannelImpl<R> = Arc::new(RwLock::new(wrapper));

        info!("Ch{} created via IGW (gpio)", channel_id);
        Ok((channel_impl, command_trigger, command_tx))
    }

    /// Create IGW-based CAN channel.
    ///
    /// Uses igw::CanClient with RedisDataStore for data persistence.
    /// CAN protocol is event-driven and read-only (no M2C control).
    #[cfg(all(feature = "can", target_os = "linux"))]
    async fn create_igw_can_channel(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
    ) -> Result<(
        ChannelImpl<R>,
        Option<Arc<RwLock<CommandTrigger<R>>>>,
        Option<tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>>,
    )> {
        debug!("Ch{} creating via IGW CAN path", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert CAN mappings to IGW formats
        let can_point_configs = convert_to_can_point_configs(runtime_config);
        let igw_point_configs = convert_can_to_igw_point_configs(runtime_config);

        if can_point_configs.is_empty() {
            warn!("Ch{} has no CAN point mappings configured", channel_id);
        }

        store.set_point_configs(channel_id, igw_point_configs);

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Extract CAN interface from runtime config parameters
        let params = &runtime_config.base.parameters;
        let can_interface = params
            .get("device")
            .and_then(|v| v.as_str())
            .unwrap_or("can0");

        // 5. Create CanClient via igw_bridge
        let protocol = create_can_channel(channel_id, can_interface, can_point_configs);

        // 6. Setup command trigger (CAN is read-only, but we still create the trigger for consistency)
        let (command_trigger, rx, command_tx) = self.create_command_trigger(channel_id).await?;

        // 7. Create IgwChannelWrapper
        // CAN is event-driven, needs faster polling (default 200ms)
        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(200);

        // Point types are encoded in internal_id by igw_bridge - no registration needed
        let wrapper = IgwChannelWrapper::new(protocol, channel_id, store, rx, poll_interval_ms);
        let channel_impl: ChannelImpl<R> = Arc::new(RwLock::new(wrapper));

        info!("Ch{} created via IGW (can)", channel_id);
        Ok((channel_impl, command_trigger, command_tx))
    }

    /// Create RedisDataStore with optional shared memory support
    ///
    /// This is a helper method that creates a RedisDataStore and optionally
    /// configures it with shared memory support if available.
    ///
    /// Note: Removed VecRtdb - using SharedMemory + Redis two-tier architecture
    fn create_data_store(&self) -> Arc<RedisDataStore<R>> {
        let store = RedisDataStore::new(Arc::clone(&self.rtdb), Arc::clone(&self.routing_cache));

        // Add shared memory support if available
        let store = if let (Some(writer), Some(index)) = (&self.shared_writer, &self.channel_index)
        {
            store.with_shared_memory(Arc::clone(writer), Arc::clone(index))
        } else {
            store
        };

        Arc::new(store)
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
        // Unregister from cache before removing channel
        if let Some(ref cache) = self.command_tx_cache {
            cache.unregister(channel_id);
        }

        // O(1) atomic swap
        let slot = self
            .channels
            .get(channel_id as usize)
            .ok_or_else(|| ComSrvError::invalid_channel_id(channel_id))?;

        if let Some(entry) = slot.swap(None) {
            // Disconnect channel using ChannelImpl's unified interface
            let _ = entry.channel.write().await.disconnect().await;

            // Stop command trigger if exists
            if let Some(trigger_arc) = &entry.command_trigger {
                let mut trigger = trigger_arc.write().await;
                let _ = trigger.stop().await;
            }

            info!("Ch{} removed", channel_id);
            Ok(())
        } else {
            Err(ComSrvError::channel_not_found(channel_id))
        }
    }

    /// Get channel implementation (dual-mode)
    ///
    /// # O(1) lock-free access (~5ns)
    #[inline]
    pub fn get_channel(&self, channel_id: u32) -> Option<ChannelImpl<R>> {
        self.channels
            .get(channel_id as usize)?
            .load_full()
            .map(|entry| entry.channel.clone())
    }

    /// Get channel entry (for direct command_tx access)
    ///
    /// Unlike `get_channel()` which returns only the ChannelImpl,
    /// this returns the full ChannelEntry Arc for accessing command_tx.
    ///
    /// # Optimization
    /// Used by API handlers to directly send commands via mpsc,
    /// bypassing the Redis TODO queue latency.
    ///
    /// # O(1) lock-free access (~5ns)
    /// Returns `Arc<ChannelEntry>` instead of DashMap Ref (no lifetime constraints)
    #[inline]
    pub fn get_channel_entry(&self, channel_id: u32) -> Option<Arc<ChannelEntry<R>>> {
        self.channels.get(channel_id as usize)?.load_full()
    }

    /// Get channel IDs
    ///
    /// # Iterate over pre-allocated Vec
    pub fn get_channel_ids(&self) -> Vec<u32> {
        self.channels
            .iter()
            .enumerate()
            .filter_map(|(id, slot)| {
                if slot.load().is_some() {
                    Some(id as u32)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get channel count
    ///
    /// # Count non-empty slots
    pub fn channel_count(&self) -> usize {
        self.channels
            .iter()
            .filter(|slot| slot.load().is_some())
            .count()
    }

    /// Get running channel count
    pub async fn running_channel_count(&self) -> usize {
        let mut count = 0;
        for (id, slot) in self.channels.iter().enumerate() {
            if let Some(entry) = slot.load_full() {
                if entry.channel.read().await.is_connected().await {
                    count += 1;
                }
            }
            // Skip slots beyond reasonable range to avoid unnecessary checks
            if id > 10000 {
                break;
            }
        }
        count
    }

    /// Get channel metadata
    pub fn get_channel_metadata(&self, channel_id: u32) -> Option<(String, String)> {
        self.channels
            .get(channel_id as usize)?
            .load_full()
            .map(|entry| {
                (
                    entry.metadata.name.to_string(),
                    format!("{:?}", entry.metadata.protocol_type),
                )
            })
    }

    /// Get channel stats
    pub async fn get_channel_stats(&self, channel_id: u32) -> Option<ChannelStats> {
        let entry = self.channels.get(channel_id as usize)?.load_full()?;
        Some(entry.get_stats(channel_id).await)
    }

    /// Get all channel stats
    pub async fn get_all_channel_stats(&self) -> Vec<ChannelStats> {
        let mut stats = Vec::new();
        for (channel_id, slot) in self.channels.iter().enumerate() {
            if let Some(entry) = slot.load_full() {
                stats.push(entry.get_stats(channel_id as u32).await);
            }
        }
        stats
    }

    /// Connect all channels
    pub async fn connect_all_channels(&self) -> Result<()> {
        let mut connect_tasks = Vec::new();

        for (channel_id, slot) in self.channels.iter().enumerate() {
            if let Some(entry) = slot.load_full() {
                let channel_id = channel_id as u32;
                let channel_impl = entry.channel.clone();

                let task = tokio::spawn(async move {
                    match channel_impl.write().await.connect().await {
                        Ok(_) => {
                            // Note: igw TracingLogHandler outputs "Channel connected" at info level
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
            Err(ComSrvError::batch(format!(
                "Failed to connect {} channels",
                failed_channels.len()
            )))
        }
    }

    /// Cleanup all resources
    pub async fn cleanup(&self) -> Result<()> {
        info!("Cleanup started");

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

            // Get existing point IDs from Redis (use cached config)
            let config = voltage_rtdb::KeySpaceConfig::production_cached();
            let point_type: voltage_model::PointType = four_remote;
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

            // Build ChannelPointUpdate vector for missing points (using voltage-routing)
            let updates: Vec<voltage_routing::ChannelPointUpdate> = missing_point_ids
                .iter()
                .map(|point_id| voltage_routing::ChannelPointUpdate {
                    channel_id,
                    point_type,
                    point_id: *point_id,
                    value: 0.0,           // Initialize with 0
                    raw_value: Some(0.0), // Initialize with 0
                    cascade_depth: 0,     // Initial depth for direct writes
                })
                .collect();

            // Call voltage-routing batch write
            voltage_routing::write_channel_batch(rtdb.as_ref(), &self.routing_cache, updates)
                .await
                .map_err(|e| {
                    ComSrvError::storage(format!(
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
    /// Returns (trigger, rx, tx) - tx is for direct command sending
    async fn create_command_trigger(
        &self,
        channel_id: u32,
    ) -> Result<(
        Option<Arc<RwLock<crate::core::channels::trigger::CommandTrigger<R>>>>,
        tokio::sync::mpsc::Receiver<crate::core::channels::traits::ChannelCommand>,
        Option<tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>>,
    )> {
        use crate::core::channels::trigger::{CommandTrigger, CommandTriggerConfig};

        debug!("Ch{} trigger creating", channel_id);

        let config = CommandTriggerConfig {
            channel_id,
            timeout_seconds: 1, // Default BLPOP timeout
        };

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Pass RTDB directly to trigger (works with both RedisRtdb and MemoryRtdb)
        let mut trigger = CommandTrigger::new(config, tx.clone(), self.rtdb.clone()).await?;
        trigger.start().await?;

        debug!("Ch{} trigger created", channel_id);

        // Return tx for direct command sending (bypasses TODO queue)
        Ok((Some(Arc::new(RwLock::new(trigger))), rx, Some(tx)))
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
        let manager: ChannelManager<voltage_rtdb::MemoryRtdb> =
            ChannelManager::new(rtdb, routing_cache);

        assert_eq!(manager.channel_count(), 0);
        assert_eq!(manager.get_channel_ids().len(), 0);
    }

    #[tokio::test]
    async fn test_channel_manager_running_count() {
        let rtdb = create_test_rtdb();
        let routing_cache = create_test_routing_cache();
        let manager: ChannelManager<voltage_rtdb::MemoryRtdb> =
            ChannelManager::new(rtdb, routing_cache);

        let count = manager.running_channel_count().await;
        assert_eq!(count, 0);
    }
}
