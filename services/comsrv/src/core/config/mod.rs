//! Configuration Management Module
//! 
//! This module provides comprehensive configuration management for the communication service.
//! It includes business-level configuration management, point table management, and 
//! protocol-specific configuration structures.
//! 
//! # Architecture
//! 
//! - **config_manager**: Main configuration manager for service and channel configuration
//! - **point_table**: Point table management with CSV/YAML support and optimization
//! - **csv_parser**: CSV point table parsing and management
//! - **protocol_config**: Protocol-specific configuration structures (BaseCommConfig, NetworkConfig, etc.)
//! 
//! # Usage
//! 
//! ```rust
//! use comsrv::core::config::{ConfigManager, PointTableManager, ModbusTcpConfig};
//! 
//! // Load main configuration
//! let config_manager = ConfigManager::from_file("config/comsrv.yaml")?;
//! 
//! // Create point table manager
//! let point_table_manager = PointTableManager::new("config/points");
//! 
//! // Create protocol-specific configuration
//! let modbus_config = ModbusTcpConfig::new("192.168.1.100".to_string(), 502, 1);
//! ```

pub mod config_manager;
pub mod point_table;
pub mod csv_parser;
pub mod protocol_config;

// Re-export main configuration components
pub use config_manager::*;
pub use csv_parser::CsvPointRecord;
pub use point_table::*;

// Re-export protocol configuration components
pub use protocol_config::{
    BaseCommConfig, ConnectionPoolConfig, NetworkConfig, SerialConfig,
    DataBits, StopBits, Parity, FlowControl,
    ModbusConfig, ModbusTcpConfig, ModbusRtuConfig,
};
 