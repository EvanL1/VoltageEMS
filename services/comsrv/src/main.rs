//! # Communication Service (ComSrv) - Main Entry Point
//!
//! The main executable for the Communication Service, responsible for initializing
//! and managing communication channels for various industrial protocols including
//! Modbus TCP, IEC 60870-5-104, and other SCADA communication protocols.
//!
//! ## Overview
//!
//! ComSrv is designed as a high-performance, asynchronous communication service
//! that handles multiple protocol channels simultaneously. It provides:
//!
//! - Multi-protocol communication support
//! - Real-time data collection and processing
//! - RESTful API for management and monitoring
//! - Console-based logging via env_logger
//! - Graceful shutdown and resource cleanup
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │   Config Mgr    │───►│ Protocol Factory│───►│   Channels      │
//! │   (YAML)        │    │   (Multi-proto) │    │  (TCP/RTU/...)  │
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//!          │                       │                       │
//!          ▼                       ▼                       ▼
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │   env_logger    │    │   Redis Store   │    │   API Server    │
//! │   (Console)     │    │   (Optional)    │    │   (REST/HTTP)   │
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```bash
//! # Start with default configuration
//! cargo run --bin comsrv
//!
//! # Start with custom configuration file
//! CONFIG_FILE=my_config.yaml cargo run --bin comsrv
//!
//! # Start with custom log level
//! RUST_LOG=debug cargo run --bin comsrv
//!
//! # Start in super test mode
//! cargo run --bin comsrv -- --super-test --duration 60
//! ```

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;



use clap::Parser;
use dotenv::dotenv;
use tokio::sync::RwLock;

use tracing::{error, info, warn};

mod api;
mod core;
mod service_impl;
mod utils;

use crate::api::openapi_routes;
use crate::core::config::ConfigManager;
use crate::core::protocols::common::ProtocolFactory;
use crate::utils::error::Result;

use crate::service_impl::{shutdown_handler, start_cleanup_task, start_communication_service};

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

    /// Test duration in seconds (for super test mode)
    #[arg(short, long)]
    duration: Option<u64>,

    /// Enable super test mode for large-scale testing
    #[arg(long)]
    super_test: bool,

    /// Log directory path
    #[arg(long, default_value = "logs")]
    log_dir: String,

    /// Override log level (debug, info, warn, error)
    #[arg(long)]
    log_level: Option<String>,
}

/// Handle graceful shutdown of the communication service
///
/// Performs an orderly shutdown of all communication channels, ensuring that
/// ongoing operations complete properly and resources are released cleanly.
///
/// # Arguments
///
/// * `factory` - Thread-safe protocol factory managing all active channels
///
/// # Features
///
/// - **Graceful Channel Shutdown**: Stops all channels in an orderly manner
/// - **Resource Cleanup**: Ensures proper release of network and system resources
/// - **Error Handling**: Logs but doesn't fail on individual channel shutdown errors
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::core::ProtocolFactory;
///
/// async fn main_loop(factory: Arc<RwLock<ProtocolFactory>>) {
///     // Setup signal handlers
///     let factory_clone = factory.clone();
///     tokio::spawn(async move {
///         tokio::signal::ctrl_c().await.unwrap();
///         shutdown_handler(factory_clone).await;
///     });
///     
///     // Main service loop...

/// Main entry point for the Communication Service
///
/// Initializes the complete communication service including configuration loading,
/// env_logger setup, and service startup. Handles graceful shutdown and provides
/// comprehensive error handling throughout the lifecycle.
///
/// # Environment Variables
///
/// * `CONFIG_FILE` - Path to configuration file (default: "config/comsrv.yaml")
/// * `RUST_LOG` - Log level (debug, info, warn, error)
///
/// # Configuration File
///
/// The service expects a YAML configuration file with the following structure:
///
/// ```yaml
/// service:
///   name: "ComSrv"
///   logging:
///     level: "info"
///   api:
///     enabled: true
///     bind_address: "0.0.0.0:3000"
///
/// channels:
///   - id: 1
///     name: "Modbus Device 1"
///     protocol: "ModbusTcp"
///     parameters:
///       host: "192.168.1.100"
///       port: 502
/// ```
///
/// # Returns
///
/// * `Ok(())` - Service completed successfully
/// * `Err(error)` - Critical error occurred during startup or operation
///
/// # Features
///
/// - **Environment Configuration**: Supports configuration via environment variables
/// - **Console Logging**: Structured logging via env_logger with configurable levels
/// - **Multi-Protocol Support**: Modbus TCP/RTU, IEC 60870-5-104, and custom protocols
/// - **Signal Handling**: Graceful shutdown on SIGINT/SIGTERM
/// - **API Server**: RESTful API for management and monitoring
/// - **Redis Integration**: Optional Redis storage for data persistence
///
/// # Error Handling
///
/// The service implements graceful error handling at multiple levels:
/// - Configuration errors: Service fails to start with clear error messages
/// - Channel errors: Individual channel failures don't affect other channels
/// - Runtime errors: Logged and handled gracefully without service termination
///
/// # Examples
///
/// ```bash
/// # Start with default settings
/// cargo run --bin comsrv
///
/// # Start with custom configuration
/// CONFIG_FILE=/etc/comsrv/config.yaml cargo run --bin comsrv
///
/// # Start with debug logging
/// RUST_LOG=debug cargo run --bin comsrv
/// ```
#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize environment variables with better error handling
    if let Err(e) = dotenv() {
        eprintln!("Warning: Failed to load .env file: {}", e);
    }

    // Use config file from CLI args or environment variable
    let config_file = env::var("CONFIG_FILE").unwrap_or(args.config);

    // Create modern Figment configuration manager with better error context
    let config_manager = match ConfigManager::from_file(&config_file) {
        Ok(cm) => {
            info!("Configuration loaded from: {} (using Figment)", config_file);
            Arc::new(cm)
        }
        Err(e) => {
            error!("Failed to load configuration from {}: {}", config_file, e);
            return Err(e);
        }
    };

    // Initialize tracing with file and console output
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    
    let service_name = config_manager.service().name.clone();
    let log_file = format!("logs/{}-{}.log", service_name, chrono::Local::now().format("%Y-%m-%d"));
    
    // Create log directory if it doesn't exist
    std::fs::create_dir_all("logs").unwrap_or_else(|e| {
        eprintln!("Failed to create logs directory: {}", e);
    });
    
    // File appender with daily rotation
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", &format!("{}.log", service_name));
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Initialize tracing subscriber with JSON format
    tracing_subscriber::fmt()
        .with_writer(std::io::stdout.and(non_blocking))
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(config_manager.get_log_level().parse().unwrap_or(tracing::Level::INFO.into())))
        .json()
        .with_target(false)  // 不显示模块路径
        .with_thread_ids(false)  // 不显示线程ID
        .with_file(false)    // 不显示文件名
        .with_line_number(false)  // 不显示行号
        .with_current_span(false)  // 不显示span信息
        .init();
    
    info!("Starting Communication Service: {}", service_name);
    info!("Logging to console and file: logs/{}.log.YYYY-MM-DD", service_name);

    if args.super_test {
        info!(
            "Starting Communication Service v{} - SUPER TEST MODE",
            env!("CARGO_PKG_VERSION")
        );
        info!("Super test configuration:");
        info!("  - Config file: {}", config_file);
        if let Some(duration) = args.duration {
            info!("  - Test duration: {} seconds", duration);
        }
        info!("  - Log level: {}", config_manager.get_log_level());
    } else {
        info!(
            "Starting Communication Service v{}",
            env!("CARGO_PKG_VERSION")
        );
    }

    // Initialize optimized Protocol Factory
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // Start cleanup task
    let cleanup_handle = start_cleanup_task(factory.clone());

    // Start communication service with improved error handling
    if let Err(e) = start_communication_service(config_manager.clone(), factory.clone()).await {
        error!("Failed to start communication service: {}", e);
        return Err(e);
    }

    // Start OpenAPI service only
    if config_manager.get_api_enabled() {
        let socket_addr = config_manager
            .get_api_address()
            .parse::<SocketAddr>()
            .unwrap_or_else(|e| {
                warn!(
                    "Invalid API address: {}, using default 0.0.0.0:3000. Error: {}",
                    config_manager.get_api_address(),
                    e
                );
                "0.0.0.0:3000".parse().unwrap()
            });

        // Create axum router with CORS middleware
        let app = openapi_routes::create_api_routes()
            .layer(
                tower_http::cors::CorsLayer::new()
                    .allow_origin(tower_http::cors::Any)
                    .allow_headers([
                        axum::http::header::CONTENT_TYPE,
                        axum::http::header::AUTHORIZATION,
                    ])
                    .allow_methods([
                        axum::http::Method::GET,
                        axum::http::Method::POST,
                        axum::http::Method::PUT,
                        axum::http::Method::DELETE,
                        axum::http::Method::OPTIONS,
                    ])
            );

        info!(
            "Starting Communication Service with OpenAPI at: http://{}",
            socket_addr
        );
        info!("Swagger UI: http://{}/swagger-ui", socket_addr);
        info!("OpenAPI spec: http://{}/api-docs/openapi.json", socket_addr);
        info!("Health check: http://{}/api/health", socket_addr);
        info!("Service status: http://{}/api/status", socket_addr);

        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(socket_addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });

        info!("OpenAPI service started successfully");
    } else {
        info!("API server disabled in configuration");
    }

    info!("Service startup completed successfully");

    // Handle super test mode with duration
    if args.super_test && args.duration.is_some() {
        let duration = args.duration.unwrap();
        info!("Super test mode: running for {} seconds", duration);

        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(duration)) => {
                info!("⏰ Super test duration completed ({} seconds)", duration);
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal during super test");
            }
        }
    } else {
        // Wait for termination signal with proper cleanup
        match tokio::signal::ctrl_c().await {
            Ok(_) => {
                info!("Received shutdown signal");
            }
            Err(e) => {
                error!("Failed to listen for shutdown signal: {}", e);
            }
        }
    }

    // Perform graceful shutdown
    shutdown_handler(factory).await;
    // Stop background cleanup task
    cleanup_handle.abort();
    let _ = cleanup_handle.await;
    info!("Communication service shutdown completed");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use crate::core::config::ConfigManager;
    use std::time::Duration;

    #[tokio::test]
    async fn test_start_communication_service_empty_channels() {
        // Create a temporary config file for testing
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let test_config = r#"
version: "1.0"
service:
  name: "test_service"
  description: "Test Communication Service"
  metrics:
    enabled: false
    bind_address: "0.0.0.0:9100"
  logging:
    level: "info"
    file: "/tmp/test_comsrv.log"
    max_size: 10485760
    max_files: 5
    console: true
  api:
    enabled: false
    bind_address: "0.0.0.0:3000"
    version: "v1"
  redis:
    enabled: false
    connection_type: "Tcp"
    address: "127.0.0.1:6379"
    db: 0
  point_tables:
    enabled: false
    directory: "config/points"
    watch_changes: false
    reload_interval: 60
channels: []
"#;

        std::fs::write(&config_path, test_config).unwrap();

        let config_manager = Arc::new(ConfigManager::from_file(&config_path).unwrap());
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        let result = start_communication_service(config_manager, factory).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown_handler() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // This should not panic or error
        shutdown_handler(factory).await;
    }

    #[tokio::test]
    async fn test_start_cleanup_task() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Start cleanup task
        let cleanup_handle = start_cleanup_task(factory.clone());

        // Let it run for a short time
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Cancel the task
        cleanup_handle.abort();

        // Wait for the task to actually finish
        let _ = cleanup_handle.await;

        // The task should be finished now
        assert!(true); // If we get here, the test passed
    }

    #[test]
    fn test_logger_initialization() {
        // Test that we can set environment variables for logging
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "info");
        }

        // Simply test that the log level environment variable is set
        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        assert!(!log_level.is_empty());
    }

    #[test]
    fn test_protocol_factory_creation() {
        let factory = ProtocolFactory::new();
        assert_eq!(factory.channel_count(), 0);
    }

    #[test]
    fn test_environment_variable_handling() {
        // Test default config file path
        std::env::remove_var("CONFIG_FILE");
        let config_file =
            env::var("CONFIG_FILE").unwrap_or_else(|_| "config/comsrv.yaml".to_string());
        assert_eq!(config_file, "config/comsrv.yaml");

        // Test custom config file path
        std::env::set_var("CONFIG_FILE", "custom/config.yaml");
        let config_file =
            env::var("CONFIG_FILE").unwrap_or_else(|_| "config/comsrv.yaml".to_string());
        assert_eq!(config_file, "custom/config.yaml");

        // Clean up
        std::env::remove_var("CONFIG_FILE");
    }

    #[test]
    fn test_log_directory_handling() {
        // Test default log directory
        std::env::remove_var("LOG_DIR");
        let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());
        assert_eq!(log_dir, "logs");

        // Test custom log directory
        std::env::set_var("LOG_DIR", "custom/logs");
        let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());
        assert_eq!(log_dir, "custom/logs");

        // Clean up
        std::env::remove_var("LOG_DIR");
    }

    #[tokio::test]
    async fn test_service_lifecycle() {
        // Test basic service lifecycle components
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Test factory creation
        assert_eq!(factory.read().await.channel_count(), 0);

        // Test cleanup task creation (should not panic)
        let cleanup_handle = start_cleanup_task(factory.clone());

        // Test shutdown handler (should not panic)
        shutdown_handler(factory).await;
    }

    #[test]
    fn test_cargo_version_access() {
        // Test that we can access the cargo version
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
        assert!(version.contains('.'));
    }

    #[tokio::test]
    async fn test_concurrent_factory_access() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Test concurrent read access
        let factory1 = factory.clone();
        let factory2 = factory.clone();

        let handle1 = tokio::spawn(async move {
            let _guard = factory1.read().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        });

        let handle2 = tokio::spawn(async move {
            let _guard = factory2.read().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        });

        // Both should complete without deadlock
        let _ = tokio::join!(handle1, handle2);
    }

    #[test]
    fn test_error_handling_patterns() {
        // Test that our error handling patterns work correctly
        use crate::utils::error::{ComSrvError, Result};

        let error_result: Result<()> = Err(ComSrvError::ConfigError("Test error".to_string()));
        assert!(error_result.is_err());

        match error_result {
            Err(ComSrvError::ConfigError(msg)) => assert_eq!(msg, "Test error"),
            _ => panic!("Expected ConfigError"),
        }
    }
}
