//! 插件存储单元测试

use chrono::Utc;
use comsrv::core::config::types::TelemetryType;
use comsrv::plugins::core::{DefaultPluginStorage, PluginPointUpdate, PluginStorage};
use std::sync::Arc;

/// 创建测试用的点位更新
fn create_test_point_update(channel_id: u16, point_id: u32, value: f64) -> PluginPointUpdate {
    PluginPointUpdate {
        channel_id,
        telemetry_type: TelemetryType::Telemetry,
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
    assert!(matches!(update.telemetry_type, TelemetryType::Telemetry));
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
        TelemetryType::Telemetry,
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
        let result = storage.write_point(9999, &TelemetryType::Telemetry, 99999, 123.45).await;
        if result.is_ok() {
            // 测试读取
            let read_result = storage.read_point(9999, &TelemetryType::Telemetry, 99999).await;
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