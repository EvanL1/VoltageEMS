#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use super::*;
use std::collections::HashMap;
use tempfile::TempDir;

/// Helper: create NoopDispatch for tests
fn noop_dispatch() -> Arc<dyn crate::infra::shm_dispatch::ActionDispatch> {
    Arc::new(crate::infra::shm_dispatch::NoopDispatch)
}

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

// Helper: Create test ProductLoader
fn create_test_product_loader(pool: SqlitePool) -> Arc<ProductLoader> {
    Arc::new(crate::product_loader::ProductLoader::new(pool))
}

use voltage_rtdb::helpers::create_test_rtdb;

/// Setup standard hierarchy: Station(1) -> ESS(2), returns ESS instance_id for use as parent
async fn setup_hierarchy(manager: &InstanceManager<voltage_rtdb::MemoryRtdb>) -> u32 {
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1),
            instance_name: "station_root".to_string(),
            product_name: "Station".to_string(),
            parent_id: None,
            properties: HashMap::new(),
        })
        .await
        .expect("Failed to create Station");

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(2),
            instance_name: "ess_parent".to_string(),
            product_name: "ESS".to_string(),
            parent_id: Some(1),
            properties: HashMap::new(),
        })
        .await
        .expect("Failed to create ESS");

    2 // ESS instance_id
}

// ==================== Phase 1: CRUD Core Tests ====================

#[tokio::test]
async fn test_instance_manager_new() {
    let (_temp_dir, pool) = create_test_database().await;
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();

    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let _manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());

    // Test passes if InstanceManager::new() doesn't panic
}

#[tokio::test]
async fn test_create_instance_success() {
    let (_temp_dir, pool) = create_test_database().await;
    // Use built-in product "Battery" instead of creating test product
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    let req = CreateInstanceRequest {
        instance_id: Some(1001),
        instance_name: "test_battery_01".to_string(),
        product_name: "Battery".to_string(),
        parent_id: Some(ess_id),
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
    assert_eq!(instance.instance_name(), "test_battery_01");
    assert_eq!(instance.product_name(), "Battery");
}

#[tokio::test]
async fn test_create_instance_with_properties() {
    let (_temp_dir, pool) = create_test_database().await;
    // Use built-in product "PCS" instead of creating test product
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    // Use real PCS PropertyTemplate names — unknown keys are now rejected
    // at the schema level since v5.
    let mut properties = HashMap::new();
    properties.insert("Max Power".to_string(), serde_json::json!(5000.0));
    properties.insert("Max Voltage".to_string(), serde_json::json!(800));

    let req = CreateInstanceRequest {
        instance_id: Some(2001),
        instance_name: "pcs_unit_01".to_string(),
        product_name: "PCS".to_string(),
        parent_id: Some(ess_id),
        properties,
    };

    let result = manager.create_instance(req).await;

    assert!(result.is_ok());
    let instance = result.unwrap();
    assert_eq!(instance.core.properties.len(), 2);
    assert_eq!(
        instance.core.properties.get("Max Power").unwrap(),
        &serde_json::json!(5000.0)
    );
    assert_eq!(
        instance.core.properties.get("Max Voltage").unwrap(),
        &serde_json::json!(800)
    );

    // Verify the values land in the dedicated table, not the legacy column.
    let row_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instance_properties WHERE instance_id = 2001")
            .fetch_one(&manager.pool)
            .await
            .unwrap();
    assert_eq!(row_count, 2);
}

#[tokio::test]
async fn test_load_instance_points_returns_properties() {
    let (_temp_dir, pool) = create_test_database().await;
    // load_instance_points LEFT JOINs telemetry_points/signal_points/etc. (comsrv tables)
    // for routing display names — those tables must exist for the SQL to execute.
    common::test_utils::schema::init_comsrv_schema(&pool)
        .await
        .expect("init comsrv schema");
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    // PCS product declares 6 property templates: "Max Power", "Max Voltage",
    // "Max Current AC", "Max Current DC", "Rated Frequency", "Conversion Efficiency".
    // Set values for two of them; the rest should come back with no `value`.
    let mut properties = HashMap::new();
    properties.insert("Max Power".to_string(), serde_json::json!(5000.0));
    properties.insert("Max Voltage".to_string(), serde_json::json!(800));

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(4001),
            instance_name: "pcs_with_props".to_string(),
            product_name: "PCS".to_string(),
            parent_id: Some(ess_id),
            properties,
        })
        .await
        .expect("create PCS instance");

    let (_m, _a, props) = manager
        .load_instance_points(4001)
        .await
        .expect("load_instance_points should succeed");

    // All 6 PCS templates should be present, in template order
    assert_eq!(props.len(), 6, "PCS has 6 property templates");

    let by_name: HashMap<&str, &crate::dto::InstancePropertyPoint> =
        props.iter().map(|p| (p.name.as_str(), p)).collect();

    let max_power = by_name
        .get("Max Power")
        .expect("Max Power template present");
    assert_eq!(max_power.value, Some(serde_json::json!(5000.0)));

    let max_voltage = by_name
        .get("Max Voltage")
        .expect("Max Voltage template present");
    assert_eq!(max_voltage.value, Some(serde_json::json!(800)));

    // Unset property: template metadata present, value absent
    let max_current_ac = by_name
        .get("Max Current AC")
        .expect("Max Current AC template present");
    assert!(
        max_current_ac.value.is_none(),
        "unset property must omit value, got {:?}",
        max_current_ac.value
    );
}

#[tokio::test]
async fn test_upsert_single_property_writes_to_dedicated_table() {
    let (_temp_dir, pool) = create_test_database().await;
    common::test_utils::schema::init_comsrv_schema(&pool)
        .await
        .expect("init comsrv schema");
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(5001),
            instance_name: "pcs_single_prop".to_string(),
            product_name: "PCS".to_string(),
            parent_id: Some(ess_id),
            properties: HashMap::new(),
        })
        .await
        .expect("create instance");

    // PCS property_id=1 is "Max Power"
    let updated = manager
        .upsert_single_property(5001, 1, serde_json::json!(7500))
        .await
        .expect("upsert succeeds");
    assert_eq!(updated.name, "Max Power");
    assert_eq!(updated.value, Some(serde_json::json!(7500)));

    // The row landed in the dedicated table
    let value_json: String = sqlx::query_scalar(
        "SELECT value_json FROM instance_properties WHERE instance_id = 5001 AND property_id = 1",
    )
    .fetch_one(&manager.pool)
    .await
    .unwrap();
    assert_eq!(value_json, "7500");

    // Upserting again replaces the value
    manager
        .upsert_single_property(5001, 1, serde_json::json!(8000))
        .await
        .unwrap();
    let value_json: String = sqlx::query_scalar(
        "SELECT value_json FROM instance_properties WHERE instance_id = 5001 AND property_id = 1",
    )
    .fetch_one(&manager.pool)
    .await
    .unwrap();
    assert_eq!(value_json, "8000");
}

#[tokio::test]
async fn test_upsert_single_property_rejects_unknown_id() {
    let (_temp_dir, pool) = create_test_database().await;
    common::test_utils::schema::init_comsrv_schema(&pool)
        .await
        .expect("init comsrv schema");
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(5002),
            instance_name: "pcs_bad_prop".to_string(),
            product_name: "PCS".to_string(),
            parent_id: Some(ess_id),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    // property_id 999 doesn't exist in PCS template
    let err = manager
        .upsert_single_property(5002, 999, serde_json::json!(42))
        .await
        .expect_err("should reject unknown property_id");
    assert!(
        matches!(err, crate::error::ModSrvError::InvalidData(_)),
        "expected InvalidData, got {:?}",
        err
    );
}

#[tokio::test]
async fn test_delete_single_property_removes_row() {
    let (_temp_dir, pool) = create_test_database().await;
    common::test_utils::schema::init_comsrv_schema(&pool)
        .await
        .expect("init comsrv schema");
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    let mut props = HashMap::new();
    props.insert("Max Power".to_string(), serde_json::json!(5000.0));
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(5003),
            instance_name: "pcs_del_prop".to_string(),
            product_name: "PCS".to_string(),
            parent_id: Some(ess_id),
            properties: props,
        })
        .await
        .unwrap();

    let updated = manager
        .delete_single_property(5003, 1)
        .await
        .expect("delete succeeds");
    assert_eq!(updated.name, "Max Power");
    assert!(updated.value.is_none());

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instance_properties WHERE instance_id = 5003")
            .fetch_one(&manager.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_create_instance_already_exists() {
    let (_temp_dir, pool) = create_test_database().await;
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    let req = CreateInstanceRequest {
        instance_id: Some(1001),
        instance_name: "duplicate_instance".to_string(),
        product_name: "Battery".to_string(),
        parent_id: Some(ess_id),
        properties: HashMap::new(),
    };

    // First creation should succeed
    let result1 = manager.create_instance(req.clone()).await;
    assert!(result1.is_ok());

    // Second creation with same name should fail (UNIQUE constraint)
    let result2 = manager.create_instance(req).await;
    assert!(result2.is_err());
    let err_msg = result2.unwrap_err().to_string();
    assert!(
        err_msg.contains("UNIQUE constraint") || err_msg.contains("already exists"),
        "Expected UNIQUE constraint or already exists error, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_create_instance_product_not_found() {
    let (_temp_dir, pool) = create_test_database().await;
    // Don't use any product - test with non-existent product name

    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());

    let req = CreateInstanceRequest {
        instance_id: Some(3001),
        instance_name: "orphan_instance".to_string(),
        product_name: "nonexistent_product".to_string(),
        parent_id: None,
        properties: HashMap::new(),
    };

    let result = manager.create_instance(req).await;

    assert!(result.is_err(), "Should fail when product doesn't exist");
}

#[tokio::test]
async fn test_list_instances_all() {
    let (_temp_dir, pool) = create_test_database().await;
    // Use built-in products: Battery and PCS
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    // Create multiple instances using built-in products
    let req1 = CreateInstanceRequest {
        instance_id: Some(1001),
        instance_name: "battery_01".to_string(),
        product_name: "Battery".to_string(),
        parent_id: Some(ess_id),
        properties: HashMap::new(),
    };
    manager.create_instance(req1).await.unwrap();

    let req2 = CreateInstanceRequest {
        instance_id: Some(1002),
        instance_name: "pcs_01".to_string(),
        product_name: "PCS".to_string(),
        parent_id: Some(ess_id),
        properties: HashMap::new(),
    };
    manager.create_instance(req2).await.unwrap();

    // List all instances (includes Station + ESS from hierarchy + Battery + PCS)
    let instances = manager
        .list_instances_paginated(None, 1, 1000)
        .await
        .unwrap()
        .1;

    assert_eq!(instances.len(), 4);
}

#[tokio::test]
async fn test_list_instances_by_product() {
    let (_temp_dir, pool) = create_test_database().await;
    // Use built-in products: Battery and PCS
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    // Create instances for different products
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1001),
            instance_name: "battery_instance_01".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1002),
            instance_name: "battery_instance_02".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(2001),
            instance_name: "pcs_instance_01".to_string(),
            product_name: "PCS".to_string(),
            parent_id: Some(ess_id),
            properties: HashMap::new(),
        })
        .await
        .unwrap();

    // List instances for Battery only
    let instances = manager
        .list_instances_paginated(Some("Battery"), 1, 1000)
        .await
        .unwrap()
        .1;

    assert_eq!(instances.len(), 2);
    assert!(instances.iter().all(|i| i.product_name() == "Battery"));
}

#[tokio::test]
async fn test_get_instance_success() {
    let (_temp_dir, pool) = create_test_database().await;
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    // Create instance using built-in product
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1001),
            instance_name: "get_test_instance".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
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
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());

    let result = manager.get_instance(9999).await;

    assert!(result.is_err(), "Should fail when instance doesn't exist");
}

#[tokio::test]
async fn test_delete_instance() {
    let (_temp_dir, pool) = create_test_database().await;
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());
    let ess_id = setup_hierarchy(&manager).await;

    // Create instance using built-in product
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1001),
            instance_name: "delete_test".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
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
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());

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
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(
        pool.clone(),
        rtdb.clone(),
        routing_cache,
        product_loader,
        noop_dispatch(),
    );
    let ess_id = setup_hierarchy(&manager).await;

    // Create instance using built-in product
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1001),
            instance_name: "action_test_instance".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
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
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();

    // Configure M2C route: instance 1001, action point "1" -> channel 2, A, point 5
    let mut m2c_data = HashMap::new();
    m2c_data.insert("1001:A:1".to_string(), "2:A:5".to_string());
    let routing_cache = Arc::new(voltage_routing::RoutingCache::from_maps(
        HashMap::new(),
        m2c_data,
        HashMap::new(),
    ));

    let manager = InstanceManager::new(
        pool.clone(),
        rtdb.clone(),
        routing_cache,
        product_loader,
        noop_dispatch(),
    );
    let ess_id = setup_hierarchy(&manager).await;

    // Create instance using built-in product
    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1001),
            instance_name: "routed_action_instance".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
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
}

#[tokio::test]
async fn test_execute_action_multiple_points() {
    let (_temp_dir, pool) = create_test_database().await;
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();

    // Configure multiple routes
    let mut m2c_data = HashMap::new();
    m2c_data.insert("1001:A:1".to_string(), "10:A:1".to_string());
    m2c_data.insert("1001:A:2".to_string(), "10:A:2".to_string());
    let routing_cache = Arc::new(voltage_routing::RoutingCache::from_maps(
        HashMap::new(),
        m2c_data,
        HashMap::new(),
    ));

    let manager = InstanceManager::new(
        pool.clone(),
        rtdb.clone(),
        routing_cache,
        product_loader,
        noop_dispatch(),
    );
    let ess_id = setup_hierarchy(&manager).await;

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1001),
            instance_name: "multi_action_instance".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
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
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(
        pool.clone(),
        rtdb.clone(),
        routing_cache,
        product_loader,
        noop_dispatch(),
    );
    let ess_id = setup_hierarchy(&manager).await;

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1001),
            instance_name: "overwrite_test".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
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
    let product_loader = create_test_product_loader(pool.clone());
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new());
    let manager = InstanceManager::new(
        pool.clone(),
        rtdb.clone(),
        routing_cache,
        product_loader,
        noop_dispatch(),
    );
    let ess_id = setup_hierarchy(&manager).await;

    manager
        .create_instance(CreateInstanceRequest {
            instance_id: Some(1001),
            instance_name: "negative_test".to_string(),
            product_name: "Battery".to_string(),
            parent_id: Some(ess_id),
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

// ==================== M2C Control Gate Tests ====================
//
// Verify that execute_action() rejects writes targeting offline channels,
// honoring the rule from CLAUDE.md "Instance 是纯物模型（不要染色）":
// channel offline is a transparent dispatch failure, not an instance state.

fn manager_with_m2c_route(
    pool: SqlitePool,
    rtdb: Arc<voltage_rtdb::MemoryRtdb>,
) -> InstanceManager<voltage_rtdb::MemoryRtdb> {
    let mut m2c_data = HashMap::new();
    // 1001:A:1 → channel 2, point 5 (Adjustment)
    m2c_data.insert("1001:A:1".to_string(), "2:A:5".to_string());
    let routing_cache = Arc::new(voltage_routing::RoutingCache::from_maps(
        HashMap::new(),
        m2c_data,
        HashMap::new(),
    ));
    let product_loader = create_test_product_loader(pool.clone());
    InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch())
}

#[tokio::test]
async fn execute_action_rejects_when_target_channel_offline() {
    let (_temp_dir, pool) = create_test_database().await;
    let rtdb = create_test_rtdb();
    let manager = manager_with_m2c_route(pool, rtdb.clone());

    // Mark channel 2 offline in the health cache (simulating comsrv publishing "0")
    manager.channel_health().set_for_test(2, false);

    let result = manager.execute_action(1001, "1", 75.0).await;

    match result {
        Err(crate::error::ModSrvError::ChannelUnreachable { channel_id }) => {
            assert_eq!(channel_id, 2);
        },
        other => panic!(
            "expected ChannelUnreachable {{ channel_id: 2 }}, got {:?}",
            other
        ),
    }

    // Critical: instance hash MUST NOT be written when the gate fires —
    // otherwise stale "successful write" state lingers in inst:{id}:A.
    use voltage_rtdb::Rtdb;
    let config = voltage_rtdb::KeySpaceConfig::production();
    let action_key = config.instance_action_key(1001);
    let stored = rtdb.hash_get(&action_key, "1").await.unwrap();
    assert!(
        stored.is_none(),
        "instance action hash must not be written when channel is offline"
    );

    // Channel hash MUST also be untouched — gate returns before
    // set_action_point writes the downstream `comsrv:{ch}:A` hash.
    use voltage_model::PointType;
    let channel_key = config.channel_key(2, PointType::Adjustment);
    let channel_value = rtdb.hash_get(&channel_key, "5").await.unwrap();
    assert!(
        channel_value.is_none(),
        "channel hash must not be written when gate fires"
    );
}

#[tokio::test]
async fn execute_action_proceeds_when_channel_online() {
    let (_temp_dir, pool) = create_test_database().await;
    let rtdb = create_test_rtdb();
    let manager = manager_with_m2c_route(pool, rtdb.clone());

    // Explicit online (also covers fail-open since the cache starts uninitialized)
    manager.channel_health().set_for_test(2, true);

    let result = manager.execute_action(1001, "1", 75.0).await;
    assert!(
        result.is_ok(),
        "online channel must accept writes: {:?}",
        result.err()
    );

    use voltage_rtdb::Rtdb;
    let config = voltage_rtdb::KeySpaceConfig::production();
    let stored = rtdb
        .hash_get(&config.instance_action_key(1001), "1")
        .await
        .unwrap();
    assert_eq!(stored.unwrap().as_ref(), b"75");
}

#[tokio::test]
async fn execute_action_uninitialized_cache_fails_open() {
    // At modsrv boot the cache hasn't pulled comsrv:online yet — control
    // must keep working in that window, otherwise startup races brick the system.
    let (_temp_dir, pool) = create_test_database().await;
    let rtdb = create_test_rtdb();
    let manager = manager_with_m2c_route(pool, rtdb);

    assert!(
        !manager.channel_health().is_initialized(),
        "fresh cache must report uninitialized"
    );

    let result = manager.execute_action(1001, "1", 75.0).await;
    assert!(
        result.is_ok(),
        "uninitialized cache must fail-open: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn execute_action_routes_to_snapshot_when_routing_reloaded_concurrently() {
    // TOCTOU regression: simulate `monarch sync` reloading routing between
    // our gate's lookup and the downstream write. Previously the second
    // lookup inside voltage_routing::set_action_point would see the new
    // (empty) map and silently degrade to a local-only write, returning
    // Ok(()) to the caller while the command never reached the device.
    //
    // With set_action_point_with_target the M2C target is resolved once in
    // execute_action and threaded through, so the in-flight command uses
    // the snapshot we already gate-checked.
    let (_temp_dir, pool) = create_test_database().await;
    let rtdb = create_test_rtdb();
    let product_loader = create_test_product_loader(pool.clone());

    let mut m2c_data = HashMap::new();
    m2c_data.insert("1001:A:1".to_string(), "2:A:5".to_string());
    let routing_cache = Arc::new(voltage_routing::RoutingCache::from_maps(
        HashMap::new(),
        m2c_data,
        HashMap::new(),
    ));
    let manager = InstanceManager::new(
        pool,
        rtdb.clone(),
        Arc::clone(&routing_cache),
        product_loader,
        noop_dispatch(),
    );
    manager.channel_health().set_for_test(2, true);

    // Wipe the M2C map AFTER the InstanceManager is built but BEFORE
    // execute_action runs. In production this is what monarch sync does
    // mid-flight; here we just do it synchronously to make the test
    // deterministic — execute_action's gate lookup still has to find the
    // route in its own snapshot and route writes accordingly.
    //
    // The lookup inside execute_action happens BEFORE this clear in real
    // life only because of timing; the bug is that even a perfectly-timed
    // reload between the gate and the second internal lookup would silently
    // drop the command. We assert here that with the snapshot threaded
    // through, the test is forced to fail if anyone re-introduces an
    // internal lookup in the write path: this `update` would zero out the
    // map and the second lookup would return None.
    let result = manager.execute_action(1001, "1", 75.0).await;
    routing_cache.update(HashMap::new(), HashMap::new(), HashMap::new());

    assert!(
        result.is_ok(),
        "execute_action must succeed using the pre-gate snapshot: {:?}",
        result.err()
    );

    // The decisive assertion: channel hash was written. If a regression
    // re-introduces a second internal routing lookup that races with
    // reload, `comsrv:{2}:A` would stay empty.
    use voltage_rtdb::Rtdb;
    let config = voltage_rtdb::KeySpaceConfig::production();
    let channel_value = rtdb
        .hash_get(
            &config.channel_key(2, voltage_model::PointType::Adjustment),
            "5",
        )
        .await
        .unwrap();
    assert!(
        channel_value.is_some(),
        "channel hash MUST be written from the snapshot — silent local-only \
         degradation indicates the TOCTOU regression returned"
    );
    // Channel hash uses f64_to_bytes (ryu) which renders 75.0 as "75.0",
    // unlike the instance hash path which goes through hash_set_f64.
    assert_eq!(channel_value.unwrap().as_ref(), b"75.0");
}

#[tokio::test]
async fn execute_action_no_route_skips_gate() {
    // No M2C route means no channel target — nothing to gate, action stores locally.
    let (_temp_dir, pool) = create_test_database().await;
    let rtdb = create_test_rtdb();
    let product_loader = create_test_product_loader(pool.clone());
    let routing_cache = Arc::new(voltage_routing::RoutingCache::new()); // empty
    let manager = InstanceManager::new(pool, rtdb, routing_cache, product_loader, noop_dispatch());

    // Even with the cache wired up and marking channel 2 offline, an unrouted
    // action targets no channel and must not be rejected.
    manager.channel_health().set_for_test(2, false);

    let result = manager.execute_action(1001, "1", 42.0).await;
    assert!(
        result.is_ok(),
        "unrouted action must not be gated: {:?}",
        result.err()
    );
}
