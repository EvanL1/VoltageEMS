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
use std::sync::atomic::{fence, AtomicU64, Ordering};

// ========== Instance Point Type Constants ==========

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

// ========== PointSlot ==========

/// Point slot for atomic storage of point data
///
/// 32-byte aligned for cache-line friendliness.
/// Uses atomic operations for lock-free concurrent access.
///
/// # Seqlock Protocol
///
/// Uses the upper 32 bits of `flags` as a seqlock sequence counter to guarantee
/// consistent reads across all fields. This prevents torn reads when the same
/// slot is written multiple times within the same millisecond (where timestamp
/// alone cannot distinguish writes).
///
/// - **Writer**: increments sequence to odd (write-in-progress), writes data,
///   increments sequence to even (write-complete).
/// - **Reader**: reads sequence, reads data, re-reads sequence. If both reads
///   match and the value is even, the data is consistent.
///
/// # Safety (shared memory usage)
///
/// This struct is `#[repr(C)]` to guarantee a deterministic field layout
/// when cast from raw pointers in `unified_shm.rs`. The compile-time
/// assertion below ensures the size never drifts from expectations.
#[repr(C, align(32))]
pub struct PointSlot {
    /// Engineering value (IEEE 754 double as bits)
    value_bits: AtomicU64,
    /// Timestamp in milliseconds
    timestamp: AtomicU64,
    /// Raw value (as bits)
    raw_bits: AtomicU64,
    /// Flags layout:
    /// - bits 0:    dirty flag (1 = modified since last flush)
    /// - bits 1-7:  quality code (reserved)
    /// - bits 32-63: seqlock sequence counter (odd = write in progress)
    flags: AtomicU64,
}

/// Seqlock sequence counter occupies the upper 32 bits of `flags`.
/// Each write cycle increments it twice: once before (odd) and once after (even).
const SEQ_INCREMENT: u64 = 1u64 << 32;

const _: () = assert!(std::mem::size_of::<PointSlot>() == 32);

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

    /// Maximum retry attempts for `load_consistent` before returning possibly stale data.
    ///
    /// Must be large enough to handle bursts of rapid writes with multiple
    /// concurrent readers. Under extreme cache-line contention on AArch64,
    /// each retry can take 100-500ns. 32768 retries bounds worst-case
    /// spinning to ~3-16ms, acceptable for SCADA rule execution while being
    /// virtually impossible to exhaust in production (where protocol I/O
    /// between writes makes retries almost never exceed single digits).
    const MAX_CONSISTENCY_RETRIES: u32 = 32_768;

    /// Load all point data with consistency guarantee (seqlock read protocol).
    ///
    /// Loops [`try_load_consistent`](Self::try_load_consistent) up to
    /// [`MAX_CONSISTENCY_RETRIES`](Self::MAX_CONSISTENCY_RETRIES) times.
    /// On exhaustion, falls back to an unprotected read (may be inconsistent).
    ///
    /// See `try_load_consistent` for the seqlock algorithm and memory ordering details.
    #[inline]
    pub fn load_consistent(&self) -> (f64, f64, u64) {
        for _ in 0..Self::MAX_CONSISTENCY_RETRIES {
            if let Some(result) = self.try_load_consistent() {
                return result;
            }
            std::hint::spin_loop();
        }

        tracing::warn!(
            "load_consistent exceeded {} retries, returning possibly stale data",
            Self::MAX_CONSISTENCY_RETRIES
        );
        self.load_relaxed()
    }

    /// Single-attempt seqlock read. Returns `None` on contention instead of retrying.
    ///
    /// This is the core seqlock read protocol used by both `load_consistent` (retrying)
    /// and callers that prefer to skip stale data rather than spin.
    ///
    /// ## Algorithm
    ///
    /// 1. Read flags (Relaxed) to get sequence counter
    /// 2. If sequence is odd (write in progress), return None
    /// 3. **fence(SeqCst)** — full memory barrier (`dmb ish` on ARM)
    /// 4. Read value, raw, timestamp (Relaxed)
    /// 5. **fence(SeqCst)** — full memory barrier
    /// 6. Re-read flags (Relaxed)
    /// 7. If sequence unchanged → `Some(data)`; otherwise `None`
    ///
    /// ## Memory Ordering (pure fence-based, AArch64-safe)
    ///
    /// All individual atomic operations use `Relaxed` ordering. Cross-address
    /// ordering is enforced entirely by explicit `fence(SeqCst)` barriers,
    /// which compile to `dmb ish` (full barrier) on AArch64. This follows
    /// the Linux kernel seqlock pattern (`smp_rmb()`/`smp_wmb()` between
    /// sequence counter operations and data access).
    ///
    /// On x86 (TSO), `fence(SeqCst)` compiles to `mfence` and Relaxed
    /// loads/stores compile to plain `mov`.
    #[inline]
    pub fn try_load_consistent(&self) -> Option<(f64, f64, u64)> {
        let flags1 = self.flags.load(Ordering::Relaxed);
        let seq1 = flags1 >> 32;

        if seq1 & 1 != 0 {
            return None;
        }

        fence(Ordering::SeqCst);

        let value = f64::from_bits(self.value_bits.load(Ordering::Relaxed));
        let raw = f64::from_bits(self.raw_bits.load(Ordering::Relaxed));
        let ts = self.timestamp.load(Ordering::Relaxed);

        fence(Ordering::SeqCst);

        let flags2 = self.flags.load(Ordering::Relaxed);
        let seq2 = flags2 >> 32;

        if seq1 == seq2 {
            Some((value, raw, ts))
        } else {
            None
        }
    }

    /// Unprotected read of all fields (no seqlock guarantee).
    ///
    /// Used only as a last-resort fallback after retries are exhausted.
    /// A leading `fence(SeqCst)` provides best-effort freshness.
    #[inline]
    fn load_relaxed(&self) -> (f64, f64, u64) {
        fence(Ordering::SeqCst);
        let value = f64::from_bits(self.value_bits.load(Ordering::Relaxed));
        let raw = f64::from_bits(self.raw_bits.load(Ordering::Relaxed));
        let ts = self.timestamp.load(Ordering::Relaxed);
        (value, raw, ts)
    }

    /// Set all point data with seqlock write protocol.
    ///
    /// Increments the sequence counter to odd (write-in-progress), writes all
    /// data fields, then increments to even (write-complete). Readers that
    /// observe an odd sequence or a changed sequence will retry.
    ///
    /// # Memory Ordering (fence-based, AArch64-safe)
    ///
    /// Uses explicit `fence(SeqCst)` barriers between the sequence counter
    /// and data stores, following the Linux kernel seqlock pattern (`smp_wmb`).
    /// This guarantees cross-address ordering on weakly-ordered architectures:
    ///
    /// - **After odd increment**: fence ensures the odd sequence is globally
    ///   visible before any data store. Readers will see "write in progress".
    /// - **Before even increment**: fence ensures ALL data stores are globally
    ///   visible before the even sequence is published.
    ///
    /// On x86 (TSO), `fence(SeqCst)` compiles to `mfence` and stores compile
    /// to plain `mov`, so overhead is minimal on server hardware.
    #[inline]
    pub fn set(&self, value: f64, raw: f64, timestamp: u64) {
        // Begin write: sequence → odd (signals write-in-progress)
        // Relaxed: ordering enforced by the fence below, not by this RMW.
        self.flags.fetch_add(SEQ_INCREMENT, Ordering::Relaxed);

        // FULL BARRIER (dmb ish on ARM, mfence on x86):
        // Ensures the odd sequence is globally visible to ALL cores
        // before any data store. Prevents Store→Store reordering across
        // different addresses (flags vs value_bits/raw_bits/timestamp).
        fence(Ordering::SeqCst);

        // Data stores — Relaxed because ordering is fence-enclosed.
        self.value_bits.store(value.to_bits(), Ordering::Relaxed);
        self.raw_bits.store(raw.to_bits(), Ordering::Relaxed);
        self.timestamp.store(timestamp, Ordering::Relaxed);

        // FULL BARRIER:
        // Ensures ALL data stores are globally visible before the
        // even sequence is published. Readers that see the even seq
        // after their own fence(SeqCst) will see all data values.
        fence(Ordering::SeqCst);

        // End write: sequence → even (signals write-complete)
        self.flags.fetch_add(SEQ_INCREMENT, Ordering::Relaxed);

        // Dirty flag (advisory, not part of seqlock protocol)
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

// ========== ChannelVecStore ==========

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

    /// O(1) slot lookup by point_id (bounds check + direct array index).
    #[inline]
    fn get_slot(&self, point_id: u32) -> Option<&PointSlot> {
        if point_id > self.max_point_id {
            return None;
        }
        let slot_idx = self.point_to_slot[point_id as usize];
        if slot_idx == Self::INVALID_SLOT {
            return None;
        }
        Some(&self.slots[slot_idx as usize])
    }

    /// Get point data by point ID
    ///
    /// Returns (value, raw_value, timestamp) or None if not found.
    #[inline]
    pub fn get(&self, point_id: u32) -> Option<(f64, f64, u64)> {
        let slot = self.get_slot(point_id)?;
        Some((slot.get_value(), slot.get_raw(), slot.get_timestamp()))
    }

    /// Set point data by point ID
    ///
    /// Returns true if the point exists and was updated.
    #[inline]
    pub fn set(&self, point_id: u32, value: f64, raw: f64, timestamp: u64) -> bool {
        if let Some(slot) = self.get_slot(point_id) {
            slot.set(value, raw, timestamp);
            true
        } else {
            false
        }
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

// ========== VecRtdb ==========

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

    // ========== Instance Mode API ==========

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

    // ========== Instance Mode Tests ==========

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

    // ========== load_consistent Tests ==========

    #[test]
    fn test_load_consistent_basic() {
        let slot = PointSlot::new();
        slot.set(100.5, 1005.0, 1729000000);

        // load_consistent should return same values as individual getters
        let (value, raw, ts) = slot.load_consistent();
        assert_eq!(value, 100.5);
        assert_eq!(raw, 1005.0);
        assert_eq!(ts, 1729000000);
    }

    #[test]
    fn test_try_load_consistent_basic() {
        let slot = PointSlot::new();
        slot.set(42.0, 420.0, 12345);

        // try_load_consistent should succeed without concurrent writes
        let result = slot.try_load_consistent();
        assert!(result.is_some());

        let (value, raw, ts) = result.unwrap();
        assert_eq!(value, 42.0);
        assert_eq!(raw, 420.0);
        assert_eq!(ts, 12345);
    }

    #[test]
    fn test_load_consistent_multiple_updates() {
        let slot = PointSlot::new();

        // Multiple sequential updates
        for i in 1..=10 {
            let v = i as f64 * 10.0;
            slot.set(v, v * 10.0, i as u64 * 1000);

            let (value, raw, ts) = slot.load_consistent();
            assert_eq!(value, v);
            assert_eq!(raw, v * 10.0);
            assert_eq!(ts, i as u64 * 1000);
        }
    }

    #[test]
    fn test_load_consistent_zero_values() {
        let slot = PointSlot::new();

        // Default values
        let (value, raw, ts) = slot.load_consistent();
        assert_eq!(value, 0.0);
        assert_eq!(raw, 0.0);
        assert_eq!(ts, 0);
    }

    #[test]
    fn test_load_consistent_same_timestamp() {
        // Regression test: multiple writes with identical timestamp must still
        // produce consistent reads via seqlock (not timestamp comparison).
        let slot = PointSlot::new();
        let same_ts = 1729000000u64;

        slot.set(100.0, 1000.0, same_ts);
        let (v1, r1, t1) = slot.load_consistent();
        assert_eq!(v1, 100.0);
        assert_eq!(r1, 1000.0);
        assert_eq!(t1, same_ts);

        // Second write with SAME timestamp but different values
        slot.set(200.0, 2000.0, same_ts);
        let (v2, r2, t2) = slot.load_consistent();
        assert_eq!(v2, 200.0);
        assert_eq!(r2, 2000.0);
        assert_eq!(t2, same_ts);

        // Verify try_load_consistent also works
        slot.set(300.0, 3000.0, same_ts);
        let result = slot.try_load_consistent();
        assert!(result.is_some());
        let (v3, r3, t3) = result.unwrap();
        assert_eq!(v3, 300.0);
        assert_eq!(r3, 3000.0);
        assert_eq!(t3, same_ts);
    }

    #[test]
    fn test_seqlock_dirty_flag_preserved() {
        let slot = PointSlot::new();

        // After set(), dirty flag should be set
        slot.set(1.0, 1.0, 100);
        assert!(slot.is_dirty());

        // Clear dirty and verify
        slot.clear_dirty();
        assert!(!slot.is_dirty());

        // After another set(), dirty should be set again
        slot.set(2.0, 2.0, 200);
        assert!(slot.is_dirty());

        // load_consistent should still work after dirty flag operations
        let (v, r, ts) = slot.load_consistent();
        assert_eq!(v, 2.0);
        assert_eq!(r, 2.0);
        assert_eq!(ts, 200);
    }

    #[test]
    fn test_seqlock_concurrent_read_write() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        use std::thread;

        let slot = Arc::new(PointSlot::new());
        let running = Arc::new(AtomicBool::new(true));
        let write_iterations = 100_000;

        // Pre-seed a value so the slot is not empty before threads start
        slot.set(0.0, 0.0, 42);

        // Writer thread: rapidly writes with same timestamp
        let writer_slot = Arc::clone(&slot);
        let writer_running = Arc::clone(&running);
        let writer = thread::spawn(move || {
            for i in 0..write_iterations {
                let v = i as f64;
                writer_slot.set(v, v * 10.0, 42);
                // Yield occasionally so the reader can grab a consistent snapshot
                if i % 1000 == 0 {
                    thread::yield_now();
                }
            }
            writer_running.store(false, Ordering::Relaxed);
        });

        // Reader thread: try_load_consistent to verify seqlock correctness
        // (avoids fallback path which is not seqlock-protected)
        let reader_slot = Arc::clone(&slot);
        let reader_running = Arc::clone(&running);
        let reader = thread::spawn(move || {
            let mut consistent_reads = 0u64;
            // Keep reading while writer is active, then do a final batch
            while reader_running.load(Ordering::Relaxed) {
                if let Some((value, raw, _ts)) = reader_slot.try_load_consistent() {
                    assert!(
                        (raw - value * 10.0).abs() < f64::EPSILON || value == 0.0,
                        "Torn read detected: value={value}, raw={raw} (expected raw={})",
                        value * 10.0
                    );
                    consistent_reads += 1;
                }
                thread::yield_now();
            }
            // Writer is done — reads should always succeed now
            for _ in 0..100 {
                if let Some((value, raw, _ts)) = reader_slot.try_load_consistent() {
                    assert!(
                        (raw - value * 10.0).abs() < f64::EPSILON || value == 0.0,
                        "Torn read detected: value={value}, raw={raw} (expected raw={})",
                        value * 10.0
                    );
                    consistent_reads += 1;
                }
            }
            consistent_reads
        });

        writer.join().unwrap();
        let reads = reader.join().unwrap();
        assert!(reads > 0, "No consistent reads achieved");
    }

    // ========== Additional Seqlock Tests (test-expert) ==========

    #[test]
    fn test_seqlock_sequence_counter_values() {
        // Verify the sequence counter in flags[32:63] increments by 2 per write
        let slot = PointSlot::new();

        let get_seq = || slot.flags.load(Ordering::Relaxed) >> 32;

        assert_eq!(get_seq(), 0, "initial sequence should be 0");
        assert_eq!(get_seq() & 1, 0, "initial sequence should be even");

        slot.set(1.0, 1.0, 100);
        assert_eq!(get_seq(), 2, "after 1 write, sequence should be 2");

        slot.set(2.0, 2.0, 200);
        assert_eq!(get_seq(), 4, "after 2 writes, sequence should be 4");

        slot.set(3.0, 3.0, 300);
        assert_eq!(get_seq(), 6, "after 3 writes, sequence should be 6");
    }

    #[test]
    fn test_seqlock_same_timestamp_rapid_writes() {
        // Hammers the same slot with identical timestamps, validates consistency
        let slot = PointSlot::new();
        let ts = 9999u64;

        for i in 0..1000 {
            let v = i as f64 * 0.01;
            let r = v + 1000.0;
            slot.set(v, r, ts);

            let (rv, rr, rt) = slot.load_consistent();
            assert_eq!(rv, v, "value mismatch at iteration {}", i);
            assert_eq!(rr, r, "raw mismatch at iteration {}", i);
            assert_eq!(rt, ts, "timestamp mismatch at iteration {}", i);
        }
    }

    #[test]
    fn test_seqlock_multi_reader_stress() {
        // Multiple reader threads + 1 writer: verifies the seqlock protocol itself
        // is correct — uses try_load_consistent() which returns None instead of
        // falling back to unprotected reads under extreme contention.
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        use std::thread;

        let slot = Arc::new(PointSlot::new());
        let running = Arc::new(AtomicBool::new(true));

        // Writer: encodes i into all fields with a known relationship
        let w_slot = Arc::clone(&slot);
        let w_running = Arc::clone(&running);
        let writer = thread::spawn(move || {
            let mut i = 0u64;
            while w_running.load(Ordering::Relaxed) {
                let v = i as f64;
                w_slot.set(v, v * 10.0, i);
                i += 1;
            }
            i
        });

        // 4 readers verify consistency using try_load_consistent (no fallback)
        let readers: Vec<_> = (0..4)
            .map(|_| {
                let r_slot = Arc::clone(&slot);
                let r_running = Arc::clone(&running);
                thread::spawn(move || {
                    let mut torn = 0u64;
                    let mut consistent = 0u64;
                    let mut skipped = 0u64;
                    while r_running.load(Ordering::Relaxed) {
                        // try_load_consistent returns None on contention instead
                        // of falling back to an unprotected read
                        if let Some((value, raw, ts)) = r_slot.try_load_consistent() {
                            consistent += 1;
                            if value != 0.0 {
                                let raw_ok = (raw - value * 10.0).abs() < f64::EPSILON;
                                let ts_ok = ts == value as u64;
                                if !raw_ok || !ts_ok {
                                    torn += 1;
                                }
                            }
                        } else {
                            skipped += 1;
                        }
                    }
                    (consistent, skipped, torn)
                })
            })
            .collect();

        thread::sleep(std::time::Duration::from_millis(150));
        running.store(false, Ordering::Relaxed);

        let writes = writer.join().unwrap();
        for handle in readers {
            let (consistent, _skipped, torn) = handle.join().unwrap();
            assert_eq!(
                torn, 0,
                "Torn reads detected: {torn}/{consistent} (seqlock protocol bug!)"
            );
        }
        assert!(writes > 100, "Writer did too few iterations: {writes}");
    }

    #[test]
    fn test_point_slot_layout_stability() {
        // SHM binary compatibility: PointSlot must remain exactly 32 bytes, 32-byte aligned
        assert_eq!(std::mem::size_of::<PointSlot>(), 32);
        assert_eq!(std::mem::align_of::<PointSlot>(), 32);
    }
}
