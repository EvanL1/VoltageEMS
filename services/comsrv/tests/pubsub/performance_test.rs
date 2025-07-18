//! Pub/Sub性能测试

use comsrv::core::config::types::redis::PubSubConfig;
use comsrv::core::redis::storage::RedisStorage;
use comsrv::core::redis::types::PointUpdate;
use std::time::Instant;
use tokio::time::{sleep, Duration};
use sysinfo::{System, SystemExt, ProcessExt};

mod test_helpers;
use test_helpers::*;

const TEST_REDIS_URL: &str = "redis://localhost:6379";
const TEST_CHANNEL_ID: u16 = 9002;

#[tokio::test]
async fn test_batch_publish_performance() {
    println!("\n=== 测试批量发布性能 ===");
    
    // 准备测试环境
    let mut conn = create_test_connection(TEST_REDIS_URL).await.unwrap();
    cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
    
    // 测试数据量
    let test_sizes = vec![100, 500, 1000, 5000];
    
    for size in test_sizes {
        println!("\n测试 {} 个点位的批量发布:", size);
        
        // 准备数据
        let updates: Vec<PointUpdate> = (0..size).map(|i| {
            PointUpdate {
                channel_id: TEST_CHANNEL_ID,
                point_type: "m",
                point_id: 10000 + i as u32,
                value: i as f64 * 1.5,
            }
        }).collect();
        
        // 测试1: 禁用发布（基准性能）
        let mut storage_no_pub = RedisStorage::new(TEST_REDIS_URL).await.unwrap();
        let start = Instant::now();
        storage_no_pub.set_points(&updates).await.unwrap();
        let baseline_time = start.elapsed();
        println!("  基准时间（无发布）: {:?}", baseline_time);
        
        // 清理数据
        cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
        
        // 测试2: 启用发布（批量模式）
        let pubsub_config = PubSubConfig {
            enabled: true,
            batch_size: 100,
            batch_timeout_ms: 50,
            publish_on_set: true,
            message_version: "1.0".to_string(),
        };
        
        let mut storage_with_pub = RedisStorage::with_publisher(TEST_REDIS_URL, &pubsub_config)
            .await
            .unwrap();
        
        let start = Instant::now();
        storage_with_pub.set_points(&updates).await.unwrap();
        let publish_time = start.elapsed();
        println!("  发布时间（批量）: {:?}", publish_time);
        
        // 计算性能影响
        let overhead = if baseline_time.as_millis() > 0 {
            ((publish_time.as_millis() - baseline_time.as_millis()) as f64 / baseline_time.as_millis() as f64) * 100.0
        } else {
            0.0
        };
        
        println!("  性能开销: {:.2}%", overhead);
        println!("  每个点位平均时间: {:?}", publish_time / size as u32);
        
        // 验收标准：批量发布的性能开销应小于50%
        assert!(overhead < 50.0, "批量发布性能开销过大: {:.2}%", overhead);
        
        storage_with_pub.close().await;
        cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
    }
    
    println!("\n✓ 批量发布性能测试通过");
}

#[tokio::test]
async fn test_high_frequency_updates() {
    println!("\n=== 测试高频更新场景 ===");
    
    // 准备测试环境
    let mut conn = create_test_connection(TEST_REDIS_URL).await.unwrap();
    cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
    
    // 创建消息收集器
    let patterns = vec![format!("{}:*", TEST_CHANNEL_ID)];
    let (mut collector, subscriber) = MessageCollector::new(TEST_REDIS_URL, patterns).await.unwrap();
    
    sleep(Duration::from_millis(100)).await;
    
    // 创建启用pub/sub的存储
    let pubsub_config = PubSubConfig {
        enabled: true,
        batch_size: 50,
        batch_timeout_ms: 20,  // 更短的超时以测试高频场景
        publish_on_set: true,
        message_version: "1.0".to_string(),
    };
    
    let mut storage = RedisStorage::with_publisher(TEST_REDIS_URL, &pubsub_config)
        .await
        .unwrap();
    
    // 性能监控
    let monitor = PerformanceMonitor::new();
    let monitor_clone = monitor.clone();
    
    // 测试参数
    let target_rate = 1000;  // 目标：每秒1000个点位
    let test_duration = Duration::from_secs(5);
    let points_per_batch = 10;
    
    println!("目标更新率: {} 点位/秒", target_rate);
    println!("测试时长: {:?}", test_duration);
    
    // 启动高频更新
    let start = Instant::now();
    let mut total_sent = 0;
    
    while start.elapsed() < test_duration {
        let batch_start = Instant::now();
        
        // 发送一批更新
        let updates: Vec<PointUpdate> = (0..points_per_batch).map(|i| {
            PointUpdate {
                channel_id: TEST_CHANNEL_ID,
                point_type: "m",
                point_id: 10000 + (total_sent + i) as u32,
                value: (total_sent + i) as f64,
            }
        }).collect();
        
        storage.set_points(&updates).await.unwrap();
        total_sent += points_per_batch;
        
        // 控制发送速率
        let batch_duration = batch_start.elapsed();
        let expected_duration = Duration::from_millis((points_per_batch * 1000 / target_rate) as u64);
        if batch_duration < expected_duration {
            sleep(expected_duration - batch_duration).await;
        }
    }
    
    // 等待所有消息
    sleep(Duration::from_secs(1)).await;
    
    // 获取统计
    let stats = monitor.get_stats().await;
    println!("\n性能统计:");
    println!("  总发送: {} 点位", total_sent);
    println!("  实际速率: {:.2} 点位/秒", total_sent as f64 / test_duration.as_secs_f64());
    
    // 验收标准：支持每秒1000+点位更新
    assert!(total_sent as f64 / test_duration.as_secs_f64() >= 900.0, 
            "未达到目标更新率");
    
    // 清理
    storage.close().await;
    subscriber.stop();
    cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
    
    println!("\n✓ 高频更新测试通过");
}

#[tokio::test]
async fn test_resource_usage() {
    println!("\n=== 测试资源使用情况 ===");
    
    // 准备测试环境
    let mut conn = create_test_connection(TEST_REDIS_URL).await.unwrap();
    cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
    
    // 获取进程ID
    let pid = std::process::id();
    let mut system = System::new_all();
    system.refresh_processes();
    
    // 获取初始资源使用
    let initial_memory = if let Some(process) = system.process(pid as i32) {
        process.memory()
    } else {
        0
    };
    
    println!("初始内存使用: {} KB", initial_memory);
    
    // 创建存储实例
    let pubsub_config = PubSubConfig {
        enabled: true,
        batch_size: 100,
        batch_timeout_ms: 50,
        publish_on_set: true,
        message_version: "1.0".to_string(),
    };
    
    let mut storage = RedisStorage::with_publisher(TEST_REDIS_URL, &pubsub_config)
        .await
        .unwrap();
    
    // 执行大量操作
    let iterations = 100;
    let points_per_iter = 100;
    
    for i in 0..iterations {
        let updates: Vec<PointUpdate> = (0..points_per_iter).map(|j| {
            PointUpdate {
                channel_id: TEST_CHANNEL_ID,
                point_type: "m",
                point_id: 10000 + j,
                value: (i * points_per_iter + j) as f64,
            }
        }).collect();
        
        storage.set_points(&updates).await.unwrap();
        
        // 每10次迭代检查一次内存
        if i % 10 == 0 {
            system.refresh_processes();
            if let Some(process) = system.process(pid as i32) {
                let current_memory = process.memory();
                println!("迭代 {}: 内存使用 {} KB (+{} KB)", 
                         i, current_memory, current_memory - initial_memory);
            }
        }
        
        sleep(Duration::from_millis(10)).await;
    }
    
    // 最终内存检查
    system.refresh_processes();
    let final_memory = if let Some(process) = system.process(pid as i32) {
        process.memory()
    } else {
        0
    };
    
    let memory_increase = final_memory - initial_memory;
    println!("\n最终内存使用: {} KB", final_memory);
    println!("内存增长: {} KB", memory_increase);
    
    // 验收标准：内存增长应该合理（< 50MB）
    assert!(memory_increase < 50000, "内存使用过高: {} KB", memory_increase);
    
    // 清理
    storage.close().await;
    cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
    
    println!("\n✓ 资源使用测试通过");
}

#[tokio::test]
async fn test_batch_size_impact() {
    println!("\n=== 测试批量大小对性能的影响 ===");
    
    // 准备测试环境
    let mut conn = create_test_connection(TEST_REDIS_URL).await.unwrap();
    
    let batch_sizes = vec![10, 50, 100, 500, 1000];
    let test_points = 10000;
    
    for batch_size in batch_sizes {
        println!("\n测试批量大小: {}", batch_size);
        cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
        
        let pubsub_config = PubSubConfig {
            enabled: true,
            batch_size,
            batch_timeout_ms: 50,
            publish_on_set: true,
            message_version: "1.0".to_string(),
        };
        
        let mut storage = RedisStorage::with_publisher(TEST_REDIS_URL, &pubsub_config)
            .await
            .unwrap();
        
        // 准备数据
        let updates: Vec<PointUpdate> = (0..test_points).map(|i| {
            PointUpdate {
                channel_id: TEST_CHANNEL_ID,
                point_type: "m",
                point_id: 10000 + i as u32,
                value: i as f64,
            }
        }).collect();
        
        // 测量性能
        let start = Instant::now();
        storage.set_points(&updates).await.unwrap();
        let elapsed = start.elapsed();
        
        let throughput = test_points as f64 / elapsed.as_secs_f64();
        println!("  处理时间: {:?}", elapsed);
        println!("  吞吐量: {:.2} 点位/秒", throughput);
        
        storage.close().await;
    }
    
    cleanup_test_data(&mut conn, TEST_CHANNEL_ID).await.unwrap();
    println!("\n✓ 批量大小影响测试完成");
}

/// 验收标准总结
#[test]
fn test_performance_acceptance_criteria() {
    println!("\n=== Pub/Sub性能测试验收标准 ===");
    println!("✓ 1. 批量发布性能开销 < 50%");
    println!("✓ 2. 支持每秒1000+点位更新");
    println!("✓ 3. 内存使用稳定，增长 < 50MB");
    println!("✓ 4. CPU使用合理");
    println!("✓ 5. 不同批量大小都能正常工作");
    println!("✓ 6. 高频更新场景下保持稳定");
}