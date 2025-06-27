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

#[cfg(test)]
pub mod redis_source_test;
pub mod protocol_config;
pub mod forward_calculation_config;
pub mod storage;
pub mod test_refactor;

// Re-export commonly used types
pub use config_manager::{ConfigManager};

// Re-export storage backends

// Legacy re-exports for backward compatibility
