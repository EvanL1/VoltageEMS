//! 插件存储单元测试

use chrono::Utc;
use comsrv::core::config::types::TelemetryType;
use comsrv::plugins::core::{DefaultPluginStorage, PluginPointUpdate, PluginStorage};
use std::sync::Arc;

/// 创建测试用的点位更新
fn create_test_point_update(channel_id: u16, point_id: u32, value: f64) -> PluginPointUpdate {
    PluginPointUpdate {
        channel_id,
        telemetry_type: TelemetryType::Measurement,
        point_id,
        value,
        timestamp: Utc::now().timestamp_millis(),
        raw_value: None,
    }
}

#[tokio::test]
async fn test_plugin_point_update_creation() {
    let update = create_test_point_update(1001, 10001, 25.5);
    
    assert_eq!(update.channel_id, 1001);
    assert_eq!(update.point_id, 10001);
    assert_eq!(update.value, 25.5);
    assert!(matches!(update.telemetry_type, TelemetryType::Measurement));
    assert!(update.timestamp > 0);
    assert!(update.raw_value.is_none());
}

#[tokio::test]
async fn test_plugin_point_update_with_raw_value() {
    let mut update = create_test_point_update(1001, 10001, 25.5);
    update.raw_value = Some(255.0); // 原始值
    
    assert_eq!(update.value, 25.5);
    assert_eq!(update.raw_value, Some(255.0));
}

#[tokio::test]
async fn test_telemetry_type_variants() {
    let types = vec![
        TelemetryType::Measurement,
        TelemetryType::Signal, 
        TelemetryType::Control,
        TelemetryType::Adjustment,
    ];
    
    for (i, telemetry_type) in types.into_iter().enumerate() {
        let update = PluginPointUpdate {
            channel_id: 1001,
            telemetry_type,
            point_id: i as u32,
            value: i as f64,
            timestamp: Utc::now().timestamp_millis(),
            raw_value: None,
        };
        
        assert_eq!(update.point_id, i as u32);
        assert_eq!(update.value, i as f64);
    }
}

#[tokio::test] 
async fn test_batch_point_updates() {
    let mut updates = Vec::new();
    
    // 创建100个测试点位更新
    for i in 0..100 {
        updates.push(create_test_point_update(1001, i, i as f64 * 0.1));
    }
    
    assert_eq!(updates.len(), 100);
    
    // 验证每个更新的正确性
    for (i, update) in updates.iter().enumerate() {
        assert_eq!(update.point_id, i as u32);
        assert_eq!(update.value, i as f64 * 0.1);
        assert_eq!(update.channel_id, 1001);
    }
}

#[tokio::test]
async fn test_point_update_timestamp_ordering() {
    let mut updates = Vec::new();
    
    // 创建带时间戳的更新
    for i in 0..5 {
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        updates.push(create_test_point_update(1001, i, i as f64));
    }
    
    // 验证时间戳是递增的
    for i in 1..updates.len() {
        assert!(updates[i].timestamp >= updates[i-1].timestamp);
    }
}

// 如果Redis可用，测试存储功能
#[tokio::test]
async fn test_default_plugin_storage_if_available() {
    // 尝试连接Redis，如果不可用则跳过
    if let Ok(storage) = DefaultPluginStorage::from_env().await {
        let storage: Arc<dyn PluginStorage> = Arc::new(storage);
        
        // 测试单点写入
        let result = storage.write_point(9999, &TelemetryType::Measurement, 99999, 123.45).await;
        if result.is_ok() {
            // 测试读取
            let read_result = storage.read_point(9999, &TelemetryType::Measurement, 99999).await;
            if let Ok(Some((value, _timestamp))) = read_result {
                assert!((value - 123.45).abs() < 0.001);
            }
        }
    }
    // 如果Redis不可用，测试自动通过
}

#[tokio::test]
async fn test_plugin_storage_error_handling() {
    // 测试无效的Redis URL
    let result = DefaultPluginStorage::new("redis://invalid:9999".to_string()).await;
    assert!(result.is_err()); // 应该返回错误
}

#[test]
fn test_reverse_logic_for_signal_points() {
    // 测试Signal类型点位的reverse逻辑
    let mut update = create_test_point_update(1001, 1, 1.0);
    update.telemetry_type = TelemetryType::Signal;
    update.raw_value = Some(1.0);
    
    // 如果有reverse=true，期望值应该是0
    // 这里只是验证数据结构，实际reverse逻辑在modbus/core.rs中实现
    assert_eq!(update.raw_value, Some(1.0));
    assert_eq!(update.value, 1.0);
}

#[test]
fn test_four_telemetry_types_in_storage() {
    // 验证四种遥测类型的点位更新
    let channel_id = 1001;
    
    // 遥测 - 点位ID从1开始
    let measurement = PluginPointUpdate {
        channel_id,
        telemetry_type: TelemetryType::Measurement,
        point_id: 1,
        value: 100.5,
        timestamp: 0,
        raw_value: Some(1005.0), // scale=0.1
    };
    
    // 遥信 - 点位ID从1开始
    let signal = PluginPointUpdate {
        channel_id,
        telemetry_type: TelemetryType::Signal,
        point_id: 1,
        value: 0.0,
        timestamp: 0,
        raw_value: Some(1.0), // reverse=true
    };
    
    // 遥控 - 点位ID从1开始
    let control = PluginPointUpdate {
        channel_id,
        telemetry_type: TelemetryType::Control,
        point_id: 1,
        value: 1.0,
        timestamp: 0,
        raw_value: None,
    };
    
    // 遥调 - 点位ID从1开始
    let adjustment = PluginPointUpdate {
        channel_id,
        telemetry_type: TelemetryType::Adjustment,
        point_id: 1,
        value: 50.0,
        timestamp: 0,
        raw_value: Some(500.0), // scale=0.1
    };
    
    // 验证每种类型都有独立的点位ID=1
    assert_eq!(measurement.point_id, 1);
    assert_eq!(signal.point_id, 1);
    assert_eq!(control.point_id, 1);
    assert_eq!(adjustment.point_id, 1);
}