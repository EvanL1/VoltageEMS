//! C2C Cascade and Complex Routing Tests
//!
//! Tests for complete upstream data flow with C2C cascade:
//! - Channel-to-Channel cascade (C2C)
//! - Multi-channel parallel upstream
//! - Combined C2M + C2C routing scenarios
//! - Three-layer data flow through cascade

#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use std::collections::HashMap;
use std::sync::Arc;
use voltage_model::PointType;
use voltage_routing::{write_channel_batch, ChannelPointUpdate};
use voltage_rtdb::MemoryRtdb;
use voltage_rtdb::Rtdb;
use voltage_rtdb::{KeySpaceConfig, RoutingCache};

// ============================================================================
// Test Setup Helpers
// ============================================================================

/// Creates a memory RTDB for testing
fn create_test_rtdb() -> Arc<MemoryRtdb> {
    Arc::new(MemoryRtdb::new())
}

/// Creates routing cache with C2M and C2C configurations
fn create_routing_cache(
    c2m_routes: Vec<(&str, &str)>,
    c2c_routes: Vec<(&str, &str)>,
) -> Arc<RoutingCache> {
    let mut c2m_map = HashMap::new();
    let mut c2c_map = HashMap::new();

    for (source, target) in c2m_routes {
        c2m_map.insert(source.to_string(), target.to_string());
    }
    for (source, target) in c2c_routes {
        c2c_map.insert(source.to_string(), target.to_string());
    }

    Arc::new(RoutingCache::from_maps(c2m_map, HashMap::new(), c2c_map))
}

/// Assert channel value exists with expected value
async fn assert_channel_value<R: Rtdb>(
    rtdb: &R,
    channel_id: u32,
    point_type: PointType,
    point_id: u32,
    expected: f64,
) {
    let config = KeySpaceConfig::production();
    let key = config.channel_key(channel_id, point_type);
    let value = rtdb
        .hash_get(&key, &point_id.to_string())
        .await
        .expect("Read failed")
        .expect("Value should exist");
    let parsed: f64 = String::from_utf8(value.to_vec())
        .expect("UTF-8")
        .parse()
        .expect("f64");
    assert!(
        (parsed - expected).abs() < 1e-10,
        "Expected {} but got {}",
        expected,
        parsed
    );
}

/// Assert instance measurement value exists with expected value
async fn assert_instance_measurement<R: Rtdb>(
    rtdb: &R,
    instance_id: u32,
    point_id: u32,
    expected: f64,
) {
    let config = KeySpaceConfig::production();
    let key = config.instance_measurement_key(instance_id);
    let value = rtdb
        .hash_get(&key, &point_id.to_string())
        .await
        .expect("Read failed")
        .expect("Measurement should exist");
    let parsed: f64 = String::from_utf8(value.to_vec())
        .expect("UTF-8")
        .parse()
        .expect("f64");
    assert!(
        (parsed - expected).abs() < 1e-10,
        "Instance {} point {} expected {} but got {}",
        instance_id,
        point_id,
        expected,
        parsed
    );
}

/// Assert instance measurement does NOT exist
async fn assert_instance_measurement_not_exists<R: Rtdb>(
    rtdb: &R,
    instance_id: u32,
    point_id: u32,
) {
    let config = KeySpaceConfig::production();
    let key = config.instance_measurement_key(instance_id);
    let value = rtdb
        .hash_get(&key, &point_id.to_string())
        .await
        .expect("Read failed");
    assert!(
        value.is_none(),
        "Instance {} point {} should not exist",
        instance_id,
        point_id
    );
}

// ============================================================================
// C2C Cascade Tests
// ============================================================================

#[tokio::test]
async fn test_c2c_basic_cascade() {
    // Scenario: Channel 1001:T:1 cascades to Channel 1002:T:1
    // Both should be written, only 1002 has C2M routing
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(
        vec![("1002:T:1", "10:M:1")],   // Only target channel has C2M
        vec![("1001:T:1", "1002:T:1")], // C2C: 1001 -> 1002
    );

    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 220.5,
        raw_value: None,
        cascade_depth: 0,
    }];

    let result = write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    // Verify: Source channel written
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1, 220.5).await;

    // Verify: Target channel written via C2C
    assert_channel_value(rtdb.as_ref(), 1002, PointType::Telemetry, 1, 220.5).await;

    // Verify: Instance measurement written (from 1002)
    assert_instance_measurement(rtdb.as_ref(), 10, 1, 220.5).await;

    // Verify result stats
    assert_eq!(result.channel_writes, 2, "Both channels should be written");
    assert_eq!(result.c2c_forwards, 1, "One C2C forward");
    assert_eq!(
        result.c2m_writes, 1,
        "One C2M write (from cascaded channel)"
    );
}

#[tokio::test]
async fn test_c2c_multi_hop_cascade() {
    // Scenario: 1001:T:1 -> 1002:T:1 -> 1003:T:1 (2 hops)
    // Final channel (1003) has C2M routing
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(
        vec![("1003:T:1", "10:M:1")], // Only final channel has C2M
        vec![
            ("1001:T:1", "1002:T:1"), // Hop 1
            ("1002:T:1", "1003:T:1"), // Hop 2
        ],
    );

    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 100.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    let result = write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    // All three channels should have the value
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1, 100.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, PointType::Telemetry, 1, 100.0).await;
    assert_channel_value(rtdb.as_ref(), 1003, PointType::Telemetry, 1, 100.0).await;

    // Instance should have value from final hop
    assert_instance_measurement(rtdb.as_ref(), 10, 1, 100.0).await;

    // Stats: 3 channels, 2 C2C forwards, 1 C2M
    assert_eq!(result.channel_writes, 3);
    assert_eq!(result.c2c_forwards, 2);
    assert_eq!(result.c2m_writes, 1);
}

#[tokio::test]
async fn test_c2c_with_c2m_at_each_hop() {
    // Scenario: Both source and cascade target have C2M routing
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(
        vec![
            ("1001:T:1", "10:M:1"), // Source channel has C2M
            ("1002:T:1", "20:M:1"), // Target channel also has C2M
        ],
        vec![("1001:T:1", "1002:T:1")], // C2C cascade
    );

    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 50.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    let result = write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    // Both channels should be written
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1, 50.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, PointType::Telemetry, 1, 50.0).await;

    // Both instances should have the value
    assert_instance_measurement(rtdb.as_ref(), 10, 1, 50.0).await;
    assert_instance_measurement(rtdb.as_ref(), 20, 1, 50.0).await;

    // Stats: 2 channels, 1 C2C forward, 2 C2M writes
    assert_eq!(result.channel_writes, 2);
    assert_eq!(result.c2c_forwards, 1);
    assert_eq!(result.c2m_writes, 2);
}

#[tokio::test]
async fn test_c2c_cross_type_cascade() {
    // Scenario: Telemetry cascades to Signal type
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(
        vec![("1002:S:1", "10:M:1")],   // Signal channel has C2M
        vec![("1001:T:1", "1002:S:1")], // T -> S type cascade
    );

    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 1.0, // Could represent a threshold being exceeded
        raw_value: None,
        cascade_depth: 0,
    }];

    let result = write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    // Source (Telemetry) written
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1, 1.0).await;

    // Target (Signal) written via C2C
    assert_channel_value(rtdb.as_ref(), 1002, PointType::Signal, 1, 1.0).await;

    // Instance gets the value
    assert_instance_measurement(rtdb.as_ref(), 10, 1, 1.0).await;

    assert_eq!(result.c2c_forwards, 1);
}

// ============================================================================
// Multi-Channel Parallel Upstream Tests
// ============================================================================

#[tokio::test]
async fn test_multi_channel_parallel_upstream() {
    // Scenario: 10 channels sending data simultaneously to same instance
    let rtdb = create_test_rtdb();

    // Setup: 10 channels (1001-1010) all route to instance 10, different points
    let c2m_routes: Vec<(&str, &str)> = (1..=10)
        .map(|i| {
            // Leak the strings to get static lifetimes for the test
            let source = format!("{}:T:1", 1000 + i); // 1001, 1002, ..., 1010
            let target = format!("10:M:{}", i);
            (
                Box::leak(source.into_boxed_str()) as &str,
                Box::leak(target.into_boxed_str()) as &str,
            )
        })
        .collect();

    let routing_cache = create_routing_cache(c2m_routes, vec![]);

    // Create updates from all 10 channels
    let updates: Vec<ChannelPointUpdate> = (1..=10)
        .map(|i| ChannelPointUpdate {
            channel_id: 1000 + i,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: i as f64 * 10.0, // 10, 20, 30, ...
            raw_value: None,
            cascade_depth: 0,
        })
        .collect();

    let result = write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    // Verify all channels written
    for i in 1..=10 {
        assert_channel_value(
            rtdb.as_ref(),
            1000 + i,
            PointType::Telemetry,
            1,
            i as f64 * 10.0,
        )
        .await;
    }

    // Verify all instance points written
    for i in 1..=10 {
        assert_instance_measurement(rtdb.as_ref(), 10, i, i as f64 * 10.0).await;
    }

    assert_eq!(result.channel_writes, 10);
    assert_eq!(result.c2m_writes, 10);
}

#[tokio::test]
async fn test_multi_instance_dispatch() {
    // Scenario: One channel distributes to multiple instances
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(
        vec![
            ("1001:T:1", "10:M:1"),
            ("1001:T:2", "20:M:1"),
            ("1001:T:3", "30:M:1"),
        ],
        vec![],
    );

    let updates = vec![
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 100.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 2,
            value: 200.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 3,
            value: 300.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    let result = write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    // Verify instances received their data
    assert_instance_measurement(rtdb.as_ref(), 10, 1, 100.0).await;
    assert_instance_measurement(rtdb.as_ref(), 20, 1, 200.0).await;
    assert_instance_measurement(rtdb.as_ref(), 30, 1, 300.0).await;

    // Note: c2m_writes counts unique instance keys
    assert!(result.c2m_writes >= 1);
}

// ============================================================================
// Combined C2M + C2C Complex Scenarios
// ============================================================================

#[tokio::test]
async fn test_mixed_routing_scenario() {
    // Complex scenario:
    // - Channel 1001 has both C2M (to inst 10) and C2C (to 1002)
    // - Channel 1002 has C2M (to inst 20) and C2C (to 1003)
    // - Channel 1003 has only C2M (to inst 30)
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(
        vec![
            ("1001:T:1", "10:M:1"),
            ("1002:T:1", "20:M:1"),
            ("1003:T:1", "30:M:1"),
        ],
        vec![("1001:T:1", "1002:T:1"), ("1002:T:1", "1003:T:1")],
    );

    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 42.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    let result = write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    // All 3 channels should be written
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1, 42.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, PointType::Telemetry, 1, 42.0).await;
    assert_channel_value(rtdb.as_ref(), 1003, PointType::Telemetry, 1, 42.0).await;

    // All 3 instances should receive the value
    assert_instance_measurement(rtdb.as_ref(), 10, 1, 42.0).await;
    assert_instance_measurement(rtdb.as_ref(), 20, 1, 42.0).await;
    assert_instance_measurement(rtdb.as_ref(), 30, 1, 42.0).await;

    // Stats
    assert_eq!(result.channel_writes, 3, "3 channels written");
    assert_eq!(result.c2c_forwards, 2, "2 C2C forwards");
    assert_eq!(result.c2m_writes, 3, "3 C2M writes");
}

#[tokio::test]
async fn test_partial_routing() {
    // Scenario: Some points have routing, others don't
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(
        vec![
            ("1001:T:1", "10:M:1"), // Point 1 has routing
            ("1001:T:3", "10:M:3"), // Point 3 has routing
                                    // Point 2 has NO routing
        ],
        vec![],
    );

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
            point_type: PointType::Telemetry,
            point_id: 2,
            value: 20.0, // No routing for this point
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 3,
            value: 30.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    // All channel points should be written
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1, 10.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 2, 20.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 3, 30.0).await;

    // Only routed points should exist in instance
    assert_instance_measurement(rtdb.as_ref(), 10, 1, 10.0).await;
    assert_instance_measurement_not_exists(rtdb.as_ref(), 10, 2).await; // No routing
    assert_instance_measurement(rtdb.as_ref(), 10, 3, 30.0).await;
}

// ============================================================================
// Three-Layer Data Flow Tests
// ============================================================================

#[tokio::test]
async fn test_three_layer_with_cascade() {
    // Scenario: Full 3-layer data (value/ts/raw) flows through C2C cascade
    let rtdb = create_test_rtdb();
    let routing_cache =
        create_routing_cache(vec![("1002:T:1", "10:M:1")], vec![("1001:T:1", "1002:T:1")]);

    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 220.5,
        raw_value: Some(2205.0), // Raw value
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");

    let config = KeySpaceConfig::production();

    // Verify source channel 3-layer
    let src_key = config.channel_key(1001, PointType::Telemetry);
    let src_value = rtdb.hash_get(&src_key, "1").await.unwrap().unwrap();
    assert_eq!(String::from_utf8(src_value.to_vec()).unwrap(), "220.5");

    let src_raw = rtdb
        .hash_get(&format!("{}:raw", src_key), "1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(String::from_utf8(src_raw.to_vec()).unwrap(), "2205.0");

    // Verify target channel also has 3-layer (cascaded)
    let tgt_key = config.channel_key(1002, PointType::Telemetry);
    let tgt_value = rtdb.hash_get(&tgt_key, "1").await.unwrap().unwrap();
    assert_eq!(String::from_utf8(tgt_value.to_vec()).unwrap(), "220.5");

    let tgt_raw = rtdb
        .hash_get(&format!("{}:raw", tgt_key), "1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(String::from_utf8(tgt_raw.to_vec()).unwrap(), "2205.0");

    // Verify instance gets engineering value (not raw)
    assert_instance_measurement(rtdb.as_ref(), 10, 1, 220.5).await;
}

// ============================================================================
// Performance / Scale Tests
// ============================================================================

#[tokio::test]
async fn test_high_volume_mixed_routing() {
    // Scenario: 500 points, mix of routed and unrouted
    let rtdb = create_test_rtdb();

    // Route even-numbered points only
    let c2m_routes: Vec<(&str, &str)> = (1..=250)
        .map(|i| {
            let point_id = i * 2; // Even numbers: 2, 4, 6, ..., 500
            let source = format!("1001:T:{}", point_id);
            let target = format!("10:M:{}", point_id);
            (
                Box::leak(source.into_boxed_str()) as &str,
                Box::leak(target.into_boxed_str()) as &str,
            )
        })
        .collect();

    let routing_cache = create_routing_cache(c2m_routes, vec![]);

    // Create 500 updates
    let updates: Vec<ChannelPointUpdate> = (1..=500)
        .map(|point_id| ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id,
            value: point_id as f64,
            raw_value: None,
            cascade_depth: 0,
        })
        .collect();

    let start = std::time::Instant::now();
    let result = write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Write failed");
    let elapsed = start.elapsed();

    println!("500 points with mixed routing: {:?}", elapsed);

    // Channel points should be written (grouped by channel+type, so 1 batch = 500 field writes internally)
    // Note: channel_writes counts the number of grouped batches processed, not individual points
    assert!(
        result.channel_writes >= 1,
        "At least one channel batch should be written"
    );

    // Verify samples
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1, 1.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 500, 500.0).await;

    // Even points should be in instance
    assert_instance_measurement(rtdb.as_ref(), 10, 2, 2.0).await;
    assert_instance_measurement(rtdb.as_ref(), 10, 500, 500.0).await;

    // Odd points should NOT be in instance
    assert_instance_measurement_not_exists(rtdb.as_ref(), 10, 1).await;
    assert_instance_measurement_not_exists(rtdb.as_ref(), 10, 499).await;

    // Should complete in reasonable time
    assert!(
        elapsed.as_millis() < 200,
        "500 points should complete in <200ms"
    );
}
