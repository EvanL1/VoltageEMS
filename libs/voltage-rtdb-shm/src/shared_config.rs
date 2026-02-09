//! Shared Memory Configuration and Channel-to-Slot Index
//!
//! Provides configuration for shared memory and pre-computed O(1)
//! channel-to-slot mapping for the hottest write path.
//!
//! # Key Components
//!
//! - **SharedConfig**: Configuration for shared memory file path, capacity, and snapshots
//! - **ChannelToSlotIndex**: Pre-computed direct mapping from channel points to SHM slots
//! - **Utility functions**: `default_shm_path()`, `is_shm_available()`, `timestamp_ms()`

use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use tracing::info;
use voltage_model::PointType;

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

    // ========== Snapshot Configuration ==========
    /// Path to snapshot file (None = disabled)
    pub snapshot_path: Option<PathBuf>,
    /// Automatic snapshot interval (None = disabled)
    pub snapshot_interval: Option<std::time::Duration>,
    /// Whether to restore from snapshot on startup
    pub restore_on_start: bool,
}

impl Default for SharedConfig {
    fn default() -> Self {
        Self {
            path: default_shm_path(),
            max_instances: 1024,
            max_points_per_instance: 65536,
            max_channels: 1024,
            max_points_per_channel: 65536,
            // Snapshot defaults
            snapshot_path: None,
            snapshot_interval: None,
            restore_on_start: true,
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

    /// Get shared memory path
    #[inline]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get max slots for unified shared memory
    ///
    /// This is used by the new unified_shm implementation.
    /// Returns the configured value or None if not set.
    #[inline]
    pub fn max_slots(&self) -> Option<u32> {
        // Use max_channels * max_points_per_channel as default
        // This can be overridden by explicit configuration
        Some((self.max_channels * self.max_points_per_channel) as u32)
    }

    /// Create config with explicit max_slots for unified shm
    pub fn with_max_slots(mut self, max_slots: usize) -> Self {
        // Store in max_channels * max_points_per_channel
        // This is a simplification - in practice we might add a dedicated field
        self.max_channels = max_slots;
        self.max_points_per_channel = 1;
        self
    }

    // ========== Snapshot Configuration Methods ==========

    /// Configure snapshot file path
    ///
    /// If set, enables snapshot saving. If None, snapshots are disabled.
    pub fn with_snapshot_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.snapshot_path = Some(path.into());
        self
    }

    /// Configure automatic snapshot interval
    ///
    /// If set, a background task will save snapshots at this interval.
    /// Requires snapshot_path to be set.
    pub fn with_snapshot_interval(mut self, interval: std::time::Duration) -> Self {
        self.snapshot_interval = Some(interval);
        self
    }

    /// Configure whether to restore from snapshot on startup
    ///
    /// Default is true. If false, always starts with fresh empty data.
    pub fn with_restore_on_start(mut self, restore: bool) -> Self {
        self.restore_on_start = restore;
        self
    }

    /// Get snapshot path
    #[inline]
    pub fn snapshot_path(&self) -> Option<&PathBuf> {
        self.snapshot_path.as_ref()
    }

    /// Get snapshot interval
    #[inline]
    pub fn snapshot_interval(&self) -> Option<std::time::Duration> {
        self.snapshot_interval
    }

    /// Check if should restore on start
    #[inline]
    pub fn restore_on_start(&self) -> bool {
        self.restore_on_start
    }

    /// Build snapshot config from environment variables
    ///
    /// Reads:
    /// - SHM_SNAPSHOT_PATH: Path to snapshot file (default: data/shm-snapshot.bin)
    /// - SHM_SNAPSHOT_INTERVAL: Interval in seconds (default: 300 = 5 minutes)
    /// - SHM_RESTORE_ON_START: "true" or "false" (default: true)
    pub fn with_snapshot_from_env(mut self) -> Self {
        use std::env;

        // Snapshot path
        if let Ok(path) = env::var("SHM_SNAPSHOT_PATH") {
            self.snapshot_path = Some(PathBuf::from(path));
        } else {
            // Default path if not explicitly disabled
            self.snapshot_path = Some(PathBuf::from("data/shm-snapshot.bin"));
        }

        // Snapshot interval
        if let Ok(interval_str) = env::var("SHM_SNAPSHOT_INTERVAL") {
            if let Ok(secs) = interval_str.parse::<u64>() {
                self.snapshot_interval = Some(std::time::Duration::from_secs(secs));
            }
        } else {
            // Default: 5 minutes
            self.snapshot_interval = Some(std::time::Duration::from_secs(300));
        }

        // Restore on start
        if let Ok(restore_str) = env::var("SHM_RESTORE_ON_START") {
            self.restore_on_start = restore_str.to_lowercase() != "false";
        }

        self
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

// ========== ChannelToSlotIndex ==========

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
    /// (channel_id, point_type, point_id) → slot index (0-based)
    index: FxHashMap<(u32, PointType, u32), usize>,
    /// Statistics
    mapped_count: usize,
}

impl ChannelToSlotIndex {
    /// Look up slot index for a channel point
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `point_type` - Point type (Telemetry, Signal, Control, Adjustment)
    /// * `point_id` - Point identifier within the channel
    ///
    /// # Returns
    /// Slot index (0-based) for `UnifiedWriter::set_direct`, or None if not mapped
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

    /// Build from UnifiedWriter's channel_layouts
    ///
    /// This method creates a ChannelToSlotIndex from the unified shared memory format.
    /// Stores slot indices (0-based) that map directly to `UnifiedWriter::set_direct`.
    ///
    /// # Arguments
    /// * `writer` - UnifiedWriter with registered channel points
    ///
    /// # Returns
    /// ChannelToSlotIndex with pre-computed mappings
    pub fn from_unified_writer(writer: &crate::unified_shm::UnifiedWriter) -> Self {
        let mut index = FxHashMap::default();
        let layouts = writer.channel_layouts();

        // Iterate through all channels and their layouts
        for (channel_id, layout) in layouts.iter().enumerate() {
            if layout.total_points == 0 {
                continue; // Skip unused channel slots
            }

            // For each point type (T=0, S=1, C=2, A=3)
            for type_idx in 0..4u8 {
                let point_type = match type_idx {
                    0 => PointType::Telemetry,
                    1 => PointType::Signal,
                    2 => PointType::Control,
                    3 => PointType::Adjustment,
                    _ => unreachable!(),
                };

                let count = layout.type_counts[type_idx as usize];
                for point_id in 0..count {
                    if let Some(slot) = layout.slot(type_idx, point_id) {
                        // Store slot index directly (used by set_direct)
                        index.insert((channel_id as u32, point_type, point_id), slot);
                    }
                }
            }
        }

        let mapped_count = index.len();
        info!(
            "ChannelToSlotIndex built from UnifiedWriter: {} direct mappings",
            mapped_count
        );

        Self {
            index,
            mapped_count,
        }
    }
}

// ========== Utility Functions ==========

/// Check if shared memory path is available
pub fn is_shm_available(config: &SharedConfig) -> bool {
    config.path.parent().map(|p| p.exists()).unwrap_or(false)
}
