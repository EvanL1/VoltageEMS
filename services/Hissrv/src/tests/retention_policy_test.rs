//! 保留策略测试

use crate::retention_policy::{
    RetentionExecutor, RetentionPolicy, RetentionPolicyConfig, RetentionPolicyManager,
    RetentionStats,
};
use crate::storage::{DataPoint, DataValue, QueryOptions};
use crate::tests::mock_storage::create_memory_storage;
use crate::tests::test_utils::create_time_series_data;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 创建测试用的保留策略
fn create_test_policies() -> Vec<RetentionPolicy> {
    vec![
        RetentionPolicy {
            name: "raw_data".to_string(),
            pattern: "raw.*".to_string(),
            retention_days: 7,
            downsampling: None,
            enabled: true,
        },
        RetentionPolicy {
            name: "aggregated_data".to_string(),
            pattern: "agg.*".to_string(),
            retention_days: 30,
            downsampling: Some(crate::retention_policy::DownsamplingPolicy {
                interval_seconds: 300,
                aggregate_function: crate::storage::AggregateFunction::Mean,
                retention_days: 90,
            }),
            enabled: true,
        },
        RetentionPolicy {
            name: "long_term".to_string(),
            pattern: "long.*".to_string(),
            retention_days: 365,
            downsampling: None,
            enabled: true,
        },
    ]
}

#[tokio::test]
async fn test_retention_policy_matching() {
    let policies = create_test_policies();
    let manager = RetentionPolicyManager::new(policies.clone());
    
    // 测试模式匹配
    assert!(manager.find_policy("raw.temperature").is_some());
    assert_eq!(manager.find_policy("raw.temperature").unwrap().name, "raw_data");
    
    assert!(manager.find_policy("agg.hourly").is_some());
    assert_eq!(manager.find_policy("agg.hourly").unwrap().name, "aggregated_data");
    
    assert!(manager.find_policy("long.archive").is_some());
    assert_eq!(manager.find_policy("long.archive").unwrap().name, "long_term");
    
    assert!(manager.find_policy("unknown.metric").is_none());
}

#[tokio::test]
async fn test_retention_policy_crud() {
    let mut manager = RetentionPolicyManager::new(vec![]);
    
    // 添加策略
    let policy = RetentionPolicy {
        name: "test_policy".to_string(),
        pattern: "test.*".to_string(),
        retention_days: 30,
        downsampling: None,
        enabled: true,
    };
    
    manager.add_policy(policy.clone());
    assert_eq!(manager.list_policies().len(), 1);
    
    // 更新策略
    let mut updated_policy = policy.clone();
    updated_policy.retention_days = 60;
    manager.update_policy("test_policy", updated_policy);
    
    let found = manager.find_policy_by_name("test_policy").unwrap();
    assert_eq!(found.retention_days, 60);
    
    // 删除策略
    manager.remove_policy("test_policy");
    assert_eq!(manager.list_policies().len(), 0);
}

#[tokio::test]
async fn test_retention_executor() {
    let mut storage = create_memory_storage();
    storage.connect().await.unwrap();
    
    // 创建测试数据 - 包括过期和未过期的数据
    let now = Utc::now();
    let old_data_start = now - Duration::days(10);
    let recent_data_start = now - Duration::days(2);
    
    // 添加旧数据
    let old_data = create_time_series_data("raw.temperature", old_data_start, 100, 60);
    for point in old_data {
        storage.write(point).await.unwrap();
    }
    
    // 添加新数据
    let recent_data = create_time_series_data("raw.temperature", recent_data_start, 100, 60);
    for point in recent_data {
        storage.write(point).await.unwrap();
    }
    
    // 创建保留策略（7天）
    let policy = RetentionPolicy {
        name: "raw_data".to_string(),
        pattern: "raw.*".to_string(),
        retention_days: 7,
        downsampling: None,
        enabled: true,
    };
    
    let config = RetentionPolicyConfig {
        policies: vec![policy],
        check_interval_minutes: 60,
        batch_size: 1000,
        dry_run: false,
    };
    
    // 执行保留策略
    let storage_arc = Arc::new(RwLock::new(storage));
    let executor = RetentionExecutor::new(config, storage_arc.clone());
    
    let stats = executor.execute_retention("raw.temperature").await.unwrap();
    
    // 验证统计
    assert!(stats.points_deleted > 0);
    assert_eq!(stats.keys_processed, 1);
    
    // 验证旧数据被删除
    let storage = storage_arc.read().await;
    let query_result = storage.query("raw.temperature", QueryOptions {
        start_time: old_data_start,
        end_time: now,
        limit: None,
        aggregate: None,
        group_by: None,
        fill: None,
    }).await.unwrap();
    
    // 只应该保留最近7天的数据
    for point in &query_result.points {
        let age = now.signed_duration_since(point.timestamp);
        assert!(age < Duration::days(7));
    }
}

#[tokio::test]
async fn test_retention_with_downsampling() {
    let mut storage = create_memory_storage();
    storage.connect().await.unwrap();
    
    // 创建高频率数据
    let now = Utc::now();
    let start_time = now - Duration::days(35);
    
    // 每分钟一个数据点，35天的数据
    let data = create_time_series_data("agg.metrics", start_time, 35 * 24 * 60, 60);
    for point in data {
        storage.write(point).await.unwrap();
    }
    
    // 创建带降采样的保留策略
    let policy = RetentionPolicy {
        name: "aggregated_data".to_string(),
        pattern: "agg.*".to_string(),
        retention_days: 30,
        downsampling: Some(crate::retention_policy::DownsamplingPolicy {
            interval_seconds: 3600, // 1小时
            aggregate_function: crate::storage::AggregateFunction::Mean,
            retention_days: 90,
        }),
        enabled: true,
    };
    
    let config = RetentionPolicyConfig {
        policies: vec![policy],
        check_interval_minutes: 60,
        batch_size: 1000,
        dry_run: false,
    };
    
    let storage_arc = Arc::new(RwLock::new(storage));
    let executor = RetentionExecutor::new(config, storage_arc.clone());
    
    // 执行降采样
    let stats = executor.execute_downsampling("agg.metrics").await.unwrap();
    
    // 验证降采样统计
    assert!(stats.points_downsampled > 0);
    assert!(stats.downsampled_points_created > 0);
    
    // 降采样后的点数应该大约是原始点数的1/60（从每分钟到每小时）
    assert!(stats.downsampled_points_created < stats.points_downsampled / 50);
}

#[tokio::test]
async fn test_retention_dry_run() {
    let mut storage = create_memory_storage();
    storage.connect().await.unwrap();
    
    // 添加将被删除的数据
    let now = Utc::now();
    let old_time = now - Duration::days(10);
    
    let old_data = create_time_series_data("raw.test", old_time, 100, 60);
    for point in old_data {
        storage.write(point).await.unwrap();
    }
    
    let initial_count = storage.get_data_count().await;
    
    // 创建干运行模式的保留策略
    let policy = RetentionPolicy {
        name: "raw_data".to_string(),
        pattern: "raw.*".to_string(),
        retention_days: 7,
        downsampling: None,
        enabled: true,
    };
    
    let config = RetentionPolicyConfig {
        policies: vec![policy],
        check_interval_minutes: 60,
        batch_size: 1000,
        dry_run: true, // 干运行模式
    };
    
    let storage_arc = Arc::new(RwLock::new(storage));
    let executor = RetentionExecutor::new(config, storage_arc.clone());
    
    let stats = executor.execute_retention("raw.test").await.unwrap();
    
    // 验证统计显示将删除的数据
    assert!(stats.points_deleted > 0);
    
    // 验证实际上没有删除任何数据
    let final_count = storage_arc.read().await.get_data_count().await;
    assert_eq!(initial_count, final_count);
}

#[tokio::test]
async fn test_retention_stats_aggregation() {
    let mut stats = RetentionStats::default();
    
    stats.keys_processed = 10;
    stats.points_scanned = 10000;
    stats.points_deleted = 5000;
    stats.points_downsampled = 2000;
    stats.downsampled_points_created = 100;
    stats.execution_time_ms = 1500;
    
    // 测试统计计算
    assert_eq!(stats.deletion_rate(), 50.0);
    assert_eq!(stats.downsampling_ratio(), 20.0);
    
    // 测试合并统计
    let mut other_stats = RetentionStats::default();
    other_stats.keys_processed = 5;
    other_stats.points_scanned = 5000;
    other_stats.points_deleted = 2500;
    
    stats.merge(&other_stats);
    
    assert_eq!(stats.keys_processed, 15);
    assert_eq!(stats.points_scanned, 15000);
    assert_eq!(stats.points_deleted, 7500);
}

#[tokio::test]
async fn test_concurrent_retention_execution() {
    let mut storage = create_memory_storage();
    storage.connect().await.unwrap();
    
    // 创建多个键的数据
    let now = Utc::now();
    let old_time = now - Duration::days(10);
    
    for i in 0..5 {
        let key = format!("raw.metric_{}", i);
        let data = create_time_series_data(&key, old_time, 100, 60);
        for point in data {
            storage.write(point).await.unwrap();
        }
    }
    
    let policy = RetentionPolicy {
        name: "raw_data".to_string(),
        pattern: "raw.*".to_string(),
        retention_days: 7,
        downsampling: None,
        enabled: true,
    };
    
    let config = RetentionPolicyConfig {
        policies: vec![policy],
        check_interval_minutes: 60,
        batch_size: 1000,
        dry_run: false,
    };
    
    let storage_arc = Arc::new(RwLock::new(storage));
    let executor = Arc::new(RetentionExecutor::new(config, storage_arc));
    
    // 并发执行保留策略
    let mut handles = vec![];
    for i in 0..5 {
        let executor_clone = executor.clone();
        let handle = tokio::spawn(async move {
            let key = format!("raw.metric_{}", i);
            executor_clone.execute_retention(&key).await
        });
        handles.push(handle);
    }
    
    // 收集结果
    let mut total_stats = RetentionStats::default();
    for handle in handles {
        let stats = handle.await.unwrap().unwrap();
        total_stats.merge(&stats);
    }
    
    // 验证所有键都被处理
    assert_eq!(total_stats.keys_processed, 5);
    assert!(total_stats.points_deleted > 0);
}

#[tokio::test]
async fn test_retention_policy_validation() {
    // 测试无效的保留策略
    let invalid_policies = vec![
        RetentionPolicy {
            name: "".to_string(), // 空名称
            pattern: "test.*".to_string(),
            retention_days: 30,
            downsampling: None,
            enabled: true,
        },
        RetentionPolicy {
            name: "test".to_string(),
            pattern: "".to_string(), // 空模式
            retention_days: 30,
            downsampling: None,
            enabled: true,
        },
        RetentionPolicy {
            name: "test".to_string(),
            pattern: "test.*".to_string(),
            retention_days: 0, // 无效的保留天数
            downsampling: None,
            enabled: true,
        },
    ];
    
    for policy in invalid_policies {
        assert!(!policy.is_valid());
    }
    
    // 测试有效的保留策略
    let valid_policy = RetentionPolicy {
        name: "valid".to_string(),
        pattern: "test.*".to_string(),
        retention_days: 30,
        downsampling: None,
        enabled: true,
    };
    
    assert!(valid_policy.is_valid());
}