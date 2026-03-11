//! Tests for ring buffer memory ordering and correctness.
//!
//! H1: push() uses Relaxed fetch_add for head before write_volatile — ARM64 unsafe.
//! H5: open_readonly uses MmapMut (read+write) instead of read-only Mmap.
//! H6: push_batch is serial loop — no batch head reservation.

use tempfile::tempdir;
use voltage_rtdb_shm::ring_buffer::{DataPoint, SharedRingBuffer};

fn make_point(id: u32, value: f64, ts: u64) -> DataPoint {
    DataPoint {
        timestamp_us: ts,
        value,
        channel_id: 1001,
        instance_id: 0,
        point_id: id,
        point_type: 0,
        quality: 0,
        _reserved: [0; 2],
    }
}

// ============================================================================
// H1: push ordering tests
// ============================================================================

#[test]
fn test_push_then_read_returns_correct_data() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_ordering.ring");

    let mut buffer = SharedRingBuffer::create_or_open(&path, 1024, 60_000_000).unwrap();

    // Write 100 points
    for i in 0..100u32 {
        buffer.push(make_point(i, i as f64 * 1.5, 1000 + i as u64));
    }

    // Read back — all data should be consistent
    let points = buffer.buffer().read_range(1000, 1099);
    assert_eq!(points.len(), 100, "Should read back all 100 points");

    for p in &points {
        let expected_value = p.point_id as f64 * 1.5;
        assert!(
            (p.value - expected_value).abs() < 0.001,
            "Point {} value mismatch: {} vs {}",
            p.point_id,
            p.value,
            expected_value
        );
    }
}

#[test]
fn test_push_total_writes_matches_head() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_writes.ring");

    let mut buffer = SharedRingBuffer::create_or_open(&path, 1024, 60_000_000).unwrap();

    for i in 0..50 {
        buffer.push(make_point(i, i as f64, 1000 + i as u64));
    }

    // After the fix, total_writes should exactly match push count
    assert_eq!(buffer.buffer().total_writes(), 50);
    assert_eq!(buffer.buffer().head(), 50);
}

// ============================================================================
// H5: open_readonly tests
// ============================================================================

#[test]
fn test_open_readonly_reads_data() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_readonly.ring");

    // Create and write data
    {
        let mut buffer = SharedRingBuffer::create_or_open(&path, 256, 60_000_000).unwrap();
        buffer.push(make_point(0, 42.0, 1000));
    }

    // Open read-only and verify data is accessible
    let reader = SharedRingBuffer::open_readonly(&path).unwrap();
    let points = reader.buffer().read_range(1000, 1000);
    assert_eq!(points.len(), 1);
    assert!((points[0].value - 42.0).abs() < 0.001);
}

// ============================================================================
// H6: push_batch tests
// ============================================================================

#[test]
fn test_push_batch_writes_all_points() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_batch.ring");

    let mut buffer = SharedRingBuffer::create_or_open(&path, 1024, 60_000_000).unwrap();

    let points: Vec<DataPoint> = (0..50)
        .map(|i| make_point(i, i as f64 * 3.0, 2000 + i as u64))
        .collect();

    buffer.push_batch(&points);

    assert_eq!(buffer.buffer().total_writes(), 50);
    assert_eq!(buffer.buffer().head(), 50);

    let read_back = buffer.buffer().read_range(2000, 2049);
    assert_eq!(read_back.len(), 50);
}

#[test]
fn test_push_batch_contiguous_slots() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_batch_contiguous.ring");

    let mut buffer = SharedRingBuffer::create_or_open(&path, 1024, 60_000_000).unwrap();

    // Write a batch — all points should occupy contiguous slots
    let points: Vec<DataPoint> = (0..10)
        .map(|i| make_point(i, i as f64, 5000 + i as u64))
        .collect();

    let head_before = buffer.buffer().head();
    buffer.push_batch(&points);
    let head_after = buffer.buffer().head();

    // Head should advance by exactly the batch size
    assert_eq!(head_after - head_before, 10);
}

#[test]
fn test_push_batch_empty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_batch_empty.ring");

    let mut buffer = SharedRingBuffer::create_or_open(&path, 256, 60_000_000).unwrap();

    // Empty batch should be no-op
    buffer.push_batch(&[]);
    assert_eq!(buffer.buffer().head(), 0);
    assert_eq!(buffer.buffer().total_writes(), 0);
}
