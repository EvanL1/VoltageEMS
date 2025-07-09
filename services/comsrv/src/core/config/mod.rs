//! # Configuration Management Module
//!
//! This module provides configuration management for the communication service.
//!
//! ## Features
//!
//! - **Multi-format support**: YAML, TOML, JSON auto-detection
//! - **Type-safe**: Compile-time validation
//! - **CSV point tables**: Support for loading point definitions from CSV files
//! - **Environment variables**: Override configuration with environment variables
//!
//! ## Architecture
//!
//! ```
//! ConfigManager
//!   ├── Service Configuration
//!   ├── Channel Configuration
//!   └── Point Tables (CSV)
//! ```

// Core modules
pub mod config_center;
pub mod config_manager;
pub mod loaders;
pub mod point;
pub mod types;
pub mod unified_loader;

// Re-export ConfigManager
pub use config_manager::ConfigManager;

// Re-export types
pub use types::{
    ApiConfig, AppConfig, ChannelConfig, ChannelLoggingConfig, ChannelParameters, DataType,
    FourTelemetryFiles, ProtocolAddress, ProtocolType, RedisConfig, ScalingConfig, ServiceConfig,
    TelemetryType, UnifiedPointMapping, ValidationConfig,
};

// Re-export point
pub use point::Point;

// Re-export all loaders
pub use loaders::*;
