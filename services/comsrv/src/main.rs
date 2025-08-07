//! Communication Service (`ComsrvRust`)
//!
//! A high-performance, async-first industrial communication service written in Rust.
//! This provides a unified interface for communicating with various industrial
//! protocols including Modbus TCP/RTU, IEC60870-5-104, and more.

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::serve;
use clap::Parser;
use dotenv::dotenv;
use tokio::sync::RwLock;

use tracing::{error, info, warn, Level};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

// Module declarations
pub mod api;
pub mod core;
pub mod plugins;
pub mod service;
pub mod storage;
pub mod utils;

// Re-export commonly used types from utils::error
pub use utils::error::{ComSrvError, Result};

// Internal imports using crate::
use crate::api::routes::create_api_routes;
use crate::core::combase::factory::ProtocolFactory;
use crate::core::config::ConfigManager;
use crate::service::{shutdown_handler, start_cleanup_task, start_communication_service};

fn print_startup_banner() {
    println!();
    println!("  ██████╗ ██████╗ ███╗   ███╗███████╗██████╗ ██╗   ██╗");
    println!(" ██╔════╝██╔═══██╗████╗ ████║██╔════╝██╔══██╗██║   ██║");
    println!(" ██║     ██║   ██║██╔████╔██║███████╗██████╔╝██║   ██║");
    println!(" ██║     ██║   ██║██║╚██╔╝██║╚════██║██╔══██╗╚██╗ ██╔╝");
    println!(" ╚██████╗╚██████╔╝██║ ╚═╝ ██║███████║██║  ██║ ╚████╔╝ ");
    println!("  ╚═════╝ ╚═════╝ ╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝  ╚═══╝  ");
    println!();
    println!(
        " ⚡ Industrial Communication Service v{} ⚡",
        env!("CARGO_PKG_VERSION")
    );
    println!(" 🦀 Built with Rust for High Performance");
    println!(" 📡 Multi-Protocol Support (Modbus/IEC60870)");
    println!();
}

#[derive(Parser)]
#[command(
    name = "comsrv",
    version = env!("CARGO_PKG_VERSION"),
    about = "Industrial Communication Service",
    long_about = None
)]
struct Args {
    /// Configuration file path
    #[arg(short = 'c', long, default_value = "config/comsrv.yaml")]
    config: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short = 'l', long, default_value = "info")]
    log_level: String,

    /// Bind address for API server
    #[arg(short = 'b', long)]
    bind_address: Option<String>,

    /// Enable debug mode
    #[arg(short = 'd', long)]
    debug: bool,

    /// Disable colored output
    #[arg(long)]
    no_color: bool,

    /// Log to file instead of console
    #[arg(long)]
    log_file: Option<String>,

    /// Skip loading CSV point tables
    #[arg(long)]
    skip_csv: bool,

    /// Validation mode - only validate configuration without starting service
    #[arg(long)]
    validate: bool,
}

fn initialize_logging(args: &Args) -> Result<()> {
    // Load environment variables from .env file
    if let Err(e) = dotenv() {
        eprintln!("Warning: Failed to load .env file: {}", e);
    }

    // Determine log level
    let log_level = if args.debug {
        Level::DEBUG
    } else {
        env::var("RUST_LOG")
            .unwrap_or_else(|_| args.log_level.clone())
            .parse()
            .unwrap_or(Level::INFO)
    };

    // Build the subscriber
    let subscriber = tracing_subscriber::registry();

    // Configure console output
    if args.log_file.is_none() {
        let console_layer = tracing_subscriber::fmt::layer()
            .with_ansi(!args.no_color)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(args.debug)
            .with_thread_names(args.debug)
            .with_file(args.debug)
            .with_line_number(args.debug)
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                log_level,
            ));

        subscriber.with(console_layer).init();
    } else {
        // Configure file output
        let file_path = args.log_file.as_ref().unwrap();
        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            std::path::Path::new(file_path)
                .parent()
                .unwrap_or(std::path::Path::new(".")),
            std::path::Path::new(file_path)
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("comsrv.log")),
        );

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                log_level,
            ));

        subscriber.with(file_layer).init();
    }

    info!(
        "Logging initialized with level: {}",
        log_level.as_str().to_uppercase()
    );
    Ok(())
}

fn check_system_requirements() -> Result<()> {
    // Check CPU cores
    let cpu_count = num_cpus::get();
    if cpu_count < 2 {
        warn!(
            "System has only {} CPU core(s). Performance may be limited.",
            cpu_count
        );
    } else {
        info!("System has {} CPU cores available", cpu_count);
    }

    // Check available memory (this is a simplified check)
    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            if let Some(line) = meminfo.lines().find(|l| l.starts_with("MemAvailable:")) {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        let mb = kb / 1024;
                        if mb < 512 {
                            warn!("Low available memory: {} MB", mb);
                        } else {
                            info!("Available memory: {} MB", mb);
                        }
                    }
                }
            }
        }
    }

    // Check Redis connectivity if required
    if env::var("COMSRV_REDIS_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true" {
        info!("Redis storage is enabled, connection will be tested during startup");
    }

    Ok(())
}

async fn validate_configuration(config_path: &str) -> Result<()> {
    info!("Validating configuration from: {}", config_path);

    // Load and validate configuration
    let config_manager = ConfigManager::from_file(config_path)?;
    info!("Configuration loaded successfully");

    // Validate service configuration
    let service_config = config_manager.service_config();
    info!("Service: {}", service_config.name);
    if let Some(desc) = &service_config.description {
        info!("Description: {}", desc);
    }

    // Validate channels
    let channels = config_manager.channels();
    info!("Found {} channel(s)", channels.len());

    for channel in channels {
        info!(
            "  Channel {}: {} (protocol: {})",
            channel.id, channel.name, channel.protocol
        );

        // Check point counts
        let point_count = channel.get_total_points_count();
        if point_count == 0 {
            warn!("    No points configured for channel {}", channel.id);
        } else {
            info!("    Points configured: {}", point_count);
        }
    }

    info!("Configuration validation completed successfully");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Initialize logging first
    initialize_logging(&args)?;

    // Print startup banner
    if !args.no_color && args.log_file.is_none() {
        print_startup_banner();
    }

    // Check system requirements
    check_system_requirements()?;

    // If in validation mode, only validate and exit
    if args.validate {
        validate_configuration(&args.config).await?;
        info!("Validation completed successfully");
        return Ok(());
    }

    // Load configuration
    info!("Loading configuration from: {}", args.config);
    let config_manager = match ConfigManager::from_file(&args.config) {
        Ok(cm) => {
            info!("Configuration loaded successfully");
            Arc::new(cm)
        },
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            return Err(e);
        },
    };

    // Initialize protocol factory
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
    info!("Protocol factory initialized");

    // Get service configuration
    let service_config = config_manager.service_config();
    let api_config = &service_config.api;

    // Determine bind address
    let bind_address = args
        .bind_address
        .or_else(|| env::var("COMSRV_BIND_ADDRESS").ok())
        .unwrap_or_else(|| format!("{}:{}", api_config.host, api_config.port));

    let addr: SocketAddr = bind_address.parse().map_err(|e| {
        ComSrvError::ConfigError(format!("Invalid bind address '{}': {}", bind_address, e))
    })?;

    info!("Starting {} service", service_config.name);
    if let Some(desc) = &service_config.description {
        info!("Service description: {}", desc);
    }

    // Check Redis configuration
    if service_config.redis.enabled {
        info!("Redis storage enabled at: {}", service_config.redis.url);
    } else {
        info!("Redis storage disabled - using in-memory storage");
    }

    // Start communication service
    info!("Starting communication channels...");
    start_communication_service(config_manager.clone(), factory.clone()).await?;

    // Start cleanup task for resource management
    let (cleanup_handle, cleanup_token) = start_cleanup_task(factory.clone());

    // Create API routes
    let app = create_api_routes(factory.clone());

    // Create TCP listener
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| ComSrvError::NetworkError(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("API server listening on http://{}", addr);
    info!("Service is ready to accept connections");

    // Health check endpoint information
    info!("Health check available at: http://{}/health", addr);
    info!("API documentation available at: http://{}/api-docs", addr);

    // Start the server with graceful shutdown
    let server = serve(listener, app);

    // Wait for shutdown signal
    let shutdown_future = shutdown_handler(factory.clone());

    // Run server until shutdown signal
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server
            .with_graceful_shutdown(async move {
                shutdown_future.await;
            })
            .await
        {
            error!("Server error: {}", e);
        }
    });

    // Wait for the server to complete
    if let Err(e) = server_handle.await {
        error!("Server task failed: {}", e);
    }

    // Cleanup
    cleanup_token.cancel();
    cleanup_handle.abort();
    info!("Service shutdown complete");

    Ok(())
}
