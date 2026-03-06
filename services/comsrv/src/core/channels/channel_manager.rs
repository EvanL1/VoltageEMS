//! Channel lifecycle management module
//!
//! Handles channel creation, removal, and lifecycle operations.
//! Channel entry types are in `channel_entry`, task logic in `channel_task`,
//! and creation/factory methods in `channel_creation`.

use arc_swap::ArcSwapOption;
use dashmap::DashSet;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::core::channels::channel_entry::{ChannelEntry, ChannelStats, MAX_CHANNELS};
use crate::core::channels::shm_listener::ShmCommandListener;
use crate::core::channels::shm_poller::ShmCommandPoller;
use crate::error::{ComSrvError, Result};
use voltage_rtdb::Rtdb;
use voltage_rtdb_shm::{ChannelIndex, ShmHandle, SlotBitmap, UnifiedReader};

// Re-export types for backwards compatibility
pub use crate::core::channels::channel_entry::{unix_timestamp_ms, ChannelMetadata};

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
    /// Index = channel_id, value = `Option<Arc<ChannelEntry>>`
    pub(super) channels: Vec<ArcSwapOption<ChannelEntry<R>>>,
    /// Active channel ID index for O(1) iteration (avoids O(10000) full scan)
    /// Synchronized with channels: insert on create_channel, remove on remove_channel
    pub(super) active_channel_ids: DashSet<u32>,
    /// Shared RTDB (Redis or Memory for testing)
    pub(super) rtdb: Arc<R>,
    /// Routing cache for C2M/M2C routing (public for reload operations)
    pub routing_cache: Arc<voltage_routing::RoutingCache>,
    /// SQLite connection pool for configuration loading
    pub(super) sqlite_pool: Option<sqlx::SqlitePool>,
    /// Runtime-swappable shared memory handle (writer + index, rebuilt on routing reload)
    pub(super) shm_handle: Option<Arc<ShmHandle>>,
    /// Command TX cache for O(1) hot path access
    /// Shared with AppState for direct API access bypassing RwLock
    pub(super) command_tx_cache: Option<Arc<crate::api::command_cache::CommandTxCache>>,

    // ========== Dynamic Slot Allocation ==========
    /// Dynamic channel index for unified pool architecture (optional)
    pub(super) dynamic_channel_index: Option<Arc<ChannelIndex>>,
    /// Slot bitmap for dynamic allocation (optional, requires RwLock for &mut access)
    pub(super) slot_bitmap: Option<Arc<parking_lot::RwLock<SlotBitmap>>>,

    // ========== SHM Command Polling (M2C via SHM) ==========
    /// Shared memory reader for polling C/A timestamps (optional)
    pub(super) unified_reader: Option<Arc<UnifiedReader>>,
    /// SHM command poller for detecting M2C commands (optional, replaces TODO queue)
    pub(super) shm_poller: Option<Arc<ShmCommandPoller>>,
    /// Shutdown signal for SHM poller
    pub(super) shm_poller_shutdown: Option<tokio::sync::watch::Sender<bool>>,

    // ========== SHM Command Listener (Event-driven M2C) ==========
    /// SHM command listener for event-driven M2C command dispatch (optional)
    pub(super) shm_listener: Option<Arc<ShmCommandListener>>,
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
    pub fn new(rtdb: Arc<R>, routing_cache: Arc<voltage_routing::RoutingCache>) -> Self {
        Self {
            channels: Self::create_channel_slots(),
            active_channel_ids: DashSet::new(),
            rtdb,
            routing_cache,
            sqlite_pool: None,
            shm_handle: None,
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
        routing_cache: Arc<voltage_routing::RoutingCache>,
        sqlite_pool: sqlx::SqlitePool,
    ) -> Self {
        Self {
            channels: Self::create_channel_slots(),
            active_channel_ids: DashSet::new(),
            rtdb,
            routing_cache,
            sqlite_pool: Some(sqlite_pool),
            shm_handle: None,
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
    pub fn with_shared_memory(
        rtdb: Arc<R>,
        routing_cache: Arc<voltage_routing::RoutingCache>,
        sqlite_pool: sqlx::SqlitePool,
        shm_handle: Option<Arc<ShmHandle>>,
        command_tx_cache: Option<Arc<crate::api::command_cache::CommandTxCache>>,
    ) -> Self {
        Self {
            channels: Self::create_channel_slots(),
            active_channel_ids: DashSet::new(),
            rtdb,
            routing_cache,
            sqlite_pool: Some(sqlite_pool),
            shm_handle,
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
    pub fn with_dynamic_allocation(
        mut self,
        dynamic_index: Arc<ChannelIndex>,
        slot_bitmap: Arc<parking_lot::RwLock<SlotBitmap>>,
    ) -> Self {
        self.dynamic_channel_index = Some(dynamic_index);
        self.slot_bitmap = Some(slot_bitmap);
        self
    }

    /// Configure SHM command poller for M2C commands via shared memory
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
    pub fn with_shm_listener(mut self, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> Self {
        let listener = ShmCommandListener::new(None, shutdown_rx);
        self.shm_listener = Some(Arc::new(listener));
        self
    }

    /// Start the SHM command listener background task
    pub fn start_shm_listener(&self) -> Option<tokio::task::JoinHandle<std::io::Result<()>>> {
        let listener = self.shm_listener.clone()?;
        Some(tokio::spawn(async move { listener.run().await }))
    }

    /// Get SHM listener for channel registration (internal use)
    pub fn shm_listener(&self) -> Option<&Arc<ShmCommandListener>> {
        self.shm_listener.as_ref()
    }

    /// Get ShmHandle for routing reload SHM rebuild
    pub fn shm_handle(&self) -> Option<&Arc<ShmHandle>> {
        self.shm_handle.as_ref()
    }

    /// Get dynamic ChannelIndex (for external access, e.g., API stats)
    pub fn dynamic_channel_index(&self) -> Option<&Arc<ChannelIndex>> {
        self.dynamic_channel_index.as_ref()
    }

    /// Get SlotBitmap stats (for monitoring)
    pub fn slot_bitmap_stats(&self) -> Option<voltage_rtdb_shm::BitmapStats> {
        self.slot_bitmap.as_ref().map(|b| b.read().stats())
    }

    // ========================================================================
    // Channel Lifecycle
    // ========================================================================

    /// Remove channel with graceful shutdown.
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
            self.shutdown_channel_entry(&entry, channel_id).await?;
            self.cleanup_channel_redis(channel_id).await;
            info!("Ch{} removed (graceful shutdown)", channel_id);
            Ok(())
        } else {
            Err(ComSrvError::channel_not_found(channel_id))
        }
    }

    /// Shutdown a channel entry gracefully with timeout.
    async fn shutdown_channel_entry(&self, entry: &ChannelEntry<R>, channel_id: u32) -> Result<()> {
        // 1. Send shutdown signal to unified task (non-blocking)
        entry.shutdown();

        // 2. Wait for task to exit gracefully (up to 500ms)
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
        entry.store.shutdown().await;

        // 5. Disconnect channel
        let _ = entry.disconnect().await;

        // 6. Dynamic Slot Deallocation
        if let (Some(index), Some(bitmap)) = (&self.dynamic_channel_index, &self.slot_bitmap) {
            let mut bitmap_guard = bitmap.write();
            match index.remove_channel(channel_id, &mut bitmap_guard) {
                Ok(layout) => {
                    debug!(
                        "Ch{} slot freed: base={}, count={}",
                        channel_id, layout.base_slot, layout.total_points
                    );
                },
                Err(e) => {
                    warn!("Ch{} slot deallocation failed: {}", channel_id, e);
                },
            }
        }

        Ok(())
    }

    /// Clean up Redis keys for a removed channel.
    async fn cleanup_channel_redis(&self, channel_id: u32) {
        let keyspace = voltage_model::KeySpaceConfig::production_cached();

        // Delete channel name from Redis hash
        let channels_key = keyspace.channels_hash_key();
        if let Err(e) = self
            .rtdb
            .hash_del(&channels_key, &channel_id.to_string())
            .await
        {
            warn!(
                "Ch{} failed to delete name from Redis hash ({}): {}",
                channel_id, channels_key, e
            );
        }

        // Delete channel online status from Redis hash
        let online_key = keyspace.channel_online_key();
        if let Err(e) = self
            .rtdb
            .hash_del(&online_key, &channel_id.to_string())
            .await
        {
            warn!(
                "Ch{} failed to delete online status from Redis: {}",
                channel_id, e
            );
        }
    }

    // ========================================================================
    // Channel Query Methods
    // ========================================================================

    /// Get channel entry by ID (O(1) lock-free access ~5ns)
    #[inline]
    pub fn get_channel(&self, channel_id: u32) -> Option<Arc<ChannelEntry<R>>> {
        self.channels.get(channel_id as usize)?.load_full()
    }

    /// Alias for get_channel (for backwards compatibility)
    #[inline]
    pub fn get_channel_entry(&self, channel_id: u32) -> Option<Arc<ChannelEntry<R>>> {
        self.get_channel(channel_id)
    }

    /// Get channel IDs (O(n) where n = active channels)
    pub fn get_channel_ids(&self) -> Vec<u32> {
        self.active_channel_ids.iter().map(|id| *id).collect()
    }

    /// Get channel count (O(1))
    pub fn channel_count(&self) -> usize {
        self.active_channel_ids.len()
    }

    /// Get running channel count (O(n) where n = active channels)
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

    /// Get all channel stats (O(n) where n = active channels)
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
    pub async fn connect_all_channels(&self) -> Result<()> {
        const MAX_CONCURRENT_CONNECTS: usize = 16;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CONNECTS));

        let mut connect_tasks = Vec::with_capacity(self.active_channel_ids.len());

        for channel_id_ref in self.active_channel_ids.iter() {
            let channel_id = *channel_id_ref;
            if let Some(entry) = self
                .channels
                .get(channel_id as usize)
                .and_then(|s| s.load_full())
            {
                let entry_clone = Arc::clone(&entry);
                let sem = Arc::clone(&semaphore);

                let task = tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    match entry_clone.connect().await {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            error!("Ch{} connect err: {}", channel_id, e);
                            Err(e)
                        },
                    }
                });

                connect_tasks.push(task);
            }
        }

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
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    use voltage_rtdb::helpers::create_test_rtdb;

    /// Create test routing cache for unit tests
    fn create_test_routing_cache() -> Arc<voltage_routing::RoutingCache> {
        Arc::new(voltage_routing::RoutingCache::new())
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
