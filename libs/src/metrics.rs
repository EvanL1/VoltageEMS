//! Common metrics utilities for VoltageEMS services

use once_cell::sync::Lazy;
use prometheus::{
    register_gauge_vec, register_histogram_vec, register_int_counter_vec, register_int_gauge_vec,
    Encoder, GaugeVec, HistogramVec, IntCounterVec, IntGaugeVec, TextEncoder,
};
use std::collections::HashMap;

/// Common HTTP request metrics
pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "http_requests_total",
        "Total number of HTTP requests",
        &["service", "method", "endpoint", "status"]
    )
    .expect("Failed to register http_requests_total metric")
});

/// Common HTTP request duration metrics
pub static HTTP_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "http_request_duration_seconds",
        "HTTP request duration in seconds",
        &["service", "method", "endpoint"]
    )
    .expect("Failed to register http_request_duration_seconds metric")
});

/// Active connections gauge
pub static ACTIVE_CONNECTIONS: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "active_connections",
        "Number of active connections",
        &["service", "type"]
    )
    .expect("Failed to register active_connections metric")
});

/// Message processing metrics
pub static MESSAGES_PROCESSED: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "messages_processed_total",
        "Total number of messages processed",
        &["service", "type", "status"]
    )
    .expect("Failed to register messages_processed_total metric")
});

/// Data points metrics
pub static DATA_POINTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "data_points_total",
        "Total number of data points processed",
        &["service", "point_type", "operation"]
    )
    .expect("Failed to register data_points_total metric")
});

/// Service uptime gauge
pub static SERVICE_UPTIME: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "service_uptime_seconds",
        "Service uptime in seconds",
        &["service"]
    )
    .expect("Failed to register service_uptime_seconds metric")
});

/// Error counter
pub static ERRORS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "errors_total",
        "Total number of errors",
        &["service", "error_type"]
    )
    .expect("Failed to register errors_total metric")
});

/// Memory usage gauge
pub static MEMORY_USAGE: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "memory_usage_bytes",
        "Memory usage in bytes",
        &["service", "type"]
    )
    .expect("Failed to register memory_usage_bytes metric")
});

/// Protocol-specific metrics
pub static PROTOCOL_OPERATIONS: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "protocol_operations_total",
        "Total number of protocol operations",
        &["service", "protocol", "operation", "status"]
    )
    .expect("Failed to register protocol_operations_total metric")
});

/// Storage operations metrics
pub static STORAGE_OPERATIONS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "storage_operation_duration_seconds",
        "Storage operation duration in seconds",
        &["service", "operation", "storage_type"]
    )
    .expect("Failed to register storage_operation_duration_seconds metric")
});

/// Helper to create service-specific labels
pub fn service_labels(service_name: &str) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    labels.insert("service".to_string(), service_name.to_string());
    labels
}

/// Record an HTTP request
pub fn record_http_request(
    service: &str,
    method: &str,
    endpoint: &str,
    status: u16,
    duration: f64,
) {
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[service, method, endpoint, &status.to_string()])
        .inc();

    HTTP_REQUEST_DURATION
        .with_label_values(&[service, method, endpoint])
        .observe(duration);
}

/// Record a protocol operation
pub fn record_protocol_operation(service: &str, protocol: &str, operation: &str, success: bool) {
    let status = if success { "success" } else { "failure" };
    PROTOCOL_OPERATIONS
        .with_label_values(&[service, protocol, operation, status])
        .inc();
}

/// Record an error
pub fn record_error(service: &str, error_type: &str) {
    ERRORS_TOTAL.with_label_values(&[service, error_type]).inc();
}

/// Update active connections
pub fn update_active_connections(service: &str, conn_type: &str, delta: i64) {
    if delta > 0 {
        ACTIVE_CONNECTIONS
            .with_label_values(&[service, conn_type])
            .add(delta);
    } else {
        ACTIVE_CONNECTIONS
            .with_label_values(&[service, conn_type])
            .sub(-delta);
    }
}

/// Get metrics in Prometheus text format
pub fn get_metrics_text() -> Result<String, Box<dyn std::error::Error>> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}

/// Initialize common metrics for a service
pub fn init_service_metrics(service_name: &str) {
    // Initialize uptime metric
    SERVICE_UPTIME.with_label_values(&[service_name]).set(0.0);

    // Initialize memory usage metrics
    MEMORY_USAGE
        .with_label_values(&[service_name, "heap"])
        .set(0);
    MEMORY_USAGE
        .with_label_values(&[service_name, "stack"])
        .set(0);
}

/// Update service uptime
pub fn update_service_uptime(service_name: &str, uptime_seconds: f64) {
    SERVICE_UPTIME
        .with_label_values(&[service_name])
        .set(uptime_seconds);
}

/// Update memory usage
pub fn update_memory_usage(service_name: &str, heap_bytes: i64, stack_bytes: i64) {
    MEMORY_USAGE
        .with_label_values(&[service_name, "heap"])
        .set(heap_bytes);
    MEMORY_USAGE
        .with_label_values(&[service_name, "stack"])
        .set(stack_bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_http_request() {
        record_http_request("test_service", "GET", "/api/test", 200, 0.1);

        // Verify the metric was recorded
        let metrics = get_metrics_text().unwrap();
        assert!(metrics.contains("http_requests_total"));
        assert!(metrics.contains("test_service"));
    }

    #[test]
    fn test_record_error() {
        record_error("test_service", "connection_timeout");

        let metrics = get_metrics_text().unwrap();
        assert!(metrics.contains("errors_total"));
        assert!(metrics.contains("connection_timeout"));
    }

    #[test]
    fn test_active_connections() {
        update_active_connections("test_service", "tcp", 5);
        update_active_connections("test_service", "tcp", -2);

        let metrics = get_metrics_text().unwrap();
        assert!(metrics.contains("active_connections"));
    }
}
