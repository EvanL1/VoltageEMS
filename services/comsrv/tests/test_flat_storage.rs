//! 扁平化存储测试

use comsrv::core::redis::storage::RedisStorage;
use comsrv::core::redis::types::*;

#[tokio::test]
async fn test_point_value() {
    let pv = PointValue::new(25.6);
    assert_eq!(pv.value, 25.6);

    let redis_str = pv.to_redis();
    assert!(redis_str.contains("25.6"));
    assert!(redis_str.contains(":"));

    let parsed = PointValue::from_redis(&redis_str).unwrap();
    assert_eq!(parsed.value, 25.6);
    assert_eq!(parsed.timestamp, pv.timestamp);
}

#[test]
fn test_key_generation() {
    // 测试实时数据键
    assert_eq!(make_key(1001, TYPE_MEASUREMENT, 10001), "1001:m:10001");
    assert_eq!(make_key(1001, TYPE_SIGNAL, 20001), "1001:s:20001");
    assert_eq!(make_key(1001, TYPE_CONTROL, 30001), "1001:c:30001");
    assert_eq!(make_key(1001, TYPE_ADJUSTMENT, 40001), "1001:a:40001");

    // 测试配置键
    assert_eq!(
        make_config_key(1001, TYPE_MEASUREMENT, 10001),
        "cfg:1001:m:10001"
    );
}

#[test]
fn test_point_config() {
    let config = PointConfig {
        name: "温度传感器".to_string(),
        unit: "°C".to_string(),
        scale: 0.1,
        offset: 0.0,
    };

    // 测试序列化
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("温度传感器"));
    assert!(json.contains("°C"));

    // 测试反序列化
    let parsed: PointConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, config.name);
    assert_eq!(parsed.scale, config.scale);
}

#[tokio::test]
#[ignore] // 需要Redis实例
async fn test_redis_storage_integration() {
    let mut storage = RedisStorage::new("redis://localhost:6379")
        .await
        .expect("Failed to connect to Redis");

    // 测试单点写入和读取
    storage
        .set_point(1001, TYPE_MEASUREMENT, 10001, 25.6)
        .await
        .expect("Failed to set point");

    let result = storage
        .get_point(1001, TYPE_MEASUREMENT, 10001)
        .await
        .expect("Failed to get point");

    assert!(result.is_some());
    let (value, _timestamp) = result.unwrap();
    assert_eq!(value, 25.6);

    // 测试批量写入
    let updates = vec![
        PointUpdate {
            channel_id: 1001,
            point_type: TYPE_MEASUREMENT,
            point_id: 10002,
            value: 380.5,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: TYPE_MEASUREMENT,
            point_id: 10003,
            value: 125.3,
        },
    ];

    storage
        .set_points(&updates)
        .await
        .expect("Failed to set points");

    // 测试批量读取
    let keys = vec![
        PointKey {
            channel_id: 1001,
            point_type: TYPE_MEASUREMENT,
            point_id: 10002,
        },
        PointKey {
            channel_id: 1001,
            point_type: TYPE_MEASUREMENT,
            point_id: 10003,
        },
    ];

    let results = storage
        .get_points(&keys)
        .await
        .expect("Failed to get points");

    assert_eq!(results.len(), 2);
    assert!(results[0].is_some());
    assert!(results[1].is_some());

    // 清理测试数据
    storage
        .delete_point(1001, TYPE_MEASUREMENT, 10001)
        .await
        .ok();
    storage
        .delete_point(1001, TYPE_MEASUREMENT, 10002)
        .await
        .ok();
    storage
        .delete_point(1001, TYPE_MEASUREMENT, 10003)
        .await
        .ok();
}
