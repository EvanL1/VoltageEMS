//! Core Communication Service Components
//! 
//! This module contains the core functionality of the communication service,
//! including protocol implementations, configuration management, connection
//! pooling, and factory patterns for creating protocol instances.
//! 
//! # Architecture
//! 
//! The core module is organized into several key components:
//! 
//! - **`config`** - Configuration management and validation, including enhanced point table management
//! - **`protocols`** - Protocol implementations (Modbus RTU/TCP, IEC60870, etc.)
//! - **`storage`** - Data storage and caching mechanisms
//! - **`connection_pool`** - Connection pooling for network efficiency
//! - **`protocol_factory`** - Factory pattern for creating protocol instances
//! - **`metrics`** - Performance monitoring and metrics collection
//! - **`monitoring`** - Real-time monitoring and alerting for protocols
//! 
//! # Design Principles
//! 
//! ## Async-First
//! 
//! All core components are designed with async/await in mind for maximum
//! concurrency and performance.
//! 
//! ## Protocol Agnostic
//! 
//! The core provides a unified interface for all protocols through the
//! ComBase trait, allowing protocols to be treated uniformly.
//! 
//! ## Configuration Driven
//! 
//! All behavior is controlled through configuration files, enabling
//! runtime customization without code changes.
//! 
//! ## Business Layer Focus
//! 
//! Enhanced with business-level features including:
//! - Intelligent point table management with optimization
//! - Real-time monitoring and diagnostics
//! - Advanced channel configuration management
//! - Performance analytics and reporting
//! 
//! # Example Usage
//! 
//! ```rust
//! use comsrv::{ConfigManager, ProtocolFactory, EnhancedPointTableManager, RtuMonitor};
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//! 
//! async fn setup_enhanced_service() -> comsrv::Result<()> {
//!     // Load configuration
//!     let config_manager = ConfigManager::from_file("config/comsrv.yaml")?;
//!     
//!     // Create enhanced point table manager
//!     let point_table_manager = Arc::new(EnhancedPointTableManager::new(
//!         "config/points",
//!         Default::default(),
//!     ));
//!     point_table_manager.start().await?;
//!     
//!     // Create RTU monitor for real-time monitoring
//!     let rtu_monitor = Arc::new(RtuMonitor::new(Default::default()));
//!     rtu_monitor.start().await?;
//!     
//!     // Create protocol factory with enhanced capabilities
//!     let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
//!     
//!     // Register channels from configuration with enhanced point tables
//!     for channel_config in config_manager.get_channels() {
//!         let channel = factory.write().await.create_enhanced_channel(
//!             channel_config.clone(),
//!             point_table_manager.clone(),
//!             rtu_monitor.clone(),
//!         )?;
//!     }
//!     
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod protocols;
pub mod storage;
pub mod connection_pool;
pub mod protocol_factory;
pub mod metrics;
pub mod monitoring;

// Re-export commonly used protocol components for public API
pub use protocols::common::{ComBase, ComBaseImpl, ChannelStatus, PointData};

// Re-export enhanced components for business layer
pub use config::point_table::{
    PointTableConfig,
    PointTableStats,
    PointTableOptimization,
    OptimizationType,
};

pub use monitoring::rtu_monitor::{
    RtuMonitor,
    RtuMonitorConfig,
    RtuMonitorStatus,
    RtuAlarm,
    RtuAlarmType,
    RtuAlarmSeverity,
    RtuMonitorReport,
};

pub use protocols::modbus::{
    ModbusClient,
    ModbusCommunicationMode,
    ModbusClientConfig,
    ModbusClientStats,
    ModbusConnectionState,
}; 