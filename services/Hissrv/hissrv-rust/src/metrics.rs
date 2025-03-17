use lazy_static::lazy_static;
use prometheus::{
    Counter, Gauge, HistogramOpts, HistogramVec, IntCounter, IntGauge, Opts, Registry,
};
use std::convert::Infallible;
use warp::Filter;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    
    // Data processing metrics
    pub static ref POINTS_PROCESSED: IntCounter = IntCounter::new(
        "hissrv_points_processed_total",
        "Total number of data points processed"
    ).expect("metric can be created");
    
    pub static ref POINTS_STORED: IntCounter = IntCounter::new(
        "hissrv_points_stored_total",
        "Total number of data points stored in InfluxDB"
    ).expect("metric can be created");
    
    pub static ref STORAGE_ERRORS: IntCounter = IntCounter::new(
        "hissrv_storage_errors_total",
        "Total number of storage errors"
    ).expect("metric can be created");
    
    pub static ref PROCESSING_TIME: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "hissrv_processing_time_seconds",
            "Time spent processing data points"
        ),
        &["operation"]
    ).expect("metric can be created");
    
    pub static ref QUEUE_SIZE: IntGauge = IntGauge::new(
        "hissrv_queue_size",
        "Current size of the processing queue"
    ).expect("metric can be created");
}

pub fn register_metrics() {
    REGISTRY.register(Box::new(POINTS_PROCESSED.clone())).expect("collector can be registered");
    REGISTRY.register(Box::new(POINTS_STORED.clone())).expect("collector can be registered");
    REGISTRY.register(Box::new(STORAGE_ERRORS.clone())).expect("collector can be registered");
    REGISTRY.register(Box::new(PROCESSING_TIME.clone())).expect("collector can be registered");
    REGISTRY.register(Box::new(QUEUE_SIZE.clone())).expect("collector can be registered");
}

pub async fn metrics_handler() -> Result<impl warp::Reply, Infallible> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&REGISTRY.gather(), &mut buffer).unwrap();
    Ok(String::from_utf8(buffer).unwrap())
}

pub async fn serve_metrics() {
    let metrics_route = warp::path!("metrics")
        .and(warp::get())
        .and_then(metrics_handler);

    warp::serve(metrics_route)
        .run(([0, 0, 0, 0], 9100))
        .await;
} 