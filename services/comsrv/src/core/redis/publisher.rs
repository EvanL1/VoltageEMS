//! Redis Pub/Sub Publisher Implementation
//!
//! Provides batched publishing functionality with configurable buffering

use crate::utils::error::{ComSrvError, Result};
use redis::aio::ConnectionManager;
use redis::Pipeline;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;
use tracing::{debug, error};

/// Published message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointMessage {
    /// Channel ID
    pub channel_id: u16,
    /// Point type (m/s/c/a)
    pub point_type: String,
    /// Point ID
    pub point_id: u32,
    /// Point value
    pub value: f64,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: i64,
    /// Message version
    pub version: String,
}

impl PointMessage {
    /// Create new message
    pub fn new(
        channel_id: u16,
        point_type: String,
        point_id: u32,
        value: f64,
        timestamp: i64,
        version: String,
    ) -> Self {
        Self {
            channel_id,
            point_type,
            point_id,
            value,
            timestamp,
            version,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| ComSrvError::Storage(format!("Failed to serialize message: {}", e)))
    }

    /// Get channel name for publishing
    pub fn channel_name(&self) -> String {
        format!("{}:{}:{}", self.channel_id, self.point_type, self.point_id)
    }
}

/// Batched message for publishing
#[derive(Debug)]
struct BatchedMessage {
    channel: String,
    message: String,
    timestamp: Instant,
}

/// Redis publisher with batching support
pub struct RedisPublisher {
    /// Connection manager
    conn: Arc<Mutex<ConnectionManager>>,
    /// Message sender
    sender: mpsc::Sender<BatchedMessage>,
    /// Configuration
    config: PublisherConfig,
}

/// Publisher configuration
#[derive(Debug, Clone)]
pub struct PublisherConfig {
    /// Whether publishing is enabled
    pub enabled: bool,
    /// Batch size
    pub batch_size: usize,
    /// Batch timeout
    pub batch_timeout: Duration,
    /// Message version
    pub message_version: String,
}

impl RedisPublisher {
    /// Create new publisher
    pub async fn new(
        conn: ConnectionManager,
        config: PublisherConfig,
    ) -> Result<(Self, PublisherHandle)> {
        let (sender, receiver) = mpsc::channel::<BatchedMessage>(1000);
        let conn = Arc::new(Mutex::new(conn));

        let publisher = Self {
            conn: conn.clone(),
            sender,
            config: config.clone(),
        };

        // Start background worker
        let handle = PublisherHandle::start(conn, receiver, config).await;

        Ok((publisher, handle))
    }

    /// Publish a single point update
    pub async fn publish_point(
        &self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
        timestamp: i64,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let message = PointMessage::new(
            channel_id,
            point_type.to_string(),
            point_id,
            value,
            timestamp,
            self.config.message_version.clone(),
        );

        let channel = message.channel_name();
        let json = message.to_json()?;

        self.send_message(channel, json).await
    }

    /// Publish multiple point updates
    pub async fn publish_points(&self, updates: &[(u16, &str, u32, f64, i64)]) -> Result<()> {
        if !self.config.enabled || updates.is_empty() {
            return Ok(());
        }

        for (channel_id, point_type, point_id, value, timestamp) in updates {
            self.publish_point(*channel_id, point_type, *point_id, *value, *timestamp)
                .await?;
        }

        Ok(())
    }

    /// Send message to background worker
    async fn send_message(&self, channel: String, message: String) -> Result<()> {
        let batched = BatchedMessage {
            channel,
            message,
            timestamp: Instant::now(),
        };

        self.sender
            .send(batched)
            .await
            .map_err(|_| ComSrvError::Storage("Publisher channel closed".to_string()))?;

        Ok(())
    }
}

/// Handle for background publisher task
pub struct PublisherHandle {
    task: tokio::task::JoinHandle<()>,
}

impl PublisherHandle {
    /// Start background publisher
    async fn start(
        conn: Arc<Mutex<ConnectionManager>>,
        mut receiver: mpsc::Receiver<BatchedMessage>,
        config: PublisherConfig,
    ) -> Self {
        let task = tokio::spawn(async move {
            let mut buffer = Vec::with_capacity(config.batch_size);
            let mut ticker = interval(config.batch_timeout);

            loop {
                tokio::select! {
                    Some(msg) = receiver.recv() => {
                        buffer.push(msg);

                        // Flush if buffer is full
                        if buffer.len() >= config.batch_size {
                            Self::flush_buffer(&conn, &mut buffer).await;
                        }
                    }
                    _ = ticker.tick() => {
                        // Flush on timeout
                        if !buffer.is_empty() {
                            Self::flush_buffer(&conn, &mut buffer).await;
                        }
                    }
                    else => {
                        // Channel closed, flush remaining and exit
                        if !buffer.is_empty() {
                            Self::flush_buffer(&conn, &mut buffer).await;
                        }
                        break;
                    }
                }
            }
        });

        Self { task }
    }

    /// Flush buffered messages
    async fn flush_buffer(conn: &Arc<Mutex<ConnectionManager>>, buffer: &mut Vec<BatchedMessage>) {
        if buffer.is_empty() {
            return;
        }

        let start = Instant::now();
        let count = buffer.len();

        // Use pipeline for batch publishing
        let mut pipe = Pipeline::new();
        for msg in buffer.iter() {
            pipe.publish(&msg.channel, &msg.message);
        }

        // Execute pipeline
        let mut conn = conn.lock().await;
        match pipe.query_async::<()>(&mut *conn).await {
            Ok(_) => {
                let elapsed = start.elapsed();
                debug!(
                    "Published {} messages in {:?} (avg {:?}/msg)",
                    count,
                    elapsed,
                    elapsed / count as u32
                );
            }
            Err(e) => {
                error!("Failed to publish batch: {}", e);
            }
        }

        buffer.clear();
    }

    /// Wait for publisher to finish
    pub async fn wait(self) {
        let _ = self.task.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_message() {
        let msg = PointMessage::new(
            1001,
            "m".to_string(),
            10001,
            25.6,
            1234567890,
            "1.0".to_string(),
        );

        assert_eq!(msg.channel_name(), "1001:m:10001");

        let json = msg.to_json().unwrap();
        assert!(json.contains("1001"));
        assert!(json.contains("25.6"));
    }

    #[test]
    fn test_publisher_config() {
        let config = PublisherConfig {
            enabled: true,
            batch_size: 100,
            batch_timeout: Duration::from_millis(50),
            message_version: "1.0".to_string(),
        };

        assert!(config.enabled);
        assert_eq!(config.batch_size, 100);
    }
}
