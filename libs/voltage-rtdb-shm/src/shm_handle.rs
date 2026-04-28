//! ShmHandle — runtime-swappable shared memory layout
//!
//! Wraps `UnifiedWriter`, `ChannelToSlotIndex`, and `ReverseSlotIndex` behind a
//! single `ArcSwapOption` layout so that a routing reload can atomically replace
//! all hot-path lookup structures without mixing versions.
//!
//! # Usage
//!
//! ```text
//! Hot path (every poll cycle):
//!   handle.layout()   → Option<Guard<Arc<ShmLayout>>>   (wait-free)
//!
//! Cold path (routing reload):
//!   handle.rebuild(&channel_points) → Result<()>
//!     1. Creates new UnifiedWriter from SharedConfig + new channel point counts
//!     2. Builds new forward and reverse indexes from new writer
//!     3. ArcSwap::store() — old layout drops when last Guard releases
//! ```

use std::sync::Arc;

use crate::channel_points::ChannelPointCounts;
use crate::reverse_index::ReverseSlotIndex;
use crate::shared_config::{ChannelToSlotIndex, SharedConfig};
use crate::unified_shm::UnifiedWriter;
use arc_swap::{ArcSwapOption, Guard};
use tracing::info;

/// Coherent SHM layout snapshot used by hot paths.
///
/// All fields are built from the same `UnifiedWriter` layout and are swapped as
/// one `Arc`, preventing writer/index/reverse_index version skew during rebuild.
pub struct ShmLayout {
    pub writer: Arc<UnifiedWriter>,
    pub index: Arc<ChannelToSlotIndex>,
    pub reverse_index: Arc<ReverseSlotIndex>,
}

impl ShmLayout {
    fn new(writer: UnifiedWriter, index: ChannelToSlotIndex) -> Self {
        let slot_count = writer.slot_count();
        let reverse_index = ReverseSlotIndex::from_forward(&index, slot_count);

        Self {
            writer: Arc::new(writer),
            index: Arc::new(index),
            reverse_index: Arc::new(reverse_index),
        }
    }
}

/// Shared handle for runtime-swappable SHM layout.
///
/// All reads are wait-free (`ArcSwapOption::load`). Writes (rebuild) are
/// extremely infrequent (only on routing reload) and non-blocking to readers.
pub struct ShmHandle {
    layout: ArcSwapOption<ShmLayout>,
    config: SharedConfig,
}

impl ShmHandle {
    /// Create a new ShmHandle with initial writer and index.
    pub fn new(config: SharedConfig, writer: UnifiedWriter, index: ChannelToSlotIndex) -> Self {
        Self {
            layout: ArcSwapOption::new(Some(Arc::new(ShmLayout::new(writer, index)))),
            config,
        }
    }

    /// Create an empty ShmHandle (SHM not available).
    pub fn empty(config: SharedConfig) -> Self {
        Self {
            layout: ArcSwapOption::empty(),
            config,
        }
    }

    /// Load current coherent SHM layout (wait-free, zero-copy).
    #[inline]
    pub fn layout(&self) -> Option<Guard<Option<Arc<ShmLayout>>>> {
        let guard = self.layout.load();
        if guard.is_some() { Some(guard) } else { None }
    }

    /// Load current layout Arc (clones the Arc, slightly more expensive than guard).
    #[inline]
    pub fn layout_arc(&self) -> Option<Arc<ShmLayout>> {
        self.layout.load_full()
    }

    /// Rebuild SHM writer and indexes from updated channel point counts.
    ///
    /// Called after routing reload succeeds. Reconfigures the existing
    /// SHM file in place, then atomically swaps the coherent layout handle.
    /// Old objects are dropped when the last reader Guard releases.
    pub fn rebuild(&self, channel_points: &ChannelPointCounts) -> anyhow::Result<()> {
        let writer = UnifiedWriter::reconfigure_existing(&self.config, channel_points)?;
        let slot_count = writer.slot_count();
        let index = ChannelToSlotIndex::from_unified_writer(&writer);
        let index_len = index.len();
        let layout = Arc::new(ShmLayout::new(writer, index));
        let reverse_len = layout.reverse_index.mapped_count();

        self.layout.store(Some(layout));

        info!(
            "ShmHandle: rebuilt with {} slots, {} index mappings, {} reverse mappings",
            slot_count, index_len, reverse_len
        );
        Ok(())
    }

    /// Get the SharedConfig.
    pub fn config(&self) -> &SharedConfig {
        &self.config
    }

    /// Check if SHM is currently available.
    pub fn is_available(&self) -> bool {
        self.layout.load().is_some()
    }
}

impl std::fmt::Debug for ShmHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShmHandle")
            .field("available", &self.is_available())
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use voltage_model::PointType;

    fn counts(channel_id: u32) -> ChannelPointCounts {
        ChannelPointCounts::from_map(BTreeMap::from([(channel_id, [1, 0, 0, 0])]))
    }

    fn test_config(path: std::path::PathBuf) -> SharedConfig {
        SharedConfig::default().with_path(path).with_max_slots(8)
    }

    #[test]
    fn rebuild_replaces_reverse_index_with_layout() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path().join("test.shm"));

        let initial_counts = counts(1001);
        let writer = UnifiedWriter::create(&config, &initial_counts).unwrap();
        let index = ChannelToSlotIndex::from_unified_writer(&writer);
        let handle = ShmHandle::new(config, writer, index);

        let layout = handle.layout_arc().unwrap();
        let origin = layout.reverse_index.get(0).unwrap();
        assert_eq!(origin.channel_id, 1001);
        assert_eq!(origin.point_type, PointType::Telemetry);

        let rebuilt_counts = counts(2002);
        handle.rebuild(&rebuilt_counts).unwrap();

        let layout = handle.layout_arc().unwrap();
        assert!(layout.index.lookup(1001, PointType::Telemetry, 0).is_none());
        assert_eq!(layout.index.lookup(2002, PointType::Telemetry, 0), Some(0));
        let origin = layout.reverse_index.get(0).unwrap();
        assert_eq!(origin.channel_id, 2002);
        assert_eq!(origin.point_type, PointType::Telemetry);
    }
}
