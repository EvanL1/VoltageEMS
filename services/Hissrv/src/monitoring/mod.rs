use crate::storage::StorageManager;
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use prometheus::{Encoder, TextEncoder, Counter, Gauge, Histogram, HistogramOpts, Registry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use utoipa::ToSchema;
use tracing::{debug, error, info};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Metrics {
    /// Process uptime in seconds
    pub uptime_seconds: u64,
    /// Total processed messages
    pub total_messages_processed: u64,
    /// Messages processed per second (rate)
    pub messages_per_second: f64,
    /// Total API requests
    pub total_api_requests: u64,
    /// API requests per second
    pub api_requests_per_second: f64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Storage backend metrics
    pub storage_metrics: HashMap<String, StorageBackendMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StorageBackendMetrics {
    /// Connection status
    pub connection_status: String,
    /// Total operations
    pub total_operations: u64,
    /// Operations per second
    pub operations_per_second: f64,
    /// Last operation timestamp
    pub last_operation: Option<chrono::DateTime<chrono::Utc>>,
    /// Error count
    pub error_count: u64,
}

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    start_time: Instant,
    total_messages: Arc<std::sync::atomic::AtomicU64>,
    total_api_requests: Arc<std::sync::atomic::AtomicU64>,
    message_timestamps: Arc<RwLock<Vec<Instant>>>,
    api_timestamps: Arc<RwLock<Vec<Instant>>>,
    prometheus_registry: Arc<Registry>,
    // Prometheus 指标
    messages_total: Counter,
    messages_rate: Gauge,
    api_requests_total: Counter,
    api_requests_rate: Gauge,
    processing_duration: Histogram,
    storage_operations_total: Counter,
    storage_errors_total: Counter,
    active_connections: Gauge,
    memory_usage_bytes: Gauge,
    cpu_usage_percent: Gauge,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        let registry = Registry::new();
        
        // 创建 Prometheus 指标
        let messages_total = Counter::new("hissrv_messages_total", "Total number of messages processed")
            .expect("Failed to create messages_total counter");
        let messages_rate = Gauge::new("hissrv_messages_rate", "Messages processing rate per second")
            .expect("Failed to create messages_rate gauge");
        let api_requests_total = Counter::new("hissrv_api_requests_total", "Total number of API requests")
            .expect("Failed to create api_requests_total counter");
        let api_requests_rate = Gauge::new("hissrv_api_requests_rate", "API requests rate per second")
            .expect("Failed to create api_requests_rate gauge");
        
        let processing_duration = Histogram::with_opts(
            HistogramOpts::new("hissrv_processing_duration_seconds", "Message processing duration")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0])
        ).expect("Failed to create processing_duration histogram");
        
        let storage_operations_total = Counter::new("hissrv_storage_operations_total", "Total storage operations")
            .expect("Failed to create storage_operations_total counter");
        let storage_errors_total = Counter::new("hissrv_storage_errors_total", "Total storage errors")
            .expect("Failed to create storage_errors_total counter");
        let active_connections = Gauge::new("hissrv_active_connections", "Number of active connections")
            .expect("Failed to create active_connections gauge");
        let memory_usage_bytes = Gauge::new("hissrv_memory_usage_bytes", "Memory usage in bytes")
            .expect("Failed to create memory_usage_bytes gauge");
        let cpu_usage_percent = Gauge::new("hissrv_cpu_usage_percent", "CPU usage percentage")
            .expect("Failed to create cpu_usage_percent gauge");
        
        // 注册所有指标
        registry.register(Box::new(messages_total.clone())).unwrap();
        registry.register(Box::new(messages_rate.clone())).unwrap();
        registry.register(Box::new(api_requests_total.clone())).unwrap();
        registry.register(Box::new(api_requests_rate.clone())).unwrap();
        registry.register(Box::new(processing_duration.clone())).unwrap();
        registry.register(Box::new(storage_operations_total.clone())).unwrap();
        registry.register(Box::new(storage_errors_total.clone())).unwrap();
        registry.register(Box::new(active_connections.clone())).unwrap();
        registry.register(Box::new(memory_usage_bytes.clone())).unwrap();
        registry.register(Box::new(cpu_usage_percent.clone())).unwrap();
        
        Self {
            start_time: Instant::now(),
            total_messages: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            total_api_requests: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            message_timestamps: Arc::new(RwLock::new(Vec::new())),
            api_timestamps: Arc::new(RwLock::new(Vec::new())),
            prometheus_registry: Arc::new(registry),
            messages_total,
            messages_rate,
            api_requests_total,
            api_requests_rate,
            processing_duration,
            storage_operations_total,
            storage_errors_total,
            active_connections,
            memory_usage_bytes,
            cpu_usage_percent,
        }
    }

    pub async fn record_message_processed(&self) {
        use std::sync::atomic::Ordering;

        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.messages_total.inc();

        let mut timestamps = self.message_timestamps.write().await;
        let now = Instant::now();

        // Keep only timestamps from the last minute
        timestamps.retain(|&ts| now.duration_since(ts) < Duration::from_secs(60));
        timestamps.push(now);
        
        // 更新速率
        let rate = timestamps.len() as f64 / 60.0;
        self.messages_rate.set(rate);
    }
    
    pub async fn record_message_processing_time(&self, duration: Duration) {
        self.processing_duration.observe(duration.as_secs_f64());
    }
    
    pub async fn record_storage_operation(&self) {
        self.storage_operations_total.inc();
    }
    
    pub async fn record_storage_error(&self) {
        self.storage_errors_total.inc();
    }
    
    pub async fn update_active_connections(&self, count: f64) {
        self.active_connections.set(count);
    }

    pub async fn record_api_request(&self) {
        use std::sync::atomic::Ordering;

        self.total_api_requests.fetch_add(1, Ordering::Relaxed);
        self.api_requests_total.inc();

        let mut timestamps = self.api_timestamps.write().await;
        let now = Instant::now();

        // Keep only timestamps from the last minute
        timestamps.retain(|&ts| now.duration_since(ts) < Duration::from_secs(60));
        timestamps.push(now);
        
        // 更新速率
        let rate = timestamps.len() as f64 / 60.0;
        self.api_requests_rate.set(rate);
    }

    pub async fn get_metrics(&self, storage_manager: &StorageManager) -> Metrics {
        use std::sync::atomic::Ordering;

        let uptime = self.start_time.elapsed().as_secs();
        let total_messages = self.total_messages.load(Ordering::Relaxed);
        let total_api_requests = self.total_api_requests.load(Ordering::Relaxed);

        // Calculate rates based on recent activity
        let message_timestamps = self.message_timestamps.read().await;
        let api_timestamps = self.api_timestamps.read().await;

        let messages_per_second = message_timestamps.len() as f64 / 60.0;
        let api_requests_per_second = api_timestamps.len() as f64 / 60.0;

        // Get system metrics
        let (memory_usage_bytes, cpu_usage_percent) = self.get_system_metrics();

        // Get storage metrics
        let storage_metrics = self.collect_storage_metrics(storage_manager).await;

        Metrics {
            uptime_seconds: uptime,
            total_messages_processed: total_messages,
            messages_per_second,
            total_api_requests,
            api_requests_per_second,
            memory_usage_bytes,
            cpu_usage_percent,
            storage_metrics,
        }
    }

    fn get_system_metrics(&self) -> (u64, f64) {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            
            // 获取内存使用情况
            let memory_usage = if let Ok(status) = fs::read_to_string("/proc/self/status") {
                status.lines()
                    .find(|line| line.starts_with("VmRSS:"))
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(|kb| kb * 1024) // 转换为字节
                    })
                    .unwrap_or(0)
            } else {
                0
            };
            
            // 获取 CPU 使用率（简化版本）
            let cpu_usage = if let Ok(stat) = fs::read_to_string("/proc/self/stat") {
                // 这是一个简化的实现，实际应用中需要更复杂的计算
                0.0
            } else {
                0.0
            };
            
            // 更新 Prometheus 指标
            self.memory_usage_bytes.set(memory_usage as f64);
            self.cpu_usage_percent.set(cpu_usage);
            
            (memory_usage, cpu_usage)
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // 非 Linux 系统返回默认值
            (0, 0.0)
        }
    }
    
    /// 获取 Prometheus 格式的指标
    pub fn get_prometheus_metrics(&self) -> Result<String, Box<dyn std::error::Error>> {
        let encoder = TextEncoder::new();
        let metric_families = self.prometheus_registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }

    async fn collect_storage_metrics(
        &self,
        storage_manager: &StorageManager,
    ) -> HashMap<String, StorageBackendMetrics> {
        let mut metrics = HashMap::new();

        let all_stats = storage_manager.get_all_statistics().await;

        for (backend_name, stats) in all_stats {
            let backend_metrics = StorageBackendMetrics {
                connection_status: stats.connection_status,
                total_operations: 0,        // TODO: Implement operation counting
                operations_per_second: 0.0, // TODO: Implement operation rate tracking
                last_operation: stats.last_write_time.or(stats.last_read_time),
                error_count: 0, // TODO: Implement error counting
            };

            metrics.insert(backend_name, backend_metrics);
        }

        metrics
    }
}

#[derive(Clone)]
pub struct MonitoringState {
    pub metrics_collector: MetricsCollector,
    pub storage_manager: Arc<RwLock<StorageManager>>,
}

/// Get service metrics in JSON format
#[utoipa::path(
    get,
    path = "/metrics/json",
    tag = "monitoring",
    responses(
        (status = 200, description = "Metrics retrieved successfully", body = Metrics),
    )
)]
pub async fn get_metrics_json(
    State(state): State<MonitoringState>,
) -> Result<Json<Metrics>, StatusCode> {
    let storage_manager = state.storage_manager.read().await;
    let metrics = state.metrics_collector.get_metrics(&*storage_manager).await;
    Ok(Json(metrics))
}

/// Get service metrics in Prometheus format
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "monitoring",
    responses(
        (status = 200, description = "Prometheus metrics retrieved successfully", body = String),
    )
)]
pub async fn get_prometheus_metrics(
    State(state): State<MonitoringState>,
) -> Result<String, StatusCode> {
    match state.metrics_collector.get_prometheus_metrics() {
        Ok(metrics) => Ok(metrics),
        Err(e) => {
            error!("Failed to encode Prometheus metrics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    tag = "monitoring",
    responses(
        (status = 200, description = "Service is healthy", body = HealthStatus),
        (status = 503, description = "Service is unhealthy", body = HealthStatus),
    )
)]
pub async fn health_check(
    State(state): State<MonitoringState>,
) -> Result<Json<HealthStatus>, StatusCode> {
    let storage_manager = state.storage_manager.read().await;
    let all_stats = storage_manager.get_all_statistics().await;
    
    let mut healthy = true;
    let mut checks = HashMap::new();
    
    // 检查存储后端连接状态
    for (backend_name, stats) in all_stats {
        let backend_healthy = stats.connection_status == "connected";
        if !backend_healthy {
            healthy = false;
        }
        checks.insert(
            format!("storage_{}", backend_name),
            HealthCheck {
                status: if backend_healthy { "healthy".to_string() } else { "unhealthy".to_string() },
                message: stats.connection_status.clone(),
            }
        );
    }
    
    // 检查消息处理
    let message_rate = state.metrics_collector.messages_rate.get();
    checks.insert(
        "message_processing".to_string(),
        HealthCheck {
            status: "healthy".to_string(),
            message: format!("Processing {} msg/s", message_rate),
        }
    );
    
    let status = HealthStatus {
        status: if healthy { "healthy".to_string() } else { "unhealthy".to_string() },
        timestamp: chrono::Utc::now(),
        checks,
    };
    
    if healthy {
        Ok(Json(status))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub checks: HashMap<String, HealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthCheck {
    pub status: String,
    pub message: String,
}

pub fn create_monitoring_router(state: MonitoringState) -> Router {
    Router::new()
        .route("/metrics", get(get_prometheus_metrics))
        .route("/metrics/json", get(get_metrics_json))
        .route("/health", get(health_check))
        .with_state(state)
}

// Middleware for tracking API requests
pub async fn api_metrics_middleware<B>(
    State(collector): State<MetricsCollector>,
    request: axum::http::Request<B>,
    next: axum::middleware::Next<B>,
) -> axum::response::Response {
    collector.record_api_request().await;
    next.run(request).await
}
