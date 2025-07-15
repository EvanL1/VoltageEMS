//! 批量写入器测试

use crate::batch_writer::{BatchWriteBuffer, BatchWriter, BatchWriterConfig};
use crate::error::Result;
use crate::storage::{DataPoint, DataValue};
use crate::tests::test_utils::{create_test_data_point, create_test_batch};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

/// 测试用的批量写入器
struct TestBatchWriter {
    write_count: Arc<Mutex<usize>>,
    points_written: Arc<Mutex<Vec<DataPoint>>>,
    fail_count: usize,
    delay_ms: Option<u64>,
}

impl TestBatchWriter {
    fn new() -> Self {
        Self {
            write_count: Arc::new(Mutex::new(0)),
            points_written: Arc::new(Mutex::new(Vec::new())),
            fail_count: 0,
            delay_ms: None,
        }
    }
    
    fn with_failures(fail_count: usize) -> Self {
        Self {
            write_count: Arc::new(Mutex::new(0)),
            points_written: Arc::new(Mutex::new(Vec::new())),
            fail_count,
            delay_ms: None,
        }
    }
    
    fn with_delay(delay_ms: u64) -> Self {
        Self {
            write_count: Arc::new(Mutex::new(0)),
            points_written: Arc::new(Mutex::new(Vec::new())),
            fail_count: 0,
            delay_ms: Some(delay_ms),
        }
    }
    
    async fn get_write_count(&self) -> usize {
        *self.write_count.lock().await
    }
    
    async fn get_points_written(&self) -> Vec<DataPoint> {
        self.points_written.lock().await.clone()
    }
}

#[async_trait]
impl BatchWriter for TestBatchWriter {
    async fn write_batch(&mut self, points: &[DataPoint]) -> Result<()> {
        if let Some(delay) = self.delay_ms {
            sleep(Duration::from_millis(delay)).await;
        }
        
        let mut count = self.write_count.lock().await;
        if *count < self.fail_count {
            *count += 1;
            return Err(crate::error::HisSrvError::WriteError(
                format!("Test failure {}/{}", *count, self.fail_count)
            ));
        }
        
        *count += 1;
        let mut written = self.points_written.lock().await;
        written.extend_from_slice(points);
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "test_writer"
    }
}

#[tokio::test]
async fn test_batch_writer_basic_functionality() {
    let writer = TestBatchWriter::new();
    let writer_clone = writer.write_count.clone();
    let points_clone = writer.points_written.clone();
    
    let config = BatchWriterConfig {
        max_batch_size: 10,
        flush_interval_secs: 60,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    // 添加数据点
    for i in 0..5 {
        buffer.add(create_test_data_point(&format!("metric_{}", i), i as f64)).await.unwrap();
    }
    
    // 检查缓冲区大小
    assert_eq!(buffer.buffer_size().await, 5);
    
    // 手动刷新
    buffer.flush().await.unwrap();
    
    // 验证写入
    assert_eq!(*writer_clone.lock().await, 1);
    assert_eq!(points_clone.lock().await.len(), 5);
    assert_eq!(buffer.buffer_size().await, 0);
}

#[tokio::test]
async fn test_batch_writer_auto_flush() {
    let writer = TestBatchWriter::new();
    let points_clone = writer.points_written.clone();
    
    let config = BatchWriterConfig {
        max_batch_size: 3,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    // 添加3个点，应该触发自动刷新
    for i in 0..3 {
        buffer.add(create_test_data_point(&format!("metric_{}", i), i as f64)).await.unwrap();
    }
    
    // 等待一小段时间确保刷新完成
    sleep(Duration::from_millis(100)).await;
    
    // 验证自动刷新
    assert_eq!(points_clone.lock().await.len(), 3);
    assert_eq!(buffer.buffer_size().await, 0);
}

#[tokio::test]
async fn test_batch_writer_retry_mechanism() {
    let writer = TestBatchWriter::with_failures(2); // 前两次失败
    let write_count_clone = writer.write_count.clone();
    let points_clone = writer.points_written.clone();
    
    let config = BatchWriterConfig {
        max_batch_size: 2,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    // 添加数据触发刷新
    buffer.add(create_test_data_point("metric_1", 1.0)).await.unwrap();
    buffer.add(create_test_data_point("metric_2", 2.0)).await.unwrap();
    
    // 等待重试完成
    sleep(Duration::from_millis(100)).await;
    
    // 验证重试成功
    assert_eq!(*write_count_clone.lock().await, 3); // 2次失败 + 1次成功
    assert_eq!(points_clone.lock().await.len(), 2);
    
    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_written, 2);
    assert_eq!(stats.total_batches_written, 1);
}

#[tokio::test]
async fn test_batch_writer_retry_failure() {
    let writer = TestBatchWriter::with_failures(5); // 总是失败
    
    let config = BatchWriterConfig {
        max_batch_size: 1,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    // 添加数据
    buffer.add(create_test_data_point("metric_1", 1.0)).await.unwrap();
    
    // 等待重试完成
    sleep(Duration::from_millis(200)).await;
    
    // 验证失败统计
    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_failed, 1);
    assert_eq!(stats.total_batches_failed, 1);
    assert_eq!(buffer.buffer_size().await, 1); // 数据应该回到缓冲区
}

#[tokio::test]
async fn test_batch_writer_flush_task() {
    let writer = TestBatchWriter::new();
    let points_clone = writer.points_written.clone();
    
    let config = BatchWriterConfig {
        max_batch_size: 100,
        flush_interval_secs: 1,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    let buffer_clone = Arc::clone(&buffer);
    
    // 启动刷新任务
    let flush_handle = buffer_clone.start_flush_task();
    
    // 添加数据
    buffer.add(create_test_data_point("metric_1", 1.0)).await.unwrap();
    
    // 等待自动刷新
    sleep(Duration::from_secs(2)).await;
    
    // 验证定期刷新
    assert_eq!(points_clone.lock().await.len(), 1);
    assert_eq!(buffer.buffer_size().await, 0);
    
    // 关闭
    buffer.shutdown().await.unwrap();
    flush_handle.abort();
}

#[tokio::test]
async fn test_batch_writer_statistics() {
    let writer = TestBatchWriter::new();
    
    let config = BatchWriterConfig {
        max_batch_size: 5,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    // 添加多批数据
    for batch in 0..3 {
        for i in 0..5 {
            buffer.add(create_test_data_point(&format!("metric_{}_{}", batch, i), i as f64))
                .await
                .unwrap();
        }
        sleep(Duration::from_millis(50)).await;
    }
    
    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_received, 15);
    assert_eq!(stats.total_points_written, 15);
    assert_eq!(stats.total_batches_written, 3);
    assert!(stats.average_batch_size > 4.0 && stats.average_batch_size <= 5.0);
    assert_eq!(stats.success_rate(), 100.0);
}

#[tokio::test]
async fn test_batch_writer_add_batch() {
    let writer = TestBatchWriter::new();
    let points_clone = writer.points_written.clone();
    
    let config = BatchWriterConfig {
        max_batch_size: 10,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    // 批量添加
    let batch = create_test_batch(8, 100.0);
    buffer.add_batch(batch).await.unwrap();
    
    assert_eq!(buffer.buffer_size().await, 8);
    
    // 添加更多触发刷新
    let batch2 = create_test_batch(3, 200.0);
    buffer.add_batch(batch2).await.unwrap();
    
    sleep(Duration::from_millis(100)).await;
    
    // 验证
    assert_eq!(points_clone.lock().await.len(), 10);
    assert_eq!(buffer.buffer_size().await, 1);
}

#[tokio::test]
async fn test_batch_writer_concurrent_adds() {
    let writer = TestBatchWriter::new();
    let points_clone = writer.points_written.clone();
    
    let config = BatchWriterConfig {
        max_batch_size: 50,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    // 并发添加数据
    let mut handles = vec![];
    for i in 0..10 {
        let buffer_clone = Arc::clone(&buffer);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                buffer_clone
                    .add(create_test_data_point(&format!("metric_{}_{}", i, j), (i * 10 + j) as f64))
                    .await
                    .unwrap();
            }
        });
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }
    
    // 刷新剩余数据
    buffer.flush().await.unwrap();
    
    // 验证所有数据都被写入
    assert_eq!(points_clone.lock().await.len(), 100);
}

#[tokio::test]
async fn test_batch_writer_performance() {
    let writer = TestBatchWriter::with_delay(10); // 模拟10ms写入延迟
    
    let config = BatchWriterConfig {
        max_batch_size: 100,
        flush_interval_secs: 5,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    let start = tokio::time::Instant::now();
    
    // 添加1000个数据点
    for i in 0..1000 {
        buffer.add(create_test_data_point(&format!("metric_{}", i), i as f64))
            .await
            .unwrap();
    }
    
    // 最终刷新
    buffer.flush().await.unwrap();
    
    let elapsed = start.elapsed();
    
    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_written, 1000);
    assert_eq!(stats.total_batches_written, 10); // 1000 / 100
    
    // 批量写入应该比单独写入快得多
    // 10批 * 10ms = 100ms，而不是 1000 * 10ms = 10000ms
    assert!(elapsed.as_millis() < 500);
    
    println!("批量写入1000个点耗时: {:?}", elapsed);
    println!("平均批量大小: {}", stats.average_batch_size);
    println!("写入延迟: {}ms", stats.write_latency_ms);
}

#[tokio::test]
async fn test_batch_writer_shutdown() {
    let writer = TestBatchWriter::new();
    let points_clone = writer.points_written.clone();
    
    let config = BatchWriterConfig {
        max_batch_size: 100,
        ..Default::default()
    };
    
    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    
    // 添加一些数据但不触发自动刷新
    for i in 0..5 {
        buffer.add(create_test_data_point(&format!("metric_{}", i), i as f64))
            .await
            .unwrap();
    }
    
    assert_eq!(buffer.buffer_size().await, 5);
    
    // 关闭时应该刷新剩余数据
    buffer.shutdown().await.unwrap();
    
    assert_eq!(points_clone.lock().await.len(), 5);
}