mod config_api;
mod config_new;
mod error;
mod formatter;
mod network;
mod redis;

use crate::config_api::{create_config_router, ConfigState};
use crate::config_new::NetServiceConfig;
use crate::error::Result;
use crate::formatter::{create_formatter, FormatType};
use crate::network::{create_network_client, NetworkClient};
use crate::redis::NewRedisDataFetcher;
use clap::Parser;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, warn};
use voltage_config::load_config;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, value_parser, default_value = "config/netsrv.yml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config = match load_config::<NetServiceConfig>(&args.config) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            eprintln!("Using default configuration");
            NetServiceConfig::default()
        }
    };

    // Initialize logging
    init_logging(&config);

    info!("Starting Network Service with Backend Configuration Management");
    info!("Redis configuration: {}", config.base.redis.url);
    info!("Found {} network configurations", config.networks.len());

    // Create configuration state for API
    let config_state = ConfigState::new(config.clone(), args.config.clone());

    // Start configuration management API server
    let config_api_port = config.base.monitoring.health_check_port + 1; // Use next port after health check
    let config_router = create_config_router(config_state.clone()).layer(CorsLayer::permissive());

    let config_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config_api_port))
        .await
        .expect("Failed to bind configuration API server");

    info!(
        "Configuration API server starting on port {}",
        config_api_port
    );

    tokio::spawn(async move {
        if let Err(e) = axum::serve(config_listener, config_router).await {
            error!("Configuration API server error: {}", e);
        }
    });

    // Create data channel
    let (tx, mut rx) = mpsc::channel::<Value>(100);

    // Start Redis data fetcher with new configuration
    let mut data_fetcher = NewRedisDataFetcher::new(
        config.base.redis.clone(),
        config.data.redis_data_key.clone(),
        config.data.redis_polling_interval_secs,
    )?;

    tokio::spawn(async move {
        if let Err(e) = data_fetcher.start_polling(tx).await {
            error!("Redis data fetcher error: {}", e);
        }
    });

    // Create network clients using new configuration system
    let mut clients = Vec::new();

    // Process all network configurations
    for network_config in &config.networks {
        let network_name = match network_config {
            crate::config_new::NetworkConfig::LegacyMqtt(mqtt_config) => &mqtt_config.name,
            crate::config_new::NetworkConfig::Http(http_config) => &http_config.name,
            crate::config_new::NetworkConfig::CloudMqtt(cloud_config) => &cloud_config.name,
        };

        info!("Initializing network: {}", network_name);

        // Create formatter based on configuration
        let formatter = match network_config {
            crate::config_new::NetworkConfig::LegacyMqtt(mqtt_config) => {
                create_formatter(&convert_format_type(&mqtt_config.format_type))
            }
            crate::config_new::NetworkConfig::Http(http_config) => {
                create_formatter(&convert_format_type(&http_config.format_type))
            }
            crate::config_new::NetworkConfig::CloudMqtt(cloud_config) => {
                create_formatter(&convert_format_type(&cloud_config.format_type))
            }
        };

        // Create client using new factory
        match create_network_client(network_config, formatter) {
            Ok(client) => {
                let client_name = network_name.to_string();
                let client = Arc::new(tokio::sync::Mutex::new(client));
                clients.push((client_name, client));
            }
            Err(e) => {
                error!(
                    "Failed to create client for network '{}': {}",
                    network_name, e
                );
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

            // Format data - the formatter is embedded in the new client
            let formatted_data = match serde_json::to_string(&data) {
                Ok(formatted) => formatted,
                Err(e) => {
                    error!("Failed to format data for network '{}': {}", name, e);
                    continue;
                }
            };

            // Send data using new client interface
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

fn init_logging(config: &NetServiceConfig) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.base.logging.level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    info!(
        "Logging initialized at level: {}",
        config.base.logging.level
    );
}

fn convert_format_type(format_type: &crate::config_new::FormatType) -> FormatType {
    match format_type {
        crate::config_new::FormatType::Json => FormatType::Json,
        crate::config_new::FormatType::Ascii => FormatType::Ascii,
        crate::config_new::FormatType::Binary => FormatType::Json, // Fallback to JSON
        crate::config_new::FormatType::Protobuf => FormatType::Json, // Fallback to JSON
    }
}
