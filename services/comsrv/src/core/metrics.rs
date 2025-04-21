use prometheus::{
    Counter, Encoder, Gauge, GaugeVec, HistogramVec,
    IntCounterVec, Registry, TextEncoder,
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::task;
use std::ops::Deref;
use hyper::{Body, Response, Server, Request};
use hyper::service::{make_service_fn, service_fn};

use crate::utils::{ComSrvError, Result};

/// Metrics manager for the communication service
#[derive(Clone)]
pub struct Metrics {
    registry: Arc<Registry>,
    // Communication metrics
    bytes_total: Arc<IntCounterVec>,
    packets_total: Arc<IntCounterVec>,
    packet_errors_total: Arc<IntCounterVec>,
    packet_processing_duration: Arc<HistogramVec>,

    // Channel metrics
    channel_status: Arc<GaugeVec>,
    channel_response_time: Arc<GaugeVec>,
    channel_errors_total: Arc<IntCounterVec>,

    // Protocol metrics
    protocol_status: Arc<GaugeVec>,
    protocol_errors_total: Arc<IntCounterVec>,

    // Service metrics
    service_status: Arc<Gauge>,
    service_uptime: Arc<Counter>,
    service_errors_total: Arc<IntCounterVec>,
}

impl Metrics {
    /// Create a new Metrics instance with all metrics registered
    pub fn new(_service_name: &str) -> Self {
        let registry = Arc::new(Registry::new());

        // Create metrics with service label
        let bytes_total = Arc::new(
            IntCounterVec::new(
                prometheus::opts!("comsrv_bytes_total", "Total number of bytes sent/received"),
                &["service", "direction"],
            )
            .expect("Failed to create bytes_total metric"),
        );

        let packets_total = Arc::new(
            IntCounterVec::new(
                prometheus::opts!("comsrv_packets_total", "Total number of packets sent/received"),
                &["service", "direction"],
            )
            .expect("Failed to create packets_total metric"),
        );

        let packet_errors_total = Arc::new(
            IntCounterVec::new(
                prometheus::opts!("comsrv_packet_errors_total", "Total number of packet errors"),
                &["service", "type"],
            )
            .expect("Failed to create packet_errors_total metric"),
        );

        let packet_processing_duration = Arc::new(
            HistogramVec::new(
                prometheus::histogram_opts!(
                    "comsrv_packet_processing_duration_seconds",
                    "Packet processing duration in seconds",
                    vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
                ),
                &["service", "protocol"],
            )
            .expect("Failed to create packet_processing_duration metric"),
        );

        let channel_status = Arc::new(
            GaugeVec::new(
                prometheus::opts!("comsrv_channel_status", "Channel connection status (1=connected, 0=disconnected)"),
                &["service", "channel"],
            )
            .expect("Failed to create channel_status metric"),
        );

        let channel_response_time = Arc::new(
            GaugeVec::new(
                prometheus::opts!("comsrv_channel_response_time_seconds", "Channel response time in seconds"),
                &["service", "channel"],
            )
            .expect("Failed to create channel_response_time metric"),
        );

        let channel_errors_total = Arc::new(
            IntCounterVec::new(
                prometheus::opts!("comsrv_channel_errors_total", "Total number of channel errors"),
                &["service", "channel", "type"],
            )
            .expect("Failed to create channel_errors_total metric"),
        );

        let protocol_status = Arc::new(
            GaugeVec::new(
                prometheus::opts!("comsrv_protocol_status", "Protocol status (1=active, 0=inactive)"),
                &["service", "protocol"],
            )
            .expect("Failed to create protocol_status metric"),
        );

        let protocol_errors_total = Arc::new(
            IntCounterVec::new(
                prometheus::opts!("comsrv_protocol_errors_total", "Total number of protocol errors"),
                &["service", "protocol", "type"],
            )
            .expect("Failed to create protocol_errors_total metric"),
        );

        let service_status = Arc::new(
            Gauge::new("comsrv_service_status", "Service status (1=running, 0=stopped)")
                .expect("Failed to create service_status metric"),
        );

        let service_uptime = Arc::new(
            Counter::new("comsrv_service_uptime_seconds", "Service uptime in seconds")
                .expect("Failed to create service_uptime metric"),
        );

        let service_errors_total = Arc::new(
            IntCounterVec::new(
                prometheus::opts!("comsrv_service_errors_total", "Total number of service errors"),
                &["service", "type"],
            )
            .expect("Failed to create service_errors_total metric"),
        );

        // Register all metrics
        registry.register(Box::new(bytes_total.clone().deref().clone())).expect("Failed to register bytes_total");
        registry.register(Box::new(packets_total.clone().deref().clone())).expect("Failed to register packets_total");
        registry.register(Box::new(packet_errors_total.clone().deref().clone())).expect("Failed to register packet_errors_total");
        registry.register(Box::new(packet_processing_duration.clone().deref().clone())).expect("Failed to register packet_processing_duration");
        registry.register(Box::new(channel_status.clone().deref().clone())).expect("Failed to register channel_status");
        registry.register(Box::new(channel_response_time.clone().deref().clone())).expect("Failed to register channel_response_time");
        registry.register(Box::new(channel_errors_total.clone().deref().clone())).expect("Failed to register channel_errors_total");
        registry.register(Box::new(protocol_status.clone().deref().clone())).expect("Failed to register protocol_status");
        registry.register(Box::new(protocol_errors_total.clone().deref().clone())).expect("Failed to register protocol_errors_total");
        registry.register(Box::new(service_status.clone().deref().clone())).expect("Failed to register service_status");
        registry.register(Box::new(service_uptime.clone().deref().clone())).expect("Failed to register service_uptime");
        registry.register(Box::new(service_errors_total.clone().deref().clone())).expect("Failed to register service_errors_total");

        Metrics {
            registry,
            bytes_total,
            packets_total,
            packet_errors_total,
            packet_processing_duration,
            channel_status,
            channel_response_time,
            channel_errors_total,
            protocol_status,
            protocol_errors_total,
            service_status,
            service_uptime,
            service_errors_total,
        }
    }

    /// Start the metrics server on the given address
    pub async fn start_server(&self, addr: &str) -> Result<()> {
        let metrics = self.clone();
        let addr: SocketAddr = addr.parse().map_err(|e| {
            ComSrvError::ConfigError(format!("Invalid metrics bind address: {}, error: {}", addr, e))
        })?;

        // Spawn a task to run the server
        task::spawn(async move {
            async fn serve_metrics(_req: Request<Body>, metrics: Metrics) -> hyper::Result<Response<Body>> {
                let encoder = TextEncoder::new();
                let mut buffer = vec![];
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

        tracing::info!("Metrics server started on http://{}/metrics", addr);
        Ok(())
    }

    /// Record bytes sent/received
    pub fn record_bytes(&self, direction: &str, count: u64, service_name: &str) {
        self.bytes_total
            .with_label_values(&[service_name, direction])
            .inc_by(count);
    }

    /// Record packets sent/received
    pub fn record_packets(&self, direction: &str, count: u64, service_name: &str) {
        self.packets_total
            .with_label_values(&[service_name, direction])
            .inc_by(count);
    }

    /// Record packet errors
    pub fn record_packet_error(&self, error_type: &str, service_name: &str) {
        self.packet_errors_total
            .with_label_values(&[service_name, error_type])
            .inc();
    }

    /// Record packet processing duration
    pub fn record_packet_processing(&self, protocol: &str, duration: f64, service_name: &str) {
        self.packet_processing_duration
            .with_label_values(&[service_name, protocol])
            .observe(duration);
    }

    /// Update channel status
    pub fn update_channel_status(&self, channel: &str, connected: bool, service_name: &str) {
        let value = if connected { 1.0 } else { 0.0 };
        self.channel_status
            .with_label_values(&[service_name, channel])
            .set(value);
    }

    /// Update channel response time
    pub fn update_channel_response_time(&self, channel: &str, time: f64, service_name: &str) {
        self.channel_response_time
            .with_label_values(&[service_name, channel])
            .set(time);
    }

    /// Record channel error
    pub fn record_channel_error(&self, channel: &str, error_type: &str, service_name: &str) {
        self.channel_errors_total
            .with_label_values(&[service_name, channel, error_type])
            .inc();
    }

    /// Update protocol status
    pub fn update_protocol_status(&self, protocol: &str, active: bool, service_name: &str) {
        let value = if active { 1.0 } else { 0.0 };
        self.protocol_status
            .with_label_values(&[service_name, protocol])
            .set(value);
    }

    /// Record protocol error
    pub fn record_protocol_error(&self, protocol: &str, error_type: &str, service_name: &str) {
        self.protocol_errors_total
            .with_label_values(&[service_name, protocol, error_type])
            .inc();
    }

    /// Update service status
    pub fn update_service_status(&self, running: bool) {
        let value = if running { 1.0 } else { 0.0 };
        self.service_status.set(value);
    }

    /// Increment service uptime
    pub fn increment_uptime(&self, seconds: f64) {
        self.service_uptime.inc_by(seconds);
    }

    /// Record service error
    pub fn record_service_error(&self, error_type: &str, service_name: &str) {
        self.service_errors_total
            .with_label_values(&[service_name, error_type])
            .inc();
    }
}

// Global metrics instance
pub struct MetricsManager {
    instance: Arc<Mutex<Option<Metrics>>>,
}

impl MetricsManager {
    /// Create a new metrics manager
    pub fn new() -> Self {
        Self {
            instance: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize the metrics manager with a service name
    pub fn init(&self, service_name: &str) {
        let metrics = Metrics::new(service_name);
        let mut instance = self.instance.lock().unwrap();
        *instance = Some(metrics);
    }

    /// Get the metrics instance
    pub fn get(&self) -> Option<Metrics> {
        let instance = self.instance.lock().unwrap();
        instance.clone()
    }
}

// Global metrics instance
lazy_static::lazy_static! {
    static ref METRICS_MANAGER: MetricsManager = MetricsManager::new();
}

/// Get the global metrics manager
pub fn metrics_manager() -> &'static MetricsManager {
    &METRICS_MANAGER
}

/// Initialize the global metrics manager
pub fn init_metrics(service_name: &str) {
    metrics_manager().init(service_name);
}

/// Get the global metrics instance
pub fn get_metrics() -> Option<Metrics> {
    metrics_manager().get()
} 