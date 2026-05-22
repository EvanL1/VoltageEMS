//! Unified Shared Memory Implementation (Simplified)
//!
//! SharedMemory only stores values (Header + PointSlot[]).
//! Indexes are Vec in process memory, accessed by ID as array index.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  SharedMemory (mmap file)                                   │
//! │  ┌────────────────────────────────────────────────────────┐ │
//! │  │ Header (64B) │ PointSlot[0] │ PointSlot[1] │ ...       │ │
//! │  └────────────────────────────────────────────────────────┘ │
//! │  Only values, no metadata!                                  │
//! └─────────────────────────────────────────────────────────────┘
//!
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Process Memory (Vec index, not HashMap)                    │
//! │  - channel_layouts: Vec<ChannelLayout>                      │
//! │  - slot = layouts[channel_id].base + type_offset + point_id │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Design Points
//!
//! - **Deterministic allocation**: Writer and Reader use same order → same slot numbers
//! - **Vec index**: O(1) array access, faster than HashMap
//! - **Routing is permission**: Runtime DashMap, not data synchronization

use crate::channel_points::ChannelPointCounts;
use crate::core::slot::PointSlot;
use crate::layout::{ChannelLayout, allocate_layouts};
use crate::shared_config::SharedConfig;
use anyhow::{Context, Result, bail};
use memmap2::{Mmap, MmapMut, MmapOptions};
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering, fence};
use voltage_model::PointType;
use voltage_routing::RoutingCache;

pub use crate::core::header::{
    DEFAULT_MAX_SLOTS, UNIFIED_MAGIC, UNIFIED_VERSION, UnifiedHeader, calculate_file_size,
    slot_offset,
};

#[inline]
fn dirty_word_count(slot_count: usize) -> usize {
    slot_count.div_ceil(u64::BITS as usize)
}

fn new_dirty_words(slot_count: usize) -> Vec<AtomicU64> {
    (0..dirty_word_count(slot_count))
        .map(|_| AtomicU64::new(0))
        .collect()
}

fn read_ne_bytes<const N: usize>(buf: &[u8], start: usize, label: &str) -> Result<[u8; N]> {
    let end = start
        .checked_add(N)
        .with_context(|| format!("Invalid snapshot offset for {}", label))?;
    let bytes = buf
        .get(start..end)
        .with_context(|| format!("Snapshot missing {}", label))?;
    bytes
        .try_into()
        .with_context(|| format!("Invalid snapshot field size for {}", label))
}

fn read_u64_ne(buf: &[u8], start: usize, label: &str) -> Result<u64> {
    Ok(u64::from_ne_bytes(read_ne_bytes(buf, start, label)?))
}

fn read_u32_ne(buf: &[u8], start: usize, label: &str) -> Result<u32> {
    Ok(u32::from_ne_bytes(read_ne_bytes(buf, start, label)?))
}

// ========== Shared Helpers ==========

/// Save shared memory data to a snapshot file (atomic write via temp + rename).
///
/// Shared implementation used by both `UnifiedWriter::save_snapshot` and
/// `UnifiedReader::save_snapshot`.
///
/// # Tear-resistant slot serialization
///
/// Earlier versions did a raw `memcpy` of the mmap region. If the writer
/// was mid-update through a seqlock at snapshot time, the snapshot
/// captured torn bytes (new value + old raw, or seq=odd mid-write) and
/// preserved them across restart — a stale or impossible reading would
/// then be restored to SHM and propagate through Redis until overwritten
/// by the next live write.
///
/// Now we iterate slots and call `try_load_consistent()` per slot. Torn
/// reads (writer concurrently mid-update) produce `None` and we write
/// the unwritten-NaN sentinel for that slot — restore treats it as
/// never-written and skips it, matching the safe behavior of fresh
/// slots. The header bytes are still copied verbatim (header atomics
/// are byte-stable so a `memcpy` of a 64-byte aligned region is safe).
fn save_snapshot_impl(
    mmap_data: &[u8],
    slot_count: usize,
    path: &std::path::Path,
    label: &str,
) -> Result<()> {
    use std::io::Write;

    if mmap_data.len() < slot_offset() + slot_count * std::mem::size_of::<PointSlot>() {
        bail!(
            "snapshot source mmap too small: len={} slot_count={}",
            mmap_data.len(),
            slot_count
        );
    }

    let temp_path = path.with_extension("tmp");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create snapshot directory: {:?}", parent))?;
    }

    let mut file = std::fs::File::create(&temp_path)
        .with_context(|| format!("Failed to create temp snapshot file: {:?}", temp_path))?;

    // Header: copied verbatim. AtomicU64 fields are byte-stable; this is
    // a 64-byte region and individual atomic stores are torn-safe at the
    // u64 level, so a single memcpy captures a consistent header.
    file.write_all(&mmap_data[..slot_offset()])
        .with_context(|| "Failed to write snapshot header")?;

    // Slots: iterate one at a time via the seqlock-aware load. Torn slots
    // become unwritten-NaN sentinels in the snapshot.
    // SAFETY: bounds checked above; PointSlot is repr(C, align(32)).
    let slots_ptr =
        unsafe { mmap_data.as_ptr().add(slot_offset()) as *const crate::core::slot::PointSlot };
    let mut torn = 0usize;
    for i in 0..slot_count {
        // SAFETY: i < slot_count, and the mmap covers slot_count slots.
        let slot = unsafe { &*slots_ptr.add(i) };
        let bytes = match slot.try_load_consistent() {
            Some((value, raw, ts)) => slot_snapshot_bytes(value, raw, ts),
            None => {
                torn += 1;
                slot_unwritten_bytes()
            },
        };
        file.write_all(&bytes)
            .with_context(|| "Failed to write snapshot slot")?;
    }

    file.flush().context("Failed to flush snapshot file")?;
    file.sync_all().context("Failed to sync snapshot file")?;

    std::fs::rename(&temp_path, path)
        .with_context(|| format!("Failed to rename temp to snapshot: {:?}", path))?;

    let data_size = slot_offset() + slot_count * std::mem::size_of::<PointSlot>();
    if torn > 0 {
        tracing::warn!(
            "{} snapshot saved with {} torn slot(s) elided as unwritten: {:?}, size={} bytes, slots={}",
            label,
            torn,
            path,
            data_size,
            slot_count
        );
    } else {
        tracing::info!(
            "{} snapshot saved: {:?}, size={} bytes, slots={}",
            label,
            path,
            data_size,
            slot_count
        );
    }
    Ok(())
}

/// Encode a known-consistent slot as 32 bytes matching PointSlot's
/// `#[repr(C)]` layout: value_bits(u64) | timestamp(u64) | raw_bits(u64) |
/// seq(u32) | dirty(u32). seq is written as 2 (even, signaling "value
/// committed") and dirty as 0. Native-endian bytes match the in-memory
/// representation that restore will load directly.
fn slot_snapshot_bytes(value: f64, raw: f64, ts: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[0..8].copy_from_slice(&value.to_bits().to_ne_bytes());
    out[8..16].copy_from_slice(&ts.to_ne_bytes());
    out[16..24].copy_from_slice(&raw.to_bits().to_ne_bytes());
    out[24..28].copy_from_slice(&2u32.to_ne_bytes());
    out[28..32].copy_from_slice(&0u32.to_ne_bytes());
    out
}

/// Encode an unwritten-sentinel slot for the snapshot file: NaN
/// value_bits + NaN raw_bits + seq=0, dirty=0. Matches the layout
/// produced by `PointSlot::new()` and is recognized by restore via
/// the NaN sentinel filter.
fn slot_unwritten_bytes() -> [u8; 32] {
    let mut out = [0u8; 32];
    out[0..8].copy_from_slice(&crate::core::slot::SLOT_UNWRITTEN_BITS.to_ne_bytes());
    // timestamp = 0 (already)
    out[16..24].copy_from_slice(&crate::core::slot::SLOT_UNWRITTEN_BITS.to_ne_bytes());
    // seq = 0, dirty = 0 (already)
    out
}

/// Validate a shared memory header: checks magic, version, and routing hash
///
/// Returns (max_slots, slot_count) on success. Used by both `open_for_actions`
/// and `UnifiedReader::open` to eliminate duplicate validation logic.
///
/// # Safety
/// The caller must ensure `header` points to a valid, readable `UnifiedHeader`.
fn validate_shm_header(
    header: &UnifiedHeader,
    channel_points: &ChannelPointCounts,
) -> Result<(u32, usize)> {
    if header.magic != UNIFIED_MAGIC {
        bail!(
            "Invalid magic: expected 0x{:X}, got 0x{:X}",
            UNIFIED_MAGIC,
            header.magic
        );
    }
    if header.version != UNIFIED_VERSION {
        bail!(
            "Version mismatch: expected {}, got {}",
            UNIFIED_VERSION,
            header.version
        );
    }

    let expected_hash = channel_points.layout_hash();
    let actual_hash = header.routing_hash.load(Ordering::Acquire);
    if expected_hash != actual_hash {
        bail!(
            "Channel layout mismatch! \
             SHM layout_hash=0x{:016X}, local layout_hash=0x{:016X}. \
             Slot indexes may be misaligned. \
             Solution: Restart the writer process (comsrv) to synchronize.",
            actual_hash,
            expected_hash
        );
    }

    let max_slots = header.max_slots;
    let slot_count = header.slot_count.load(Ordering::Acquire) as usize;
    Ok((max_slots, slot_count))
}

/// Verify that calculated slot count matches what's stored in the SHM header
fn verify_slot_count(file_slot_count: usize, calculated_slots: usize) -> Result<()> {
    if calculated_slots != file_slot_count {
        bail!(
            "Slot count mismatch: file={}, calculated={}. \
             allocate_layouts() produced different results. \
             Solution: Restart both services to synchronize.",
            file_slot_count,
            calculated_slots
        );
    }
    Ok(())
}

/// Validate a shared memory header for writer-side reconfiguration.
///
/// Unlike `validate_shm_header`, this intentionally does not verify
/// `routing_hash` because the caller is about to replace it.
fn validate_reconfigurable_header(header: &UnifiedHeader) -> Result<u32> {
    if header.magic != UNIFIED_MAGIC {
        bail!(
            "Invalid magic: expected 0x{:X}, got 0x{:X}",
            UNIFIED_MAGIC,
            header.magic
        );
    }
    if header.version != UNIFIED_VERSION {
        bail!(
            "Version mismatch: expected {}, got {}",
            UNIFIED_VERSION,
            header.version
        );
    }
    Ok(header.max_slots)
}

/// Shared accessor methods for both UnifiedWriter and UnifiedReader.
///
/// Both types store an mmap region with identical layout: Header + PointSlot[].
/// This macro generates `header()`, `slot_at()`, and `lookup()` to avoid duplication.
macro_rules! impl_shm_accessors {
    ($mmap_field:ident) => {
        /// Get header reference
        #[inline]
        fn header(&self) -> &UnifiedHeader {
            // SAFETY: mmap region starts with a valid UnifiedHeader.
            // UnifiedHeader is #[repr(C, align(64))], mmap base is page-aligned.
            unsafe { &*(self.$mmap_field.as_ptr() as *const UnifiedHeader) }
        }

        /// Get PointSlot at index
        #[inline]
        fn slot_at(&self, index: usize) -> &PointSlot {
            assert!(
                index < self.slot_count,
                "slot_at: index {} out of bounds (slot_count={})",
                index,
                self.slot_count
            );
            // SAFETY: index is bounds-checked above. PointSlot is #[repr(C, align(32))].
            unsafe {
                let ptr = self.$mmap_field.as_ptr().add(slot_offset()) as *const PointSlot;
                &*ptr.add(index)
            }
        }

        /// Lookup slot by channel key
        #[inline]
        pub fn lookup(&self, channel_id: u32, point_type: u8, point_id: u32) -> Option<usize> {
            self.channel_layouts
                .get(channel_id as usize)?
                .slot(point_type, point_id)
        }

        /// Get current slot count
        #[inline]
        pub fn slot_count(&self) -> usize {
            self.slot_count
        }

        /// Get max slots
        #[inline]
        pub fn max_slots(&self) -> u32 {
            self.max_slots
        }

        /// Get channel layouts
        #[inline]
        pub fn channel_layouts(&self) -> &[ChannelLayout] {
            &self.channel_layouts
        }
    };
}

// ========== Memory Layout ==========

/// Calculate file size for given max_slots
///
/// Verify a file is at least header-sized before any unsafe header cast.
///
/// Returns Err if the file is shorter than `size_of::<UnifiedHeader>()`,
/// which would make casting the mmap pointer to `*const UnifiedHeader`
/// immediate UB. Guards against truncated or corrupt SHM files at startup.
fn verify_file_min_size(file: &File, path: &std::path::Path) -> Result<()> {
    let len = file
        .metadata()
        .with_context(|| format!("Failed to stat {:?}", path))?
        .len();
    let min = std::mem::size_of::<UnifiedHeader>() as u64;
    if len < min {
        bail!(
            "SHM file {:?} truncated: len={} < header size {} — refusing unsafe header cast",
            path,
            len,
            min
        );
    }
    Ok(())
}

/// Verify the mmap covers the full slot array implied by `max_slots`.
///
/// Called after `validate_shm_header` so we have a trusted `max_slots`.
/// Returns Err if the mmap is shorter than the slot region would require,
/// indicating a truncated file that would cause out-of-bounds slot reads.
fn verify_mmap_covers_slots(mmap_len: usize, max_slots: u32, path: &std::path::Path) -> Result<()> {
    let required = calculate_file_size(max_slots);
    if mmap_len < required {
        bail!(
            "SHM file {:?} too small for declared max_slots={}: have {} bytes, need {}",
            path,
            max_slots,
            mmap_len,
            required
        );
    }
    Ok(())
}

// ========== UnifiedWriter ==========

/// Unified shared memory writer
///
/// Single writer per shared memory file (comsrv).
pub struct UnifiedWriter {
    mmap: MmapMut,
    path: PathBuf,
    max_slots: u32,
    slot_count: usize,
    /// Channel layouts (Vec index by channel_id)
    channel_layouts: Vec<ChannelLayout>,
    /// Process-local dirty slot bitmap for fast SHM→Redis sync.
    ///
    /// PointSlot.dirty is shared across processes, but scanning it still costs O(slots).
    /// This bitmap is set by this writer's `set*` calls so comsrv can drain changed
    /// slots in O(dirty_words + dirty_slots), with periodic full scans as fallback.
    dirty_words: Vec<AtomicU64>,
}

impl UnifiedWriter {
    /// Create shared memory and initialize from ChannelPointCounts
    pub fn create(config: &SharedConfig, channel_points: &ChannelPointCounts) -> Result<Self> {
        let path = config.path();
        let max_slots = config.max_slots().unwrap_or(DEFAULT_MAX_SLOTS);

        // Allocate layouts
        let (channel_layouts, slot_count) = allocate_layouts(channel_points);

        if slot_count > max_slots as usize {
            bail!("Too many slots: {} (max={})", slot_count, max_slots);
        }

        let file_size = calculate_file_size(max_slots);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        // Open or create file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true) // Always create fresh
            .open(path)
            .with_context(|| format!("Failed to open {:?}", path))?;

        // Set file size
        file.set_len(file_size as u64)
            .with_context(|| "Failed to set file size")?;

        // Memory map
        // SAFETY: File was just created/truncated with the correct size.
        // We have exclusive write access (single writer design).
        let mut mmap = unsafe {
            MmapOptions::new()
                .len(file_size)
                .map_mut(&file)
                .with_context(|| "Failed to mmap")?
        };

        // SAFETY: mmap region is at least size_of::<UnifiedHeader>() (64 bytes).
        // UnifiedHeader is #[repr(C, align(64))], and mmap base is page-aligned.
        let header = unsafe { &mut *(mmap.as_mut_ptr() as *mut UnifiedHeader) };
        header.magic = UNIFIED_MAGIC;
        header.version = UNIFIED_VERSION;
        header.max_slots = max_slots;
        header.slot_count = AtomicU32::new(slot_count as u32);
        header._pad = [0; 4];
        header.last_update_ts = AtomicU64::new(0);
        header.writer_heartbeat = AtomicU64::new(0);
        // Store channel layout hash for cross-process synchronization
        header.routing_hash = AtomicU64::new(channel_points.layout_hash());
        // Seed generation so a comsrv restart that recreates the SHM file
        // produces a different generation than the previous incarnation.
        // Wall-clock alone is not enough — an NTP step-back within the same
        // second could yield ≤ the previous seed and silently bypass the
        // mismatch detection in ShmDispatch. XOR with a per-process nonce
        // (PID + a static-address ASLR bit) so monotonicity does not
        // depend on the system clock.
        static NONCE_ANCHOR: AtomicU64 = AtomicU64::new(0);
        let process_nonce = (std::process::id() as u64)
            .wrapping_mul(0x9E3779B97F4A7C15)
            .wrapping_add(&NONCE_ANCHOR as *const _ as usize as u64);
        let wall_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        // Force the initial generation to be even and nonzero. The reconfigure
        // path relies on the invariant "generation is even at rest, odd while
        // a reconfigure is in flight" — readers gate themselves out on odd.
        // A random odd seed would defeat that gating (and only fire the
        // debug_assert in debug builds, silently corrupting release).
        let generation_seed = (wall_nanos.wrapping_add(process_nonce) & !1u64).max(2);
        header.writer_generation = AtomicU64::new(generation_seed);
        header._reserved = [0; 8];

        // Initialize every PointSlot to the "unwritten" sentinel (NaN).
        // `set_len` above zero-filled the file, so without this loop slots
        // would default to (value=0.0, raw=0.0) — indistinguishable from a
        // real device reading of zero. ShmRedisSync.full_scan and downstream
        // readers rely on `is_finite(value)` to filter unwritten slots.
        // SAFETY: `slots_ptr` points at the slot region inside the mmap;
        // each slot index < slot_count is within the file range we just
        // sized. PointSlot is `#[repr(C, align(32))]` so pointer arithmetic
        // is well-defined and reads back as a valid PointSlot reference.
        let slots_ptr =
            unsafe { mmap.as_mut_ptr().add(slot_offset()) as *const crate::core::slot::PointSlot };
        for i in 0..slot_count {
            let slot = unsafe { &*slots_ptr.add(i) };
            slot.init_unwritten();
        }

        // Flush header to backing file for cross-process visibility.
        // Without this, a reader on ARM64 mmap'ing the same file could see
        // partially-written header fields.
        mmap.flush()
            .with_context(|| "Failed to flush mmap after create")?;

        tracing::info!(
            "Created unified shared memory: {:?}, slots={}/{}, size={}KB, channels={}",
            path,
            slot_count,
            max_slots,
            file_size / 1024,
            channel_layouts.iter().filter(|l| l.is_valid()).count()
        );

        Ok(Self {
            mmap,
            path: path.to_path_buf(),
            max_slots,
            slot_count,
            channel_layouts,
            dirty_words: new_dirty_words(slot_count),
        })
    }

    impl_shm_accessors!(mmap);

    /// Write value to slot by channel key
    ///
    /// # Arguments
    /// - `channel_id`: Channel ID
    /// - `point_type`: Point type (T=0, S=1, C=2, A=3)
    /// - `point_id`: Point ID
    /// - `value`: Engineering value
    /// - `raw`: Raw value
    /// - `timestamp_ms`: Timestamp in milliseconds
    #[inline]
    pub fn set(
        &self,
        channel_id: u32,
        point_type: u8,
        point_id: u32,
        value: f64,
        raw: f64,
        timestamp_ms: u64,
    ) -> bool {
        if let Some(layout) = self.channel_layouts.get(channel_id as usize)
            && let Some(slot) = layout.slot(point_type, point_id)
        {
            self.slot_at(slot).set(value, raw, timestamp_ms);
            self.mark_dirty_slot(slot);
            // Update heartbeat
            self.header()
                .writer_heartbeat
                .store(timestamp_ms, Ordering::Relaxed);
            return true;
        }
        false
    }

    /// Direct write to slot index (for hot path)
    #[inline]
    pub fn set_direct(&self, slot: usize, value: f64, raw: f64, timestamp_ms: u64) {
        // SAFETY: Bounds check required before unsafe pointer arithmetic below
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
    fn mark_dirty_slot(&self, slot: usize) {
        let word_idx = slot / u64::BITS as usize;
        let bit_idx = slot % u64::BITS as usize;
        if let Some(word) = self.dirty_words.get(word_idx) {
            word.fetch_or(1u64 << bit_idx, Ordering::Release);
        }
    }

    /// Drain process-local dirty slots set by this writer.
    ///
    /// Concurrent writes are safe: if a writer sets a bit before `swap(0)`, this
    /// call returns it; if it sets after the swap, the bit remains for the next pass.
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

    /// Returns the current writer generation from the SHM header.
    /// Used by modsrv to detect comsrv restarts.
    pub fn generation(&self) -> u64 {
        self.header().writer_generation.load(Ordering::Acquire)
    }

    /// Get file path
    #[inline]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Flush changes to disk
    pub fn flush(&self) -> Result<()> {
        self.mmap.flush().context("Failed to flush mmap")
    }

    /// Read current heartbeat timestamp (ms since epoch)
    #[inline]
    pub fn writer_heartbeat(&self) -> u64 {
        self.header().writer_heartbeat.load(Ordering::Relaxed)
    }

    /// Read-only access to a slot by index.
    #[inline]
    pub fn slot(&self, index: usize) -> &crate::core::slot::PointSlot {
        self.slot_at(index)
    }

    /// Update heartbeat timestamp
    #[inline]
    pub fn update_heartbeat(&self, timestamp_ms: u64) {
        self.header()
            .writer_heartbeat
            .store(timestamp_ms, Ordering::Relaxed);
    }

    /// Open existing shared memory for writing Action points (C/A types only)
    ///
    /// This is used by modsrv to write downstream commands (Control/Adjustment)
    /// while comsrv owns the main writer for upstream data (Telemetry/Signal).
    ///
    /// # Safety
    /// - Only writes to C/A slots (type 2 and 3)
    /// - T/S slots (type 0 and 1) should not be written by this writer
    /// - Atomic operations ensure cross-process safety
    pub fn open_for_actions(
        config: &SharedConfig,
        channel_points: &ChannelPointCounts,
    ) -> Result<Self> {
        let path = config.path();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("Failed to open {:?} for actions", path))?;

        // Guard against truncated/corrupt files: a file shorter than the
        // header makes the pointer cast below UB. Must verify BEFORE mmap.
        verify_file_min_size(&file, path)?;

        // SAFETY: File exists, was created by the primary writer (comsrv),
        // and we just verified it is at least header-sized.
        let mmap = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .with_context(|| "Failed to mmap for actions")?
        };

        // SAFETY: mmap.len() >= sizeof(UnifiedHeader) by verify_file_min_size.
        let header = unsafe { &*(mmap.as_ptr() as *const UnifiedHeader) };
        let (max_slots, slot_count) = validate_shm_header(header, channel_points)?;

        // Now that max_slots is trusted, confirm the mmap actually covers
        // the slot array. Without this, set_direct on slot_count-1 could
        // read/write past the mmap end.
        verify_mmap_covers_slots(mmap.len(), max_slots, path)?;

        let (channel_layouts, calculated_slots) = allocate_layouts(channel_points);
        verify_slot_count(slot_count, calculated_slots)?;

        tracing::info!(
            "Opened unified writer for actions: {:?}, slots={}, C/A channels={}",
            path,
            slot_count,
            channel_layouts
                .iter()
                .filter(|l| l.type_counts[2] > 0 || l.type_counts[3] > 0)
                .count()
        );

        Ok(Self {
            mmap,
            path: path.to_path_buf(),
            max_slots,
            slot_count,
            channel_layouts,
            dirty_words: new_dirty_words(slot_count),
        })
    }

    /// Write Action point (Control or Adjustment only)
    ///
    /// This is a safe wrapper that only allows writing to C/A slots.
    /// Returns false if point_type is not Control (2) or Adjustment (3).
    #[inline]
    pub fn set_action(
        &self,
        channel_id: u32,
        point_type: u8,
        point_id: u32,
        value: f64,
        timestamp_ms: u64,
    ) -> bool {
        // Only allow Control (2) and Adjustment (3) types
        if point_type != 2 && point_type != 3 {
            tracing::warn!(
                "set_action called with non-action type: {} (expected 2 or 3)",
                point_type
            );
            return false;
        }
        self.set(channel_id, point_type, point_id, value, value, timestamp_ms)
    }

    // ========== Snapshot API ==========

    /// Save current shared memory state to a snapshot file
    ///
    /// Uses atomic write: writes to temp file first, then renames to final path.
    pub fn save_snapshot(&self, path: &std::path::Path) -> Result<()> {
        self.mmap
            .flush()
            .context("Failed to flush mmap before snapshot")?;
        let current_slot_count = self.header().slot_count.load(Ordering::Acquire) as usize;
        save_snapshot_impl(&self.mmap, current_slot_count, path, "Writer")
    }

    /// Reconfigure the existing SHM file in place for updated channel point counts.
    ///
    /// This keeps the same underlying file/inode alive so existing mmaps are not
    /// invalidated by truncation. All point slots are cleared before the new
    /// layout is published to avoid stale values being interpreted as new points.
    pub fn reconfigure_existing(
        config: &SharedConfig,
        channel_points: &ChannelPointCounts,
    ) -> Result<Self> {
        let path = config.path();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("Failed to open {:?} for SHM reconfigure", path))?;

        // Match the safety gating in open_for_actions / UnifiedReader::open:
        // a truncated file makes the header cast immediate UB. Without this
        // check the reconfigure path was the one remaining hole — A.1
        // patched the three open paths but missed this one.
        verify_file_min_size(&file, path)?;

        let mut mmap = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .with_context(|| "Failed to mmap for SHM reconfigure")?
        };

        // SAFETY: mmap.len() >= sizeof(UnifiedHeader) per verify_file_min_size.
        let max_slots = {
            let header = unsafe { &*(mmap.as_ptr() as *const UnifiedHeader) };
            validate_reconfigurable_header(header)?
        };

        // Ensure the mmap actually covers the declared slot array. Otherwise
        // the slot_clear loop and pointer arithmetic below could run off
        // the mapped region.
        verify_mmap_covers_slots(mmap.len(), max_slots, path)?;

        // Read old slot_count before allocating new layout
        let old_slot_count = {
            let header = unsafe { &*(mmap.as_ptr() as *const UnifiedHeader) };
            header.slot_count.load(Ordering::Acquire) as usize
        };
        if old_slot_count > max_slots as usize {
            bail!(
                "Invalid SHM header: slot_count {} exceeds max_slots {}",
                old_slot_count,
                max_slots
            );
        }

        let (channel_layouts, slot_count) = allocate_layouts(channel_points);
        if slot_count > max_slots as usize {
            bail!("Too many slots: {} (max={})", slot_count, max_slots);
        }

        // Mark reconfigure-in-progress by flipping writer_generation to an
        // ODD value (low bit = 1). Readers/dispatchers gate on this: any
        // SHM consumer that reads writer_generation while odd MUST skip
        // this tick / refuse this dispatch — the slot array is being
        // cleared and torn reads would produce all-zero garbage. The
        // matching even-store after the slot init below restores normal
        // operation with the generation incremented by 2 (odd→even),
        // which still trips ShmDispatch's mismatch check so the next
        // dispatch picks up the new layout.
        {
            // SAFETY: mmap covers the header; we hold &mut mmap so no
            // other writer in this process can racing-bump the counter.
            let header = unsafe { &mut *(mmap.as_mut_ptr() as *mut UnifiedHeader) };
            let prev = header.writer_generation.fetch_add(1, Ordering::Release);
            debug_assert!(
                prev & 1 == 0,
                "reconfigure entered with odd writer_generation = {} — nested reconfigure?",
                prev
            );
        }

        // Reset only the previously-used slot region (not the entire 2GB sparse file).
        // Clear max(old, new) slots to cover both old stale data and any new slots.
        let clear_slots = old_slot_count.max(slot_count);
        let clear_end = slot_offset() + clear_slots * std::mem::size_of::<PointSlot>();
        mmap[slot_offset()..clear_end].fill(0);

        // SHM v3 uses NaN, not zero, as the "unwritten" sentinel. The byte
        // clear above resets seq/dirty/timestamp; this pass restores value/raw
        // to NaN so routing reload cannot fabricate real 0.0 readings.
        // SAFETY: clear_end was computed from max(old_slot_count, slot_count),
        // both bounded by max_slots. The mmap covers max_slots PointSlot values,
        // and PointSlot is #[repr(C, align(32))].
        let slots_ptr =
            unsafe { mmap.as_mut_ptr().add(slot_offset()) as *const crate::core::slot::PointSlot };
        for i in 0..clear_slots {
            // SAFETY: i < clear_slots and clear_slots is within the mmap slot region.
            let slot = unsafe { &*slots_ptr.add(i) };
            slot.init_unwritten();
        }

        // FULL BARRIER: ensure all slot resets are globally visible
        // before the new routing_hash/slot_count are published.
        // Without this fence, a reader on ARM64 could see the new
        // routing_hash but read stale slot data.
        fence(Ordering::SeqCst);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_millis() as u64;

        let header = unsafe { &mut *(mmap.as_mut_ptr() as *mut UnifiedHeader) };
        header
            .slot_count
            .store(slot_count as u32, Ordering::Release);
        header.last_update_ts.store(now_ms, Ordering::Relaxed);
        header.writer_heartbeat.store(now_ms, Ordering::Relaxed);
        header
            .routing_hash
            .store(channel_points.layout_hash(), Ordering::Release);
        // Restore even parity (was bumped to odd at the start of this
        // function). Net effect: generation incremented by 2. ShmDispatch's
        // mismatch check still trips on the next dispatch (any change),
        // and the low-bit "reconfigure in progress" sentinel is cleared
        // so normal consumers resume.
        let post = header.writer_generation.fetch_add(1, Ordering::Release);
        debug_assert!(
            post & 1 == 1,
            "reconfigure exited with even writer_generation = {} — sentinel logic broken",
            post
        );

        // Flush mmap to backing file — ensures cross-process visibility
        // on systems where mmap coherency isn't guaranteed (H3).
        mmap.flush()
            .with_context(|| "Failed to flush mmap after reconfigure")?;

        tracing::info!(
            "Reconfigured unified shared memory in place: {:?}, slots={}, channels={}",
            path,
            slot_count,
            channel_layouts.iter().filter(|l| l.is_valid()).count()
        );

        Ok(Self {
            mmap,
            path: path.to_path_buf(),
            max_slots,
            slot_count,
            channel_layouts,
            dirty_words: new_dirty_words(slot_count),
        })
    }

    /// Restore from snapshot file
    ///
    /// Creates a new UnifiedWriter and loads PointSlot data from the snapshot.
    /// The header is re-initialized from current config, only point data is restored.
    ///
    /// ## Data Validation
    ///
    /// Each slot is validated before restoration:
    /// - NaN values are skipped (logged as warning)
    /// - Infinite values are skipped (logged as warning)
    /// - New slots (not in snapshot) are initialized to default (0.0, 0.0, 0)
    ///
    /// ## Routing Hash Check
    ///
    /// If the snapshot's routing hash differs from current config, a warning is logged
    /// but restoration continues. This handles the case where routing changed after
    /// the snapshot was created.
    ///
    /// # Arguments
    /// - `config`: Shared memory configuration
    /// - `snapshot_path`: Path to the snapshot file
    /// - `channel_points`: Channel point counts for slot allocation
    ///
    /// # Returns
    /// - `Ok(Self)` with restored data
    /// - `Err` if snapshot is invalid or incompatible
    pub fn restore_from_snapshot(
        config: &SharedConfig,
        snapshot_path: &std::path::Path,
        channel_points: &ChannelPointCounts,
    ) -> Result<Self> {
        use std::io::Read;

        // First create a fresh writer with current config
        // Note: create() already initializes all slots to default values (zeroed)
        let writer = Self::create(config, channel_points)?;

        // Read snapshot file
        let mut file = std::fs::File::open(snapshot_path)
            .with_context(|| format!("Failed to open snapshot file: {:?}", snapshot_path))?;

        let mut snapshot_data = Vec::new();
        file.read_to_end(&mut snapshot_data)
            .with_context(|| "Failed to read snapshot file")?;

        // Validate snapshot header
        if snapshot_data.len() < std::mem::size_of::<UnifiedHeader>() {
            bail!("Snapshot file too small: {} bytes", snapshot_data.len());
        }

        // Read header fields individually from unaligned buffer.
        // Cannot use read_unaligned on UnifiedHeader because it contains
        // AtomicU32/AtomicU64 fields — creating Atomics from unaligned
        // memory is UB (hardware atomic instructions require alignment).
        //
        // UnifiedHeader layout (#[repr(C, align(64))]):
        //   offset 0:  magic (u64)
        //   offset 8:  version (u32)
        //   offset 12: max_slots (u32)
        //   offset 16: slot_count (AtomicU32 → read as u32)
        //   offset 20: _pad (4 bytes)
        //   offset 24: last_update_ts (AtomicU64)
        //   offset 32: writer_heartbeat (AtomicU64)
        //   offset 40: routing_hash (AtomicU64 → read as u64)
        //   offset 48: _reserved (16 bytes)
        let snap = &snapshot_data;
        let snap_magic = read_u64_ne(snap, 0, "header.magic")?;
        let snap_version = read_u32_ne(snap, 8, "header.version")?;
        let snap_slot_count_val = read_u32_ne(snap, 16, "header.slot_count")?;
        let snap_routing_hash = read_u64_ne(snap, 40, "header.routing_hash")?;

        if snap_magic != UNIFIED_MAGIC {
            bail!(
                "Invalid snapshot magic: expected 0x{:X}, got 0x{:X}",
                UNIFIED_MAGIC,
                snap_magic
            );
        }
        if snap_version != UNIFIED_VERSION {
            // v2 snapshots used 0.0 as the slot default — restoring them in v3
            // would re-introduce the pseudo-zero contamination this layout
            // bump fixed. Refuse the restore so the writer starts fresh; the
            // next protocol poll repopulates each live slot with a finite
            // value, and downstream readers see "missing" instead of
            // counterfeit zeros for whatever hasn't been re-polled yet.
            bail!(
                "Snapshot version mismatch: expected {}, got {} — refusing restore (start fresh)",
                UNIFIED_VERSION,
                snap_version
            );
        }

        // Check layout hash (warn but don't fail — channel config may have changed since snapshot)
        let current_hash = channel_points.layout_hash();
        if current_hash != snap_routing_hash {
            tracing::warn!(
                "Snapshot layout hash differs: snapshot=0x{:016X}, current=0x{:016X}",
                snap_routing_hash,
                current_hash
            );
        }

        let snapshot_slot_count = snap_slot_count_val as usize;

        // Determine how many slots to restore (min of snapshot and current allocation)
        let slots_to_restore = snapshot_slot_count.min(writer.slot_count);

        if slots_to_restore == 0 {
            tracing::warn!("Snapshot has no slots to restore");
            return Ok(writer);
        }

        // Calculate data ranges
        let header_size = slot_offset();
        let slot_size = std::mem::size_of::<PointSlot>();
        let snapshot_data_end = header_size + slots_to_restore * slot_size;

        if snapshot_data.len() < snapshot_data_end {
            bail!(
                "Snapshot file truncated: expected at least {} bytes, got {}",
                snapshot_data_end,
                snapshot_data.len()
            );
        }

        // Restore slots one by one with validation
        let mut restored_count = 0usize;
        let mut skipped_unwritten = 0usize;
        let mut skipped_invalid = 0usize;

        for i in 0..slots_to_restore {
            let slot_offset_in_file = header_size + i * slot_size;
            // Read PointSlot fields individually from unaligned buffer.
            // Cannot use read_unaligned on PointSlot because it contains
            // AtomicU64 fields — atomics require alignment for correctness.
            //
            // PointSlot layout (#[repr(C, align(32))]):
            //   offset 0:  value_bits (AtomicU64 → read as u64)
            //   offset 8:  timestamp  (AtomicU64 → read as u64)
            //   offset 16: raw_bits   (AtomicU64 → read as u64)
            //   offset 24: seq        (AtomicU32 → not needed for restore)
            //   offset 28: dirty      (AtomicU32 → not needed for restore)
            let sb = &snapshot_data[slot_offset_in_file..slot_offset_in_file + slot_size];
            let value = f64::from_bits(read_u64_ne(sb, 0, "slot.value_bits")?);
            let timestamp = read_u64_ne(sb, 8, "slot.timestamp")?;
            let raw = f64::from_bits(read_u64_ne(sb, 16, "slot.raw_bits")?);

            // NaN is the "unwritten" sentinel in v3 — leave the writer at its
            // create() default (already NaN). Skipping `set_direct` here also
            // leaves seq=0, so ShmRedisSync.full_scan won't push these slots
            // to Redis until the next protocol poll fills them.
            if value.is_nan() && raw.is_nan() {
                skipped_unwritten += 1;
                continue;
            }

            // Reject infinities and half-NaN combinations as data corruption.
            if !value.is_finite() || !raw.is_finite() {
                tracing::warn!(
                    "Skipping corrupt slot {}: value={}, raw={} (mixed finiteness or Infinity)",
                    i,
                    value,
                    raw
                );
                skipped_invalid += 1;
                continue;
            }

            // Write to current slot
            writer.set_direct(i, value, raw, timestamp);
            restored_count += 1;
        }

        // New slots (if current config has more than snapshot) are already initialized
        // to default values (0.0, 0.0, 0) by create()
        let new_slots = writer.slot_count.saturating_sub(snapshot_slot_count);

        // Update timestamps
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_millis() as u64;
        writer
            .header()
            .last_update_ts
            .store(now_ms, Ordering::Relaxed);
        writer
            .header()
            .writer_heartbeat
            .store(now_ms, Ordering::Relaxed);

        tracing::info!(
            "Snapshot restored: {:?}, restored={}, skipped_unwritten={}, skipped_invalid={}, new_slots={}",
            snapshot_path,
            restored_count,
            skipped_unwritten,
            skipped_invalid,
            new_slots
        );

        if skipped_invalid > 0 {
            tracing::warn!(
                "{} slots skipped due to invalid data (NaN/Infinity)",
                skipped_invalid
            );
        }

        if new_slots > 0 {
            tracing::info!(
                "{} new slots initialized to default values (routing config changed)",
                new_slots
            );
        }

        Ok(writer)
    }
}

// ========== UnifiedReader ==========

/// Unified shared memory reader
///
/// Multiple readers allowed (modsrv, monarch, etc.).
/// Builds indexes from ChannelPointCounts using same allocation algorithm.
pub struct UnifiedReader {
    mmap: Mmap,
    max_slots: u32,
    slot_count: usize,
    /// Channel layouts (Vec index by channel_id)
    channel_layouts: Vec<ChannelLayout>,
    /// For monarch API: list of valid channel IDs
    channel_ids: Vec<u32>,
}

impl UnifiedReader {
    /// Open shared memory and build indexes from ChannelPointCounts
    pub fn open(config: &SharedConfig, channel_points: &ChannelPointCounts) -> Result<Self> {
        let path = config.path();

        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .with_context(|| format!("Failed to open {:?}", path))?;

        // Guard against truncated/corrupt files before any unsafe cast.
        verify_file_min_size(&file, path)?;

        // SAFETY: file is at least header-sized per verify_file_min_size.
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .with_context(|| "Failed to mmap")?
        };

        // SAFETY: mmap.len() >= sizeof(UnifiedHeader) by verify_file_min_size.
        let header = unsafe { &*(mmap.as_ptr() as *const UnifiedHeader) };
        let (max_slots, slot_count) = validate_shm_header(header, channel_points)?;

        verify_mmap_covers_slots(mmap.len(), max_slots, path)?;

        let (channel_layouts, calculated_slots) = allocate_layouts(channel_points);
        verify_slot_count(slot_count, calculated_slots)?;

        let channel_ids: Vec<u32> = channel_layouts
            .iter()
            .enumerate()
            .filter(|(_, l)| l.is_valid())
            .map(|(id, _)| id as u32)
            .collect();

        tracing::info!(
            "Opened unified reader: {} slots, {} channels",
            slot_count,
            channel_ids.len()
        );

        Ok(Self {
            mmap,
            max_slots,
            slot_count,
            channel_layouts,
            channel_ids,
        })
    }

    /// Open shared memory read-only without layout validation.
    ///
    /// For debug tools (monarch) that don't have channel point data.
    /// Reads slot_count from header but cannot navigate to specific channel points.
    pub fn open_raw(config: &SharedConfig) -> Result<Self> {
        let path = config.path();
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .with_context(|| format!("Failed to open {:?}", path))?;

        // Guard against truncated/corrupt files before any unsafe cast.
        verify_file_min_size(&file, path)?;

        // SAFETY: file is at least header-sized per verify_file_min_size.
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .with_context(|| "Failed to mmap")?
        };

        // SAFETY: mmap.len() >= sizeof(UnifiedHeader) by verify_file_min_size.
        let header = unsafe { &*(mmap.as_ptr() as *const UnifiedHeader) };
        if header.magic != UNIFIED_MAGIC {
            bail!(
                "Invalid magic: expected 0x{:X}, got 0x{:X}",
                UNIFIED_MAGIC,
                header.magic
            );
        }

        let max_slots = header.max_slots;
        verify_mmap_covers_slots(mmap.len(), max_slots, path)?;
        let slot_count = header.slot_count.load(Ordering::Acquire) as usize;

        tracing::info!(
            "Opened raw reader: {} slots (no layout validation)",
            slot_count
        );

        Ok(Self {
            mmap,
            max_slots,
            slot_count,
            channel_layouts: Vec::new(),
            channel_ids: Vec::new(),
        })
    }

    impl_shm_accessors!(mmap);

    // ========== Point Query API ==========

    /// Get value by channel key
    #[inline]
    pub fn get_channel(
        &self,
        channel_id: u32,
        point_type: u8,
        point_id: u32,
    ) -> Option<(f64, u64)> {
        let layout = self.channel_layouts.get(channel_id as usize)?;
        let slot = layout.slot(point_type, point_id)?;
        let point = self.slot_at(slot);
        let (value, _raw, timestamp) = point.load_consistent()?;
        Some((value, timestamp))
    }

    /// Get value by instance key (requires RoutingCache for C2M lookup)
    ///
    /// For Measurement (type=0): looks up C2M routing
    /// For Action (type=1): looks up M2C routing
    #[inline]
    pub fn get_instance(
        &self,
        instance_id: u32,
        instance_type: u8,
        point_id: u32,
        routing_cache: &RoutingCache,
    ) -> Option<(f64, u64)> {
        // Measurement: instance reads channel T/S via C2M
        // Action: instance writes channel C/A via M2C
        let (channel_id, channel_type, channel_point_id) = if instance_type == 0 {
            // Measurement - need reverse lookup: find which channel maps to this instance point
            // C2M: (channel, type, point) → (instance, point)
            // We need: (instance, point) → (channel, type, point)
            // This requires iterating C2M - inefficient but works for now
            let mut found = None;
            for ((ch_id, pt_type, ch_pt_id), target) in routing_cache.c2m_iter() {
                if target.instance_id == instance_id && target.point_id == point_id {
                    found = Some((ch_id, pt_type.to_u8(), ch_pt_id));
                    break;
                }
            }
            found?
        } else {
            // Action - lookup M2C (try Control first, then Adjustment)
            // Instance "Action" can map to either C (Control) or A (Adjustment)
            let target = routing_cache
                .lookup_m2c_by_parts(instance_id, PointType::Control, point_id)
                .or_else(|| {
                    routing_cache.lookup_m2c_by_parts(instance_id, PointType::Adjustment, point_id)
                })?;
            (
                target.channel_id,
                target.point_type.to_u8(),
                target.point_id,
            )
        };

        self.get_channel(channel_id, channel_type, channel_point_id)
    }

    // ========== Monarch Compatible API ==========

    /// Get all channel IDs
    #[inline]
    pub fn channel_ids(&self) -> &[u32] {
        &self.channel_ids
    }

    /// Get instance IDs from RoutingCache
    pub fn instance_ids(&self, routing_cache: &RoutingCache) -> Vec<u32> {
        let mut ids = std::collections::HashSet::new();
        for (_, target) in routing_cache.c2m_iter() {
            ids.insert(target.instance_id);
        }
        ids.into_iter().collect()
    }

    /// Iterate channel points of given type
    pub fn iter_channel_points<F>(&self, channel_id: u32, point_type: PointType, mut f: F)
    where
        F: FnMut(u32, f64),
    {
        if let Some(layout) = self.channel_layouts.get(channel_id as usize) {
            let type_idx = point_type.to_u8() as usize;
            let count = layout.type_counts[type_idx];
            for pt_id in 0..count {
                if let Some(slot) = layout.slot(point_type.to_u8(), pt_id) {
                    let point = self.slot_at(slot);
                    f(pt_id, point.get_value());
                }
            }
        }
    }

    /// Iterate instance Measurement points (requires RoutingCache)
    pub fn iter_instance_measurements<F>(
        &self,
        instance_id: u32,
        routing_cache: &RoutingCache,
        mut f: F,
    ) where
        F: FnMut(u32, f64),
    {
        for ((ch_id, pt_type, ch_pt_id), target) in routing_cache.c2m_iter() {
            if target.instance_id == instance_id
                && let Some((val, _ts)) = self.get_channel(ch_id, pt_type.to_u8(), ch_pt_id)
            {
                f(target.point_id, val);
            }
        }
    }

    /// Iterate instance Action points (requires RoutingCache)
    pub fn iter_instance_actions<F>(&self, instance_id: u32, routing_cache: &RoutingCache, mut f: F)
    where
        F: FnMut(u32, f64),
    {
        // M2C: (instance, point_type, point) → (channel, type, point)
        // Filter by instance_id
        for ((inst_id, _inst_type, inst_pt_id), target) in routing_cache.m2c_iter() {
            if inst_id == instance_id
                && let Some((val, _ts)) = self.get_channel(
                    target.channel_id,
                    target.point_type.to_u8(),
                    target.point_id,
                )
            {
                f(inst_pt_id, val);
            }
        }
    }

    // ========== Stats ==========

    /// Get writer heartbeat
    #[inline]
    pub fn writer_heartbeat(&self) -> u64 {
        self.header().writer_heartbeat.load(Ordering::Relaxed)
    }

    /// Check if writer is alive based on heartbeat timestamp
    #[inline]
    pub fn is_writer_alive(&self, timeout_ms: u64) -> bool {
        let heartbeat = self.writer_heartbeat();
        if heartbeat == 0 {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_millis() as u64;
        now.saturating_sub(heartbeat) < timeout_ms
    }

    // ========== Snapshot API ==========

    /// Save current shared memory state to a snapshot file (read-only snapshot)
    ///
    /// This is useful for readers like modsrv to save state before shutdown.
    /// Uses atomic write: writes to temp file first, then renames to final path.
    pub fn save_snapshot(&self, path: &std::path::Path) -> Result<()> {
        save_snapshot_impl(&self.mmap, self.slot_count, path, "Reader")
    }
}

// ========== Pure-infra contract: SlotIo ==========
//
// UnifiedWriter implements the business-unaware slot I/O contract declared
// in `core::slot_io`. The trait deliberately omits every channel/point-type
// adapter on UnifiedWriter — anything reachable via `&dyn SlotIo` is, by
// type system enforcement, infrastructure only.

impl crate::core::slot_io::SlotIo for UnifiedWriter {
    #[inline]
    fn slot_count(&self) -> usize {
        self.slot_count
    }

    fn read_slot(&self, index: usize) -> Option<crate::core::slot_io::SlotRead> {
        if index >= self.slot_count {
            return None;
        }
        let (value, raw, timestamp_ms) = self.slot_at(index).try_load_consistent()?;
        Some(crate::core::slot_io::SlotRead {
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
        UnifiedWriter::take_dirty_slots(self)
    }

    fn generation(&self) -> u64 {
        UnifiedWriter::generation(self)
    }

    fn writer_heartbeat(&self) -> u64 {
        UnifiedWriter::writer_heartbeat(self)
    }

    fn header(&self) -> &UnifiedHeader {
        UnifiedWriter::header(self)
    }
}

// ========== Tests ==========

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
#[allow(clippy::approx_constant)] // Test values, not actual PI
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::tempdir;
    use voltage_routing::RoutingCache;

    fn test_config(dir: &std::path::Path) -> SharedConfig {
        SharedConfig::default()
            .with_path(dir.join("test.shm"))
            .with_max_slots(1000)
    }

    /// Build ChannelPointCounts matching the test topology:
    /// - channel 1001: T:3, C:1 (maps to instance 23)
    /// - channel 1002: S:1     (maps to instance 24)
    fn test_channel_points() -> ChannelPointCounts {
        let mut map = BTreeMap::new();
        map.insert(1001, [3u32, 0, 1, 0]); // T=3, S=0, C=1, A=0
        map.insert(1002, [0u32, 1, 0, 0]); // T=0, S=1, C=0, A=0
        ChannelPointCounts::from_map(map)
    }

    /// Build RoutingCache for tests that still exercise instance-based lookup APIs.
    fn test_routing_cache() -> RoutingCache {
        let mut c2m = std::collections::HashMap::new();
        let mut m2c = std::collections::HashMap::new();
        let c2c = std::collections::HashMap::new();

        c2m.insert("1001:T:0".to_string(), "23:M:0".to_string());
        c2m.insert("1001:T:1".to_string(), "23:M:1".to_string());
        c2m.insert("1001:T:2".to_string(), "23:M:2".to_string());
        c2m.insert("1002:S:0".to_string(), "24:M:0".to_string());
        m2c.insert("23:C:0".to_string(), "1001:C:0".to_string());

        RoutingCache::from_maps(c2m, m2c, c2c)
    }

    fn setup_test_env() -> (tempfile::TempDir, SharedConfig, ChannelPointCounts) {
        let dir = tempdir().unwrap();
        let config = test_config(dir.path());
        let channel_points = test_channel_points();
        (dir, config, channel_points)
    }

    /// Create writer, run setup closure, flush, then open reader
    fn write_and_open_reader(
        config: &SharedConfig,
        channel_points: &ChannelPointCounts,
        setup: impl FnOnce(&UnifiedWriter),
    ) -> UnifiedReader {
        let writer = UnifiedWriter::create(config, channel_points).unwrap();
        setup(&writer);
        writer.flush().unwrap();
        UnifiedReader::open(config, channel_points).unwrap()
    }

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<UnifiedHeader>(), 64);
    }

    #[test]
    fn test_file_size() {
        // Header (64B) + PointSlot[1000] (32B each) = 64 + 32000 = 32064
        assert_eq!(calculate_file_size(1000), 64 + 32 * 1000);
    }

    #[test]
    fn test_allocate_layouts() {
        let channel_points = test_channel_points();
        let (layouts, slot_count) = allocate_layouts(&channel_points);

        // Should have layouts up to channel 1002
        assert!(layouts.len() >= 1003);

        // Channel 1001: T:3, C:1 = 4 points
        let layout_1001 = &layouts[1001];
        assert_eq!(layout_1001.type_counts[0], 3); // T
        assert_eq!(layout_1001.type_counts[2], 1); // C
        assert_eq!(layout_1001.total_points, 4);

        // Channel 1002: S:1 = 1 point
        let layout_1002 = &layouts[1002];
        assert_eq!(layout_1002.type_counts[1], 1); // S
        assert_eq!(layout_1002.total_points, 1);

        // Total slots
        assert_eq!(slot_count, 5); // 4 + 1
    }

    #[test]
    fn test_writer_create() {
        let (_dir, config, channel_points) = setup_test_env();
        let writer = UnifiedWriter::create(&config, &channel_points).unwrap();
        assert_eq!(writer.slot_count(), 5);
        assert_eq!(writer.max_slots(), 1000);
    }

    #[test]
    fn test_write_read() {
        let (_dir, config, channel_points) = setup_test_env();
        let reader = write_and_open_reader(&config, &channel_points, |w| {
            assert!(w.set(1001, 0, 0, 3.14, 3.14, 1705234567890));
            assert!(w.set(1001, 0, 1, 2.71, 2.71, 1705234567890));
        });
        assert_eq!(reader.slot_count(), 5);

        let (val, ts) = reader.get_channel(1001, 0, 0).unwrap();
        assert!((val - 3.14).abs() < 1e-10);
        assert_eq!(ts, 1705234567890);

        let (val2, _) = reader.get_channel(1001, 0, 1).unwrap();
        assert!((val2 - 2.71).abs() < 1e-10);
    }

    #[test]
    fn test_instance_lookup() {
        let (_dir, config, channel_points) = setup_test_env();
        let routing = test_routing_cache();
        let reader = write_and_open_reader(&config, &channel_points, |w| {
            w.set(1001, 0, 1, 42.0, 42.0, 100);
        });
        let (val, _) = reader.get_instance(23, 0, 1, &routing).unwrap();
        assert!((val - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_monarch_api() {
        let (_dir, config, channel_points) = setup_test_env();
        let routing = test_routing_cache();
        let reader = write_and_open_reader(&config, &channel_points, |w| {
            w.set(1001, 0, 0, 1.0, 1.0, 100);
            w.set(1001, 0, 1, 2.0, 2.0, 100);
            w.set(1001, 0, 2, 3.0, 3.0, 100);
        });

        assert!(reader.channel_ids().contains(&1001));
        assert!(reader.channel_ids().contains(&1002));

        let mut sum = 0.0;
        reader.iter_channel_points(1001, PointType::Telemetry, |_pt_id, val| {
            sum += val;
        });
        assert!((sum - 6.0).abs() < 1e-10);

        let mut count = 0;
        reader.iter_instance_measurements(23, &routing, |_pt_id, _val| {
            count += 1;
        });
        assert_eq!(count, 3);
    }

    #[test]
    fn test_direct_write() {
        let (_dir, config, channel_points) = setup_test_env();
        let writer = UnifiedWriter::create(&config, &channel_points).unwrap();
        let slot = writer.lookup(1001, 0, 0).unwrap();
        writer.set_direct(slot, 99.0, 99.0, 100);
        writer.flush().unwrap();

        let reader = UnifiedReader::open(&config, &channel_points).unwrap();
        let (val, _) = reader.get_channel(1001, 0, 0).unwrap();
        assert!((val - 99.0).abs() < 1e-10);
    }

    #[test]
    fn test_reconfigure_resets_slots_to_unwritten_nan() {
        let (_dir, config, channel_points) = setup_test_env();
        let writer = UnifiedWriter::create(&config, &channel_points).unwrap();
        let slot = writer.lookup(1001, 0, 0).unwrap();
        writer.set_direct(slot, 0.0, 0.0, 100);
        assert!(
            !writer.slot(slot).is_unwritten(),
            "real 0.0 write must not be treated as unwritten"
        );
        drop(writer);

        let writer = UnifiedWriter::reconfigure_existing(&config, &channel_points).unwrap();
        let slot = writer.lookup(1001, 0, 0).unwrap();
        let point = writer.slot(slot);
        let (value, raw, timestamp) = point.load_consistent().unwrap();

        assert!(value.is_nan(), "reconfigured value must be NaN sentinel");
        assert!(raw.is_nan(), "reconfigured raw must be NaN sentinel");
        assert_eq!(timestamp, 0);
        assert_eq!(point.seq_raw(), 0);
        assert!(point.is_unwritten());
    }

    #[test]
    fn test_reconfigure_rejects_corrupt_slot_count() {
        let (_dir, config, channel_points) = setup_test_env();
        let writer = UnifiedWriter::create(&config, &channel_points).unwrap();
        let max_slots = writer.max_slots();
        writer
            .header()
            .slot_count
            .store(max_slots.saturating_add(1), Ordering::Release);
        writer.flush().unwrap();
        drop(writer);

        let err = match UnifiedWriter::reconfigure_existing(&config, &channel_points) {
            Ok(_) => panic!("reconfigure must reject corrupt slot_count"),
            Err(err) => err,
        };
        let message = err.to_string();
        assert!(
            message.contains("slot_count") && message.contains("exceeds max_slots"),
            "expected corrupt header error, got: {message}"
        );
    }
}
