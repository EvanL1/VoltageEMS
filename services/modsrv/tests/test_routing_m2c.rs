//! M2C (Model to Channel) Routing End-to-End Tests
//!
//! Tests the complete data flow from instance action points to channel Hash writes

#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use anyhow::Result;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_routing::set_action_point;
use voltage_routing::RoutingCache;
use voltage_rtdb::{MemoryRtdb, Rtdb};

// ==================== Test Helper Functions ====================

/// Creates a test environment with M2C routing and instance mappings
///
/// # Arguments
/// * `m2c_routes` - M2C routing table [("23:A:1", "1001:A:1"), ...]
/// * `instance_mappings` - Instance name mappings [("inverter_01", 23), ...]
///
/// # Returns
/// (Rtdb instance, RoutingCache instance)
async fn setup_m2c_routing(
    m2c_routes: Vec<(&str, &str)>,
    instance_mappings: Vec<(&str, u32)>,
) -> (Arc<MemoryRtdb>, Arc<RoutingCache>) {
    let rtdb = Arc::new(MemoryRtdb::new());

    // Step 1: Set up instance name index (inst:name:index Hash)
    for (name, id) in instance_mappings {
        rtdb.hash_set("inst:name:index", name, Bytes::from(id.to_string()))
            .await
            .unwrap();
    }

    // Step 2: Configure M2C routing table
    let mut m2c_map = HashMap::new();
    for (source, target) in m2c_routes {
        m2c_map.insert(source.to_string(), target.to_string());
    }

    let routing_cache = Arc::new(RoutingCache::from_maps(
        HashMap::new(), // C2M routing (empty)
        m2c_map,        // M2C routing
        HashMap::new(), // C2C routing (empty)
    ));

    (rtdb, routing_cache)
}

// ==================== Test Cases ====================

/// Test 1: Basic M2C routing
///
/// Given: Configure routing 23:A:1 -> 1001:A:1, instance name "inverter_01" -> 23
/// When: Call set_action_point("inverter_01", "1", 12.3)
/// Then:
///   - Instance Action Hash written: inst:23:A["1"] = "12.3"
///   - Channel Hash written: comsrv:1001:A["1"] = "12.3"
///   - Routing result: routed=true, route_result=Some("1001")
#[tokio::test]
async fn test_m2c_basic_routing() -> Result<()> {
    // Given: Configure M2C routing and instance mapping
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![("23:A:1", "1001:A:1")], // M2C routing: Instance 23 action point 1 -> Channel 1001 adjustment point 1
        vec![("inverter_01", 23)],    // Instance name mapping
    )
    .await;

    // When: Set instance action point
    let outcome = set_action_point(
        rtdb.as_ref(),
        &routing_cache,
        23,   // Instance ID
        "1",  // Action point ID
        12.3, // Value
    )
    .await?;

    // Then: Verify routing result
    assert!(outcome.is_success(), "Routing should succeed");
    assert!(outcome.routed, "Action should be routed to channel");
    assert_eq!(
        outcome.route_result,
        Some("1001".to_string()),
        "Should route to channel 1001"
    );

    // Verify routing context
    let route_ctx = outcome.route_context.as_ref().unwrap();
    assert_eq!(route_ctx.channel_id, "1001");
    assert_eq!(route_ctx.point_type, "A");
    assert_eq!(route_ctx.comsrv_point_id, "1");

    // Verify instance Action Hash written
    let value = rtdb
        .hash_get("inst:23:A", "1")
        .await?
        .expect("Action point should be written");
    assert_eq!(
        String::from_utf8(value.to_vec())?,
        "12.3",
        "Action value should match"
    );

    // Verify channel Hash written
    let ch_value = rtdb
        .hash_get("comsrv:1001:A", "1")
        .await?
        .expect("Channel hash should be written");
    assert_eq!(
        String::from_utf8(ch_value.to_vec())?,
        "12.3",
        "Channel value should match"
    );

    Ok(())
}

/// Test 2: Instance name resolution
///
/// Given: Multiple instance name mappings
/// When: Call set_action_point with different instance names
/// Then: Correctly resolved to corresponding instance IDs
#[tokio::test]
async fn test_m2c_instance_name_resolution() -> Result<()> {
    // Given: Configure multiple instance mappings
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![
            ("10:A:1", "1001:A:1"),
            ("20:A:1", "1002:A:1"),
            ("30:A:1", "1003:A:1"),
        ],
        vec![
            ("pv_inverter", 10),
            ("battery_pack", 20),
            ("grid_meter", 30),
        ],
    )
    .await;

    // When & Then: Test first instance
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", 100.0).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1001".to_string()));

    // Verify written to correct instance Hash
    let value = rtdb.hash_get("inst:10:A", "1").await?.unwrap();
    assert_eq!(String::from_utf8(value.to_vec())?, "100");

    // When & Then: Test second instance
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 20, "1", 50.0).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1002".to_string()));

    let value = rtdb.hash_get("inst:20:A", "1").await?.unwrap();
    assert_eq!(String::from_utf8(value.to_vec())?, "50");

    // Test instance ID without routing config - should succeed but return no_route
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 9999, "1", 0.0).await?;
    assert!(!outcome.routed, "Should not be routed");
    assert_eq!(
        outcome.route_result,
        Some("no_route".to_string()),
        "Should indicate no route"
    );

    Ok(())
}

/// Test 3: No routing configuration
///
/// Given: No M2C routing configured
/// When: Call set_action_point
/// Then:
///   - Instance Action Hash still written
///   - Routing result: routed=false, route_result=Some("no_route")
#[tokio::test]
async fn test_m2c_no_routing() -> Result<()> {
    // Given: No M2C routing config
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![],                    // Empty routing table
        vec![("inverter_01", 23)], // Only instance mapping
    )
    .await;

    // When: Set action point
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 23, "1", 15.5).await?;

    // Then: Verify routing result
    assert!(outcome.is_success(), "Operation should succeed");
    assert!(!outcome.routed, "Should not be routed");
    assert_eq!(
        outcome.route_result,
        Some("no_route".to_string()),
        "Should indicate no route"
    );
    assert!(outcome.route_context.is_none(), "No route context");

    // Verify instance Action Hash still written
    let value = rtdb
        .hash_get("inst:23:A", "1")
        .await?
        .expect("Action point should still be written");
    assert_eq!(String::from_utf8(value.to_vec())?, "15.5");

    Ok(())
}

/// Test 4: Batch action trigger
///
/// Given: Multiple points M2C routing configured
/// When: Batch set multiple action points
/// Then: All channel Hashes written
#[tokio::test]
async fn test_m2c_batch_actions() -> Result<()> {
    // Given: Configure multiple point routing
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![
            ("23:A:1", "1001:A:1"),
            ("23:A:2", "1001:A:2"),
            ("23:A:3", "1001:A:3"),
        ],
        vec![("inverter_01", 23)],
    )
    .await;

    // When: Batch set action points
    let actions = vec![("1", 10.0), ("2", 20.0), ("3", 30.0)];

    for (point_id, value) in actions {
        let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 23, point_id, value).await?;
        assert!(outcome.routed, "Point {} should be routed", point_id);
    }

    // Then: Verify all points written to instance Hash
    for (point_id, expected_value) in [("1", "10"), ("2", "20"), ("3", "30")] {
        let value = rtdb.hash_get("inst:23:A", point_id).await?.unwrap();
        assert_eq!(
            String::from_utf8(value.to_vec())?,
            expected_value,
            "Point {} value mismatch",
            point_id
        );
    }

    // Verify channel Hash has all 3 points (f64 serialization includes decimal)
    for (point_id, expected_value) in [("1", "10.0"), ("2", "20.0"), ("3", "30.0")] {
        let ch_value = rtdb.hash_get("comsrv:1001:A", point_id).await?.unwrap();
        assert_eq!(
            String::from_utf8(ch_value.to_vec())?,
            expected_value,
            "Channel point {} value mismatch",
            point_id
        );
    }

    Ok(())
}

/// Test 5: Control/Adjustment routing (C/A types)
///
/// Given: Configure C(Control) and A(Adjustment) two types of routing
/// When: Set action points separately
/// Then: Route to comsrv:{channel_id}:C and :A Hash
#[tokio::test]
async fn test_m2c_different_channel_types() -> Result<()> {
    // Given: Configure control and adjustment routing
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![
            ("23:A:1", "1001:C:5"), // Action point 1 -> Control point 5
            ("23:A:2", "1001:A:6"), // Action point 2 -> Adjustment point 6
        ],
        vec![("inverter_01", 23)],
    )
    .await;

    // When: Set control type action point
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 23, "1", 1.0).await?;

    // Then: Verify routed to C(Control) Hash
    assert!(outcome.routed);
    let route_ctx = outcome.route_context.as_ref().unwrap();
    assert_eq!(route_ctx.point_type, "C", "Should route to Control type");
    let ch_value = rtdb.hash_get("comsrv:1001:C", "5").await?.unwrap();
    assert_eq!(String::from_utf8(ch_value.to_vec())?, "1.0");

    // When: Set adjustment type action point
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 23, "2", 2.0).await?;

    // Then: Verify routed to A(Adjustment) Hash
    assert!(outcome.routed);
    let route_ctx = outcome.route_context.as_ref().unwrap();
    assert_eq!(route_ctx.point_type, "A", "Should route to Adjustment type");
    let ch_value = rtdb.hash_get("comsrv:1001:A", "6").await?.unwrap();
    assert_eq!(String::from_utf8(ch_value.to_vec())?, "2.0");

    Ok(())
}

/// Test 6: Write execution order validation
///
/// Given: Configure M2C routing
/// When: Set action point
/// Then:
///   - Instance Hash written: inst:{id}:A
///   - Channel Hash written: comsrv:{ch}:A
///   - Both must succeed with consistent values
#[tokio::test]
async fn test_m2c_write_triggers_routing_order() -> Result<()> {
    // Given: Configure routing
    let (rtdb, routing_cache) =
        setup_m2c_routing(vec![("23:A:1", "1001:A:1")], vec![("inverter_01", 23)]).await;

    // When: Set action point
    set_action_point(rtdb.as_ref(), &routing_cache, 23, "1", 99.9).await?;

    // Then: Verify instance Hash written
    let hash_value = rtdb.hash_get("inst:23:A", "1").await?;
    assert!(hash_value.is_some(), "Instance Hash must be written");
    assert_eq!(String::from_utf8(hash_value.unwrap().to_vec())?, "99.9");

    // Verify channel Hash written with same value
    let ch_value = rtdb.hash_get("comsrv:1001:A", "1").await?;
    assert!(ch_value.is_some(), "Channel Hash must be written");
    assert_eq!(
        String::from_utf8(ch_value.unwrap().to_vec())?,
        "99.9",
        "Channel value should match instance value"
    );

    Ok(())
}

/// Test 8: Invalid route target handling
///
/// Given: Configure malformed route target (missing fields)
/// When: Set action point
/// Then:
///   - Invalid entries filtered at load time (fail-fast)
///   - lookup returns None, goes to no_route branch
///   - Operation succeeds but not routed
#[tokio::test]
async fn test_m2c_invalid_route_target() -> Result<()> {
    // Given: Configure invalid route target (malformed format)
    // Note: RoutingCache::from_maps filters out invalid entries at load time
    let rtdb = Arc::new(MemoryRtdb::new());
    rtdb.hash_set("inst:name:index", "inverter_01", Bytes::from("23"))
        .await?;

    let mut m2c_map = HashMap::new();
    m2c_map.insert("23:A:1".to_string(), "invalid_target".to_string()); // Wrong format, will be filtered
    let routing_cache = Arc::new(RoutingCache::from_maps(
        HashMap::new(),
        m2c_map,
        HashMap::new(),
    ));

    // When: Set action point
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 23, "1", 50.0).await?;

    // Then: Operation succeeds but not routed (invalid entry filtered at load time)
    assert!(outcome.is_success(), "Operation should succeed");
    assert!(!outcome.routed, "Routing should fail for invalid target");

    // Verify route_result indicates no route (invalid entries filtered at load time)
    if let Some(route_result) = &outcome.route_result {
        assert_eq!(
            route_result, "no_route",
            "Should indicate no_route since invalid entries are filtered at load time"
        );
    } else {
        panic!("Expected route_result with no_route, got None");
    }

    Ok(())
}

/// Test 9: Multiple instances multiple channels routing
///
/// Given: Multiple instances routed to different channels
/// When: Batch set different instance action points
/// Then: Each instance correctly routed to corresponding channel
#[tokio::test]
async fn test_m2c_multiple_instances_multiple_channels() -> Result<()> {
    // Given: Configure multi-instance multi-channel routing
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![
            ("10:A:1", "1001:A:1"), // Instance 10 -> Channel 1001
            ("20:A:1", "1002:A:1"), // Instance 20 -> Channel 1002
            ("30:A:1", "1003:A:1"), // Instance 30 -> Channel 1003
        ],
        vec![("inverter_a", 10), ("inverter_b", 20), ("inverter_c", 30)],
    )
    .await;

    // When & Then: Test instance A -> Channel 1001
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", 111.1).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1001".to_string()));

    // When & Then: Test instance B -> Channel 1002
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 20, "1", 222.2).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1002".to_string()));

    // When & Then: Test instance C -> Channel 1003
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 30, "1", 333.3).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1003".to_string()));

    // Verify all three instance Hashes written correctly
    assert_eq!(
        String::from_utf8(rtdb.hash_get("inst:10:A", "1").await?.unwrap().to_vec())?,
        "111.1"
    );
    assert_eq!(
        String::from_utf8(rtdb.hash_get("inst:20:A", "1").await?.unwrap().to_vec())?,
        "222.2"
    );
    assert_eq!(
        String::from_utf8(rtdb.hash_get("inst:30:A", "1").await?.unwrap().to_vec())?,
        "333.3"
    );

    Ok(())
}
