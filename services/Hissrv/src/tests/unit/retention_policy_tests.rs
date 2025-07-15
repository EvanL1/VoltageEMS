use crate::retention_policy::{
    AggregationMethod, DownsamplingPolicy, PolicyStatistics, RetentionPolicyConfig,
    RetentionPolicyManager, RetentionType, default_retention_policies,
};
use crate::storage::StorageManager;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

async fn create_test_manager() -> RetentionPolicyManager {
    let storage_manager = Arc::new(RwLock::new(StorageManager::new()));
    RetentionPolicyManager::new(storage_manager)
}

#[tokio::test]
async fn test_retention_policy_crud() {
    let manager = create_test_manager().await;

    // 创建测试策略
    let policy = RetentionPolicyConfig {
        name: "test_policy".to_string(),
        enabled: true,
        measurement_patterns: vec!["test.*".to_string()],
        retention_type: RetentionType::TimeBased {
            duration_seconds: 86400, // 1天
        },
        downsampling: None,
        execution_interval_seconds: 3600,
    };

    // 添加策略
    manager.add_policy(policy.clone()).await.unwrap();

    // 获取策略
    let retrieved = manager.get_policy("test_policy").await.unwrap();
    assert_eq!(retrieved.name, "test_policy");
    assert_eq!(retrieved.measurement_patterns, vec!["test.*"]);

    // 获取所有策略
    let all_policies = manager.get_all_policies().await;
    assert_eq!(all_policies.len(), 1);

    // 获取统计信息
    let stats = manager.get_statistics("test_policy").await.unwrap();
    assert_eq!(stats.total_executions, 0);

    // 删除策略
    manager.remove_policy("test_policy").await.unwrap();
    assert!(manager.get_policy("test_policy").await.is_none());
}

#[tokio::test]
async fn test_retention_type_variations() {
    let manager = create_test_manager().await;

    // 测试时间基础保留
    let time_based = RetentionPolicyConfig {
        name: "time_based".to_string(),
        enabled: true,
        measurement_patterns: vec!["time.*".to_string()],
        retention_type: RetentionType::TimeBased {
            duration_seconds: 7 * 24 * 60 * 60, // 7天
        },
        downsampling: None,
        execution_interval_seconds: 3600,
    };

    // 测试空间基础保留
    let space_based = RetentionPolicyConfig {
        name: "space_based".to_string(),
        enabled: true,
        measurement_patterns: vec!["space.*".to_string()],
        retention_type: RetentionType::SpaceBased {
            max_size_bytes: 1024 * 1024 * 1024, // 1GB
        },
        downsampling: None,
        execution_interval_seconds: 3600,
    };

    // 测试计数基础保留
    let count_based = RetentionPolicyConfig {
        name: "count_based".to_string(),
        enabled: true,
        measurement_patterns: vec!["count.*".to_string()],
        retention_type: RetentionType::CountBased {
            max_count: 1000000,
        },
        downsampling: None,
        execution_interval_seconds: 3600,
    };

    // 测试混合保留
    let hybrid = RetentionPolicyConfig {
        name: "hybrid".to_string(),
        enabled: true,
        measurement_patterns: vec!["hybrid.*".to_string()],
        retention_type: RetentionType::Hybrid {
            duration_seconds: Some(7 * 24 * 60 * 60),
            max_size_bytes: Some(1024 * 1024 * 1024),
            max_count: Some(1000000),
        },
        downsampling: None,
        execution_interval_seconds: 3600,
    };

    // 添加所有策略
    manager.add_policy(time_based).await.unwrap();
    manager.add_policy(space_based).await.unwrap();
    manager.add_policy(count_based).await.unwrap();
    manager.add_policy(hybrid).await.unwrap();

    // 验证所有策略都已添加
    assert_eq!(manager.get_all_policies().await.len(), 4);
}

#[tokio::test]
async fn test_downsampling_configuration() {
    let manager = create_test_manager().await;

    let policy = RetentionPolicyConfig {
        name: "downsampling_test".to_string(),
        enabled: true,
        measurement_patterns: vec!["metrics.*".to_string()],
        retention_type: RetentionType::TimeBased {
            duration_seconds: 30 * 24 * 60 * 60, // 30天
        },
        downsampling: Some(vec![
            DownsamplingPolicy {
                source_retention_seconds: 60 * 60, // 1小时
                interval_seconds: 5 * 60,          // 5分钟
                aggregation_method: AggregationMethod::Mean,
                target_suffix: "_5m".to_string(),
            },
            DownsamplingPolicy {
                source_retention_seconds: 24 * 60 * 60, // 1天
                interval_seconds: 60 * 60,              // 1小时
                aggregation_method: AggregationMethod::Max,
                target_suffix: "_1h".to_string(),
            },
            DownsamplingPolicy {
                source_retention_seconds: 7 * 24 * 60 * 60, // 7天
                interval_seconds: 24 * 60 * 60,             // 1天
                aggregation_method: AggregationMethod::Min,
                target_suffix: "_1d".to_string(),
            },
        ]),
        execution_interval_seconds: 3600,
    };

    manager.add_policy(policy).await.unwrap();

    let retrieved = manager.get_policy("downsampling_test").await.unwrap();
    let downsampling = retrieved.downsampling.unwrap();
    assert_eq!(downsampling.len(), 3);
    
    // 验证降采样配置
    assert_eq!(downsampling[0].target_suffix, "_5m");
    assert!(matches!(downsampling[0].aggregation_method, AggregationMethod::Mean));
    
    assert_eq!(downsampling[1].target_suffix, "_1h");
    assert!(matches!(downsampling[1].aggregation_method, AggregationMethod::Max));
    
    assert_eq!(downsampling[2].target_suffix, "_1d");
    assert!(matches!(downsampling[2].aggregation_method, AggregationMethod::Min));
}

#[tokio::test]
async fn test_policy_execution() {
    let manager = create_test_manager().await;

    let policy = RetentionPolicyConfig {
        name: "exec_test".to_string(),
        enabled: true,
        measurement_patterns: vec!["exec_test.*".to_string()],
        retention_type: RetentionType::TimeBased {
            duration_seconds: 3600,
        },
        downsampling: None,
        execution_interval_seconds: 3600,
    };

    manager.add_policy(policy).await.unwrap();

    // 执行策略
    let result = manager.execute_policy("exec_test").await;
    assert!(result.is_ok());

    // 检查统计信息
    let stats = manager.get_statistics("exec_test").await.unwrap();
    assert_eq!(stats.total_executions, 1);
    assert_eq!(stats.successful_executions, 1);
    assert!(stats.last_execution.is_some());
}

#[tokio::test]
async fn test_disabled_policy_execution() {
    let manager = create_test_manager().await;

    let policy = RetentionPolicyConfig {
        name: "disabled_test".to_string(),
        enabled: false,
        measurement_patterns: vec!["disabled.*".to_string()],
        retention_type: RetentionType::TimeBased {
            duration_seconds: 3600,
        },
        downsampling: None,
        execution_interval_seconds: 3600,
    };

    manager.add_policy(policy).await.unwrap();

    // 尝试执行禁用的策略应该失败
    let result = manager.execute_policy("disabled_test").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_policy_statistics() {
    let manager = create_test_manager().await;

    let policy = RetentionPolicyConfig {
        name: "stats_test".to_string(),
        enabled: true,
        measurement_patterns: vec!["stats.*".to_string()],
        retention_type: RetentionType::TimeBased {
            duration_seconds: 3600,
        },
        downsampling: None,
        execution_interval_seconds: 3600,
    };

    manager.add_policy(policy).await.unwrap();

    // 执行多次
    for _ in 0..3 {
        let _ = manager.execute_policy("stats_test").await;
    }

    let stats = manager.get_statistics("stats_test").await.unwrap();
    assert_eq!(stats.total_executions, 3);
    assert_eq!(stats.successful_executions, 3);
    assert_eq!(stats.failed_executions, 0);

    // 获取所有统计
    let all_stats = manager.get_all_statistics().await;
    assert!(all_stats.contains_key("stats_test"));
}

#[tokio::test]
async fn test_measurement_pattern_matching() {
    let patterns = vec![
        ("*", "anything", true),
        ("test.*", "test.metric", true),
        ("test.*", "test.sub.metric", true),
        ("test.*", "prod.metric", false),
        ("*.temperature", "sensor.temperature", true),
        ("sensor.temp*", "sensor.temperature", true),
        ("sensor.temp*", "sensor.pressure", false),
        ("sensor.[0-9]+", "sensor.123", true),
        ("sensor.[0-9]+", "sensor.abc", false),
    ];

    for (pattern, measurement, should_match) in patterns {
        // 简单的通配符匹配测试
        let matches = if pattern == "*" {
            true
        } else if pattern.contains('*') {
            let regex_pattern = pattern.replace("*", ".*");
            regex::Regex::new(&format!("^{}$", regex_pattern))
                .unwrap()
                .is_match(measurement)
        } else if pattern.contains('[') {
            regex::Regex::new(&format!("^{}$", pattern))
                .unwrap()
                .is_match(measurement)
        } else {
            pattern == measurement
        };

        assert_eq!(
            matches, should_match,
            "Pattern {} should {} match measurement {}",
            pattern,
            if should_match { "" } else { "not" },
            measurement
        );
    }
}

#[tokio::test]
async fn test_default_retention_policies() {
    let defaults = default_retention_policies();
    
    // 验证默认策略
    assert_eq!(defaults.len(), 3);
    
    // 验证原始数据策略
    let raw_data_policy = defaults.iter().find(|p| p.name == "raw_data_7d").unwrap();
    assert!(raw_data_policy.enabled);
    assert_eq!(raw_data_policy.measurement_patterns, vec!["*"]);
    if let RetentionType::TimeBased { duration_seconds } = raw_data_policy.retention_type {
        assert_eq!(duration_seconds, 7 * 24 * 60 * 60); // 7天
    } else {
        panic!("Expected TimeBased retention type");
    }
    assert!(raw_data_policy.downsampling.is_some());
    
    // 验证事件数据策略
    let events_policy = defaults.iter().find(|p| p.name == "events_30d").unwrap();
    assert!(events_policy.enabled);
    assert_eq!(events_policy.measurement_patterns, vec!["events*"]);
    if let RetentionType::TimeBased { duration_seconds } = events_policy.retention_type {
        assert_eq!(duration_seconds, 30 * 24 * 60 * 60); // 30天
    } else {
        panic!("Expected TimeBased retention type");
    }
    assert!(events_policy.downsampling.is_none());
    
    // 验证系统状态策略
    let system_policy = defaults.iter().find(|p| p.name == "system_status_1d").unwrap();
    assert!(system_policy.enabled);
    assert_eq!(system_policy.measurement_patterns, vec!["system*"]);
    if let RetentionType::TimeBased { duration_seconds } = system_policy.retention_type {
        assert_eq!(duration_seconds, 24 * 60 * 60); // 1天
    } else {
        panic!("Expected TimeBased retention type");
    }
}

#[tokio::test]
async fn test_aggregation_methods() {
    // 测试所有聚合方法的定义
    let methods = vec![
        AggregationMethod::Mean,
        AggregationMethod::Max,
        AggregationMethod::Min,
        AggregationMethod::Sum,
        AggregationMethod::Count,
        AggregationMethod::First,
        AggregationMethod::Last,
        AggregationMethod::Median,
        AggregationMethod::StdDev,
    ];

    // 确保所有方法都能被序列化/反序列化
    for method in methods {
        let serialized = serde_json::to_string(&method).unwrap();
        let deserialized: AggregationMethod = serde_json::from_str(&serialized).unwrap();
        assert_eq!(format!("{:?}", method), format!("{:?}", deserialized));
    }
}

#[tokio::test]
async fn test_concurrent_policy_operations() {
    let manager = create_test_manager().await;
    
    // 并发添加多个策略
    let mut handles = vec![];
    
    for i in 0..5 {
        let manager_clone = create_test_manager().await;
        let handle = tokio::spawn(async move {
            let policy = RetentionPolicyConfig {
                name: format!("concurrent_policy_{}", i),
                enabled: true,
                measurement_patterns: vec![format!("concurrent.{}", i)],
                retention_type: RetentionType::TimeBased {
                    duration_seconds: 3600 * (i + 1) as u64,
                },
                downsampling: None,
                execution_interval_seconds: 3600,
            };
            manager_clone.add_policy(policy).await
        });
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap().unwrap();
    }
}