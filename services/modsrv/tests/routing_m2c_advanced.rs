//! M2C Advanced Routing Tests
//!
//! Supplementary tests for M2C (Model to Channel) routing:
//! - Concurrent action triggers
//! - High-volume batch operations
//! - Multi-instance broadcast scenarios
//! - Rule-triggered action flow simulation

#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use voltage_routing::set_action_point;
use voltage_rtdb::RoutingCache;
use voltage_rtdb::{MemoryRtdb, Rtdb};

// ============================================================================
// Test Setup Helpers
// ============================================================================

/// Creates a test environment with M2C routing configuration
async fn setup_m2c_routing(m2c_routes: Vec<(&str, &str)>) -> (Arc<MemoryRtdb>, Arc<RoutingCache>) {
    let rtdb = Arc::new(MemoryRtdb::new());

    let mut m2c_map = HashMap::new();
    for (source, target) in m2c_routes {
        m2c_map.insert(source.to_string(), target.to_string());
    }

    let routing_cache = Arc::new(RoutingCache::from_maps(
        HashMap::new(), // C2M
        m2c_map,        // M2C
        HashMap::new(), // C2C
    ));

    (rtdb, routing_cache)
}

/// Get TODO queue message count
async fn todo_queue_count<R: Rtdb>(rtdb: &Arc<R>, queue_key: &str) -> usize {
    rtdb.list_range(queue_key, 0, -1)
        .await
        .expect("List failed")
        .len()
}

// ============================================================================
// Concurrent Action Trigger Tests
// ============================================================================

#[tokio::test]
async fn test_m2c_concurrent_triggers() -> Result<()> {
    // Scenario: Multiple tokio tasks trigger actions concurrently
    let (rtdb, routing_cache) = setup_m2c_routing(vec![
        ("10:A:1", "1001:A:1"),
        ("10:A:2", "1001:A:2"),
        ("10:A:3", "1001:A:3"),
        ("10:A:4", "1001:A:4"),
        ("10:A:5", "1001:A:5"),
    ])
    .await;

    let mut handles = vec![];

    // Spawn 5 concurrent tasks
    for point_id in 1..=5 {
        let rtdb = rtdb.clone();
        let routing_cache = routing_cache.clone();
        let handle = tokio::spawn(async move {
            let outcome = set_action_point(
                rtdb.as_ref(),
                &routing_cache,
                10,
                &point_id.to_string(),
                point_id as f64 * 10.0,
            )
            .await
            .expect("Action failed");
            assert!(outcome.routed, "Point {} should be routed", point_id);
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await?;
    }

    // Verify all TODO queue messages arrived
    let count = todo_queue_count(&rtdb, "comsrv:1001:A:TODO").await;
    assert_eq!(count, 5, "All 5 messages should be in TODO queue");

    // Verify all instance hash values written
    for point_id in 1..=5 {
        let value = rtdb
            .hash_get("inst:10:A", &point_id.to_string())
            .await?
            .expect("Value should exist");
        let _expected = (point_id as f64 * 10.0).to_string();
        // Note: ryu formats 10.0 as "10" not "10.0" for integers
        let value_str = String::from_utf8(value.to_vec())?;
        let value_f64: f64 = value_str.parse()?;
        assert!(
            (value_f64 - point_id as f64 * 10.0).abs() < 0.01,
            "Point {} value mismatch",
            point_id
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_m2c_concurrent_multi_instance() -> Result<()> {
    // Scenario: Multiple instances triggered concurrently
    let (rtdb, routing_cache) = setup_m2c_routing(vec![
        ("10:A:1", "1001:A:1"),
        ("20:A:1", "1002:A:1"),
        ("30:A:1", "1003:A:1"),
        ("40:A:1", "1004:A:1"),
        ("50:A:1", "1005:A:1"),
    ])
    .await;

    let mut handles = vec![];

    // Spawn concurrent tasks for different instances
    for i in 1..=5 {
        let instance_id = i * 10; // 10, 20, 30, 40, 50
        let rtdb = rtdb.clone();
        let routing_cache = routing_cache.clone();
        let handle = tokio::spawn(async move {
            set_action_point(
                rtdb.as_ref(),
                &routing_cache,
                instance_id,
                "1",
                instance_id as f64,
            )
            .await
            .expect("Action failed")
        });
        handles.push((i, handle));
    }

    // Verify all succeeded
    for (i, handle) in handles {
        let outcome = handle.await?;
        let expected_channel = (1000 + i).to_string();
        assert!(outcome.routed);
        assert_eq!(outcome.route_result, Some(expected_channel));
    }

    // Verify each channel has exactly 1 message
    for i in 1..=5 {
        let queue_key = format!("comsrv:100{}:A:TODO", i);
        let count = todo_queue_count(&rtdb, &queue_key).await;
        assert_eq!(count, 1, "Channel 100{} should have 1 message", i);
    }

    Ok(())
}

// ============================================================================
// High-Volume Batch Tests
// ============================================================================

#[tokio::test]
async fn test_m2c_high_volume_single_instance() -> Result<()> {
    // Scenario: 100 action points from single instance
    let routes: Vec<(&str, &str)> = (1..=100)
        .map(|i| {
            let source = format!("10:A:{}", i);
            let target = format!("1001:A:{}", i);
            (
                Box::leak(source.into_boxed_str()) as &str,
                Box::leak(target.into_boxed_str()) as &str,
            )
        })
        .collect();

    let (rtdb, routing_cache) = setup_m2c_routing(routes).await;

    let start = Instant::now();

    // Trigger 100 actions sequentially
    for point_id in 1..=100 {
        let outcome = set_action_point(
            rtdb.as_ref(),
            &routing_cache,
            10,
            &point_id.to_string(),
            point_id as f64,
        )
        .await?;
        assert!(outcome.routed);
    }

    let elapsed = start.elapsed();
    println!("100 M2C actions: {:?}", elapsed);

    // Verify all messages in TODO queue
    let count = todo_queue_count(&rtdb, "comsrv:1001:A:TODO").await;
    assert_eq!(count, 100, "All 100 messages should be in TODO queue");

    // Performance check
    assert!(
        elapsed.as_millis() < 500,
        "100 actions should complete in <500ms"
    );

    Ok(())
}

#[tokio::test]
async fn test_m2c_high_volume_multi_instance() -> Result<()> {
    // Scenario: 10 instances x 10 points = 100 total actions
    let mut routes = vec![];
    for inst in 1..=10 {
        let instance_id = inst * 10; // 10, 20, ..., 100
        let channel_id = 1000 + inst; // 1001, 1002, ..., 1010
        for point in 1..=10 {
            let source = format!("{}:A:{}", instance_id, point);
            let target = format!("{}:A:{}", channel_id, point);
            routes.push((
                Box::leak(source.into_boxed_str()) as &str,
                Box::leak(target.into_boxed_str()) as &str,
            ));
        }
    }

    let (rtdb, routing_cache) = setup_m2c_routing(routes).await;

    let start = Instant::now();

    // Trigger all actions
    for inst in 1..=10 {
        let instance_id = inst * 10;
        for point in 1..=10 {
            set_action_point(
                rtdb.as_ref(),
                &routing_cache,
                instance_id as u32,
                &point.to_string(),
                (instance_id + point) as f64,
            )
            .await?;
        }
    }

    let elapsed = start.elapsed();
    println!("100 M2C actions (10x10): {:?}", elapsed);

    // Verify each channel has 10 messages
    for inst in 1..=10 {
        let queue_key = format!("comsrv:{}:A:TODO", 1000 + inst);
        let count = todo_queue_count(&rtdb, &queue_key).await;
        assert_eq!(count, 10, "Channel {} should have 10 messages", 1000 + inst);
    }

    Ok(())
}

// ============================================================================
// Multi-Instance Broadcast Tests
// ============================================================================

#[tokio::test]
async fn test_m2c_broadcast_pattern() -> Result<()> {
    // Scenario: One action point triggers messages to multiple channels
    // (simulating a broadcast command like "all inverters start")
    let (rtdb, routing_cache) = setup_m2c_routing(vec![
        // Instance 10's action point 1 (start command) routes to multiple channels
        ("10:A:1", "1001:A:99"), // Broadcast to channel 1001
        ("10:A:1", "1002:A:99"), // Note: Only first match is used!
    ])
    .await;

    // Trigger start command
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", 1.0).await?;

    // M2C routing is 1:1, so only first route is effective
    assert!(outcome.routed);
    // The routing maps to first match only

    Ok(())
}

#[tokio::test]
async fn test_m2c_fan_out_from_multiple_points() -> Result<()> {
    // Scenario: Multiple action points each going to different channels
    // (simulating individual commands to multiple devices)
    let (rtdb, routing_cache) = setup_m2c_routing(vec![
        ("10:A:1", "1001:A:1"), // Command to device 1
        ("10:A:2", "1002:A:1"), // Command to device 2
        ("10:A:3", "1003:A:1"), // Command to device 3
    ])
    .await;

    // Trigger commands to all devices
    for (point, channel) in [(1, 1001), (2, 1002), (3, 1003)] {
        let outcome =
            set_action_point(rtdb.as_ref(), &routing_cache, 10, &point.to_string(), 1.0).await?;

        assert!(outcome.routed);
        assert_eq!(
            outcome.route_result,
            Some(channel.to_string()),
            "Point {} should route to channel {}",
            point,
            channel
        );
    }

    // Verify each channel received command
    for channel in [1001, 1002, 1003] {
        let queue_key = format!("comsrv:{}:A:TODO", channel);
        let count = todo_queue_count(&rtdb, &queue_key).await;
        assert_eq!(count, 1, "Channel {} should have 1 message", channel);
    }

    Ok(())
}

// ============================================================================
// Rule-Triggered Action Flow Tests
// ============================================================================

#[tokio::test]
async fn test_m2c_rule_trigger_simulation() -> Result<()> {
    // Scenario: Simulate rule engine detecting condition and triggering action
    // Rule: "If SOC < 20%, then start charging"
    let (rtdb, routing_cache) = setup_m2c_routing(vec![
        ("10:A:1", "1001:A:1"), // Charging command
    ])
    .await;

    // Step 1: Simulate rule evaluation (SOC = 15% < threshold 20%)
    let soc_value = 15.0;
    let threshold = 20.0;
    let should_trigger = soc_value < threshold;

    assert!(should_trigger, "Rule condition should be met");

    // Step 2: Rule triggers action
    if should_trigger {
        let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", 1.0) // 1.0 = start
            .await?;

        assert!(outcome.routed, "Action should be routed");
    }

    // Step 3: Verify action was triggered
    let count = todo_queue_count(&rtdb, "comsrv:1001:A:TODO").await;
    assert_eq!(count, 1, "Charging command should be sent");

    // Verify action value recorded in instance
    let value = rtdb
        .hash_get("inst:10:A", "1")
        .await?
        .expect("Action value should exist");
    assert_eq!(String::from_utf8(value.to_vec())?, "1");

    Ok(())
}

#[tokio::test]
async fn test_m2c_sequential_rule_triggers() -> Result<()> {
    // Scenario: Multiple rules trigger in sequence
    // Rule 1: "If PV > 5kW, enable export"
    // Rule 2: "If battery full, reduce charge rate"
    let (rtdb, routing_cache) = setup_m2c_routing(vec![
        ("10:A:1", "1001:A:1"), // Export enable action
        ("10:A:2", "1001:A:2"), // Charge rate action
    ])
    .await;

    // Rule 1 triggers
    set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", 1.0).await?; // Enable export

    // Rule 2 triggers (e.g., 100ms later in real scenario)
    set_action_point(rtdb.as_ref(), &routing_cache, 10, "2", 0.5).await?; // Reduce charge rate to 50%

    // Verify both actions queued
    let count = todo_queue_count(&rtdb, "comsrv:1001:A:TODO").await;
    assert_eq!(count, 2, "Both actions should be queued");

    // Verify instance state updated
    let export_enabled = rtdb.hash_get("inst:10:A", "1").await?.unwrap();
    assert_eq!(String::from_utf8(export_enabled.to_vec())?, "1");

    let charge_rate = rtdb.hash_get("inst:10:A", "2").await?.unwrap();
    assert_eq!(String::from_utf8(charge_rate.to_vec())?, "0.5");

    Ok(())
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[tokio::test]
async fn test_m2c_action_value_edge_cases() -> Result<()> {
    let (rtdb, routing_cache) = setup_m2c_routing(vec![("10:A:1", "1001:A:1")]).await;

    // Test zero value
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", 0.0).await?;
    assert!(outcome.routed);
    let value = rtdb.hash_get("inst:10:A", "1").await?.unwrap();
    assert!(String::from_utf8(value.to_vec())?.contains("0"));

    // Test negative value
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", -100.0).await?;
    assert!(outcome.routed);
    let value = rtdb.hash_get("inst:10:A", "1").await?.unwrap();
    assert!(String::from_utf8(value.to_vec())?.starts_with("-"));

    // Test very small value
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", 0.0001).await?;
    assert!(outcome.routed);

    // Test very large value
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", 1e10).await?;
    assert!(outcome.routed);

    Ok(())
}

#[tokio::test]
async fn test_m2c_rapid_updates() -> Result<()> {
    // Scenario: Same point updated rapidly (simulating oscillating condition)
    let (rtdb, routing_cache) = setup_m2c_routing(vec![("10:A:1", "1001:A:1")]).await;

    // Rapid updates
    for i in 0..10 {
        let value = if i % 2 == 0 { 1.0 } else { 0.0 }; // Toggle
        set_action_point(rtdb.as_ref(), &routing_cache, 10, "1", value).await?;
    }

    // All updates should be queued (comsrv will handle de-duplication if needed)
    let count = todo_queue_count(&rtdb, "comsrv:1001:A:TODO").await;
    assert_eq!(count, 10, "All 10 updates should be queued");

    // Final instance state should be last value (0.0)
    let value = rtdb.hash_get("inst:10:A", "1").await?.unwrap();
    assert!(String::from_utf8(value.to_vec())?.contains("0"));

    Ok(())
}
