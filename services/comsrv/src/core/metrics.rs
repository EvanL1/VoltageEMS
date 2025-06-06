use prometheus::{
    Counter, Encoder, Gauge, GaugeVec, HistogramVec,
    IntCounterVec, Registry, TextEncoder, Opts,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task;
use hyper::{Body, Response, Server, Request};
use hyper::service::{make_service_fn, service_fn};
use parking_lot::RwLock;
use dashmap::DashMap;
use once_cell::sync::Lazy;

use crate::utils::{ComSrvError, Result};
use std::time::SystemTime;
use serde::{Serialize, Deserialize};

/// Data point for real-time monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    /// Point identifier
    pub id: String,
    /// Point value
    pub value: String,
    /// Data quality (0-100)
    pub quality: u8,
    /// Timestamp when the value was captured
    pub timestamp: SystemTime,
    /// Point description
    pub description: String,
}

impl DataPoint {
    /// Create a new data point
    pub fn new(id: String, value: String, quality: u8, description: String) -> Self {
        Self {
            id,
            value,
            quality,
            timestamp: SystemTime::now(),
            description,
        }
    }
}

/// Protocol-specific metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMetrics {
    /// Protocol name
    pub protocol: String,
    /// Total requests sent
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Connection uptime percentage
    pub uptime_percentage: f64,
    /// Last update timestamp
    pub last_update: SystemTime,
}

impl ProtocolMetrics {
    /// Create new protocol metrics
    pub fn new(protocol: String) -> Self {
        Self {
            protocol,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_response_time_ms: 0.0,
            uptime_percentage: 100.0,
            last_update: SystemTime::now(),
        }
    }
    
    /// Update metrics with new request data
    pub fn update_request(&mut self, success: bool, response_time_ms: f64) {
        self.total_requests += 1;
        if success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }
        
        // Update average response time
        if self.total_requests == 1 {
            self.avg_response_time_ms = response_time_ms;
        } else {
            let total_time = self.avg_response_time_ms * (self.total_requests - 1) as f64;
            self.avg_response_time_ms = (total_time + response_time_ms) / self.total_requests as f64;
        }
        
        // Update uptime percentage
        self.uptime_percentage = if self.total_requests > 0 {
            (self.successful_requests as f64 / self.total_requests as f64) * 100.0
        } else {
            100.0
        };
        
        self.last_update = SystemTime::now();
    }
}

/// Optimized metrics container with reduced allocations
struct MetricsContainer {
    // Communication metrics
    bytes_total: IntCounterVec,
    packets_total: IntCounterVec,
    packet_errors_total: IntCounterVec,
    packet_processing_duration: HistogramVec,

    // Channel metrics
    channel_status: GaugeVec,
    channel_response_time: GaugeVec,
    channel_errors_total: IntCounterVec,

    // Protocol metrics
    protocol_status: GaugeVec,
    protocol_errors_total: IntCounterVec,

    // Service metrics
    service_status: Gauge,
    service_uptime: Counter,
    service_errors_total: IntCounterVec,
}

impl MetricsContainer {
    fn new() -> Result<Self> {
        Ok(Self {
            bytes_total: IntCounterVec::new(
                Opts::new("comsrv_bytes_total", "Total number of bytes sent/received"),
                &["service", "direction"],
            )?,
            packets_total: IntCounterVec::new(
                Opts::new("comsrv_packets_total", "Total number of packets sent/received"),
                &["service", "direction"],
            )?,
            packet_errors_total: IntCounterVec::new(
                Opts::new("comsrv_packet_errors_total", "Total number of packet errors"),
                &["service", "type"],
            )?,
            packet_processing_duration: HistogramVec::new(
                prometheus::histogram_opts!(
                    "comsrv_packet_processing_duration_seconds",
                    "Packet processing duration in seconds",
                    vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
                ),
                &["service", "protocol"],
            )?,
            channel_status: GaugeVec::new(
                Opts::new("comsrv_channel_status", "Channel connection status (1=connected, 0=disconnected)"),
                &["service", "channel"],
            )?,
            channel_response_time: GaugeVec::new(
                Opts::new("comsrv_channel_response_time_seconds", "Channel response time in seconds"),
                &["service", "channel"],
            )?,
            channel_errors_total: IntCounterVec::new(
                Opts::new("comsrv_channel_errors_total", "Total number of channel errors"),
                &["service", "channel", "type"],
            )?,
            protocol_status: GaugeVec::new(
                Opts::new("comsrv_protocol_status", "Protocol status (1=active, 0=inactive)"),
                &["service", "protocol"],
            )?,
            protocol_errors_total: IntCounterVec::new(
                Opts::new("comsrv_protocol_errors_total", "Total number of protocol errors"),
                &["service", "protocol", "type"],
            )?,
            service_status: Gauge::new("comsrv_service_status", "Service status (1=running, 0=stopped)")?,
            service_uptime: Counter::new("comsrv_service_uptime_seconds", "Service uptime in seconds")?,
            service_errors_total: IntCounterVec::new(
                Opts::new("comsrv_service_errors_total", "Total number of service errors"),
                &["service", "type"],
            )?,
        })
    }

    fn register_all(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.bytes_total.clone()))?;
        registry.register(Box::new(self.packets_total.clone()))?;
        registry.register(Box::new(self.packet_errors_total.clone()))?;
        registry.register(Box::new(self.packet_processing_duration.clone()))?;
        registry.register(Box::new(self.channel_status.clone()))?;
        registry.register(Box::new(self.channel_response_time.clone()))?;
        registry.register(Box::new(self.channel_errors_total.clone()))?;
        registry.register(Box::new(self.protocol_status.clone()))?;
        registry.register(Box::new(self.protocol_errors_total.clone()))?;
        registry.register(Box::new(self.service_status.clone()))?;
        registry.register(Box::new(self.service_uptime.clone()))?;
        registry.register(Box::new(self.service_errors_total.clone()))?;
        Ok(())
    }
}

/// High-performance metrics manager
#[derive(Clone)]
pub struct Metrics {
    registry: Arc<Registry>,
    container: Arc<MetricsContainer>,
    // Cache frequently used label combinations
    label_cache: Arc<DashMap<String, Vec<String>>>,
}

impl Metrics {
    /// Create a new Metrics instance with optimized performance
    pub fn new(service_name: &str) -> Result<Self> {
        let registry = Arc::new(Registry::new());
        let container = Arc::new(MetricsContainer::new()?);
        
        // Register all metrics
        container.register_all(&registry)?;

        let metrics = Self {
            registry,
            container,
            label_cache: Arc::new(DashMap::new()),
        };

        // Pre-populate common label combinations
        metrics.initialize_label_cache(service_name);

        Ok(metrics)
    }

    /// Pre-populate label cache with common combinations
    fn initialize_label_cache(&self, service_name: &str) {
        let service_name = service_name.to_string();
        
        // Common direction labels
        self.label_cache.insert("send".to_string(), vec![service_name.clone(), "send".to_string()]);
        self.label_cache.insert("receive".to_string(), vec![service_name.clone(), "receive".to_string()]);
        
        // Common error types
        for error_type in ["timeout", "connection", "protocol", "serialization"] {
            self.label_cache.insert(
                format!("error_{}", error_type),
                vec![service_name.clone(), error_type.to_string()]
            );
        }

        // Common protocol types
        for protocol in ["modbus_tcp", "modbus_rtu", "iec104"] {
            self.label_cache.insert(
                format!("protocol_{}", protocol),
                vec![service_name.clone(), protocol.to_string()]
            );
        }
    }

    /// Get cached labels or create new ones
    fn get_labels(&self, key: &str, fallback: Vec<&str>) -> Vec<String> {
        self.label_cache
            .get(key)
            .map(|cached| cached.value().clone())
            .unwrap_or_else(|| fallback.into_iter().map(|s| s.to_string()).collect())
    }

    /// Start the metrics server with optimized performance
    pub async fn start_server(&self, addr: &str) -> Result<()> {
        let metrics = self.clone();
        let addr: SocketAddr = addr.parse().map_err(|e| {
            ComSrvError::ConfigError(format!("Invalid metrics bind address: {}, error: {}", addr, e))
        })?;

        task::spawn(async move {
            async fn serve_metrics(_req: Request<Body>, metrics: Metrics) -> hyper::Result<Response<Body>> {
                let encoder = TextEncoder::new();
                let mut buffer = Vec::with_capacity(8192); // Pre-allocate buffer
                let metric_families = metrics.registry.gather();
                encoder.encode(&metric_families, &mut buffer).unwrap();

                Ok(Response::builder()
                    .status(200)
                    .header("Content-Type", encoder.format_type())
                    .body(Body::from(buffer))
                    .unwrap())
            }

            let service = make_service_fn(move |_| {
                let metrics = metrics.clone();
                async move {
                    Ok::<_, hyper::Error>(service_fn(move |req| serve_metrics(req, metrics.clone())))
                }
            });

            let server = Server::bind(&addr).serve(service);
            if let Err(e) = server.await {
                tracing::error!("Metrics server error: {}", e);
            }
        });

        Ok(())
    }

    // High-performance metric recording methods with reduced allocations

    /// Record bytes with optimized labels
    #[inline]
    pub fn record_bytes(&self, direction: &str, count: u64, service_name: &str) {
        let labels = self.get_labels(direction, vec![service_name, direction]);
        self.container.bytes_total.with_label_values(&labels.iter().map(|s| s.as_str()).collect::<Vec<_>>()).inc_by(count);
    }

    /// Record packets with optimized labels
    #[inline]
    pub fn record_packets(&self, direction: &str, count: u64, service_name: &str) {
        let labels = self.get_labels(direction, vec![service_name, direction]);
        self.container.packets_total.with_label_values(&labels.iter().map(|s| s.as_str()).collect::<Vec<_>>()).inc_by(count);
    }

    /// Record packet error with optimized labels
    #[inline]
    pub fn record_packet_error(&self, error_type: &str, service_name: &str) {
        let key = format!("error_{}", error_type);
        let labels = self.get_labels(&key, vec![service_name, error_type]);
        self.container.packet_errors_total.with_label_values(&labels.iter().map(|s| s.as_str()).collect::<Vec<_>>()).inc();
    }

    /// Record packet processing duration with optimized labels
    #[inline]
    pub fn record_packet_processing(&self, protocol: &str, duration: f64, service_name: &str) {
        let key = format!("protocol_{}", protocol);
        let labels = self.get_labels(&key, vec![service_name, protocol]);
        self.container.packet_processing_duration.with_label_values(&labels.iter().map(|s| s.as_str()).collect::<Vec<_>>()).observe(duration);
    }

    /// Update channel status with optimized performance
    #[inline]
    pub fn update_channel_status(&self, channel: &str, connected: bool, service_name: &str) {
        let value = if connected { 1.0 } else { 0.0 };
        self.container.channel_status.with_label_values(&[service_name, channel]).set(value);
    }

    /// Update channel response time with optimized performance
    #[inline]
    pub fn update_channel_response_time(&self, channel: &str, time: f64, service_name: &str) {
        self.container.channel_response_time.with_label_values(&[service_name, channel]).set(time);
    }

    /// Record channel error with optimized performance
    #[inline]
    pub fn record_channel_error(&self, channel: &str, error_type: &str, service_name: &str) {
        self.container.channel_errors_total.with_label_values(&[service_name, channel, error_type]).inc();
    }

    /// Update protocol status with optimized performance
    #[inline]
    pub fn update_protocol_status(&self, protocol: &str, active: bool, service_name: &str) {
        let value = if active { 1.0 } else { 0.0 };
        self.container.protocol_status.with_label_values(&[service_name, protocol]).set(value);
    }

    /// Record protocol error with optimized performance
    #[inline]
    pub fn record_protocol_error(&self, protocol: &str, error_type: &str, service_name: &str) {
        self.container.protocol_errors_total.with_label_values(&[service_name, protocol, error_type]).inc();
    }

    /// Update service status
    #[inline]
    pub fn update_service_status(&self, running: bool) {
        let value = if running { 1.0 } else { 0.0 };
        self.container.service_status.set(value);
    }

    /// Increment uptime counter
    #[inline]
    pub fn increment_uptime(&self, seconds: f64) {
        self.container.service_uptime.inc_by(seconds);
    }

    /// Record service error with optimized performance
    #[inline]
    pub fn record_service_error(&self, error_type: &str, service_name: &str) {
        self.container.service_errors_total.with_label_values(&[service_name, error_type]).inc();
    }

    /// Get registry for external use
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }
}

impl From<prometheus::Error> for ComSrvError {
    fn from(err: prometheus::Error) -> Self {
        ComSrvError::InternalError(format!("Prometheus error: {}", err))
    }
}

/// Global metrics manager with lazy initialization
pub struct MetricsManager {
    instance: Arc<RwLock<Option<Metrics>>>,
}

impl MetricsManager {
    /// Create a new metrics manager
    pub fn new() -> Self {
        Self {
            instance: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize metrics with service name
    pub fn init(&self, service_name: &str) -> Result<()> {
        let metrics = Metrics::new(service_name)?;
        let mut instance = self.instance.write();
        *instance = Some(metrics);
        Ok(())
    }

    /// Get metrics instance
    pub fn get(&self) -> Option<Metrics> {
        self.instance.read().clone()
    }
}

impl Default for MetricsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global metrics manager instance
static METRICS_MANAGER: Lazy<MetricsManager> = Lazy::new(MetricsManager::new);

/// Get the global metrics manager
pub fn metrics_manager() -> &'static MetricsManager {
    &METRICS_MANAGER
}

/// Initialize global metrics
pub fn init_metrics(service_name: &str) -> Result<()> {
    metrics_manager().init(service_name)
}

/// Get global metrics instance
pub fn get_metrics() -> Option<Metrics> {
    metrics_manager().get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_metrics_container_creation() {
        let container = MetricsContainer::new().unwrap();
        
        // Verify all metrics are created successfully
        // The fact that these don't panic means they were created correctly
        let _bytes = &container.bytes_total;
        let _packets = &container.packets_total;
        let _channel_status = &container.channel_status;
        let _service_status = &container.service_status;
        
        // If we get here, all metrics were created successfully
        assert!(true);
    }

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new("test_service").unwrap();
        assert!(!metrics.registry.gather().is_empty());
    }

    #[test]
    fn test_record_bytes() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.record_bytes("send", 1024, "test_service");
        metrics.record_bytes("receive", 2048, "test_service");
        
        let families = metrics.registry.gather();
        let bytes_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_bytes_total")
            .unwrap();
        
        assert_eq!(bytes_metric.get_metric().len(), 2);
    }

    #[test]
    fn test_record_packets() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.record_packets("send", 10, "test_service");
        metrics.record_packets("receive", 15, "test_service");
        
        let families = metrics.registry.gather();
        let packets_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_packets_total")
            .unwrap();
        
        assert_eq!(packets_metric.get_metric().len(), 2);
    }

    #[test]
    fn test_record_packet_error() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.record_packet_error("timeout", "test_service");
        metrics.record_packet_error("connection", "test_service");
        
        let families = metrics.registry.gather();
        let error_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_packet_errors_total")
            .unwrap();
        
        assert_eq!(error_metric.get_metric().len(), 2);
    }

    #[test]
    fn test_record_packet_processing() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.record_packet_processing("modbus_tcp", 0.005, "test_service");
        metrics.record_packet_processing("iec104", 0.010, "test_service");
        
        let families = metrics.registry.gather();
        let duration_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_packet_processing_duration_seconds")
            .unwrap();
        
        assert_eq!(duration_metric.get_metric().len(), 2);
    }

    #[test]
    fn test_update_channel_status() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.update_channel_status("channel_1", true, "test_service");
        metrics.update_channel_status("channel_2", false, "test_service");
        
        let families = metrics.registry.gather();
        let status_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_channel_status")
            .unwrap();
        
        assert_eq!(status_metric.get_metric().len(), 2);
        
        // Check that values are correct
        let channel_1_metric = status_metric.get_metric().iter()
            .find(|m| m.get_label().iter().any(|l| l.get_value() == "channel_1"))
            .unwrap();
        assert_eq!(channel_1_metric.get_gauge().get_value(), 1.0);
        
        let channel_2_metric = status_metric.get_metric().iter()
            .find(|m| m.get_label().iter().any(|l| l.get_value() == "channel_2"))
            .unwrap();
        assert_eq!(channel_2_metric.get_gauge().get_value(), 0.0);
    }

    #[test]
    fn test_update_channel_response_time() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.update_channel_response_time("channel_1", 0.123, "test_service");
        
        let families = metrics.registry.gather();
        let response_time_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_channel_response_time_seconds")
            .unwrap();
        
        assert_eq!(response_time_metric.get_metric().len(), 1);
        assert_eq!(response_time_metric.get_metric()[0].get_gauge().get_value(), 0.123);
    }

    #[test]
    fn test_record_channel_error() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.record_channel_error("channel_1", "timeout", "test_service");
        metrics.record_channel_error("channel_1", "connection", "test_service");
        
        let families = metrics.registry.gather();
        let error_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_channel_errors_total")
            .unwrap();
        
        assert_eq!(error_metric.get_metric().len(), 2);
    }

    #[test]
    fn test_update_protocol_status() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.update_protocol_status("modbus_tcp", true, "test_service");
        metrics.update_protocol_status("iec104", false, "test_service");
        
        let families = metrics.registry.gather();
        let protocol_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_protocol_status")
            .unwrap();
        
        assert_eq!(protocol_metric.get_metric().len(), 2);
    }

    #[test]
    fn test_record_protocol_error() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.record_protocol_error("modbus_tcp", "parse_error", "test_service");
        
        let families = metrics.registry.gather();
        let error_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_protocol_errors_total")
            .unwrap();
        
        assert_eq!(error_metric.get_metric().len(), 1);
    }

    #[test]
    fn test_update_service_status() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.update_service_status(true);
        
        let families = metrics.registry.gather();
        let service_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_service_status")
            .unwrap();
        
        assert_eq!(service_metric.get_metric().len(), 1);
        assert_eq!(service_metric.get_metric()[0].get_gauge().get_value(), 1.0);
        
        metrics.update_service_status(false);
        let families = metrics.registry.gather();
        let service_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_service_status")
            .unwrap();
        assert_eq!(service_metric.get_metric()[0].get_gauge().get_value(), 0.0);
    }

    #[test]
    fn test_increment_uptime() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.increment_uptime(60.0);
        metrics.increment_uptime(30.0);
        
        let families = metrics.registry.gather();
        let uptime_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_service_uptime_seconds")
            .unwrap();
        
        assert_eq!(uptime_metric.get_metric().len(), 1);
        assert_eq!(uptime_metric.get_metric()[0].get_counter().get_value(), 90.0);
    }

    #[test]
    fn test_record_service_error() {
        let metrics = Metrics::new("test_service").unwrap();
        
        metrics.record_service_error("startup", "test_service");
        metrics.record_service_error("config", "test_service");
        
        let families = metrics.registry.gather();
        let error_metric = families.iter()
            .find(|f| f.get_name() == "comsrv_service_errors_total")
            .unwrap();
        
        assert_eq!(error_metric.get_metric().len(), 2);
    }

    #[test]
    fn test_label_cache() {
        let metrics = Metrics::new("test_service").unwrap();
        
        // Test that label cache contains expected entries
        assert!(metrics.label_cache.contains_key("send"));
        assert!(metrics.label_cache.contains_key("receive"));
        assert!(metrics.label_cache.contains_key("error_timeout"));
        assert!(metrics.label_cache.contains_key("protocol_modbus_tcp"));
        
        // Test get_labels function
        let labels = metrics.get_labels("send", vec!["fallback", "send"]);
        assert_eq!(labels, vec!["test_service", "send"]);
        
        let fallback_labels = metrics.get_labels("non_existent", vec!["test", "fallback"]);
        assert_eq!(fallback_labels, vec!["test", "fallback"]);
    }

    #[test]
    fn test_metrics_manager() {
        let manager = MetricsManager::new();
        
        // Initially no metrics
        assert!(manager.get().is_none());
        
        // Initialize metrics
        manager.init("test_service").unwrap();
        assert!(manager.get().is_some());
        
        // Test that we get the same instance
        let metrics1 = manager.get().unwrap();
        let metrics2 = manager.get().unwrap();
        // Verify both instances have the same registry by checking they gather the same metrics
        assert_eq!(metrics1.registry().gather().len(), metrics2.registry().gather().len());
    }

    #[test]
    fn test_global_metrics_functions() {
        // Test init_metrics
        init_metrics("global_test_service").unwrap();
        
        // Test get_metrics
        let metrics = get_metrics().unwrap();
        metrics.update_service_status(true);
        
        let families = metrics.registry.gather();
        assert!(!families.is_empty());
    }

    #[tokio::test]
    async fn test_metrics_server_start() {
        let metrics = Metrics::new("test_service").unwrap();
        
        // Test server start with invalid address
        let result = metrics.start_server("invalid_address").await;
        assert!(result.is_err());
        
        // Test server start with valid address (but we won't actually bind)
        // This test mainly checks that the parsing works correctly
        let addr_result: std::result::Result<SocketAddr, _> = "127.0.0.1:0".parse();
        assert!(addr_result.is_ok());
    }

    #[test]
    fn test_prometheus_error_conversion() {
        let prometheus_error = prometheus::Error::Msg("Test error".to_string());
        let comsrv_error: ComSrvError = prometheus_error.into();
        
        assert!(matches!(comsrv_error, ComSrvError::InternalError(_)));
        assert!(comsrv_error.to_string().contains("Test error"));
    }

    #[test]
    fn test_metrics_performance() {
        let metrics = Metrics::new("perf_test").unwrap();
        
        // Test that we can record many metrics quickly
        let start = std::time::Instant::now();
        
        for i in 0..1000 {
            metrics.record_bytes("send", i, "perf_test");
            metrics.record_packets("receive", i, "perf_test");
            metrics.update_channel_status(&format!("channel_{}", i % 10), i % 2 == 0, "perf_test");
        }
        
        let duration = start.elapsed();
        assert!(duration < Duration::from_millis(100)); // Should be very fast
        
        // Verify that metrics were recorded
        let families = metrics.registry.gather();
        assert!(!families.is_empty());
    }
} 