//! Pure-infra SHM reader.
//!
//! `SlotReader` owns a read-only mmap of a SHM segment and exposes
//! slot-indexed reads, header introspection, and snapshot save. Like
//! `SlotWriter`, it has no knowledge of channels, point types,
//! instances, or routing.
//!
//! `UnifiedReader` (in `unified_shm.rs`) composes this struct and adds
//! the channel-aware iterators; consumers that only need slot reads
//! should program against `&SlotReader` or `&dyn SlotIo`.

use std::path::Path;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use memmap2::Mmap;

use crate::core::header::{UnifiedHeader, slot_offset};
use crate::core::slot::PointSlot;
use crate::core::slot_io::{SlotIo, SlotRead};

/// Pure-infra view of a SHM reader.
///
/// Owns the read-only mmap. Provides slot-indexed reads and header
/// access. **Does not understand any business concept** (channel,
/// instance, point type, routing).
pub struct SlotReader {
    pub(crate) mmap: Mmap,
    pub(crate) max_slots: u32,
    pub(crate) slot_count: usize,
}

impl SlotReader {
    /// Wrap an already-opened mmap. The mmap must point at a valid
    /// SHM segment (header magic+version already validated by caller).
    pub(crate) fn from_mmap(mmap: Mmap, max_slots: u32, slot_count: usize) -> Self {
        Self {
            mmap,
            max_slots,
            slot_count,
        }
    }

    /// Header reference.
    #[inline]
    pub fn header(&self) -> &UnifiedHeader {
        // SAFETY: mmap region starts with a valid UnifiedHeader.
        unsafe { &*(self.mmap.as_ptr() as *const UnifiedHeader) }
    }

    /// PointSlot at index. Panics if out of bounds.
    #[inline]
    pub(crate) fn slot_at(&self, index: usize) -> &PointSlot {
        assert!(
            index < self.slot_count,
            "slot_at: index {} out of bounds (slot_count={})",
            index,
            self.slot_count
        );
        // SAFETY: alignment chain — mmap base is page-aligned (≥4096),
        // slot_offset() == size_of::<UnifiedHeader>() == 64 (asserted at
        // const time in core::header), and 64 is divisible by 32. So the
        // base pointer for the slot array is 32-byte aligned, matching
        // PointSlot's `#[repr(C, align(32))]` requirement. `index` is
        // bounds-checked above against `slot_count`, and the constructor
        // verified the mmap covers at least `max_slots` slots.
        unsafe {
            let ptr = self.mmap.as_ptr().add(slot_offset()) as *const PointSlot;
            &*ptr.add(index)
        }
    }

    #[inline]
    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    #[inline]
    pub fn max_slots(&self) -> u32 {
        self.max_slots
    }

    /// Most recent heartbeat timestamp written by the writer.
    pub fn writer_heartbeat(&self) -> u64 {
        self.header().writer_heartbeat.load(Ordering::Relaxed)
    }

    /// Check if the writer is alive within the given timeout.
    pub fn is_writer_alive(&self, timeout_ms: u64) -> bool {
        let last_hb = self.writer_heartbeat();
        if last_hb == 0 {
            return false;
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        now_ms.saturating_sub(last_hb) < timeout_ms
    }

    /// Save a tear-resistant snapshot of the current SHM state.
    ///
    /// Uses `self.slot_count` (captured at `open()`) rather than the live
    /// `header().slot_count` field. Reading the live header here would race
    /// with comsrv's `reconfigure_existing`: during the reconfigure window
    /// `header.slot_count` has already been advanced to the new value, but
    /// the slot array is still being zeroed. Snapshotting through the live
    /// count in that window would capture all-zero slots that read as
    /// `value=0.0` (a valid finite reading) rather than NaN sentinels, and
    /// the snapshot would silently encode garbage as live data.
    pub fn save_snapshot(&self, path: &Path) -> Result<()> {
        crate::core::snapshot_save::save_snapshot_impl(
            &self.mmap,
            self.slot_count,
            path,
            "SlotReader",
        )
        .context("SlotReader::save_snapshot")
    }
}

// ========== SlotIo (read-only) impl ==========

impl SlotIo for SlotReader {
    #[inline]
    fn slot_count(&self) -> usize {
        self.slot_count
    }

    fn read_slot(&self, index: usize) -> Option<SlotRead> {
        if index >= self.slot_count {
            return None;
        }
        let (value, raw, timestamp_ms) = self.slot_at(index).try_load_consistent()?;
        Some(SlotRead {
            value,
            raw,
            timestamp_ms,
        })
    }

    fn generation(&self) -> u64 {
        self.header().writer_generation.load(Ordering::Acquire)
    }

    fn writer_heartbeat(&self) -> u64 {
        SlotReader::writer_heartbeat(self)
    }

    fn header(&self) -> &UnifiedHeader {
        SlotReader::header(self)
    }
}
