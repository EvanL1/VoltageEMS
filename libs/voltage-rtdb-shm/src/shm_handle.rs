//! ShmHandle — runtime-swappable shared memory writer + index
//!
//! Wraps `UnifiedWriter` and `ChannelToSlotIndex` behind `ArcSwapOption` so that
//! a routing reload can atomically replace them without blocking the hot write path.
//!
//! # Usage
//!
//! ```text
//! Hot path (every poll cycle):
//!   handle.writer()   → Option<Guard<Arc<UnifiedWriter>>>   (wait-free)
//!   handle.index()    → Option<Guard<Arc<ChannelToSlotIndex>>>
//!
//! Cold path (routing reload):
//!   handle.rebuild(&channel_points) → Result<()>
//!     1. Creates new UnifiedWriter from SharedConfig + new channel point counts
//!     2. Builds new ChannelToSlotIndex from new writer
//!     3. ArcSwap::store() — old objects drop when last Guard releases
//! ```

use std::sync::Arc;

use crate::channel_points::ChannelPointCounts;
use crate::shared_config::{ChannelToSlotIndex, SharedConfig};
use crate::unified_shm::UnifiedWriter;
use arc_swap::{ArcSwapOption, Guard};
use tracing::info;

/// Shared handle for runtime-swappable SHM writer and index.
///
/// All reads are wait-free (`ArcSwapOption::load`). Writes (rebuild) are
/// extremely infrequent (only on routing reload) and non-blocking to readers.
pub struct ShmHandle {
    writer: ArcSwapOption<UnifiedWriter>,
    index: ArcSwapOption<ChannelToSlotIndex>,
    config: SharedConfig,
}

impl ShmHandle {
    /// Create a new ShmHandle with initial writer and index.
    pub fn new(config: SharedConfig, writer: UnifiedWriter, index: ChannelToSlotIndex) -> Self {
        Self {
            writer: ArcSwapOption::new(Some(Arc::new(writer))),
            index: ArcSwapOption::new(Some(Arc::new(index))),
            config,
        }
    }

    /// Create an empty ShmHandle (SHM not available).
    pub fn empty(config: SharedConfig) -> Self {
        Self {
            writer: ArcSwapOption::empty(),
            index: ArcSwapOption::empty(),
            config,
        }
    }

    /// Load current writer (wait-free, returns Guard for zero-copy access).
    #[inline]
    pub fn writer(&self) -> Option<Guard<Option<Arc<UnifiedWriter>>>> {
        let guard = self.writer.load();
        if guard.is_some() {
            Some(guard)
        } else {
            None
        }
    }

    /// Load current writer Arc (clones the Arc, slightly more expensive than guard).
    #[inline]
    pub fn writer_arc(&self) -> Option<Arc<UnifiedWriter>> {
        self.writer.load_full()
    }

    /// Load current index (wait-free).
    #[inline]
    pub fn index(&self) -> Option<Guard<Option<Arc<ChannelToSlotIndex>>>> {
        let guard = self.index.load();
        if guard.is_some() {
            Some(guard)
        } else {
            None
        }
    }

    /// Load current index Arc.
    #[inline]
    pub fn index_arc(&self) -> Option<Arc<ChannelToSlotIndex>> {
        self.index.load_full()
    }

    /// Rebuild SHM writer and index from updated channel point counts.
    ///
    /// Called after routing reload succeeds. Reconfigures the existing
    /// SHM file in place, then atomically swaps the writer/index handles.
    /// Old objects are dropped when the last reader Guard releases.
    pub fn rebuild(&self, channel_points: &ChannelPointCounts) -> anyhow::Result<()> {
        let writer = UnifiedWriter::reconfigure_existing(&self.config, channel_points)?;
        let slot_count = writer.slot_count();
        let index = ChannelToSlotIndex::from_unified_writer(&writer);
        let index_len = index.len();

        self.writer.store(Some(Arc::new(writer)));
        self.index.store(Some(Arc::new(index)));

        info!(
            "ShmHandle: rebuilt with {} slots, {} index mappings",
            slot_count, index_len
        );
        Ok(())
    }

    /// Get the SharedConfig.
    pub fn config(&self) -> &SharedConfig {
        &self.config
    }

    /// Check if SHM is currently available.
    pub fn is_available(&self) -> bool {
        self.writer.load().is_some()
    }
}

impl std::fmt::Debug for ShmHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShmHandle")
            .field("available", &self.is_available())
            .finish()
    }
}
