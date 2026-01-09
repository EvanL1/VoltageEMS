//! Instance Lifecycle Integration Tests
//!
//! Tests the complete instance lifecycle: create → query → update → delete
//!
//! Note: Products are now compile-time built-in constants from voltage-model crate.
//! Use built-in product names like "Battery", "PCS", "ESS", "Station", etc.

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable

mod common;

use anyhow::Result;
use common::{fixtures, helpers, TestEnv};
use modsrv::instance_manager::InstanceManager;
use modsrv::product_loader::{CreateInstanceRequest, ProductLoader};
use std::sync::Arc;
use voltage_rtdb::RoutingCache;

#[tokio::test]
async fn test_create_instance_full_flow() -> Result<()> {
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
    );

    // 4. Create instance
    let req = CreateInstanceRequest {
        instance_id: 1001,
        instance_name: "battery_001".to_string(),
        product_name: product_name.to_string(),
        properties: fixtures::create_test_instance_properties(),
    };

    let instance = instance_manager.create_instance(req).await?;

    // 5. Verify instance created successfully
    assert_eq!(instance.instance_id(), 1001);
    assert_eq!(instance.instance_name(), "battery_001");
    assert_eq!(instance.product_name(), product_name);

    // 6. Verify database record
    assert!(helpers::assert_instance_exists(env.pool(), 1001).await?);

    // 7. Cleanup
    env.cleanup().await?;

    Ok(())
}

#[tokio::test]
async fn test_create_instance_duplicate_error() -> Result<()> {
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
    );

    // Create first instance
    let req = CreateInstanceRequest {
        instance_id: 1001,
        instance_name: "battery_001".to_string(),
        product_name: product_name.to_string(),
        properties: fixtures::create_test_instance_properties(),
    };
    instance_manager.create_instance(req.clone()).await?;

    // Try to create instance with the same name, should fail (UNIQUE constraint)
    let result = instance_manager.create_instance(req).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("UNIQUE constraint") || err_msg.contains("already exists"),
        "Expected UNIQUE constraint or already exists error, got: {}",
        err_msg
    );

    env.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_get_instance_data() -> Result<()> {
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
    );

    // Create instance
    let req = CreateInstanceRequest {
        instance_id: 1001,
        instance_name: "battery_001".to_string(),
        product_name: product_name.to_string(),
        properties: fixtures::create_test_instance_properties(),
    };
    let instance = instance_manager.create_instance(req).await?;

    // Simulate writing measurement data to Redis
    let m_key = format!("modsrv:{}:M", instance.instance_name());
    env.redis().hset(&m_key, "1", "100.5".to_string()).await?;
    env.redis().hset(&m_key, "2", "50.2".to_string()).await?;

    // Get instance data by ID
    let data = instance_manager
        .get_instance_data(instance.instance_id(), None)
        .await?;

    // Verify data (returned as JSON)
    assert!(data.is_object(), "Response should be JSON object");

    // Print returned data for debugging
    println!(
        "Returned data: {}",
        serde_json::to_string_pretty(&data).unwrap()
    );

    // Only verify that a data object is returned; do not depend on specific structure
    let obj = data.as_object().unwrap();
    assert!(!obj.is_empty(), "Data object should not be empty");

    env.cleanup().await?;
    Ok(())
}
