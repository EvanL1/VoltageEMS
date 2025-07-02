use crate::storage::StorageManager;
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use utoipa::ToSchema;

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
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_messages: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            total_api_requests: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            message_timestamps: Arc::new(RwLock::new(Vec::new())),
            api_timestamps: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn record_message_processed(&self) {
        use std::sync::atomic::Ordering;
        
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        
        let mut timestamps = self.message_timestamps.write().await;
        let now = Instant::now();
        
        // Keep only timestamps from the last minute
        timestamps.retain(|&ts| now.duration_since(ts) < Duration::from_secs(60));
        timestamps.push(now);
    }

    pub async fn record_api_request(&self) {
        use std::sync::atomic::Ordering;
        
        self.total_api_requests.fetch_add(1, Ordering::Relaxed);
        
        let mut timestamps = self.api_timestamps.write().await;
        let now = Instant::now();
        
        // Keep only timestamps from the last minute
        timestamps.retain(|&ts| now.duration_since(ts) < Duration::from_secs(60));
        timestamps.push(now);
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
        // Simple implementation - in production, you might want to use sysinfo crate
        // For now, return dummy values
        (0, 0.0)
    }

    async fn collect_storage_metrics(&self, storage_manager: &StorageManager) -> HashMap<String, StorageBackendMetrics> {
        let mut metrics = HashMap::new();

        let all_stats = storage_manager.get_all_statistics().await;
        
        for (backend_name, stats) in all_stats {
            let backend_metrics = StorageBackendMetrics {
                connection_status: stats.connection_status,
                total_operations: 0, // TODO: Implement operation counting
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

/// Get service metrics
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "monitoring",
    responses(
        (status = 200, description = "Metrics retrieved successfully", body = Metrics),
    )
)]
pub async fn get_metrics(
    State(state): State<MonitoringState>,
) -> Result<Json<Metrics>, StatusCode> {
    let storage_manager = state.storage_manager.read().await;
    let metrics = state.metrics_collector.get_metrics(&*storage_manager).await;
    Ok(Json(metrics))
}

pub fn create_monitoring_router(state: MonitoringState) -> Router {
    Router::new()
        .route("/metrics", get(get_metrics))
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