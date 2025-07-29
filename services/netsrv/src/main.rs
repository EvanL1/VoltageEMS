mod config;
mod error;

use crate::config::{load_config, Config};
use crate::error::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, value_parser, default_value = "config/netsrv.yml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config = match load_config() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    info!("Starting Network Service: {}", config.service.name);
    info!("Redis URL: {}", config.redis.url);
    info!("Networks configured: {}", config.networks.len());

    // TODO: Implement actual service logic
    info!("Service starting successfully - TODO: implement network forwarding logic");

    // Keep the service running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
