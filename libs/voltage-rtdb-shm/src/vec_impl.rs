//! Vec-based in-memory point storage
//!
//! Provides atomic `PointSlot` for lock-free point data access in shared memory.

use std::sync::atomic::{fence, AtomicU32, AtomicU64, Ordering};

// ========== Instance Point Type Constants ==========

// ========== PointSlot ==========

/// Point slot for atomic storage of point data
///
/// 32-byte aligned for cache-line friendliness.
/// Uses atomic operations for lock-free concurrent access.
///
/// # Seqlock Protocol
///
/// Uses `seq: AtomicU32` as a seqlock sequence counter to guarantee
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
    /// Seqlock sequence counter (odd = write in progress, even = idle)
    seq: AtomicU32,
    /// Dirty flag (1 = modified since last flush)
    dirty: AtomicU32,
}

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
            seq: AtomicU32::new(0),
            dirty: AtomicU32::new(0),
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
            "load_consistent exceeded {} retries, returning possibly inconsistent data",
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
    /// 1. Read seq (Relaxed) to get sequence counter
    /// 2. If sequence is odd (write in progress), return None
    /// 3. **fence(Acquire)** — prevents data loads from being speculated
    ///    before seq1. x86: compiler barrier only. ARM64: `dmb ishld`.
    /// 4. Read value, raw, timestamp (Relaxed)
    /// 5. Re-read seq with **load(Acquire)** — pairs with writer's
    ///    `store(Release)`, ensures data loads complete before seq2.
    /// 6. If seq unchanged → `Some(data)`; otherwise `None`
    ///
    /// ## Memory Ordering (Acquire/Release, AArch64-safe)
    ///
    /// Reader uses `fence(Acquire)` + `load(Acquire)` to pair with
    /// writer's `store(Release)`. On x86 (TSO), both are compiler
    /// barriers only (0 instructions). On ARM64, `fence(Acquire)`
    /// emits `dmb ishld` (load barrier) and `load(Acquire)` emits `ldar`.
    #[inline]
    pub fn try_load_consistent(&self) -> Option<(f64, f64, u64)> {
        let seq1 = self.seq.load(Ordering::Relaxed);

        if seq1 & 1 != 0 {
            return None; // write in progress
        }

        // ACQUIRE fence: prevents data loads from being speculated before seq1.
        // x86: compiler barrier only. ARM64: dmb ishld.
        fence(Ordering::Acquire);

        let value = f64::from_bits(self.value_bits.load(Ordering::Relaxed));
        let raw = f64::from_bits(self.raw_bits.load(Ordering::Relaxed));
        let ts = self.timestamp.load(Ordering::Relaxed);

        // Acquire load: ensures data loads above complete before reading seq2,
        // and pairs with writer's store(Release) for cross-thread visibility.
        let seq2 = self.seq.load(Ordering::Acquire);

        if seq1 == seq2 {
            Some((value, raw, ts))
        } else {
            None
        }
    }

    /// Unprotected read of all fields (no seqlock guarantee).
    ///
    /// Used only as a last-resort fallback after retries are exhausted.
    /// A leading `fence(Acquire)` provides best-effort freshness.
    #[inline]
    fn load_relaxed(&self) -> (f64, f64, u64) {
        fence(Ordering::Acquire);
        let value = f64::from_bits(self.value_bits.load(Ordering::Relaxed));
        let raw = f64::from_bits(self.raw_bits.load(Ordering::Relaxed));
        let ts = self.timestamp.load(Ordering::Relaxed);
        (value, raw, ts)
    }

    /// Set all point data with seqlock write protocol.
    ///
    /// Stores the sequence counter to odd (write-in-progress), writes all
    /// data fields, then stores to even (write-complete). Readers that
    /// observe an odd sequence or a changed sequence will retry.
    ///
    /// # Memory Ordering (store-release, AArch64-safe)
    ///
    /// Both seq stores use `Release` ordering:
    /// - Opening `store(odd, Release)` prevents data stores from being
    ///   reordered before it. x86: plain `mov`; ARM64: `stlr`.
    /// - Closing `store(even, Release)` prevents data stores from being
    ///   reordered after it. x86: plain `mov`; ARM64: `stlr`.
    ///
    /// # Single-writer assumption
    ///
    /// Uses `load` + `store` instead of `fetch_add` for the seq counter.
    /// Correct ONLY when a single thread writes to each slot. Guaranteed
    /// by SCADA architecture: comsrv owns telemetry slots, modsrv owns
    /// control slots. Concurrent `set()` on the same slot is UB.
    #[inline]
    pub fn set(&self, value: f64, raw: f64, timestamp: u64) {
        let s = self.seq.load(Ordering::Relaxed);

        // Begin write: seq → odd (signals write-in-progress).
        // Release: prevents subsequent data stores from being reordered
        // before this store. x86: plain mov. ARM64: stlr.
        self.seq.store(s.wrapping_add(1), Ordering::Release);

        // Data stores — Relaxed; ordering enforced by surrounding Release stores.
        self.value_bits.store(value.to_bits(), Ordering::Relaxed);
        self.raw_bits.store(raw.to_bits(), Ordering::Relaxed);
        self.timestamp.store(timestamp, Ordering::Relaxed);

        // End write: seq → even (signals write-complete).
        // Release: prevents prior data stores from being reordered after
        // this store. Pairs with reader's load(Acquire). x86: mov. ARM64: stlr.
        self.seq.store(s.wrapping_add(2), Ordering::Release);

        // Dirty flag — outside seqlock envelope (advisory, not consistency-critical).
        self.dirty.store(1, Ordering::Relaxed);
    }

    /// Check if dirty flag is set
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed) != 0
    }

    /// Clear the dirty flag
    #[inline]
    pub fn clear_dirty(&self) {
        self.dirty.store(0, Ordering::Relaxed);
    }

    /// Get raw seq counter value (for testing/debugging)
    #[inline]
    pub fn seq_raw(&self) -> u32 {
        self.seq.load(Ordering::Relaxed)
    }

    /// Force-set the sequence counter to an arbitrary value.
    ///
    /// **Test-only helper** — never call this in production code.
    /// Used to seed the seqlock counter near u32::MAX in integration tests so
    /// that wrapping arithmetic can be exercised without performing billions of
    /// writes.  Concurrent readers will see an inconsistent state until the
    /// next completed `set()` call.
    ///
    /// `#[doc(hidden)]` keeps this out of rustdoc while remaining callable from
    /// the integration-test binary (where `cfg(test)` is NOT active for
    /// dependencies, so a bare `#[cfg(test)]` would hide the symbol).
    #[doc(hidden)]
    pub fn set_seq_for_testing(&self, val: u32) {
        self.seq.store(val, Ordering::Relaxed);
    }
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
        // Verify the sequence counter increments by 2 per write
        let slot = PointSlot::new();

        assert_eq!(slot.seq_raw(), 0, "initial sequence should be 0");
        assert_eq!(slot.seq_raw() & 1, 0, "initial sequence should be even");

        slot.set(1.0, 1.0, 100);
        assert_eq!(slot.seq_raw(), 2, "after 1 write, sequence should be 2");

        slot.set(2.0, 2.0, 200);
        assert_eq!(slot.seq_raw(), 4, "after 2 writes, sequence should be 4");

        slot.set(3.0, 3.0, 300);
        assert_eq!(slot.seq_raw(), 6, "after 3 writes, sequence should be 6");
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
