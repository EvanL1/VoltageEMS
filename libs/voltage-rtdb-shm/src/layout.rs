//! Channel slot layout — **business-side**, not part of `core::`.
//!
//! `ChannelLayout` knows the 4 PointType semantics (T/S/C/A) and translates
//! `(channel_id, point_type, point_id)` triples into flat slot indices. This
//! is the adapter between SHM's slot-only world (`core::slot::PointSlot`,
//! addressed by usize) and the channel/point-type vocabulary that comsrv
//! and modsrv speak.
//!
//! Long-term direction: when SHM core is promoted to a standalone crate,
//! this module stays in the business adapter. The header on-mmap already
//! captures the layout fingerprint as `routing_hash` (a manifest hash);
//! `ChannelLayout` itself never crosses the process boundary — it is
//! re-derived in each process from the same `ChannelPointCounts` source.

use crate::channel_points::ChannelPointCounts;

/// Channel layout — allocation info for one channel.
///
/// Stored in process memory as `Vec<ChannelLayout>`, indexed by `channel_id`.
/// Re-derived independently in comsrv (writer) and modsrv (reader) from the
/// same `ChannelPointCounts` source; cross-process agreement is verified via
/// the header's `routing_hash` field, not by sharing this struct.
#[derive(Clone, Default, Debug)]
pub struct ChannelLayout {
    /// Base slot index for this channel
    pub base_slot: usize,
    /// Offset for each point type [T, S, C, A]
    pub type_offsets: [usize; 4],
    /// Point count for each type [T, S, C, A]
    pub type_counts: [u32; 4],
    /// Total points for this channel
    pub total_points: u32,
}

impl ChannelLayout {
    /// Calculate slot index for given type and point_id.
    #[inline]
    pub fn slot(&self, point_type: u8, point_id: u32) -> Option<usize> {
        let type_idx = point_type as usize;
        if type_idx >= 4 || point_id >= self.type_counts[type_idx] {
            return None;
        }
        Some(self.base_slot + self.type_offsets[type_idx] + point_id as usize)
    }

    /// Check if this layout is valid (has any points).
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.total_points > 0
    }
}

/// Allocate layouts from channel point counts.
///
/// Both Writer and Reader call this with the same `ChannelPointCounts` →
/// same slot allocation. Routing-independent: layout only depends on which
/// channels have which point counts.
pub fn allocate_layouts(channel_points: &ChannelPointCounts) -> (Vec<ChannelLayout>, usize) {
    // Find max channel_id to size the Vec
    let max_channel_id = channel_points.0.keys().copied().max().unwrap_or(0);
    let vec_size = (max_channel_id + 1) as usize;

    let mut layouts = vec![ChannelLayout::default(); vec_size];
    let mut next_slot = 0usize;

    // Allocate in channel_id order (BTreeMap is sorted)
    for (&channel_id, counts) in &channel_points.0 {
        let layout = &mut layouts[channel_id as usize];
        layout.base_slot = next_slot;

        // Allocate each type in T/S/C/A order
        for (type_idx, &count) in counts.iter().enumerate() {
            layout.type_offsets[type_idx] = next_slot - layout.base_slot;
            layout.type_counts[type_idx] = count;
            next_slot += count as usize;
        }

        layout.total_points = counts.iter().sum();
    }

    (layouts, next_slot)
}
