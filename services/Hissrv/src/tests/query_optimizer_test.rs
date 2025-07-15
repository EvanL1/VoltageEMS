//! 查询优化器测试

use crate::query_optimizer::{
    CacheEntry, DownsamplingConfig, QueryCache, QueryOptimizer, QueryOptimizerConfig,
    QueryPlan, QueryStats,
};
use crate::storage::{AggregateFunction, QueryOptions};
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 创建测试用的查询优化器配置
fn create_test_optimizer_config() -> QueryOptimizerConfig {
    QueryOptimizerConfig {
        cache_enabled: true,
        cache_ttl_seconds: 300,
        cache_max_entries: 1000,
        parallel_queries_enabled: true,
        max_parallel_queries: 4,
        query_timeout_seconds: 30,
        downsampling_enabled: true,
        downsampling_configs: vec![
            DownsamplingConfig {
                time_range_hours: 1,
                interval_seconds: 60,
                aggregate: AggregateFunction::Mean,
            },
            DownsamplingConfig {
                time_range_hours: 24,
                interval_seconds: 300,
                aggregate: AggregateFunction::Mean,
            },
            DownsamplingConfig {
                time_range_hours: 168, // 7 days
                interval_seconds: 3600,
                aggregate: AggregateFunction::Mean,
            },
        ],
    }
}

#[tokio::test]
async fn test_query_optimizer_creation() {
    let config = create_test_optimizer_config();
    let optimizer = QueryOptimizer::new(config.clone());
    
    assert!(optimizer.config.cache_enabled);
    assert_eq!(optimizer.config.cache_ttl_seconds, 300);
    assert_eq!(optimizer.config.downsampling_configs.len(), 3);
}

#[tokio::test]
async fn test_query_plan_generation() {
    let config = create_test_optimizer_config();
    let optimizer = QueryOptimizer::new(config);
    
    let end_time = Utc::now();
    let start_time = end_time - Duration::hours(2);
    
    let options = QueryOptions {
        start_time,
        end_time,
        limit: Some(1000),
        aggregate: None,
        group_by: None,
        fill: None,
    };
    
    let plan = optimizer.create_query_plan("test_metric", &options).await;
    
    // 验证查询计划
    assert_eq!(plan.key, "test_metric");
    assert_eq!(plan.original_options.start_time, start_time);
    assert_eq!(plan.original_options.end_time, end_time);
    
    // 2小时的查询应该使用5分钟的降采样
    assert!(plan.use_downsampling);
    assert_eq!(plan.downsampling_interval, Some(300));
    assert!(matches!(
        plan.downsampling_aggregate,
        Some(AggregateFunction::Mean)
    ));
}

#[tokio::test]
async fn test_downsampling_selection() {
    let config = create_test_optimizer_config();
    let optimizer = QueryOptimizer::new(config);
    
    let end_time = Utc::now();
    
    // 测试不同时间范围的降采样选择
    let test_cases = vec![
        (30, None), // 30分钟 - 不降采样
        (90, Some(60)), // 1.5小时 - 1分钟降采样
        (12 * 60, Some(300)), // 12小时 - 5分钟降采样
        (3 * 24 * 60, Some(3600)), // 3天 - 1小时降采样
    ];
    
    for (minutes, expected_interval) in test_cases {
        let options = QueryOptions {
            start_time: end_time - Duration::minutes(minutes),
            end_time,
            limit: None,
            aggregate: None,
            group_by: None,
            fill: None,
        };
        
        let plan = optimizer.create_query_plan("test_metric", &options).await;
        
        assert_eq!(
            plan.downsampling_interval, expected_interval,
            "Failed for {} minutes", minutes
        );
    }
}

#[tokio::test]
async fn test_query_cache() {
    let cache = QueryCache::new(100, 60);
    
    let key = "test_metric";
    let options = QueryOptions {
        start_time: Utc::now() - Duration::hours(1),
        end_time: Utc::now(),
        limit: Some(100),
        aggregate: Some(AggregateFunction::Mean),
        group_by: None,
        fill: None,
    };
    
    // 缓存应该初始为空
    assert!(cache.get(key, &options).await.is_none());
    
    // 添加缓存条目
    let entry = CacheEntry {
        key: key.to_string(),
        options: options.clone(),
        data: vec![],
        created_at: Utc::now(),
        ttl_seconds: 60,
        hit_count: 0,
    };
    
    cache.put(key, &options, entry.clone()).await;
    
    // 应该能获取到缓存
    let cached = cache.get(key, &options).await;
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().key, key);
}

#[tokio::test]
async fn test_cache_expiration() {
    let cache = QueryCache::new(100, 1); // 1秒TTL
    
    let key = "test_metric";
    let options = QueryOptions {
        start_time: Utc::now() - Duration::hours(1),
        end_time: Utc::now(),
        limit: None,
        aggregate: None,
        group_by: None,
        fill: None,
    };
    
    let entry = CacheEntry {
        key: key.to_string(),
        options: options.clone(),
        data: vec![],
        created_at: Utc::now() - Duration::seconds(2), // 已过期
        ttl_seconds: 1,
        hit_count: 0,
    };
    
    cache.put(key, &options, entry).await;
    
    // 过期的条目应该返回None
    assert!(cache.get(key, &options).await.is_none());
}

#[tokio::test]
async fn test_query_stats() {
    let stats = Arc::new(RwLock::new(QueryStats::default()));
    
    // 模拟查询统计
    {
        let mut s = stats.write().await;
        s.total_queries += 10;
        s.cache_hits += 7;
        s.cache_misses += 3;
        s.total_query_time_ms += 1500.0;
        s.downsampled_queries += 5;
    }
    
    let s = stats.read().await;
    assert_eq!(s.total_queries, 10);
    assert_eq!(s.cache_hit_rate(), 70.0);
    assert_eq!(s.average_query_time(), 150.0);
    assert_eq!(s.downsampling_rate(), 50.0);
}

#[tokio::test]
async fn test_parallel_query_planning() {
    let config = QueryOptimizerConfig {
        parallel_queries_enabled: true,
        max_parallel_queries: 4,
        ..create_test_optimizer_config()
    };
    
    let optimizer = QueryOptimizer::new(config);
    
    let keys = vec!["metric1", "metric2", "metric3", "metric4", "metric5"];
    let options = QueryOptions {
        start_time: Utc::now() - Duration::hours(1),
        end_time: Utc::now(),
        limit: None,
        aggregate: None,
        group_by: None,
        fill: None,
    };
    
    // 生成并行查询计划
    let mut plans = vec![];
    for key in &keys {
        let plan = optimizer.create_query_plan(key, &options).await;
        plans.push(plan);
    }
    
    // 验证所有计划都已创建
    assert_eq!(plans.len(), 5);
    for (i, plan) in plans.iter().enumerate() {
        assert_eq!(plan.key, keys[i]);
    }
}

#[tokio::test]
async fn test_query_optimization_with_aggregation() {
    let config = create_test_optimizer_config();
    let optimizer = QueryOptimizer::new(config);
    
    // 已经有聚合函数的查询不应该再添加降采样聚合
    let options = QueryOptions {
        start_time: Utc::now() - Duration::hours(24),
        end_time: Utc::now(),
        limit: None,
        aggregate: Some(AggregateFunction::Max), // 用户指定的聚合
        group_by: None,
        fill: None,
    };
    
    let plan = optimizer.create_query_plan("test_metric", &options).await;
    
    // 即使时间范围很大，如果已有聚合函数，不应该使用降采样
    assert!(!plan.use_downsampling);
    assert!(plan.downsampling_interval.is_none());
}

#[tokio::test]
async fn test_cache_key_generation() {
    let cache = QueryCache::new(100, 300);
    
    let options1 = QueryOptions {
        start_time: Utc::now() - Duration::hours(1),
        end_time: Utc::now(),
        limit: Some(100),
        aggregate: Some(AggregateFunction::Mean),
        group_by: None,
        fill: None,
    };
    
    let options2 = QueryOptions {
        start_time: Utc::now() - Duration::hours(1),
        end_time: Utc::now(),
        limit: Some(200), // 不同的limit
        aggregate: Some(AggregateFunction::Mean),
        group_by: None,
        fill: None,
    };
    
    // 创建缓存条目
    let entry1 = CacheEntry {
        key: "metric".to_string(),
        options: options1.clone(),
        data: vec![],
        created_at: Utc::now(),
        ttl_seconds: 300,
        hit_count: 0,
    };
    
    cache.put("metric", &options1, entry1).await;
    
    // 不同的options应该产生cache miss
    assert!(cache.get("metric", &options1).await.is_some());
    assert!(cache.get("metric", &options2).await.is_none());
}

#[tokio::test]
async fn test_optimizer_with_disabled_features() {
    let config = QueryOptimizerConfig {
        cache_enabled: false,
        parallel_queries_enabled: false,
        downsampling_enabled: false,
        ..create_test_optimizer_config()
    };
    
    let optimizer = QueryOptimizer::new(config);
    
    let options = QueryOptions {
        start_time: Utc::now() - Duration::hours(24),
        end_time: Utc::now(),
        limit: None,
        aggregate: None,
        group_by: None,
        fill: None,
    };
    
    let plan = optimizer.create_query_plan("test_metric", &options).await;
    
    // 所有优化都应该被禁用
    assert!(!plan.use_cache);
    assert!(!plan.use_downsampling);
    assert!(!plan.use_parallel);
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let cache = Arc::new(QueryCache::new(1000, 300));
    
    // 并发读写缓存
    let mut handles = vec![];
    
    for i in 0..10 {
        let cache_clone = cache.clone();
        let handle = tokio::spawn(async move {
            let key = format!("metric_{}", i);
            let options = QueryOptions {
                start_time: Utc::now() - Duration::hours(1),
                end_time: Utc::now(),
                limit: Some(100),
                aggregate: None,
                group_by: None,
                fill: None,
            };
            
            // 写入缓存
            let entry = CacheEntry {
                key: key.clone(),
                options: options.clone(),
                data: vec![],
                created_at: Utc::now(),
                ttl_seconds: 300,
                hit_count: 0,
            };
            
            cache_clone.put(&key, &options, entry).await;
            
            // 读取缓存
            for _ in 0..5 {
                let _ = cache_clone.get(&key, &options).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        });
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }
    
    // 验证缓存统计
    let stats = cache.get_stats().await;
    assert!(stats.total_gets > 0);
    assert!(stats.total_puts > 0);
}