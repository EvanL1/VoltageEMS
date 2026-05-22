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
        // SAFETY: bounds checked above; PointSlot is repr(C, align(32)).
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
    pub fn save_snapshot(&self, path: &Path) -> Result<()> {
        let current_slot_count = self.header().slot_count.load(Ordering::Acquire) as usize;
        crate::core::snapshot_save::save_snapshot_impl(
            &self.mmap,
            current_slot_count,
            path,
            "SlotReader",
        )
        .context("SlotReader::save_snapshot")
    }
}

// ========== SlotIo impl ==========

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

    /// Readers are not writers — calling `write_slot` always returns false.
    /// The trait surface is symmetric for the read path; only writers
    /// actually back this method with a real implementation.
    fn write_slot(&self, _index: usize, _value: f64, _raw: f64, _timestamp_ms: u64) -> bool {
        false
    }

    /// Readers do not maintain a dirty bitmap; always returns empty.
    fn take_dirty_slots(&self) -> Vec<usize> {
        Vec::new()
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
