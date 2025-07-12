//! 并发存储测试
//!
//! 测试多通道并发写入Redis的性能和数据一致性

use comsrv::core::framework::TelemetryType;
use comsrv::core::redis::storage::RedisStorage;
use comsrv::core::redis::types::{PointUpdate, TYPE_MEASUREMENT, TYPE_SIGNAL};
use comsrv::plugins::plugin_storage::{DefaultPluginStorage, PluginPointUpdate, PluginStorage};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info};

/// 配置常量
const CONCURRENT_CHANNELS: u16 = 10; // 并发通道数
const POINTS_PER_CHANNEL: u32 = 1000; // 每个通道的点数
const UPDATES_PER_POINT: u32 = 10; // 每个点的更新次数
const MAX_CONCURRENT_TASKS: usize = 50; // 最大并发任务数

#[tokio::test]
async fn test_concurrent_channel_writes() {
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

    info!(
        "开始并发写入测试：{}个通道，每个通道{}个点",
        CONCURRENT_CHANNELS, POINTS_PER_CHANNEL
    );

    let start = Instant::now();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TASKS));
    let mut tasks = JoinSet::new();

    // 为每个通道创建写入任务
    for channel_id in 1..=CONCURRENT_CHANNELS {
        let storage_clone = storage.clone();
        let sem_clone = semaphore.clone();

        tasks.spawn(async move { write_channel_data(channel_id, storage_clone, sem_clone).await });
    }

    // 等待所有任务完成
    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(e)) => {
                error!("通道写入失败: {}", e);
                error_count += 1;
            }
            Err(e) => {
                error!("任务执行失败: {}", e);
                error_count += 1;
            }
        }
    }

    let elapsed = start.elapsed();
    let total_points = CONCURRENT_CHANNELS as u64 * POINTS_PER_CHANNEL as u64;
    let total_updates = total_points * UPDATES_PER_POINT as u64;
    let updates_per_sec = total_updates as f64 / elapsed.as_secs_f64();

    info!("并发写入测试完成");
    info!("耗时: {:?}", elapsed);
    info!("成功通道数: {}", success_count);
    info!("失败通道数: {}", error_count);
    info!("总点数: {}", total_points);
    info!("总更新次数: {}", total_updates);
    info!("更新速率: {:.2} updates/sec", updates_per_sec);

    assert_eq!(success_count, CONCURRENT_CHANNELS as usize);
    assert_eq!(error_count, 0);

    // 验证数据一致性
    verify_data_consistency(&storage).await;
}

/// 写入单个通道的数据
async fn write_channel_data(
    channel_id: u16,
    storage: Arc<dyn PluginStorage>,
    semaphore: Arc<Semaphore>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let base_id = channel_id as u32 * 10000;

    // 为每个点进行多次更新
    for update_round in 0..UPDATES_PER_POINT {
        let mut updates = Vec::new();

        // 批量准备更新数据
        for i in 0..POINTS_PER_CHANNEL {
            let point_id = base_id + i;
            let value = (update_round as f64 + 1.0) * 100.0 + i as f64;

            updates.push(PluginPointUpdate {
                channel_id,
                telemetry_type: if i % 2 == 0 {
                    TelemetryType::Telemetry
                } else {
                    TelemetryType::Signal
                },
                point_id,
                value,
            });
        }

        // 获取许可并执行批量写入
        let _permit = semaphore.acquire().await?;
        storage.write_points(updates).await?;
    }

    info!("通道 {} 完成所有更新", channel_id);
    Ok(())
}

/// 验证数据一致性
async fn verify_data_consistency(storage: &Arc<dyn PluginStorage>) {
    info!("开始验证数据一致性...");

    let mut verification_errors = 0;
    let sample_points = 10; // 每个通道抽样检查的点数

    for channel_id in 1..=CONCURRENT_CHANNELS {
        let base_id = channel_id as u32 * 10000;

        for i in 0..sample_points {
            let point_id = base_id + i;
            let expected_value = (UPDATES_PER_POINT as f64) * 100.0 + i as f64;

            let telemetry_type = if i % 2 == 0 {
                TelemetryType::Telemetry
            } else {
                TelemetryType::Signal
            };

            match storage
                .read_point(channel_id, &telemetry_type, point_id)
                .await
            {
                Ok(Some((value, _))) => {
                    if (value - expected_value).abs() > 0.001 {
                        error!(
                            "数据不一致: 通道{} 点{} 期望值{} 实际值{}",
                            channel_id, point_id, expected_value, value
                        );
                        verification_errors += 1;
                    }
                }
                Ok(None) => {
                    error!("数据丢失: 通道{} 点{}", channel_id, point_id);
                    verification_errors += 1;
                }
                Err(e) => {
                    error!("读取失败: 通道{} 点{} 错误{}", channel_id, point_id, e);
                    verification_errors += 1;
                }
            }
        }
    }

    info!("数据一致性验证完成，错误数: {}", verification_errors);
    assert_eq!(verification_errors, 0, "数据一致性验证失败");
}

#[tokio::test]
async fn test_concurrent_read_write() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    let channel_id = 9999;
    let point_id = 99999;
    let iterations = 1000;

    let start = Instant::now();
    let mut tasks = JoinSet::new();

    // 创建写入任务
    let storage_write = storage.clone();
    tasks.spawn(async move {
        for i in 0..iterations {
            let value = i as f64;
            storage_write
                .write_point(channel_id, &TelemetryType::Telemetry, point_id, value)
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
    });

    // 创建多个读取任务
    for reader_id in 0..5 {
        let storage_read = storage.clone();
        tasks.spawn(async move {
            let mut last_value = -1.0;
            let mut read_count = 0;

            for _ in 0..iterations {
                if let Ok(Some((value, _))) = storage_read
                    .read_point(channel_id, &TelemetryType::Telemetry, point_id)
                    .await
                {
                    // 确保读取的值是递增的（或相等）
                    assert!(
                        value >= last_value,
                        "Reader {} 检测到值回退: {} -> {}",
                        reader_id,
                        last_value,
                        value
                    );
                    last_value = value;
                    read_count += 1;
                }
                tokio::time::sleep(Duration::from_micros(50)).await;
            }

            info!("Reader {} 读取了 {} 次", reader_id, read_count);
        });
    }

    // 等待所有任务完成
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }

    let elapsed = start.elapsed();
    info!("并发读写测试完成，耗时: {:?}", elapsed);
}

#[tokio::test]
async fn test_channel_isolation() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    let channels = vec![100, 200, 300, 400, 500];
    let point_id = 12345;

    // 并发写入不同通道的相同点ID
    let mut tasks = JoinSet::new();

    for &channel_id in &channels {
        let storage_clone = storage.clone();
        let value = channel_id as f64;

        tasks.spawn(async move {
            storage_clone
                .write_point(channel_id, &TelemetryType::Telemetry, point_id, value)
                .await
                .unwrap();
        });
    }

    // 等待所有写入完成
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }

    // 验证每个通道的数据独立性
    for &channel_id in &channels {
        let result = storage
            .read_point(channel_id, &TelemetryType::Telemetry, point_id)
            .await
            .unwrap();

        assert!(result.is_some());
        let (value, _) = result.unwrap();
        assert_eq!(
            value, channel_id as f64,
            "通道 {} 的数据隔离失败",
            channel_id
        );
    }

    info!("通道隔离测试通过");
}
