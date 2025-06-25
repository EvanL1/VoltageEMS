//! Configuration Management Module
//!
//! This module provides comprehensive configuration management for the communication service.
//! It includes business-level configuration management, point table management, and
//! protocol-specific configuration structures.
//!
//! # Architecture
//!
//! - **config_manager**: Main configuration manager for service and channel configuration
//! - **protocol_table_manager**: Protocol-agnostic table manager with four telemetry types support (遥测/遥信/遥控/遥调)
//! - **protocol_config**: Protocol-specific configuration structures (BaseCommConfig, NetworkConfig, etc.)
//!
//! # Usage
//!
//! ```rust
//! use comsrv::core::config::{ConfigManager, PointTableManager};
//!
//! // Load main configuration
//! let config_manager = ConfigManager::from_file("config/comsrv.yaml")?;
//!
//! // Create point table manager
//! let point_table_manager = PointTableManager::new("config/points");
//! ```

pub mod config_manager;
pub mod protocol_table_manager;
pub mod protocol_config;

// Re-export commonly used types
pub use config_manager::{ChannelConfig, ConfigManager, ProtocolType};
pub use protocol_table_manager::{
    ChannelPointRecord, DataPoint, FourTelemetryStatistics, FourTelemetryTableManager, ProtocolConfigRecord,
    TelemetryCategory,
};

// Re-export protocol configuration types
pub use protocol_config::{
    BaseCommConfig, ConnectionPoolConfig, DataBits, FlowControl, ModbusConfig, ModbusRtuConfig,
    ModbusTcpConfig, NetworkConfig, Parity, SerialConfig, StopBits,
};

// Re-export main configuration components
pub use config_manager::*;
