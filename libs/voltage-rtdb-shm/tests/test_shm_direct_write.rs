//! SharedMemory Direct Write Tests
//!
//! Tests for direct SharedMemory write path:
//! - UnifiedWriter write operations
//! - ChannelToSlotIndex lookup
//! - Protocol → SharedMemory integration

#![allow(clippy::disallowed_methods)]

use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;
use voltage_routing::RoutingCache;
use voltage_rtdb_shm::{ChannelToSlotIndex, SharedConfig, UnifiedReader, UnifiedWriter};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create test SharedConfig
fn test_config(dir: &std::path::Path) -> SharedConfig {
    SharedConfig::default()
        .with_path(dir.join("test.shm"))
        .with_max_slots(1000)
}

/// Create test RoutingCache with predefined channels
fn test_routing_cache() -> RoutingCache {
    let mut c2m = HashMap::new();
    let m2c = HashMap::new();
    let c2c = HashMap::new();

    // Channel 1001: T:0-9, S:0-4, C:0-2 → instance 23
    for i in 0..10 {
        c2m.insert(format!("1001:T:{}", i), format!("23:M:{}", i));
    }
    for i in 0..5 {
        c2m.insert(format!("1001:S:{}", i), format!("23:M:{}", 10 + i));
    }
    for i in 0..3 {
        c2m.insert(format!("1001:C:{}", i), format!("23:M:{}", 15 + i));
    }

    // Channel 1002: T:0-4, S:0-2 → instance 24
    for i in 0..5 {
        c2m.insert(format!("1002:T:{}", i), format!("24:M:{}", i));
    }
    for i in 0..3 {
        c2m.insert(format!("1002:S:{}", i), format!("24:M:{}", 5 + i));
    }

    RoutingCache::from_maps(c2m, m2c, c2c)
}

// ============================================================================
// Basic Write Tests
// ============================================================================

#[test]
fn test_shm_write_single_point() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();

    let writer = UnifiedWriter::create(&config, &routing).unwrap();
    let now = 1704067200000u64;

    // Write value: channel 1001, T:0
    assert!(writer.set(1001, 0, 0, 123.456, 1234.0, now));

    writer.flush().unwrap();

    // Read back and verify
    let reader = UnifiedReader::open(&config, &routing).unwrap();
    let (value, ts) = reader.get_channel(1001, 0, 0).unwrap();

    assert!((value - 123.456).abs() < 0.001);
    assert_eq!(ts, now);
}

#[test]
fn test_shm_write_all_point_types() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let now = 1704067200000u64;

    let writer = UnifiedWriter::create(&config, &routing).unwrap();

    // Write all point types for channel 1001
    // T=0 (Telemetry), S=1 (Signal), C=2 (Control)
    let test_cases = [
        (0u8, 0u32, 100.0), // T:0
        (1u8, 0u32, 1.0),   // S:0
        (2u8, 0u32, 50.0),  // C:0
    ];

    for (pt, pid, value) in test_cases {
        assert!(writer.set(1001, pt, pid, value, value, now));
    }

    writer.flush().unwrap();

    // Verify all
    let reader = UnifiedReader::open(&config, &routing).unwrap();

    for (pt, pid, expected_value) in test_cases {
        let (value, _) = reader.get_channel(1001, pt, pid).unwrap();
        assert!(
            (value - expected_value).abs() < 0.001,
            "type={} point={} value mismatch: {} vs {}",
            pt,
            pid,
            value,
            expected_value
        );
    }
}

#[test]
fn test_shm_write_batch() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let now = 1704067200000u64;

    let writer = UnifiedWriter::create(&config, &routing).unwrap();

    // Write 10 telemetry points
    for pid in 0..10 {
        assert!(writer.set(1001, 0, pid, pid as f64 * 10.0, pid as f64 * 100.0, now));
    }

    writer.flush().unwrap();

    // Verify all
    let reader = UnifiedReader::open(&config, &routing).unwrap();

    for pid in 0..10 {
        let (value, _) = reader.get_channel(1001, 0, pid).unwrap();
        assert!((value - pid as f64 * 10.0).abs() < 0.001);
    }
}

// ============================================================================
// ChannelToSlotIndex Tests
// ============================================================================

#[test]
fn test_channel_to_slot_index_lookup() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();

    let writer = UnifiedWriter::create(&config, &routing).unwrap();
    let index = ChannelToSlotIndex::from_unified_writer(&writer);

    // Verify mapped points exist
    assert!(index
        .lookup(1001, voltage_model::PointType::Telemetry, 0)
        .is_some());
    assert!(index
        .lookup(1001, voltage_model::PointType::Signal, 0)
        .is_some());
    assert!(index
        .lookup(1001, voltage_model::PointType::Control, 0)
        .is_some());
    assert!(index
        .lookup(1002, voltage_model::PointType::Telemetry, 0)
        .is_some());

    // Verify unmapped points return None
    assert!(index
        .lookup(9999, voltage_model::PointType::Telemetry, 0)
        .is_none()); // Unknown channel
    assert!(index
        .lookup(1001, voltage_model::PointType::Telemetry, 100)
        .is_none()); // Out of range
}

#[test]
fn test_channel_to_slot_index_direct_write() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let now = 1704067200000u64;

    let writer = Arc::new(UnifiedWriter::create(&config, &routing).unwrap());
    let index = ChannelToSlotIndex::from_unified_writer(&writer);

    // Get slot offset and write directly
    let _slot_offset = index
        .lookup(1001, voltage_model::PointType::Telemetry, 0)
        .unwrap();

    // Verify we can use writer.lookup to get slot index
    let slot = writer.lookup(1001, 0, 0).unwrap();
    writer.set_direct(slot, 42.0, 420.0, now);

    writer.flush().unwrap();

    // Verify
    let reader = UnifiedReader::open(&config, &routing).unwrap();
    let (value, _) = reader.get_channel(1001, 0, 0).unwrap();
    assert!((value - 42.0).abs() < 0.001);
}

// ============================================================================
// Timestamp Tests
// ============================================================================

#[test]
fn test_shm_timestamp_update() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();

    let writer = UnifiedWriter::create(&config, &routing).unwrap();

    // Write with increasing timestamps
    let ts1 = 1704067200000u64;
    let ts2 = 1704067201000u64;
    let ts3 = 1704067202000u64;

    writer.set(1001, 0, 0, 100.0, 100.0, ts1);
    writer.flush().unwrap();

    let reader = UnifiedReader::open(&config, &routing).unwrap();
    let (_, ts) = reader.get_channel(1001, 0, 0).unwrap();
    assert_eq!(ts, ts1);

    writer.set(1001, 0, 0, 200.0, 200.0, ts2);
    let (_, ts) = reader.get_channel(1001, 0, 0).unwrap();
    assert_eq!(ts, ts2);

    writer.set(1001, 0, 0, 300.0, 300.0, ts3);
    let (_, ts) = reader.get_channel(1001, 0, 0).unwrap();
    assert_eq!(ts, ts3);
}

// ============================================================================
// Multi-Channel Tests
// ============================================================================

#[test]
fn test_shm_multi_channel_write() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let now = 1704067200000u64;

    let writer = UnifiedWriter::create(&config, &routing).unwrap();

    // Write to both channels
    assert!(writer.set(1001, 0, 0, 111.0, 111.0, now));
    assert!(writer.set(1002, 0, 0, 222.0, 222.0, now));

    writer.flush().unwrap();

    // Verify isolation
    let reader = UnifiedReader::open(&config, &routing).unwrap();

    let (v1, _) = reader.get_channel(1001, 0, 0).unwrap();
    let (v2, _) = reader.get_channel(1002, 0, 0).unwrap();

    assert!((v1 - 111.0).abs() < 0.001);
    assert!((v2 - 222.0).abs() < 0.001);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_shm_write_unmapped_returns_false() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let now = 1704067200000u64;

    let writer = UnifiedWriter::create(&config, &routing).unwrap();

    // Unknown channel
    assert!(!writer.set(9999, 0, 0, 100.0, 100.0, now));

    // Point ID out of range
    assert!(!writer.set(1001, 0, 100, 100.0, 100.0, now));
}

#[test]
fn test_shm_reader_unmapped_returns_none() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();

    let writer = UnifiedWriter::create(&config, &routing).unwrap();
    writer.flush().unwrap();

    let reader = UnifiedReader::open(&config, &routing).unwrap();

    // Unknown channel
    assert!(reader.get_channel(9999, 0, 0).is_none());

    // Point ID out of range
    assert!(reader.get_channel(1001, 0, 100).is_none());
}

// ============================================================================
// Performance Hint Tests
// ============================================================================

#[test]
fn test_shm_direct_write_performance() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());
    let routing = test_routing_cache();
    let now = 1704067200000u64;

    let writer = UnifiedWriter::create(&config, &routing).unwrap();

    // Pre-lookup slot for hot path
    let slot = writer.lookup(1001, 0, 0).unwrap();

    // Direct write (no lookup overhead)
    for i in 0..1000 {
        writer.set_direct(slot, i as f64, i as f64, now + i);
    }

    writer.flush().unwrap();

    // Verify last value
    let reader = UnifiedReader::open(&config, &routing).unwrap();
    let (value, ts) = reader.get_channel(1001, 0, 0).unwrap();
    assert!((value - 999.0).abs() < 0.001);
    assert_eq!(ts, now + 999);
}
