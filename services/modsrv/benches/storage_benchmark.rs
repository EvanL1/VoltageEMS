//! modsrv存储性能基准测试
//!
//! 使用criterion进行精确的性能测量

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use modsrv::storage::{
    make_control_key, make_monitor_key, ModelStorage, MonitorKey, MonitorType, MonitorUpdate,
    MonitorValue,
};
use std::time::Duration;
use tokio::runtime::Runtime;

/// 创建测试用的监视值更新
fn create_monitor_updates(count: usize, model_id: &str) -> Vec<MonitorUpdate> {
    (0..count)
        .map(|i| MonitorUpdate {
            model_id: model_id.to_string(),
            monitor_type: MonitorType::ModelOutput,
            point_id: 10000 + i as u32,
            value: MonitorValue::new(100.0 + i as f64, model_id.to_string()),
        })
        .collect()
}

/// 创建测试用的监视键
fn create_monitor_keys(count: usize, model_id: &str) -> Vec<MonitorKey> {
    (0..count)
        .map(|i| MonitorKey {
            model_id: model_id.to_string(),
            monitor_type: MonitorType::ModelOutput,
            point_id: 10000 + i as u32,
        })
        .collect()
}

/// 批量写入性能测试
fn bench_batch_write(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let mut group = c.benchmark_group("batch_write");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_function(format!("write_{}_points", size), |b| {
            b.iter_batched(
                || {
                    let storage = rt.block_on(ModelStorage::new(&redis_url)).unwrap();
                    let updates = create_monitor_updates(*size, "bench_model");
                    (storage, updates)
                },
                |(mut storage, updates)| {
                    rt.block_on(async {
                        storage
                            .set_monitor_values(black_box(&updates))
                            .await
                            .unwrap();
                    });
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// 批量读取性能测试
fn bench_batch_read(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    // 预先写入测试数据
    rt.block_on(async {
        let mut storage = ModelStorage::new(&redis_url).await.unwrap();
        let updates = create_monitor_updates(1000, "bench_read_model");
        storage.set_monitor_values(&updates).await.unwrap();
    });

    let mut group = c.benchmark_group("batch_read");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_function(format!("read_{}_points", size), |b| {
            b.iter_batched(
                || {
                    let storage = rt.block_on(ModelStorage::new(&redis_url)).unwrap();
                    let keys = create_monitor_keys(*size, "bench_read_model");
                    (storage, keys)
                },
                |(mut storage, keys)| {
                    rt.block_on(async {
                        let _values = storage.get_monitor_values(black_box(&keys)).await.unwrap();
                    });
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// 键生成性能测试
fn bench_key_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("key_generation");

    group.bench_function("monitor_key", |b| {
        b.iter(|| {
            let key = make_monitor_key(
                black_box("model_123"),
                black_box(&MonitorType::ModelOutput),
                black_box(10001),
            );
            black_box(key);
        });
    });

    group.bench_function("control_key", |b| {
        b.iter(|| {
            let key = make_control_key(black_box("cmd_123"));
            black_box(key);
        });
    });

    group.finish();
}

/// 数据序列化/反序列化性能测试
fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    let monitor_value = MonitorValue::new(123.45, "bench_source".to_string());
    let redis_str = monitor_value.to_redis();

    group.bench_function("monitor_value_to_redis", |b| {
        b.iter(|| {
            let str = black_box(&monitor_value).to_redis();
            black_box(str);
        });
    });

    group.bench_function("monitor_value_from_redis", |b| {
        b.iter(|| {
            let value = MonitorValue::from_redis(black_box(&redis_str));
            black_box(value);
        });
    });

    group.finish();
}

/// 并发写入性能测试
fn bench_concurrent_write(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let mut group = c.benchmark_group("concurrent_write");
    group.measurement_time(Duration::from_secs(10));

    for concurrency in [1, 5, 10, 20].iter() {
        group.bench_function(format!("{}_concurrent_models", concurrency), |b| {
            b.iter(|| {
                rt.block_on(async {
                    let mut tasks = Vec::new();

                    for i in 0..*concurrency {
                        let redis_url_clone = redis_url.clone();
                        let model_id = format!("concurrent_model_{}", i);

                        let task = tokio::spawn(async move {
                            let mut storage = ModelStorage::new(&redis_url_clone).await.unwrap();
                            let updates = create_monitor_updates(100, &model_id);
                            storage.set_monitor_values(&updates).await.unwrap();
                        });

                        tasks.push(task);
                    }

                    futures::future::join_all(tasks).await;
                });
            });
        });
    }

    group.finish();
}

/// comsrv点位读取性能测试
fn bench_comsrv_read(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    // 预先写入comsrv格式的测试数据
    rt.block_on(async {
        let client = redis::Client::open(&redis_url).unwrap();
        let mut conn = client.get_multiplexed_async_connection().await.unwrap();

        for i in 0..100 {
            let key = format!("1001:m:{}", 10000 + i);
            let value = format!(
                "{}:{}",
                100.0 + i as f64,
                chrono::Utc::now().timestamp_millis()
            );
            redis::cmd("SET")
                .arg(&key)
                .arg(&value)
                .query_async::<_, ()>(&mut conn)
                .await
                .unwrap();
        }
    });

    let mut group = c.benchmark_group("comsrv_read");

    for size in [10, 50, 100].iter() {
        group.bench_function(format!("read_{}_comsrv_points", size), |b| {
            b.iter_batched(
                || {
                    let storage = rt.block_on(ModelStorage::new(&redis_url)).unwrap();
                    let points: Vec<(u16, &str, u32)> =
                        (0..*size).map(|i| (1001, "m", 10000 + i as u32)).collect();
                    (storage, points)
                },
                |(mut storage, points)| {
                    rt.block_on(async {
                        let _values = storage
                            .read_comsrv_points(black_box(&points))
                            .await
                            .unwrap();
                    });
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_write,
    bench_batch_read,
    bench_key_generation,
    bench_serialization,
    bench_concurrent_write,
    bench_comsrv_read
);
criterion_main!(benches);
