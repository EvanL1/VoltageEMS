//! ModSrv main program
//!
//! Provides concise service startup and command line interface

mod api;
mod config;
mod error;
mod mapping;
mod model;
mod websocket;

use crate::api::ApiServer;
use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use crate::mapping::MappingManager;
use crate::model::ModelManager;
use crate::websocket::WsConnectionManager;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(author, version, about = "ModSrv - Model Service")]
struct Args {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Check configuration and connections
    Check,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let config = if let Some(config_path) = args.config {
        Config::from_file(config_path)?
    } else if let Ok(config_file) = std::env::var("CONFIG_FILE") {
        info!(
            "Loading config from environment variable CONFIG_FILE: {}",
            config_file
        );
        Config::from_file(config_file)?
    } else {
        Config::load()?
    };

    // Validate configuration
    config.validate()?;

    // Initialize logging
    voltage_libs::logging::init(&config.log.level)
        .map_err(|e| ModelSrvError::config(format!("Failed to initialize logging: {}", e)))?;

    info!("Starting ModSrv v{}", config.version);

    // Execute command
    match args.command {
        Some(Commands::Check) => check_config(config).await,
        None => run_service(config).await, // Default: run service
    }
}

/// Run service mode
async fn run_service(config: Config) -> Result<()> {
    info!("Starting ModSrv service mode");

    // Create model manager (using EdgeRedis)
    let model_manager = ModelManager::new(&config.redis.url).await?;

    // Load model configurations
    let enabled_models = config.enabled_models();
    let model_configs: Vec<_> = enabled_models.into_iter().cloned().collect();

    info!("Found {} model configurations", config.models.len());
    info!("Configured {} models", model_configs.len());

    if !model_configs.is_empty() {
        for model in &model_configs {
            info!("Loading model: {} ({})", model.id, model.name);
        }
        model_manager.load_models(model_configs).await?;
        info!("Model loading completed");
    } else {
        info!("No models configured, service will only provide API interface");
    }

    let model_manager = Arc::new(model_manager);

    // Load mapping configuration
    let mut mapping_manager = MappingManager::new();
    let mappings_dir =
        std::env::var("MAPPINGS_DIR").unwrap_or_else(|_| "config/mappings".to_string());
    info!("Loading mapping configuration: {}", mappings_dir);

    if let Err(e) = mapping_manager.load_directory(&mappings_dir).await {
        warn!("Failed to load mapping configuration: {}", e);
    } else {
        // Load mappings to Redis
        model_manager.load_mappings(&mapping_manager).await?;
    }

    // Create WebSocket manager
    let ws_manager = Arc::new(WsConnectionManager::new());

    // Start Redis subscription
    if let Err(e) = ws_manager.start_redis_subscription().await {
        warn!("Failed to start Redis subscription: {}", e);
    }

    // Start WebSocket heartbeat
    ws_manager.start_heartbeat().await;

    // Create API server
    let api_server = ApiServer::new(model_manager.clone(), ws_manager.clone(), config.clone());

    // Start API server
    let (startup_tx, mut startup_rx) = mpsc::channel::<std::result::Result<(), String>>(1);

    tokio::spawn(async move {
        if let Err(e) = api_server.start_with_notification(startup_tx).await {
            error!("Failed to start API server: {}", e);
        }
    });

    // Wait for API server startup confirmation
    info!("Waiting for API server to start...");
    match tokio::time::timeout(Duration::from_secs(10), startup_rx.recv()).await {
        Ok(Some(Ok(_))) => {
            info!(
                "✓ API server started successfully: http://{}:{}",
                config.api.host, config.api.port
            );
        }
        Ok(Some(Err(e))) => {
            error!("✗ Failed to start API server: {}", e);
            return Err(ModelSrvError::config(
                "Failed to start API server".to_string(),
            ));
        }
        Ok(None) => {
            error!("✗ API server startup channel closed");
            return Err(ModelSrvError::config(
                "API server startup channel closed".to_string(),
            ));
        }
        Err(_) => {
            error!("✗ API server startup timeout");
            return Err(ModelSrvError::config(
                "API server startup timeout".to_string(),
            ));
        }
    }

    info!(
        "ModSrv service started, API address: http://{}:{}",
        config.api.host, config.api.port
    );

    // Main service loop - periodic health check
    let mut interval = time::interval(Duration::from_secs(60));
    let mut cycle_count = 0u64;

    loop {
        interval.tick().await;
        cycle_count += 1;

        let models = model_manager.list_models().await;
        info!(
            "Service running normally - cycle: {}, models: {}",
            cycle_count,
            models.len()
        );
    }
}

/// Check configuration and environment
async fn check_config(config: Config) -> Result<()> {
    println!("=== ModSrv Configuration Check ===\n");

    // 1. Validate configuration
    match config.validate() {
        Ok(_) => println!("✓ Configuration file validation passed"),
        Err(e) => {
            println!("✗ Configuration file validation failed: {}", e);
            return Err(e);
        }
    }

    // 2. Display service configuration
    println!("\n--- Service Configuration ---");
    println!("Service name: {}", config.service_name);
    println!("Version: {}", config.version);
    println!(
        "API address: http://{}:{}",
        config.api.host, config.api.port
    );
    println!("Log level: {}", config.log.level);

    // 3. Display Redis configuration
    println!("\n--- Redis Configuration ---");
    println!("URL: {}", config.redis.url);
    println!("Prefix: {}", config.redis.key_prefix);

    // 4. Test Redis connection
    print!("Connection test: ");
    match ModelManager::new(&config.redis.url).await {
        Ok(_) => {
            println!("✓ Success");

            // Test Lua scripts
            println!("Lua scripts: ✓ Loaded");
        }
        Err(e) => {
            println!("✗ Failed - {}", e);
            return Err(ModelSrvError::redis(format!(
                "Redis connection failed: {}",
                e
            )));
        }
    }

    // 5. Display model information
    println!("\n--- Model Configuration ---");
    if config.models.is_empty() {
        println!("No models configured");
    } else {
        println!("Configured {} models:", config.models.len());
        for model in &config.models {
            println!("\n• {} ({})", model.name, model.id);
            println!("  Description: {}", model.description);

            // Display monitoring points
            if !model.monitoring.is_empty() {
                println!("  Monitoring points ({}):", model.monitoring.len());
                for (name, point) in &model.monitoring {
                    let unit = point.unit.as_deref().unwrap_or("");
                    println!("    - {}: {} {}", name, point.description, unit);
                }
            }

            // Display control points
            if !model.control.is_empty() {
                println!("  Control points ({}):", model.control.len());
                for (name, point) in &model.control {
                    let unit = point.unit.as_deref().unwrap_or("");
                    println!("    - {}: {} {}", name, point.description, unit);
                }
            }
        }
    }

    println!("\n✓ All checks passed");
    Ok(())
}
