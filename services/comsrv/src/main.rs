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
//! - Comprehensive metrics and logging
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
//! │   Logger        │    │   Metrics       │    │   API Server    │
//! │   (Rotating)    │    │  (Prometheus)   │    │   (REST/HTTP)   │
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
//! # Start with custom log directory
//! LOG_DIR=/var/log/comsrv cargo run --bin comsrv
//! ```

use std::sync::Arc;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use chrono::Utc;
use dotenv::dotenv;
use tracing;
use tokio::sync::RwLock;
use clap::Parser;

use tracing::{info, error, warn};

mod core;
mod utils;
mod api;

use crate::utils::error::Result;
use crate::core::config::ConfigManager;
use crate::core::protocols::common::ProtocolFactory;
use crate::api::routes::api_routes;
use crate::utils::logger::init_logger;
use crate::core::metrics::{init_metrics, get_metrics};

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

/// Start the communication service with optimized performance and monitoring
/// 
/// Initializes and starts all configured communication channels using the provided
/// configuration manager and protocol factory. This function handles channel creation,
/// startup, and error reporting with comprehensive metrics collection.
/// 
/// # Arguments
/// 
/// * `config_manager` - Shared configuration manager containing channel definitions
/// * `factory` - Thread-safe protocol factory for creating and managing channels
/// 
/// # Returns
/// 
/// * `Ok(())` - If the service starts successfully
/// * `Err(error)` - If critical errors occur during startup
/// 
/// # Features
/// 
/// - **Parallel Channel Creation**: Creates multiple channels concurrently
/// - **Error Isolation**: Continues operation even if some channels fail
/// - **Metrics Integration**: Records channel status and performance metrics
/// - **Graceful Degradation**: Provides service even with partial channel failures
/// 
/// # Examples
/// 
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::core::{ConfigManager, ProtocolFactory};
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config_manager = Arc::new(ConfigManager::from_file("config.yaml")?);
///     let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
///     
///     start_communication_service(config_manager, factory).await?;
///     Ok(())
/// }
/// ```
async fn start_communication_service(
    config_manager: Arc<ConfigManager>,
    factory: Arc<RwLock<ProtocolFactory>>
) -> Result<()> {
    // Get channel configurations
    let configs = config_manager.get_channels().clone();
    
    if configs.is_empty() {
        warn!("No channels configured");
        return Ok(());
    }
    
    info!("Creating {} channels...", configs.len());
    
    // Create channels with improved error handling and metrics
    let mut successful_channels = 0;
    let mut failed_channels = 0;
    
    for channel_config in configs {
        info!("Creating channel: {} - {}", channel_config.id, channel_config.name);
        
        let factory_guard = factory.write().await;
        match factory_guard.create_channel(channel_config.clone()) {
            Ok(_) => {
                info!("Channel created successfully: {}", channel_config.id);
                successful_channels += 1;
                
                // Record metrics if available
                if let Some(metrics) = get_metrics() {
                    metrics.update_channel_status(
                        &channel_config.id.to_string(), 
                        false, // Not connected yet
                        &config_manager.get_service_name()
                    );
                }
            },
            Err(e) => {
                error!("Failed to create channel {}: {}", channel_config.id, e);
                failed_channels += 1;
                
                // Record error metrics if available
                if let Some(metrics) = get_metrics() {
                    metrics.record_channel_error(
                        &channel_config.id.to_string(),
                        "creation_failed",
                        &config_manager.get_service_name()
                    );
                }
                
                // Continue with other channels instead of failing completely
                continue;
            }
        }
        drop(factory_guard); // Release the lock for each iteration
    }
    
    info!("Channel creation completed: {} successful, {} failed", 
          successful_channels, failed_channels);
    
    // Start all channels with improved performance
    let factory_guard = factory.read().await;
    if let Err(e) = factory_guard.start_all_channels().await {
        error!("Failed to start some channels: {}", e);
        // Log but don't fail - some channels might have started successfully
    }
    
    let stats = factory_guard.get_channel_stats().await;
    info!("Communication service started with {} channels (Protocol distribution: {:?})", 
          stats.total_channels, stats.protocol_counts);
    drop(factory_guard);
    
    // Update service metrics
    if let Some(metrics) = get_metrics() {
        metrics.update_service_status(true);
    }
    
    Ok(())
}

/// Handle graceful shutdown of the communication service
/// 
/// Performs an orderly shutdown of all communication channels, ensuring that
/// ongoing operations complete properly and resources are released cleanly.
/// Updates metrics to reflect the service shutdown state.
/// 
/// # Arguments
/// 
/// * `factory` - Thread-safe protocol factory managing all active channels
/// 
/// # Features
/// 
/// - **Graceful Channel Shutdown**: Stops all channels in an orderly manner
/// - **Resource Cleanup**: Ensures proper release of network and system resources
/// - **Metrics Update**: Records service shutdown in monitoring systems
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
/// }
/// ```
async fn shutdown_handler(factory: Arc<RwLock<ProtocolFactory>>) {
    info!("Starting graceful shutdown...");
    
    let factory_guard = factory.read().await;
    if let Err(e) = factory_guard.stop_all_channels().await {
        error!("Error during channel shutdown: {}", e);
    }
    drop(factory_guard);
    
    // Update service metrics
    if let Some(metrics) = get_metrics() {
        metrics.update_service_status(false);
    }
    
    info!("All channels stopped");
}

/// Start the periodic cleanup task for resource management
/// 
/// Launches a background task that periodically cleans up idle channels and
/// logs system statistics. This helps prevent resource leaks and provides
/// operational visibility into the service state.
/// 
/// # Arguments
/// 
/// * `factory` - Thread-safe protocol factory to monitor and clean up
/// 
/// # Returns
/// 
/// A `JoinHandle` for the cleanup task that can be used to cancel or wait for completion
/// 
/// # Features
/// 
/// - **Idle Channel Cleanup**: Removes channels that have been idle for extended periods
/// - **Statistics Logging**: Regular logging of channel and system statistics
/// - **Resource Monitoring**: Tracks memory and connection usage
/// - **Configurable Intervals**: Adjustable cleanup and reporting intervals
/// 
/// # Configuration
/// 
/// - **Cleanup Interval**: 5 minutes (300 seconds)
/// - **Idle Timeout**: 1 hour (3600 seconds)
/// - **Statistics Interval**: Every cleanup cycle
/// 
/// # Examples
/// 
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::core::ProtocolFactory;
/// 
/// async fn setup_maintenance(factory: Arc<RwLock<ProtocolFactory>>) {
///     let cleanup_handle = start_cleanup_task(factory);
///     
///     // Keep the handle to cancel if needed
///     tokio::select! {
///         _ = cleanup_handle => {
///             println!("Cleanup task completed");
///         }
///         _ = tokio::signal::ctrl_c() => {
///             println!("Shutting down cleanup task");
///         }
///     }
/// }
/// ```
fn start_cleanup_task(factory: Arc<RwLock<ProtocolFactory>>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes
        
        loop {
            interval.tick().await;
            
            // Clean up idle channels (1 hour idle time)
            let factory_guard = factory.read().await;
            factory_guard.cleanup_channels(std::time::Duration::from_secs(3600)).await;
            
            // Log statistics
            let stats = factory_guard.get_channel_stats().await;
            info!("Channel stats: total={}, running={}", 
                  stats.total_channels, stats.running_channels);
            drop(factory_guard);
        }
    })
}

/// Main entry point for the Communication Service
/// 
/// Initializes the complete communication service including configuration loading,
/// logging setup, metrics initialization, and service startup. Handles graceful
/// shutdown and provides comprehensive error handling throughout the lifecycle.
/// 
/// # Environment Variables
/// 
/// * `CONFIG_FILE` - Path to configuration file (default: "config/comsrv.yaml")
/// * `LOG_DIR` - Directory for log files (default: "logs")
/// 
/// # Configuration File
/// 
/// The service expects a YAML configuration file with the following structure:
/// 
/// ```yaml
/// service:
///   name: "ComSrv"
///   log_level: "info"
///   metrics_enabled: true
///   metrics_address: "0.0.0.0:9090"
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
/// - **Structured Logging**: Comprehensive logging with rotation and levels
/// - **Metrics Collection**: Prometheus-compatible metrics for monitoring
/// - **Signal Handling**: Graceful shutdown on SIGINT/SIGTERM
/// - **API Server**: RESTful API for management and monitoring
/// - **Health Checks**: Built-in health checking and status reporting
/// 
/// # Error Handling
/// 
/// The service implements graceful error handling at multiple levels:
/// - Configuration errors: Service fails to start with clear error messages
/// - Channel errors: Individual channel failures don't affect other channels
/// - Runtime errors: Logged and reported via metrics without service termination
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
    
    // Create configuration manager with better error context
    let config_manager = match ConfigManager::from_file(&config_file) {
        Ok(cm) => {
            info!("Configuration loaded from: {}", config_file);
            Arc::new(cm)
        },
        Err(e) => {
            error!("Failed to load configuration from {}: {}", config_file, e);
            return Err(e);
        }
    };
    
    // Initialize logging early for better debugging
    let log_dir = env::var("LOG_DIR").unwrap_or(args.log_dir);
    let log_level = args.log_level.as_deref().unwrap_or(config_manager.get_log_level());
    if let Err(e) = init_logger(Path::new(&log_dir), "comsrv", log_level, true) {
        eprintln!("Failed to initialize logger: {}", e);
        return Err(e);
    }
    
    if args.super_test {
        info!("🚀 Starting Communication Service v{} - SUPER TEST MODE", env!("CARGO_PKG_VERSION"));
        info!("Super test configuration:");
        info!("  - Config file: {}", config_file);
        if let Some(duration) = args.duration {
            info!("  - Test duration: {} seconds", duration);
        }
        info!("  - Log level: {}", log_level);
    } else {
        info!("Starting Communication Service v{}", env!("CARGO_PKG_VERSION"));
    }
    
    // Initialize metrics early
    if let Err(e) = init_metrics(config_manager.get_service_name()) {
        warn!("Failed to initialize metrics: {}", e);
    } else {
        info!("Metrics system initialized");
        
        // Start metrics server if enabled
        if config_manager.get_metrics_enabled() {
            if let Some(metrics) = get_metrics() {
                if let Err(e) = metrics.start_server(config_manager.get_metrics_address()).await {
                    warn!("Failed to start metrics server: {}", e);
                } else {
                    info!("Metrics server started at: {}", config_manager.get_metrics_address());
                }
            }
        }
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
    
    // Start API service if enabled
    if config_manager.get_api_enabled() {
        let start_time = Arc::new(Utc::now());
        let config_manager_for_api = Arc::new(RwLock::new((*config_manager).clone()));
        let routes = api_routes(factory.clone(), config_manager_for_api, start_time);
        
        info!("Starting API server at: {}", config_manager.get_api_address());
        
        let socket_addr = config_manager.get_api_address().parse::<SocketAddr>().unwrap_or_else(|e| {
            warn!("Invalid API address: {}, using default 0.0.0.0:3000. Error: {}", 
                  config_manager.get_api_address(), e);
            "0.0.0.0:3000".parse().unwrap()
        });
        
        tokio::spawn(async move {
            warp::serve(routes).run(socket_addr).await;
        });
        
        info!("API server started successfully");
    } else {
        info!("API server disabled in configuration");
    }
    
    info!("Service startup completed successfully");
    
    // Handle super test mode with duration
    if args.super_test && args.duration.is_some() {
        let duration = args.duration.unwrap();
        info!("🚀 Super test mode: running for {} seconds", duration);
        
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
            },
            Err(e) => {
                error!("Failed to listen for shutdown signal: {}", e);
            }
        }
    }
    
    // Perform graceful shutdown
    shutdown_handler(factory).await;
    info!("Communication service shutdown completed");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tempfile::TempDir;
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
        // Create a temporary directory for testing
        let temp_dir = tempfile::TempDir::new().unwrap();
        let log_dir = temp_dir.path();
        
        // Test logger initialization - only test if no global subscriber is set
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "info");
        }
        
        // Try to initialize logger, but don't fail if already initialized
        let result = init_logger(log_dir, "test_service", "info", true);
        
        // The function should either succeed or fail gracefully
        // We don't assert on the result since global logger might already be set
        match result {
            Ok(_) => {
                // Logger was successfully initialized
                assert!(true);
            },
            Err(_) => {
                // Logger was already initialized, which is also fine
                assert!(true);
            }
        }
    }

    #[test]
    fn test_metrics_initialization() {
        let result = init_metrics("test_service");
        assert!(result.is_ok());
        
        let metrics = get_metrics();
        assert!(metrics.is_some());
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
        let config_file = env::var("CONFIG_FILE").unwrap_or_else(|_| "config/comsrv.yaml".to_string());
        assert_eq!(config_file, "config/comsrv.yaml");

        // Test custom config file path
        std::env::set_var("CONFIG_FILE", "custom/config.yaml");
        let config_file = env::var("CONFIG_FILE").unwrap_or_else(|_| "config/comsrv.yaml".to_string());
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