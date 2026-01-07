//! Vec-based in-memory RTDB implementation
//!
//! Provides O(1) read access for hot paths, complementing Redis-backed storage.
//! Used as a local cache for high-frequency point reads.
//!
//! # Architecture
//!
//! ```text
//! VecRtdb
//!   └─ stores: RwLock<FxHashMap<(channel_id, point_type), ChannelVecStore>>
//!        └─ ChannelVecStore
//!             ├─ slots: Box<[PointSlot]>  (contiguous memory)
//!             └─ index: FxHashMap<point_id, slot_index>
//! ```
//!
//! # Instance Mode (for modsrv rule engine)
//!
//! VecRtdb can also be used as a read-through cache for instance data:
//! - `(instance_id, MEASUREMENT)` → inst:{id}:M hash
//! - `(instance_id, ACTION)` → inst:{id}:A hash
//!
//! Use `register_instance()`, `get_measurement()`, `set_measurement()` etc.

use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// ========== Instance Point Type Constants (Round 111) ==========

/// Instance point type constants for VecRtdb instance mode.
///
/// These map to Redis key suffixes:
/// - `MEASUREMENT` (0) → `inst:{id}:M`
/// - `ACTION` (1) → `inst:{id}:A`
pub mod instance_point_type {
    /// Measurement points (inst:{id}:M) - read from comsrv via C2M routing
    pub const MEASUREMENT: u8 = 0;
    /// Action points (inst:{id}:A) - written by rule engine
    pub const ACTION: u8 = 1;
}

// ========== PointSlot (Round 104) ==========

/// Point slot for atomic storage of point data
///
/// 32-byte aligned for cache-line friendliness.
/// Uses atomic operations for lock-free concurrent access.
#[repr(C, align(32))]
pub struct PointSlot {
    /// Engineering value (IEEE 754 double as bits)
    value_bits: AtomicU64,
    /// Timestamp in milliseconds
    timestamp: AtomicU64,
    /// Raw value (as bits)
    raw_bits: AtomicU64,
    /// Flags: bit 0 = dirty, bits 1-7 = quality
    flags: AtomicU64,
}

impl Default for PointSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl PointSlot {
    /// Create a new empty point slot
    pub const fn new() -> Self {
        Self {
            value_bits: AtomicU64::new(0),
            timestamp: AtomicU64::new(0),
            raw_bits: AtomicU64::new(0),
            flags: AtomicU64::new(0),
        }
    }

    /// Get the engineering value
    #[inline]
    pub fn get_value(&self) -> f64 {
        f64::from_bits(self.value_bits.load(Ordering::Relaxed))
    }

    /// Get the engineering value with specified ordering (for shared memory)
    #[inline]
    pub fn load_value(&self, order: Ordering) -> f64 {
        f64::from_bits(self.value_bits.load(order))
    }

    /// Get the timestamp in milliseconds
    #[inline]
    pub fn get_timestamp(&self) -> u64 {
        self.timestamp.load(Ordering::Relaxed)
    }

    /// Get the raw value
    #[inline]
    pub fn get_raw(&self) -> f64 {
        f64::from_bits(self.raw_bits.load(Ordering::Relaxed))
    }

    /// Set all point data atomically (per-field)
    #[inline]
    pub fn set(&self, value: f64, raw: f64, timestamp: u64) {
        self.value_bits.store(value.to_bits(), Ordering::Relaxed);
        self.raw_bits.store(raw.to_bits(), Ordering::Relaxed);
        self.timestamp.store(timestamp, Ordering::Relaxed);
        // Set dirty flag
        self.flags.fetch_or(1, Ordering::Relaxed);
    }

    /// Check if dirty flag is set
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.flags.load(Ordering::Relaxed) & 1 != 0
    }

    /// Clear the dirty flag
    #[inline]
    pub fn clear_dirty(&self) {
        self.flags.fetch_and(!1, Ordering::Relaxed);
    }
}

// ========== ChannelVecStore (Round 105) ==========

pub struct ChannelVecStore {
    /// Contiguous memory storage for point slots
    slots: Box<[PointSlot]>,
    /// point_id → slot_index direct mapping array
    /// Index by point_id, value is slot index in `slots`
    /// Uses u32::MAX as sentinel for non-existent points
    point_to_slot: Box<[u32]>,
    /// Maximum valid point_id (for bounds checking)
    max_point_id: u32,
    /// Number of registered points
    point_count: u32,
    /// Channel metadata
    channel_id: u32,
    point_type: u8,
}

impl ChannelVecStore {
    /// Sentinel value for non-existent points in the mapping array
    const INVALID_SLOT: u32 = u32::MAX;

    /// Create a new channel store from a list of point IDs
    ///
    /// Uses Vec direct indexing for O(1) point lookup (~1-5ns vs ~20ns for HashMap).
    /// The mapping array is indexed by point_id, with u32::MAX as sentinel for
    /// non-existent points.
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `point_type` - Point type (0=T, 1=S, 2=C, 3=A)
    /// * `point_ids` - List of point IDs to allocate slots for
    ///
    /// # Memory Trade-off
    /// For sparse point_ids (e.g., [1, 100, 10000]), the mapping array may waste
    /// memory. However, in typical industrial control scenarios, point_ids are
    /// usually contiguous (0-255), making this trade-off favorable.
    pub fn new(channel_id: u32, point_type: u8, point_ids: &[u32]) -> Self {
        let point_count = point_ids.len();

        // Find max point_id for mapping array size
        let max_point_id = point_ids.iter().copied().max().unwrap_or(0);

        // Allocate mapping array (index by point_id)
        // Initialize with INVALID_SLOT sentinel
        let mut point_to_slot = vec![Self::INVALID_SLOT; (max_point_id + 1) as usize];
        let mut slots = Vec::with_capacity(point_count);

        for (slot_idx, &point_id) in point_ids.iter().enumerate() {
            slots.push(PointSlot::new());
            point_to_slot[point_id as usize] = slot_idx as u32;
        }

        Self {
            slots: slots.into_boxed_slice(),
            point_to_slot: point_to_slot.into_boxed_slice(),
            max_point_id,
            point_count: point_count as u32,
            channel_id,
            point_type,
        }
    }

    /// Get point data by point ID
    ///
    /// Returns (value, raw_value, timestamp) or None if not found.
    ///
    /// # Performance
    /// O(1) direct array access (~1-5ns) - no hashing or collision handling.
    #[inline]
    pub fn get(&self, point_id: u32) -> Option<(f64, f64, u64)> {
        // Bounds check: point_id must be within mapping array
        if point_id > self.max_point_id {
            return None;
        }
        // Direct array lookup
        let slot_idx = self.point_to_slot[point_id as usize];
        if slot_idx == Self::INVALID_SLOT {
            return None;
        }
        // Access slot data
        let slot = &self.slots[slot_idx as usize];
        Some((slot.get_value(), slot.get_raw(), slot.get_timestamp()))
    }

    /// Set point data by point ID
    ///
    /// Returns true if the point exists and was updated.
    ///
    /// # Performance
    /// O(1) direct array access (~1-5ns) - no hashing or collision handling.
    #[inline]
    pub fn set(&self, point_id: u32, value: f64, raw: f64, timestamp: u64) -> bool {
        // Bounds check
        if point_id > self.max_point_id {
            return false;
        }
        // Direct array lookup
        let slot_idx = self.point_to_slot[point_id as usize];
        if slot_idx == Self::INVALID_SLOT {
            return false;
        }
        // Write to slot
        self.slots[slot_idx as usize].set(value, raw, timestamp);
        true
    }

    /// Get channel ID
    pub fn channel_id(&self) -> u32 {
        self.channel_id
    }

    /// Get point type
    pub fn point_type(&self) -> u8 {
        self.point_type
    }

    /// Get the number of points in this store
    pub fn len(&self) -> usize {
        self.point_count as usize
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

// ========== VecRtdb (Round 106) ==========

/// Global Vec-based realtime database
///
/// Provides fast in-memory storage for point data, used as a read cache
/// alongside Redis-backed persistent storage.
pub struct VecRtdb {
    /// (channel_id, point_type) → ChannelVecStore
    stores: RwLock<FxHashMap<(u32, u8), ChannelVecStore>>,
}

impl Default for VecRtdb {
    fn default() -> Self {
        Self::new()
    }
}

impl VecRtdb {
    /// Create a new empty VecRtdb
    pub fn new() -> Self {
        Self {
            stores: RwLock::new(FxHashMap::default()),
        }
    }

    /// Register a channel with its point IDs (called at startup)
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `point_type` - Point type (0=T, 1=S, 2=C, 3=A)
    /// * `point_ids` - List of point IDs for this channel/type
    pub fn register_channel(&self, channel_id: u32, point_type: u8, point_ids: &[u32]) {
        let store = ChannelVecStore::new(channel_id, point_type, point_ids);
        self.stores.write().insert((channel_id, point_type), store);
    }

    /// Read point data
    ///
    /// Returns (value, raw_value, timestamp) or None if not found.
    #[inline]
    pub fn get(&self, channel_id: u32, point_type: u8, point_id: u32) -> Option<(f64, f64, u64)> {
        self.stores
            .read()
            .get(&(channel_id, point_type))
            .and_then(|store| store.get(point_id))
    }

    /// Write point data
    ///
    /// Returns true if the point exists and was updated.
    #[inline]
    pub fn set(
        &self,
        channel_id: u32,
        point_type: u8,
        point_id: u32,
        value: f64,
        raw: f64,
        timestamp: u64,
    ) -> bool {
        if let Some(store) = self.stores.read().get(&(channel_id, point_type)) {
            store.set(point_id, value, raw, timestamp)
        } else {
            false
        }
    }

    /// Get statistics about the cache
    pub fn stats(&self) -> VecRtdbStats {
        let stores = self.stores.read();
        let channel_count = stores.len();
        let point_count: usize = stores.values().map(|s| s.len()).sum();
        VecRtdbStats {
            channel_count,
            point_count,
        }
    }

    /// Bulk register multiple channels at once
    ///
    /// # Arguments
    /// * `channels` - Iterator of (channel_id, point_type, point_ids) tuples
    ///
    /// # Returns
    /// Total number of points registered
    pub fn register_channels<I>(&self, channels: I) -> usize
    where
        I: IntoIterator<Item = (u32, u8, Vec<u32>)>,
    {
        let mut total_points = 0;
        for (channel_id, point_type, point_ids) in channels {
            total_points += point_ids.len();
            self.register_channel(channel_id, point_type, &point_ids);
        }
        total_points
    }

    /// Check if a channel is registered
    pub fn has_channel(&self, channel_id: u32, point_type: u8) -> bool {
        self.stores.read().contains_key(&(channel_id, point_type))
    }

    // ========== Instance Mode API (Round 111) ==========

    /// Register an instance with its point IDs (for modsrv rule engine)
    ///
    /// This is the instance-mode equivalent of `register_channel()`.
    /// Reuses the same underlying storage with `(instance_id, point_type)` as key.
    ///
    /// # Arguments
    /// * `instance_id` - Instance identifier
    /// * `measurement_points` - Measurement point IDs (inst:{id}:M)
    /// * `action_points` - Action point IDs (inst:{id}:A)
    pub fn register_instance(
        &self,
        instance_id: u32,
        measurement_points: &[u32],
        action_points: &[u32],
    ) {
        if !measurement_points.is_empty() {
            self.register_channel(
                instance_id,
                instance_point_type::MEASUREMENT,
                measurement_points,
            );
        }
        if !action_points.is_empty() {
            self.register_channel(instance_id, instance_point_type::ACTION, action_points);
        }
    }

    /// Get measurement value (inst:{id}:M)
    ///
    /// Returns only the engineering value for rule engine use.
    /// For full data (value, raw, timestamp), use `get()` directly.
    #[inline]
    pub fn get_measurement(&self, instance_id: u32, point_id: u32) -> Option<f64> {
        self.get(instance_id, instance_point_type::MEASUREMENT, point_id)
            .map(|(value, _, _)| value)
    }

    /// Get action value (inst:{id}:A)
    ///
    /// Returns only the engineering value for rule engine use.
    #[inline]
    pub fn get_action(&self, instance_id: u32, point_id: u32) -> Option<f64> {
        self.get(instance_id, instance_point_type::ACTION, point_id)
            .map(|(value, _, _)| value)
    }

    /// Set measurement value (read-through cache update)
    ///
    /// Called when Redis read completes to populate the cache.
    /// Raw value defaults to engineering value for instance data.
    #[inline]
    pub fn set_measurement(
        &self,
        instance_id: u32,
        point_id: u32,
        value: f64,
        timestamp: u64,
    ) -> bool {
        self.set(
            instance_id,
            instance_point_type::MEASUREMENT,
            point_id,
            value,
            value, // raw = value for instance data
            timestamp,
        )
    }

    /// Set action value
    ///
    /// Called when rule engine writes action outputs.
    #[inline]
    pub fn set_action(&self, instance_id: u32, point_id: u32, value: f64, timestamp: u64) -> bool {
        self.set(
            instance_id,
            instance_point_type::ACTION,
            point_id,
            value,
            value, // raw = value for instance data
            timestamp,
        )
    }

    /// Check if an instance is registered
    pub fn has_instance(&self, instance_id: u32) -> bool {
        self.has_channel(instance_id, instance_point_type::MEASUREMENT)
            || self.has_channel(instance_id, instance_point_type::ACTION)
    }
}

/// Statistics for VecRtdb
#[derive(Debug, Clone, Copy)]
pub struct VecRtdbStats {
    /// Number of registered channels
    pub channel_count: usize,
    /// Total number of points across all channels
    pub point_count: usize,
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_point_slot_atomic_ops() {
        let slot = PointSlot::new();
        slot.set(100.5, 1005.0, 1729000000);

        assert_eq!(slot.get_value(), 100.5);
        assert_eq!(slot.get_raw(), 1005.0);
        assert_eq!(slot.get_timestamp(), 1729000000);
        assert!(slot.is_dirty());

        slot.clear_dirty();
        assert!(!slot.is_dirty());
    }

    #[test]
    fn test_channel_vec_store() {
        let store = ChannelVecStore::new(1001, 0, &[1, 2, 3]);

        assert_eq!(store.channel_id(), 1001);
        assert_eq!(store.point_type(), 0);
        assert_eq!(store.len(), 3);

        // Initially all zeros
        let (v, r, t) = store.get(1).unwrap();
        assert_eq!(v, 0.0);
        assert_eq!(r, 0.0);
        assert_eq!(t, 0);

        // Set and verify
        assert!(store.set(1, 100.5, 1005.0, 1729000000));
        let (v, r, t) = store.get(1).unwrap();
        assert_eq!(v, 100.5);
        assert_eq!(r, 1005.0);
        assert_eq!(t, 1729000000);

        // Non-existent point
        assert!(store.get(999).is_none());
        assert!(!store.set(999, 1.0, 1.0, 1));
    }

    #[test]
    fn test_vec_rtdb_read_write() {
        let rtdb = VecRtdb::new();
        rtdb.register_channel(1001, 0, &[1, 2, 3]);

        assert!(rtdb.set(1001, 0, 1, 100.5, 1005.0, 1729000000));

        let result = rtdb.get(1001, 0, 1);
        assert_eq!(result, Some((100.5, 1005.0, 1729000000)));

        // Non-existent channel
        assert!(rtdb.get(9999, 0, 1).is_none());

        // Non-existent point in existing channel
        assert!(rtdb.get(1001, 0, 999).is_none());
    }

    #[test]
    fn test_vec_rtdb_stats() {
        let rtdb = VecRtdb::new();
        rtdb.register_channel(1001, 0, &[1, 2, 3]);
        rtdb.register_channel(1002, 0, &[1, 2]);

        let stats = rtdb.stats();
        assert_eq!(stats.channel_count, 2);
        assert_eq!(stats.point_count, 5);
    }

    #[test]
    fn test_vec_rtdb_bulk_register() {
        let rtdb = VecRtdb::new();

        // Bulk register channels
        let channels = vec![
            (1001, 0, vec![1, 2, 3]), // Telemetry
            (1001, 2, vec![10, 11]),  // Control
            (1002, 0, vec![1, 2]),    // Telemetry
        ];

        let total = rtdb.register_channels(channels);
        assert_eq!(total, 7);

        // Verify channels exist
        assert!(rtdb.has_channel(1001, 0));
        assert!(rtdb.has_channel(1001, 2));
        assert!(rtdb.has_channel(1002, 0));
        assert!(!rtdb.has_channel(9999, 0));

        // Verify stats
        let stats = rtdb.stats();
        assert_eq!(stats.channel_count, 3);
        assert_eq!(stats.point_count, 7);
    }

    // ========== Instance Mode Tests (Round 111) ==========

    #[test]
    fn test_instance_register_and_check() {
        let rtdb = VecRtdb::new();

        // Register instance with measurement and action points
        rtdb.register_instance(5, &[1, 2, 3], &[10, 11]);

        // Verify instance exists
        assert!(rtdb.has_instance(5));
        assert!(!rtdb.has_instance(999));

        // Verify underlying channel registration
        assert!(rtdb.has_channel(5, super::instance_point_type::MEASUREMENT));
        assert!(rtdb.has_channel(5, super::instance_point_type::ACTION));
    }

    #[test]
    fn test_instance_measurement_read_write() {
        let rtdb = VecRtdb::new();
        rtdb.register_instance(5, &[1, 2, 3], &[]);

        // Initially 0.0 (default value for registered points)
        assert_eq!(rtdb.get_measurement(5, 1), Some(0.0));

        // Set measurement value (simulating read-through cache update)
        assert!(rtdb.set_measurement(5, 1, 100.5, 1729000000));
        assert!(rtdb.set_measurement(5, 2, 200.0, 1729000000));

        // Verify reads
        assert_eq!(rtdb.get_measurement(5, 1), Some(100.5));
        assert_eq!(rtdb.get_measurement(5, 2), Some(200.0));
        assert_eq!(rtdb.get_measurement(5, 3), Some(0.0)); // Registered but not set
        assert!(rtdb.get_measurement(5, 999).is_none()); // Non-existent point
    }

    #[test]
    fn test_instance_action_read_write() {
        let rtdb = VecRtdb::new();
        rtdb.register_instance(5, &[], &[10, 11]);

        // Set action values
        assert!(rtdb.set_action(5, 10, 1.0, 1729000000));
        assert!(rtdb.set_action(5, 11, 0.0, 1729000000));

        // Verify reads
        assert_eq!(rtdb.get_action(5, 10), Some(1.0));
        assert_eq!(rtdb.get_action(5, 11), Some(0.0));
        assert!(rtdb.get_action(5, 999).is_none());
    }

    #[test]
    fn test_instance_measurement_and_action_isolation() {
        let rtdb = VecRtdb::new();

        // Register instance with both measurement and action points
        // Note: point_id=1 exists in both, but they're isolated by point_type
        rtdb.register_instance(5, &[1, 2], &[1, 2]);

        // Set different values for same point_id in different types
        rtdb.set_measurement(5, 1, 100.0, 1729000000);
        rtdb.set_action(5, 1, 1.0, 1729000000);

        // Verify isolation
        assert_eq!(rtdb.get_measurement(5, 1), Some(100.0));
        assert_eq!(rtdb.get_action(5, 1), Some(1.0));
    }

    #[test]
    fn test_instance_empty_registration() {
        let rtdb = VecRtdb::new();

        // Register with empty lists (should not panic)
        rtdb.register_instance(5, &[], &[]);

        // Instance should not be registered (no points)
        assert!(!rtdb.has_instance(5));
    }
}
