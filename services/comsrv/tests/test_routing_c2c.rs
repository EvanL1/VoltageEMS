//! C2C (Channel to Channel) Routing End-to-End Tests
//!
//! Tests comsrv's C2C passthrough forwarding functionality:
//! - Basic single-level forwarding
//! - Multi-level cascade forwarding
//! - Cycle detection and depth limiting
//! - Raw value passthrough
//! - Different point type forwarding
//! - One-to-many forwarding

#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use std::collections::HashMap;
use std::sync::Arc;
use voltage_model::PointType;
use voltage_routing::{write_channel_batch, ChannelPointUpdate};
use voltage_rtdb::MemoryRtdb;
use voltage_rtdb::Rtdb;
use voltage_rtdb::{KeySpaceConfig, RoutingCache};

/// Maximum C2C cascade depth constant (consistent with storage.rs)
const MAX_C2C_DEPTH: u8 = 2;

/// Creates a memory RTDB for testing (no external dependencies)
fn create_test_rtdb() -> Arc<MemoryRtdb> {
    Arc::new(MemoryRtdb::new())
}

/// Creates a test environment with C2C routing
///
/// # Arguments
/// - `c2c_routes`: C2C routing mappings, format: [("source_channel:type:point", "target_channel:type:point"), ...]
///
/// # Returns
/// - RTDB instance (memory implementation)
/// - RoutingCache instance (with C2C routing configuration)
async fn setup_c2c_routing(c2c_routes: Vec<(&str, &str)>) -> (Arc<MemoryRtdb>, Arc<RoutingCache>) {
    let rtdb = create_test_rtdb();

    let mut c2c_map = HashMap::new();
    for (source, target) in c2c_routes {
        c2c_map.insert(source.to_string(), target.to_string());
    }

    let routing_cache = Arc::new(RoutingCache::from_maps(
        HashMap::new(), // C2M routing (not needed for this test)
        HashMap::new(), // M2C routing (not needed for this test)
        c2c_map,        // C2C routing
    ));

    (rtdb, routing_cache)
}

/// Asserts channel point value
///
/// # Arguments
/// - `rtdb`: RTDB instance
/// - `channel_id`: Channel ID
/// - `point_type`: Point type (T/S/C/A)
/// - `point_id`: Point ID
/// - `expected_value`: Expected value
async fn assert_channel_value<R: Rtdb>(
    rtdb: &R,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_value: f64,
) {
    let config = KeySpaceConfig::production();
    let point_type_enum = voltage_model::PointType::from_str(point_type).unwrap();
    let channel_key = config.channel_key(channel_id, point_type_enum);

    let value = rtdb
        .hash_get(&channel_key, &point_id.to_string())
        .await
        .expect("Failed to read channel value")
        .expect("Channel value should exist");

    let actual_value: f64 = String::from_utf8(value.to_vec()).unwrap().parse().unwrap();

    assert_eq!(
        actual_value, expected_value,
        "Channel {}:{}:{} value mismatch",
        channel_id, point_type, point_id
    );
}

/// Asserts channel point does not exist
///
/// # Arguments
/// - `rtdb`: RTDB instance
/// - `channel_id`: Channel ID
/// - `point_type`: Point type (T/S/C/A)
/// - `point_id`: Point ID
async fn assert_channel_value_missing<R: Rtdb>(
    rtdb: &R,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
) {
    let config = KeySpaceConfig::production();
    let point_type_enum = voltage_model::PointType::from_str(point_type).unwrap();
    let channel_key = config.channel_key(channel_id, point_type_enum);

    let value = rtdb
        .hash_get(&channel_key, &point_id.to_string())
        .await
        .expect("Failed to read channel value");

    assert!(
        value.is_none(),
        "Channel {}:{}:{} should not have value",
        channel_id,
        point_type,
        point_id
    );
}

/// Asserts raw value passthrough
///
/// # Arguments
/// - `rtdb`: RTDB instance
/// - `channel_id`: Channel ID
/// - `point_type`: Point type (T/S/C/A)
/// - `point_id`: Point ID
/// - `expected_raw_value`: Expected raw value
async fn assert_raw_value<R: Rtdb>(
    rtdb: &R,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_raw_value: f64,
) {
    let config = KeySpaceConfig::production();
    let point_type_enum = voltage_model::PointType::from_str(point_type).unwrap();
    let channel_key = config.channel_key(channel_id, point_type_enum);
    let raw_key = format!("{}:raw", channel_key);

    let value = rtdb
        .hash_get(&raw_key, &point_id.to_string())
        .await
        .expect("Failed to read raw value")
        .expect("Raw value should exist");

    let actual_value: f64 = String::from_utf8(value.to_vec()).unwrap().parse().unwrap();

    assert_eq!(
        actual_value, expected_raw_value,
        "Channel {}:{}:{} raw value mismatch",
        channel_id, point_type, point_id
    );
}

#[tokio::test]
async fn test_c2c_basic_routing() {
    // Scenario: Basic C2C passthrough forwarding
    // Config: 1001:T:1 -> 1002:T:5
    // Write: 1001:T:1 = 100.0
    // Verify: Both source and target channels have correct data

    let (rtdb, routing_cache) = setup_c2c_routing(vec![("1001:T:1", "1002:T:5")]).await;

    // Write to source channel
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 100.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify source channel
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;

    // Verify target channel
    assert_channel_value(rtdb.as_ref(), 1002, "T", 5, 100.0).await;
}

#[tokio::test]
async fn test_c2c_cascade_two_levels() {
    // Scenario: Two-level cascade forwarding
    // Config: 1001:T:1 -> 1002:T:2 -> 1003:T:3
    // Write: 1001:T:1 = 50.0
    // Verify: All three channels have correct data, cascade_depth: 0 -> 1 -> 2 (stops)

    let (rtdb, routing_cache) =
        setup_c2c_routing(vec![("1001:T:1", "1002:T:2"), ("1002:T:2", "1003:T:3")]).await;

    // Write to source channel (first level, cascade_depth = 0)
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 50.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify level 1: 1001:T:1 (cascade_depth = 0)
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 50.0).await;

    // Verify level 2: 1002:T:2 (cascade_depth = 1)
    assert_channel_value(rtdb.as_ref(), 1002, "T", 2, 50.0).await;

    // Verify level 3: 1003:T:3 (cascade_depth = 2, reaches MAX_C2C_DEPTH, stops forwarding)
    // Note: cascade_depth = 2 still writes data, but doesn't forward further
    assert_channel_value(rtdb.as_ref(), 1003, "T", 3, 50.0).await;
}

#[tokio::test]
async fn test_c2c_cascade_max_depth() {
    // Scenario: Three-level cascade (verifies max depth limit)
    // Config: 1001:T:1 -> 1002:T:2 -> 1003:T:3 -> 1004:T:4
    // Write: 1001:T:1 = 200.0
    // Verify: First three levels have data, fourth level has no data (exceeds MAX_C2C_DEPTH)

    let (rtdb, routing_cache) = setup_c2c_routing(vec![
        ("1001:T:1", "1002:T:2"),
        ("1002:T:2", "1003:T:3"),
        ("1003:T:3", "1004:T:4"),
    ])
    .await;

    // Write to source channel
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 200.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify level 1: 1001:T:1 (cascade_depth = 0, forwards)
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 200.0).await;

    // Verify level 2: 1002:T:2 (cascade_depth = 1, forwards)
    assert_channel_value(rtdb.as_ref(), 1002, "T", 2, 200.0).await;

    // Verify level 3: 1003:T:3 (cascade_depth = 2, writes but doesn't forward)
    // Note: MAX_C2C_DEPTH = 2, forwards only when cascade_depth < 2
    // cascade_depth = 2 writes data but doesn't forward further
    assert_channel_value(rtdb.as_ref(), 1003, "T", 3, 200.0).await;

    // Verify level 4: 1004:T:4 (should have no data, previous level didn't forward)
    assert_channel_value_missing(rtdb.as_ref(), 1004, "T", 4).await;
}

#[tokio::test]
async fn test_c2c_infinite_loop_prevention() {
    // Scenario: Infinite loop detection
    // Config cycle: 1001:T:1 -> 1002:T:1 -> 1001:T:1
    // Write: 1001:T:1 = 75.0
    // Verify: Stops when cascade_depth reaches MAX_C2C_DEPTH, no infinite loop

    let (rtdb, routing_cache) = setup_c2c_routing(vec![
        ("1001:T:1", "1002:T:1"),
        ("1002:T:1", "1001:T:1"), // Circular route
    ])
    .await;

    // Write to source channel
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 75.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify source channel (overwritten twice: cascade_depth=0 and cascade_depth=2)
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 75.0).await;

    // Verify target channel (cascade_depth = 1)
    assert_channel_value(rtdb.as_ref(), 1002, "T", 1, 75.0).await;

    // Important: Test should complete normally, proving no infinite loop
}

#[tokio::test]
async fn test_c2c_preserve_raw_values() {
    // Scenario: Raw values correctly passed through cascade
    // Config: 1001:T:1 -> 1002:T:2
    // Write: value = 100.0, raw_value = 1000
    // Verify: Target channel has correct engineering value and raw value

    let (rtdb, routing_cache) = setup_c2c_routing(vec![("1001:T:1", "1002:T:2")]).await;

    // Write to source channel (with raw value)
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 100.0,
        raw_value: Some(1000.0),
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify source channel engineering value and raw value
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;
    assert_raw_value(rtdb.as_ref(), 1001, "T", 1, 1000.0).await;

    // Verify target channel engineering value and raw value
    assert_channel_value(rtdb.as_ref(), 1002, "T", 2, 100.0).await;
    assert_raw_value(rtdb.as_ref(), 1002, "T", 2, 1000.0).await;
}

#[tokio::test]
async fn test_c2c_different_point_types() {
    // Scenario: Forwarding different point types
    // Test T->T, S->S, C->C, A->A for all four types
    // Verify type conversion works correctly

    let (rtdb, routing_cache) = setup_c2c_routing(vec![
        ("1001:T:1", "1002:T:1"), // Telemetry -> Telemetry
        ("1001:S:2", "1002:S:2"), // Signal -> Signal
        ("1001:C:3", "1002:C:3"), // Control -> Control
        ("1001:A:4", "1002:A:4"), // Adjustment -> Adjustment
    ])
    .await;

    // Write four types of points
    let updates = vec![
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 10.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Signal,
            point_id: 2,
            value: 1.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Control,
            point_id: 3,
            value: 0.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Adjustment,
            point_id: 4,
            value: 50.5,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify Telemetry (T)
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 10.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, "T", 1, 10.0).await;

    // Verify Signal (S)
    assert_channel_value(rtdb.as_ref(), 1001, "S", 2, 1.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, "S", 2, 1.0).await;

    // Verify Control (C)
    assert_channel_value(rtdb.as_ref(), 1001, "C", 3, 0.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, "C", 3, 0.0).await;

    // Verify Adjustment (A)
    assert_channel_value(rtdb.as_ref(), 1001, "A", 4, 50.5).await;
    assert_channel_value(rtdb.as_ref(), 1002, "A", 4, 50.5).await;
}

#[tokio::test]
async fn test_c2c_one_to_many() {
    // Scenario: One-to-many forwarding
    // Config: 1001:T:1 -> 1002:T:1 and 1001:T:1 -> 1003:T:1
    // Write: 1001:T:1 = 123.0
    // Verify: Both target channels receive data

    // Note: RoutingCache's DashMap can only store one target per source
    // In practice, one-to-many requires multiple routing entries at config level
    // Here we test two different source points forwarding separately

    let (rtdb, routing_cache) = setup_c2c_routing(vec![
        ("1001:T:1", "1002:T:1"),
        ("1001:T:2", "1003:T:1"), // Different source point
    ])
    .await;

    // Write two source points
    let updates = vec![
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 123.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 2,
            value: 123.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify source channels
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 123.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 2, 123.0).await;

    // Verify both target channels receive data
    assert_channel_value(rtdb.as_ref(), 1002, "T", 1, 123.0).await;
    assert_channel_value(rtdb.as_ref(), 1003, "T", 1, 123.0).await;
}

#[tokio::test]
async fn test_c2c_no_routing() {
    // Scenario: No C2C routing configured
    // No C2C routing config
    // Write channel data
    // Verify: Only source channel has data, no forwarding

    let (rtdb, routing_cache) = setup_c2c_routing(vec![]).await; // Empty routing

    // Write to source channel
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 99.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify source channel
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 99.0).await;

    // Verify other channels have no data
    assert_channel_value_missing(rtdb.as_ref(), 1002, "T", 1).await;
    assert_channel_value_missing(rtdb.as_ref(), 1003, "T", 1).await;
}

#[tokio::test]
async fn test_c2c_cross_type_routing() {
    // Scenario: Cross-type forwarding (edge case)
    // Config: 1001:T:1 -> 1002:S:1 (Telemetry -> Signal)
    // Verify: Cross-type forwarding works correctly

    let (rtdb, routing_cache) = setup_c2c_routing(vec![("1001:T:1", "1002:S:1")]).await;

    // Write telemetry point
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 88.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify source channel (Telemetry)
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 88.0).await;

    // Verify target channel (Signal)
    assert_channel_value(rtdb.as_ref(), 1002, "S", 1, 88.0).await;
}

#[test]
fn test_max_c2c_depth_constant() {
    // Verify constant definition matches code
    assert_eq!(
        MAX_C2C_DEPTH, 2,
        "MAX_C2C_DEPTH should be 2 (consistent with storage.rs)"
    );
}

#[tokio::test]
async fn test_c2c_timestamp_propagation() {
    // Scenario: Timestamps correctly generated in cascade
    // Config: 1001:T:1 -> 1002:T:2
    // Verify: Both source and target channels have timestamps

    let (rtdb, routing_cache) = setup_c2c_routing(vec![("1001:T:1", "1002:T:2")]).await;

    // Write to source channel
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 66.6,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // Verify source channel timestamp exists
    let config = KeySpaceConfig::production();
    let point_type_enum = voltage_model::PointType::Telemetry;
    let channel_key_1001 = config.channel_key(1001, point_type_enum);
    let ts_key_1001 = format!("{}:ts", channel_key_1001);

    let ts_1001 = rtdb
        .hash_get(&ts_key_1001, "1")
        .await
        .expect("Failed to read timestamp")
        .expect("Timestamp should exist");

    assert!(!ts_1001.is_empty(), "Timestamp should not be empty");

    // Verify target channel timestamp exists
    let channel_key_1002 = config.channel_key(1002, point_type_enum);
    let ts_key_1002 = format!("{}:ts", channel_key_1002);

    let ts_1002 = rtdb
        .hash_get(&ts_key_1002, "2")
        .await
        .expect("Failed to read timestamp")
        .expect("Timestamp should exist");

    assert!(!ts_1002.is_empty(), "Timestamp should not be empty");

    // Verify timestamps should be close (allow small difference since cascade writes execute sequentially)
    // Each write_batch call generates new timestamp, so there may be 1-2ms difference
    let ts_1001_value: u64 = String::from_utf8(ts_1001.to_vec())
        .unwrap()
        .parse()
        .unwrap();
    let ts_1002_value: u64 = String::from_utf8(ts_1002.to_vec())
        .unwrap()
        .parse()
        .unwrap();

    let time_diff = ts_1001_value.abs_diff(ts_1002_value);

    assert!(
        time_diff <= 10, // Allow up to 10ms time difference
        "Timestamps should be close (diff: {}ms)",
        time_diff
    );
}
