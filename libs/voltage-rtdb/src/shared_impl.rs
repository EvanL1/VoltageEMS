//! Shared Memory RTDB Implementation (Round 115-117)
//!
//! Provides cross-process shared memory for zero-latency data sharing between
//! comsrv (writer) and modsrv (reader) Docker containers.
//!
//! # Architecture
//!
//! ```text
//! comsrv container                    modsrv container
//! ┌─────────────────┐                ┌─────────────────┐
//! │SharedVecRtdbWriter│              │SharedVecRtdbReader│
//! └────────┬────────┘                └────────┬────────┘
//!          │ mmap write               mmap read │
//!          ▼                                    ▼
//!     ┌─────────────────────────────────────────┐
//!     │     tmpfs volume: /shm/rtdb             │
//!     │     voltage-rtdb.shm (16MB)             │
//!     └─────────────────────────────────────────┘
//! ```
//!
//! # Memory Layout
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │ SharedHeader (64 bytes)                      │
//! ├──────────────────────────────────────────────┤
//! │ InstanceIndex[] (48 bytes each)              │
//! ├──────────────────────────────────────────────┤
//! │ PointSlot[] (32 bytes each, aligned)         │
//! └──────────────────────────────────────────────┘
//! ```

use crate::vec_impl::PointSlot;
use anyhow::{Context, Result};
use memmap2::{Mmap, MmapMut, MmapOptions};
use rustc_hash::FxHashMap;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use tracing::{debug, info, warn};

// ========== Constants ==========

/// Magic number for validation: "VOLTAGE_" in ASCII
pub const SHARED_MAGIC: u64 = 0x564F4C544147455F;

/// Current shared memory format version
pub const SHARED_VERSION: u32 = 1;

/// Default shared memory file path
pub const DEFAULT_SHM_PATH: &str = "/shm/rtdb/voltage-rtdb.shm";

// ========== SharedHeader ==========

/// Shared memory header (64 bytes, cache-line aligned)
///
/// Located at offset 0 of the shared memory file.
/// Contains metadata and synchronization fields.
#[repr(C, align(64))]
pub struct SharedHeader {
    /// Magic number for validation (0x564F4C544147455F = "VOLTAGE_")
    pub magic: u64,
    /// Format version for compatibility checking
    pub version: u32,
    /// Number of registered instances
    pub instance_count: AtomicU32,
    /// Total number of points across all instances
    pub total_points: AtomicU32,
    /// Padding for alignment
    pub _pad1: u32,
    /// Offset to InstanceIndex array (from file start)
    pub index_offset: u64,
    /// Offset to PointSlot data array (from file start)
    pub data_offset: u64,
    /// Last data update timestamp (milliseconds since epoch)
    pub last_update_ts: AtomicU64,
    /// Writer heartbeat timestamp (for liveness check)
    pub writer_heartbeat: AtomicU64,
}

impl SharedHeader {
    /// Check if the header is valid
    pub fn is_valid(&self) -> bool {
        self.magic == SHARED_MAGIC && self.version == SHARED_VERSION
    }
}

// ========== InstanceIndex ==========

/// Instance index entry (48 bytes)
///
/// Maps instance_id to its point data locations in the shared memory.
#[derive(Default)]
#[repr(C)]
pub struct InstanceIndex {
    /// Instance identifier
    pub instance_id: u32,
    /// Number of measurement points
    pub measurement_count: u16,
    /// Number of action points
    pub action_count: u16,
    /// Offset to measurement slots (relative to data_offset)
    pub measurement_offset: u32,
    /// Offset to action slots (relative to data_offset)
    pub action_offset: u32,
    /// Hot point IDs cache (most frequently accessed)
    pub hot_point_ids: [u32; 8],
}

// ========== SharedConfig ==========

/// Configuration for shared memory
#[derive(Debug, Clone)]
pub struct SharedConfig {
    /// Path to shared memory file
    pub path: PathBuf,
    /// Maximum number of instances
    pub max_instances: usize,
    /// Maximum points per instance (measurement + action)
    pub max_points_per_instance: usize,
}

impl Default for SharedConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from(DEFAULT_SHM_PATH),
            max_instances: 64,
            max_points_per_instance: 256,
        }
    }
}

impl SharedConfig {
    /// Create config with custom path
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = path.into();
        self
    }

    /// Calculate total file size needed
    pub fn calculate_file_size(&self) -> usize {
        let header_size = std::mem::size_of::<SharedHeader>();
        let index_size = self.max_instances * std::mem::size_of::<InstanceIndex>();
        // Each instance can have max_points_per_instance for both M and A
        let data_size =
            self.max_instances * self.max_points_per_instance * std::mem::size_of::<PointSlot>();
        header_size + index_size + data_size
    }
}

// ========== Helper Functions ==========

/// Get current timestamp in milliseconds
#[inline]
pub fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ========== InstanceLayout (Round 120) ==========

/// Per-instance layout with Vec direct indexing for O(1) point lookup
///
/// Instead of FxHashMap<(instance_id, point_type, point_id), offset>,
/// we use Vec direct indexing: point_to_offset[point_id] = slot_offset
struct InstanceLayout {
    /// Base offset for measurement slots (relative to data_offset)
    measurement_base: usize,
    /// Base offset for action slots (relative to data_offset)
    action_base: usize,
    /// point_id → slot_offset mapping for measurements (Vec direct index)
    /// Uses u32::MAX as sentinel for non-existent points
    measurement_point_to_offset: Box<[u32]>,
    /// point_id → slot_offset mapping for actions (Vec direct index)
    action_point_to_offset: Box<[u32]>,
    /// Max valid measurement point_id (for bounds check)
    max_measurement_point_id: u32,
    /// Max valid action point_id (for bounds check)
    max_action_point_id: u32,
}

impl InstanceLayout {
    const INVALID_OFFSET: u32 = u32::MAX;

    fn new(
        measurement_base: usize,
        action_base: usize,
        measurement_points: &[u32],
        action_points: &[u32],
    ) -> Self {
        // Build measurement mapping
        let max_m = measurement_points.iter().copied().max().unwrap_or(0);
        let mut m_map = vec![Self::INVALID_OFFSET; (max_m + 1) as usize];
        for (slot_idx, &point_id) in measurement_points.iter().enumerate() {
            // slot_offset relative to measurement_base
            let offset = slot_idx * std::mem::size_of::<crate::vec_impl::PointSlot>();
            m_map[point_id as usize] = offset as u32;
        }

        // Build action mapping
        let max_a = action_points.iter().copied().max().unwrap_or(0);
        let mut a_map = vec![Self::INVALID_OFFSET; (max_a + 1) as usize];
        for (slot_idx, &point_id) in action_points.iter().enumerate() {
            let offset = slot_idx * std::mem::size_of::<crate::vec_impl::PointSlot>();
            a_map[point_id as usize] = offset as u32;
        }

        Self {
            measurement_base,
            action_base,
            measurement_point_to_offset: m_map.into_boxed_slice(),
            action_point_to_offset: a_map.into_boxed_slice(),
            max_measurement_point_id: max_m,
            max_action_point_id: max_a,
        }
    }

    /// Get slot offset for a point (relative to data_offset)
    /// Returns None if point not registered
    #[inline]
    fn get_slot_offset(&self, point_type: u8, point_id: u32) -> Option<usize> {
        if point_type == 0 {
            // Measurement
            if point_id > self.max_measurement_point_id {
                return None;
            }
            let rel_offset = self.measurement_point_to_offset[point_id as usize];
            if rel_offset == Self::INVALID_OFFSET {
                return None;
            }
            Some(self.measurement_base + rel_offset as usize)
        } else {
            // Action
            if point_id > self.max_action_point_id {
                return None;
            }
            let rel_offset = self.action_point_to_offset[point_id as usize];
            if rel_offset == Self::INVALID_OFFSET {
                return None;
            }
            Some(self.action_base + rel_offset as usize)
        }
    }
}

// ========== SharedVecRtdbWriter (Round 116, optimized Round 120) ==========

/// Shared memory writer for comsrv
///
/// Creates and manages the shared memory file, writing point data
/// that can be read by modsrv's SharedVecRtdbReader.
///
/// # Performance (Round 120)
/// Uses Vec direct indexing for O(1) point lookup (~1-5ns vs ~20ns for FxHashMap).
pub struct SharedVecRtdbWriter {
    /// Memory-mapped file (read-write)
    mmap: MmapMut,
    /// Configuration
    config: SharedConfig,
    /// Instance ID → index in InstanceIndex array
    instance_indices: FxHashMap<u32, usize>,
    /// Instance ID → InstanceLayout (Vec direct indexing) - Round 120 optimization
    instance_layouts: FxHashMap<u32, InstanceLayout>,
    /// Next available instance index
    next_instance_idx: usize,
    /// Next available slot offset (bytes)
    next_slot_offset: usize,
}

impl SharedVecRtdbWriter {
    /// Create or open shared memory file for writing
    ///
    /// # Arguments
    /// * `config` - Shared memory configuration
    ///
    /// # Returns
    /// * `Ok(Self)` - Writer instance
    /// * `Err` - If file creation or mapping fails
    pub fn open(config: &SharedConfig) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = config.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        let total_size = config.calculate_file_size();
        info!(
            "SharedVecRtdbWriter: creating {} bytes at {:?}",
            total_size, config.path
        );

        // Create/truncate file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&config.path)
            .with_context(|| format!("Failed to open shared memory file: {:?}", config.path))?;

        file.set_len(total_size as u64)
            .context("Failed to set file size")?;

        // Memory map with read-write access
        let mmap = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .context("Failed to memory map file")?
        };

        let mut writer = Self {
            mmap,
            config: config.clone(),
            instance_indices: FxHashMap::default(),
            instance_layouts: FxHashMap::default(),
            next_instance_idx: 0,
            next_slot_offset: 0,
        };

        // Initialize header
        writer.init_header()?;

        Ok(writer)
    }

    /// Initialize the shared memory header
    fn init_header(&mut self) -> Result<()> {
        // Calculate offsets before borrowing header
        let index_offset = std::mem::size_of::<SharedHeader>() as u64;
        let data_offset = index_offset
            + (self.config.max_instances * std::mem::size_of::<InstanceIndex>()) as u64;

        // Initialize header fields
        {
            let header = self.header_mut();
            header.magic = SHARED_MAGIC;
            header.version = SHARED_VERSION;
            header.instance_count.store(0, Ordering::Release);
            header.total_points.store(0, Ordering::Release);
            header.index_offset = index_offset;
            header.data_offset = data_offset;
            header.last_update_ts.store(0, Ordering::Release);
            header
                .writer_heartbeat
                .store(timestamp_ms(), Ordering::Release);
        }

        // Flush to ensure visibility
        self.mmap.flush().context("Failed to flush header")?;

        debug!(
            "SharedVecRtdbWriter: header initialized, index_offset={}, data_offset={}",
            index_offset, data_offset
        );

        Ok(())
    }

    /// Get mutable reference to header
    fn header_mut(&mut self) -> &mut SharedHeader {
        unsafe { &mut *(self.mmap.as_mut_ptr() as *mut SharedHeader) }
    }

    /// Get reference to header
    fn header(&self) -> &SharedHeader {
        unsafe { &*(self.mmap.as_ptr() as *const SharedHeader) }
    }

    /// Get mutable reference to instance index at given position
    fn instance_index_mut(&mut self, idx: usize) -> &mut InstanceIndex {
        let header = self.header();
        let offset = header.index_offset as usize + idx * std::mem::size_of::<InstanceIndex>();
        unsafe { &mut *(self.mmap.as_mut_ptr().add(offset) as *mut InstanceIndex) }
    }

    /// Get reference to point slot at given offset (bytes from data_offset)
    fn slot_at(&self, slot_offset: usize) -> &PointSlot {
        let header = self.header();
        let offset = header.data_offset as usize + slot_offset;
        unsafe { &*(self.mmap.as_ptr().add(offset) as *const PointSlot) }
    }

    /// Register an instance with its measurement and action points
    ///
    /// # Arguments
    /// * `instance_id` - Instance identifier
    /// * `measurement_points` - Point IDs for measurements (inst:{id}:M)
    /// * `action_points` - Point IDs for actions (inst:{id}:A)
    ///
    /// # Returns
    /// * `Ok(())` - Registration successful
    /// * `Err` - If max instances exceeded
    pub fn register_instance(
        &mut self,
        instance_id: u32,
        measurement_points: &[u32],
        action_points: &[u32],
    ) -> Result<()> {
        if self.next_instance_idx >= self.config.max_instances {
            anyhow::bail!("Max instances ({}) exceeded", self.config.max_instances);
        }

        let total_points = measurement_points.len() + action_points.len();
        if total_points > self.config.max_points_per_instance {
            anyhow::bail!(
                "Instance {} has {} points, max is {}",
                instance_id,
                total_points,
                self.config.max_points_per_instance
            );
        }

        let idx = self.next_instance_idx;
        self.instance_indices.insert(instance_id, idx);

        // Allocate slots for measurement points
        let measurement_base = self.next_slot_offset;
        self.next_slot_offset += measurement_points.len() * std::mem::size_of::<PointSlot>();

        // Allocate slots for action points
        let action_base = self.next_slot_offset;
        self.next_slot_offset += action_points.len() * std::mem::size_of::<PointSlot>();

        // Build InstanceLayout with Vec direct indexing (Round 120)
        let layout = InstanceLayout::new(
            measurement_base,
            action_base,
            measurement_points,
            action_points,
        );
        self.instance_layouts.insert(instance_id, layout);

        // Update instance index
        let inst_idx = self.instance_index_mut(idx);
        inst_idx.instance_id = instance_id;
        inst_idx.measurement_count = measurement_points.len() as u16;
        inst_idx.action_count = action_points.len() as u16;
        inst_idx.measurement_offset = measurement_base as u32;
        inst_idx.action_offset = action_base as u32;

        // Cache hot point IDs (first 8)
        let hot_ids: Vec<u32> = measurement_points
            .iter()
            .chain(action_points.iter())
            .take(8)
            .copied()
            .collect();
        for (i, &pid) in hot_ids.iter().enumerate() {
            inst_idx.hot_point_ids[i] = pid;
        }

        // Update header counters
        let header = self.header_mut();
        header.instance_count.fetch_add(1, Ordering::Release);
        header
            .total_points
            .fetch_add(total_points as u32, Ordering::Release);

        self.next_instance_idx += 1;

        debug!(
            "Registered instance {} with {} measurement + {} action points",
            instance_id,
            measurement_points.len(),
            action_points.len()
        );

        Ok(())
    }

    /// Write a point value
    ///
    /// # Arguments
    /// * `instance_id` - Instance identifier
    /// * `point_type` - 0 for measurement, 1 for action
    /// * `point_id` - Point identifier
    /// * `value` - Engineering value
    /// * `timestamp` - Timestamp in milliseconds
    ///
    /// # Returns
    /// * `true` - Point found and updated
    /// * `false` - Point not registered
    ///
    /// # Performance (Round 120)
    /// Uses Vec direct indexing for O(1) lookup (~1-5ns vs ~20ns for FxHashMap).
    #[inline]
    pub fn set(
        &self,
        instance_id: u32,
        point_type: u8,
        point_id: u32,
        value: f64,
        timestamp: u64,
    ) -> bool {
        // Two-level lookup: instance_id → layout → point_id (Vec direct index)
        if let Some(layout) = self.instance_layouts.get(&instance_id) {
            if let Some(slot_offset) = layout.get_slot_offset(point_type, point_id) {
                let slot = self.slot_at(slot_offset);
                // Use Release ordering to ensure visibility to readers
                slot.set(value, value, timestamp);

                // Update last_update_ts (relaxed is fine here, it's advisory)
                self.header()
                    .last_update_ts
                    .store(timestamp, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    /// Write measurement value (convenience method)
    #[inline]
    pub fn set_measurement(
        &self,
        instance_id: u32,
        point_id: u32,
        value: f64,
        timestamp: u64,
    ) -> bool {
        self.set(instance_id, 0, point_id, value, timestamp)
    }

    /// Write action value (convenience method)
    #[inline]
    pub fn set_action(&self, instance_id: u32, point_id: u32, value: f64, timestamp: u64) -> bool {
        self.set(instance_id, 1, point_id, value, timestamp)
    }

    /// Update heartbeat timestamp (call periodically to indicate liveness)
    pub fn heartbeat(&self) {
        self.header()
            .writer_heartbeat
            .store(timestamp_ms(), Ordering::Release);
    }

    /// Flush changes to file
    pub fn flush(&self) -> Result<()> {
        self.mmap.flush().context("Failed to flush mmap")
    }

    /// Get statistics
    pub fn stats(&self) -> SharedWriterStats {
        let header = self.header();
        SharedWriterStats {
            instance_count: header.instance_count.load(Ordering::Relaxed),
            total_points: header.total_points.load(Ordering::Relaxed),
            last_update_ts: header.last_update_ts.load(Ordering::Relaxed),
            file_size: self.mmap.len(),
        }
    }

    // ======================== Round 123: Direct Access API ========================

    /// Get data section offset (for ChannelToSlotIndex)
    ///
    /// Returns the byte offset where point data begins in the shared memory file.
    #[inline]
    pub fn data_offset(&self) -> usize {
        self.header().data_offset as usize
    }

    /// Get slot offset for a specific point (for building ChannelToSlotIndex)
    ///
    /// # Arguments
    /// * `instance_id` - Instance identifier
    /// * `point_type` - 0 for measurement, 1 for action
    /// * `point_id` - Point identifier
    ///
    /// # Returns
    /// Byte offset of the slot, or None if not found
    pub fn get_slot_offset(
        &self,
        instance_id: u32,
        point_type: u8,
        point_id: u32,
    ) -> Option<usize> {
        let layout = self.instance_layouts.get(&instance_id)?;

        let (base, mapping, max_point_id) = if point_type == 0 {
            (
                layout.measurement_base,
                &layout.measurement_point_to_offset,
                layout.max_measurement_point_id,
            )
        } else {
            (
                layout.action_base,
                &layout.action_point_to_offset,
                layout.max_action_point_id,
            )
        };

        if point_id > max_point_id {
            return None;
        }

        let relative_offset = mapping.get(point_id as usize).copied()?;
        if relative_offset == u32::MAX {
            return None;
        }

        Some(base + (relative_offset as usize) * std::mem::size_of::<PointSlot>())
    }

    /// Direct write to a slot by offset (bypass instance/point lookup)
    ///
    /// This is the fastest write path, used when slot offset is pre-computed.
    /// Typically used with ChannelToSlotIndex for O(1) channel writes.
    ///
    /// # Arguments
    /// * `slot_offset` - Byte offset of the slot (from get_slot_offset or ChannelToSlotIndex)
    /// * `value` - Engineering value
    /// * `timestamp` - Timestamp in milliseconds
    #[inline]
    pub fn set_direct(&self, slot_offset: usize, value: f64, timestamp: u64) {
        let slot = self.slot_at(slot_offset);
        slot.set(value, value, timestamp);

        // Update last_update_ts
        self.header()
            .last_update_ts
            .store(timestamp, Ordering::Release);
    }
}

/// Statistics for SharedVecRtdbWriter
#[derive(Debug, Clone)]
pub struct SharedWriterStats {
    pub instance_count: u32,
    pub total_points: u32,
    pub last_update_ts: u64,
    pub file_size: usize,
}

// ========== SharedVecRtdbReader (Round 117) ==========

/// Shared memory reader for modsrv
///
/// Opens the shared memory file created by comsrv and provides
/// zero-copy read access to point data.
///
/// # Performance (Round 120)
/// Uses Vec direct indexing for O(1) point lookup (~1-5ns vs ~20ns for FxHashMap).
pub struct SharedVecRtdbReader {
    /// Memory-mapped file (read-only)
    mmap: Mmap,
    /// Instance ID → InstanceLayout (Vec direct indexing) - Round 120 optimization
    instance_layouts: FxHashMap<u32, InstanceLayout>,
    /// Data offset from header
    data_offset: usize,
}

impl SharedVecRtdbReader {
    /// Open existing shared memory file for reading
    ///
    /// # Arguments
    /// * `config` - Shared memory configuration
    ///
    /// # Returns
    /// * `Ok(Self)` - Reader instance
    /// * `Err` - If file doesn't exist, is invalid, or mapping fails
    pub fn open(config: &SharedConfig) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .open(&config.path)
            .with_context(|| format!("Failed to open shared memory file: {:?}", config.path))?;

        // Memory map with read-only access
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .context("Failed to memory map file")?
        };

        let mut reader = Self {
            mmap,
            instance_layouts: FxHashMap::default(),
            data_offset: 0,
        };

        // Validate and build index
        reader.validate_and_build_index()?;

        info!(
            "SharedVecRtdbReader: opened {:?}, {} instances indexed",
            config.path,
            reader.instance_layouts.len()
        );

        Ok(reader)
    }

    /// Validate header and build point index
    ///
    /// # Note
    /// The shared memory format only stores point counts, not actual point_ids.
    /// Reader assumes sequential point_ids (0, 1, 2...) matching writer's slot order.
    /// For arbitrary point_id support, store point_ids in the shared memory format.
    fn validate_and_build_index(&mut self) -> Result<()> {
        // Read header values into locals to avoid borrow issues
        let (magic, version, data_offset, index_offset, instance_count) = {
            let header = self.header();
            (
                header.magic,
                header.version,
                header.data_offset as usize,
                header.index_offset as usize,
                header.instance_count.load(Ordering::Acquire) as usize,
            )
        };

        if magic != SHARED_MAGIC {
            anyhow::bail!(
                "Invalid magic: expected {:016x}, got {:016x}",
                SHARED_MAGIC,
                magic
            );
        }

        if version != SHARED_VERSION {
            anyhow::bail!(
                "Version mismatch: expected {}, got {}",
                SHARED_VERSION,
                version
            );
        }

        self.data_offset = data_offset;

        // Build InstanceLayout from instance indices (Round 120)
        for i in 0..instance_count {
            // Copy values from instance index to avoid borrow issues
            let (instance_id, measurement_count, action_count, measurement_offset, action_offset) = {
                let inst_idx = self.instance_index_at(index_offset, i);
                (
                    inst_idx.instance_id,
                    inst_idx.measurement_count as usize,
                    inst_idx.action_count as usize,
                    inst_idx.measurement_offset as usize,
                    inst_idx.action_offset as usize,
                )
            };

            // Build sequential point_ids (0, 1, 2, ... count-1)
            // This matches the slot assignment order in writer
            let measurement_points: Vec<u32> = (0..measurement_count as u32).collect();
            let action_points: Vec<u32> = (0..action_count as u32).collect();

            // Create InstanceLayout with Vec direct indexing
            let layout = InstanceLayout::new(
                measurement_offset,
                action_offset,
                &measurement_points,
                &action_points,
            );
            self.instance_layouts.insert(instance_id, layout);
        }

        Ok(())
    }

    /// Get reference to header
    fn header(&self) -> &SharedHeader {
        unsafe { &*(self.mmap.as_ptr() as *const SharedHeader) }
    }

    /// Get reference to instance index at position
    fn instance_index_at(&self, index_offset: usize, idx: usize) -> &InstanceIndex {
        let offset = index_offset + idx * std::mem::size_of::<InstanceIndex>();
        unsafe { &*(self.mmap.as_ptr().add(offset) as *const InstanceIndex) }
    }

    /// Get reference to point slot at given offset
    fn slot_at(&self, slot_offset: usize) -> &PointSlot {
        let offset = self.data_offset + slot_offset;
        unsafe { &*(self.mmap.as_ptr().add(offset) as *const PointSlot) }
    }

    /// Read a point value
    ///
    /// # Arguments
    /// * `instance_id` - Instance identifier
    /// * `point_type` - 0 for measurement, 1 for action
    /// * `point_id` - Point identifier
    ///
    /// # Returns
    /// * `Some(f64)` - Point value
    /// * `None` - Point not found
    ///
    /// # Performance (Round 120)
    /// Uses Vec direct indexing for O(1) lookup (~1-5ns vs ~20ns for FxHashMap).
    #[inline]
    pub fn get(&self, instance_id: u32, point_type: u8, point_id: u32) -> Option<f64> {
        // Two-level lookup: instance_id → layout → point_id (Vec direct index)
        self.instance_layouts.get(&instance_id).and_then(|layout| {
            layout
                .get_slot_offset(point_type, point_id)
                .map(|slot_offset| {
                    let slot = self.slot_at(slot_offset);
                    // Use Acquire ordering to see Release writes from writer
                    slot.load_value(Ordering::Acquire)
                })
        })
    }

    /// Read measurement value (convenience method)
    #[inline]
    pub fn get_measurement(&self, instance_id: u32, point_id: u32) -> Option<f64> {
        self.get(instance_id, 0, point_id)
    }

    /// Read action value (convenience method)
    #[inline]
    pub fn get_action(&self, instance_id: u32, point_id: u32) -> Option<f64> {
        self.get(instance_id, 1, point_id)
    }

    /// Check if writer is alive based on heartbeat
    ///
    /// # Arguments
    /// * `timeout_ms` - Consider writer dead if heartbeat older than this
    ///
    /// # Returns
    /// * `true` - Writer is alive
    /// * `false` - Writer may be dead or not started
    pub fn is_writer_alive(&self, timeout_ms: u64) -> bool {
        let header = self.header();
        let heartbeat = header.writer_heartbeat.load(Ordering::Acquire);
        let now = timestamp_ms();
        if heartbeat == 0 {
            return false; // Never written
        }
        now.saturating_sub(heartbeat) < timeout_ms
    }

    /// Get last update timestamp
    pub fn last_update_ts(&self) -> u64 {
        self.header().last_update_ts.load(Ordering::Acquire)
    }

    /// Rebuild index from current file state
    ///
    /// Call this if the writer has registered new instances after
    /// the reader was opened.
    pub fn rebuild_index(&mut self) -> Result<()> {
        self.instance_layouts.clear();
        self.validate_and_build_index()
    }

    /// Get statistics
    pub fn stats(&self) -> SharedReaderStats {
        let header = self.header();
        SharedReaderStats {
            instance_count: header.instance_count.load(Ordering::Acquire),
            total_points: header.total_points.load(Ordering::Acquire),
            indexed_instances: self.instance_layouts.len(),
            last_update_ts: header.last_update_ts.load(Ordering::Acquire),
            writer_heartbeat: header.writer_heartbeat.load(Ordering::Acquire),
        }
    }
}

/// Statistics for SharedVecRtdbReader
#[derive(Debug, Clone)]
pub struct SharedReaderStats {
    pub instance_count: u32,
    pub total_points: u32,
    /// Number of indexed instances (Round 120: was indexed_points)
    pub indexed_instances: usize,
    pub last_update_ts: u64,
    pub writer_heartbeat: u64,
}

// ========== Round 123: ChannelToSlotIndex ==========

use crate::routing_cache::RoutingCache;
use voltage_model::PointType;

/// Direct mapping from channel points to shared memory slots
///
/// Round 123: Pre-computed at startup to eliminate runtime C2M routing lookup.
/// Provides O(1) channel-to-slot mapping for the hottest path.
///
/// # Architecture
/// ```text
/// Before (2 lookups):
///   Channel → C2M Route → Instance → SharedMemory Lookup → Slot
///
/// After (1 lookup):
///   Channel → ChannelToSlotIndex → Slot
/// ```
///
/// # Performance
/// - Before: ~90ns (two hash lookups)
/// - After: ~50ns (single hash lookup)
#[derive(Debug)]
pub struct ChannelToSlotIndex {
    /// (channel_id, point_type, point_id) → slot byte offset
    index: FxHashMap<(u32, PointType, u32), usize>,
    /// Cached data_offset from SharedHeader
    data_offset: usize,
    /// Statistics
    mapped_count: usize,
}

impl ChannelToSlotIndex {
    /// Build direct mapping from routing cache and shared memory writer
    ///
    /// This should be called at startup after:
    /// 1. RoutingCache is loaded with C2M routes
    /// 2. SharedVecRtdbWriter has registered all instances
    ///
    /// # Arguments
    /// * `routing_cache` - Loaded routing cache with C2M routes
    /// * `writer` - SharedVecRtdbWriter with registered instances
    ///
    /// # Returns
    /// ChannelToSlotIndex with pre-computed mappings
    pub fn build(routing_cache: &RoutingCache, writer: &SharedVecRtdbWriter) -> Self {
        let mut index = FxHashMap::default();
        let data_offset = writer.data_offset();

        // Iterate over all C2M routes
        for ((channel_id, point_type, channel_point_id), target) in routing_cache.c2m_iter() {
            // Look up the slot offset in shared memory
            // C2M target has: instance_id, point_id (measurement point)
            // Measurement type = 0
            if let Some(slot_offset) =
                writer.get_slot_offset(target.instance_id, 0, target.point_id)
            {
                index.insert((channel_id, point_type, channel_point_id), slot_offset);
            }
        }

        let mapped_count = index.len();
        tracing::info!(
            "ChannelToSlotIndex built: {} direct mappings, data_offset={}",
            mapped_count,
            data_offset
        );

        Self {
            index,
            data_offset,
            mapped_count,
        }
    }

    /// Look up slot offset for a channel point
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `point_type` - Point type (Telemetry, Signal, Control, Adjustment)
    /// * `point_id` - Point identifier within the channel
    ///
    /// # Returns
    /// Byte offset of the slot in shared memory, or None if not mapped
    #[inline]
    pub fn lookup(&self, channel_id: u32, point_type: PointType, point_id: u32) -> Option<usize> {
        self.index.get(&(channel_id, point_type, point_id)).copied()
    }

    /// Get number of mapped channel points
    pub fn len(&self) -> usize {
        self.mapped_count
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.mapped_count == 0
    }

    /// Get data offset
    pub fn data_offset(&self) -> usize {
        self.data_offset
    }
}

// ========== Utility Functions ==========

/// Check if shared memory path is available
pub fn is_shm_available(config: &SharedConfig) -> bool {
    config.path.parent().map(|p| p.exists()).unwrap_or(false)
}

/// Try to open reader, returning None if unavailable
pub fn try_open_reader(config: &SharedConfig) -> Option<SharedVecRtdbReader> {
    match SharedVecRtdbReader::open(config) {
        Ok(reader) => Some(reader),
        Err(e) => {
            warn!("Failed to open shared memory reader: {}", e);
            None
        },
    }
}

// ========== Tests ==========

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests can use unwrap for clarity
mod tests {
    use super::*;
    use std::thread;

    fn test_config(name: &str) -> SharedConfig {
        SharedConfig {
            path: PathBuf::from(format!("/tmp/voltage-rtdb-test-{}.shm", name)),
            max_instances: 8,
            max_points_per_instance: 32,
        }
    }

    #[test]
    fn test_shared_header_size() {
        assert_eq!(std::mem::size_of::<SharedHeader>(), 64);
    }

    #[test]
    fn test_instance_index_size() {
        assert_eq!(std::mem::size_of::<InstanceIndex>(), 48);
    }

    #[test]
    fn test_point_slot_size() {
        assert_eq!(std::mem::size_of::<PointSlot>(), 32);
    }

    #[test]
    fn test_writer_create_and_register() {
        let config = test_config("writer_create");
        // Cleanup before test
        std::fs::remove_file(&config.path).ok();

        let mut writer = SharedVecRtdbWriter::open(&config).unwrap();

        // Register instance
        writer.register_instance(5, &[1, 2, 3], &[10, 11]).unwrap();

        // Check stats
        let stats = writer.stats();
        assert_eq!(stats.instance_count, 1);
        assert_eq!(stats.total_points, 5);

        // Write values
        assert!(writer.set_measurement(5, 1, 100.5, 1729000000));
        assert!(writer.set_measurement(5, 2, 200.0, 1729000001));
        assert!(writer.set_action(5, 10, 1.0, 1729000002));

        // Non-existent point
        assert!(!writer.set_measurement(5, 999, 0.0, 0));

        // Cleanup
        std::fs::remove_file(&config.path).ok();
    }

    #[test]
    fn test_writer_reader_roundtrip() {
        let config = test_config("roundtrip");
        // Cleanup before test
        std::fs::remove_file(&config.path).ok();

        // Writer creates and writes
        {
            let mut writer = SharedVecRtdbWriter::open(&config).unwrap();
            // Register with sequential point_ids: measurements [0, 1, 2], actions [0]
            // Reader uses sequential indices, so we must use sequential point_ids
            writer.register_instance(5, &[0, 1, 2], &[0]).unwrap();
            // Write using registered point_ids
            writer.set_measurement(5, 0, 100.5, 1729000000);
            writer.set_measurement(5, 1, 200.0, 1729000001);
            writer.set_action(5, 0, 1.0, 1729000002);
            writer.heartbeat();
            // Flush may fail on some systems (e.g., MacOS) - ignore for tests
            let _ = writer.flush();
        }

        // Reader opens and reads
        {
            let reader = SharedVecRtdbReader::open(&config).unwrap();

            // Check values (using sequential point indices)
            assert_eq!(reader.get_measurement(5, 0), Some(100.5));
            assert_eq!(reader.get_measurement(5, 1), Some(200.0));
            assert_eq!(reader.get_action(5, 0), Some(1.0));
            assert!(reader.get_measurement(5, 999).is_none());

            // Check stats
            let stats = reader.stats();
            assert_eq!(stats.instance_count, 1);
            assert_eq!(stats.total_points, 4);
            assert!(reader.is_writer_alive(5000));
        }

        // Cleanup
        std::fs::remove_file(&config.path).ok();
    }

    #[test]
    fn test_concurrent_write_read() {
        let config = test_config("concurrent");
        // Cleanup before test
        std::fs::remove_file(&config.path).ok();

        // Create shared memory
        let mut writer = SharedVecRtdbWriter::open(&config).unwrap();
        // Register with sequential point_id 0 (reader uses sequential indices)
        writer.register_instance(1, &[0], &[]).unwrap();
        // Flush may fail on some systems - ignore error for tests
        let _ = writer.flush();

        // Arc for sharing config path
        let path = config.path.clone();

        // Spawn reader thread
        let reader_handle = thread::spawn(move || {
            let reader_config = SharedConfig {
                path,
                ..Default::default()
            };
            let reader = SharedVecRtdbReader::open(&reader_config).unwrap();

            // Read 100 times using sequential point index 0
            let mut last_value = 0.0f64;
            for _ in 0..100 {
                if let Some(v) = reader.get_measurement(1, 0) {
                    assert!(v >= last_value, "Value should be monotonically increasing");
                    last_value = v;
                }
                thread::sleep(std::time::Duration::from_micros(100));
            }
            last_value
        });

        // Write 100 values using point_id 0
        for i in 0..100 {
            writer.set_measurement(1, 0, i as f64, timestamp_ms());
            thread::sleep(std::time::Duration::from_micros(50));
        }

        let final_value = reader_handle.join().unwrap();
        assert!(final_value > 0.0, "Reader should have seen some writes");

        // Cleanup
        std::fs::remove_file(&config.path).ok();
    }

    #[test]
    fn test_channel_to_slot_index() {
        use crate::RoutingCache;
        use std::collections::HashMap;

        // Create temp file
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("voltage_c2s_test_{}.shm", std::process::id()));

        let config = SharedConfig {
            path: path.clone(),
            max_instances: 8,
            max_points_per_instance: 32,
        };

        // Create writer and register instance
        let mut writer = SharedVecRtdbWriter::open(&config).unwrap();
        // Instance 5 has measurement points 10, 20, 30
        writer.register_instance(5, &[10, 20, 30], &[]).unwrap();

        // Create routing cache with C2M routes
        // channel 1001, Telemetry, point 1 → instance 5, measurement point 10
        // channel 1001, Telemetry, point 2 → instance 5, measurement point 20
        let mut c2m_data = HashMap::new();
        c2m_data.insert("1001:T:1".to_string(), "5:M:10".to_string());
        c2m_data.insert("1001:T:2".to_string(), "5:M:20".to_string());
        c2m_data.insert("1002:S:1".to_string(), "5:M:30".to_string());
        let routing_cache = RoutingCache::from_maps(c2m_data, HashMap::new(), HashMap::new());

        // Build ChannelToSlotIndex
        let channel_index = ChannelToSlotIndex::build(&routing_cache, &writer);

        // Verify mapping count
        assert_eq!(channel_index.len(), 3);
        assert!(!channel_index.is_empty());

        // Verify lookups work
        use voltage_model::PointType;
        assert!(channel_index
            .lookup(1001, PointType::Telemetry, 1)
            .is_some());
        assert!(channel_index
            .lookup(1001, PointType::Telemetry, 2)
            .is_some());
        assert!(channel_index.lookup(1002, PointType::Signal, 1).is_some());

        // Non-existent lookups return None
        assert!(channel_index
            .lookup(9999, PointType::Telemetry, 1)
            .is_none());
        assert!(channel_index
            .lookup(1001, PointType::Telemetry, 999)
            .is_none());

        // Test direct write using the index
        let slot_offset = channel_index.lookup(1001, PointType::Telemetry, 1).unwrap();
        writer.set_direct(slot_offset, 42.5, 1729000000);

        // Verify the write by reading the slot directly
        // Note: SharedVecRtdbReader assumes sequential point_ids, but ChannelToSlotIndex
        // bypasses this by using pre-computed slot offsets.
        // We verify by reading from the writer's slot directly using slot_at method.
        let slot = writer.slot_at(slot_offset);
        let value = slot.get_value();
        assert!(
            (value - 42.5).abs() < f64::EPSILON,
            "Expected 42.5, got {}",
            value
        );

        // Verify timestamp was also set
        let ts = slot.get_timestamp();
        assert_eq!(ts, 1729000000, "Expected 1729000000, got {}", ts);

        // Cleanup
        std::fs::remove_file(&path).ok();
    }
}
