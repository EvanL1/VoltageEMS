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
//! - **Configuration**: YAML-based configuration with hot-reload support
//! - **Point Tables**: CSV-based point table management with dynamic loading
//! - **REST API**: RESTful API for monitoring and control
//! - **Storage**: Optional Redis integration for data persistence
//! - **Logging**: Structured logging with configurable levels
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
//! # Quick Start
//!
//! ```
//! use comsrv::{ConfigManager, ProtocolFactory};
//! use comsrv::utils::Result;
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Load configuration
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
//!     println!("Communication service initialized");
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
//! CONFIG_FILE=my_config.yaml cargo run --bin comsrv
//!
//! # Start with debug logging
//! RUST_LOG=debug cargo run --bin comsrv
//! ```
//!
//! ## Environment Variables
//!
//! - `CONFIG_FILE`: Path to configuration file (default: "config/comsrv.yaml")
//! - `RUST_LOG`: Log level for env_logger (debug, info, warn, error)
//!
//! ## Configuration
//!
//! Configuration is managed through YAML files with the following structure:
//!
//! ```yaml
//! service:
//!   name: "ComsrvRust"
//!   logging:
//!     level: "info"
//!   api:
//!     enabled: true
//!     bind_address: "0.0.0.0:3000"
//!
//! channels:
//!   - id: 1
//!     name: "Modbus Device 1"
//!     protocol: "ModbusTcp"
//!     parameters:
//!       host: "192.168.1.100"
//!       port: 502
//!       slave_id: 1
//!
//! redis:
//!   enabled: false
//!   connection_type: "Tcp"
//!   address: "127.0.0.1:6379"
//! ```
//!
//! # Protocol Support
//!
//! ## Modbus
//!
//! Full support for Modbus TCP with the following features:
//! - All standard function codes (read/write coils, discrete inputs, holding/input registers)
//! - Advanced data types (bool, int16/32/64, uint16/32/64, float32/64, strings)
//! - Byte order handling for multi-register values
//! - Automatic register grouping for optimized batch reads
//! - Connection retry and heartbeat monitoring
//!
//! ## IEC60870-5-104
//!
//! IEC60870-5-104 telecontrol protocol support:
//! - General interrogation and cyclic data transmission
//! - Command transmission (single/double point, regulating step)
//! - File transfer capabilities
//! - Event-driven and polled data acquisition
//!
//! # Error Handling
//!
//! The library uses a comprehensive error type [`ComSrvError`] that covers all
//! possible error conditions. All operations return `Result<T, ComSrvError>` for
//! consistent error handling.
//!
//! # Performance
//!
//! The library is designed for high performance:
//! - Async/await throughout for maximum concurrency
//! - Connection pooling to minimize overhead
//! - Batch operations for improved network efficiency
//! - Lock-free data structures where possible
//! - Efficient memory management and resource cleanup

pub mod api;
/// Communication Service Library
/// Provides core functionality for protocol communication, data exchange, and management
pub mod core;
mod service_impl;
pub mod utils;

/// Service entry point and lifecycle management
///
/// This module contains the main service functions and lifecycle management
/// utilities that are used by the binary executable. These functions are
/// also exposed here for library users who want to embed the service.
///
/// # Service Functions
///
/// The main service functions provide complete lifecycle management:
/// - Service initialization and startup
/// - Graceful shutdown handling
/// - Background task management
/// - Configuration and environment handling
///
/// # Examples
///
/// ## Embedding the Service
///
/// ```rust,no_run
/// use comsrv::service::{start_communication_service, shutdown_handler};
/// use comsrv::{ConfigManager, ProtocolFactory};
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
///
/// #[tokio::main]
/// async fn main() -> comsrv::Result<()> {
///     // Initialize service components
///     let config_manager = Arc::new(ConfigManager::from_file("config.yaml")?);
///     let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
///     
///     // Start the communication service
///     start_communication_service(config_manager, factory.clone()).await?;
///     
///     // Setup shutdown handling
///     tokio::signal::ctrl_c().await.unwrap();
///     shutdown_handler(factory).await;
///     
///     Ok(())
/// }
/// ```
///
/// ## Custom Service Integration
///
/// ```rust,no_run
/// use comsrv::service::start_cleanup_task;
/// use comsrv::ProtocolFactory;
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
///
/// async fn setup_monitoring(factory: Arc<RwLock<ProtocolFactory>>) {
///     // Start background cleanup task
///     let cleanup_handle = start_cleanup_task(factory);
///     
///     // Your custom monitoring logic here
///     tokio::spawn(async move {
///         // Custom monitoring implementation
///     });
/// }
/// ```
pub mod service {
    //! Service lifecycle management and main entry points
    //!
    //! This module provides the core service functions that manage the complete
    //! lifecycle of the communication service, from initialization to shutdown.

    use crate::service_impl as impls;
    use crate::utils::Result;
    use crate::{ConfigManager, ProtocolFactory};
    use std::sync::Arc;
    use tokio::sync::RwLock;

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
        impls::start_communication_service(config_manager, factory).await
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
        impls::shutdown_handler(factory).await;
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
        impls::start_cleanup_task(factory)
    }
}

// Re-export commonly used types and traits
pub use core::config::config_manager::ConfigManager;
pub use core::protocols::common::combase::{ChannelStatus, ComBase, ComBaseImpl, PointData};
pub use core::protocols::common::ProtocolFactory;
pub use utils::{ComSrvError, Result};
