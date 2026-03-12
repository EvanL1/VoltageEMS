//! Tests for seqlock with split seq/dirty fields.
//!
//! Validates that seq counter and dirty flag are independent
//! (no shared AtomicU64, no fetch_add/fetch_or interference).

#[test]
fn test_seq_increments_by_2_per_write() {
    use voltage_rtdb_shm::PointSlot;

    let slot = PointSlot::new();
    assert_eq!(slot.seq_raw(), 0);

    slot.set(1.0, 1.0, 100);
    assert_eq!(slot.seq_raw(), 2, "1 write = seq 2");

    slot.set(2.0, 2.0, 200);
    assert_eq!(slot.seq_raw(), 4, "2 writes = seq 4");
}

#[test]
fn test_seq_always_even_after_write() {
    use voltage_rtdb_shm::PointSlot;

    let slot = PointSlot::new();

    for i in 0..100u64 {
        slot.set(i as f64, i as f64 * 10.0, 1000 + i);
        let seq = slot.seq_raw();
        assert_eq!(seq % 2, 0, "seq should be even after write #{i}, got {seq}");
    }
}

#[test]
fn test_dirty_independent_of_seq() {
    use voltage_rtdb_shm::PointSlot;

    let slot = PointSlot::new();
    assert!(!slot.is_dirty());

    slot.set(1.0, 1.0, 100);
    assert!(slot.is_dirty());
    assert_eq!(slot.seq_raw(), 2);

    // clear_dirty must NOT affect seq
    slot.clear_dirty();
    assert!(!slot.is_dirty());
    assert_eq!(slot.seq_raw(), 2, "clear_dirty must not change seq");
}

#[test]
fn test_consistent_read_after_split() {
    use voltage_rtdb_shm::PointSlot;

    let slot = PointSlot::new();

    for i in 0..50u64 {
        slot.set(i as f64, i as f64 * 2.0, 1000 + i);
    }

    let (value, raw, ts) = slot.load_consistent();
    assert!((value - 49.0).abs() < 0.001);
    assert!((raw - 98.0).abs() < 0.001);
    assert_eq!(ts, 1049);
    assert_eq!(slot.seq_raw(), 100); // 50 writes × 2
}
