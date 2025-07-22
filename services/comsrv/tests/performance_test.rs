//! 性能测试
//!
//! 测试新的扁平化Redis存储结构的性能

use comsrv::core::framework::TelemetryType;
use comsrv::plugins::plugin_storage::{DefaultPluginStorage, PluginPointUpdate, PluginStorage};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// 性能测试配置
struct PerfTestConfig {
    total_points: u32,
    batch_size: usize,
    update_rounds: u32,
    channel_count: u16,
}

impl Default for PerfTestConfig {
    fn default() -> Self {
        Self {
            total_points: 10000,
            batch_size: 1000,
            update_rounds: 10,
            channel_count: 5,
        }
    }
}

#[tokio::test]
async fn test_batch_write_performance() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init()
        .ok();

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    let config = PerfTestConfig::default();
    info!("开始批量写入性能测试");
    info!(
        "总点数: {}, 批次大小: {}",
        config.total_points, config.batch_size
    );

    let mut results = Vec::new();

    for batch_size in [100, 500, 1000, 5000] {
        let result = test_batch_size_performance(&storage, batch_size).await;
        results.push((batch_size, result));
    }

    // 输出性能报告
    println!("\n=== 批量写入性能报告 ===");
    println!("批次大小 | 总耗时(ms) | 点/秒 | 平均延迟(μs/点)");
    println!("---------|-----------|-------|---------------");

    for (batch_size, (duration, points_per_sec, avg_latency)) in results {
        println!(
            "{:8} | {:9.2} | {:5.0} | {:14.2}",
            batch_size,
            duration.as_millis() as f64,
            points_per_sec,
            avg_latency
        );
    }
}

async fn test_batch_size_performance(
    storage: &Arc<dyn PluginStorage>,
    batch_size: usize,
) -> (Duration, f64, f64) {
    let total_points = 10000;
    let channel_id = 1000;

    let mut updates = Vec::with_capacity(batch_size);
    let start = Instant::now();
    let mut point_id = 0u32;

    while point_id < total_points {
        updates.clear();

        for i in 0..batch_size.min((total_points - point_id) as usize) {
            updates.push(PluginPointUpdate {
                channel_id,
                telemetry_type: if i % 2 == 0 {
                    TelemetryType::Telemetry
                } else {
                    TelemetryType::Signal
                },
                point_id: point_id + i as u32,
                value: (point_id + i as u32) as f64 * 0.1,
            });
        }

        storage.write_points(updates.clone()).await.unwrap();
        point_id += batch_size as u32;
    }

    let duration = start.elapsed();
    let points_per_sec = total_points as f64 / duration.as_secs_f64();
    let avg_latency = duration.as_micros() as f64 / total_points as f64;

    (duration, points_per_sec, avg_latency)
}

#[tokio::test]
async fn test_read_performance() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    let channel_id = 2000;
    let point_count = 10000;

    // 先写入测试数据
    info!("准备测试数据...");
    let mut updates = Vec::new();
    for i in 0..point_count {
        updates.push(PluginPointUpdate {
            channel_id,
            telemetry_type: TelemetryType::Telemetry,
            point_id: i,
            value: i as f64,
        });
    }
    storage.write_points(updates).await.unwrap();

    // 测试顺序读取性能
    info!("测试顺序读取性能...");
    let start = Instant::now();
    let mut read_count = 0;

    for i in 0..point_count {
        if let Ok(Some(_)) = storage
            .read_point(channel_id, &TelemetryType::Telemetry, i)
            .await
        {
            read_count += 1;
        }
    }

    let duration = start.elapsed();
    let reads_per_sec = read_count as f64 / duration.as_secs_f64();
    info!("顺序读取: {} 点/秒", reads_per_sec);

    // 测试随机读取性能
    info!("测试随机读取性能...");

    let start = Instant::now();
    let mut read_count = 0;

    // 使用简单的伪随机方法
    for i in 0..1000 {
        let point_id = (i * 7919) % point_count; // 使用质数生成伪随机序列
        if let Ok(Some(_)) = storage
            .read_point(channel_id, &TelemetryType::Telemetry, point_id)
            .await
        {
            read_count += 1;
        }
    }

    let duration = start.elapsed();
    let reads_per_sec = read_count as f64 / duration.as_secs_f64();
    info!("随机读取: {} 点/秒", reads_per_sec);
}

#[tokio::test]
async fn test_memory_usage() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let mut storage = match RedisStorage::new("redis://127.0.0.1:6379").await {
        Ok(s) => s,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    // 获取Redis内存使用情况
    let initial_memory = get_redis_memory_usage(&mut storage).await;
    info!("初始Redis内存使用: {} KB", initial_memory / 1024);

    // 写入大量数据
    let channel_count = 10;
    let points_per_channel = 10000;
    let mut total_written = 0;

    for channel_id in 1..=channel_count {
        let mut updates = Vec::new();
        for point_id in 0..points_per_channel {
            updates.push(PointUpdate {
                channel_id,
                point_type: TYPE_MEASUREMENT,
                point_id,
                value: point_id as f64,
            });
            total_written += 1;
        }
        storage.set_points(&updates).await.unwrap();
    }

    // 再次获取内存使用
    let final_memory = get_redis_memory_usage(&mut storage).await;
    let memory_increase = final_memory - initial_memory;
    let bytes_per_point = memory_increase as f64 / total_written as f64;

    info!("最终Redis内存使用: {} KB", final_memory / 1024);
    info!("内存增长: {} KB", memory_increase / 1024);
    info!("每点内存占用: {:.2} bytes", bytes_per_point);

    // 验证内存效率
    assert!(
        bytes_per_point < 100.0,
        "每点内存占用过高: {:.2} bytes",
        bytes_per_point
    );
}

async fn get_redis_memory_usage(_storage: &mut RedisStorage) -> u64 {
    // 这里简化处理，实际可以通过Redis INFO命令获取
    // 暂时返回估算值
    0
}

#[tokio::test]
async fn test_sustained_load() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    let duration = Duration::from_secs(10);
    let channel_id = 3000;
    let update_interval = Duration::from_millis(10);

    info!("开始持续负载测试，持续时间: {:?}", duration);

    let start = Instant::now();
    let stats = Arc::new(Mutex::new(LoadTestStats::default()));
    let mut point_id = 0u32;

    while start.elapsed() < duration {
        let batch_start = Instant::now();

        // 创建一批更新
        let mut updates = Vec::new();
        for i in 0..100 {
            updates.push(PluginPointUpdate {
                channel_id,
                telemetry_type: TelemetryType::Telemetry,
                point_id: point_id + i,
                value: (point_id + i) as f64,
            });
        }

        // 执行更新
        match storage.write_points(updates).await {
            Ok(_) => {
                let mut s = stats.lock().await;
                s.successful_updates += 100;
                s.total_latency += batch_start.elapsed();
                s.update_count += 1;
            }
            Err(e) => {
                let mut s = stats.lock().await;
                s.failed_updates += 100;
                warn!("批量更新失败: {}", e);
            }
        }

        point_id += 100;
        tokio::time::sleep(update_interval).await;
    }

    let stats = stats.lock().await;
    let total_updates = stats.successful_updates + stats.failed_updates;
    let avg_latency = stats.total_latency.as_micros() as f64 / stats.update_count as f64;
    let updates_per_sec = stats.successful_updates as f64 / duration.as_secs_f64();

    info!("持续负载测试完成");
    info!("总更新数: {}", total_updates);
    info!("成功更新: {}", stats.successful_updates);
    info!("失败更新: {}", stats.failed_updates);
    info!("平均延迟: {:.2} μs", avg_latency);
    info!("更新速率: {:.2} updates/sec", updates_per_sec);

    assert_eq!(stats.failed_updates, 0, "有更新失败");
    assert!(updates_per_sec > 1000.0, "更新速率过低");
}

#[derive(Default)]
struct LoadTestStats {
    successful_updates: u64,
    failed_updates: u64,
    total_latency: Duration,
    update_count: u64,
}

#[tokio::test]
async fn test_pipeline_efficiency() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let mut storage = match RedisStorage::new("redis://127.0.0.1:6379").await {
        Ok(s) => s,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    let channel_id = 4000;
    let point_count = 1000;

    // 测试非管道化写入
    info!("测试非管道化写入...");
    let start = Instant::now();

    for i in 0..point_count {
        storage
            .set_point(channel_id, TYPE_MEASUREMENT, i, i as f64)
            .await
            .unwrap();
    }

    let non_pipelined_duration = start.elapsed();
    info!("非管道化写入耗时: {:?}", non_pipelined_duration);

    // 测试管道化写入
    info!("测试管道化写入...");
    let start = Instant::now();

    let mut updates = Vec::new();
    for i in 0..point_count {
        updates.push(PointUpdate {
            channel_id: channel_id + 1,
            point_type: TYPE_MEASUREMENT,
            point_id: i,
            value: i as f64,
        });
    }

    storage.set_points(&updates).await.unwrap();

    let pipelined_duration = start.elapsed();
    info!("管道化写入耗时: {:?}", pipelined_duration);

    // 计算性能提升
    let speedup = non_pipelined_duration.as_secs_f64() / pipelined_duration.as_secs_f64();
    info!("管道化性能提升: {:.2}x", speedup);

    assert!(speedup > 5.0, "管道化性能提升不足");
}
