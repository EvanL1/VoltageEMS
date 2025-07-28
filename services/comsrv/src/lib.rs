//! Communication Service Library (`ComsrvRust`)
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
//! - **REST API**: `RESTful` API built with axum
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
/// Service layer for lifecycle management and reconnection
pub mod service;
/// Storage module for flat key-value storage
pub mod storage;
/// Utility functions
pub mod utils;

/// Error handling is now in utils module
pub use utils::error;

// Re-export commonly used types and traits
pub use api::routes::{get_service_start_time, set_service_start_time};
pub use core::combase::{ChannelStatus, ComBase, DefaultProtocol, PointData, ProtocolFactory};
pub use core::config::ConfigManager;
pub use service::{shutdown_handler, start_cleanup_task, start_communication_service};
pub use utils::error::{ComSrvError, Result};

// #[cfg(test)]
// mod test_plugin_debug; // Moved to tests directory
