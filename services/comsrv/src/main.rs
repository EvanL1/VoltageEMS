use std::sync::Arc;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use chrono::Utc;
use dotenv::dotenv;
use tracing;
use tokio::sync::RwLock;

mod core;
mod utils;
mod api;

use crate::utils::logger;
use crate::utils::error::Result;
use crate::core::config::ConfigManager;
use crate::core::protocol_factory::ProtocolFactory;
use crate::api::routes::api_routes;
use crate::core::protocols::modbus::client::ModbusClientFactory;
use crate::utils::ComSrvError;
use crate::core::storage::redisStorage::RedisStore;

/// Function to create Modbus TCP client for the factory
fn create_modbus_tcp(config: crate::core::config::config_manager::ChannelConfig) 
    -> Result<Box<dyn crate::core::protocols::common::ComBase>> {
    let config_clone = config.clone();
    let result = ModbusClientFactory::create_client(config);
    
    match result {
        Ok(_) => {
            // Create a new Box<dyn ComBase> object
            let client = crate::core::protocols::modbus::tcp::ModbusTcpClient::new(config_clone);
            Ok(Box::new(client) as Box<dyn crate::core::protocols::common::ComBase>)
        },
        Err(e) => Err(e),
    }
}

/// Function to create Modbus RTU client for the factory
fn create_modbus_rtu(config: crate::core::config::config_manager::ChannelConfig) 
    -> Result<Box<dyn crate::core::protocols::common::ComBase>> {
    let config_clone = config.clone();
    let result = ModbusClientFactory::create_client(config);
    
    match result {
        Ok(_) => {
            // Create a new Box<dyn ComBase> object
            let client = crate::core::protocols::modbus::rtu::ModbusRtuClient::new(config_clone);
            Ok(Box::new(client) as Box<dyn crate::core::protocols::common::ComBase>)
        },
        Err(e) => Err(e),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging system
    let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());
    crate::utils::logger::init_logger(Path::new(&log_dir), "comsrv", "info", true)?;
    tracing::info!("Starting Comsrv Service");
    
    // Record start time
    let start_time = Arc::new(Utc::now());
    
    // Load configuration
    let config_path_env = env::var("CONFIG_PATH").unwrap_or_else(|_| "config".to_string());
    let config_path = if Path::new(&config_path_env).is_dir() {
        // If it's a directory, append the default config filename
        format!("{}/comsrv.yaml", config_path_env)
    } else {
        config_path_env
    };
    tracing::info!("Loading configuration from {}", config_path);
    let config_manager = ConfigManager::from_file(&config_path)?;
    
    // Initialize Redis storage
    let redis_config = config_manager.get_redis_config();
    let redis_store = RedisStore::from_config(&redis_config).await?;
        
    // Create protocol factory
    let protocol_factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // Register protocol implementations
    {
        let mut factory = protocol_factory.write().await;
        // Register various protocol implementations
        tracing::info!("Registering protocol implementations");
        
        // Register Modbus TCP and RTU protocols
        factory.register_protocol("ModbusTcp", create_modbus_tcp).await?;
        factory.register_protocol("ModbusRtu", create_modbus_rtu).await?;
    }
    
    // Initialize channels
    {
        let mut factory = protocol_factory.write().await;
        // Load channels from configuration
        tracing::info!("Initializing channels from configuration");
        
        for channel_config in config_manager.get_channels() {
            match factory.create_channel(channel_config.clone()).await {
                Ok(_) => tracing::info!("Channel {} initialized", channel_config.id),
                Err(e) => tracing::error!("Failed to initialize channel {}: {}", channel_config.id, e),
            }
        }
    }
    
    // Start all channels
    {
        let mut factory = protocol_factory.write().await;
        let channels = factory.get_all_channels_mut().await;
        for (id, channel) in channels.iter_mut() {
            match channel.start().await {
                Ok(_) => tracing::info!("Channel {} started", id),
                Err(e) => tracing::error!("Failed to start channel {}: {}", id, e),
            }
        }
    }
    
    // Start metrics service
    if config_manager.get_metrics_enabled() {
        let metrics_addr = config_manager.get_metrics_address()
            .parse::<SocketAddr>()
            .unwrap_or_else(|_| "0.0.0.0:9100".parse().unwrap());
            
        tracing::info!("Starting metrics service on {}", metrics_addr);
        
        // Initialize metrics system
        crate::core::metrics::init_metrics(&config_manager.get_service_name());
        
        // Get metrics instance
        if let Some(metrics) = crate::core::metrics::get_metrics() {
            tokio::spawn(async move {
                if let Err(e) = metrics.start_server(&metrics_addr.to_string()).await {
                    tracing::error!("Failed to start metrics server: {}", e);
                }
            });
        } else {
            tracing::error!("Failed to get metrics instance");
        }
    }
    
    // Start API service
    let api_addr = config_manager.get_api_address()
        .parse::<SocketAddr>()
        .unwrap_or_else(|_| "0.0.0.0:3000".parse().unwrap());
    
    tracing::info!("Starting API service on {}", api_addr);
    
    // Create API routes
    let api = api_routes(protocol_factory.clone(), start_time.clone());
    
    // Start warp server
    warp::serve(api)
        .run(api_addr)
        .await;
    
    // Normal exit
    tracing::info!("Comsrv Service shutdown");
    Ok(())
}