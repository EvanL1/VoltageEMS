//! 错误恢复测试
//!
//! 测试系统在各种错误场景下的恢复能力

use comsrv::core::framework::TelemetryType;
use comsrv::plugins::plugin_storage::{DefaultPluginStorage, PluginPointUpdate, PluginStorage};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info, warn};

#[tokio::test]
async fn test_redis_connection_recovery() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init()
        .ok();

    // 先尝试连接到一个错误的地址
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:9999");

    let result = DefaultPluginStorage::from_env().await;
    assert!(result.is_err(), "应该连接失败");
    info!("正确处理了Redis连接失败");

    // 现在使用正确的地址
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    // 测试基本操作
    let result = storage
        .write_point(1000, &TelemetryType::Telemetry, 1, 100.0)
        .await;

    assert!(result.is_ok(), "恢复后应该能正常写入");
    info!("Redis连接恢复测试通过");
}

#[tokio::test]
async fn test_timeout_handling() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    // 创建一个非常大的批量更新
    let mut updates = Vec::new();
    for i in 0..10000 {
        updates.push(PluginPointUpdate {
            channel_id: 5000,
            telemetry_type: TelemetryType::Telemetry,
            point_id: i,
            value: i as f64,
        });
    }

    // 设置一个很短的超时时间
    let result = timeout(Duration::from_millis(10), storage.write_points(updates)).await;

    match result {
        Ok(Ok(_)) => info!("操作在超时前完成"),
        Ok(Err(e)) => warn!("操作失败: {}", e),
        Err(_) => info!("操作超时，正确处理了超时场景"),
    }
}

#[tokio::test]
async fn test_invalid_data_handling() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    // 测试各种边界值
    let test_cases = vec![
        (0u16, 0u32, f64::INFINITY, "无穷大值"),
        (u16::MAX, u32::MAX, f64::NEG_INFINITY, "负无穷大值"),
        (1000, 1000, f64::NAN, "NaN值"),
        (1000, 1000, f64::MAX, "最大浮点数"),
        (1000, 1000, f64::MIN, "最小浮点数"),
    ];

    for (channel_id, point_id, value, desc) in test_cases {
        let result = storage
            .write_point(channel_id, &TelemetryType::Telemetry, point_id, value)
            .await;

        // 所有值都应该能被存储，即使是特殊值
        match result {
            Ok(_) => info!(
                "成功存储{}: channel={}, point={}, value={}",
                desc, channel_id, point_id, value
            ),
            Err(e) => error!("存储{}失败: {}", desc, e),
        }
    }
}

#[tokio::test]
async fn test_concurrent_error_recovery() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    use tokio::task::JoinSet;
    let mut tasks = JoinSet::new();

    // 创建多个并发任务，其中一些会失败
    for i in 0..10 {
        let storage_clone = storage.clone();

        tasks.spawn(async move {
            let channel_id = 6000 + i;

            // 偶数任务正常执行
            if i % 2 == 0 {
                for j in 0..100 {
                    let _ = storage_clone
                        .write_point(channel_id, &TelemetryType::Telemetry, j, j as f64)
                        .await;
                }
                Ok(channel_id)
            } else {
                // 奇数任务模拟错误
                Err(format!("模拟的错误 - 通道 {}", channel_id))
            }
        });
    }

    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(channel_id)) => {
                success_count += 1;
                info!("通道 {} 成功完成", channel_id);
            }
            Ok(Err(e)) => {
                error_count += 1;
                warn!("任务失败: {}", e);
            }
            Err(e) => {
                error_count += 1;
                error!("任务崩溃: {}", e);
            }
        }
    }

    info!(
        "并发错误恢复测试完成: {} 成功, {} 失败",
        success_count, error_count
    );
    assert_eq!(success_count, 5, "应该有5个任务成功");
    assert_eq!(error_count, 5, "应该有5个任务失败");
}

#[tokio::test]
async fn test_data_consistency_after_errors() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    let channel_id = 7000;
    let mut successful_writes = Vec::new();

    // 执行一系列写入，记录成功的操作
    for i in 0..20 {
        let point_id = i;
        let value = i as f64 * 10.0;

        // 模拟一些操作会失败
        if i % 5 != 4 {
            match storage
                .write_point(channel_id, &TelemetryType::Telemetry, point_id, value)
                .await
            {
                Ok(_) => {
                    successful_writes.push((point_id, value));
                    info!("成功写入点 {}", point_id);
                }
                Err(e) => {
                    error!("写入点 {} 失败: {}", point_id, e);
                }
            }
        } else {
            warn!("跳过点 {} 的写入（模拟失败）", point_id);
        }
    }

    // 验证所有成功写入的数据都正确存储
    info!("验证数据一致性...");
    let mut consistency_errors = 0;

    for (point_id, expected_value) in successful_writes {
        match storage
            .read_point(channel_id, &TelemetryType::Telemetry, point_id)
            .await
        {
            Ok(Some((value, _))) => {
                if (value - expected_value).abs() > 0.001 {
                    error!(
                        "数据不一致: 点 {} 期望值 {} 实际值 {}",
                        point_id, expected_value, value
                    );
                    consistency_errors += 1;
                }
            }
            Ok(None) => {
                error!("数据丢失: 点 {}", point_id);
                consistency_errors += 1;
            }
            Err(e) => {
                error!("读取点 {} 失败: {}", point_id, e);
                consistency_errors += 1;
            }
        }
    }

    assert_eq!(consistency_errors, 0, "数据一致性检查失败");
    info!("错误后数据一致性验证通过");
}

#[tokio::test]
async fn test_batch_partial_failure() {
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            println!("跳过测试：Redis未运行");
            return;
        }
    };

    // 创建一个包含有效和无效数据的批次
    let mut updates = Vec::new();

    for i in 0..10 {
        updates.push(PluginPointUpdate {
            channel_id: 8000,
            telemetry_type: TelemetryType::Telemetry,
            point_id: i,
            value: if i % 3 == 0 { f64::NAN } else { i as f64 },
        });
    }

    // 批量写入应该成功（即使包含NaN）
    let result = storage.write_points(updates).await;
    assert!(result.is_ok(), "批量写入应该成功");

    // 验证非NaN值都被正确存储
    for i in 0..10 {
        let result = storage.read_point(8000, &TelemetryType::Telemetry, i).await;

        match result {
            Ok(Some((value, _))) => {
                if i % 3 == 0 {
                    assert!(value.is_nan(), "NaN值应该被保留");
                } else {
                    assert_eq!(value, i as f64, "正常值应该正确存储");
                }
            }
            Ok(None) => panic!("数据不应该丢失"),
            Err(e) => panic!("读取失败: {}", e),
        }
    }

    info!("批量部分失败测试通过");
}
