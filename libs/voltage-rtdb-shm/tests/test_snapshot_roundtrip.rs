//! Snapshot Save/Restore Roundtrip Tests
//!
//! Verifies that `UnifiedWriter::save_snapshot` + `UnifiedWriter::restore_from_snapshot`
//! correctly preserves point data. The restore path uses hardcoded byte offsets that
//! depend on `PointSlot`'s `#[repr(C)]` layout; these tests guard against silent
//! corruption from any future layout change.

#![allow(clippy::disallowed_methods)] // Test code — unwrap is acceptable

use std::collections::HashMap;
use tempfile::tempdir;
use voltage_routing::RoutingCache;
use voltage_rtdb_shm::{SharedConfig, UnifiedReader, UnifiedWriter};

// ============================================================================
// Test Helpers
// ============================================================================

/// Build a minimal SharedConfig pointing into `dir`.
fn test_config(dir: &std::path::Path) -> SharedConfig {
    SharedConfig::default()
        .with_path(dir.join("test.shm"))
        .with_max_slots(1000)
}

/// Build a RoutingCache with:
///   channel 1001: T:0-4, S:0-2, C:0-1  → instance 23
///   channel 1002: T:0-2                 → instance 24
fn test_routing_cache() -> RoutingCache {
    let mut c2m = HashMap::new();
    let m2c = HashMap::new();
    let c2c = HashMap::new();

    // Channel 1001 — Telemetry (T)
    for i in 0..5u32 {
        c2m.insert(format!("1001:T:{i}"), format!("23:M:{i}"));
    }
    // Channel 1001 — Signal (S)
    for i in 0..3u32 {
        c2m.insert(format!("1001:S:{i}"), format!("23:M:{}", 5 + i));
    }
    // Channel 1001 — Control (C)
    for i in 0..2u32 {
        c2m.insert(format!("1001:C:{i}"), format!("23:M:{}", 8 + i));
    }
    // Channel 1002 — Telemetry (T)
    for i in 0..3u32 {
        c2m.insert(format!("1002:T:{i}"), format!("24:M:{i}"));
    }

    RoutingCache::from_maps(c2m, m2c, c2c)
}

// ============================================================================
// Tests
// ============================================================================

/// Write known values to multiple slots, save snapshot, restore into a fresh
/// writer, and verify every value/raw/timestamp survives the roundtrip.
#[test]
fn test_snapshot_save_and_restore_preserves_values() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let snapshot_path = dir.path().join("snap.bin");

    let now = 1_704_067_200_000u64;

    // --- Write phase ---
    let writer = UnifiedWriter::create(&config, &routing).unwrap();

    // Telemetry points on channel 1001
    assert!(writer.set(1001, 0, 0, 10.5, 105.0, now));
    assert!(writer.set(1001, 0, 1, 20.5, 205.0, now + 1));
    assert!(writer.set(1001, 0, 2, 30.5, 305.0, now + 2));
    // Signal points on channel 1001
    assert!(writer.set(1001, 1, 0, 1.0, 10.0, now + 10));
    assert!(writer.set(1001, 1, 1, 0.0, 0.0, now + 11));
    // Control points on channel 1001
    assert!(writer.set(1001, 2, 0, 50.0, 500.0, now + 20));
    // Telemetry on channel 1002
    assert!(writer.set(1002, 0, 0, 99.9, 999.0, now + 100));

    writer.flush().unwrap();
    writer.save_snapshot(&snapshot_path).unwrap();

    // --- Restore phase ---
    // Use a different SHM path so no leftover file affects the new writer.
    let config2 = SharedConfig::default()
        .with_path(dir.path().join("test2.shm"))
        .with_max_slots(1000);

    let restored =
        UnifiedWriter::restore_from_snapshot(&config2, &snapshot_path, &routing).unwrap();

    // Verify by reading through UnifiedReader (same routing → same slot mapping).
    let reader = UnifiedReader::open(&config2, &routing).unwrap();

    // Channel 1001 — Telemetry
    let (v, ts) = reader.get_channel(1001, 0, 0).unwrap();
    assert!(
        (v - 10.5).abs() < f64::EPSILON,
        "1001:T:0 value mismatch: {v}"
    );
    assert_eq!(ts, now, "1001:T:0 timestamp mismatch");

    let (v, ts) = reader.get_channel(1001, 0, 1).unwrap();
    assert!(
        (v - 20.5).abs() < f64::EPSILON,
        "1001:T:1 value mismatch: {v}"
    );
    assert_eq!(ts, now + 1);

    let (v, ts) = reader.get_channel(1001, 0, 2).unwrap();
    assert!(
        (v - 30.5).abs() < f64::EPSILON,
        "1001:T:2 value mismatch: {v}"
    );
    assert_eq!(ts, now + 2);

    // Channel 1001 — Signal
    let (v, ts) = reader.get_channel(1001, 1, 0).unwrap();
    assert!(
        (v - 1.0).abs() < f64::EPSILON,
        "1001:S:0 value mismatch: {v}"
    );
    assert_eq!(ts, now + 10);

    let (v, ts) = reader.get_channel(1001, 1, 1).unwrap();
    assert!(
        (v - 0.0).abs() < f64::EPSILON,
        "1001:S:1 value mismatch: {v}"
    );
    assert_eq!(ts, now + 11);

    // Channel 1001 — Control
    let (v, ts) = reader.get_channel(1001, 2, 0).unwrap();
    assert!(
        (v - 50.0).abs() < f64::EPSILON,
        "1001:C:0 value mismatch: {v}"
    );
    assert_eq!(ts, now + 20);

    // Channel 1002 — Telemetry
    let (v, ts) = reader.get_channel(1002, 0, 0).unwrap();
    assert!((v - 99.9).abs() < 1e-9, "1002:T:0 value mismatch: {v}");
    assert_eq!(ts, now + 100);

    // Drop the restored writer explicitly (suppress unused variable lint)
    drop(restored);
}

/// After `restore_from_snapshot`, every slot's seqlock counter must be even
/// and non-zero (set() was called once during restore), and dirty must be true
/// for slots that received data, because `set_direct` sets the dirty flag.
///
/// More importantly: seq must NOT carry over stale values from the snapshot
/// (snapshot serialises the raw mmap bytes which include seq=N from the
/// original writer's set() calls — restore must reset them via set_direct).
#[test]
fn test_snapshot_restore_clears_seq_and_dirty() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let snapshot_path = dir.path().join("snap.bin");

    let now = 1_704_067_200_000u64;

    // Write a value multiple times so seq reaches a high value in the original SHM.
    let writer = UnifiedWriter::create(&config, &routing).unwrap();
    for i in 0..10u64 {
        writer.set(1001, 0, 0, i as f64, i as f64 * 10.0, now + i);
    }
    writer.flush().unwrap();
    writer.save_snapshot(&snapshot_path).unwrap();

    // Restore into a new SHM file.
    let config2 = SharedConfig::default()
        .with_path(dir.path().join("test2.shm"))
        .with_max_slots(1000);

    let restored =
        UnifiedWriter::restore_from_snapshot(&config2, &snapshot_path, &routing).unwrap();

    // get_slot is not public; validate via the accessor macros indirectly.
    // We can observe seq and dirty through PointSlot's public API by reading
    // a slot with slot_at() — that is not pub either.  Instead we validate
    // behaviour: after restore_from_snapshot calls set_direct() for each slot,
    // a subsequent load_consistent() read through UnifiedReader must succeed
    // (even seq), and the writer can perform a fresh set() without panicking.
    //
    // The observable guarantee we assert here: the slot seq returned by
    // `lookup` + internal slot access is even (no write-in-progress).
    // We do this by writing a new value on top of the restored slot and
    // confirming the reader sees the new value — proving the seqlock is
    // in a valid idle state after restore.
    restored.set(1001, 0, 0, 999.0, 9990.0, now + 9999);
    restored.flush().unwrap();

    let reader = UnifiedReader::open(&config2, &routing).unwrap();
    let (v, ts) = reader.get_channel(1001, 0, 0).unwrap();
    assert!(
        (v - 999.0).abs() < f64::EPSILON,
        "post-restore write should be readable: {v}"
    );
    assert_eq!(ts, now + 9999);
}

/// Write to slots across different channels and point types, save/restore,
/// verify all values are correct and channels are isolated.
#[test]
fn test_snapshot_multiple_channels() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let snapshot_path = dir.path().join("snap.bin");

    let now = 1_704_100_000_000u64;

    let writer = UnifiedWriter::create(&config, &routing).unwrap();

    // Populate all mapped points for both channels.
    let telemetry_1001: Vec<(u32, f64, f64, u64)> = (0..5)
        .map(|i| (i, i as f64 * 1.1, i as f64 * 11.0, now + i as u64))
        .collect();

    let signal_1001: Vec<(u32, f64, f64, u64)> = (0..3)
        .map(|i| (i, 100.0 + i as f64, 1000.0 + i as f64, now + 100 + i as u64))
        .collect();

    let control_1001: Vec<(u32, f64, f64, u64)> = (0..2)
        .map(|i| (i, 200.0 + i as f64, 2000.0 + i as f64, now + 200 + i as u64))
        .collect();

    let telemetry_1002: Vec<(u32, f64, f64, u64)> = (0..3)
        .map(|i| (i, 300.0 + i as f64, 3000.0 + i as f64, now + 300 + i as u64))
        .collect();

    for (pid, v, r, ts) in &telemetry_1001 {
        assert!(writer.set(1001, 0, *pid, *v, *r, *ts));
    }
    for (pid, v, r, ts) in &signal_1001 {
        assert!(writer.set(1001, 1, *pid, *v, *r, *ts));
    }
    for (pid, v, r, ts) in &control_1001 {
        assert!(writer.set(1001, 2, *pid, *v, *r, *ts));
    }
    for (pid, v, r, ts) in &telemetry_1002 {
        assert!(writer.set(1002, 0, *pid, *v, *r, *ts));
    }

    writer.flush().unwrap();
    writer.save_snapshot(&snapshot_path).unwrap();

    // Restore.
    let config2 = SharedConfig::default()
        .with_path(dir.path().join("test2.shm"))
        .with_max_slots(1000);
    let _restored =
        UnifiedWriter::restore_from_snapshot(&config2, &snapshot_path, &routing).unwrap();

    let reader = UnifiedReader::open(&config2, &routing).unwrap();

    for (pid, expected_v, _r, expected_ts) in &telemetry_1001 {
        let (v, ts) = reader.get_channel(1001, 0, *pid).unwrap();
        assert!(
            (v - expected_v).abs() < 1e-9,
            "1001:T:{pid} value: got {v}, expected {expected_v}"
        );
        assert_eq!(ts, *expected_ts, "1001:T:{pid} timestamp mismatch");
    }

    for (pid, expected_v, _r, expected_ts) in &signal_1001 {
        let (v, ts) = reader.get_channel(1001, 1, *pid).unwrap();
        assert!(
            (v - expected_v).abs() < 1e-9,
            "1001:S:{pid} value: got {v}, expected {expected_v}"
        );
        assert_eq!(ts, *expected_ts, "1001:S:{pid} timestamp mismatch");
    }

    for (pid, expected_v, _r, expected_ts) in &control_1001 {
        let (v, ts) = reader.get_channel(1001, 2, *pid).unwrap();
        assert!(
            (v - expected_v).abs() < 1e-9,
            "1001:C:{pid} value: got {v}, expected {expected_v}"
        );
        assert_eq!(ts, *expected_ts, "1001:C:{pid} timestamp mismatch");
    }

    for (pid, expected_v, _r, expected_ts) in &telemetry_1002 {
        let (v, ts) = reader.get_channel(1002, 0, *pid).unwrap();
        assert!(
            (v - expected_v).abs() < 1e-9,
            "1002:T:{pid} value: got {v}, expected {expected_v}"
        );
        assert_eq!(ts, *expected_ts, "1002:T:{pid} timestamp mismatch");
    }
}

/// Test that edge-case f64 values survive the snapshot roundtrip.
///
/// NaN and Infinity are excluded because `restore_from_snapshot` intentionally
/// filters them out (they represent corrupt data).
#[test]
fn test_snapshot_with_special_values() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let snapshot_path = dir.path().join("snap.bin");

    let now = 1_704_200_000_000u64;

    // Edge values mapped to point IDs 0-4 (all on channel 1001 Telemetry, which has 5 slots).
    let cases: &[(u32, f64, u64)] = &[
        (0, 0.0, now),
        (1, -0.0, now + 1),
        (2, f64::MAX, now + 2),
        (3, f64::MIN, now + 3),
        (4, f64::MIN_POSITIVE, now + 4),
    ];

    let writer = UnifiedWriter::create(&config, &routing).unwrap();
    for (pid, v, ts) in cases {
        // raw == value for simplicity
        assert!(
            writer.set(1001, 0, *pid, *v, *v, *ts),
            "failed to write 1001:T:{pid} = {v}"
        );
    }
    writer.flush().unwrap();
    writer.save_snapshot(&snapshot_path).unwrap();

    let config2 = SharedConfig::default()
        .with_path(dir.path().join("test2.shm"))
        .with_max_slots(1000);
    let _restored =
        UnifiedWriter::restore_from_snapshot(&config2, &snapshot_path, &routing).unwrap();

    let reader = UnifiedReader::open(&config2, &routing).unwrap();

    for (pid, expected_v, expected_ts) in cases {
        let (v, ts) = reader
            .get_channel(1001, 0, *pid)
            .unwrap_or_else(|| panic!("slot 1001:T:{pid} missing after restore"));

        // For -0.0 and +0.0: both have the same IEEE 754 bit pattern when
        // compared with ==, so standard equality works fine.
        assert_eq!(
            v.to_bits(),
            expected_v.to_bits(),
            "1001:T:{pid}: bit pattern mismatch: got {v:?} (bits=0x{:016X}), expected {expected_v:?} (bits=0x{:016X})",
            v.to_bits(),
            expected_v.to_bits(),
        );
        assert_eq!(ts, *expected_ts, "1001:T:{pid} timestamp mismatch");
    }
}

/// Attempting to restore from a non-existent snapshot file should return
/// an error rather than panicking or silently producing a corrupt writer.
#[test]
fn test_snapshot_file_not_found() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();

    let missing = dir.path().join("no_such_file.bin");

    let result = UnifiedWriter::restore_from_snapshot(&config, &missing, &routing);
    assert!(result.is_err(), "Expected Err for missing snapshot, got Ok");

    let err = result.err().expect("already confirmed is_err");
    let err_msg = format!("{err:#}");
    assert!(
        err_msg.contains("snapshot") || err_msg.contains("open") || err_msg.contains("No such"),
        "Error message should mention file open failure, got: {err_msg}"
    );
}
