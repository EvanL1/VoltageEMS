use crate::api::start_api_server;
use crate::config::Config;
use crate::error::Result;
use crate::message_processor::MessageProcessor;
use crate::monitoring::MetricsCollector;
use crate::storage::influxdb_storage::InfluxDBStorage;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::Mutex;
use tracing::{error, info};
use voltage_common::redis::RedisClient;

mod config;
mod api;
mod batch_writer;
mod error;
mod influxdb_handler;
mod logging;
mod message_processor;
mod monitoring;
mod optimized_reader;
mod pubsub;
mod redis_handler;
mod storage;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::load().await?;
    
    // Initialize logging
    if let Err(e) = crate::logging::init_logging(&config.logging) {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }
    
    info!("Starting HisSrv v{}", config.service.version);
    info!("Configuration loaded from: {}", config.config_file);
    
    // Initialize Redis client
    let redis_url = if !config.redis.connection.socket.is_empty() {
        format!("unix://{}", config.redis.connection.socket)
    } else {
        format!(
            "redis://{}:{}/{}",
            config.redis.connection.host,
            config.redis.connection.port,
            config.redis.connection.database
        )
    };
    
    let redis_client = RedisClient::new(&redis_url).await?;
    info!("Connected to Redis");
    
    // Initialize InfluxDB storage
    let mut influxdb_storage = InfluxDBStorage::new(config.storage.backends.influxdb.clone());
    influxdb_storage.connect().await?;
    let influxdb_storage = Arc::new(Mutex::new(influxdb_storage));
    info!("Connected to InfluxDB");
    
    // Initialize message processor with batch writer
    let mut message_processor = MessageProcessor::new(config.clone(), redis_client).await?;
    message_processor.init_storage(influxdb_storage.clone()).await?;
    
    // Initialize metrics collector
    let _metrics_collector = MetricsCollector::new();
    
    // Start message processing
    let processor = Arc::new(message_processor);
    let processor_handle = {
        let processor = processor.clone();
        tokio::spawn(async move {
            if let Err(e) = processor.start().await {
                error!("Message processor error: {}", e);
            }
        })
    };
    
    // Start API server if enabled
    let api_handle = if config.api.enabled {
        let api_config = config.clone();
        let api_processor = processor.clone();
        
        Some(tokio::spawn(async move {
            // Create a simple API handler that can access stats
            let app = axum::Router::new()
                .route("/health", axum::routing::get(health_check))
                .route("/stats", axum::routing::get({
                    let processor = api_processor.clone();
                    move || async move {
                        let stats = processor.get_stats().await;
                        axum::Json(stats)
                    }
                }))
                .route("/metrics", axum::routing::get({
                    let processor = api_processor.clone();
                    move || async move {
                        if let Some(stats) = processor.get_stats().await {
                            let mut metrics = String::new();
                            metrics.push_str(&format!("# HELP hissrv_points_received Total points received\n"));
                            metrics.push_str(&format!("# TYPE hissrv_points_received counter\n"));
                            metrics.push_str(&format!("hissrv_points_received {}\n", stats.total_points_received));
                            
                            metrics.push_str(&format!("# HELP hissrv_points_written Total points written\n"));
                            metrics.push_str(&format!("# TYPE hissrv_points_written counter\n"));
                            metrics.push_str(&format!("hissrv_points_written {}\n", stats.total_points_written));
                            
                            metrics.push_str(&format!("# HELP hissrv_points_failed Total points failed\n"));
                            metrics.push_str(&format!("# TYPE hissrv_points_failed counter\n"));
                            metrics.push_str(&format!("hissrv_points_failed {}\n", stats.total_points_failed));
                            
                            metrics.push_str(&format!("# HELP hissrv_write_success_rate Write success rate\n"));
                            metrics.push_str(&format!("# TYPE hissrv_write_success_rate gauge\n"));
                            metrics.push_str(&format!("hissrv_write_success_rate {}\n", stats.success_rate()));
                            
                            metrics.push_str(&format!("# HELP hissrv_average_batch_size Average batch size\n"));
                            metrics.push_str(&format!("# TYPE hissrv_average_batch_size gauge\n"));
                            metrics.push_str(&format!("hissrv_average_batch_size {}\n", stats.average_batch_size));
                            
                            metrics.push_str(&format!("# HELP hissrv_write_latency_ms Write latency in milliseconds\n"));
                            metrics.push_str(&format!("# TYPE hissrv_write_latency_ms gauge\n"));
                            metrics.push_str(&format!("hissrv_write_latency_ms {}\n", stats.write_latency_ms));
                            
                            metrics
                        } else {
                            String::from("# No metrics available\n")
                        }
                    }
                }));
            
            let addr = format!("{}:{}", api_config.service.host, api_config.service.port);
            info!("API server listening on {}", addr);
            
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .expect("Failed to bind to address");
            
            axum::serve(listener, app)
                .await
                .unwrap();
        }))
    } else {
        info!("API server is disabled");
        None
    };
    
    info!("HisSrv started successfully");
    info!("Monitoring channels: {:?}", config.redis.subscription.channel_ids);
    info!("Batch size: {}, Flush interval: {}s", 
        config.storage.backends.influxdb.batch_size,
        config.storage.backends.influxdb.flush_interval
    );
    
    // Wait for shutdown signal
    shutdown_signal().await;
    info!("Shutdown signal received");
    
    // Graceful shutdown
    processor.shutdown().await?;
    
    // Wait for tasks to complete
    processor_handle.abort();
    if let Some(api_handle) = api_handle {
        api_handle.abort();
    }
    
    info!("HisSrv shutdown complete");
    Ok(())
}

async fn health_check() -> impl axum::response::IntoResponse {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "service": "hissrv",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}