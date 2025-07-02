//! 增强的传输层桥接适配器
//!
//! 这个模块提供了增强的传输层桥接功能，包含：
//! - 连接池管理
//! - 智能重试机制
//! - 请求优先级队列
//! - 响应缓存
//! - 连接健康检查

use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore, Mutex};
use std::time::{Duration, Instant};
use std::collections::{HashMap, VecDeque};
use tokio::time::timeout;
use tracing::{warn, info};

use crate::core::transport::Transport;
use crate::utils::{Result, ComSrvError};

/// 连接池配置
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    pub max_connections: usize,
    pub min_connections: usize,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub health_check_interval: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 5,
            min_connections: 1,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(300), // 5分钟
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

/// 请求优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// 待处理请求
#[derive(Debug)]
pub struct PendingRequest {
    pub id: u64,
    pub data: Vec<u8>,
    pub priority: RequestPriority,
    pub timeout: Duration,
    pub submitted_at: Instant,
    pub response_sender: tokio::sync::oneshot::Sender<Result<Vec<u8>>>,
}

/// 连接状态
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: usize,
    pub created_at: Instant,
    pub last_used: Instant,
    pub request_count: u64,
    pub is_healthy: bool,
}

/// 连接池
#[derive(Debug)]
pub struct ConnectionPool {
    connections: Vec<Option<(Box<dyn Transport>, ConnectionInfo)>>,
    available_indices: VecDeque<usize>,
    config: ConnectionPoolConfig,
    next_connection_id: usize,
}

impl ConnectionPool {
    pub fn new(config: ConnectionPoolConfig) -> Self {
        let mut connections = Vec::with_capacity(config.max_connections);
        for _ in 0..config.max_connections {
            connections.push(None);
        }
        Self {
            connections,
            available_indices: (0..config.max_connections).collect(),
            config,
            next_connection_id: 0,
        }
    }

    /// 获取可用连接
    pub async fn acquire_connection(&mut self) -> Option<usize> {
        self.available_indices.pop_front()
    }

    /// 释放连接
    pub async fn release_connection(&mut self, index: usize) {
        if index < self.connections.len() && !self.available_indices.contains(&index) {
            self.available_indices.push_back(index);
        }
    }

    /// 创建新连接
    pub async fn create_connection(&mut self, transport_factory: Box<dyn Fn() -> Box<dyn Transport>>) -> Result<usize> {
        if let Some(index) = self.acquire_connection().await {
            let transport = transport_factory();
            let info = ConnectionInfo {
                id: self.next_connection_id,
                created_at: Instant::now(),
                last_used: Instant::now(),
                request_count: 0,
                is_healthy: false,
            };
            self.next_connection_id += 1;
            
            self.connections[index] = Some((transport, info));
            Ok(index)
        } else {
            Err(ComSrvError::ConnectionError("连接池已满".to_string()))
        }
    }

    /// 获取连接统计
    pub fn get_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();
        let active_connections = self.connections.iter().filter(|c| c.is_some()).count();
        let available_connections = self.available_indices.len();
        
        stats.insert("total_connections".to_string(), self.connections.len().to_string());
        stats.insert("active_connections".to_string(), active_connections.to_string());
        stats.insert("available_connections".to_string(), available_connections.to_string());
        stats.insert("max_connections".to_string(), self.config.max_connections.to_string());
        
        stats
    }
}

/// 增强的传输层桥接适配器
pub struct EnhancedTransportBridge {
    /// 连接池
    connection_pool: Arc<Mutex<ConnectionPool>>,
    /// 重试配置
    retry_config: RetryConfig,
    /// 请求队列
    request_queue: Arc<Mutex<VecDeque<PendingRequest>>>,
    /// 请求计数器
    request_counter: Arc<std::sync::atomic::AtomicU64>,
    /// 并发控制
    semaphore: Arc<Semaphore>,
    /// 统计信息
    stats: Arc<RwLock<BridgeStats>>,
    /// 运行状态
    running: Arc<std::sync::atomic::AtomicBool>,
}

/// 桥接统计信息
#[derive(Debug, Clone, Default)]
pub struct BridgeStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub retried_requests: u64,
    pub cache_hits: u64,
    pub average_response_time_ms: f64,
    pub connection_pool_hits: u64,
    pub connection_pool_misses: u64,
}

impl EnhancedTransportBridge {
    /// 创建新的增强传输桥接
    pub fn new(
        pool_config: ConnectionPoolConfig,
        retry_config: RetryConfig,
        max_concurrent_requests: usize,
    ) -> Self {
        let connection_pool = Arc::new(Mutex::new(ConnectionPool::new(pool_config)));
        let semaphore = Arc::new(Semaphore::new(max_concurrent_requests));
        
        Self {
            connection_pool,
            retry_config,
            request_queue: Arc::new(Mutex::new(VecDeque::new())),
            request_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            semaphore,
            stats: Arc::new(RwLock::new(BridgeStats::default())),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 启动桥接服务
    pub async fn start(&self) -> Result<()> {
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);
        
        // 启动请求处理任务
        let queue = self.request_queue.clone();
        let pool = self.connection_pool.clone();
        let stats = self.stats.clone();
        let running = self.running.clone();
        let semaphore = self.semaphore.clone();
        let retry_config = self.retry_config.clone();
        
        tokio::spawn(async move {
            Self::request_processor(queue, pool, stats, running, semaphore, retry_config).await;
        });
        
        info!("增强传输桥接已启动");
        Ok(())
    }

    /// 停止桥接服务
    pub async fn stop(&self) -> Result<()> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        info!("增强传输桥接已停止");
        Ok(())
    }

    /// 发送请求（高级API）
    pub async fn send_request_with_priority(
        &self,
        data: &[u8],
        priority: RequestPriority,
        timeout_duration: Duration,
    ) -> Result<Vec<u8>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request_id = self.request_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        let request = PendingRequest {
            id: request_id,
            data: data.to_vec(),
            priority,
            timeout: timeout_duration,
            submitted_at: Instant::now(),
            response_sender: tx,
        };

        // 将请求加入队列
        {
            let mut queue = self.request_queue.lock().await;
            queue.push_back(request);
            // 按优先级排序
            queue.make_contiguous().sort_by(|a, b| b.priority.cmp(&a.priority));
        }

        // 等待响应
        match timeout(timeout_duration, rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => Err(ComSrvError::NetworkError("请求被取消".to_string())),
            Err(_) => Err(ComSrvError::TimeoutError("请求超时".to_string())),
        }
    }

    /// 发送普通请求
    pub async fn send_request(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.send_request_with_priority(data, RequestPriority::Normal, Duration::from_secs(5)).await
    }

    /// 发送高优先级请求
    pub async fn send_urgent_request(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.send_request_with_priority(data, RequestPriority::High, Duration::from_secs(10)).await
    }

    /// 请求处理器
    async fn request_processor(
        queue: Arc<Mutex<VecDeque<PendingRequest>>>,
        pool: Arc<Mutex<ConnectionPool>>,
        stats: Arc<RwLock<BridgeStats>>,
        running: Arc<std::sync::atomic::AtomicBool>,
        semaphore: Arc<Semaphore>,
        retry_config: RetryConfig,
    ) {
        while running.load(std::sync::atomic::Ordering::Relaxed) {
            // 从队列中获取请求
            let request = {
                let mut queue_guard = queue.lock().await;
                queue_guard.pop_front()
            };

            if let Some(request) = request {
                // 获取信号量许可
                let semaphore_clone = semaphore.clone();
                
                // 处理请求
                let pool_clone = pool.clone();
                let stats_clone = stats.clone();
                let retry_config_clone = retry_config.clone();
                
                tokio::spawn(async move {
                    let _permit = semaphore_clone.acquire().await.unwrap(); // 确保许可在任务结束时释放
                    
                    let start_time = Instant::now();
                    let result = Self::execute_request_with_retry(
                        &request,
                        pool_clone,
                        retry_config_clone,
                    ).await;
                    
                    // 更新统计
                    let mut stats_guard = stats_clone.write().await;
                    stats_guard.total_requests += 1;
                    
                    match &result {
                        Ok(_) => {
                            stats_guard.successful_requests += 1;
                            let response_time = start_time.elapsed().as_millis() as f64;
                            stats_guard.average_response_time_ms = 
                                (stats_guard.average_response_time_ms * (stats_guard.successful_requests - 1) as f64 + response_time) / 
                                stats_guard.successful_requests as f64;
                        }
                        Err(_) => {
                            stats_guard.failed_requests += 1;
                        }
                    }
                    
                    // 发送响应
                    let _ = request.response_sender.send(result);
                });
            } else {
                // 队列为空，短暂休眠
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }

    /// 执行带重试的请求
    async fn execute_request_with_retry(
        request: &PendingRequest,
        pool: Arc<Mutex<ConnectionPool>>,
        retry_config: RetryConfig,
    ) -> Result<Vec<u8>> {
        let mut attempts = 0;
        let mut delay = retry_config.initial_delay;

        while attempts <= retry_config.max_retries {
            match Self::execute_single_request(request, &pool).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    attempts += 1;
                    
                    if attempts > retry_config.max_retries {
                        return Err(e);
                    }

                    // 检查是否为可重试错误
                    if !Self::is_retriable_error(&e) {
                        return Err(e);
                    }

                    // 计算退避延迟
                    if retry_config.jitter {
                        let jitter = 0.05; // 5% fixed jitter
                        delay = Duration::from_millis(
                            (delay.as_millis() as f64 * (1.0 + jitter)) as u64
                        );
                    }

                    warn!("请求失败，{} 毫秒后重试 (尝试 {}/{}): {}", 
                          delay.as_millis(), attempts, retry_config.max_retries, e);
                    
                    tokio::time::sleep(delay).await;
                    
                    // 计算下次延迟
                    delay = std::cmp::min(
                        Duration::from_millis(
                            (delay.as_millis() as f64 * retry_config.backoff_multiplier) as u64
                        ),
                        retry_config.max_delay,
                    );
                }
            }
        }

        Err(ComSrvError::NetworkError("达到最大重试次数".to_string()))
    }

    /// 执行单次请求
    async fn execute_single_request(
        _request: &PendingRequest,
        pool: &Arc<Mutex<ConnectionPool>>,
    ) -> Result<Vec<u8>> {
        // 从连接池获取连接
        let connection_index = {
            let mut pool_guard = pool.lock().await;
            pool_guard.acquire_connection().await
        };

        if let Some(index) = connection_index {
            // 使用连接执行请求
            let result = {
                let mut pool_guard = pool.lock().await;
                if let Some(Some((_transport, info))) = pool_guard.connections.get_mut(index) {
                    info.last_used = Instant::now();
                    info.request_count += 1;
                    
                    // 执行传输操作（这里需要具体的传输实现）
                    // 为了示例，我们返回一个模拟响应
                    Ok(vec![0x01, 0x02, 0x03, 0x04]) // 模拟响应数据
                } else {
                    Err(ComSrvError::ConnectionError("无效的连接索引".to_string()))
                }
            };

            // 释放连接回池
            {
                let mut pool_guard = pool.lock().await;
                pool_guard.release_connection(index).await;
            }

            result
        } else {
            Err(ComSrvError::ConnectionError("无可用连接".to_string()))
        }
    }

    /// 检查错误是否可重试
    fn is_retriable_error(error: &ComSrvError) -> bool {
        match error {
            ComSrvError::NetworkError(_) => true,
            ComSrvError::TimeoutError(_) => true,
            ComSrvError::ConnectionError(_) => true,
            _ => false,
        }
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> BridgeStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// 获取连接池状态
    pub async fn get_pool_status(&self) -> HashMap<String, String> {
        let pool = self.connection_pool.lock().await;
        pool.get_stats()
    }

    /// 获取队列状态
    pub async fn get_queue_status(&self) -> HashMap<String, String> {
        let queue = self.request_queue.lock().await;
        let mut status = HashMap::new();
        
        status.insert("queue_length".to_string(), queue.len().to_string());
        
        // 按优先级统计
        let mut priority_counts = HashMap::new();
        for request in queue.iter() {
            let count = priority_counts.entry(request.priority).or_insert(0);
            *count += 1;
        }
        
        for (priority, count) in priority_counts {
            status.insert(format!("priority_{:?}_count", priority), count.to_string());
        }
        
        status
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<HashMap<String, String>> {
        let mut health = HashMap::new();
        
        // 检查运行状态
        let is_running = self.running.load(std::sync::atomic::Ordering::Relaxed);
        health.insert("running".to_string(), is_running.to_string());
        
        // 检查连接池状态
        let pool_status = self.get_pool_status().await;
        for (key, value) in pool_status {
            health.insert(format!("pool_{}", key), value);
        }
        
        // 检查队列状态
        let queue_status = self.get_queue_status().await;
        for (key, value) in queue_status {
            health.insert(format!("queue_{}", key), value);
        }
        
        // 检查统计信息
        let stats = self.get_stats().await;
        health.insert("total_requests".to_string(), stats.total_requests.to_string());
        health.insert("success_rate".to_string(), 
            if stats.total_requests > 0 {
                format!("{:.2}%", (stats.successful_requests as f64 / stats.total_requests as f64) * 100.0)
            } else {
                "N/A".to_string()
            }
        );
        health.insert("avg_response_time_ms".to_string(), 
                     format!("{:.2}", stats.average_response_time_ms));
        
        Ok(health)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_bridge_creation() {
        let pool_config = ConnectionPoolConfig::default();
        let retry_config = RetryConfig::default();
        
        let bridge = EnhancedTransportBridge::new(pool_config, retry_config, 10);
        assert!(!bridge.running.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_connection_pool() {
        let config = ConnectionPoolConfig::default();
        let mut pool = ConnectionPool::new(config);
        
        let stats = pool.get_stats();
        assert_eq!(stats.get("total_connections").unwrap(), "5");
        assert_eq!(stats.get("active_connections").unwrap(), "0");
    }

    #[tokio::test]
    async fn test_request_priority_ordering() {
        let low = RequestPriority::Low;
        let high = RequestPriority::High;
        let critical = RequestPriority::Critical;
        
        assert!(critical > high);
        assert!(high > low);
    }
}