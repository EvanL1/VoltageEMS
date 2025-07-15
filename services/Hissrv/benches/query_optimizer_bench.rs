//! 查询优化器性能基准测试

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use hissrv_rust::query_optimizer::{QueryOptimizer, QueryOptimizerConfig, DownsamplingConfig};
use hissrv_rust::storage::{AggregateFunction, QueryOptions};
use chrono::{Duration, Utc};
use tokio::runtime::Runtime;

fn create_test_optimizer() -> QueryOptimizer {
    let config = QueryOptimizerConfig {
        cache_enabled: true,
        cache_ttl_seconds: 300,
        cache_max_entries: 10000,
        parallel_queries_enabled: true,
        max_parallel_queries: 8,
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
                time_range_hours: 168,
                interval_seconds: 3600,
                aggregate: AggregateFunction::Mean,
            },
        ],
    };
    
    QueryOptimizer::new(config)
}

fn benchmark_query_plan_generation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("query_plan_generation");
    
    let time_ranges = vec![
        ("1_hour", 1),
        ("1_day", 24),
        ("1_week", 168),
        ("1_month", 720),
    ];
    
    for (name, hours) in time_ranges {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let optimizer = create_test_optimizer();
                    let end_time = Utc::now();
                    let start_time = end_time - Duration::hours(hours);
                    
                    let options = QueryOptions {
                        start_time: black_box(start_time),
                        end_time: black_box(end_time),
                        limit: None,
                        aggregate: None,
                        group_by: None,
                        fill: None,
                    };
                    
                    optimizer.create_query_plan("test_metric", &options).await
                });
            });
        });
    }
    
    group.finish();
}

fn benchmark_cache_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_operations");
    
    // 缓存写入性能
    group.bench_function("cache_write", |b| {
        b.iter(|| {
            rt.block_on(async {
                let optimizer = create_test_optimizer();
                let cache = &optimizer.cache;
                
                for i in 0..100 {
                    let key = format!("metric_{}", i);
                    let options = QueryOptions {
                        start_time: Utc::now() - Duration::hours(1),
                        end_time: Utc::now(),
                        limit: Some(i),
                        aggregate: None,
                        group_by: None,
                        fill: None,
                    };
                    
                    let entry = hissrv_rust::query_optimizer::CacheEntry {
                        key: key.clone(),
                        options: options.clone(),
                        data: vec![],
                        created_at: Utc::now(),
                        ttl_seconds: 300,
                        hit_count: 0,
                    };
                    
                    cache.put(&key, &options, entry).await;
                }
            });
        });
    });
    
    // 缓存读取性能
    group.bench_function("cache_read", |b| {
        let optimizer = create_test_optimizer();
        let cache = &optimizer.cache;
        
        // 预填充缓存
        rt.block_on(async {
            for i in 0..1000 {
                let key = format!("metric_{}", i);
                let options = QueryOptions {
                    start_time: Utc::now() - Duration::hours(1),
                    end_time: Utc::now(),
                    limit: Some(100),
                    aggregate: None,
                    group_by: None,
                    fill: None,
                };
                
                let entry = hissrv_rust::query_optimizer::CacheEntry {
                    key: key.clone(),
                    options: options.clone(),
                    data: vec![],
                    created_at: Utc::now(),
                    ttl_seconds: 300,
                    hit_count: 0,
                };
                
                cache.put(&key, &options, entry).await;
            }
        });
        
        b.iter(|| {
            rt.block_on(async {
                for i in 0..100 {
                    let key = format!("metric_{}", i % 1000);
                    let options = QueryOptions {
                        start_time: Utc::now() - Duration::hours(1),
                        end_time: Utc::now(),
                        limit: Some(100),
                        aggregate: None,
                        group_by: None,
                        fill: None,
                    };
                    
                    black_box(cache.get(&key, &options).await);
                }
            });
        });
    });
    
    group.finish();
}

fn benchmark_parallel_query_planning(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("parallel_query_planning");
    
    for num_queries in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_queries),
            num_queries,
            |b, &num_queries| {
                b.iter(|| {
                    rt.block_on(async {
                        let optimizer = create_test_optimizer();
                        let mut handles = vec![];
                        
                        for i in 0..num_queries {
                            let optimizer_clone = &optimizer;
                            let handle = tokio::spawn(async move {
                                let options = QueryOptions {
                                    start_time: Utc::now() - Duration::hours(24),
                                    end_time: Utc::now(),
                                    limit: None,
                                    aggregate: None,
                                    group_by: None,
                                    fill: None,
                                };
                                
                                optimizer_clone
                                    .create_query_plan(&format!("metric_{}", i), &options)
                                    .await
                            });
                            handles.push(handle);
                        }
                        
                        for handle in handles {
                            handle.await.unwrap();
                        }
                    });
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_downsampling_selection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("downsampling_selection");
    
    group.bench_function("downsampling_logic", |b| {
        b.iter(|| {
            rt.block_on(async {
                let optimizer = create_test_optimizer();
                
                // 测试不同时间范围的降采样选择
                for hours in [0.5, 1.0, 6.0, 24.0, 168.0, 720.0].iter() {
                    let options = QueryOptions {
                        start_time: Utc::now() - Duration::minutes((hours * 60.0) as i64),
                        end_time: Utc::now(),
                        limit: None,
                        aggregate: None,
                        group_by: None,
                        fill: None,
                    };
                    
                    black_box(optimizer.create_query_plan("test_metric", &options).await);
                }
            });
        });
    });
    
    group.finish();
}

fn benchmark_cache_eviction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_eviction");
    
    group.bench_function("lru_eviction", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut config = create_test_optimizer().config;
                config.cache_max_entries = 100; // 小缓存以触发驱逐
                let optimizer = QueryOptimizer::new(config);
                let cache = &optimizer.cache;
                
                // 填充超过缓存容量的条目
                for i in 0..200 {
                    let key = format!("metric_{}", i);
                    let options = QueryOptions {
                        start_time: Utc::now() - Duration::hours(1),
                        end_time: Utc::now(),
                        limit: Some(i),
                        aggregate: None,
                        group_by: None,
                        fill: None,
                    };
                    
                    let entry = hissrv_rust::query_optimizer::CacheEntry {
                        key: key.clone(),
                        options: options.clone(),
                        data: vec![],
                        created_at: Utc::now(),
                        ttl_seconds: 300,
                        hit_count: 0,
                    };
                    
                    cache.put(&key, &options, entry).await;
                }
            });
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_query_plan_generation,
    benchmark_cache_operations,
    benchmark_parallel_query_planning,
    benchmark_downsampling_selection,
    benchmark_cache_eviction
);
criterion_main!(benches);