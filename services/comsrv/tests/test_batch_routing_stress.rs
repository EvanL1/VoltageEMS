//! Batch Routing Stress Tests
//!
//! Tests for large-scale batch write operations:
//! - 100+ mixed point types (T/S/C/A)
//! - 1000+ telemetry points
//! - Three-layer data (value/timestamp/raw)
//! - Multi-channel concurrent writes

#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use voltage_model::PointType;
use voltage_routing::{write_channel_batch, ChannelPointUpdate};
use voltage_rtdb::Rtdb;
use voltage_rtdb::{KeySpaceConfig, MemoryRtdb, RoutingCache};

/// Creates a memory RTDB for testing
fn create_test_rtdb() -> Arc<MemoryRtdb> {
    Arc::new(MemoryRtdb::new())
}

/// Creates routing cache with C2M routes for given channel/point ranges
fn create_routing_cache(
    channel_id: u32,
    point_type: PointType,
    point_range: std::ops::Range<u32>,
    instance_id: u16,
) -> Arc<RoutingCache> {
    let mut c2m_map = HashMap::new();
    let type_char = match point_type {
        PointType::Telemetry => "T",
        PointType::Signal => "S",
        PointType::Control => "C",
        PointType::Adjustment => "A",
    };

    for point_id in point_range {
        let source = format!("{}:{}:{}", channel_id, type_char, point_id);
        let target = format!("{}:M:{}", instance_id, point_id);
        c2m_map.insert(source, target);
    }

    Arc::new(RoutingCache::from_maps(
        c2m_map,
        HashMap::new(),
        HashMap::new(),
    ))
}

/// Helper: Verify channel value
async fn verify_channel_value<R: Rtdb>(
    rtdb: &R,
    channel_id: u32,
    point_type: PointType,
    point_id: u32,
) -> Option<f64> {
    let config = KeySpaceConfig::production();
    let channel_key = config.channel_key(channel_id, point_type);

    rtdb.hash_get(&channel_key, &point_id.to_string())
        .await
        .ok()
        .flatten()
        .and_then(|bytes| {
            String::from_utf8(bytes.to_vec())
                .ok()
                .and_then(|s| s.parse().ok())
        })
}

/// Helper: Verify instance measurement value
async fn verify_instance_value<R: Rtdb>(rtdb: &R, instance_id: u16, point_id: u32) -> Option<f64> {
    let config = KeySpaceConfig::production();
    let instance_key = config.instance_measurement_key(instance_id.into());

    rtdb.hash_get(&instance_key, &point_id.to_string())
        .await
        .ok()
        .flatten()
        .and_then(|bytes| {
            String::from_utf8(bytes.to_vec())
                .ok()
                .and_then(|s| s.parse().ok())
        })
}

// ============================================================================
// 100 Points Mixed Types Tests
// ============================================================================

#[tokio::test]
async fn test_batch_write_100_mixed_point_types() {
    let rtdb = create_test_rtdb();

    // Create routing for all point types
    let mut c2m_map = HashMap::new();
    for point_id in 1..=25 {
        c2m_map.insert(format!("1001:T:{}", point_id), format!("10:M:{}", point_id));
        c2m_map.insert(
            format!("1001:S:{}", point_id),
            format!("10:M:{}", 25 + point_id),
        );
        c2m_map.insert(
            format!("1001:C:{}", point_id),
            format!("10:M:{}", 50 + point_id),
        );
        c2m_map.insert(
            format!("1001:A:{}", point_id),
            format!("10:M:{}", 75 + point_id),
        );
    }
    let routing_cache = Arc::new(RoutingCache::from_maps(
        c2m_map,
        HashMap::new(),
        HashMap::new(),
    ));

    // Create 100 updates (25 each for T/S/C/A)
    let mut updates = Vec::with_capacity(100);

    // Telemetry points (voltage readings)
    for point_id in 1..=25 {
        updates.push(ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id,
            value: 220.0 + point_id as f64,
            raw_value: None,
            cascade_depth: 0,
        });
    }

    // Status points (binary states)
    for point_id in 1..=25 {
        updates.push(ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Signal,
            point_id,
            value: (point_id % 2) as f64, // Alternating 0/1
            raw_value: None,
            cascade_depth: 0,
        });
    }

    // Control points
    for point_id in 1..=25 {
        updates.push(ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Control,
            point_id,
            value: point_id as f64,
            raw_value: None,
            cascade_depth: 0,
        });
    }

    // Adjustment points
    for point_id in 1..=25 {
        updates.push(ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Adjustment,
            point_id,
            value: point_id as f64 * 100.0,
            raw_value: None,
            cascade_depth: 0,
        });
    }

    assert_eq!(updates.len(), 100);

    // Execute batch write
    let start = Instant::now();
    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");
    let elapsed = start.elapsed();

    println!("100 mixed points write: {:?}", elapsed);

    // Verify samples from each type
    // Telemetry
    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1).await;
    assert_eq!(value, Some(221.0));

    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 25).await;
    assert_eq!(value, Some(245.0));

    // Status
    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Signal, 1).await;
    assert_eq!(value, Some(1.0));

    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Signal, 2).await;
    assert_eq!(value, Some(0.0));

    // Control
    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Control, 10).await;
    assert_eq!(value, Some(10.0));

    // Adjustment
    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Adjustment, 5).await;
    assert_eq!(value, Some(500.0));

    // Verify instance routing
    let value = verify_instance_value(rtdb.as_ref(), 10, 1).await;
    assert_eq!(value, Some(221.0)); // First telemetry point
}

// ============================================================================
// 1000 Points Large Scale Tests
// ============================================================================

#[tokio::test]
async fn test_batch_write_1000_telemetry_points() {
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(1001, PointType::Telemetry, 1..1001, 10);

    // Create 1000 telemetry updates
    let updates: Vec<_> = (1..=1000)
        .map(|point_id| ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id,
            value: point_id as f64 * 0.1, // 0.1, 0.2, ..., 100.0
            raw_value: None,
            cascade_depth: 0,
        })
        .collect();

    assert_eq!(updates.len(), 1000);

    // Execute batch write
    let start = Instant::now();
    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");
    let elapsed = start.elapsed();

    println!("1000 telemetry points write: {:?}", elapsed);
    assert!(
        elapsed.as_millis() < 500,
        "1000 points should complete in < 500ms"
    );

    // Verify samples
    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1).await;
    assert_eq!(value, Some(0.1));

    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 500).await;
    assert_eq!(value, Some(50.0));

    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1000).await;
    assert_eq!(value, Some(100.0));

    // Verify instance routing for samples
    let value = verify_instance_value(rtdb.as_ref(), 10, 1).await;
    assert_eq!(value, Some(0.1));

    let value = verify_instance_value(rtdb.as_ref(), 10, 500).await;
    assert_eq!(value, Some(50.0));

    let value = verify_instance_value(rtdb.as_ref(), 10, 1000).await;
    assert_eq!(value, Some(100.0));
}

#[tokio::test]
async fn test_batch_write_multi_channel_1000_points() {
    let rtdb = create_test_rtdb();

    // Create routing for 10 channels, 100 points each
    let mut c2m_map = HashMap::new();
    for channel_id in 1001..=1010 {
        let instance_id = channel_id - 1000; // 1, 2, ..., 10
        for point_id in 1..=100 {
            c2m_map.insert(
                format!("{}:T:{}", channel_id, point_id),
                format!("{}:M:{}", instance_id, point_id),
            );
        }
    }
    let routing_cache = Arc::new(RoutingCache::from_maps(
        c2m_map,
        HashMap::new(),
        HashMap::new(),
    ));

    // Create 1000 updates across 10 channels
    let mut updates = Vec::with_capacity(1000);
    for channel_id in 1001..=1010 {
        for point_id in 1..=100 {
            updates.push(ChannelPointUpdate {
                channel_id,
                point_type: PointType::Telemetry,
                point_id,
                value: channel_id as f64 + point_id as f64 * 0.01,
                raw_value: None,
                cascade_depth: 0,
            });
        }
    }

    assert_eq!(updates.len(), 1000);

    // Execute batch write
    let start = Instant::now();
    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");
    let elapsed = start.elapsed();

    println!("1000 points across 10 channels: {:?}", elapsed);

    // Verify samples from different channels
    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 1).await;
    assert_eq!(value, Some(1001.01));

    let value = verify_channel_value(rtdb.as_ref(), 1005, PointType::Telemetry, 50).await;
    assert_eq!(value, Some(1005.5));

    let value = verify_channel_value(rtdb.as_ref(), 1010, PointType::Telemetry, 100).await;
    assert_eq!(value, Some(1011.0));

    // Verify instance routing
    let value = verify_instance_value(rtdb.as_ref(), 1, 1).await;
    assert_eq!(value, Some(1001.01));

    let value = verify_instance_value(rtdb.as_ref(), 5, 50).await;
    assert_eq!(value, Some(1005.5));

    let value = verify_instance_value(rtdb.as_ref(), 10, 100).await;
    assert_eq!(value, Some(1011.0));
}

// ============================================================================
// Three-Layer Data Tests
// ============================================================================

#[tokio::test]
async fn test_batch_write_three_layer_100_points() {
    let rtdb = create_test_rtdb();
    let routing_cache = create_routing_cache(1001, PointType::Telemetry, 1..101, 10);

    // Create 100 updates with raw values (three-layer)
    let updates: Vec<_> = (1..=100)
        .map(|point_id| ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id,
            value: point_id as f64 * 2.2,            // Engineering value
            raw_value: Some(point_id as f64 * 22.0), // Raw value (10x)
            cascade_depth: 0,
        })
        .collect();

    // Execute batch write
    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Verify three layers for samples
    let config = KeySpaceConfig::production();

    // Point 1
    let channel_key = config.channel_key(1001, PointType::Telemetry);
    let value = rtdb
        .hash_get(&channel_key, "1")
        .await
        .unwrap()
        .map(|b| String::from_utf8(b.to_vec()).unwrap());
    assert_eq!(value.as_deref(), Some("2.2"));

    let ts_key = format!("{}:ts", channel_key);
    let ts = rtdb.hash_get(&ts_key, "1").await.unwrap();
    assert!(ts.is_some(), "Timestamp should exist");

    let raw_key = format!("{}:raw", channel_key);
    let raw = rtdb
        .hash_get(&raw_key, "1")
        .await
        .unwrap()
        .map(|b| String::from_utf8(b.to_vec()).unwrap());
    assert_eq!(raw.as_deref(), Some("22.0"));

    // Point 50 - use approximate comparison due to float precision
    let value = rtdb
        .hash_get(&channel_key, "50")
        .await
        .unwrap()
        .map(|b| String::from_utf8(b.to_vec()).unwrap())
        .and_then(|s| s.parse::<f64>().ok());
    assert!(
        (value.unwrap() - 110.0).abs() < 1e-10,
        "Value should be ~110.0"
    );

    let raw = rtdb
        .hash_get(&raw_key, "50")
        .await
        .unwrap()
        .map(|b| String::from_utf8(b.to_vec()).unwrap())
        .and_then(|s| s.parse::<f64>().ok());
    assert!(
        (raw.unwrap() - 1100.0).abs() < 1e-10,
        "Raw should be ~1100.0"
    );

    // Verify instance routing uses engineering value
    let value = verify_instance_value(rtdb.as_ref(), 10, 1).await;
    assert!((value.unwrap() - 2.2).abs() < 1e-10);

    let value = verify_instance_value(rtdb.as_ref(), 10, 50).await;
    assert!((value.unwrap() - 110.0).abs() < 1e-10);
}

// ============================================================================
// No Routing Stress Tests
// ============================================================================

#[tokio::test]
async fn test_batch_write_1000_points_no_routing() {
    // Test that writes succeed even without routing (channel data only)
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(RoutingCache::new()); // Empty routing

    let updates: Vec<_> = (1..=1000)
        .map(|point_id| ChannelPointUpdate {
            channel_id: 1001,
            point_type: PointType::Telemetry,
            point_id,
            value: point_id as f64,
            raw_value: None,
            cascade_depth: 0,
        })
        .collect();

    // Should succeed even without routing
    write_channel_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Verify channel data exists
    let value = verify_channel_value(rtdb.as_ref(), 1001, PointType::Telemetry, 500).await;
    assert_eq!(value, Some(500.0));

    // Verify no instance data (no routing)
    let value = verify_instance_value(rtdb.as_ref(), 10, 500).await;
    assert!(value.is_none());
}
