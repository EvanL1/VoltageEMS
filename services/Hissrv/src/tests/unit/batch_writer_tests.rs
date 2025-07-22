use crate::batch_writer::{BatchWriteBuffer, BatchWriteStats, BatchWriter, BatchWriterConfig};
use crate::error::{HisSrvError, Result};
use crate::storage::{DataPoint, DataValue};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Mock BatchWriter 用于测试
struct MockBatchWriter {
    write_count: Arc<Mutex<usize>>,
    fail_count: usize,
    points_written: Arc<Mutex<Vec<DataPoint>>>,
    write_delay_ms: u64,
    should_fail: bool,
}

impl MockBatchWriter {
    fn new() -> Self {
        Self {
            write_count: Arc::new(Mutex::new(0)),
            fail_count: 0,
            points_written: Arc::new(Mutex::new(Vec::new())),
            write_delay_ms: 0,
            should_fail: false,
        }
    }

    fn with_fail_count(mut self, count: usize) -> Self {
        self.fail_count = count;
        self
    }

    fn with_delay(mut self, delay_ms: u64) -> Self {
        self.write_delay_ms = delay_ms;
        self
    }

    fn with_permanent_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    async fn get_written_points(&self) -> Vec<DataPoint> {
        self.points_written.lock().await.clone()
    }
}

#[async_trait]
impl BatchWriter for MockBatchWriter {
    async fn write_batch(&mut self, points: &[DataPoint]) -> Result<()> {
        if self.write_delay_ms > 0 {
            sleep(Duration::from_millis(self.write_delay_ms)).await;
        }

        let mut count = self.write_count.lock().await;

        if self.should_fail {
            return Err(HisSrvError::WriteError("Permanent failure".to_string()));
        }

        if *count < self.fail_count {
            *count += 1;
            Err(HisSrvError::WriteError("Mock transient error".to_string()))
        } else {
            self.points_written.lock().await.extend_from_slice(points);
            Ok(())
        }
    }

    fn name(&self) -> &str {
        "mock_writer"
    }
}

#[tokio::test]
async fn test_batch_writer_basic_functionality() {
    let writer = MockBatchWriter::new();
    let config = BatchWriterConfig {
        max_batch_size: 3,
        flush_interval_secs: 10,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        wal_path: "./test_wal".to_string(),
    };

    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

    // 创建测试数据点
    let points = vec![
        DataPoint {
            key: "test.metric1".to_string(),
            value: DataValue::Float(1.0),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        },
        DataPoint {
            key: "test.metric2".to_string(),
            value: DataValue::Float(2.0),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        },
    ];

    // 添加数据点（未达到批量大小，不会触发写入）
    for point in &points {
        buffer.add(point.clone()).await.unwrap();
    }
    assert_eq!(buffer.buffer_size().await, 2);

    // 添加第三个点，触发批量写入
    let point3 = DataPoint {
        key: "test.metric3".to_string(),
        value: DataValue::Float(3.0),
        timestamp: Utc::now(),
        tags: Default::default(),
        metadata: Default::default(),
    };
    buffer.add(point3).await.unwrap();

    // 缓冲区应该已清空
    assert_eq!(buffer.buffer_size().await, 0);

    // 验证统计信息
    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_received, 3);
    assert_eq!(stats.total_points_written, 3);
    assert_eq!(stats.total_batches_written, 1);
}

#[tokio::test]
async fn test_batch_writer_retry_mechanism() {
    let writer = MockBatchWriter::new().with_fail_count(2);
    let config = BatchWriterConfig {
        max_batch_size: 1,
        flush_interval_secs: 10,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        wal_path: "./test_wal".to_string(),
    };

    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

    let point = DataPoint {
        key: "test.retry".to_string(),
        value: DataValue::Float(1.0),
        timestamp: Utc::now(),
        tags: Default::default(),
        metadata: Default::default(),
    };

    // 添加点将触发写入，应该在第3次尝试时成功
    buffer.add(point).await.unwrap();

    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_written, 1);
    assert_eq!(stats.total_batches_written, 1);
}

#[tokio::test]
async fn test_batch_writer_max_retries_exceeded() {
    let writer = MockBatchWriter::new().with_permanent_failure();
    let config = BatchWriterConfig {
        max_batch_size: 1,
        flush_interval_secs: 10,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        wal_path: "./test_wal".to_string(),
    };

    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

    let point = DataPoint {
        key: "test.fail".to_string(),
        value: DataValue::Float(1.0),
        timestamp: Utc::now(),
        tags: Default::default(),
        metadata: Default::default(),
    };

    // 应该失败并返回错误
    let result = buffer.add(point).await;
    assert!(result.is_err());

    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_failed, 1);
    assert_eq!(stats.total_batches_failed, 1);
}

#[tokio::test]
async fn test_batch_writer_flush_on_timer() {
    let writer = MockBatchWriter::new();
    let config = BatchWriterConfig {
        max_batch_size: 100, // 大批量，不会触发基于大小的刷新
        flush_interval_secs: 1,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        wal_path: "./test_wal".to_string(),
    };

    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
    let buffer_clone = Arc::clone(&buffer);

    // 启动刷新任务
    let flush_handle = buffer_clone.start_flush_task();

    let point = DataPoint {
        key: "test.timer".to_string(),
        value: DataValue::Float(1.0),
        timestamp: Utc::now(),
        tags: Default::default(),
        metadata: Default::default(),
    };

    buffer.add(point).await.unwrap();
    assert!(buffer.buffer_size().await > 0);

    // 等待定时刷新
    sleep(Duration::from_secs(2)).await;
    assert_eq!(buffer.buffer_size().await, 0);

    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_written, 1);

    // 清理
    buffer.shutdown().await.unwrap();
    flush_handle.abort();
}

#[tokio::test]
async fn test_batch_writer_batch_addition() {
    let writer = MockBatchWriter::new();
    let config = BatchWriterConfig {
        max_batch_size: 5,
        flush_interval_secs: 10,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        wal_path: "./test_wal".to_string(),
    };

    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

    let mut points = Vec::new();
    for i in 0..4 {
        points.push(DataPoint {
            key: format!("test.batch{}", i),
            value: DataValue::Float(i as f64),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        });
    }

    // 批量添加4个点
    buffer.add_batch(points).await.unwrap();
    assert_eq!(buffer.buffer_size().await, 4);

    // 再添加2个点触发刷新
    let extra_points = vec![
        DataPoint {
            key: "test.batch4".to_string(),
            value: DataValue::Float(4.0),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        },
        DataPoint {
            key: "test.batch5".to_string(),
            value: DataValue::Float(5.0),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        },
    ];

    buffer.add_batch(extra_points).await.unwrap();
    assert_eq!(buffer.buffer_size().await, 0);

    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_received, 6);
    assert_eq!(stats.total_points_written, 6);
}

#[tokio::test]
async fn test_batch_writer_statistics() {
    let writer = MockBatchWriter::new().with_delay(50);
    let config = BatchWriterConfig {
        max_batch_size: 2,
        flush_interval_secs: 10,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        wal_path: "./test_wal".to_string(),
    };

    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

    // 写入多批数据
    for batch in 0..3 {
        for i in 0..2 {
            let point = DataPoint {
                key: format!("test.stats.batch{}.point{}", batch, i),
                value: DataValue::Float((batch * 2 + i) as f64),
                timestamp: Utc::now(),
                tags: Default::default(),
                metadata: Default::default(),
            };
            buffer.add(point).await.unwrap();
        }
    }

    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_received, 6);
    assert_eq!(stats.total_points_written, 6);
    assert_eq!(stats.total_batches_written, 3);
    assert_eq!(stats.average_batch_size, 2.0);
    assert!(stats.write_latency_ms >= 50.0);
    assert_eq!(stats.success_rate(), 100.0);
    assert_eq!(stats.batch_success_rate(), 100.0);
}

#[tokio::test]
async fn test_batch_writer_concurrent_adds() {
    let writer = MockBatchWriter::new();
    let config = BatchWriterConfig {
        max_batch_size: 10,
        flush_interval_secs: 10,
        max_retries: 3,
        retry_delay_ms: 10,
        enable_wal: false,
        wal_path: "./test_wal".to_string(),
    };

    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

    // 并发添加数据
    let mut handles = vec![];
    for i in 0..5 {
        let buffer_clone = Arc::clone(&buffer);
        let handle = tokio::spawn(async move {
            for j in 0..2 {
                let point = DataPoint {
                    key: format!("test.concurrent.{}.{}", i, j),
                    value: DataValue::Float((i * 2 + j) as f64),
                    timestamp: Utc::now(),
                    tags: Default::default(),
                    metadata: Default::default(),
                };
                buffer_clone.add(point).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }

    // 手动刷新确保所有数据都写入
    buffer.flush().await.unwrap();

    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_received, 10);
    assert_eq!(stats.total_points_written, 10);
}

#[tokio::test]
async fn test_batch_writer_empty_flush() {
    let writer = MockBatchWriter::new();
    let config = BatchWriterConfig::default();

    let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());

    // 空缓冲区刷新应该成功
    buffer.flush().await.unwrap();

    let stats = buffer.get_stats().await;
    assert_eq!(stats.total_points_written, 0);
    assert_eq!(stats.total_batches_written, 0);
}
