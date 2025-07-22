//! 端到端集成测试

use crate::batch_writer::{BatchWriteBuffer, BatchWriter, BatchWriterConfig};
use crate::config::Config;
use crate::error::Result;
use crate::message_processor::MessageProcessor;
use crate::pubsub::RedisSubscriber;
use crate::redis_subscriber::{EnhancedRedisSubscriber, SubscriberConfig, SubscriptionMessage};
use crate::storage::{DataPoint, DataValue, QueryOptions, Storage, StorageManager};
use crate::tests::mock_storage::create_memory_storage;
use crate::tests::test_utils::{
    create_test_config, create_test_data_point, create_test_point_data, generate_test_channel,
    wait_for_condition,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use crate::types::GenericPointData as PointData;

/// 集成测试用的批量写入器
struct IntegrationBatchWriter {
    storage: Arc<RwLock<Box<dyn Storage>>>,
    write_count: Arc<Mutex<usize>>,
}

impl IntegrationBatchWriter {
    fn new(storage: Arc<RwLock<Box<dyn Storage>>>) -> Self {
        Self {
            storage,
            write_count: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl BatchWriter for IntegrationBatchWriter {
    async fn write_batch(&mut self, points: &[DataPoint]) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.write_batch(points).await?;

        let mut count = self.write_count.lock().await;
        *count += points.len();

        Ok(())
    }

    fn name(&self) -> &str {
        "integration_writer"
    }
}

#[tokio::test]
async fn test_end_to_end_data_flow() {
    // 创建存储后端
    let mut mock_storage = create_memory_storage();
    mock_storage.connect().await.unwrap();

    let storage: Box<dyn Storage> = Box::new(mock_storage);
    let storage_arc = Arc::new(RwLock::new(storage));

    // 创建批量写入器
    let batch_writer = IntegrationBatchWriter::new(storage_arc.clone());
    let write_count = batch_writer.write_count.clone();

    let batch_config = BatchWriterConfig {
        max_batch_size: 10,
        flush_interval_secs: 1,
        enable_wal: false,
        ..Default::default()
    };

    let batch_buffer = Arc::new(BatchWriteBuffer::new(batch_writer, batch_config).unwrap());
    let batch_buffer_clone = batch_buffer.clone();

    // 启动批量写入器的刷新任务
    let flush_handle = batch_buffer_clone.start_flush_task();

    // 创建消息通道
    let (msg_sender, msg_receiver) = mpsc::unbounded_channel::<SubscriptionMessage>();

    // 创建消息处理器
    let processor_sender = msg_sender.clone();
    let processor_batch_buffer = batch_buffer.clone();
    let processor_task = tokio::spawn(async move {
        while let Some(msg) = msg_receiver.recv().await {
            // 将消息转换为 DataPoint
            if let Some(point_data) = msg.point_data {
                let data_point = DataPoint {
                    key: format!("{}:{}", msg.channel, point_data.point_id),
                    value: DataValue::Float(point_data.value),
                    timestamp: point_data.timestamp,
                    tags: msg.metadata.clone(),
                    metadata: Default::default(),
                };

                processor_batch_buffer.add(data_point).await.unwrap();
            }
        }
    });

    // 模拟发送数据
    for i in 0..50 {
        let channel = generate_test_channel(1001, "m", 10000 + i);
        let point_data = create_test_point_data(1001, 10000 + i, i as f64);

        let msg = SubscriptionMessage {
            id: format!("msg-{}", i),
            channel: channel.clone(),
            channel_info: crate::redis_subscriber::ChannelInfo::from_channel(&channel),
            timestamp: Utc::now(),
            point_data: Some(point_data),
            raw_data: None,
            metadata: Default::default(),
        };

        msg_sender.send(msg).unwrap();

        // 模拟真实的消息间隔
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // 等待数据被处理和写入
    let success = wait_for_condition(
        || async {
            let count = *write_count.lock().await;
            count >= 50
        },
        5000,
        100,
    )
    .await;

    assert!(success, "数据未在预期时间内完成写入");

    // 验证数据
    let storage = storage_arc.read().await;
    let query_result = storage
        .query(
            "1001:m:10025",
            QueryOptions {
                start_time: Utc::now() - Duration::hours(1),
                end_time: Utc::now(),
                limit: None,
                aggregate: None,
                group_by: None,
                fill: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(query_result.points.len(), 1);
    assert!(matches!(
        query_result.points[0].value,
        DataValue::Float(v) if v == 25.0
    ));

    // 清理
    batch_buffer.shutdown().await.unwrap();
    flush_handle.abort();
    drop(msg_sender);
    let _ = processor_task.await;
}

#[tokio::test]
async fn test_multi_channel_processing() {
    // 创建存储管理器
    let mut storage_manager = StorageManager::new();

    let mock_storage = create_memory_storage();
    storage_manager.add_backend("memory".to_string(), Box::new(mock_storage));
    storage_manager.set_default_backend("memory".to_string());
    storage_manager.connect_all().await.unwrap();

    let storage_manager_arc = Arc::new(RwLock::new(storage_manager));

    // 创建消息通道
    let (msg_sender, msg_receiver) = mpsc::unbounded_channel();

    // 创建消息处理器
    let mut processor = MessageProcessor::new(storage_manager_arc.clone(), msg_receiver);

    let processor_handle = tokio::spawn(async move {
        processor.start_processing().await.ok();
    });

    // 发送多个通道的数据
    let channels = vec![
        (1001, "m", vec![10001, 10002, 10003]),
        (1002, "s", vec![20001, 20002]),
        (1003, "c", vec![30001]),
    ];

    for (channel_id, msg_type, point_ids) in channels.clone() {
        for point_id in point_ids {
            let channel = generate_test_channel(channel_id, msg_type, point_id);
            let point_data = create_test_point_data(channel_id, point_id, point_id as f64);

            let msg = SubscriptionMessage {
                id: format!("msg-{}-{}", channel_id, point_id),
                channel: channel.clone(),
                channel_info: crate::redis_subscriber::ChannelInfo::from_channel(&channel),
                timestamp: Utc::now(),
                point_data: Some(point_data),
                raw_data: None,
                metadata: Default::default(),
            };

            msg_sender.send(msg).unwrap();
        }
    }

    // 等待处理完成
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 验证所有数据都被存储
    let storage_manager = storage_manager_arc.read().await;

    for (channel_id, msg_type, point_ids) in channels {
        for point_id in point_ids {
            let key = format!("{}:{}:{}", channel_id, msg_type, point_id);
            let result = storage_manager
                .query(
                    &key,
                    QueryOptions {
                        start_time: Utc::now() - Duration::hours(1),
                        end_time: Utc::now(),
                        limit: None,
                        aggregate: None,
                        group_by: None,
                        fill: None,
                    },
                )
                .await
                .unwrap();

            assert!(!result.is_empty(), "未找到键 {} 的数据", key);
        }
    }

    // 清理
    drop(msg_sender);
    processor_handle.abort();
}

#[tokio::test]
async fn test_error_recovery() {
    // 创建会失败的存储后端
    let failing_storage = crate::tests::mock_storage::create_failing_storage(5, 0);
    let storage: Box<dyn Storage> = Box::new(failing_storage);
    let storage_arc = Arc::new(RwLock::new(storage));

    // 创建带重试的批量写入器
    let batch_writer = IntegrationBatchWriter::new(storage_arc.clone());

    let batch_config = BatchWriterConfig {
        max_batch_size: 3,
        max_retries: 3,
        retry_delay_ms: 50,
        enable_wal: false,
        ..Default::default()
    };

    let batch_buffer = Arc::new(BatchWriteBuffer::new(batch_writer, batch_config).unwrap());

    // 添加数据
    for i in 0..10 {
        batch_buffer
            .add(create_test_data_point(&format!("metric_{}", i), i as f64))
            .await
            .unwrap();
    }

    // 手动刷新以确保所有数据都被处理
    batch_buffer.flush().await.ok(); // 第一批可能失败

    // 等待重试
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 再次刷新
    batch_buffer.flush().await.ok();

    let stats = batch_buffer.get_stats().await;
    println!("写入统计: {:?}", stats);

    // 验证部分数据成功写入（在失败次数用尽后）
    assert!(stats.total_points_written > 0);
    assert!(stats.total_points_failed > 0);
}

#[tokio::test]
async fn test_performance_under_load() {
    // 创建高性能存储后端
    let mut storage = create_memory_storage();
    storage.connect().await.unwrap();

    let storage: Box<dyn Storage> = Box::new(storage);
    let storage_arc = Arc::new(RwLock::new(storage));

    // 创建批量写入器
    let batch_writer = IntegrationBatchWriter::new(storage_arc.clone());
    let write_count = batch_writer.write_count.clone();

    let batch_config = BatchWriterConfig {
        max_batch_size: 1000,
        flush_interval_secs: 1,
        enable_wal: false,
        ..Default::default()
    };

    let batch_buffer = Arc::new(BatchWriteBuffer::new(batch_writer, batch_config).unwrap());
    let batch_buffer_clone = batch_buffer.clone();

    // 启动刷新任务
    let flush_handle = batch_buffer_clone.start_flush_task();

    let start = tokio::time::Instant::now();

    // 并发发送大量数据
    let mut handles = vec![];
    for thread_id in 0..10 {
        let batch_buffer = batch_buffer.clone();
        let handle = tokio::spawn(async move {
            for i in 0..1000 {
                let point = create_test_data_point(
                    &format!("metric_{}_{}", thread_id, i),
                    (thread_id * 1000 + i) as f64,
                );
                batch_buffer.add(point).await.unwrap();

                // 模拟真实负载
                if i % 100 == 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.await.unwrap();
    }

    // 等待所有数据写入
    batch_buffer.flush().await.unwrap();

    let elapsed = start.elapsed();
    let total_written = *write_count.lock().await;

    println!("性能测试结果:");
    println!("  总数据点: {}", total_written);
    println!("  总耗时: {:?}", elapsed);
    println!(
        "  吞吐量: {:.2} 点/秒",
        total_written as f64 / elapsed.as_secs_f64()
    );

    let stats = batch_buffer.get_stats().await;
    println!("  平均批量大小: {:.2}", stats.average_batch_size);
    println!("  写入延迟: {:.2} ms", stats.write_latency_ms);

    // 验证所有数据都被写入
    assert_eq!(total_written, 10000);
    assert!(elapsed.as_secs() < 10); // 应该在10秒内完成

    // 清理
    batch_buffer.shutdown().await.unwrap();
    flush_handle.abort();
}

#[tokio::test]
async fn test_graceful_shutdown() {
    let mut storage = create_memory_storage();
    storage.connect().await.unwrap();

    let storage: Box<dyn Storage> = Box::new(storage);
    let storage_arc = Arc::new(RwLock::new(storage));

    let batch_writer = IntegrationBatchWriter::new(storage_arc.clone());
    let write_count = batch_writer.write_count.clone();

    let batch_config = BatchWriterConfig {
        max_batch_size: 100,
        flush_interval_secs: 60, // 长间隔，确保不会自动刷新
        enable_wal: false,
        ..Default::default()
    };

    let batch_buffer = Arc::new(BatchWriteBuffer::new(batch_writer, batch_config).unwrap());

    // 添加一些数据但不触发自动刷新
    for i in 0..50 {
        batch_buffer
            .add(create_test_data_point(&format!("metric_{}", i), i as f64))
            .await
            .unwrap();
    }

    // 验证数据还在缓冲区中
    assert_eq!(batch_buffer.buffer_size().await, 50);
    assert_eq!(*write_count.lock().await, 0);

    // 优雅关闭
    batch_buffer.shutdown().await.unwrap();

    // 验证关闭时数据被刷新
    assert_eq!(*write_count.lock().await, 50);
}
