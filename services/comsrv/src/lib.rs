//! Communication Service Library (ComsrvRust)
//!
//! A high-performance, async-first industrial communication service written in Rust.
//! This library provides a unified interface for communicating with various industrial
//! protocols including Modbus TCP/RTU, IEC60870-5-104, and more.
//!
//! # Features
//!
//! - **Multi-Protocol Support**: Modbus TCP/RTU, IEC60870-5-104, and extensible protocol framework
//! - **High Performance**: Async/await throughout, connection pooling, and optimized batch operations  
//! - **Reliability**: Automatic retry logic, heartbeat monitoring, and comprehensive error handling
//! - **Configuration**: YAML-based configuration with hot-reload support and environment overrides
//! - **Point Tables**: CSV-based point table management with dynamic loading and four telemetry types
//! - **REST API**: RESTful API built with axum
//! - **Storage**: Optional Redis integration for data persistence and caching
//! - **Logging**: Structured logging with tracing instead of traditional log framework
//!
//! # Architecture
//!
//! The library is organized into several main modules:
//!
//! - **`core`**: Core functionality including protocol implementations, configuration management, and factories
//! - **`utils`**: Utility functions, error handling, and shared components  
//! - **`api`**: REST API endpoints and request/response models
//! - **`service`**: Main service entry point and lifecycle management
//!
//! ## Service Architecture
//!
//! ```text
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │   Config Mgr    │───►│ Protocol Factory│───►│   Channels      │
//! │ (YAML+Figment)  │    │   (Multi-proto) │    │  (TCP/RTU/...)  │
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//!          │                       │                       │
//!          ▼                       ▼                       ▼
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │     tracing     │    │   Redis Store   │    │   Axum Server   │
//! │  (Structured)   │    │  (Optional)     │    │    (REST)       │
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use comsrv::{ConfigManager, ProtocolFactory};
//! use comsrv::utils::Result;
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Initialize tracing
//!     tracing_subscriber::fmt::init();
//!     
//!     // Load configuration with Figment (supports YAML, TOML, JSON, env vars)
//!     let config_manager = ConfigManager::from_file("config/comsrv.yaml")?;
//!     
//!     // Create protocol factory
//!     let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
//!     
//!     // Initialize channels from configuration
//!     let channels = config_manager.get_channels();
//!     for channel_config in channels {
//!         // Create and register channel
//!         factory.write().await.create_channel(channel_config.clone())?;
//!     }
//!     
//!     tracing::info!("Communication service initialized");
//!     Ok(())
//! }
//! ```
//!
//! # Service Deployment
//!
//! ## Command Line Usage
//!
//! The service can be started with various options:
//!
//! ```bash
//! # Start with default configuration
//! cargo run --bin comsrv
//!
//! # Start with custom configuration file
//! COMSRV__CONFIG_FILE=my_config.yaml cargo run --bin comsrv
//!
//! # Start with debug logging  
//! RUST_LOG=debug cargo run --bin comsrv
//! ```
//!
//! ## Environment Variables
//!
//! The service supports comprehensive environment variable configuration with the `COMSRV__` prefix:
//!
//! - `COMSRV__SERVICE__NAME`: Service name override
//! - `COMSRV__SERVICE__API__BIND_ADDRESS`: API server bind address
//! - `COMSRV__SERVICE__REDIS__URL`: Redis connection URL
//! - `RUST_LOG`: Log level for tracing (trace, debug, info, warn, error)
//!
//! ## Configuration
//!
//! Configuration uses Figment for flexible multi-source loading with the following structure:
//!
//! ```yaml
//! service:
//!   name: "ComsrvRust"
//!   description: "Industrial Communication Service"
//!   api:
//!     enabled: true
//!     bind_address: "0.0.0.0:3000"
//!     version: "v1"
//!   logging:
//!     level: "info"
//!     console: true
//!   redis:
//!     enabled: false
//!     url: "redis://127.0.0.1:6379"
//!     database: 0
//!
//! channels:
//!   - id: 1
//!     name: "Modbus Device 1"
//!     protocol: "ModbusTcp"
//!     parameters:
//!       host: "192.168.1.100"
//!       port: 502
//!       slave_id: 1
//!     table_config:
//!       four_telemetry_route: "channels/modbus1"
//!       protocol_mapping_route: "mappings/modbus1"
//! ```
//!
//! # Protocol Support
//!
//! ## Modbus
//!
//! Full support for Modbus TCP/RTU with the following features:
//! - All standard function codes (read/write coils, discrete inputs, holding/input registers)
//! - Advanced data types (bool, int16/32/64, uint16/32/64, float32/64, strings)
//! - Byte order handling for multi-register values (ABCD, CDBA, BADC, DCBA)
//! - Automatic register grouping for optimized batch reads
//! - Connection retry and heartbeat monitoring
//! - Forward calculation engine for computed points
//!
//! ## IEC60870-5-104
//!
//! IEC60870-5-104 telecontrol protocol support:
//! - General interrogation and cyclic data transmission
//! - Command transmission (single/double point, regulating step)
//! - File transfer capabilities
//! - Event-driven and polled data acquisition
//!
//! # API Documentation
//!
//! The service provides a REST API:
//!
//! - **Framework**: Built with axum for high performance
//! - **Endpoints**: Channel management, point reading/writing, status monitoring
//!
//! Key API endpoints:
//! - `GET /api/v1/status` - Service status and health
//! - `GET /api/v1/channels` - List all channels  
//! - `GET /api/v1/channels/{id}/points` - Get channel point data
//! - `POST /api/v1/channels/{id}/points/{point_id}/write` - Write point value
//!
//! # Error Handling
//!
//! The library uses a comprehensive error type [`ComSrvError`] that covers all
//! possible error conditions. All operations return `Result<T, ComSrvError>` for
//! consistent error handling.
//!
//! Error types include:
//! - Configuration errors (YAML parsing, validation)
//! - Protocol errors (Modbus exceptions, IEC104 failures)
//! - Network errors (connection timeouts, DNS resolution)
//! - Storage errors (Redis connectivity, serialization)
//!
//! # Performance & Reliability
//!
//! The library is designed for high performance and reliability:
//! - **Async/await throughout** for maximum concurrency
//! - **Connection pooling** to minimize overhead  
//! - **Batch operations** for improved network efficiency
//! - **Structured logging** with tracing for observability
//! - **Graceful error handling** and automatic recovery
//! - **Resource cleanup** and proper lifecycle management
//!
//! # Storage Integration
//!
//! Optional Redis integration provides:
//! - Real-time data caching and persistence
//! - Channel metadata storage
//! - Configuration data backup
//! - Command queuing and result tracking
//! - Pub/sub messaging for distributed scenarios

pub mod api;
/// Core functionality for protocol communication, data exchange, and management
pub mod core;
/// Plugin system for protocol implementations
pub mod plugins;
/// Service implementation
/// Storage module for flat key-value storage
pub mod storage;
/// Utility functions
pub mod utils;

/// Error handling is now in utils module
pub use utils::error;

pub mod service {

    use crate::core::combase::factory::ProtocolFactory;
    use crate::core::config::ConfigManager;
    use crate::utils::Result;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tracing::{error, info, warn};

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
    /// # Service Architecture
    ///
    /// ```text
    /// ┌─────────────────────┐    ┌─────────────────────┐
    /// │   Configuration     │───►│  Channel Factory    │
    /// │   Manager           │    │                     │
    /// └─────────────────────┘    └─────────────────────┘
    ///           │                           │
    ///           ▼                           ▼
    /// ┌─────────────────────┐    ┌─────────────────────┐
    /// │   Channel Config    │───►│  Protocol Channels  │
    /// │   Validation        │    │  (Modbus/IEC/...)   │
    /// └─────────────────────┘    └─────────────────────┘
    ///                                       │
    ///                                       ▼
    ///                           ┌─────────────────────┐
    ///                           │   Metrics & Status  │
    ///                           │   Monitoring        │
    ///                           └─────────────────────┘
    /// ```
    ///
    /// # Error Handling
    ///
    /// The function implements robust error handling:
    /// - Individual channel failures don't stop service startup
    /// - Detailed error logging with context
    /// - Metrics recording for failed operations
    /// - Graceful degradation with partial functionality
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    /// use comsrv::service::start_communication_service;
    /// use comsrv::{ConfigManager, ProtocolFactory};
    ///
    /// #[tokio::main]
    /// async fn main() -> comsrv::Result<()> {
    ///     let config_manager = Arc::new(ConfigManager::from_file("config.yaml")?);
    ///     let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
    ///     
    ///     start_communication_service(config_manager, factory).await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// This function provides a convenient public interface for starting the communication service.
    pub async fn start_communication_service(
        config_manager: Arc<ConfigManager>,
        factory: Arc<RwLock<ProtocolFactory>>,
    ) -> Result<()> {
        info!("DEBUG: start_communication_service called");

        // Get channel configurations
        let configs = config_manager.channels().to_vec();

        if configs.is_empty() {
            warn!("No channels configured");
            return Ok(());
        }

        info!("Creating {} channels...", configs.len());

        // Create channels with improved error handling and metrics
        let mut successful_channels = 0;
        let mut failed_channels = 0;

        for channel_config in configs {
            info!(
                "Creating channel: {} - {}",
                channel_config.id, channel_config.name
            );

            let factory_guard = factory.write().await;
            match factory_guard
                .create_channel(&channel_config, Some(&*config_manager))
                .await
            {
                Ok(_) => {
                    info!("Channel created successfully: {}", channel_config.id);
                    successful_channels += 1;
                }
                Err(e) => {
                    error!("Failed to create channel {}: {e}", channel_config.id);
                    failed_channels += 1;

                    // Continue with other channels instead of failing completely
                    continue;
                }
            }
            drop(factory_guard); // Release the lock for each iteration
        }

        info!(
            "Channel initialization completed: {} successful, {} failed",
            successful_channels, failed_channels
        );

        // 等待短暂时间确保所有通道初始化完成
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // 第二阶段：批量建立所有通道的连接
        info!("Starting connection phase for all initialized channels...");
        let factory_guard = factory.read().await;
        match factory_guard.connect_all_channels().await {
            Ok(_) => {
                info!("All channel connections completed successfully");
            }
            Err(e) => {
                error!("Some channel connections failed: {}", e);
                // 连接失败不应阻止服务启动，继续运行
            }
        }
        drop(factory_guard);

        info!(
            "Communication service started with {} channels successfully initialized",
            successful_channels
        );

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
    /// # Shutdown Process
    ///
    /// ```text
    /// ┌─────────────────────┐
    /// │  Shutdown Signal    │
    /// │  Received           │
    /// └─────────────────────┘
    ///           │
    ///           ▼
    /// ┌─────────────────────┐
    /// │  Stop All Channels  │
    /// │  (Async)            │
    /// └─────────────────────┘
    ///           │
    ///           ▼
    /// ┌─────────────────────┐
    /// │  Update Metrics     │
    /// │  (Service Stopped)  │
    /// └─────────────────────┘
    ///           │
    ///           ▼
    /// ┌─────────────────────┐
    /// │  Cleanup Resources  │
    /// │  Complete           │
    /// └─────────────────────┘
    /// ```
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    /// use comsrv::service::shutdown_handler;
    /// use comsrv::ProtocolFactory;
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
    ///
    /// This function provides a convenient public interface for graceful service shutdown.
    pub async fn shutdown_handler(factory: Arc<RwLock<ProtocolFactory>>) {
        info!("Starting graceful shutdown...");

        // Get all channel IDs
        let channel_ids = {
            let factory_guard = factory.read().await;
            factory_guard.get_channel_ids()
        };

        // Remove all channels
        for channel_id in channel_ids {
            let factory_guard = factory.write().await;
            if let Err(e) = factory_guard.remove_channel(channel_id).await {
                error!("Error stopping channel {}: {}", channel_id, e);
            }
            drop(factory_guard);
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
    /// # Task Lifecycle
    ///
    /// ```text
    /// ┌─────────────────────┐
    /// │  Task Started       │
    /// └─────────────────────┘
    ///           │
    ///           ▼
    /// ┌─────────────────────┐    ┌─────────────────────┐
    /// │  Wait 5 Minutes     │◄───│  Cleanup Cycle      │
    /// └─────────────────────┘    │  Complete           │
    ///           │                └─────────────────────┘
    ///           ▼                           ▲
    /// ┌─────────────────────┐               │
    /// │  Cleanup Idle       │               │
    /// │  Channels           │               │
    /// └─────────────────────┘               │
    ///           │                           │
    ///           ▼                           │
    /// ┌─────────────────────┐               │
    /// │  Log Statistics     │───────────────┘
    /// └─────────────────────┘
    /// ```
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    /// use comsrv::service::start_cleanup_task;
    /// use comsrv::ProtocolFactory;
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
    ///
    /// This function provides a convenient public interface for resource cleanup management.
    pub fn start_cleanup_task(
        factory: Arc<RwLock<ProtocolFactory>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                // Clean up idle channels (1 hour idle time)
                let factory_guard = factory.read().await;

                // Log statistics
                let all_stats = factory_guard.get_all_channel_stats().await;
                info!(
                    "Channel stats: total={}, active={}",
                    all_stats.len(),
                    all_stats.iter().filter(|s| s.is_connected).count()
                );
                drop(factory_guard);
            }
        })
    }
}

// Re-export commonly used types and traits
pub use api::routes::{get_service_start_time, set_service_start_time};
pub use core::combase::{ChannelStatus, ComBase, DefaultProtocol, PointData, ProtocolFactory};
pub use core::config::ConfigManager;
pub use utils::error::{ComSrvError, Result};

// #[cfg(test)]
// mod test_plugin_debug; // Moved to tests directory
