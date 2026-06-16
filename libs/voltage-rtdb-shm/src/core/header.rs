//! On-mmap SHM header — the **physical** layout shared by all readers/writers.
//!
//! This module is pure infra: every field is a generic atomic/integer/byte
//! buffer with no business meaning. The field names retain their historical
//! identifiers (`routing_hash`) for now — that hash is fundamentally a
//! manifest fingerprint computed by the business layer, not something the
//! header itself interprets.

use std::sync::atomic::{AtomicU32, AtomicU64};

use crate::core::slot::PointSlot;

// ========== Constants ==========

/// Magic number for unified shared memory: "VOLTAGE_" in ASCII.
pub const UNIFIED_MAGIC: u64 = 0x564F4C544147455F;

/// SHM layout version.
///
/// - v3 changed the slot default from `(value=0.0, raw=0.0)` to
///   `(value=NaN, raw=NaN)`, so unwritten slots are self-describing instead
///   of relying on the `seq==0` side channel.
/// - v4 added physical padding slots to keep writer-ownership boundaries on
///   fresh cache lines. This changes physical slot indices even when channel
///   point counts are unchanged, so v3 snapshots must be rejected rather than
///   restored into the wrong slots.
pub const UNIFIED_VERSION: u32 = 4;

/// Default max slots (100,000 points).
pub const DEFAULT_MAX_SLOTS: u32 = 100_000;

// ========== Header ==========

/// Unified shared memory header.
///
/// Layout: 64 bytes, cache-line aligned. All multi-byte fields use native
/// endianness; readers and writers must run on the same architecture (we
/// only deploy on aarch64/x86_64 Linux).
#[repr(C, align(64))]
pub struct UnifiedHeader {
    /// Magic number for validation ("VOLTAGE_")
    pub magic: u64,
    /// Version number
    pub version: u32,
    /// Maximum number of slots
    pub max_slots: u32,
    /// Current slot count (atomically updated)
    pub slot_count: AtomicU32,
    /// Padding for alignment
    pub _pad: [u8; 4],
    /// Last update timestamp (for monitoring)
    pub last_update_ts: AtomicU64,
    /// Writer heartbeat (for monitoring)
    pub writer_heartbeat: AtomicU64,
    /// Manifest-style layout fingerprint for cross-process validation.
    ///
    /// Historical name `routing_hash`: comsrv writes the hash of its
    /// `ChannelPointCounts` layout on create; modsrv verifies its own hash
    /// matches on open. The header itself does not interpret the hash —
    /// what it fingerprints is a business-layer concern.
    pub routing_hash: AtomicU64,
    /// Writer generation counter — bumped on every create/reconfigure.
    /// Readers observe odd values to detect a reconfigure-in-progress window.
    pub writer_generation: AtomicU64,
    /// Reserved for future use
    pub _reserved: [u8; 8],
}

const _: () = assert!(std::mem::size_of::<UnifiedHeader>() == 64);

// ========== Layout Math ==========

/// Total file size required for a SHM with `max_slots` slots.
///
/// Layout: Header (64B) + PointSlot\[max_slots\] (32B each).
#[inline]
pub const fn calculate_file_size(max_slots: u32) -> usize {
    std::mem::size_of::<UnifiedHeader>() + (max_slots as usize) * std::mem::size_of::<PointSlot>()
}

/// Byte offset of the PointSlot array within the mmap region.
#[inline]
pub const fn slot_offset() -> usize {
    std::mem::size_of::<UnifiedHeader>()
}
