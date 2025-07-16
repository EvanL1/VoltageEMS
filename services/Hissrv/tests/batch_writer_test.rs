use hissrv::batch_writer::{
    BatchWriteBuffer, BatchWriteStats, BatchWriter, BatchWriterConfig, Result,
};
use hissrv::storage::{DataPoint, DataValue};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

/// 模拟的批量写入器，用于测试
struct MockBatchWriter {
    write_count: Arc<AtomicUsize>,
    batch_sizes: Arc<Mutex<Vec<usize>>>,
    should_fail: Arc<Mutex<bool>>,
    fail_count: Arc<AtomicUsize>,
}

impl MockBatchWriter {
    fn new() -> Self {
        Self {
            write_count: Arc::new(AtomicUsize::new(0)),
            batch_sizes: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
            fail_count: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().await = should_fail;
    }
    
    fn get_write_count(&self) -> usize {
        self.write_count.load(Ordering::SeqCst)
    }
    
    async fn get_batch_sizes(&self) -> Vec<usize> {
        self.batch_sizes.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl BatchWriter<DataPoint> for MockBatchWriter {
    async fn write_batch(&self, items: &[DataPoint]) -> Result<()> {
        if *self.should_fail.lock().await {
            self.fail_count.fetch_add(1, Ordering::SeqCst);
            return Err("Mock write error".into());
        }
        
        self.write_count.fetch_add(1, Ordering::SeqCst);
        self.batch_sizes.lock().await.push(items.len());
        
        // 模拟写入延迟
        sleep(Duration::from_millis(10)).await;
        Ok(())
    }
}

/// 创建测试数据点
fn create_test_data_point(id: u32) -> DataPoint {
    DataPoint {
        key: format!("test:m:{}", id),
        timestamp: chrono::Utc::now(),
        value: DataValue::Float(100.0 + id as f64),
        tags: HashMap::from([
            ("test".to_string(), "true".to_string()),
            ("id".to_string(), id.to_string()),
        ]),
        metadata: HashMap::new(),
    }
}

#[tokio::test]
async fn test_basic_batch_writing() {
    let config = BatchWriterConfig {
        max_batch_size: 5,
        flush_interval_secs: 1,
        max_retries: 3,
        retry_delay_ms: 100,
        enable_wal: false,
        wal_path: String::new(),
    };
    
    let mock_writer = Arc::new(MockBatchWriter::new());
    let buffer = Arc::new(BatchWriteBuffer::new(mock_writer.clone(), config).unwrap());
    
    // 启动刷新任务
    let buffer_clone = buffer.clone();
    let flush_handle = tokio::spawn(async move {
        buffer_clone.start_flush_task().await;
    });
    
    // 添加10个数据点
    for i in 1..=10 {
        buffer.add(create_test_data_point(i)).await.unwrap();
    }
    
    // 等待刷新完成
    sleep(Duration::from_secs(2)).await;
    
    // 验证结果
    assert_eq!(mock_writer.get_write_count(), 2); // 应该有2批（每批5个）
    let batch_sizes = mock_writer.get_batch_sizes().await;
    assert_eq!(batch_sizes, vec![5, 5]);
    
    // 验证统计信息
    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_items_processed, 10);
    assert_eq!(stats.total_batches_written, 2);
    assert_eq!(stats.failed_writes, 0);
    
    // 清理
    flush_handle.abort();
    buffer.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_flush_on_timeout() {
    let config = BatchWriterConfig {
        max_batch_size: 10,
        flush_interval_secs: 1,
        max_retries: 3,
        retry_delay_ms: 100,
        enable_wal: false,
        wal_path: String::new(),
    };
    
    let mock_writer = Arc::new(MockBatchWriter::new());
    let buffer = Arc::new(BatchWriteBuffer::new(mock_writer.clone(), config).unwrap());
    
    // 启动刷新任务
    let buffer_clone = buffer.clone();
    let flush_handle = tokio::spawn(async move {
        buffer_clone.start_flush_task().await;
    });
    
    // 只添加3个数据点（小于批量大小）
    for i in 1..=3 {
        buffer.add(create_test_data_point(i)).await.unwrap();
    }
    
    // 等待超时刷新
    sleep(Duration::from_secs(2)).await;
    
    // 验证：即使没有达到批量大小，也应该因为超时而刷新
    assert_eq!(mock_writer.get_write_count(), 1);
    let batch_sizes = mock_writer.get_batch_sizes().await;
    assert_eq!(batch_sizes[0], 3);
    
    // 清理
    flush_handle.abort();
    buffer.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_retry_mechanism() {
    let config = BatchWriterConfig {
        max_batch_size: 5,
        flush_interval_secs: 1,
        max_retries: 3,
        retry_delay_ms: 50,
        enable_wal: false,
        wal_path: String::new(),
    };
    
    let mock_writer = Arc::new(MockBatchWriter::new());
    let buffer = Arc::new(BatchWriteBuffer::new(mock_writer.clone(), config).unwrap());
    
    // 设置写入失败
    mock_writer.set_should_fail(true).await;
    
    // 启动刷新任务
    let buffer_clone = buffer.clone();
    let flush_handle = tokio::spawn(async move {
        buffer_clone.start_flush_task().await;
    });
    
    // 添加数据
    for i in 1..=5 {
        buffer.add(create_test_data_point(i)).await.unwrap();
    }
    
    // 等待重试
    sleep(Duration::from_millis(200)).await;
    
    // 验证重试次数
    let fail_count = mock_writer.fail_count.load(Ordering::SeqCst);
    assert!(fail_count >= 2, "应该至少重试2次，实际：{}", fail_count);
    
    // 恢复正常
    mock_writer.set_should_fail(false).await;
    
    // 等待成功写入
    sleep(Duration::from_secs(1)).await;
    
    // 验证最终写入成功
    assert_eq!(mock_writer.get_write_count(), 1);
    
    // 清理
    flush_handle.abort();
    buffer.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_immediate_flush() {
    let config = BatchWriterConfig {
        max_batch_size: 10,
        flush_interval_secs: 10, // 设置较长的时间间隔
        max_retries: 3,
        retry_delay_ms: 100,
        enable_wal: false,
        wal_path: String::new(),
    };
    
    let mock_writer = Arc::new(MockBatchWriter::new());
    let buffer = Arc::new(BatchWriteBuffer::new(mock_writer.clone(), config).unwrap());
    
    // 启动刷新任务
    let buffer_clone = buffer.clone();
    let flush_handle = tokio::spawn(async move {
        buffer_clone.start_flush_task().await;
    });
    
    // 添加数据
    for i in 1..=3 {
        buffer.add(create_test_data_point(i)).await.unwrap();
    }
    
    // 立即刷新
    buffer.flush().await.unwrap();
    
    // 验证立即写入
    assert_eq!(mock_writer.get_write_count(), 1);
    let batch_sizes = mock_writer.get_batch_sizes().await;
    assert_eq!(batch_sizes[0], 3);
    
    // 清理
    flush_handle.abort();
    buffer.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_adds() {
    let config = BatchWriterConfig {
        max_batch_size: 100,
        flush_interval_secs: 2,
        max_retries: 3,
        retry_delay_ms: 100,
        enable_wal: false,
        wal_path: String::new(),
    };
    
    let mock_writer = Arc::new(MockBatchWriter::new());
    let buffer = Arc::new(BatchWriteBuffer::new(mock_writer.clone(), config).unwrap());
    
    // 启动刷新任务
    let buffer_clone = buffer.clone();
    let flush_handle = tokio::spawn(async move {
        buffer_clone.start_flush_task().await;
    });
    
    // 并发添加数据
    let mut handles = vec![];
    for thread_id in 0..10 {
        let buffer_clone = buffer.clone();
        let handle = tokio::spawn(async move {
            for i in 0..10 {
                let id = thread_id * 100 + i;
                buffer_clone.add(create_test_data_point(id)).await.unwrap();
            }
        });
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }
    
    // 等待刷新
    sleep(Duration::from_secs(3)).await;
    
    // 验证所有数据都被处理
    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_items_processed, 100);
    
    // 清理
    flush_handle.abort();
    buffer.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_shutdown_flushes_remaining() {
    let config = BatchWriterConfig {
        max_batch_size: 10,
        flush_interval_secs: 60, // 设置很长的间隔
        max_retries: 3,
        retry_delay_ms: 100,
        enable_wal: false,
        wal_path: String::new(),
    };
    
    let mock_writer = Arc::new(MockBatchWriter::new());
    let buffer = Arc::new(BatchWriteBuffer::new(mock_writer.clone(), config).unwrap());
    
    // 启动刷新任务
    let buffer_clone = buffer.clone();
    let flush_handle = tokio::spawn(async move {
        buffer_clone.start_flush_task().await;
    });
    
    // 添加数据（少于批量大小）
    for i in 1..=7 {
        buffer.add(create_test_data_point(i)).await.unwrap();
    }
    
    // 立即关闭
    flush_handle.abort();
    buffer.shutdown().await.unwrap();
    
    // 验证关闭时刷新了剩余数据
    assert_eq!(mock_writer.get_write_count(), 1);
    let batch_sizes = mock_writer.get_batch_sizes().await;
    assert_eq!(batch_sizes[0], 7);
}

#[tokio::test]
async fn test_stats_accuracy() {
    let config = BatchWriterConfig {
        max_batch_size: 5,
        flush_interval_secs: 1,
        max_retries: 1,
        retry_delay_ms: 50,
        enable_wal: false,
        wal_path: String::new(),
    };
    
    let mock_writer = Arc::new(MockBatchWriter::new());
    let buffer = Arc::new(BatchWriteBuffer::new(mock_writer.clone(), config).unwrap());
    
    // 启动刷新任务
    let buffer_clone = buffer.clone();
    let flush_handle = tokio::spawn(async move {
        buffer_clone.start_flush_task().await;
    });
    
    // 添加15个成功的数据点
    for i in 1..=15 {
        buffer.add(create_test_data_point(i)).await.unwrap();
    }
    
    // 等待处理
    sleep(Duration::from_secs(2)).await;
    
    // 设置失败并添加更多数据
    mock_writer.set_should_fail(true).await;
    for i in 16..=20 {
        buffer.add(create_test_data_point(i)).await.unwrap();
    }
    
    // 等待失败尝试
    sleep(Duration::from_secs(2)).await;
    
    // 获取统计信息
    let stats = buffer.get_stats().await;
    
    // 验证统计信息
    assert_eq!(stats.total_items_processed, 15); // 只有前15个成功
    assert_eq!(stats.total_batches_written, 3); // 3批成功（5+5+5）
    assert!(stats.failed_writes > 0); // 应该有失败的写入
    assert_eq!(stats.items_in_buffer, 5); // 最后5个还在缓冲区
    assert!(stats.total_write_time_ms > 0);
    assert!(stats.avg_batch_size > 0.0);
    
    // 清理
    flush_handle.abort();
    buffer.shutdown().await.unwrap();
}