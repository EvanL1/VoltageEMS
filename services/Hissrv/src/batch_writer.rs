use crate::error::{HisSrvError, Result};
use crate::storage::DataPoint;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Batch writer configuration
#[derive(Clone, Debug)]
pub struct BatchWriterConfig {
    /// Maximum number of points to batch before writing
    pub max_batch_size: usize,
    /// Maximum time to wait before flushing the batch (in seconds)
    pub flush_interval_secs: u64,
    /// Maximum retry attempts for failed writes
    pub max_retries: u32,
    /// Retry delay base (exponential backoff)
    pub retry_delay_ms: u64,
    /// Enable WAL (Write-Ahead Log)
    pub enable_wal: bool,
    /// WAL directory path
    pub wal_path: String,
}

impl Default for BatchWriterConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            flush_interval_secs: 10,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_wal: true,
            wal_path: "./data/wal".to_string(),
        }
    }
}

/// Trait for batch write operations
#[async_trait]
pub trait BatchWriter: Send + Sync {
    /// Write a batch of data points
    async fn write_batch(&mut self, points: &[DataPoint]) -> Result<()>;
    
    /// Get the name of the writer
    fn name(&self) -> &str;
}

/// Batch write buffer with automatic flushing
pub struct BatchWriteBuffer<W: BatchWriter> {
    writer: Arc<Mutex<W>>,
    buffer: Arc<RwLock<VecDeque<DataPoint>>>,
    config: BatchWriterConfig,
    stats: Arc<RwLock<BatchWriteStats>>,
    shutdown: Arc<RwLock<bool>>,
    wal: Option<Arc<Mutex<WriteAheadLog>>>,
}

/// Statistics for batch write operations
#[derive(Debug, Default)]
pub struct BatchWriteStats {
    pub total_points_received: u64,
    pub total_points_written: u64,
    pub total_points_failed: u64,
    pub total_batches_written: u64,
    pub total_batches_failed: u64,
    pub last_write_time: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub average_batch_size: f64,
    pub write_latency_ms: f64,
}

/// Write-Ahead Log for durability
struct WriteAheadLog {
    path: String,
    current_file: Option<std::fs::File>,
    sequence: u64,
}

impl WriteAheadLog {
    fn new(path: &str) -> Result<Self> {
        // Create WAL directory if it doesn't exist
        std::fs::create_dir_all(path)?;
        
        Ok(Self {
            path: path.to_string(),
            current_file: None,
            sequence: 0,
        })
    }
    
    async fn append(&mut self, points: &[DataPoint]) -> Result<()> {
        // TODO: Implement WAL append logic
        // For now, just return Ok
        Ok(())
    }
    
    async fn recover(&mut self) -> Result<Vec<DataPoint>> {
        // TODO: Implement WAL recovery logic
        // For now, return empty vector
        Ok(Vec::new())
    }
    
    async fn checkpoint(&mut self) -> Result<()> {
        // TODO: Implement WAL checkpoint logic
        Ok(())
    }
}

impl<W: BatchWriter + 'static> BatchWriteBuffer<W> {
    pub fn new(writer: W, config: BatchWriterConfig) -> Result<Self> {
        let wal = if config.enable_wal {
            Some(Arc::new(Mutex::new(WriteAheadLog::new(&config.wal_path)?)))
        } else {
            None
        };
        
        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            buffer: Arc::new(RwLock::new(VecDeque::new())),
            config,
            stats: Arc::new(RwLock::new(BatchWriteStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
            wal,
        })
    }
    
    /// Add a data point to the buffer
    pub async fn add(&self, point: DataPoint) -> Result<()> {
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_points_received += 1;
        }
        
        // Add to WAL if enabled
        if let Some(wal) = &self.wal {
            let mut wal_guard = wal.lock().await;
            wal_guard.append(&[point.clone()]).await?;
        }
        
        // Add to buffer
        let buffer_size = {
            let mut buffer = self.buffer.write().await;
            buffer.push_back(point);
            buffer.len()
        };
        
        // Check if we should flush
        if buffer_size >= self.config.max_batch_size {
            self.flush().await?;
        }
        
        Ok(())
    }
    
    /// Add multiple data points to the buffer
    pub async fn add_batch(&self, points: Vec<DataPoint>) -> Result<()> {
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_points_received += points.len() as u64;
        }
        
        // Add to WAL if enabled
        if let Some(wal) = &self.wal {
            let mut wal_guard = wal.lock().await;
            wal_guard.append(&points).await?;
        }
        
        // Add to buffer
        let buffer_size = {
            let mut buffer = self.buffer.write().await;
            buffer.extend(points);
            buffer.len()
        };
        
        // Check if we should flush
        if buffer_size >= self.config.max_batch_size {
            self.flush().await?;
        }
        
        Ok(())
    }
    
    /// Flush the buffer to the writer
    pub async fn flush(&self) -> Result<()> {
        let points: Vec<DataPoint> = {
            let mut buffer = self.buffer.write().await;
            buffer.drain(..).collect()
        };
        
        if points.is_empty() {
            return Ok(());
        }
        
        let batch_size = points.len();
        let start_time = std::time::Instant::now();
        
        // Attempt to write with retries
        let mut attempt = 0;
        let mut last_error = None;
        
        while attempt < self.config.max_retries {
            match self.write_with_retry(&points).await {
                Ok(_) => {
                    let duration = start_time.elapsed();
                    
                    // Update stats on success
                    let mut stats = self.stats.write().await;
                    stats.total_points_written += batch_size as u64;
                    stats.total_batches_written += 1;
                    stats.last_write_time = Some(Utc::now());
                    stats.write_latency_ms = duration.as_millis() as f64;
                    
                    // Update average batch size
                    let total_batches = stats.total_batches_written as f64;
                    stats.average_batch_size = 
                        (stats.average_batch_size * (total_batches - 1.0) + batch_size as f64) / total_batches;
                    
                    // Clear WAL on successful write
                    if let Some(wal) = &self.wal {
                        let mut wal_guard = wal.lock().await;
                        wal_guard.checkpoint().await?;
                    }
                    
                    info!(
                        "Successfully wrote batch of {} points in {:?}",
                        batch_size, duration
                    );
                    
                    return Ok(());
                }
                Err(e) => {
                    attempt += 1;
                    last_error = Some(e.to_string());
                    
                    if attempt < self.config.max_retries {
                        let delay = Duration::from_millis(
                            self.config.retry_delay_ms * (2_u64.pow(attempt - 1))
                        );
                        warn!(
                            "Failed to write batch (attempt {}/{}): {}. Retrying in {:?}",
                            attempt, self.config.max_retries, e, delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        // Update stats on failure
        let mut stats = self.stats.write().await;
        stats.total_points_failed += batch_size as u64;
        stats.total_batches_failed += 1;
        stats.last_error = last_error.clone();
        
        error!(
            "Failed to write batch of {} points after {} attempts: {:?}",
            batch_size, self.config.max_retries, last_error
        );
        
        // Put points back in buffer for retry later
        let mut buffer = self.buffer.write().await;
        for point in points.into_iter().rev() {
            buffer.push_front(point);
        }
        
        Err(HisSrvError::WriteError(format!(
            "Failed to write batch after {} retries: {:?}",
            self.config.max_retries, last_error
        )))
    }
    
    /// Write with a single retry attempt
    async fn write_with_retry(&self, points: &[DataPoint]) -> Result<()> {
        let mut writer = self.writer.lock().await;
        writer.write_batch(points).await
    }
    
    /// Start the automatic flush task
    pub fn start_flush_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut flush_interval = interval(Duration::from_secs(self.config.flush_interval_secs));
            
            loop {
                flush_interval.tick().await;
                
                // Check if we should shutdown
                if *self.shutdown.read().await {
                    break;
                }
                
                // Flush any pending data
                if let Err(e) = self.flush().await {
                    error!("Error during periodic flush: {}", e);
                }
            }
            
            // Final flush before shutdown
            if let Err(e) = self.flush().await {
                error!("Error during final flush: {}", e);
            }
        })
    }
    
    /// Recover from WAL on startup
    pub async fn recover(&self) -> Result<()> {
        if let Some(wal) = &self.wal {
            let mut wal_guard = wal.lock().await;
            let recovered_points = wal_guard.recover().await?;
            
            if !recovered_points.is_empty() {
                info!("Recovered {} points from WAL", recovered_points.len());
                self.add_batch(recovered_points).await?;
            }
        }
        
        Ok(())
    }
    
    /// Shutdown the batch writer
    pub async fn shutdown(&self) -> Result<()> {
        // Signal shutdown
        *self.shutdown.write().await = true;
        
        // Final flush
        self.flush().await?;
        
        Ok(())
    }
    
    /// Get current statistics
    pub async fn get_stats(&self) -> BatchWriteStats {
        self.stats.read().await.clone()
    }
    
    /// Get current buffer size
    pub async fn buffer_size(&self) -> usize {
        self.buffer.read().await.len()
    }
}

impl BatchWriteStats {
    /// Calculate write success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.total_points_received as f64;
        if total > 0.0 {
            (self.total_points_written as f64 / total) * 100.0
        } else {
            0.0
        }
    }
    
    /// Calculate batch success rate
    pub fn batch_success_rate(&self) -> f64 {
        let total = (self.total_batches_written + self.total_batches_failed) as f64;
        if total > 0.0 {
            (self.total_batches_written as f64 / total) * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{DataValue, Storage};
    
    struct MockWriter {
        write_count: Arc<Mutex<usize>>,
        fail_count: usize,
    }
    
    #[async_trait]
    impl BatchWriter for MockWriter {
        async fn write_batch(&mut self, points: &[DataPoint]) -> Result<()> {
            let mut count = self.write_count.lock().await;
            if *count < self.fail_count {
                *count += 1;
                Err(HisSrvError::WriteError("Mock error".to_string()))
            } else {
                Ok(())
            }
        }
        
        fn name(&self) -> &str {
            "mock"
        }
    }
    
    #[tokio::test]
    async fn test_batch_writer_basic() {
        let writer = MockWriter {
            write_count: Arc::new(Mutex::new(0)),
            fail_count: 0,
        };
        
        let config = BatchWriterConfig {
            max_batch_size: 2,
            ..Default::default()
        };
        
        let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
        
        // Add points
        let point1 = DataPoint {
            key: "test1".to_string(),
            value: DataValue::Float(1.0),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        };
        
        let point2 = DataPoint {
            key: "test2".to_string(),
            value: DataValue::Float(2.0),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        };
        
        buffer.add(point1).await.unwrap();
        assert_eq!(buffer.buffer_size().await, 1);
        
        // Adding second point should trigger flush
        buffer.add(point2).await.unwrap();
        assert_eq!(buffer.buffer_size().await, 0);
        
        let stats = buffer.get_stats().await;
        assert_eq!(stats.total_points_received, 2);
        assert_eq!(stats.total_points_written, 2);
    }
    
    #[tokio::test]
    async fn test_batch_writer_retry() {
        let writer = MockWriter {
            write_count: Arc::new(Mutex::new(0)),
            fail_count: 2, // Fail first 2 attempts
        };
        
        let config = BatchWriterConfig {
            max_batch_size: 1,
            max_retries: 3,
            retry_delay_ms: 10,
            ..Default::default()
        };
        
        let buffer = Arc::new(BatchWriteBuffer::new(writer, config).unwrap());
        
        let point = DataPoint {
            key: "test".to_string(),
            value: DataValue::Float(1.0),
            timestamp: Utc::now(),
            tags: Default::default(),
            metadata: Default::default(),
        };
        
        // Should succeed after retries
        buffer.add(point).await.unwrap();
        
        let stats = buffer.get_stats().await;
        assert_eq!(stats.total_points_written, 1);
        assert_eq!(stats.total_batches_written, 1);
    }
}