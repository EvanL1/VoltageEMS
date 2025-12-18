//! C2M Routing End-to-End Tests
//!
//! Tests the complete Channel-to-Model routing flow:
//! 1. Channel write: comsrv:{channel_id}:{T|S|C|A} Hash
//! 2. Routing lookup: RoutingCache.lookup_c2m("{channel_id}:{type}:{point_id}")
//! 3. Instance write: inst:{instance_id}:M Hash
//!
//! Uses MemoryRtdb (no external dependencies) for fast, isolated testing.

use std::collections::HashMap;
use std::sync::Arc;
use voltage_model::PointType;
use voltage_routing::{write_channel_batch, ChannelPointUpdate};
use voltage_rtdb::Rtdb;
use voltage_rtdb::{KeySpaceConfig, RoutingCache};

/// Creates a memory RTDB for testing
fn create_test_rtdb() -> Arc<dyn Rtdb> {
    Arc::new(voltage_rtdb::MemoryRtdb::new())
}

/// Creates a test environment with routing configuration
///
/// # Arguments
/// * `c2m_routes` - C2M routing config, format: [("1001:T:1", "23:M:1"), ...]
///
/// # Returns
/// * `(Arc<dyn Rtdb>, Arc<RoutingCache>)` - RTDB and routing cache
async fn setup_c2m_routing(c2m_routes: Vec<(&str, &str)>) -> (Arc<dyn Rtdb>, Arc<RoutingCache>) {
    let rtdb = create_test_rtdb();
    let mut c2m_map = HashMap::new();
    for (source, target) in c2m_routes {
        c2m_map.insert(source.to_string(), target.to_string());
    }
    let routing_cache = Arc::new(RoutingCache::from_maps(
        c2m_map,
        HashMap::new(),
        HashMap::new(),
    ));
    (rtdb, routing_cache)
}

/// Helper: Asserts channel data (engineering value layer)
async fn assert_channel_value(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_value: f64,
) {
    use voltage_model::PointType;
    let config = KeySpaceConfig::production();
    let point_type_enum = PointType::from_str(point_type).expect("Invalid point type");
    let channel_key = config.channel_key(channel_id, point_type_enum);

    let value = rtdb
        .hash_get(&channel_key, &point_id.to_string())
        .await
        .expect("Failed to get channel value")
        .expect("Channel value not found");

    let value_str = String::from_utf8(value.to_vec()).expect("Invalid UTF-8");
    let value_f64: f64 = value_str.parse().expect("Invalid float");
    assert_eq!(value_f64, expected_value);
}

/// Helper: Asserts channel timestamp exists
async fn assert_channel_timestamp_exists(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
) {
    use voltage_model::PointType;
    let config = KeySpaceConfig::production();
    let point_type_enum = PointType::from_str(point_type).expect("Invalid point type");
    let channel_key = config.channel_key(channel_id, point_type_enum);
    let channel_ts_key = format!("{}:ts", channel_key);

    let ts = rtdb
        .hash_get(&channel_ts_key, &point_id.to_string())
        .await
        .expect("Failed to get channel timestamp")
        .expect("Channel timestamp not found");

    let ts_str = String::from_utf8(ts.to_vec()).expect("Invalid UTF-8");
    let ts_i64: i64 = ts_str.parse().expect("Invalid timestamp");
    assert!(ts_i64 > 0, "Timestamp should be positive");
}

/// Helper: Asserts channel raw value
async fn assert_channel_raw_value(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_raw: f64,
) {
    use voltage_model::PointType;
    let config = KeySpaceConfig::production();
    let point_type_enum = PointType::from_str(point_type).expect("Invalid point type");
    let channel_key = config.channel_key(channel_id, point_type_enum);
    let channel_raw_key = format!("{}:raw", channel_key);

    let raw = rtdb
        .hash_get(&channel_raw_key, &point_id.to_string())
        .await
        .expect("Failed to get channel raw value")
        .expect("Channel raw value not found");

    let raw_str = String::from_utf8(raw.to_vec()).expect("Invalid UTF-8");
    let raw_f64: f64 = raw_str.parse().expect("Invalid float");
    assert_eq!(raw_f64, expected_raw);
}

/// Helper: Asserts instance measurement value
async fn assert_instance_measurement(
    rtdb: &dyn Rtdb,
    instance_id: u16,
    point_id: u32,
    expected_value: f64,
) {
    let config = KeySpaceConfig::production();
    let instance_key = config.instance_measurement_key(instance_id.into());

    let value = rtdb
        .hash_get(&instance_key, &point_id.to_string())
        .await
        .expect("Failed to get instance measurement")
        .expect("Instance measurement not found");

    let value_str = String::from_utf8(value.to_vec()).expect("Invalid UTF-8");
    let value_f64: f64 = value_str.parse().expect("Invalid float");
    assert_eq!(value_f64, expected_value);
}

/// Helper: Asserts instance measurement does not exist
async fn assert_instance_measurement_not_exists(rtdb: &dyn Rtdb, instance_id: u16, point_id: u32) {
    let config = KeySpaceConfig::production();
    let instance_key = config.instance_measurement_key(instance_id.into());

    let value = rtdb
        .hash_get(&instance_key, &point_id.to_string())
        .await
        .expect("Failed to get instance measurement");

    assert!(value.is_none(), "Instance measurement should not exist");
}

#[tokio::test]
async fn test_c2m_basic_routing() {
    // Given: Routing config 1001:T:1 -> inst:23:M:1
    let (rtdb, routing_cache) = setup_c2m_routing(vec![("1001:T:1", "23:M:1")]).await;

    // When: Write channel point
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 230.5,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: Verify channel data written successfully
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 230.5).await;
    assert_channel_timestamp_exists(rtdb.as_ref(), 1001, "T", 1).await;

    // Then: Verify instance data routed successfully
    assert_instance_measurement(rtdb.as_ref(), 23, 1, 230.5).await;
}

#[tokio::test]
async fn test_c2m_three_layer_architecture() {
    // Given: Routing config
    let (rtdb, routing_cache) = setup_c2m_routing(vec![("1001:T:1", "23:M:1")]).await;

    // When: Write channel point (with raw value)
    let updates = vec![ChannelPointUpdate {
        channel_id: 1001,
        point_type: PointType::Telemetry,
        point_id: 1,
        value: 230.5,            // Engineering value
        raw_value: Some(2305.0), // Raw value
        cascade_depth: 0,
    }];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: Verify three-layer architecture all written correctly
    // Layer 1: Engineering value
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 230.5).await;

    // Layer 2: Timestamp
    assert_channel_timestamp_exists(rtdb.as_ref(), 1001, "T", 1).await;

    // Layer 3: Raw value
    assert_channel_raw_value(rtdb.as_ref(), 1001, "T", 1, 2305.0).await;

    // Then: Verify instance data routed successfully (uses engineering value)
    assert_instance_measurement(rtdb.as_ref(), 23, 1, 230.5).await;
}

#[tokio::test]
async fn test_c2m_routing_to_multiple_instances() {
    // Given: Multiple channel points routed to different instances
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:1", "23:M:1"), // Channel 1001 point 1 -> Instance 23
        ("1001:T:2", "24:M:1"), // Channel 1001 point 2 -> Instance 24
        ("1001:T:3", "25:M:1"), // Channel 1001 point 3 -> Instance 25
    ])
    .await;

    // When: Write multiple channel points
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

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: Verify channel data written successfully
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 2, 200.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 3, 300.0).await;

    // Then: Verify data routed to different instances
    assert_instance_measurement(rtdb.as_ref(), 23, 1, 100.0).await;
    assert_instance_measurement(rtdb.as_ref(), 24, 1, 200.0).await;
    assert_instance_measurement(rtdb.as_ref(), 25, 1, 300.0).await;
}

#[tokio::test]
async fn test_c2m_no_routing() {
    // Given: No routing configured
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(RoutingCache::new()); // Empty routing cache

    // When: Write channel point
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
        .expect("Failed to write batch");

    // Then: Verify channel data written successfully
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;

    // Then: Verify instance data does not exist (no routing)
    assert_instance_measurement_not_exists(rtdb.as_ref(), 23, 1).await;
}

#[tokio::test]
async fn test_c2m_batch_updates() {
    // Given: Batch routing config
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:1", "23:M:1"),
        ("1001:T:2", "23:M:2"),
        ("1001:T:3", "23:M:3"),
        ("1001:T:4", "23:M:4"),
        ("1001:T:5", "23:M:5"),
    ])
    .await;

    // When: Batch write 5 points
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
            value: 20.0,
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
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 4,
            value: 40.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 5,
            value: 50.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: Verify all points routed correctly
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 10.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 2, 20.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 3, 30.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 4, 40.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 5, 50.0).await;

    assert_instance_measurement(rtdb.as_ref(), 23, 1, 10.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 2, 20.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 3, 30.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 4, 40.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 5, 50.0).await;
}

#[tokio::test]
async fn test_c2m_different_point_types() {
    // Given: Four Remote types routing config
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:1", "23:M:1"), // Telemetry
        ("1001:S:2", "23:M:2"), // Signal
        ("1001:C:3", "23:M:3"), // Control
        ("1001:A:4", "23:M:4"), // Adjustment
    ])
    .await;

    // When: Write different types of points
    let updates = vec![
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry, // Telemetry
            point_id: 1,
            value: 230.5,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Signal, // Signal
            point_id: 2,
            value: 1.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Control, // Control
            point_id: 3,
            value: 0.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Adjustment, // Adjustment
            point_id: 4,
            value: 50.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: Verify all four types routed correctly
    // Telemetry
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 230.5).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 1, 230.5).await;

    // Signal
    assert_channel_value(rtdb.as_ref(), 1001, "S", 2, 1.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 2, 1.0).await;

    // Control
    assert_channel_value(rtdb.as_ref(), 1001, "C", 3, 0.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 3, 0.0).await;

    // Adjustment
    assert_channel_value(rtdb.as_ref(), 1001, "A", 4, 50.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 4, 50.0).await;
}

#[tokio::test]
async fn test_c2m_routing_with_different_point_ids() {
    // Given: Routing config (source and target point IDs differ)
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:10", "23:M:1"), // Channel point 10 -> Instance point 1
        ("1001:T:20", "23:M:5"), // Channel point 20 -> Instance point 5
    ])
    .await;

    // When: Write channel points
    let updates = vec![
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 10,
            value: 100.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id: 20,
            value: 200.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: Verify routed to correct instance points
    assert_channel_value(rtdb.as_ref(), 1001, "T", 10, 100.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 20, 200.0).await;

    assert_instance_measurement(rtdb.as_ref(), 23, 1, 100.0).await; // Point 10 -> 1
    assert_instance_measurement(rtdb.as_ref(), 23, 5, 200.0).await; // Point 20 -> 5
}

#[tokio::test]
async fn test_c2m_routing_with_multiple_channels() {
    // Given: Multiple channels routed to same instance
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:1", "23:M:1"), // Channel 1001
        ("1002:T:1", "23:M:2"), // Channel 1002
        ("1003:T:1", "23:M:3"), // Channel 1003
    ])
    .await;

    // When: Write points from multiple channels
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
            channel_id: 1002,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 200.0,
            raw_value: None,
            cascade_depth: 0,
        },
        ChannelPointUpdate {
            channel_id: 1003,
            point_type: PointType::Telemetry,
            point_id: 1,
            value: 300.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: Verify all channel data routed to instance correctly
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, "T", 1, 200.0).await;
    assert_channel_value(rtdb.as_ref(), 1003, "T", 1, 300.0).await;

    assert_instance_measurement(rtdb.as_ref(), 23, 1, 100.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 2, 200.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 3, 300.0).await;
}
