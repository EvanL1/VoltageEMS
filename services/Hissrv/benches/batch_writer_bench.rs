//! 批量写入器性能基准测试

use async_trait::async_trait;
use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use hissrv_rust::batch_writer::{BatchWriteBuffer, BatchWriter, BatchWriterConfig};
use hissrv_rust::error::Result;
use hissrv_rust::storage::{DataPoint, DataValue};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

/// 基准测试用的批量写入器
struct BenchmarkWriter {
    write_delay_ns: Option<u64>,
}

#[async_trait]
impl BatchWriter for BenchmarkWriter {
    async fn write_batch(&mut self, points: &[DataPoint]) -> Result<()> {
        if let Some(delay) = self.write_delay_ns {
            // 模拟写入延迟
            std::thread::sleep(std::time::Duration::from_nanos(delay * points.len() as u64));
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "benchmark_writer"
    }
}

fn create_test_points(count: usize) -> Vec<DataPoint> {
    (0..count)
        .map(|i| DataPoint {
            key: format!("metric_{}", i % 100),
            value: DataValue::Float(i as f64),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        })
        .collect()
}

fn benchmark_batch_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("batch_sizes");

    for size in [10, 100, 500, 1000, 5000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    let writer = BenchmarkWriter {
                        write_delay_ns: Some(100),
                    };
                    let config = BatchWriterConfig {
                        max_batch_size: size,
                        flush_interval_secs: 60,
                        enable_wal: false,
                        ..Default::default()
                    };

                    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
                    let points = create_test_points(size * 2);

                    for point in points {
                        buffer.add(black_box(point)).await.unwrap();
                    }

                    buffer.flush().await.unwrap();
                });
            });
        });
    }

    group.finish();
}

fn benchmark_concurrent_writes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_writes");

    for num_threads in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    rt.block_on(async {
                        let writer = BenchmarkWriter {
                            write_delay_ns: None,
                        };
                        let config = BatchWriterConfig {
                            max_batch_size: 1000,
                            flush_interval_secs: 60,
                            enable_wal: false,
                            ..Default::default()
                        };

                        let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
                        let points_per_thread = 1000;

                        let mut handles = vec![];
                        for thread_id in 0..num_threads {
                            let buffer_clone = buffer.clone();
                            let handle = tokio::spawn(async move {
                                let points = create_test_points(points_per_thread);
                                for point in points {
                                    buffer_clone.add(point).await.unwrap();
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }

                        buffer.flush().await.unwrap();
                    });
                });
            },
        );
    }

    group.finish();
}

fn benchmark_flush_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("flush_performance");

    group.bench_function("flush_1k_points", |b| {
        b.iter(|| {
            rt.block_on(async {
                let writer = BenchmarkWriter {
                    write_delay_ns: None,
                };
                let config = BatchWriterConfig {
                    max_batch_size: 10000,
                    flush_interval_secs: 60,
                    enable_wal: false,
                    ..Default::default()
                };

                let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

                // 预先填充缓冲区
                let points = create_test_points(1000);
                buffer.add_batch(points).await.unwrap();

                // 基准测试刷新操作
                buffer.flush().await.unwrap();
            });
        });
    });

    group.bench_function("flush_10k_points", |b| {
        b.iter(|| {
            rt.block_on(async {
                let writer = BenchmarkWriter {
                    write_delay_ns: None,
                };
                let config = BatchWriterConfig {
                    max_batch_size: 20000,
                    flush_interval_secs: 60,
                    enable_wal: false,
                    ..Default::default()
                };

                let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

                let points = create_test_points(10000);
                buffer.add_batch(points).await.unwrap();

                buffer.flush().await.unwrap();
            });
        });
    });

    group.finish();
}

fn benchmark_wal_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("wal_overhead");

    group.bench_function("without_wal", |b| {
        b.iter(|| {
            rt.block_on(async {
                let writer = BenchmarkWriter {
                    write_delay_ns: None,
                };
                let config = BatchWriterConfig {
                    max_batch_size: 100,
                    enable_wal: false,
                    ..Default::default()
                };

                let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
                let points = create_test_points(100);

                for point in points {
                    buffer.add(point).await.unwrap();
                }
            });
        });
    });

    group.bench_function("with_wal", |b| {
        b.iter(|| {
            rt.block_on(async {
                let writer = BenchmarkWriter {
                    write_delay_ns: None,
                };
                let config = BatchWriterConfig {
                    max_batch_size: 100,
                    enable_wal: true,
                    wal_path: "/tmp/hissrv_bench_wal".to_string(),
                    ..Default::default()
                };

                let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
                let points = create_test_points(100);

                for point in points {
                    buffer.add(point).await.unwrap();
                }
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_batch_sizes,
    benchmark_concurrent_writes,
    benchmark_flush_performance,
    benchmark_wal_overhead
);
criterion_main!(benches);
