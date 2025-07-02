use crate::config::Config;
use crate::error::Result;
use crate::storage::{StorageManager, influxdb_storage::InfluxDBStorage, redis_storage::RedisStorage};
use crate::pubsub::{RedisSubscriber, MessageProcessor};
use crate::api::start_api_server;
use crate::monitoring::MetricsCollector;
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;

mod config;
mod error;
mod storage;
mod pubsub;
mod api;
mod monitoring;
mod logging;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments and load configuration first
    let config = Config::from_args()?;

    // Initialize logging with config
    if let Err(e) = crate::logging::init_logging(&config.logging) {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }
    
    tracing::info!("Starting HisSrv v{}", config.service.version);
    tracing::info!("Configuration loaded from: {}", config.config_file);

    // Initialize storage manager
    let mut storage_manager = StorageManager::new();

    // Setup InfluxDB storage backend
    if config.storage.backends.influxdb.enabled {
        let influxdb_storage = InfluxDBStorage::new(config.storage.backends.influxdb.clone());
        storage_manager.add_backend("influxdb".to_string(), Box::new(influxdb_storage));
        tracing::info!("Added InfluxDB storage backend");
    }

    // Setup Redis storage backend
    let redis_storage = RedisStorage::new(config.redis.connection.clone());
    storage_manager.add_backend("redis".to_string(), Box::new(redis_storage));
    tracing::info!("Added Redis storage backend");

    // Set default storage backend
    storage_manager.set_default_backend(config.storage.default.clone());

    // Connect to all storage backends
    storage_manager.connect_all().await?;

    // Wrap storage manager in Arc<RwLock> for shared access
    let storage_manager = Arc::new(RwLock::new(storage_manager));

    // Initialize metrics collector
    let metrics_collector = MetricsCollector::new();

    // Setup message processing pipeline
    let (message_sender, message_receiver) = mpsc::unbounded_channel();

    // Clone storage_manager for the message processor
    let storage_manager_for_processor = Arc::clone(&storage_manager);
    let mut message_processor = MessageProcessor::new(storage_manager_for_processor, message_receiver);

    // Setup Redis subscriber
    let mut redis_subscriber = RedisSubscriber::new(config.redis.clone(), message_sender);
    redis_subscriber.connect().await?;

    // Start background tasks
    let processor_handle = tokio::spawn(async move {
        if let Err(e) = message_processor.start_processing().await {
            tracing::error!("Message processor error: {}", e);
        }
    });

    let subscriber_handle = tokio::spawn(async move {
        if let Err(e) = redis_subscriber.start_listening().await {
            tracing::error!("Redis subscriber error: {}", e);
        }
    });

    // Start API server if enabled
    if config.api.enabled {
        let api_config = config.clone();
        let api_storage_manager = Arc::clone(&storage_manager);
        
        let api_handle = tokio::spawn(async move {
            if let Err(e) = start_api_server(api_config, api_storage_manager).await {
                tracing::error!("API server error: {}", e);
            }
        });

        tracing::info!("API server started on {}:{}", config.service.host, config.service.port);
        tracing::info!("Swagger UI available at: http://{}:{}/api/v1/swagger-ui", config.service.host, config.service.port);

        // Wait for all tasks
        tokio::select! {
            _ = processor_handle => tracing::info!("Message processor stopped"),
            _ = subscriber_handle => tracing::info!("Redis subscriber stopped"),
            _ = api_handle => tracing::info!("API server stopped"),
        }
    } else {
        tracing::info!("API server is disabled");
        
        // Wait for background tasks only
        tokio::select! {
            _ = processor_handle => tracing::info!("Message processor stopped"),
            _ = subscriber_handle => tracing::info!("Redis subscriber stopped"),
        }
    }

    // Cleanup
    storage_manager.write().await.disconnect_all().await?;
    tracing::info!("HisSrv shutdown complete");

    Ok(())
} 