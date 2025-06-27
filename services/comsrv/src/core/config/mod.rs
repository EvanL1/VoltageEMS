//! # Modern Configuration Management Module
//!
//! This module provides streamlined configuration management using Figment,
//! replacing the previous complex manual configuration system.
//!
//! ## Features
//!
//! - **Multi-source configuration**: File → Environment → CLI arguments
//! - **Hot reload**: Runtime configuration updates
//! - **Type-safe**: Compile-time validation
//! - **Format support**: YAML, TOML, JSON auto-detection
//! - **90% code reduction**: From 6000+ lines to ~500 lines
//!
//! ## Usage
//!
//! ```rust
//! use comsrv::core::config::{ConfigManager, ConfigBuilder};
//!
//! // Load configuration with defaults and environment variables
//! let config_manager = ConfigManager::from_file("config/comsrv.yaml")?;
//!
//! // Access configuration
//! let service_name = config_manager.get_service_name();
//! let api_config = config_manager.get_api_config();
//! ```

pub mod config_manager;
pub mod types;

// Re-export essential types for backward compatibility
pub use types::{
    ChannelConfig, ChannelParameters, ProtocolType, RedisConfig,
    TelemetryType, CombasePointConfig, AnalogPointConfig, DigitalPointConfig,
    FourTelemetryTableManager,
};

// Re-export the modern ConfigManager
pub use config_manager::{ConfigManager, AppConfig, ServiceConfig, ApiConfig, LoggingConfig, ConfigBuilder};
