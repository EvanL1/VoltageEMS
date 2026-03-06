//! Routing Integration Tests
//!
//! Tests the complete data flow from channels to instances
//!
//! Note: Products are now compile-time built-in constants from voltage-model crate.
//! Use built-in product names like "Battery", "PCS", "ESS", "Station", etc.

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable

mod common;

use anyhow::Result;
use common::{fixtures, TestEnv};
use modsrv::instance_manager::InstanceManager;
use modsrv::product_loader::{CreateInstanceRequest, ProductLoader};
use std::sync::Arc;
use voltage_routing::RoutingCache;

fn noop_dispatch() -> Arc<dyn modsrv::infra::shm_dispatch::ActionDispatch> {
    Arc::new(modsrv::infra::shm_dispatch::NoopDispatch)
}

#[tokio::test]
#[ignore] // requires Redis
async fn test_measurement_routing_load_from_db() -> Result<()> {
    // 1. Create test environment
    let env = TestEnv::create().await?;

    // 2. Use built-in product "Battery" instead of creating test product
    let product_name = "Battery";

    // 3. Create ProductLoader and InstanceManager
    let product_loader = Arc::new(ProductLoader::new(env.pool().clone()));

    let redis_client = env.redis().clone();
    let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client.clone()));
    let routing_cache = Arc::new(RoutingCache::new());
    let instance_manager = InstanceManager::new(
        env.pool().clone(),
        rtdb.clone(),
        routing_cache,
        product_loader,
        noop_dispatch(),
    );

    // 4. Setup hierarchy: Station -> ESS (required for Battery)
    let station_req = CreateInstanceRequest {
        instance_id: Some(9901),
        instance_name: "test_station_root".to_string(),
        product_name: "Station".to_string(),
        parent_id: None,
        properties: std::collections::HashMap::new(),
    };
    instance_manager.create_instance(station_req).await?;

    let ess_req = CreateInstanceRequest {
        instance_id: Some(9902),
        instance_name: "test_ess_parent".to_string(),
        product_name: "ESS".to_string(),
        parent_id: Some(9901),
        properties: std::collections::HashMap::new(),
    };
    instance_manager.create_instance(ess_req).await?;

    // 5. Create Battery instance (child of ESS)
    let req = CreateInstanceRequest {
        instance_id: Some(1001),
        instance_name: "battery_001".to_string(),
        product_name: product_name.to_string(),
        parent_id: Some(9902),
        properties: fixtures::create_test_instance_properties(),
    };
    instance_manager.create_instance(req).await?;

    // 6. Create test channel (required by FK constraint in unified database architecture)
    sqlx::query(
        r#"
        INSERT INTO channels (channel_id, name, protocol, enabled)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(3001)
    .bind("test_channel_3001")
    .bind("Virtual")
    .bind(true)
    .execute(env.pool())
    .await?;

    // 6. Create a measurement routing record
    sqlx::query(
        r#"
        INSERT INTO measurement_routing
        (instance_id, instance_name, channel_id, channel_type, channel_point_id, measurement_id)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(1001)
    .bind("battery_001")
    .bind(3001) // channel_id
    .bind("T")  // telemetry
    .bind(1)    // channel point 1
    .bind(1)    // maps to measurement point 1
    .execute(env.pool())
    .await?;

    // 7. Verify the routing record is created
    let routing_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM measurement_routing WHERE instance_id = ?")
            .bind(1001)
            .fetch_one(env.pool())
            .await?;

    assert_eq!(routing_count, 1, "Should have 1 routing record");

    env.cleanup().await?;
    Ok(())
}

#[tokio::test]
#[ignore] // requires Redis
async fn test_action_routing_load_from_db() -> Result<()> {
    let env = TestEnv::create().await?;

    // Use built-in product "Battery"
    let product_name = "Battery";

    let product_loader = Arc::new(ProductLoader::new(env.pool().clone()));

    let redis_client = env.redis().clone();
    let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client.clone()));
    let routing_cache = Arc::new(RoutingCache::new());
    let instance_manager = InstanceManager::new(
        env.pool().clone(),
        rtdb.clone(),
        routing_cache,
        product_loader,
        noop_dispatch(),
    );

    // Setup hierarchy: Station -> ESS (required for Battery)
    let station_req = CreateInstanceRequest {
        instance_id: Some(9901),
        instance_name: "test_station_root".to_string(),
        product_name: "Station".to_string(),
        parent_id: None,
        properties: std::collections::HashMap::new(),
    };
    instance_manager.create_instance(station_req).await?;

    let ess_req = CreateInstanceRequest {
        instance_id: Some(9902),
        instance_name: "test_ess_parent".to_string(),
        product_name: "ESS".to_string(),
        parent_id: Some(9901),
        properties: std::collections::HashMap::new(),
    };
    instance_manager.create_instance(ess_req).await?;

    let req = CreateInstanceRequest {
        instance_id: Some(1001),
        instance_name: "battery_001".to_string(),
        product_name: product_name.to_string(),
        parent_id: Some(9902),
        properties: fixtures::create_test_instance_properties(),
    };
    instance_manager.create_instance(req).await?;

    // Create test channel (required by FK constraint in unified database architecture)
    sqlx::query(
        r#"
        INSERT INTO channels (channel_id, name, protocol, enabled)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(3001)
    .bind("test_channel_3001")
    .bind("Virtual")
    .bind(true)
    .execute(env.pool())
    .await?;

    // Create an action routing record
    sqlx::query(
        r#"
        INSERT INTO action_routing
        (instance_id, instance_name, action_id, channel_id, channel_type, channel_point_id)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(1001)
    .bind("battery_001")
    .bind(1)    // action point 1
    .bind(3001) // channel_id
    .bind("C")  // control
    .bind(1)    // channel point 1
    .execute(env.pool())
    .await?;

    // Verify the routing record
    let routing_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM action_routing WHERE instance_id = ?")
            .bind(1001)
            .fetch_one(env.pool())
            .await?;

    assert_eq!(routing_count, 1, "Should have 1 action routing record");

    env.cleanup().await?;
    Ok(())
}

#[tokio::test]
#[ignore] // requires Redis
async fn test_multiple_routing_for_instance() -> Result<()> {
    let env = TestEnv::create().await?;

    // Use built-in product "Battery"
    let product_name = "Battery";

    let product_loader = Arc::new(ProductLoader::new(env.pool().clone()));

    let redis_client = env.redis().clone();
    let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client.clone()));
    let routing_cache = Arc::new(RoutingCache::new());
    let instance_manager = InstanceManager::new(
        env.pool().clone(),
        rtdb.clone(),
        routing_cache,
        product_loader,
        noop_dispatch(),
    );

    // Setup hierarchy: Station -> ESS (required for Battery)
    let station_req = CreateInstanceRequest {
        instance_id: Some(9901),
        instance_name: "test_station_root".to_string(),
        product_name: "Station".to_string(),
        parent_id: None,
        properties: std::collections::HashMap::new(),
    };
    instance_manager.create_instance(station_req).await?;

    let ess_req = CreateInstanceRequest {
        instance_id: Some(9902),
        instance_name: "test_ess_parent".to_string(),
        product_name: "ESS".to_string(),
        parent_id: Some(9901),
        properties: std::collections::HashMap::new(),
    };
    instance_manager.create_instance(ess_req).await?;

    let req = CreateInstanceRequest {
        instance_id: Some(1001),
        instance_name: "battery_001".to_string(),
        product_name: product_name.to_string(),
        parent_id: Some(9902),
        properties: fixtures::create_test_instance_properties(),
    };
    instance_manager.create_instance(req).await?;

    // Create test channel (required by FK constraint in unified database architecture)
    sqlx::query(
        r#"
        INSERT INTO channels (channel_id, name, protocol, enabled)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(3001)
    .bind("test_channel_3001")
    .bind("Virtual")
    .bind(true)
    .execute(env.pool())
    .await?;

    // Create multiple measurement routings
    for channel_point_id in 1..=3 {
        sqlx::query(
            r#"
            INSERT INTO measurement_routing
            (instance_id, instance_name, channel_id, channel_type, channel_point_id, measurement_id)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(1001)
        .bind("battery_001")
        .bind(3001)
        .bind("T")
        .bind(channel_point_id)
        .bind(channel_point_id) // 1:1 mapping
        .execute(env.pool())
        .await?;
    }

    // Verify multiple routing records
    let routing_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM measurement_routing WHERE instance_id = ?")
            .bind(1001)
            .fetch_one(env.pool())
            .await?;

    assert_eq!(routing_count, 3, "Should have 3 routing records");

    env.cleanup().await?;
    Ok(())
}
