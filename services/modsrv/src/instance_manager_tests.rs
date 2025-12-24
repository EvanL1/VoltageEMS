#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use super::*;
use std::collections::HashMap;
use tempfile::TempDir;

// Helper: Create test database with all required tables
async fn create_test_database() -> (TempDir, SqlitePool) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_instance_manager.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let pool = SqlitePool::connect(&db_url).await.unwrap();

    // Use standard modsrv schema from common test utils
    common::test_utils::schema::init_modsrv_schema(&pool)
        .await
        .unwrap();

    (temp_dir, pool)
}

// Helper: Create test product with points
async fn create_test_product(pool: &SqlitePool, product_name: &str) {
    // Insert product
    sqlx::query("INSERT INTO products (product_name, parent_name) VALUES (?, NULL)")
        .bind(product_name)
        .execute(pool)
        .await
        .unwrap();

    // Insert measurement points
    sqlx::query(
        "INSERT INTO measurement_points (product_name, measurement_id, name, unit) VALUES (?, ?, ?, ?)"
    )
    .bind(product_name)
    .bind(1)
    .bind("Temperature")
    .bind("°C")
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO measurement_points (product_name, measurement_id, name, unit) VALUES (?, ?, ?, ?)"
    )
    .bind(product_name)
    .bind(2)
    .bind("Voltage")
    .bind("V")
    .execute(pool)
    .await
    .unwrap();

    // Insert action point
    sqlx::query("INSERT INTO action_points (product_name, action_id, name) VALUES (?, ?, ?)")
        .bind(product_name)
        .bind(1)
        .bind("SetPower")
        .execute(pool)
        .await
        .unwrap();
}

// Helper: Create test ProductLoader
fn create_test_product_loader(pool: SqlitePool) -> Arc<ProductLoader> {
    Arc::new(crate::product_loader::ProductLoader::new(pool))
}

use voltage_rtdb::helpers::create_test_rtdb;

// ==================== Phase 1: CRUD Core Tests ====================

#[tokio::test]
async fn test_instance_manager_new() {
    let (_temp_dir, pool) = create_test_database().await;
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();

    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let _manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    // Test passes if InstanceManager::new() doesn't panic
}

#[tokio::test]
async fn test_create_instance_success() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    let req = CreateInstanceRequest {
        instance_id: 1001,
        instance_name: "test_instance_01".to_string(),
        product_name: "test_product".to_string(),
        properties: HashMap::new(),
    };

    let result = manager.create_instance(req).await;

    assert!(
        result.is_ok(),
        "Instance creation should succeed: {:?}",
        result.as_ref().err()
    );
    let instance = result.unwrap();
    assert_eq!(instance.instance_id(), 1001);
    assert_eq!(instance.instance_name(), "test_instance_01");
    assert_eq!(instance.product_name(), "test_product");
}

#[tokio::test]
async fn test_create_instance_with_properties() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "solar_panel").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    let mut properties = HashMap::new();
    properties.insert("location".to_string(), serde_json::json!("Roof A"));
    properties.insert("capacity".to_string(), serde_json::json!(5000));

    let req = CreateInstanceRequest {
        instance_id: 2001,
        instance_name: "solar_panel_01".to_string(),
        product_name: "solar_panel".to_string(),
        properties,
    };

    let result = manager.create_instance(req).await;

    assert!(result.is_ok());
    let instance = result.unwrap();
    assert_eq!(instance.core.properties.len(), 2);
    assert_eq!(
        instance.core.properties.get("location").unwrap(),
        &serde_json::json!("Roof A")
    );
    assert_eq!(
        instance.core.properties.get("capacity").unwrap(),
        &serde_json::json!(5000)
    );
}

#[tokio::test]
async fn test_create_instance_already_exists() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    let req = CreateInstanceRequest {
        instance_id: 1001,
        instance_name: "duplicate_instance".to_string(),
        product_name: "test_product".to_string(),
        properties: HashMap::new(),
    };

    // First creation should succeed
    let result1 = manager.create_instance(req.clone()).await;
    assert!(result1.is_ok());

    // Second creation with same ID should fail
    let result2 = manager.create_instance(req).await;
    assert!(result2.is_err());
    assert!(result2.unwrap_err().to_string().contains("already exists"));
}

#[tokio::test]
async fn test_create_instance_product_not_found() {
    let (_temp_dir, pool) = create_test_database().await;
    // Don't create the product

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    let req = CreateInstanceRequest {
        instance_id: 3001,
        instance_name: "orphan_instance".to_string(),
        product_name: "nonexistent_product".to_string(),
        properties: HashMap::new(),
    };

    let result = manager.create_instance(req).await;

    assert!(result.is_err(), "Should fail when product doesn't exist");
}

#[tokio::test]
async fn test_list_instances_all() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "product_a").await;
    create_test_product(&pool, "product_b").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    // Create multiple instances
    let req1 = CreateInstanceRequest {
        instance_id: 1001,
        instance_name: "instance_01".to_string(),
        product_name: "product_a".to_string(),
        properties: HashMap::new(),
    };
    manager.create_instance(req1).await.unwrap();

    let req2 = CreateInstanceRequest {
        instance_id: 1002,
        instance_name: "instance_02".to_string(),
        product_name: "product_b".to_string(),
        properties: HashMap::new(),
    };
    manager.create_instance(req2).await.unwrap();

    // List all instances
    let instances = manager.list_instances(None).await.unwrap();

    assert_eq!(instances.len(), 2);
}

#[tokio::test]
async fn test_list_instances_by_product() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "product_a").await;
    create_test_product(&pool, "product_b").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    // Create instances for different products
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "product_a_instance_01".to_string(),
            product_name: "product_a".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1002,
            instance_name: "product_a_instance_02".to_string(),
            product_name: "product_a".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 2001,
            instance_name: "product_b_instance_01".to_string(),
            product_name: "product_b".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    // List instances for product_a only
    let instances = manager.list_instances(Some("product_a")).await.unwrap();

    assert_eq!(instances.len(), 2);
    assert!(instances.iter().all(|i| i.product_name() == "product_a"));
}

#[tokio::test]
async fn test_get_instance_success() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    // Create instance
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "get_test_instance".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    // Get instance by ID
    let result = manager.get_instance(1001).await;

    assert!(
        result.is_ok(),
        "Failed to get instance: {:?}",
        result.as_ref().err()
    );
    let instance = result.unwrap();
    assert_eq!(instance.instance_id(), 1001);
    assert_eq!(instance.instance_name(), "get_test_instance");
}

#[tokio::test]
async fn test_get_instance_not_found() {
    let (_temp_dir, pool) = create_test_database().await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    let result = manager.get_instance(9999).await;

    assert!(result.is_err(), "Should fail when instance doesn't exist");
}

#[tokio::test]
async fn test_delete_instance() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    // Create instance
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "delete_test".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    // Delete instance by ID
    let result = manager.delete_instance(1001).await;
    assert!(result.is_ok());

    // Verify it's deleted
    let get_result = manager.get_instance(1001).await;
    assert!(get_result.is_err());
}

// ==================== Phase 2: M2C Routing Tests (execute_action) ====================

/// Helper: Setup instance name index in RTDB (required for voltage-routing)
async fn setup_instance_name_index(
    rtdb: &voltage_rtdb::MemoryRtdb,
    instance_id: u32,
    instance_name: &str,
) {
    use bytes::Bytes;
    use voltage_rtdb::Rtdb;
    rtdb.hash_set(
        "inst:name:index",
        instance_name,
        Bytes::from(instance_id.to_string()),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_execute_action_instance_not_found() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader);

    // Execute action on non-existent instance - this now succeeds
    // but doesn't route (no route configured for instance 9999)
    // The value is stored locally to inst:9999:A hash
    let result = manager.execute_action(9999, "1", 100.0).await;

    // Current behavior: succeeds and stores locally (not routed)
    assert!(
        result.is_ok(),
        "execute_action should succeed even for unconfigured instance (stores locally)"
    );
}

#[tokio::test]
async fn test_execute_action_no_route_stores_locally() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

    // Create instance
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "action_test_instance".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    // Setup instance name index
    setup_instance_name_index(&rtdb, 1001, "action_test_instance").await;

    // Execute action (no M2C route configured, should store locally)
    let result = manager.execute_action(1001, "1", 50.0).await;

    assert!(
        result.is_ok(),
        "execute_action should succeed even without route: {:?}",
        result.as_ref().err()
    );

    // Verify value was stored in instance action key
    use voltage_rtdb::Rtdb;
    let config = voltage_rtdb::KeySpaceConfig::production();
    let action_key = config.instance_action_key(1001);
    let stored = rtdb.hash_get(&action_key, "1").await.unwrap();
    assert!(stored.is_some(), "Action value should be stored");
    assert_eq!(stored.unwrap().as_ref(), b"50");
}

#[tokio::test]
async fn test_execute_action_with_route_triggers_downstream() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();

    // Configure M2C route: instance 1001, action point "1" -> channel 2, A, point 5
    let mut m2c_data = HashMap::new();
    m2c_data.insert("1001:A:1".to_string(), "2:A:5".to_string());
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::from_maps(
        HashMap::new(),
        m2c_data,
        HashMap::new(),
    ));

    let manager = InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

    // Create instance
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "routed_action_instance".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    // Setup instance name index
    setup_instance_name_index(&rtdb, 1001, "routed_action_instance").await;

    // Execute action (M2C route configured)
    let result = manager.execute_action(1001, "1", 75.0).await;

    assert!(
        result.is_ok(),
        "execute_action should succeed with route: {:?}",
        result.as_ref().err()
    );

    // Verify instance action hash was updated
    use voltage_rtdb::Rtdb;
    let config = voltage_rtdb::KeySpaceConfig::production();
    let action_key = config.instance_action_key(1001);
    let stored = rtdb.hash_get(&action_key, "1").await.unwrap();
    assert!(stored.is_some(), "Instance action should be stored");
    assert_eq!(stored.unwrap().as_ref(), b"75");

    // Verify channel hash was updated
    use voltage_model::PointType;
    let channel_key = config.channel_key(2, PointType::Adjustment);
    let channel_value = rtdb.hash_get(&channel_key, "5").await.unwrap();
    assert!(channel_value.is_some(), "Channel point should be updated");

    // Verify TODO queue was triggered (check if there are any entries)
    let todo_key = config.todo_queue_key(2, PointType::Adjustment);
    let queue_entries = rtdb.list_range(&todo_key, 0, -1).await.unwrap();
    assert!(!queue_entries.is_empty(), "TODO queue should have entry");
}

#[tokio::test]
async fn test_execute_action_multiple_points() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();

    // Configure multiple routes
    let mut m2c_data = HashMap::new();
    m2c_data.insert("1001:A:1".to_string(), "10:A:1".to_string());
    m2c_data.insert("1001:A:2".to_string(), "10:A:2".to_string());
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::from_maps(
        HashMap::new(),
        m2c_data,
        HashMap::new(),
    ));

    let manager = InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "multi_action_instance".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    setup_instance_name_index(&rtdb, 1001, "multi_action_instance").await;

    // Execute multiple actions
    let result1 = manager.execute_action(1001, "1", 10.0).await;
    let result2 = manager.execute_action(1001, "2", 20.0).await;

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    // Verify both actions were stored
    use voltage_rtdb::Rtdb;
    let config = voltage_rtdb::KeySpaceConfig::production();
    let action_key = config.instance_action_key(1001);
    let v1 = rtdb.hash_get(&action_key, "1").await.unwrap().unwrap();
    let v2 = rtdb.hash_get(&action_key, "2").await.unwrap().unwrap();
    assert_eq!(v1.as_ref(), b"10");
    assert_eq!(v2.as_ref(), b"20");
}

#[tokio::test]
async fn test_execute_action_value_overwrite() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "overwrite_test".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    setup_instance_name_index(&rtdb, 1001, "overwrite_test").await;

    // Execute action twice with different values
    manager.execute_action(1001, "1", 100.0).await.unwrap();
    manager.execute_action(1001, "1", 200.0).await.unwrap();

    // Verify latest value wins
    use voltage_rtdb::Rtdb;
    let config = voltage_rtdb::KeySpaceConfig::production();
    let action_key = config.instance_action_key(1001);
    let stored = rtdb.hash_get(&action_key, "1").await.unwrap().unwrap();
    assert_eq!(stored.as_ref(), b"200", "Latest value should overwrite");
}

#[tokio::test]
async fn test_execute_action_negative_values() {
    let (_temp_dir, pool) = create_test_database().await;
    create_test_product(&pool, "test_product").await;

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
    let manager = InstanceManager::new(pool.clone(), rtdb.clone(), routing_cache, product_loader);

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: 1001,
            instance_name: "negative_test".to_string(),
            product_name: "test_product".to_string(),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    setup_instance_name_index(&rtdb, 1001, "negative_test").await;

    // Execute action with negative value
    let result = manager.execute_action(1001, "1", -50.5).await;
    assert!(result.is_ok());

    // Verify negative value was stored correctly
    use voltage_rtdb::Rtdb;
    let config = voltage_rtdb::KeySpaceConfig::production();
    let action_key = config.instance_action_key(1001);
    let stored = rtdb.hash_get(&action_key, "1").await.unwrap().unwrap();
    assert_eq!(stored.as_ref(), b"-50.5");
}
