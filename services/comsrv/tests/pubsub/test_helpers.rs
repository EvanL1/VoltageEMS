//! Pub/Sub测试辅助工具

use redis::aio::{ConnectionManager, PubSub};
use redis::{Client, AsyncCommands};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio::time::timeout;
use serde::{Deserialize, Serialize};

/// 测试消息格式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestPointMessage {
    pub channel_id: u16,
    pub point_type: String,
    pub point_id: u32,
    pub value: f64,
    pub timestamp: i64,
    pub version: String,
}

/// 消息收集器
pub struct MessageCollector {
    receiver: mpsc::Receiver<TestPointMessage>,
    messages: Vec<TestPointMessage>,
}

impl MessageCollector {
    /// 创建新的消息收集器
    pub async fn new(redis_url: &str, patterns: Vec<String>) -> Result<(Self, MessageSubscriber), Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel(1000);
        
        let subscriber = MessageSubscriber::new(redis_url, patterns, tx).await?;
        
        Ok((
            Self {
                receiver: rx,
                messages: Vec::new(),
            },
            subscriber,
        ))
    }
    
    /// 收集消息直到超时
    pub async fn collect_until_timeout(&mut self, duration: Duration) -> Vec<TestPointMessage> {
        let deadline = Instant::now() + duration;
        
        while Instant::now() < deadline {
            match timeout(Duration::from_millis(100), self.receiver.recv()).await {
                Ok(Some(msg)) => self.messages.push(msg),
                _ => {}
            }
        }
        
        self.messages.clone()
    }
    
    /// 收集指定数量的消息
    pub async fn collect_n(&mut self, n: usize, max_wait: Duration) -> Result<Vec<TestPointMessage>, String> {
        let deadline = Instant::now() + max_wait;
        
        while self.messages.len() < n && Instant::now() < deadline {
            match timeout(Duration::from_millis(100), self.receiver.recv()).await {
                Ok(Some(msg)) => self.messages.push(msg),
                _ => {}
            }
        }
        
        if self.messages.len() >= n {
            Ok(self.messages[..n].to_vec())
        } else {
            Err(format!("Only collected {} messages, expected {}", self.messages.len(), n))
        }
    }
    
    /// 获取所有收集的消息
    pub fn get_messages(&self) -> &[TestPointMessage] {
        &self.messages
    }
    
    /// 清空消息
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

/// 消息订阅者
pub struct MessageSubscriber {
    task: tokio::task::JoinHandle<()>,
}

impl MessageSubscriber {
    /// 创建订阅者
    async fn new(
        redis_url: &str,
        patterns: Vec<String>,
        sender: mpsc::Sender<TestPointMessage>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::open(redis_url)?;
        let conn = client.get_async_connection().await?;
        
        let task = tokio::spawn(async move {
            let mut pubsub = conn.into_pubsub();
            
            // 订阅模式
            for pattern in patterns {
                let _ = pubsub.psubscribe(&pattern).await;
            }
            
            // 接收消息
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                if let Ok(payload) = msg.get_payload::<String>() {
                    if let Ok(point_msg) = serde_json::from_str::<TestPointMessage>(&payload) {
                        let _ = sender.send(point_msg).await;
                    }
                }
            }
        });
        
        Ok(Self { task })
    }
    
    /// 停止订阅
    pub fn stop(self) {
        self.task.abort();
    }
}

/// 性能监控器
pub struct PerformanceMonitor {
    start_time: Instant,
    message_count: Arc<Mutex<usize>>,
    latencies: Arc<Mutex<Vec<Duration>>>,
}

impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            message_count: Arc::new(Mutex::new(0)),
            latencies: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// 记录消息
    pub async fn record_message(&self, latency: Duration) {
        let mut count = self.message_count.lock().await;
        *count += 1;
        
        let mut latencies = self.latencies.lock().await;
        latencies.push(latency);
    }
    
    /// 获取统计信息
    pub async fn get_stats(&self) -> PerformanceStats {
        let count = *self.message_count.lock().await;
        let elapsed = self.start_time.elapsed();
        let latencies = self.latencies.lock().await.clone();
        
        let avg_latency = if !latencies.is_empty() {
            let sum: Duration = latencies.iter().sum();
            sum / latencies.len() as u32
        } else {
            Duration::ZERO
        };
        
        let max_latency = latencies.iter().max().copied().unwrap_or(Duration::ZERO);
        let min_latency = latencies.iter().min().copied().unwrap_or(Duration::ZERO);
        
        PerformanceStats {
            total_messages: count,
            elapsed_time: elapsed,
            messages_per_second: if elapsed.as_secs() > 0 {
                count as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
            avg_latency,
            max_latency,
            min_latency,
        }
    }
}

/// 性能统计
#[derive(Debug)]
pub struct PerformanceStats {
    pub total_messages: usize,
    pub elapsed_time: Duration,
    pub messages_per_second: f64,
    pub avg_latency: Duration,
    pub max_latency: Duration,
    pub min_latency: Duration,
}

/// 创建测试Redis连接
pub async fn create_test_connection(redis_url: &str) -> Result<ConnectionManager, Box<dyn std::error::Error>> {
    let client = Client::open(redis_url)?;
    let conn = ConnectionManager::new(client).await?;
    Ok(conn)
}

/// 清理测试数据
pub async fn cleanup_test_data(
    conn: &mut ConnectionManager,
    channel_id: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let pattern = format!("{}:*", channel_id);
    let keys: Vec<String> = conn.keys(&pattern).await?;
    
    if !keys.is_empty() {
        conn.del(&keys).await?;
    }
    
    Ok(())
}

/// 生成测试点位数据
pub fn generate_test_points(channel_id: u16, count: usize) -> Vec<(u16, String, u32, f64)> {
    let mut points = Vec::new();
    
    for i in 0..count {
        let point_type = match i % 4 {
            0 => "m",
            1 => "s", 
            2 => "c",
            _ => "a",
        };
        
        points.push((
            channel_id,
            point_type.to_string(),
            10000 + i as u32,
            (i as f64) * 1.5,
        ));
    }
    
    points
}

/// 验证消息顺序
pub fn verify_message_order(messages: &[TestPointMessage]) -> bool {
    if messages.len() < 2 {
        return true;
    }
    
    for i in 1..messages.len() {
        if messages[i].timestamp < messages[i-1].timestamp {
            return false;
        }
    }
    
    true
}

/// 计算消息丢失率
pub fn calculate_message_loss(expected: usize, received: usize) -> f64 {
    if expected == 0 {
        return 0.0;
    }
    
    let loss = expected.saturating_sub(received);
    (loss as f64 / expected as f64) * 100.0
}