//! ModSrv main program
//!
//! Provides concise service startup and command line interface

mod api;
mod config;
mod error;
mod model;
mod template;

use crate::api::ApiServer;
use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use crate::model::ModelManager;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use tracing::{error, info};

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
    let template_dir = std::env::var("TEMPLATE_DIR").unwrap_or_else(|_| "templates".to_string());
    let model_manager = ModelManager::new(&config.redis.url, &template_dir).await?;

    // No pre-configured models in the new design
    info!("ModSrv started in API mode - models will be created via API");

    let model_manager = Arc::new(model_manager);

    // Create API server
    let api_server = ApiServer::new(model_manager.clone(), config.clone());

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
        },
        Ok(Some(Err(e))) => {
            error!("✗ Failed to start API server: {}", e);
            return Err(ModelSrvError::config(
                "Failed to start API server".to_string(),
            ));
        },
        Ok(None) => {
            error!("✗ API server startup channel closed");
            return Err(ModelSrvError::config(
                "API server startup channel closed".to_string(),
            ));
        },
        Err(_) => {
            error!("✗ API server startup timeout");
            return Err(ModelSrvError::config(
                "API server startup timeout".to_string(),
            ));
        },
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
    info!("=== ModSrv Configuration Check ===");

    // 1. Validate configuration
    match config.validate() {
        Ok(_) => info!("✓ Configuration file validation passed"),
        Err(e) => {
            error!("✗ Configuration file validation failed: {}", e);
            return Err(e);
        },
    }

    // 2. Display service configuration
    info!("--- Service Configuration ---");
    info!("Service name: {}", config.service_name);
    info!("Version: {}", config.version);
    info!(
        "API address: http://{}:{}",
        config.api.host, config.api.port
    );
    info!("Log level: {}", config.log.level);

    // 3. Display Redis configuration
    info!("--- Redis Configuration ---");
    info!("URL: {}", config.redis.url);
    info!("Prefix: {}", config.redis.key_prefix);

    // 4. Test Redis connection
    info!("Connection test in progress...");
    let template_dir = std::env::var("TEMPLATE_DIR").unwrap_or_else(|_| "templates".to_string());
    match ModelManager::new(&config.redis.url, &template_dir).await {
        Ok(_) => {
            info!("✓ Connection test: Success");
            info!("Lua scripts: ✓ Loaded");
        },
        Err(e) => {
            error!("✗ Connection test: Failed - {}", e);
            return Err(ModelSrvError::redis(format!(
                "Redis connection failed: {}",
                e
            )));
        },
    }

    // 5. Display template information
    info!("--- Template Configuration ---");
    info!("Template directory: {}", template_dir);
    let model_manager = ModelManager::new(&config.redis.url, &template_dir).await?;
    let template_manager = model_manager.template_manager();
    let templates = {
        let tm = template_manager.lock().await;
        tm.list_templates().into_iter().cloned().collect::<Vec<_>>()
    };
    if templates.is_empty() {
        info!("No templates loaded");
    } else {
        info!("Loaded {} templates:", templates.len());
        for template in templates {
            info!("• Template: {}", template.id);
            if !template.data.is_empty() {
                info!(
                    "  Data points: {:?}",
                    template.data.keys().collect::<Vec<_>>()
                );
            }
            if !template.action.is_empty() {
                info!(
                    "  Actions: {:?}",
                    template.action.keys().collect::<Vec<_>>()
                );
            }
        }
    }

    info!("✓ All checks passed");
    Ok(())
}
