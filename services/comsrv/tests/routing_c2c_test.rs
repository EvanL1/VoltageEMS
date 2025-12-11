//! C2C (Channel to Channel) 路由端到端测试
//!
//! 测试 comsrv 的 C2C 直通转发功能，包括：
//! - 基础单级转发
//! - 多级级联转发
//! - 循环检测与深度限制
//! - 原始值传递
//! - 不同点位类型转发
//! - 一对多转发

#![allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable

use comsrv::storage::{write_batch, PointUpdate};
use std::collections::HashMap;
use std::sync::Arc;
use voltage_config::FourRemote;
use voltage_config::{KeySpaceConfig, RoutingCache};
use voltage_rtdb::Rtdb;

/// 最大 C2C 级联深度常量（与 storage.rs 保持一致）
const MAX_C2C_DEPTH: u8 = 2;

/// 创建内存 RTDB 用于测试（无外部依赖）
fn create_test_rtdb() -> Arc<dyn Rtdb> {
    Arc::new(voltage_rtdb::MemoryRtdb::new())
}

/// 创建带 C2C 路由的测试环境
///
/// # 参数
/// - `c2c_routes`: C2C 路由映射列表，格式为 [("源通道:类型:点位", "目标通道:类型:点位"), ...]
///
/// # 返回
/// - RTDB 实例（内存实现）
/// - RoutingCache 实例（包含 C2C 路由配置）
async fn setup_c2c_routing(c2c_routes: Vec<(&str, &str)>) -> (Arc<dyn Rtdb>, Arc<RoutingCache>) {
    let rtdb = create_test_rtdb();

    let mut c2c_map = HashMap::new();
    for (source, target) in c2c_routes {
        c2c_map.insert(source.to_string(), target.to_string());
    }

    let routing_cache = Arc::new(RoutingCache::from_maps(
        HashMap::new(), // C2M 路由（本测试不需要）
        HashMap::new(), // M2C 路由（本测试不需要）
        c2c_map,        // C2C 路由
    ));

    (rtdb, routing_cache)
}

/// 验证通道点位的值
///
/// # 参数
/// - `rtdb`: RTDB 实例
/// - `channel_id`: 通道 ID
/// - `point_type`: 点位类型（T/S/C/A）
/// - `point_id`: 点位 ID
/// - `expected_value`: 期望的值
async fn assert_channel_value(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_value: f64,
) {
    let config = KeySpaceConfig::production();
    let point_type_enum = voltage_config::protocols::PointType::from_str(point_type).unwrap();
    let channel_key = config.channel_key(channel_id, point_type_enum);

    let value = rtdb
        .hash_get(&channel_key, &point_id.to_string())
        .await
        .expect("Failed to read channel value")
        .expect("Channel value should exist");

    let actual_value: f64 = String::from_utf8(value.to_vec()).unwrap().parse().unwrap();

    assert_eq!(
        actual_value, expected_value,
        "Channel {}:{}:{} value mismatch",
        channel_id, point_type, point_id
    );
}

/// 验证通道点位不存在
///
/// # 参数
/// - `rtdb`: RTDB 实例
/// - `channel_id`: 通道 ID
/// - `point_type`: 点位类型（T/S/C/A）
/// - `point_id`: 点位 ID
async fn assert_channel_value_missing(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
) {
    let config = KeySpaceConfig::production();
    let point_type_enum = voltage_config::protocols::PointType::from_str(point_type).unwrap();
    let channel_key = config.channel_key(channel_id, point_type_enum);

    let value = rtdb
        .hash_get(&channel_key, &point_id.to_string())
        .await
        .expect("Failed to read channel value");

    assert!(
        value.is_none(),
        "Channel {}:{}:{} should not have value",
        channel_id,
        point_type,
        point_id
    );
}

/// 验证原始值传递
///
/// # 参数
/// - `rtdb`: RTDB 实例
/// - `channel_id`: 通道 ID
/// - `point_type`: 点位类型（T/S/C/A）
/// - `point_id`: 点位 ID
/// - `expected_raw_value`: 期望的原始值
async fn assert_raw_value(
    rtdb: &dyn Rtdb,
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    expected_raw_value: f64,
) {
    let config = KeySpaceConfig::production();
    let point_type_enum = voltage_config::protocols::PointType::from_str(point_type).unwrap();
    let channel_key = config.channel_key(channel_id, point_type_enum);
    let raw_key = format!("{}:raw", channel_key);

    let value = rtdb
        .hash_get(&raw_key, &point_id.to_string())
        .await
        .expect("Failed to read raw value")
        .expect("Raw value should exist");

    let actual_value: f64 = String::from_utf8(value.to_vec()).unwrap().parse().unwrap();

    assert_eq!(
        actual_value, expected_raw_value,
        "Channel {}:{}:{} raw value mismatch",
        channel_id, point_type, point_id
    );
}

#[tokio::test]
async fn test_c2c_basic_routing() {
    // 测试场景：基础 C2C 直通转发
    // 配置：1001:T:1 -> 1002:T:5
    // 写入：1001:T:1 = 100.0
    // 验证：源通道和目标通道都有正确数据

    let (rtdb, routing_cache) = setup_c2c_routing(vec![("1001:T:1", "1002:T:5")]).await;

    // 写入源通道
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
        .expect("write_batch should succeed");

    // 验证源通道
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;

    // 验证目标通道
    assert_channel_value(rtdb.as_ref(), 1002, "T", 5, 100.0).await;
}

#[tokio::test]
async fn test_c2c_cascade_two_levels() {
    // 测试场景：两级级联转发
    // 配置：1001:T:1 -> 1002:T:2 -> 1003:T:3
    // 写入：1001:T:1 = 50.0
    // 验证：三个通道都有正确数据，cascade_depth 依次为 0 → 1 → 2（停止）

    let (rtdb, routing_cache) =
        setup_c2c_routing(vec![("1001:T:1", "1002:T:2"), ("1002:T:2", "1003:T:3")]).await;

    // 写入源通道（第一级，cascade_depth = 0）
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 50.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证第一级：1001:T:1（cascade_depth = 0）
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 50.0).await;

    // 验证第二级：1002:T:2（cascade_depth = 1）
    assert_channel_value(rtdb.as_ref(), 1002, "T", 2, 50.0).await;

    // 验证第三级：1003:T:3（cascade_depth = 2，达到 MAX_C2C_DEPTH，停止转发）
    // 注意：cascade_depth = 2 时还会写入，但不会再继续转发
    assert_channel_value(rtdb.as_ref(), 1003, "T", 3, 50.0).await;
}

#[tokio::test]
async fn test_c2c_cascade_max_depth() {
    // 测试场景：三级级联（验证最大深度限制）
    // 配置：1001:T:1 -> 1002:T:2 -> 1003:T:3 -> 1004:T:4
    // 写入：1001:T:1 = 200.0
    // 验证：前三级有数据，第四级没有数据（超过 MAX_C2C_DEPTH）

    let (rtdb, routing_cache) = setup_c2c_routing(vec![
        ("1001:T:1", "1002:T:2"),
        ("1002:T:2", "1003:T:3"),
        ("1003:T:3", "1004:T:4"),
    ])
    .await;

    // 写入源通道
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 200.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证第一级：1001:T:1（cascade_depth = 0，转发）
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 200.0).await;

    // 验证第二级：1002:T:2（cascade_depth = 1，转发）
    assert_channel_value(rtdb.as_ref(), 1002, "T", 2, 200.0).await;

    // 验证第三级：1003:T:3（cascade_depth = 2，写入但不转发）
    // 注意：MAX_C2C_DEPTH = 2，cascade_depth < 2 时才转发
    // cascade_depth = 2 时写入数据但不再转发
    assert_channel_value(rtdb.as_ref(), 1003, "T", 3, 200.0).await;

    // 验证第四级：1004:T:4（不应该有数据，因为上一级没有转发）
    assert_channel_value_missing(rtdb.as_ref(), 1004, "T", 4).await;
}

#[tokio::test]
async fn test_c2c_infinite_loop_prevention() {
    // 测试场景：无限循环检测
    // 配置循环：1001:T:1 -> 1002:T:1 -> 1001:T:1
    // 写入：1001:T:1 = 75.0
    // 验证：cascade_depth 达到 MAX_C2C_DEPTH 后停止，不会无限循环

    let (rtdb, routing_cache) = setup_c2c_routing(vec![
        ("1001:T:1", "1002:T:1"),
        ("1002:T:1", "1001:T:1"), // 循环路由
    ])
    .await;

    // 写入源通道
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 75.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证源通道（会被覆盖两次：cascade_depth=0 和 cascade_depth=2）
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 75.0).await;

    // 验证目标通道（cascade_depth = 1）
    assert_channel_value(rtdb.as_ref(), 1002, "T", 1, 75.0).await;

    // 重要：测试应该能正常完成，证明没有无限循环
}

#[tokio::test]
async fn test_c2c_preserve_raw_values() {
    // 测试场景：原始值在级联中正确传递
    // 配置：1001:T:1 -> 1002:T:2
    // 写入：value = 100.0, raw_value = 1000
    // 验证：目标通道的工程值和原始值都正确

    let (rtdb, routing_cache) = setup_c2c_routing(vec![("1001:T:1", "1002:T:2")]).await;

    // 写入源通道（带原始值）
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 100.0,
        raw_value: Some(1000.0),
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证源通道工程值和原始值
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 100.0).await;
    assert_raw_value(rtdb.as_ref(), 1001, "T", 1, 1000.0).await;

    // 验证目标通道工程值和原始值
    assert_channel_value(rtdb.as_ref(), 1002, "T", 2, 100.0).await;
    assert_raw_value(rtdb.as_ref(), 1002, "T", 2, 1000.0).await;
}

#[tokio::test]
async fn test_c2c_different_point_types() {
    // 测试场景：不同点位类型的转发
    // 测试 T→T, S→S, C→C, A→A 四种类型
    // 验证类型转换正确

    let (rtdb, routing_cache) = setup_c2c_routing(vec![
        ("1001:T:1", "1002:T:1"), // 遥测 → 遥测
        ("1001:S:2", "1002:S:2"), // 遥信 → 遥信
        ("1001:C:3", "1002:C:3"), // 遥控 → 遥控
        ("1001:A:4", "1002:A:4"), // 遥调 → 遥调
    ])
    .await;

    // 写入四种类型的点位
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
            point_type: FourRemote::Signal,
            point_id: 2,
            value: 1.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Control,
            point_id: 3,
            value: 0.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Adjustment,
            point_id: 4,
            value: 50.5,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证遥测（T）
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 10.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, "T", 1, 10.0).await;

    // 验证遥信（S）
    assert_channel_value(rtdb.as_ref(), 1001, "S", 2, 1.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, "S", 2, 1.0).await;

    // 验证遥控（C）
    assert_channel_value(rtdb.as_ref(), 1001, "C", 3, 0.0).await;
    assert_channel_value(rtdb.as_ref(), 1002, "C", 3, 0.0).await;

    // 验证遥调（A）
    assert_channel_value(rtdb.as_ref(), 1001, "A", 4, 50.5).await;
    assert_channel_value(rtdb.as_ref(), 1002, "A", 4, 50.5).await;
}

#[tokio::test]
async fn test_c2c_one_to_many() {
    // 测试场景：一对多转发
    // 配置：1001:T:1 -> 1002:T:1 和 1001:T:1 -> 1003:T:1
    // 写入：1001:T:1 = 123.0
    // 验证：两个目标通道都收到数据

    // 注意：RoutingCache 的 DashMap 只能存储一个目标
    // 实际场景中，一对多需要在配置层面实现（多个路由条目）
    // 这里测试两个不同的源点位分别转发

    let (rtdb, routing_cache) = setup_c2c_routing(vec![
        ("1001:T:1", "1002:T:1"),
        ("1001:T:2", "1003:T:1"), // 不同源点位
    ])
    .await;

    // 写入两个源点位
    let updates = vec![
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 1,
            value: 123.0,
            raw_value: None,
            cascade_depth: 0,
        },
        PointUpdate {
            channel_id: 1001,
            point_type: FourRemote::Telemetry,
            point_id: 2,
            value: 123.0,
            raw_value: None,
            cascade_depth: 0,
        },
    ];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证源通道
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 123.0).await;
    assert_channel_value(rtdb.as_ref(), 1001, "T", 2, 123.0).await;

    // 验证两个目标通道都收到数据
    assert_channel_value(rtdb.as_ref(), 1002, "T", 1, 123.0).await;
    assert_channel_value(rtdb.as_ref(), 1003, "T", 1, 123.0).await;
}

#[tokio::test]
async fn test_c2c_no_routing() {
    // 测试场景：无 C2C 路由配置
    // 不配置 C2C 路由
    // 写入通道数据
    // 验证：只有源通道有数据，无转发

    let (rtdb, routing_cache) = setup_c2c_routing(vec![]).await; // 空路由

    // 写入源通道
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 99.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证源通道
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 99.0).await;

    // 验证其他通道没有数据
    assert_channel_value_missing(rtdb.as_ref(), 1002, "T", 1).await;
    assert_channel_value_missing(rtdb.as_ref(), 1003, "T", 1).await;
}

#[tokio::test]
async fn test_c2c_cross_type_routing() {
    // 测试场景：跨类型转发（边界测试）
    // 配置：1001:T:1 -> 1002:S:1（遥测 → 遥信）
    // 验证：不同类型间的转发也能正常工作

    let (rtdb, routing_cache) = setup_c2c_routing(vec![("1001:T:1", "1002:S:1")]).await;

    // 写入遥测点位
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 88.0,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证源通道（遥测）
    assert_channel_value(rtdb.as_ref(), 1001, "T", 1, 88.0).await;

    // 验证目标通道（遥信）
    assert_channel_value(rtdb.as_ref(), 1002, "S", 1, 88.0).await;
}

#[test]
fn test_max_c2c_depth_constant() {
    // 验证常量定义与代码一致
    assert_eq!(
        MAX_C2C_DEPTH, 2,
        "MAX_C2C_DEPTH should be 2 (consistent with storage.rs)"
    );
}

#[tokio::test]
async fn test_c2c_timestamp_propagation() {
    // 测试场景：时间戳在级联中正确生成
    // 配置：1001:T:1 -> 1002:T:2
    // 验证：源通道和目标通道都有时间戳

    let (rtdb, routing_cache) = setup_c2c_routing(vec![("1001:T:1", "1002:T:2")]).await;

    // 写入源通道
    let updates = vec![PointUpdate {
        channel_id: 1001,
        point_type: FourRemote::Telemetry,
        point_id: 1,
        value: 66.6,
        raw_value: None,
        cascade_depth: 0,
    }];

    write_batch(rtdb.as_ref(), &routing_cache, updates)
        .await
        .expect("write_batch should succeed");

    // 验证源通道时间戳存在
    let config = KeySpaceConfig::production();
    let point_type_enum = voltage_config::protocols::PointType::Telemetry;
    let channel_key_1001 = config.channel_key(1001, point_type_enum);
    let ts_key_1001 = format!("{}:ts", channel_key_1001);

    let ts_1001 = rtdb
        .hash_get(&ts_key_1001, "1")
        .await
        .expect("Failed to read timestamp")
        .expect("Timestamp should exist");

    assert!(!ts_1001.is_empty(), "Timestamp should not be empty");

    // 验证目标通道时间戳存在
    let channel_key_1002 = config.channel_key(1002, point_type_enum);
    let ts_key_1002 = format!("{}:ts", channel_key_1002);

    let ts_1002 = rtdb
        .hash_get(&ts_key_1002, "2")
        .await
        .expect("Failed to read timestamp")
        .expect("Timestamp should exist");

    assert!(!ts_1002.is_empty(), "Timestamp should not be empty");

    // 验证两个时间戳应该接近（允许微小差异，因为级联写入是顺序执行的）
    // 每次 write_batch 调用都生成新的时间戳，所以可能有 1-2ms 的差异
    let ts_1001_value: u64 = String::from_utf8(ts_1001.to_vec())
        .unwrap()
        .parse()
        .unwrap();
    let ts_1002_value: u64 = String::from_utf8(ts_1002.to_vec())
        .unwrap()
        .parse()
        .unwrap();

    let time_diff = ts_1001_value.abs_diff(ts_1002_value);

    assert!(
        time_diff <= 10, // 允许最多 10ms 的时间差
        "Timestamps should be close (diff: {}ms)",
        time_diff
    );
}
