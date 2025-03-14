mod config;
mod error;
mod formatter;
mod network;
mod redis;

use crate::config::Config;
use crate::error::Result;
use crate::formatter::create_formatter;
use crate::network::{create_client, NetworkClient};
use crate::redis::RedisDataFetcher;
use clap::Parser;
use log::{debug, error, info, warn};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, value_parser, default_value = "netsrv.json")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config = match Config::new(args.config.to_str().unwrap_or("netsrv.json")) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            eprintln!("Using default configuration");
            Config::default()
        }
    };

    // Initialize logging
    init_logging(&config);

    info!("Starting Network Service");
    info!("Redis configuration: {}:{}", config.redis.host, config.redis.port);
    info!("Found {} network configurations", config.networks.len());

    // Create data channel
    let (tx, mut rx) = mpsc::channel::<Value>(100);

    // Start Redis data fetcher
    let redis_config = config.redis.clone();
    let mut data_fetcher = RedisDataFetcher::new(redis_config);
    
    tokio::spawn(async move {
        if let Err(e) = data_fetcher.start_polling(tx).await {
            error!("Redis data fetcher error: {}", e);
        }
    });

    // Create network clients
    let mut clients = Vec::new();
    
    for network_config in &config.networks {
        if !network_config.enabled {
            info!("Network '{}' is disabled, skipping", network_config.name);
            continue;
        }

        info!("Initializing network: {}", network_config.name);
        
        // Create formatter
        let formatter = create_formatter(&network_config.format_type);
        
        // Create client
        match create_client(network_config, formatter) {
            Ok(client) => {
                let client_name = network_config.name.clone();
                let client = Arc::new(tokio::sync::Mutex::new(client));
                clients.push((client_name, client));
            }
            Err(e) => {
                error!("Failed to create client for network '{}': {}", network_config.name, e);
            }
        }
    }

    // Connect all clients
    for (name, client) in &clients {
        let mut client = client.lock().await;
        match client.connect().await {
            Ok(_) => info!("Connected to network: {}", name),
            Err(e) => error!("Failed to connect to network '{}': {}", name, e),
        }
    }

    // Main loop: Receive data and send to all networks
    while let Some(data) = rx.recv().await {
        debug!("Received data from Redis");
        
        for (name, client) in &clients {
            let client = client.lock().await;
            
            if !client.is_connected() {
                warn!("Client '{}' is not connected, skipping", name);
                continue;
            }
            
            // Format data
            let formatted_data = match client.format_data(&data) {
                Ok(formatted) => formatted,
                Err(e) => {
                    error!("Failed to format data for network '{}': {}", name, e);
                    continue;
                }
            };
            
            // Send data
            match client.send(&formatted_data).await {
                Ok(_) => debug!("Data sent to network: {}", name),
                Err(e) => error!("Failed to send data to network '{}': {}", name, e),
            }
        }
    }

    // Disconnect all clients
    for (name, client) in &clients {
        let mut client = client.lock().await;
        if let Err(e) = client.disconnect().await {
            error!("Error disconnecting from network '{}': {}", name, e);
        }
    }

    info!("Network Service stopped");
    Ok(())
}

fn init_logging(config: &Config) {
    use env_logger::{Builder, Env};
    
    let env = Env::default().filter_or("RUST_LOG", &config.logging.level);
    let mut builder = Builder::from_env(env);
    
    builder.format_timestamp_millis();
    
    if config.logging.console {
        builder.init();
    }
    
    info!("Logging initialized at level: {}", config.logging.level);
}

// Extend NetworkClient trait to add formatting method
trait NetworkClientExt: NetworkClient {
    fn format_data(&self, data: &Value) -> Result<String>;
}

// Implement NetworkClientExt for all NetworkClient
impl<T: NetworkClient> NetworkClientExt for T {
    fn format_data(&self, data: &Value) -> Result<String> {
        // This should use the client's internal formatter, but since we can't directly access it,
        // this is just an example implementation, and it should be handled in each client implementation
        serde_json::to_string(data).map_err(|e| {
            crate::error::NetSrvError::FormatError(format!("JSON formatting error: {}", e))
        })
    }
} 