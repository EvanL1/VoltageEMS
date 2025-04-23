use std::sync::Arc;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use chrono::Utc;
use dotenv::dotenv;
use tracing;
use tokio::sync::RwLock;

use tracing::{info, error, warn};

mod core;
mod utils;
mod api;

use crate::utils::logger;
use crate::utils::error::Result;
use crate::core::config::ConfigManager;
use crate::core::protocol_factory::{protocol_factory, init_protocol_factory, ProtocolFactory};
use crate::api::routes::api_routes;
use crate::core::protocols::modbus::client::ModbusClientFactory;
use crate::utils::ComSrvError;
use crate::core::storage::redisStorage::RedisStore;
use crate::utils::logger::init_logger;

async fn start_communication_service(
    config_manager: Arc<ConfigManager>,
    factory: Arc<RwLock<ProtocolFactory>>
) -> Result<()> {
    // Get channel configurations
    let configs = config_manager.get_channels().clone();
    
    // Create channels
    for channel_config in configs {
        tracing::info!("Creating channel: {} - {}", channel_config.id, channel_config.name);
        let mut factory_write = factory.write().await;
        match factory_write.create_channel(channel_config.clone()) {
            Ok(_) => {
                tracing::info!("Channel created successfully: {}", channel_config.id);
            },
            Err(e) => {
                tracing::error!("Failed to create channel {}: {}", channel_config.id, e);
            }
        }
    }
    
    // Start all channels
    let mut factory_write = factory.write().await;
    factory_write.start_all_channels().await?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment variables
    if let Err(e) = dotenv() {
        eprintln!("Warning: Failed to load .env file: {}", e);
    }
    
    // Initialize configuration
    let config_file = std::env::var("CONFIG_FILE").unwrap_or_else(|_| "config/comsrv.yaml".to_string());
    
    // Create configuration manager
    let config_manager = match ConfigManager::from_file(&config_file) {
        Ok(cm) => {
            info!("Configuration loaded from: {}", config_file);
            Arc::new(cm)
        },
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };
    
    // Initialize logging
    let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());
    init_logger(Path::new(&log_dir), "comsrv", config_manager.get_log_level(), true)?;
    
    // Initialize Factory and wrap it as a thread-safe shared reference
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
    
    // Start communication service, passing shared factory reference
    if let Err(e) = start_communication_service(config_manager.clone(), factory.clone()).await {
        error!("Failed to start communication service: {}", e);
        return Err(e);
    }
    
    // API service uses the same factory instance
    if config_manager.get_api_enabled() {
        // Create API routes, directly using shared factory reference
        let start_time = Arc::new(Utc::now());
        let routes = api_routes(factory.clone(), start_time);
        
        info!("Starting API server at: {}", config_manager.get_api_address());
        
        let socket_addr = config_manager.get_api_address().parse::<SocketAddr>().unwrap_or_else(|_| {
            warn!("Invalid API address: {}, using default 0.0.0.0:3000", config_manager.get_api_address());
            "0.0.0.0:3000".parse().unwrap()
        });
        
        tokio::spawn(async move {
            warp::serve(routes).run(socket_addr).await;
        });
    }
    
    // Wait for termination signal
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");
    
    Ok(())
}