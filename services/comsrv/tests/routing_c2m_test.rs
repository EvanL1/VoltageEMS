//! C2M Routing End-to-End Tests
//!
//! Tests the complete Channel-to-Model routing flow:
//! 1. Channel write: comsrv:{channel_id}:{T|S|C|A} Hash
//! 2. Routing lookup: RoutingCache.lookup_c2m("{channel_id}:{type}:{point_id}")
//! 3. Instance write: inst:{instance_id}:M Hash
//!
//! Uses MemoryRtdb (no external dependencies) for fast, isolated testing.

use comsrv::storage::{write_batch, PointUpdate};
use std::collections::HashMap;
use std::sync::Arc;
use voltage_config::FourRemote;
use voltage_config::{KeySpaceConfig, RoutingCache};
use voltage_rtdb::Rtdb;

/// 创建测试用的内存 RTDB
fn create_test_rtdb() -> Arc<dyn Rtdb> {
    Arc::new(voltage_rtdb::MemoryRtdb::new())
}

/// 创建带路由配置的测试环境
///
/// # Arguments
/// * `c2m_routes` - C2M 路由配置，格式: [("1001:T:1", "23:M:1"), ...]
///
/// # Returns
/// * `(Arc<dyn Rtdb>, Arc<RoutingCache>)` - RTDB 和路由缓存
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

/// 辅助函数：验证通道数据（工程值层）
async fn assert_channel_value(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_value: f64,
) {
    use voltage_config::protocols::PointType;
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

/// 辅助函数：验证通道时间戳
async fn assert_channel_timestamp_exists(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
) {
    use voltage_config::protocols::PointType;
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

/// 辅助函数：验证通道原始值
async fn assert_channel_raw_value(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_raw: f64,
) {
    use voltage_config::protocols::PointType;
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

/// 辅助函数：验证实例测量值
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

/// 辅助函数：验证实例测量值不存在
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
    // Given: 路由配置 1001:T:1 -> inst:23:M:1
    let (rtdb, routing_cache) = setup_c2m_routing(vec![("1001:T:1", "23:M:1")]).await;

    // When: 写入通道点位
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 230.5,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: 验证通道数据写入成功
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 230.5).await;
    assert_channel_timestamp_exists(rtdb.as_ref(), 1001, "T", 1).await;

    // Then: 验证实例数据路由成功
    assert_instance_measurement(rtdb.as_ref(), 23, 1, 230.5).await;
}

#[tokio::test]
async fn test_c2m_three_layer_architecture() {
    // Given: 路由配置
    let (rtdb, routing_cache) = setup_c2m_routing(vec![("1001:T:1", "23:M:1")]).await;

    // When: 写入通道点位（包含原始值）
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 230.5,            // 工程值
        raw_value: Some(2305.0), // 原始值
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: 验证三层架构都正确写入
    // Layer 1: 工程值
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 230.5).await;

    // Layer 2: 时间戳
    assert_channel_timestamp_exists(rtdb.as_ref(), 1001, "T", 1).await;

    // Layer 3: 原始值
    assert_channel_raw_value(rtdb.as_ref(), 1001, "T", 1, 2305.0).await;

    // Then: 验证实例数据路由成功（使用工程值）
    assert_instance_measurement(rtdb.as_ref(), 23, 1, 230.5).await;
}

#[tokio::test]
async fn test_c2m_routing_to_multiple_instances() {
    // Given: 多个通道点位路由到不同实例
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:1", "23:M:1"), // 通道 1001 点位 1 -> 实例 23
        ("1001:T:2", "24:M:1"), // 通道 1001 点位 2 -> 实例 24
        ("1001:T:3", "25:M:1"), // 通道 1001 点位 3 -> 实例 25
    ])
    .await;

    // When: 写入多个通道点位
    let updates = vec![
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 1,
            value: 100.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 2,
            value: 200.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 3,
            value: 300.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: 验证通道数据写入成功
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 2, 200.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 3, 300.0).await;

    // Then: 验证数据路由到不同的实例
    assert_instance_measurement(rtdb.as_ref(), 23, 1, 100.0).await;
    assert_instance_measurement(rtdb.as_ref(), 24, 1, 200.0).await;
    assert_instance_measurement(rtdb.as_ref(), 25, 1, 300.0).await;
}

#[tokio::test]
async fn test_c2m_no_routing() {
    // Given: 不配置路由
    let rtdb = create_test_rtdb();
    let routing_cache = Arc::new(RoutingCache::new()); // 空路由缓存

    // When: 写入通道点位
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 100.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: 验证通道数据写入成功
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;

    // Then: 验证实例数据不存在（因为没有路由）
    assert_instance_measurement_not_exists(rtdb.as_ref(), 23, 1).await;
}

#[tokio::test]
async fn test_c2m_batch_updates() {
    // Given: 批量路由配置
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:1", "23:M:1"),
        ("1001:T:2", "23:M:2"),
        ("1001:T:3", "23:M:3"),
        ("1001:T:4", "23:M:4"),
        ("1001:T:5", "23:M:5"),
    ])
    .await;

    // When: 批量写入 5 个点位
    let updates = vec![
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 1,
            value: 10.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 2,
            value: 20.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 3,
            value: 30.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 4,
            value: 40.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 5,
            value: 50.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: 验证所有点位都正确路由
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
    // Given: 四遥类型路由配置
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:1", "23:M:1"), // 遥测 (Telemetry)
        ("1001:S:2", "23:M:2"), // 遥信 (Signal)
        ("1001:C:3", "23:M:3"), // 遥控 (Control)
        ("1001:A:4", "23:M:4"), // 遥调 (Adjustment)
    ])
    .await;

    // When: 写入不同类型的点位
    let updates = vec![
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry, // 遥测
            point_id: 1,
            value: 230.5,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Signal, // 遥信
            point_id: 2,
            value: 1.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Control, // 遥控
            point_id: 3,
            value: 0.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Adjustment, // 遥调
            point_id: 4,
            value: 50.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: 验证四遥类型都正确路由
    // 遥测 (Telemetry)
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 230.5).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 1, 230.5).await;

    // 遥信 (Signal)
    assert_channel_value(rtdb.as_ref(), 1001, "S", 2, 1.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 2, 1.0).await;

    // 遥控 (Control)
    assert_channel_value(rtdb.as_ref(), 1001, "C", 3, 0.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 3, 0.0).await;

    // 遥调 (Adjustment)
    assert_channel_value(rtdb.as_ref(), 1001, "A", 4, 50.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 4, 50.0).await;
}

#[tokio::test]
async fn test_c2m_routing_with_different_point_ids() {
    // Given: 路由配置（源点位和目标点位 ID 不同）
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:10", "23:M:1"), // 通道点位 10 -> 实例点位 1
        ("1001:T:20", "23:M:5"), // 通道点位 20 -> 实例点位 5
    ])
    .await;

    // When: 写入通道点位
    let updates = vec![
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 10,
            value: 100.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 20,
            value: 200.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: 验证路由到正确的实例点位
    assert_channel_value(rtdb.as_ref(), 1001, "T", 10, 100.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 20, 200.0).await;

    assert_instance_measurement(rtdb.as_ref(), 23, 1, 100.0).await; // 点位 10 -> 1
    assert_instance_measurement(rtdb.as_ref(), 23, 5, 200.0).await; // 点位 20 -> 5
}

#[tokio::test]
async fn test_c2m_routing_with_multiple_channels() {
    // Given: 多个通道路由到同一实例
    let (rtdb, routing_cache) = setup_c2m_routing(vec![
        ("1001:T:1", "23:M:1"), // 通道 1001
        ("1002:T:1", "23:M:2"), // 通道 1002
        ("1003:T:1", "23:M:3"), // 通道 1003
    ])
    .await;

    // When: 写入多个通道的点位
    let updates = vec![
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 1,
            value: 100.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1002,
            point_type: FourRemote::Telemetry,
            point_id: 1,
            value: 200.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1003,
            point_type: FourRemote::Telemetry,
            point_id: 1,
            value: 300.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("Failed to write batch");

    // Then: 验证所有通道数据都正确路由到实例
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, "T", 1, 200.0).await;
    assert_channel_value(rtdb.as_ref(), 1003, "T", 1, 300.0).await;

    assert_instance_measurement(rtdb.as_ref(), 23, 1, 100.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 2, 200.0).await;
    assert_instance_measurement(rtdb.as_ref(), 23, 3, 300.0).await;
}
