mod config;
mod error;
mod model;
mod redis_handler;
mod comsrv_handler;
mod control;

use crate::config::Config;
use crate::error::Result;
use crate::model::ModelEngine;
use crate::redis_handler::RedisConnection;
use crate::comsrv_handler::ComsrvHandler;
use crate::control::ControlManager;
use clap::Parser;
use log::{error, info};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, value_parser, default_value = "modsrv.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config = match Config::new(args.config.to_str().unwrap_or("modsrv.toml")) {
        Ok(config) => config,
        Err(_) => {
            println!("Failed to load configuration, using default");
            Config::default()
        }
    };

    // Initialize logging
    init_logging(&config);

    info!("Starting Model Service");

    // Initialize Redis connection
    let mut redis = RedisConnection::new();
    if let Err(e) = redis.connect(&config) {
        error!("Failed to connect to Redis: {}", e);
        return Err(e);
    }

    // Initialize model engine
    let mut model_engine = ModelEngine::new();

    // Initialize control manager
    let mut control_manager = ControlManager::new(&config.redis.prefix);

    // Initialize Comsrv handler
    let comsrv_handler = ComsrvHandler::new(&config.redis.prefix);

    // Main service loop
    let update_interval = Duration::from_millis(config.model.update_interval_ms);
    let mut interval = time::interval(update_interval);

    loop {
        interval.tick().await;

        // Load model configurations
        if let Err(e) = model_engine.load_models(&mut redis, &config.model.config_key_pattern) {
            error!("Failed to load models: {}", e);
            continue;
        }

        // Load control operations if enabled
        if config.control.enabled {
            if let Err(e) = control_manager.load_operations(&mut redis, &config.control.operation_key_pattern) {
                error!("Failed to load control operations: {}", e);
            }
        }

        // Execute models
        if let Err(e) = model_engine.execute_models(&mut redis) {
            error!("Failed to execute models: {}", e);
        }

        // Check and execute control operations if enabled
        if config.control.enabled {
            if let Err(e) = control_manager.check_and_execute_operations(&mut redis) {
                error!("Failed to execute control operations: {}", e);
            }
        }
    }
}

fn init_logging(config: &Config) {
    // Initialize logging based on configuration
    let env = env_logger::Env::default()
        .filter_or("RUST_LOG", &config.logging.level);
    
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .init();
    
    info!("Logging initialized at level: {}", config.logging.level);
} 