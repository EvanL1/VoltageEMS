//! Tests that dirty flag does NOT interfere with seqlock consistency.
//!
//! H4: fetch_or(1, Relaxed) after the seqlock even-write creates a window
//! where a reader sees an odd sequence (dirty OR'd into seq bits), triggering
//! a spurious retry.

#[test]
fn test_dirty_flag_does_not_corrupt_seqlock_sequence() {
    use voltage_rtdb_shm::PointSlot;

    let slot = PointSlot::new();

    // Write a value (this sets dirty + advances seqlock)
    slot.set(42.0, 420.0, 1000);

    // After set(), the sequence counter (upper 32 bits of flags) should be EVEN,
    // indicating write-complete. The dirty flag (bit 0) should NOT make the
    // sequence appear odd to a reader checking `flags >> 32`.
    let flags = slot.flags_raw();
    let seq = flags >> 32;
    assert_eq!(
        seq % 2,
        0,
        "Sequence should be even after write, got seq={}",
        seq
    );

    // The dirty bit should be separate and not affect sequence
    assert!(slot.is_dirty(), "Dirty flag should be set after write");

    // Do 100 writes and verify sequence stays even after each
    for i in 0..100u64 {
        slot.set(i as f64, i as f64 * 10.0, 2000 + i);
        let flags = slot.flags_raw();
        let seq = flags >> 32;
        assert_eq!(
            seq % 2,
            0,
            "Sequence should be even after write #{}, got seq={}",
            i,
            seq
        );
    }
}

#[test]
fn test_dirty_flag_independent_of_seqlock() {
    use voltage_rtdb_shm::PointSlot;

    let slot = PointSlot::new();

    // Initially: not dirty, seq=0
    assert!(!slot.is_dirty());

    // Write sets dirty
    slot.set(1.0, 1.0, 100);
    assert!(slot.is_dirty());

    // Clear dirty — should NOT affect seqlock
    slot.clear_dirty();
    assert!(!slot.is_dirty());

    // Sequence should be 2 (one write = +2 increments)
    let flags = slot.flags_raw();
    let seq = flags >> 32;
    assert_eq!(seq, 2, "Seq should be 2 after one write cycle");
}

#[test]
fn test_seqlock_read_returns_consistent_data() {
    use voltage_rtdb_shm::PointSlot;

    let slot = PointSlot::new();

    // Write multiple times with distinct values
    for i in 0..50u64 {
        slot.set(i as f64, i as f64 * 2.0, 1000 + i);
    }

    // Read should return the last written value consistently
    let (value, raw, ts) = slot.load_consistent();
    assert!((value - 49.0).abs() < 0.001);
    assert!((raw - 98.0).abs() < 0.001);
    assert_eq!(ts, 1049);

    // Sequence should be 100 (50 writes × 2 increments each)
    let flags = slot.flags_raw();
    let seq = flags >> 32;
    assert_eq!(seq, 100);
}
