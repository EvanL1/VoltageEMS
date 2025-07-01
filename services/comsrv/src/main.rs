use std::env;
use std::sync::Arc;
use std::net::SocketAddr;

use clap::Parser;
use dotenv::dotenv;
use tokio::sync::RwLock;
use tokio::signal;
use axum::serve;

use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use comsrv::core::config::ConfigManager;
use comsrv::core::protocols::common::combase::protocol_factory::ProtocolFactory;
use comsrv::api::openapi_routes::create_api_routes;
use comsrv::service_impl::{start_communication_service, start_cleanup_task, shutdown_handler};
use comsrv::utils::error::Result;

/// Command line arguments for the Communication Service
#[derive(Parser)]
#[command(
    name = "comsrv",
    version = env!("CARGO_PKG_VERSION"),
    about = "Communication Service for Industrial Protocols",
    long_about = "A high-performance communication service supporting Modbus, IEC 60870-5-104, and other industrial protocols"
)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config/comsrv.yaml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load environment variables
    dotenv().ok();

    // Initialize logging/tracing
    initialize_logging();

    info!("Starting Communication Service v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration using ConfigManager
    info!("Loading configuration from: {}", args.config);
    let config_manager = Arc::new(
        ConfigManager::from_file(&args.config)
            .map_err(|e| {
                error!("Failed to load configuration: {}", e);
                e
            })?
    );
    
    // Display configuration summary
    info!("Configuration loaded successfully:");
    info!("  - Service name: {}", config_manager.config().service.name);
    info!("  - Channels configured: {}", config_manager.config().channels.len());
    info!("  - API enabled: {}", config_manager.config().service.api.enabled);
    info!("  - Redis enabled: {}", config_manager.config().service.redis.enabled);

    // Create protocol factory
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // Start communication service (initializes channels, Redis, etc.)
    info!("Starting communication channels...");
    start_communication_service(config_manager.clone(), factory.clone()).await?;

    // Start cleanup task
    let cleanup_factory = factory.clone();
    let cleanup_handle = tokio::spawn(async move {
        if let Err(e) = start_cleanup_task(cleanup_factory).await {
            error!("Cleanup task error: {}", e);
        }
    });

    // Start API server if enabled
    let api_handle = if config_manager.config().service.api.enabled {
        let bind_address = &config_manager.config().service.api.bind_address;
        let addr: SocketAddr = bind_address.parse()
            .map_err(|e| {
                error!("Invalid API bind address '{}': {}", bind_address, e);
                comsrv::utils::error::ComSrvError::ConfigError(
                    format!("Invalid API bind address: {}", e)
                )
            })?;

        info!("Starting API server on {}", addr);
        
        let app = create_api_routes(factory.clone());
        let listener = tokio::net::TcpListener::bind(addr).await?;
        
        Some(tokio::spawn(async move {
            if let Err(e) = serve(listener, app).await {
                error!("API server error: {}", e);
            }
        }))
    } else {
        info!("API server disabled in configuration");
        None
    };

    info!("Communication service started successfully");
    info!("Press Ctrl+C to shutdown");

    // Wait for shutdown signal
    shutdown_signal().await;

    info!("Shutting down communication service...");

    // Shutdown channels
    shutdown_handler(factory.clone()).await;

    // Cancel background tasks
    cleanup_handle.abort();
    if let Some(api_handle) = api_handle {
        api_handle.abort();
    }

    // Give tasks time to cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    info!("Communication service shutdown complete");
    Ok(())
}

/// Initialize logging/tracing
fn initialize_logging() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Default to info level, but allow RUST_LOG to override
            "comsrv=info,tower_http=info".into()
        });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }
}