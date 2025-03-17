use std::path::Path;
use std::process;
use std::time::Duration;

use clap::Parser;
use log::{error, info};
use tokio::sync::mpsc;
use tokio::time;

use crate::config::Config;
use crate::error::Result;
use crate::influxdb_handler::InfluxDBHandler;
use crate::redis_handler::{RedisClientTrait, RedisHandler};
use crate::metrics::{register_metrics, serve_metrics};

mod config;
mod error;
mod influxdb_handler;
mod redis_handler;
mod metrics;

/// Command line arguments
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to configuration file
    #[clap(short, long, default_value = "config.yaml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();
    
    // Initialize metrics
    register_metrics();
    
    // Start metrics server in a separate task
    tokio::spawn(serve_metrics());
    
    // Parse command line arguments
    let args = Args::parse();
    
    // Load configuration
    let config_path = Path::new(&args.config);
    let config = match Config::from_file(config_path) {
        Ok(config) => {
            info!("Configuration loaded from {}", args.config);
            config
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };
    
    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Invalid configuration: {}", e);
        process::exit(1);
    }
    
    // Create channel for data transfer
    let (tx, rx) = mpsc::channel(100);
    
    // Initialize Redis handler
    let mut redis_handler = RedisHandler::from_config(&config);
    
    // Connect to Redis
    if let Err(e) = redis_handler.connect() {
        error!("Failed to connect to Redis: {}", e);
        process::exit(1);
    }
    
    info!("Connected to Redis");
    
    // Initialize InfluxDB handler
    let mut influxdb_handler = InfluxDBHandler::from_config(&config);
    
    // Connect to InfluxDB
    if let Err(e) = influxdb_handler.connect() {
        error!("Failed to connect to InfluxDB: {}", e);
        process::exit(1);
    }
    
    info!("Connected to InfluxDB");
    
    // Start polling Redis
    let polling_interval = Duration::from_secs(config.redis.polling_interval_seconds);
    if let Err(e) = redis_handler.start_polling(polling_interval, tx) {
        error!("Failed to start Redis polling: {}", e);
        process::exit(1);
    }
    
    info!("Started Redis polling with interval of {} seconds", config.redis.polling_interval_seconds);
    
    // Start processing data
    let flush_interval = Duration::from_secs(config.influxdb.flush_interval_seconds);
    if let Err(e) = influxdb_handler.start_processing(rx, flush_interval) {
        error!("Failed to start InfluxDB processing: {}", e);
        process::exit(1);
    }
    
    info!("Started InfluxDB processing with flush interval of {} seconds", config.influxdb.flush_interval_seconds);
    
    // Keep the main thread alive
    // In a real application, we would use proper signal handling for graceful shutdown
    loop {
        time::sleep(Duration::from_secs(1)).await;
    }
} 