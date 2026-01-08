//! Shared Memory RTDB Implementation
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
//! # Memory Layout (v2)
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │ SharedHeader (64 bytes)                      │
//! ├──────────────────────────────────────────────┤
//! │ InstanceIndex[] (48 bytes each)              │
//! ├──────────────────────────────────────────────┤
//! │ Instance PointSlot[] (32 bytes each)         │
//! ├──────────────────────────────────────────────┤
//! │ ChannelIndex[] (48 bytes each) [v2]          │
//! ├──────────────────────────────────────────────┤
//! │ Channel PointSlot[] (32 bytes each) [v2]     │
//! └──────────────────────────────────────────────┘
//! ```

use crate::vec_impl::PointSlot;
use anyhow::{Context, Result};
use memmap2::{Mmap, MmapMut, MmapOptions};
use rustc_hash::FxHashMap;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use tracing::{debug, info, warn};

// ========== Constants ==========

/// Magic number for validation: "VOLTAGE_" in ASCII
pub const SHARED_MAGIC: u64 = 0x564F4C544147455F;

/// Default shared memory file path (Docker tmpfs mount point)
/// This constant is kept for backward compatibility.
/// Use `default_shm_path()` for intelligent path selection.
pub const DEFAULT_SHM_PATH: &str = "/shm/rtdb/voltage-rtdb.shm";

/// Get the default shared memory path with intelligent fallback
///
/// Priority:
/// 1. `VOLTAGE_SHM_PATH` environment variable (if set)
/// 2. Docker tmpfs mount point `/shm/rtdb/voltage-rtdb.shm` (if exists)
/// 3. Linux RAM-backed tmpfs `/dev/shm/voltage-rtdb.shm`
/// 4. Fallback to `/tmp/voltage-rtdb.shm` (macOS or other platforms)
///
/// The path is automatically created if the parent directory exists.
pub fn default_shm_path() -> PathBuf {
    // Priority 1: Environment variable override
    if let Ok(path) = std::env::var("VOLTAGE_SHM_PATH") {
        return PathBuf::from(path);
    }

    // Priority 2: Docker tmpfs mount point
    let docker_path = Path::new("/shm/rtdb");
    if docker_path.exists() {
        return PathBuf::from(DEFAULT_SHM_PATH);
    }

    // Priority 3: Linux /dev/shm (RAM-backed tmpfs)
    #[cfg(target_os = "linux")]
    {
        let dev_shm = Path::new("/dev/shm");
        if dev_shm.exists() {
            return dev_shm.join("voltage-rtdb.shm");
        }
    }

    // Priority 4: Fallback to /tmp (macOS or other platforms)
    PathBuf::from("/tmp/voltage-rtdb.shm")
}

// ========== SharedHeader ==========

/// Shared memory header (64 bytes, cache-line aligned)
///
/// Located at offset 0 of the shared memory file.
/// Contains metadata and synchronization fields.
#[repr(C, align(64))]
pub struct SharedHeader {
    /// Magic number for validation (0x564F4C544147455F = "VOLTAGE_")
    pub magic: u64,
    /// Number of registered instances
    pub instance_count: AtomicU32,
    /// Total number of points across all instances
    pub total_points: AtomicU32,
    /// Number of registered channels
    pub channel_count: AtomicU32,
    /// Writer initialization complete flag (0 = initializing, 1 = ready)
    pub initialized: AtomicU32,
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
        self.magic == SHARED_MAGIC
    }

    /// Check if writer has completed initialization
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire) == 1
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

// ========== ChannelIndex ==========

/// Channel index entry (48 bytes)
///
/// Maps channel_id to its point data locations in the shared memory.
/// Each channel has 4 point types: Telemetry, Signal, Control, Adjustment.
#[derive(Default)]
#[repr(C)]
pub struct ChannelIndex {
    /// Channel identifier
    pub channel_id: u32,
    /// Number of points for each type: [T, S, C, A]
    pub point_counts: [u16; 4],
    /// Offset to slots for each type (relative to channel_data_offset): [T, S, C, A]
    pub point_offsets: [u32; 4],
    /// Total points across all types
    pub total_points: u32,
    /// Reserved for future use
    pub _reserved: [u32; 3],
}

impl ChannelIndex {
    /// Point type indices
    pub const TELEMETRY: usize = 0;
    pub const SIGNAL: usize = 1;
    pub const CONTROL: usize = 2;
    pub const ADJUSTMENT: usize = 3;

    /// Convert PointType to index
    #[inline]
    pub fn point_type_to_index(point_type: voltage_model::PointType) -> usize {
        match point_type {
            voltage_model::PointType::Telemetry => Self::TELEMETRY,
            voltage_model::PointType::Signal => Self::SIGNAL,
            voltage_model::PointType::Control => Self::CONTROL,
            voltage_model::PointType::Adjustment => Self::ADJUSTMENT,
        }
    }
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
    /// Maximum number of channels
    pub max_channels: usize,
    /// Maximum points per channel (all types combined)
    pub max_points_per_channel: usize,
}

impl Default for SharedConfig {
    fn default() -> Self {
        // Use 65536 as default ("practically unlimited")
        // mmap uses virtual address space; only accessed pages load into physical RAM
        // Large values don't waste memory but eliminate capacity limit concerns
        Self {
            path: default_shm_path(),
            max_instances: 65536,
            max_points_per_instance: 65536,
            max_channels: 65536,
            max_points_per_channel: 65536,
        }
    }
}

impl SharedConfig {
    /// Create config with custom path
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = path.into();
        self
    }

    /// Create config with custom max_instances
    pub fn with_max_instances(mut self, max_instances: usize) -> Self {
        self.max_instances = max_instances;
        self
    }

    /// Create config with custom max_points_per_instance
    pub fn with_max_points_per_instance(mut self, max_points: usize) -> Self {
        self.max_points_per_instance = max_points;
        self
    }

    /// Create config with custom max_channels
    pub fn with_max_channels(mut self, max_channels: usize) -> Self {
        self.max_channels = max_channels;
        self
    }

    /// Create config with custom max_points_per_channel
    pub fn with_max_points_per_channel(mut self, max_points: usize) -> Self {
        self.max_points_per_channel = max_points;
        self
    }

    /// Calculate total file size needed (includes Channel area in v2)
    pub fn calculate_file_size(&self) -> usize {
        let header_size = std::mem::size_of::<SharedHeader>();
        // Instance area
        let inst_index_size = self.max_instances * std::mem::size_of::<InstanceIndex>();
        let inst_data_size =
            self.max_instances * self.max_points_per_instance * std::mem::size_of::<PointSlot>();
        // Channel area
        let ch_index_size = self.max_channels * std::mem::size_of::<ChannelIndex>();
        let ch_data_size =
            self.max_channels * self.max_points_per_channel * std::mem::size_of::<PointSlot>();
        header_size + inst_index_size + inst_data_size + ch_index_size + ch_data_size
    }

    /// Calculate channel index offset
    pub fn channel_index_offset(&self) -> usize {
        let header_size = std::mem::size_of::<SharedHeader>();
        let inst_index_size = self.max_instances * std::mem::size_of::<InstanceIndex>();
        let inst_data_size =
            self.max_instances * self.max_points_per_instance * std::mem::size_of::<PointSlot>();
        header_size + inst_index_size + inst_data_size
    }

    /// Calculate channel data offset
    pub fn channel_data_offset(&self) -> usize {
        self.channel_index_offset() + self.max_channels * std::mem::size_of::<ChannelIndex>()
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

// ========== InstanceLayout ==========

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

// ========== ChannelLayout ==========

/// Per-channel layout for O(1) point lookup
#[derive(Default)]
struct ChannelLayout {
    /// Base offsets for each point type [T, S, C, A] (relative to channel_data_offset)
    type_bases: [usize; 4],
    /// point_id → slot_offset mapping for each type
    type_mappings: [Box<[u32]>; 4],
    /// Max point_id for each type
    max_point_ids: [u32; 4],
}

impl ChannelLayout {
    /// Get slot offset for a point in this channel
    ///
    /// Note: type_mappings stores slot indices, not byte offsets.
    /// This method returns the final byte offset.
    #[inline]
    fn get_slot_offset(&self, type_idx: usize, point_id: u32) -> Option<usize> {
        if type_idx >= 4 || point_id > self.max_point_ids[type_idx] {
            return None;
        }
        // Get slot index from mapping
        let slot_idx = *self.type_mappings[type_idx].get(point_id as usize)?;
        if slot_idx == u32::MAX {
            return None;
        }
        // Calculate byte offset: base + (slot_index * slot_size)
        Some(self.type_bases[type_idx] + (slot_idx as usize) * std::mem::size_of::<PointSlot>())
    }
}

// Default is derived automatically since all fields implement Default

// ========== SharedVecRtdbWriter (optimized Channel) ==========

/// Shared memory writer for comsrv
///
/// Creates and manages the shared memory file, writing point data
/// that can be read by modsrv's SharedVecRtdbReader.
///
/// # Performance
/// Uses Vec direct indexing for O(1) point lookup (~1-5ns vs ~20ns for FxHashMap).
///
/// #
/// Added Channel storage area for direct channel data access.
pub struct SharedVecRtdbWriter {
    /// Memory-mapped file (read-write)
    mmap: MmapMut,
    /// Configuration
    config: SharedConfig,
    /// Instance ID → index in InstanceIndex array
    instance_indices: FxHashMap<u32, usize>,
    /// Instance ID → InstanceLayout (Vec direct indexing) optimization
    instance_layouts: FxHashMap<u32, InstanceLayout>,
    /// Next available instance index
    next_instance_idx: usize,
    /// Next available slot offset (bytes, for instance data)
    next_slot_offset: usize,
    /// Channel ID → index in ChannelIndex array
    channel_indices: FxHashMap<u32, usize>,
    /// Channel ID → ChannelLayout
    channel_layouts: FxHashMap<u32, ChannelLayout>,
    /// Next available channel index
    next_channel_idx: usize,
    /// Next available channel slot offset (bytes, relative to channel_data_offset)
    next_channel_slot_offset: usize,
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
            // Channel storage area
            channel_indices: FxHashMap::default(),
            channel_layouts: FxHashMap::default(),
            next_channel_idx: 0,
            next_channel_slot_offset: 0,
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
            header.instance_count.store(0, Ordering::Release);
            header.total_points.store(0, Ordering::Release);
            header.channel_count.store(0, Ordering::Release);
            header.initialized.store(0, Ordering::Release); // Not ready yet
            header.index_offset = index_offset;
            header.data_offset = data_offset;
            header.last_update_ts.store(0, Ordering::Release);
            header
                .writer_heartbeat
                .store(timestamp_ms(), Ordering::Release);
        }

        // Flush to ensure visibility
        self.mmap.flush().context("Failed to flush header")?;

        // Mark as initialized after header is fully set up
        self.header().initialized.store(1, Ordering::Release);

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

        // Build InstanceLayout with Vec direct indexing
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

        // Memory fence ensures all InstanceIndex writes are visible before count update
        // This prevents readers from seeing new count but stale/uninitialized index data
        std::sync::atomic::fence(Ordering::Release);

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
    /// # Performance
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

    // ======================== Direct Access API ========================

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

    // ======================== Channel Storage API ========================

    /// Get mutable reference to channel index at given position
    fn channel_index_mut(&mut self, idx: usize) -> &mut ChannelIndex {
        let offset = self.config.channel_index_offset() + idx * std::mem::size_of::<ChannelIndex>();
        unsafe { &mut *(self.mmap.as_mut_ptr().add(offset) as *mut ChannelIndex) }
    }

    /// Get reference to channel slot at given offset (bytes from channel_data_offset)
    fn channel_slot_at(&self, slot_offset: usize) -> &PointSlot {
        let offset = self.config.channel_data_offset() + slot_offset;
        unsafe { &*(self.mmap.as_ptr().add(offset) as *const PointSlot) }
    }

    /// Register a channel with its T/S/C/A points
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `telemetry_points` - Point IDs for Telemetry
    /// * `signal_points` - Point IDs for Signal
    /// * `control_points` - Point IDs for Control
    /// * `adjustment_points` - Point IDs for Adjustment
    ///
    /// # Returns
    /// * `Ok(())` - Registration successful
    /// * `Err` - If max channels exceeded
    pub fn register_channel(
        &mut self,
        channel_id: u32,
        telemetry_points: &[u32],
        signal_points: &[u32],
        control_points: &[u32],
        adjustment_points: &[u32],
    ) -> Result<()> {
        if self.next_channel_idx >= self.config.max_channels {
            anyhow::bail!("Max channels ({}) exceeded", self.config.max_channels);
        }

        let total_points = telemetry_points.len()
            + signal_points.len()
            + control_points.len()
            + adjustment_points.len();

        if total_points > self.config.max_points_per_channel {
            anyhow::bail!(
                "Channel {} has {} points, max is {}",
                channel_id,
                total_points,
                self.config.max_points_per_channel
            );
        }

        // Build ChannelLayout for O(1) point lookup
        let point_lists: [&[u32]; 4] = [
            telemetry_points,
            signal_points,
            control_points,
            adjustment_points,
        ];

        let mut type_bases = [0usize; 4];
        let mut type_mappings: [Box<[u32]>; 4] = Default::default();
        let mut max_point_ids = [0u32; 4];
        let mut running_offset = 0usize;

        for (i, points) in point_lists.iter().enumerate() {
            type_bases[i] =
                self.next_channel_slot_offset + running_offset * std::mem::size_of::<PointSlot>();

            if !points.is_empty() {
                // SAFETY: points is non-empty, so max() always returns Some
                let max_id = *points.iter().max().expect("non-empty slice has max");
                max_point_ids[i] = max_id;

                // Create mapping vec: point_id → relative slot offset
                let mut mapping = vec![u32::MAX; (max_id + 1) as usize];
                for (slot_idx, &pid) in points.iter().enumerate() {
                    mapping[pid as usize] = slot_idx as u32;
                }
                type_mappings[i] = mapping.into_boxed_slice();
            } else {
                type_mappings[i] = Box::new([]);
            }

            running_offset += points.len();
        }

        let layout = ChannelLayout {
            type_bases,
            type_mappings,
            max_point_ids,
        };

        self.channel_layouts.insert(channel_id, layout);
        self.channel_indices
            .insert(channel_id, self.next_channel_idx);

        // Pre-calculate values before mutable borrow
        let slot_size = std::mem::size_of::<PointSlot>();
        let base_offset = self.next_channel_slot_offset;
        let t_count = telemetry_points.len();
        let s_count = signal_points.len();
        let c_count = control_points.len();

        let point_counts = [
            t_count as u16,
            s_count as u16,
            c_count as u16,
            adjustment_points.len() as u16,
        ];
        let point_offsets = [
            base_offset as u32,
            (base_offset + t_count * slot_size) as u32,
            (base_offset + (t_count + s_count) * slot_size) as u32,
            (base_offset + (t_count + s_count + c_count) * slot_size) as u32,
        ];
        let ch_idx_slot = self.next_channel_idx;

        // Write to ChannelIndex in shared memory
        let ch_idx = self.channel_index_mut(ch_idx_slot);
        ch_idx.channel_id = channel_id;
        ch_idx.point_counts = point_counts;
        ch_idx.point_offsets = point_offsets;
        ch_idx.total_points = total_points as u32;

        // Update counters
        self.next_channel_slot_offset += total_points * slot_size;
        self.next_channel_idx += 1;

        // Memory fence ensures all ChannelIndex writes are visible before count update
        std::sync::atomic::fence(Ordering::Release);

        // Update header
        self.header_mut()
            .channel_count
            .fetch_add(1, Ordering::Release);

        debug!(
            "Registered channel {} with T:{} S:{} C:{} A:{} points",
            channel_id,
            telemetry_points.len(),
            signal_points.len(),
            control_points.len(),
            adjustment_points.len()
        );

        Ok(())
    }

    /// Write a channel point value
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `point_type` - Point type (Telemetry/Signal/Control/Adjustment)
    /// * `point_id` - Point identifier
    /// * `value` - Engineering value
    /// * `timestamp` - Timestamp in milliseconds
    ///
    /// # Returns
    /// * `true` - Point found and updated
    /// * `false` - Point not registered
    #[inline]
    pub fn set_channel(
        &self,
        channel_id: u32,
        point_type: voltage_model::PointType,
        point_id: u32,
        value: f64,
        timestamp: u64,
    ) -> bool {
        if let Some(layout) = self.channel_layouts.get(&channel_id) {
            let type_idx = ChannelIndex::point_type_to_index(point_type);

            if point_id <= layout.max_point_ids[type_idx] {
                if let Some(&relative_offset) =
                    layout.type_mappings[type_idx].get(point_id as usize)
                {
                    if relative_offset != u32::MAX {
                        let slot_offset = layout.type_bases[type_idx]
                            + (relative_offset as usize) * std::mem::size_of::<PointSlot>();
                        let slot = self.channel_slot_at(slot_offset);
                        slot.set(value, value, timestamp);

                        self.header()
                            .last_update_ts
                            .store(timestamp, Ordering::Release);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get channel slot offset for a specific point (for external use)
    pub fn get_channel_slot_offset(
        &self,
        channel_id: u32,
        point_type: voltage_model::PointType,
        point_id: u32,
    ) -> Option<usize> {
        let layout = self.channel_layouts.get(&channel_id)?;
        let type_idx = ChannelIndex::point_type_to_index(point_type);

        if point_id > layout.max_point_ids[type_idx] {
            return None;
        }

        let relative_offset = layout.type_mappings[type_idx]
            .get(point_id as usize)
            .copied()?;
        if relative_offset == u32::MAX {
            return None;
        }

        Some(
            self.config.channel_data_offset()
                + layout.type_bases[type_idx]
                + (relative_offset as usize) * std::mem::size_of::<PointSlot>(),
        )
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

// ========== SharedVecRtdbReader ==========

/// Shared memory reader for modsrv
///
/// Opens the shared memory file created by comsrv and provides
/// zero-copy read access to point data.
///
/// # Performance
/// Uses Vec direct indexing for O(1) point lookup (~1-5ns vs ~20ns for FxHashMap).
pub struct SharedVecRtdbReader {
    /// Memory-mapped file (read-only)
    mmap: Mmap,
    /// Instance ID → InstanceLayout (Vec direct indexing) optimization
    instance_layouts: FxHashMap<u32, InstanceLayout>,
    /// Data offset from header
    data_offset: usize,
    /// Config for offset calculations
    config: SharedConfig,
    /// Channel ID → ChannelLayout
    channel_layouts: FxHashMap<u32, ChannelLayout>,
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
            config: config.clone(),
            channel_layouts: FxHashMap::default(),
        };

        // Validate and build index
        reader.validate_and_build_index()?;

        info!(
            "SharedVecRtdbReader: opened {:?}, {} instances, {} channels indexed",
            config.path,
            reader.instance_layouts.len(),
            reader.channel_layouts.len()
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
        let (magic, data_offset, index_offset, instance_count, channel_count) = {
            let header = self.header();
            (
                header.magic,
                header.data_offset as usize,
                header.index_offset as usize,
                header.instance_count.load(Ordering::Acquire) as usize,
                header.channel_count.load(Ordering::Acquire) as usize,
            )
        };

        if magic != SHARED_MAGIC {
            anyhow::bail!(
                "Invalid magic: expected {:016x}, got {:016x}",
                SHARED_MAGIC,
                magic
            );
        }

        // Validate file size to prevent out-of-bounds access on truncated files
        let file_size = self.mmap.len();
        let expected_min_size = std::mem::size_of::<SharedHeader>()
            + instance_count * std::mem::size_of::<InstanceIndex>()
            + channel_count * std::mem::size_of::<ChannelIndex>();

        if file_size < expected_min_size {
            anyhow::bail!(
                "File size {} too small, expected at least {} bytes \
                (header={}, instances={}x{}, channels={}x{})",
                file_size,
                expected_min_size,
                std::mem::size_of::<SharedHeader>(),
                instance_count,
                std::mem::size_of::<InstanceIndex>(),
                channel_count,
                std::mem::size_of::<ChannelIndex>()
            );
        }

        self.data_offset = data_offset;

        // Build InstanceLayout from instance indices
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

        // Build ChannelLayout from channel indices
        let channel_index_offset = self.config.channel_index_offset();
        for i in 0..channel_count {
            let (channel_id, point_counts, point_offsets) = {
                let ch_idx = self.channel_index_at(channel_index_offset, i);
                (ch_idx.channel_id, ch_idx.point_counts, ch_idx.point_offsets)
            };

            // Build ChannelLayout with sequential point_ids
            let mut type_bases = [0usize; 4];
            let mut type_mappings: [Box<[u32]>; 4] = Default::default();
            let mut max_point_ids = [0u32; 4];

            for t in 0..4 {
                type_bases[t] = point_offsets[t] as usize;
                let count = point_counts[t] as usize;
                if count > 0 {
                    max_point_ids[t] = (count - 1) as u32;
                    // Sequential mapping: point_id 0,1,2... → slot 0,1,2...
                    let mapping: Vec<u32> = (0..count as u32).collect();
                    type_mappings[t] = mapping.into_boxed_slice();
                } else {
                    type_mappings[t] = Box::new([]);
                }
            }

            let layout = ChannelLayout {
                type_bases,
                type_mappings,
                max_point_ids,
            };
            self.channel_layouts.insert(channel_id, layout);
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

    /// Get reference to channel index at position
    fn channel_index_at(&self, channel_index_offset: usize, idx: usize) -> &ChannelIndex {
        let offset = channel_index_offset + idx * std::mem::size_of::<ChannelIndex>();
        unsafe { &*(self.mmap.as_ptr().add(offset) as *const ChannelIndex) }
    }

    /// Get reference to channel slot at given offset
    fn channel_slot_at(&self, slot_offset: usize) -> &PointSlot {
        let offset = self.config.channel_data_offset() + slot_offset;
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
    /// # Performance
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

    // ======================== Channel Read API ========================

    /// Read a channel point value
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `point_type` - Point type (Telemetry/Signal/Control/Adjustment)
    /// * `point_id` - Point identifier
    ///
    /// # Returns
    /// * `Some(f64)` - Point value
    /// * `None` - Point not found
    #[inline]
    pub fn get_channel(
        &self,
        channel_id: u32,
        point_type: voltage_model::PointType,
        point_id: u32,
    ) -> Option<f64> {
        let layout = self.channel_layouts.get(&channel_id)?;
        let type_idx = ChannelIndex::point_type_to_index(point_type);
        let slot_offset = layout.get_slot_offset(type_idx, point_id)?;
        let slot = self.channel_slot_at(slot_offset);
        Some(slot.load_value(Ordering::Acquire))
    }

    /// Read channel telemetry value (convenience method)
    #[inline]
    pub fn get_channel_telemetry(&self, channel_id: u32, point_id: u32) -> Option<f64> {
        self.get_channel(channel_id, voltage_model::PointType::Telemetry, point_id)
    }

    /// Read channel signal value (convenience method)
    #[inline]
    pub fn get_channel_signal(&self, channel_id: u32, point_id: u32) -> Option<f64> {
        self.get_channel(channel_id, voltage_model::PointType::Signal, point_id)
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

        // Check if writer has completed initialization
        if !header.is_initialized() {
            return false; // Writer still initializing
        }

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
    /// Call this if the writer has registered new instances/channels after
    /// the reader was opened.
    pub fn rebuild_index(&mut self) -> Result<()> {
        self.instance_layouts.clear();
        self.channel_layouts.clear();
        self.validate_and_build_index()
    }

    /// Get statistics
    pub fn stats(&self) -> SharedReaderStats {
        let header = self.header();
        SharedReaderStats {
            instance_count: header.instance_count.load(Ordering::Acquire),
            total_points: header.total_points.load(Ordering::Acquire),
            indexed_instances: self.instance_layouts.len(),
            channel_count: header.channel_count.load(Ordering::Acquire),
            indexed_channels: self.channel_layouts.len(),
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
    /// Number of indexed instances (was indexed_points)
    pub indexed_instances: usize,
    /// Number of registered channels
    pub channel_count: u32,
    /// Number of indexed channels
    pub indexed_channels: usize,
    pub last_update_ts: u64,
    pub writer_heartbeat: u64,
}

// ========== ChannelToSlotIndex ==========

use crate::routing_cache::RoutingCache;
use voltage_model::PointType;

/// Direct mapping from channel points to shared memory slots
///
/// Pre-computed at startup to eliminate runtime C2M routing lookup.
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
            max_channels: 8,
            max_points_per_channel: 32,
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
            max_channels: 8,
            max_points_per_channel: 32,
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
