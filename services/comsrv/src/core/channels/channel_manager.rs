//! Channel lifecycle management module
//!
//! Handles channel creation, removal, and lifecycle operations

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use arc_swap::ArcSwapOption;
use dashmap::DashSet;
use std::sync::atomic::{AtomicI64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::protocols::core::file_logging::{ChannelFileLogHandler, FileLogLevel};
use crate::protocols::core::logging::{
    ChannelLogConfig, CompositeLogHandler, LogEventType, TracingLogHandler,
};
use crate::protocols::gateway::ChannelRuntime;

/// Maximum number of channel slots (pre-allocated for O(1) access)
/// Channel IDs must be < MAX_CHANNELS
const MAX_CHANNELS: usize = 10000;

use crate::core::channels::converters::convert_to_point_configs;
use crate::core::channels::factory::create_virtual_channel;

#[cfg(feature = "modbus")]
use crate::core::channels::converters::convert_to_modbus_point_configs;
#[cfg(feature = "modbus")]
use crate::core::channels::factory::{create_modbus_channel, create_modbus_rtu_channel};

#[cfg(all(target_os = "linux", feature = "gpio"))]
use crate::core::channels::factory::create_gpio_channel;

#[cfg(all(feature = "can", target_os = "linux"))]
use crate::core::channels::converters::{
    convert_can_to_point_configs, convert_to_can_point_configs,
};
#[cfg(all(feature = "can", target_os = "linux"))]
use crate::core::channels::factory::create_can_channel;
use crate::core::channels::shm_listener::ShmCommandListener;
use crate::core::channels::shm_poller::ShmCommandPoller;
use crate::core::channels::trigger::CommandTrigger;
use crate::core::config::{ChannelConfig, RuntimeChannelConfig};
use crate::error::{ComSrvError, Result};
use crate::store::RedisDataStore;
use voltage_rtdb::{
    ChannelIndex, ChannelToSlotIndex, Rtdb, SlotBitmap, UnifiedReader, UnifiedWriter,
};

// ============================================================================
// Channel Types (merged from channel.rs)
// ============================================================================

/// Channel metadata
#[derive(Debug)]
pub struct ChannelMetadata {
    pub name: Arc<str>,
    pub protocol_type: String,
    pub created_at: Instant,
    /// Last accessed timestamp in milliseconds since Unix epoch (lock-free)
    pub last_accessed_ms: AtomicI64,
}

impl Clone for ChannelMetadata {
    fn clone(&self) -> Self {
        Self {
            name: Arc::clone(&self.name),
            protocol_type: self.protocol_type.clone(),
            created_at: self.created_at,
            last_accessed_ms: AtomicI64::new(self.last_accessed_ms.load(Ordering::Relaxed)),
        }
    }
}

/// Helper function to get current Unix timestamp in milliseconds
fn unix_timestamp_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(e) => {
            warn!("System time error (clock before UNIX epoch?): {}", e);
            0
        },
    }
}

/// Channel entry with integrated protocol runtime and storage
///
/// ## Lock-Free Architecture
///
/// This struct uses message-passing instead of shared locks:
/// - Protocol client is owned by the unified channel task (not shared)
/// - External code sends `ProtocolCommand` via `protocol_tx` channel
/// - The unified task processes commands in its `tokio::select!` loop
/// - Results are returned via embedded `oneshot::Sender`
///
/// This eliminates lock contention between polling and command execution.
#[derive(Clone)]
pub struct ChannelEntry<R: Rtdb> {
    /// Protocol command sender - for connect/disconnect/diagnostics operations
    /// Commands are processed by the unified channel task
    pub protocol_tx: tokio::sync::mpsc::Sender<crate::core::channels::types::ProtocolCommand>,
    /// Data store for persisting polled data
    pub store: Arc<RedisDataStore<R>>,
    /// Unified channel task handle (polling + command execution)
    task_handle: Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
    /// Channel metadata (name, protocol type, etc.)
    pub metadata: ChannelMetadata,
    /// Command trigger for TODO queue integration
    /// NOTE: RwLock is only used during channel removal (non-hot path),
    /// so lock contention is not a concern here.
    pub command_trigger: Option<Arc<RwLock<CommandTrigger<R>>>>,
    /// Channel configuration
    pub channel_config: Arc<ChannelConfig>,
    /// Direct command sender for bypassing TODO queue (business commands)
    pub command_tx:
        Option<tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>>,
    /// Cached connection state for non-blocking access (updated by unified task)
    cached_connection_state: Arc<AtomicU8>,
    /// Cached diagnostics for non-blocking access (updated by unified task after each poll)
    cached_diagnostics: Arc<ArcSwapOption<crate::protocols::core::traits::Diagnostics>>,
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
    /// Last accessed timestamp in milliseconds since Unix epoch
    pub last_accessed_ms: i64,
}

impl<R: Rtdb + 'static> ChannelEntry<R> {
    /// Create new channel entry and start the unified channel task
    ///
    /// This method spawns a background task that owns the protocol client
    /// and processes both polling and commands via `tokio::select!`.
    pub fn new(
        protocol: Box<dyn ChannelRuntime>,
        store: Arc<RedisDataStore<R>>,
        channel_config: Arc<ChannelConfig>,
        protocol_type: String,
        command_trigger: Option<Arc<RwLock<CommandTrigger<R>>>>,
        _command_tx: Option<
            tokio::sync::mpsc::Sender<crate::core::channels::traits::ChannelCommand>,
        >,
        poll_interval_ms: u64,
    ) -> Self {
        let metadata = ChannelMetadata {
            name: Arc::from(channel_config.name()),
            protocol_type,
            created_at: Instant::now(),
            last_accessed_ms: AtomicI64::new(unix_timestamp_ms()),
        };

        let channel_id = channel_config.id();

        // Create protocol command channel (for connect/disconnect/diagnostics)
        let (protocol_tx, protocol_rx) =
            tokio::sync::mpsc::channel::<crate::core::channels::types::ProtocolCommand>(32);

        // Create business command channel (for control/adjustment from TODO queue)
        let (business_tx, business_rx) =
            tokio::sync::mpsc::channel::<crate::core::channels::traits::ChannelCommand>(100);

        // Create shared connection state cache (initialized as Connecting)
        let cached_state = Arc::new(AtomicU8::new(
            crate::core::channels::types::ConnectionState::Connecting.as_u8(),
        ));
        let cached_state_clone = Arc::clone(&cached_state);

        // Create shared diagnostics cache (initialized as None)
        let cached_diagnostics = Arc::new(ArcSwapOption::empty());
        let cached_diagnostics_clone = Arc::clone(&cached_diagnostics);

        // Spawn the unified channel task
        let store_clone = Arc::clone(&store);
        let task_handle = tokio::spawn(async move {
            run_unified_channel_task(
                protocol,
                protocol_rx,
                business_rx,
                store_clone,
                channel_id,
                poll_interval_ms,
                cached_state_clone,
                cached_diagnostics_clone,
            )
            .await;
        });

        Self {
            protocol_tx,
            store,
            task_handle: Arc::new(std::sync::Mutex::new(Some(task_handle))),
            metadata,
            command_trigger,
            channel_config,
            command_tx: Some(business_tx),
            cached_connection_state: cached_state,
            cached_diagnostics,
        }
    }

    /// Get channel statistics
    pub async fn get_stats(&self, channel_id: u32) -> ChannelStats {
        ChannelStats {
            channel_id,
            name: self.metadata.name.to_string(),
            protocol_type: self.metadata.protocol_type.clone(),
            is_connected: self.is_connected(),
            created_at: self.metadata.created_at,
            last_accessed_ms: self.metadata.last_accessed_ms.load(Ordering::Relaxed),
        }
    }

    /// Update last accessed time (lock-free)
    pub fn touch(&self) {
        self.metadata
            .last_accessed_ms
            .store(unix_timestamp_ms(), Ordering::Relaxed);
    }

    /// Check if channel is connected.
    ///
    /// Returns the cached connection state for non-blocking access.
    /// The cache is updated by the unified channel task after each poll cycle.
    pub fn is_connected(&self) -> bool {
        let state_u8 = self.cached_connection_state.load(Ordering::Relaxed);
        crate::core::channels::types::ConnectionState::from_u8(state_u8).is_connected()
    }

    /// Get cached connection state (non-blocking).
    pub fn get_cached_connection_state(&self) -> crate::core::channels::types::ConnectionState {
        let state_u8 = self.cached_connection_state.load(Ordering::Relaxed);
        crate::core::channels::types::ConnectionState::from_u8(state_u8)
    }

    /// Get channel status.
    pub async fn get_status(&self) -> crate::core::channels::types::ChannelStatus {
        crate::core::channels::types::ChannelStatus {
            is_connected: self.is_connected(),
            last_update: chrono::Utc::now().timestamp(),
        }
    }

    /// Get cached diagnostics information (non-blocking).
    ///
    /// Returns the cached diagnostics that is updated by the unified channel task
    /// after each poll cycle. This is safe to call from API handlers without
    /// blocking on slow protocol operations.
    #[allow(clippy::disallowed_methods)]
    pub fn get_diagnostics(&self, channel_id: u32) -> serde_json::Value {
        match self.cached_diagnostics.load().as_deref() {
            Some(d) => serde_json::json!({
                "protocol_type": "unified",
                "connected": d.connection_state.is_connected(),
                "channel_id": channel_id,
                "error_count": d.error_count,
                "last_error": d.last_error,
                "read_count": d.read_count,
                "write_count": d.write_count,
                "protocol": d.protocol,
                "extra": d.extra
            }),
            None => serde_json::json!({
                "protocol_type": "unified",
                "connected": false,
                "channel_id": channel_id,
                "error_count": 0,
                "last_error": null
            }),
        }
    }

    /// Connect the channel
    ///
    /// Sends a Connect command to the unified channel task.
    pub async fn connect(&self) -> crate::error::Result<()> {
        use crate::core::channels::types::ProtocolCommand;
        use std::time::Duration;

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.protocol_tx
            .send(ProtocolCommand::Connect { response_tx })
            .await
            .map_err(|_| crate::error::ComSrvError::channel_not_found(self.channel_config.id()))?;

        // Add 30s timeout to prevent indefinite blocking on connect
        tokio::time::timeout(Duration::from_secs(30), response_rx)
            .await
            .map_err(|_| {
                crate::error::ComSrvError::timeout(format!(
                    "Ch{} connect timeout (30s)",
                    self.channel_config.id()
                ))
            })?
            .map_err(|_| crate::error::ComSrvError::channel_not_found(self.channel_config.id()))?
            .map_err(crate::error::ComSrvError::from)
    }

    /// Disconnect the channel
    ///
    /// Sends a Disconnect command to the unified channel task.
    pub async fn disconnect(&self) -> crate::error::Result<()> {
        use crate::core::channels::types::ProtocolCommand;

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.protocol_tx
            .send(ProtocolCommand::Disconnect { response_tx })
            .await
            .map_err(|_| crate::error::ComSrvError::channel_not_found(self.channel_config.id()))?;

        response_rx
            .await
            .map_err(|_| crate::error::ComSrvError::channel_not_found(self.channel_config.id()))?;

        Ok(())
    }

    /// Get the channel ID from metadata name (parsed from config)
    pub fn channel_id(&self) -> u32 {
        self.channel_config.id()
    }

    /// Shutdown the unified channel task gracefully.
    ///
    /// Sends a Shutdown command to the unified task. The task will process
    /// the command and exit its loop cleanly, allowing proper resource cleanup.
    ///
    /// NOTE: This method does NOT abort the task immediately. Use `abort_task()`
    /// if you need to force-terminate after a timeout.
    pub fn shutdown(&self) {
        use crate::core::channels::types::ProtocolCommand;

        // Send shutdown command (fire-and-forget)
        // The unified task will receive this and break out of its loop
        let _ = self.protocol_tx.try_send(ProtocolCommand::Shutdown);
    }

    /// Force-abort the unified channel task.
    ///
    /// Use this only after `shutdown()` if the task doesn't exit in time.
    /// This is a last resort that may cause resource leaks.
    pub fn abort_task(&self) {
        if let Ok(mut handle) = self.task_handle.lock() {
            if let Some(h) = handle.take() {
                if !h.is_finished() {
                    warn!(
                        "Ch{} task did not exit gracefully, aborting",
                        self.channel_id()
                    );
                    h.abort();
                }
            }
        }
    }

    /// Check if the unified task has finished.
    pub fn is_task_finished(&self) -> bool {
        if let Ok(handle) = self.task_handle.lock() {
            match handle.as_ref() {
                Some(h) => h.is_finished(),
                None => true, // Task was already taken/aborted
            }
        } else {
            true // Lock poisoned, assume finished
        }
    }
}

// ============================================================================
// Unified Channel Task (Lock-Free Design)
// ============================================================================

use crate::core::channels::traits::ChannelCommand;
use crate::core::channels::types::ProtocolCommand;
use crate::protocols::core::traits::PollResult;

/// Run the unified channel task that handles both polling and commands.
///
/// ## Lock-Free Architecture
///
/// This function owns the protocol client exclusively (no shared Mutex).
/// It uses `tokio::select!` to handle multiple event sources:
/// - Timer tick: Execute poll_once() and write data to store
/// - Protocol command: Handle connect/disconnect/diagnostics requests
/// - Business command: Execute write_control/write_adjustment
///
/// This design eliminates lock contention between polling and command execution,
/// reducing command latency from 300ms to <10ms.
#[allow(clippy::too_many_arguments)]
async fn run_unified_channel_task<R: Rtdb>(
    mut protocol: Box<dyn ChannelRuntime>,
    mut protocol_rx: tokio::sync::mpsc::Receiver<ProtocolCommand>,
    mut business_rx: tokio::sync::mpsc::Receiver<ChannelCommand>,
    store: Arc<RedisDataStore<R>>,
    channel_id: u32,
    poll_interval_ms: u64,
    cached_state: Arc<AtomicU8>,
    cached_diagnostics: Arc<ArcSwapOption<crate::protocols::core::traits::Diagnostics>>,
) {
    info!(
        "Ch{} unified task started (interval: {}ms)",
        channel_id, poll_interval_ms
    );

    // Helper to update cached connection state
    let update_cached_state = |state: &dyn ChannelRuntime, cache: &AtomicU8| {
        use crate::protocols::core::traits::ConnectionState as ProtocolConnectionState;
        let protocol_state = state.connection_state();
        let channel_state = match protocol_state {
            ProtocolConnectionState::Connected => {
                crate::core::channels::types::ConnectionState::Connected
            },
            ProtocolConnectionState::Connecting => {
                crate::core::channels::types::ConnectionState::Connecting
            },
            ProtocolConnectionState::Reconnecting => {
                crate::core::channels::types::ConnectionState::Retrying
            },
            ProtocolConnectionState::Disconnected => {
                crate::core::channels::types::ConnectionState::Disconnected
            },
            ProtocolConnectionState::Error => crate::core::channels::types::ConnectionState::Failed,
        };
        cache.store(channel_state.as_u8(), Ordering::Relaxed);
    };

    // Wait a bit for the connection to be established
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Update initial connection state
    update_cached_state(protocol.as_ref(), &cached_state);

    // Use configured poll interval
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(poll_interval_ms));

    // Track previous error count to detect new errors
    let mut prev_error_count: u64 = 0;

    loop {
        // Use biased select to prioritize commands over polling
        // This ensures commands are processed promptly even during heavy polling
        tokio::select! {
            biased;

            // Priority 1: Protocol commands (connect/disconnect/diagnostics)
            Some(cmd) = protocol_rx.recv() => {
                match cmd {
                    ProtocolCommand::WriteControl { internal_id, value, response_tx } => {
                        let result = protocol.write_control(&[(internal_id, value)]).await;
                        let _ = response_tx.send(result);
                    }
                    ProtocolCommand::WriteAdjustment { internal_id, value, response_tx } => {
                        let result = protocol.write_adjustment(&[(internal_id, value)]).await;
                        let _ = response_tx.send(result);
                    }
                    ProtocolCommand::Connect { response_tx } => {
                        let result = protocol.connect().await;
                        let _ = response_tx.send(result);
                    }
                    ProtocolCommand::Disconnect { response_tx } => {
                        let _ = protocol.disconnect().await;
                        let _ = response_tx.send(());
                    }
                    ProtocolCommand::GetDiagnostics { response_tx } => {
                        let diag = protocol.diagnostics().await.ok();
                        let _ = response_tx.send(diag);
                    }
                    ProtocolCommand::GetConnectionState { response_tx } => {
                        // Get actual connection state from protocol
                        use crate::protocols::core::traits::ConnectionState as ProtocolConnectionState;
                        let protocol_state = protocol.connection_state();
                        // Map protocol state to channel state
                        let state = match protocol_state {
                            ProtocolConnectionState::Connected => {
                                crate::core::channels::types::ConnectionState::Connected
                            }
                            ProtocolConnectionState::Connecting => {
                                crate::core::channels::types::ConnectionState::Connecting
                            }
                            ProtocolConnectionState::Reconnecting => {
                                crate::core::channels::types::ConnectionState::Retrying
                            }
                            ProtocolConnectionState::Disconnected => {
                                crate::core::channels::types::ConnectionState::Disconnected
                            }
                            ProtocolConnectionState::Error => {
                                crate::core::channels::types::ConnectionState::Failed
                            }
                        };
                        let _ = response_tx.send(state);
                    }
                    ProtocolCommand::Shutdown => {
                        info!("Ch{} received shutdown command", channel_id);
                        break;
                    }
                }
            }

            // Priority 2: Business commands (control/adjustment from TODO queue)
            Some(cmd) = business_rx.recv() => {
                match cmd {
                    ChannelCommand::Control { point_id, value, .. } => {
                        // Use original point_id - protocol filters by point_type
                        match protocol.write_control(&[(point_id, value)]).await {
                            Ok(success_count) => {
                                if success_count > 0 {
                                    debug!("Ch{} control pt{} = {} ok", channel_id, point_id, value);
                                } else {
                                    warn!("Ch{} control pt{} = {} failed", channel_id, point_id, value);
                                }
                            }
                            Err(e) => {
                                error!("Ch{} control pt{} err: {}", channel_id, point_id, e);
                            }
                        }
                    }
                    ChannelCommand::Adjustment { point_id, value, .. } => {
                        // Use original point_id - protocol filters by point_type
                        match protocol.write_adjustment(&[(point_id, value)]).await {
                            Ok(success_count) => {
                                if success_count > 0 {
                                    debug!("Ch{} adjustment pt{} = {} ok", channel_id, point_id, value);
                                } else {
                                    warn!("Ch{} adjustment pt{} = {} failed", channel_id, point_id, value);
                                }
                            }
                            Err(e) => {
                                error!("Ch{} adjustment pt{} err: {}", channel_id, point_id, e);
                            }
                        }
                    }
                }
            }

            // Priority 3: Periodic polling
            _ = interval.tick() => {
                // Poll data using ChannelRuntime interface
                let result: PollResult = protocol.poll_once().await;

                // Log partial failures from poll result
                let failure_count = result.failures.len();
                if failure_count > 0 {
                    warn!(
                        "Ch{} partial read failure: {} points failed",
                        channel_id, failure_count
                    );
                }

                let count = result.data.len();
                if count > 0 {
                    debug!("Ch{} polling got {} data points", channel_id, count);
                    if let Err(e) = store.write_batch(channel_id, result.data).await {
                        error!("Ch{} failed to write to Redis: {}", channel_id, e);
                    }
                }

                // Check diagnostics for accumulated errors and update cache
                if let Ok(diag) = protocol.diagnostics().await {
                    if diag.error_count > prev_error_count {
                        let new_errors = diag.error_count - prev_error_count;
                        warn!(
                            "Ch{} accumulated errors: {} new errors, last error: {:?}",
                            channel_id, new_errors, diag.last_error
                        );
                        prev_error_count = diag.error_count;
                    }
                    // Update cached diagnostics for non-blocking access
                    cached_diagnostics.store(Some(Arc::new(diag)));
                }

                // Update cached connection state after each poll cycle
                update_cached_state(protocol.as_ref(), &cached_state);
            }
        }
    }

    // Mark as disconnected on shutdown
    cached_state.store(
        crate::core::channels::types::ConnectionState::Disconnected.as_u8(),
        Ordering::Relaxed,
    );
    info!("Ch{} unified task stopped", channel_id);
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
    /// Active channel ID index for O(1) iteration (avoids O(10000) full scan)
    /// Synchronized with channels: insert on create_channel, remove on remove_channel
    active_channel_ids: DashSet<u32>,
    /// Shared RTDB (Redis or Memory for testing)
    rtdb: Arc<R>,
    /// Routing cache for C2M/M2C routing (public for reload operations)
    pub routing_cache: Arc<voltage_rtdb::RoutingCache>,
    /// SQLite connection pool for configuration loading
    sqlite_pool: Option<sqlx::SqlitePool>,
    /// Shared memory writer for zero-copy cross-process data sharing (optional)
    shared_writer: Option<Arc<UnifiedWriter>>,
    /// Pre-computed channel → slot mapping for O(1) shared memory writes (optional)
    channel_index: Option<Arc<ChannelToSlotIndex>>,
    /// Command TX cache for O(1) hot path access
    /// Shared with AppState for direct API access bypassing RwLock
    command_tx_cache: Option<Arc<crate::api::command_cache::CommandTxCache>>,

    // ========== Dynamic Slot Allocation ==========
    /// Dynamic channel index for unified pool architecture (optional)
    /// Supports hot add/remove with ArcSwap for lock-free reads
    dynamic_channel_index: Option<Arc<ChannelIndex>>,
    /// Slot bitmap for dynamic allocation (optional, requires RwLock for &mut access)
    slot_bitmap: Option<Arc<std::sync::RwLock<SlotBitmap>>>,

    // ========== SHM Command Polling (M2C via SHM) ==========
    /// Shared memory reader for polling C/A timestamps (optional)
    unified_reader: Option<Arc<UnifiedReader>>,
    /// SHM command poller for detecting M2C commands (optional, replaces TODO queue)
    shm_poller: Option<Arc<ShmCommandPoller>>,
    /// Shutdown signal for SHM poller
    shm_poller_shutdown: Option<tokio::sync::watch::Sender<bool>>,

    // ========== SHM Command Listener (Event-driven M2C) ==========
    /// SHM command listener for event-driven M2C command dispatch (optional)
    /// Replaces polling with UDS event-driven architecture (~1-2ms latency)
    shm_listener: Option<Arc<ShmCommandListener>>,
}

impl<R: Rtdb> std::fmt::Debug for ChannelManager<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelManager")
            .field("channels", &self.channel_count())
            .finish()
    }
}

/// Get the base directory for channel log files.
/// Uses VOLTAGE_LOG_DIR environment variable if set, otherwise falls back to "/app/logs".
fn get_channel_log_base_dir() -> String {
    let base = std::env::var("VOLTAGE_LOG_DIR").unwrap_or_else(|_| "/app/logs".to_string());
    format!("{}/comsrv/channels", base)
}

impl<R: Rtdb + 'static> ChannelManager<R> {
    /// Pre-allocate channel slots for O(1) access
    #[inline]
    fn create_channel_slots() -> Vec<ArcSwapOption<ChannelEntry<R>>> {
        (0..MAX_CHANNELS).map(|_| ArcSwapOption::empty()).collect()
    }

    /// Configure logging for a channel based on ChannelLoggingConfig.
    ///
    /// Sets up both tracing and file logging handlers when enabled.
    fn configure_channel_logging(
        protocol: &mut Box<dyn ChannelRuntime>,
        channel_id: u32,
        channel_name: &str,
        logging_config: &crate::core::config::ChannelLoggingConfig,
    ) {
        // Create composite handler with tracing
        let mut composite = CompositeLogHandler::new().with_handler(Arc::new(TracingLogHandler));

        // Add file logging if enabled
        if logging_config.enabled {
            let level = FileLogLevel::parse(logging_config.level.as_deref());
            let log_dir = get_channel_log_base_dir();

            let file_handler = ChannelFileLogHandler::new(&log_dir)
                .with_level(level)
                .with_channel(channel_id, channel_name);

            composite.add_handler(Arc::new(file_handler));

            info!(
                "Ch{} file logging enabled (level={:?}, dir={})",
                channel_id, level, log_dir
            );
        }

        // Set the composite handler
        protocol.set_log_handler(Arc::new(composite));

        // Configure log config based on logging level
        let log_config = if logging_config.enabled {
            let level = logging_config.level.as_deref().unwrap_or("info");
            if level.eq_ignore_ascii_case("debug") {
                // Debug: log everything including raw packets
                ChannelLogConfig::all()
            } else {
                // Info: log raw packets + errors + connections
                ChannelLogConfig::new()
                    .with_raw_packets(true)
                    .enable_event(LogEventType::RawPacket)
            }
        } else {
            // Disabled: default config (no raw packets)
            ChannelLogConfig::default()
        };

        protocol.set_log_config(log_config);
    }

    /// Create new channel manager
    pub fn new(rtdb: Arc<R>, routing_cache: Arc<voltage_rtdb::RoutingCache>) -> Self {
        Self {
            channels: Self::create_channel_slots(),
            active_channel_ids: DashSet::new(),
            rtdb,
            routing_cache,
            sqlite_pool: None,
            shared_writer: None,
            channel_index: None,
            command_tx_cache: None,
            dynamic_channel_index: None,
            slot_bitmap: None,
            unified_reader: None,
            shm_poller: None,
            shm_poller_shutdown: None,
            shm_listener: None,
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
            active_channel_ids: DashSet::new(),
            rtdb,
            routing_cache,
            sqlite_pool: Some(sqlite_pool),
            shared_writer: None,
            channel_index: None,
            command_tx_cache: None,
            dynamic_channel_index: None,
            slot_bitmap: None,
            unified_reader: None,
            shm_poller: None,
            shm_poller_shutdown: None,
            shm_listener: None,
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
        shared_writer: Option<Arc<UnifiedWriter>>,
        channel_index: Option<Arc<ChannelToSlotIndex>>,
        command_tx_cache: Option<Arc<crate::api::command_cache::CommandTxCache>>,
    ) -> Self {
        Self {
            channels: Self::create_channel_slots(),
            active_channel_ids: DashSet::new(),
            rtdb,
            routing_cache,
            sqlite_pool: Some(sqlite_pool),
            shared_writer,
            channel_index,
            command_tx_cache,
            dynamic_channel_index: None,
            slot_bitmap: None,
            unified_reader: None,
            shm_poller: None,
            shm_poller_shutdown: None,
            shm_listener: None,
        }
    }

    /// Configure dynamic slot allocation (optional feature)
    ///
    /// Call this after construction to enable unified pool architecture.
    /// If not called, dynamic features are disabled (backward compatible).
    ///
    /// # Arguments
    /// * `dynamic_index` - ChannelIndex for dynamic channel management
    /// * `slot_bitmap` - SlotBitmap for slot allocation (wrapped in RwLock)
    pub fn with_dynamic_allocation(
        mut self,
        dynamic_index: Arc<ChannelIndex>,
        slot_bitmap: Arc<std::sync::RwLock<SlotBitmap>>,
    ) -> Self {
        self.dynamic_channel_index = Some(dynamic_index);
        self.slot_bitmap = Some(slot_bitmap);
        self
    }

    /// Configure SHM command poller for M2C commands via shared memory
    ///
    /// This enables polling Control/Adjustment points from SHM instead of Redis TODO queue.
    /// The SHM poller runs as a complement to the existing TODO queue (Redis as fallback).
    ///
    /// # Arguments
    /// * `unified_reader` - Shared memory reader for polling C/A timestamps
    /// * `poll_interval` - Polling interval (default: 20ms)
    pub fn with_shm_poller(
        mut self,
        unified_reader: Arc<UnifiedReader>,
        poll_interval: std::time::Duration,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let poller = ShmCommandPoller::new(
            unified_reader.clone(),
            &self.routing_cache,
            poll_interval,
            shutdown_rx,
        );

        self.unified_reader = Some(unified_reader);
        self.shm_poller = Some(Arc::new(poller));
        self.shm_poller_shutdown = Some(shutdown_tx);
        self
    }

    /// Start the SHM command poller background task
    ///
    /// Call this after all channels are created to start polling.
    /// Returns a JoinHandle that can be used to await completion.
    pub fn start_shm_poller(&self) -> Option<tokio::task::JoinHandle<()>> {
        let poller = self.shm_poller.clone()?;
        Some(tokio::spawn(async move {
            poller.run().await;
        }))
    }

    /// Get SHM poller for channel registration (internal use)
    pub fn shm_poller(&self) -> Option<&Arc<ShmCommandPoller>> {
        self.shm_poller.as_ref()
    }

    /// Configure SHM command listener for event-driven M2C dispatch
    ///
    /// This enables UDS event-driven notifications instead of polling.
    /// The listener receives notifications from modsrv when C/A points are written.
    /// Latency: ~1-2ms vs polling's 10-20ms average.
    ///
    /// # Arguments
    /// * `unified_reader` - Shared memory reader for reading C/A values
    /// * `shutdown_rx` - Watch receiver for graceful shutdown
    pub fn with_shm_listener(
        mut self,
        unified_reader: Arc<UnifiedReader>,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        let listener = ShmCommandListener::new(unified_reader.clone(), None, shutdown_rx);

        self.unified_reader = Some(unified_reader);
        self.shm_listener = Some(Arc::new(listener));
        self
    }

    /// Start the SHM command listener background task
    ///
    /// Call this after all channels are created to start listening.
    /// Returns a JoinHandle that can be used to await completion.
    pub fn start_shm_listener(&self) -> Option<tokio::task::JoinHandle<std::io::Result<()>>> {
        let listener = self.shm_listener.clone()?;
        Some(tokio::spawn(async move { listener.run().await }))
    }

    /// Get SHM listener for channel registration (internal use)
    pub fn shm_listener(&self) -> Option<&Arc<ShmCommandListener>> {
        self.shm_listener.as_ref()
    }

    /// Get dynamic ChannelIndex (for external access, e.g., API stats)
    pub fn dynamic_channel_index(&self) -> Option<&Arc<ChannelIndex>> {
        self.dynamic_channel_index.as_ref()
    }

    /// Get SlotBitmap stats (for monitoring)
    pub fn slot_bitmap_stats(&self) -> Option<voltage_rtdb::BitmapStats> {
        self.slot_bitmap
            .as_ref()
            .and_then(|b| b.read().ok().map(|guard| guard.stats()))
    }

    /// Create channel
    ///
    /// Returns an Arc to the created ChannelEntry for convenience.
    pub async fn create_channel(
        &self,
        channel_config: Arc<ChannelConfig>,
    ) -> Result<Arc<ChannelEntry<R>>> {
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

        // Branch based on protocol type - create ChannelEntry directly
        let entry = match protocol_name.as_str() {
            "virtual" => {
                // Protocol path: Use protocols::VirtualChannel with RedisDataStore
                self.create_virtual_channel_impl(channel_id, &runtime_config, base_config)
                    .await?
            },
            #[cfg(feature = "modbus")]
            "modbus_tcp" => {
                // Protocol path: Use protocols::ModbusChannel (TCP) with RedisDataStore
                self.create_modbus_channel_impl(channel_id, &runtime_config, base_config)
                    .await?
            },
            #[cfg(feature = "modbus")]
            "modbus_rtu" => {
                // Protocol path: Use protocols::ModbusChannel (RTU/serial) with RedisDataStore
                self.create_modbus_rtu_channel_impl(channel_id, &runtime_config, base_config)
                    .await?
            },
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            "gpio" | "di_do" | "dido" => {
                // Protocol path: Use protocols::GpioChannel for DI/DO
                self.create_gpio_channel_impl(channel_id, &runtime_config, base_config)
                    .await?
            },
            #[cfg(all(feature = "can", target_os = "linux"))]
            "can" => {
                // Protocol path: Use protocols::CanClient with RedisDataStore
                self.create_can_channel_impl(channel_id, &runtime_config, base_config)
                    .await?
            },
            _ => {
                // All protocols now use unified path - unsupported protocols should error
                // Base protocols available on all platforms
                #[allow(unused_mut)] // mut needed for cfg-conditional push_str
                let mut supported = String::from("virtual");

                #[cfg(feature = "modbus")]
                supported.push_str(", modbus_tcp, modbus_rtu");

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

        let entry = Arc::new(entry);

        // IMPORTANT: Order matters for cache consistency!
        // 1. First: atomic store (publish channel to be visible)
        slot.store(Some(Arc::clone(&entry)));

        // 2. Then: register command_tx with cache (channel is now visible)
        // This order prevents a race where cache lookup succeeds but slot is empty
        if let (Some(ref cache), Some(ref tx)) = (&self.command_tx_cache, &entry.command_tx) {
            cache.register(channel_id, tx.clone());
        }

        // 2.5: Register with SHM listener for event-driven M2C dispatch
        if let (Some(ref listener), Some(ref tx)) = (&self.shm_listener, &entry.command_tx) {
            listener.register_channel(channel_id, tx.clone());
            debug!(
                "Ch{} registered with ShmListener for event-driven dispatch",
                channel_id
            );
        }

        // 3. Finally: register in active channel index for O(1) iteration
        self.active_channel_ids.insert(channel_id);

        // 4. Dynamic Slot Allocation: Add channel to ChannelIndex
        if let (Some(index), Some(bitmap)) = (&self.dynamic_channel_index, &self.slot_bitmap) {
            let type_counts = [
                runtime_config.telemetry_points.len() as u32,
                runtime_config.signal_points.len() as u32,
                runtime_config.control_points.len() as u32,
                runtime_config.adjustment_points.len() as u32,
            ];
            let total: u32 = type_counts.iter().sum();
            if total > 0 {
                match bitmap.write() {
                    Ok(mut bitmap_guard) => {
                        match index.add_channel(channel_id, type_counts, &mut bitmap_guard) {
                            Ok(layout) => {
                                debug!(
                                    "Ch{} slot allocated: base={}, total={}",
                                    channel_id, layout.base_slot, layout.total_points
                                );
                            },
                            Err(e) => {
                                // Log warning but don't fail - dynamic allocation is optional
                                warn!("Ch{} slot allocation failed: {}", channel_id, e);
                            },
                        }
                    },
                    Err(e) => {
                        warn!("Ch{} bitmap lock poisoned: {}", channel_id, e);
                    },
                }
            }
        }

        // 5. SHM Poller Registration: Register command_tx for M2C via SHM
        // This allows ShmCommandPoller to send commands directly to the channel
        if let (Some(ref poller), Some(ref tx)) = (&self.shm_poller, &entry.command_tx) {
            poller.register_channel(channel_id, tx.clone());
            debug!("Ch{} registered with SHM poller", channel_id);
        }

        info!("Ch{} created ({})", channel_id, protocol_name);
        Ok(entry)
    }

    /// Create virtual channel entry.
    ///
    /// Uses protocols::VirtualChannel with RedisDataStore for data persistence.
    async fn create_virtual_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating virtual channel", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert point configs and register with store
        let point_configs = convert_to_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Create VirtualChannel protocol
        let mut protocol = create_virtual_channel(channel_id, runtime_config.name(), point_configs);

        // 5. Configure channel logging
        Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        // 6. Setup command trigger for M2C control
        // Note: command_rx and command_tx are unused because ChannelEntry::new creates its own channels
        let (command_trigger, _command_rx, _command_tx) =
            self.create_command_trigger(channel_id).await?;

        // 7. Get poll interval from config
        let poll_interval_ms = runtime_config
            .base
            .parameters
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        // 7. Create ChannelEntry with protocol and store (spawns unified task)
        let entry = ChannelEntry::new(
            protocol,
            store,
            base_config,
            "virtual".to_string(),
            command_trigger,
            None, // command_tx is created internally by ChannelEntry::new
            poll_interval_ms,
        );

        info!("Ch{} created (virtual)", channel_id);
        Ok(entry)
    }

    /// Create Modbus TCP channel entry.
    ///
    /// Uses protocols::ModbusChannel with RedisDataStore for data persistence.
    /// Includes batch read optimization, auto-reconnect, and zero-data detection.
    #[cfg(feature = "modbus")]
    async fn create_modbus_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating Modbus TCP channel", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert Modbus point configs and register with store
        let point_configs = convert_to_modbus_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Extract host/port from runtime config parameters
        let params = &runtime_config.base.parameters;
        let host = params
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                info!(
                    "Ch{} host not configured, using default: 127.0.0.1",
                    channel_id
                );
                "127.0.0.1"
            });
        let port = params
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|n| n as u16)
            .unwrap_or_else(|| {
                info!("Ch{} port not configured, using default: 502", channel_id);
                502
            });

        // 4b. Extract I/O timeout from config (UI parameter name: read_timeout_ms)
        let io_timeout_ms = params.get("read_timeout_ms").and_then(|v| v.as_u64());
        if let Some(timeout) = io_timeout_ms {
            debug!("Ch{} using read_timeout_ms: {}ms", channel_id, timeout);
        }

        // 5. Create ModbusChannel protocol
        let mut protocol =
            create_modbus_channel(channel_id, host, port, point_configs, io_timeout_ms);

        // 6. Configure channel logging
        Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        // 7. Setup command trigger for M2C control
        let (command_trigger, _command_rx, _command_tx) =
            self.create_command_trigger(channel_id).await?;

        // 8. Get poll interval from config
        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        // 9. Create ChannelEntry with protocol and store (spawns unified task)
        let entry = ChannelEntry::new(
            protocol,
            store,
            base_config,
            "modbus_tcp".to_string(),
            command_trigger,
            None, // command_tx is created internally by ChannelEntry::new
            poll_interval_ms,
        );

        info!("Ch{} created (modbus_tcp)", channel_id);
        Ok(entry)
    }

    /// Create Modbus RTU (serial) channel entry.
    ///
    /// Uses protocols::ModbusChannel in RTU mode with RedisDataStore for data persistence.
    /// Includes batch read optimization, auto-reconnect, and zero-data detection.
    #[cfg(feature = "modbus")]
    async fn create_modbus_rtu_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating Modbus RTU channel", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert Modbus point configs and register with store
        let point_configs = convert_to_modbus_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Extract device/baud_rate from runtime config parameters
        let params = &runtime_config.base.parameters;
        let device = params
            .get("device")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                info!(
                    "Ch{} device not configured, using default: /dev/ttyUSB0",
                    channel_id
                );
                "/dev/ttyUSB0"
            });
        let baud_rate = params
            .get("baud_rate")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or_else(|| {
                info!(
                    "Ch{} baud_rate not configured, using default: 9600",
                    channel_id
                );
                9600
            });

        // 4b. Extract I/O timeout from config (UI parameter name: read_timeout_ms)
        let io_timeout_ms = params.get("read_timeout_ms").and_then(|v| v.as_u64());
        if let Some(timeout) = io_timeout_ms {
            debug!("Ch{} using read_timeout_ms: {}ms", channel_id, timeout);
        }

        // 5. Create ModbusChannel (RTU) protocol
        let mut protocol =
            create_modbus_rtu_channel(channel_id, device, baud_rate, point_configs, io_timeout_ms);

        // 6. Configure channel logging
        Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        // 7. Setup command trigger for M2C control
        let (command_trigger, _command_rx, _command_tx) =
            self.create_command_trigger(channel_id).await?;

        // 8. Get poll interval from config
        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        // 9. Create ChannelEntry with protocol and store (spawns unified task)
        let entry = ChannelEntry::new(
            protocol,
            store,
            base_config,
            "modbus_rtu".to_string(),
            command_trigger,
            None, // command_tx is created internally by ChannelEntry::new
            poll_interval_ms,
        );

        info!("Ch{} created (modbus_rtu)", channel_id);
        Ok(entry)
    }

    /// Create GPIO channel entry for DI/DO.
    ///
    /// GPIO channels support:
    /// - Signal points (DI): Digital input reading
    /// - Control points (DO): Digital output control
    #[cfg(all(target_os = "linux", feature = "gpio"))]
    async fn create_gpio_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating GPIO channel", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert point configs and register with store
        let point_configs = convert_to_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs);

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Create GpioChannel protocol
        let mut protocol = create_gpio_channel(channel_id, runtime_config);

        // 5. Configure channel logging
        Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        // 6. Setup command trigger for M2C control (DO commands)
        let (command_trigger, _command_rx, _command_tx) =
            self.create_command_trigger(channel_id).await?;

        // 7. Get poll interval from config
        // GPIO needs faster polling (default 200ms for responsive DI detection)
        let poll_interval_ms = runtime_config
            .base
            .parameters
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(200);

        // 7. Create ChannelEntry with protocol and store (spawns unified task)
        let entry = ChannelEntry::new(
            protocol,
            store,
            base_config,
            "gpio".to_string(),
            command_trigger,
            None, // command_tx is created internally by ChannelEntry::new
            poll_interval_ms,
        );

        info!("Ch{} created (gpio)", channel_id);
        Ok(entry)
    }

    /// Create CAN channel entry.
    ///
    /// Uses protocols::CanClient with RedisDataStore for data persistence.
    /// CAN protocol is event-driven and read-only (no M2C control).
    #[cfg(all(feature = "can", target_os = "linux"))]
    async fn create_can_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating CAN channel", channel_id);

        // 1. Create RedisDataStore for this channel (with optional shared memory)
        let store = self.create_data_store();

        // 2. Convert CAN mappings to formats
        let can_point_configs = convert_to_can_point_configs(runtime_config);
        let point_configs = convert_can_to_point_configs(runtime_config);

        if can_point_configs.is_empty() {
            warn!("Ch{} has no CAN point mappings configured", channel_id);
        }

        store.set_point_configs(channel_id, point_configs);

        // 3. Start background flush task for write buffer
        store.start_flush_task().await;

        // 4. Extract CAN interface from runtime config parameters
        let params = &runtime_config.base.parameters;
        let can_interface = params
            .get("device")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                info!(
                    "Ch{} CAN device not configured, using default: can0",
                    channel_id
                );
                "can0"
            });

        // 5. Create CanClient protocol
        let mut protocol = create_can_channel(channel_id, can_interface, can_point_configs);

        // 6. Configure channel logging
        Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        // 7. Setup command trigger (CAN is read-only, but we still create the trigger for consistency)
        let (command_trigger, _command_rx, _command_tx) =
            self.create_command_trigger(channel_id).await?;

        // 8. Get poll interval from config
        // CAN is event-driven, needs faster polling (default 200ms)
        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(200);

        // 8. Create ChannelEntry with protocol and store (spawns unified task)
        let entry = ChannelEntry::new(
            protocol,
            store,
            base_config,
            "can".to_string(),
            command_trigger,
            None, // command_tx is created internally by ChannelEntry::new
            poll_interval_ms,
        );

        info!("Ch{} created (can)", channel_id);
        Ok(entry)
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

    /// Remove channel with graceful shutdown.
    ///
    /// Performs proper cleanup in the following order:
    /// 1. Unregister from external systems (cache, SHM poller/listener)
    /// 2. Send shutdown signal to unified task
    /// 3. Wait for task to exit gracefully (with timeout)
    /// 4. Shutdown store to flush WriteBuffer
    /// 5. Disconnect channel
    /// 6. Stop command trigger
    /// 7. Deallocate dynamic slots
    pub async fn remove_channel(&self, channel_id: u32) -> Result<()> {
        // Unregister from cache before removing channel
        if let Some(ref cache) = self.command_tx_cache {
            cache.unregister(channel_id);
        }

        // Unregister from SHM poller
        if let Some(ref poller) = self.shm_poller {
            poller.unregister_channel(channel_id);
        }

        // Unregister from SHM listener (event-driven M2C)
        if let Some(ref listener) = self.shm_listener {
            listener.unregister_channel(channel_id);
        }

        // Remove from active channel index
        self.active_channel_ids.remove(&channel_id);

        // O(1) atomic swap
        let slot = self
            .channels
            .get(channel_id as usize)
            .ok_or_else(|| ComSrvError::invalid_channel_id(channel_id))?;

        if let Some(entry) = slot.swap(None) {
            // 1. Send shutdown signal to unified task (non-blocking)
            entry.shutdown();

            // 2. Wait for task to exit gracefully (up to 500ms)
            //    This allows the protocol to clean up file handles, flush buffers, etc.
            let max_wait = std::time::Duration::from_millis(500);
            let start = std::time::Instant::now();
            while !entry.is_task_finished() && start.elapsed() < max_wait {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            // 3. Force-abort if task didn't exit in time
            if !entry.is_task_finished() {
                entry.abort_task();
            }

            // 4. Shutdown store to flush WriteBuffer to Redis
            //    This is CRITICAL: ensures polled data is persisted before channel removal
            entry.store.shutdown().await;

            // 5. Disconnect channel
            let _ = entry.disconnect().await;

            // 6. Stop command trigger if exists
            if let Some(trigger_arc) = &entry.command_trigger {
                let mut trigger = trigger_arc.write().await;
                let _ = trigger.stop().await;
            }

            // 7. Dynamic Slot Deallocation: Remove channel from ChannelIndex and free slots
            if let (Some(index), Some(bitmap)) = (&self.dynamic_channel_index, &self.slot_bitmap) {
                match bitmap.write() {
                    Ok(mut bitmap_guard) => {
                        match index.remove_channel(channel_id, &mut bitmap_guard) {
                            Ok(layout) => {
                                debug!(
                                    "Ch{} slot freed: base={}, count={}",
                                    channel_id, layout.base_slot, layout.total_points
                                );
                            },
                            Err(e) => {
                                // Log warning but don't fail - dynamic allocation is optional
                                warn!("Ch{} slot deallocation failed: {}", channel_id, e);
                            },
                        }
                    },
                    Err(e) => {
                        warn!("Ch{} bitmap lock poisoned: {}", channel_id, e);
                    },
                }
            }

            info!("Ch{} removed (graceful shutdown)", channel_id);
            Ok(())
        } else {
            Err(ComSrvError::channel_not_found(channel_id))
        }
    }

    /// Get channel entry by ID
    ///
    /// Returns the full ChannelEntry Arc for accessing protocol, store, and command_tx.
    ///
    /// # O(1) lock-free access (~5ns)
    /// Returns `Arc<ChannelEntry>` instead of DashMap Ref (no lifetime constraints)
    #[inline]
    pub fn get_channel(&self, channel_id: u32) -> Option<Arc<ChannelEntry<R>>> {
        self.channels.get(channel_id as usize)?.load_full()
    }

    /// Alias for get_channel (for backwards compatibility)
    #[inline]
    pub fn get_channel_entry(&self, channel_id: u32) -> Option<Arc<ChannelEntry<R>>> {
        self.get_channel(channel_id)
    }

    /// Get channel IDs
    ///
    /// # O(n) where n = active channels (not 10000 slots)
    /// Uses active_channel_ids index for efficient iteration
    pub fn get_channel_ids(&self) -> Vec<u32> {
        self.active_channel_ids.iter().map(|id| *id).collect()
    }

    /// Get channel count
    ///
    /// # O(1) using active_channel_ids index
    pub fn channel_count(&self) -> usize {
        self.active_channel_ids.len()
    }

    /// Get running channel count
    ///
    /// # O(n) where n = active channels (not 10000 slots)
    /// Uses active_channel_ids index for efficient iteration
    pub async fn running_channel_count(&self) -> usize {
        let mut count = 0;
        for channel_id in self.active_channel_ids.iter() {
            if let Some(entry) = self
                .channels
                .get(*channel_id as usize)
                .and_then(|s| s.load_full())
            {
                if entry.is_connected() {
                    count += 1;
                }
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
    ///
    /// # O(n) where n = active channels (not 10000 slots)
    /// Uses active_channel_ids index for efficient iteration
    pub async fn get_all_channel_stats(&self) -> Vec<ChannelStats> {
        let mut stats = Vec::with_capacity(self.active_channel_ids.len());
        for channel_id in self.active_channel_ids.iter() {
            let id = *channel_id;
            if let Some(entry) = self.channels.get(id as usize).and_then(|s| s.load_full()) {
                stats.push(entry.get_stats(id).await);
            }
        }
        stats
    }

    /// Connect all channels
    ///
    /// # O(n) where n = active channels (not 10000 slots)
    /// Uses active_channel_ids index for efficient iteration
    pub async fn connect_all_channels(&self) -> Result<()> {
        let mut connect_tasks = Vec::with_capacity(self.active_channel_ids.len());

        for channel_id_ref in self.active_channel_ids.iter() {
            let channel_id = *channel_id_ref;
            if let Some(entry) = self
                .channels
                .get(channel_id as usize)
                .and_then(|s| s.load_full())
            {
                let entry_clone = Arc::clone(&entry);

                let task = tokio::spawn(async move {
                    match entry_clone.connect().await {
                        Ok(_) => {
                            // Note: TracingLogHandler outputs "Channel connected" at info level
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

        // Stop SHM poller first
        if let Some(ref shutdown_tx) = self.shm_poller_shutdown {
            let _ = shutdown_tx.send(true);
            debug!("SHM poller shutdown signal sent");
        }

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
            let key_exists = match rtdb.exists(&channel_key).await {
                Ok(exists) => exists,
                Err(e) => {
                    warn!(
                        "Ch{} Redis exists check failed for {}: {}",
                        channel_id, channel_key, e
                    );
                    false
                },
            };
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

        // 缓冲区从 100 增大到 1000，提供更好的背压容量
        // 当队列接近满时，ShmListener 会记录 error 日志
        let (tx, rx) = tokio::sync::mpsc::channel(1000);

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
