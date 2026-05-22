//! Pure-infra SHM writer.
//!
//! `SlotWriter` owns the mmap region, tracks dirty slots, and exposes
//! slot-indexed I/O. It has no knowledge of channels, point types,
//! instances, or routing — that lives in `UnifiedWriter` (in
//! `unified_shm.rs`) which composes a `SlotWriter` and adds the
//! channel-aware adapters.
//!
//! Consumers that only need slot-level I/O should program against
//! `&SlotWriter` or `&dyn SlotIo`; the channel adapters are not
//! reachable that way by design.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use memmap2::MmapMut;

use crate::core::header::{UnifiedHeader, slot_offset};
use crate::core::slot::PointSlot;
use crate::core::slot_io::{SlotIo, SlotRead};

// ========== Dirty bitmap helpers ==========

#[inline]
pub(crate) fn dirty_word_count(slot_count: usize) -> usize {
    slot_count.div_ceil(u64::BITS as usize)
}

pub(crate) fn new_dirty_words(slot_count: usize) -> Vec<AtomicU64> {
    (0..dirty_word_count(slot_count))
        .map(|_| AtomicU64::new(0))
        .collect()
}

// ========== SlotWriter ==========

/// Pure-infra view of a SHM writer.
///
/// Owns the mmap region and the process-local dirty bitmap. Provides
/// slot-indexed read/write and snapshot save. **Does not understand any
/// business concept** (channel, instance, point type, routing).
pub struct SlotWriter {
    pub(crate) mmap: MmapMut,
    pub(crate) path: PathBuf,
    pub(crate) max_slots: u32,
    pub(crate) slot_count: usize,
    /// Process-local dirty slot bitmap for fast SHM→Redis sync.
    ///
    /// PointSlot.dirty is shared across processes, but scanning it still
    /// costs O(slots). This bitmap is set by this writer's `set_direct`
    /// calls so comsrv can drain changed slots in O(dirty_words +
    /// dirty_slots), with periodic full scans as fallback.
    pub(crate) dirty_words: Vec<AtomicU64>,
}

impl SlotWriter {
    /// Wrap an already-initialized mmap region.
    ///
    /// The mmap is expected to:
    /// - have a valid `UnifiedHeader` at offset 0 (magic, version, etc. set)
    /// - have `slot_count` slots initialized to a known state (NaN sentinel
    ///   for fresh, or restored live data)
    /// - be at least `calculate_file_size(max_slots)` bytes long
    pub(crate) fn from_mmap(
        mmap: MmapMut,
        path: PathBuf,
        max_slots: u32,
        slot_count: usize,
    ) -> Self {
        Self {
            dirty_words: new_dirty_words(slot_count),
            mmap,
            path,
            max_slots,
            slot_count,
        }
    }

    /// Header reference. SAFETY: mmap starts with a valid UnifiedHeader.
    #[inline]
    pub fn header(&self) -> &UnifiedHeader {
        // SAFETY: mmap region starts with a valid UnifiedHeader.
        // UnifiedHeader is #[repr(C, align(64))], mmap base is page-aligned.
        unsafe { &*(self.mmap.as_ptr() as *const UnifiedHeader) }
    }

    /// PointSlot at index. Panics if out of bounds (use `read_slot`/`write_slot`
    /// for fallible variants).
    #[inline]
    pub(crate) fn slot_at(&self, index: usize) -> &PointSlot {
        assert!(
            index < self.slot_count,
            "slot_at: index {} out of bounds (slot_count={})",
            index,
            self.slot_count
        );
        // SAFETY: index is bounds-checked above. PointSlot is #[repr(C, align(32))].
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

    #[inline]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Flush mmap-backed file changes to disk.
    pub fn flush(&self) -> Result<()> {
        self.mmap.flush().context("Failed to flush mmap")
    }

    /// Direct slot write — the hot path. Panics if `slot` is out of bounds.
    #[inline]
    pub fn set_direct(&self, slot: usize, value: f64, raw: f64, timestamp_ms: u64) {
        assert!(
            slot < self.slot_count,
            "set_direct: slot {} out of bounds (slot_count={})",
            slot,
            self.slot_count
        );
        self.slot_at(slot).set(value, raw, timestamp_ms);
        self.mark_dirty_slot(slot);
        self.header()
            .writer_heartbeat
            .store(timestamp_ms, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn mark_dirty_slot(&self, slot: usize) {
        let word_idx = slot / u64::BITS as usize;
        let bit_idx = slot % u64::BITS as usize;
        if let Some(word) = self.dirty_words.get(word_idx) {
            word.fetch_or(1u64 << bit_idx, Ordering::Release);
        }
    }

    /// Drain process-local dirty slots set by this writer.
    pub fn take_dirty_slots(&self) -> Vec<usize> {
        let mut slots = Vec::new();
        for (word_idx, word) in self.dirty_words.iter().enumerate() {
            let mut bits = word.swap(0, Ordering::AcqRel);
            while bits != 0 {
                let bit_idx = bits.trailing_zeros() as usize;
                let slot = word_idx * u64::BITS as usize + bit_idx;
                if slot < self.slot_count {
                    slots.push(slot);
                }
                bits &= bits - 1;
            }
        }
        slots
    }

    /// Current writer generation (from header).
    pub fn generation(&self) -> u64 {
        self.header().writer_generation.load(Ordering::Acquire)
    }

    /// Most recent heartbeat timestamp written by this writer.
    pub fn writer_heartbeat(&self) -> u64 {
        self.header().writer_heartbeat.load(Ordering::Relaxed)
    }

    /// Update the writer heartbeat without writing a slot.
    pub fn update_heartbeat(&self, timestamp_ms: u64) {
        self.header()
            .writer_heartbeat
            .store(timestamp_ms, Ordering::Relaxed);
    }

    /// Save a snapshot using tear-resistant per-slot serialization.
    ///
    /// Delegates to the writer-side snapshot impl in `unified_shm` (which
    /// already knows how to handle torn reads via `try_load_consistent`).
    pub fn save_snapshot(&self, path: &Path) -> Result<()> {
        let current_slot_count = self.header().slot_count.load(Ordering::Acquire) as usize;
        crate::unified_shm::save_snapshot_impl(
            &self.mmap,
            current_slot_count,
            path,
            "SlotWriter",
        )
    }
}

// ========== SlotIo impl ==========

impl SlotIo for SlotWriter {
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

    fn write_slot(&self, index: usize, value: f64, raw: f64, timestamp_ms: u64) -> bool {
        if index >= self.slot_count {
            return false;
        }
        self.slot_at(index).set(value, raw, timestamp_ms);
        self.mark_dirty_slot(index);
        self.header()
            .writer_heartbeat
            .store(timestamp_ms, Ordering::Relaxed);
        true
    }

    fn take_dirty_slots(&self) -> Vec<usize> {
        SlotWriter::take_dirty_slots(self)
    }

    fn generation(&self) -> u64 {
        SlotWriter::generation(self)
    }

    fn writer_heartbeat(&self) -> u64 {
        SlotWriter::writer_heartbeat(self)
    }

    fn header(&self) -> &UnifiedHeader {
        SlotWriter::header(self)
    }
}
