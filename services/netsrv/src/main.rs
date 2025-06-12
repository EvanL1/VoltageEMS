mod config;
mod error;
mod formatter;
mod network;
mod redis;

use crate::config::Config;
use crate::error::Result;
use crate::formatter::{create_formatter, default_formatter};
use crate::network::{create_client, create_cloud_client};
use crate::redis::RedisDataFetcher;
use clap::Parser;
use log::{debug, error, info, warn};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;


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
    info!("Found {} legacy network configurations", config.networks.len());
    if let Some(cloud_networks) = &config.cloud_networks {
        info!("Found {} cloud network configurations", cloud_networks.len());
    }

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

    // Create network clients with their formatters
    let mut clients = Vec::new();
    
    // Process legacy network configurations
    for network_config in &config.networks {
        if !network_config.enabled {
            info!("Network '{}' is disabled, skipping", network_config.name);
            continue;
        }

        info!("Initializing legacy network: {}", network_config.name);
        
        // Create formatter based on configuration
        let formatter = create_formatter(&network_config.format_type.clone().into());
        
        // Create client
        match create_client(network_config, formatter) {
            Ok(client) => {
                let client_name = network_config.name.clone();
                let client = Arc::new(tokio::sync::Mutex::new(client));
                clients.push((client_name, client));
            }
            Err(e) => {
                error!("Failed to create legacy client for network '{}': {}", network_config.name, e);
            }
        }
    }
    
    // Process cloud network configurations
    if let Some(cloud_networks) = &config.cloud_networks {
        for cloud_config in cloud_networks {
            if !cloud_config.enabled {
                info!("Cloud network '{}' is disabled, skipping", cloud_config.name);
                continue;
            }

            info!("Initializing cloud network: {} ({})", cloud_config.name, cloud_config.cloud_provider);
            
            // Validate cloud configuration
            if let Err(e) = cloud_config.validate() {
                error!("Invalid configuration for cloud network '{}': {}", cloud_config.name, e);
                continue;
            }
            
            // Create formatter (default to JSON for cloud networks, but can be configured later)
            let formatter = default_formatter();
            
            // Create unified MQTT client
            match create_cloud_client(cloud_config, formatter) {
                Ok(client) => {
                    let client_name = cloud_config.name.clone();
                    let client = Arc::new(tokio::sync::Mutex::new(client));
                    clients.push((client_name, client));
                }
                Err(e) => {
                    error!("Failed to create cloud client for network '{}': {}", cloud_config.name, e);
                }
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
        debug!("Received data from Redis: {:?}", data);
        
        for (name, client) in &clients {
            let client = client.lock().await;
            
            if !client.is_connected() {
                warn!("Client '{}' is not connected, skipping", name);
                continue;
            }
            
            // Format data using client's internal formatter
            let formatted_data = if let Some(mqtt_client) = client.as_any().downcast_ref::<crate::network::MqttClient>() {
                // For MQTT clients, use their internal formatter
                match mqtt_client.format_data(&data) {
                    Ok(formatted) => formatted,
                    Err(e) => {
                        error!("Failed to format data for network '{}': {}", name, e);
                        continue;
                    }
                }
            } else {
                // For other clients, use default JSON formatting
                match serde_json::to_string(&data) {
                    Ok(formatted) => formatted,
                    Err(e) => {
                        error!("Failed to format data for network '{}': {}", name, e);
                        continue;
                    }
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

 