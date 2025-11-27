//! # Configuration Management Module
//!
//! This module provides configuration management for the communication service.
//!
//! ## Features
//!
//! - **Multi-format support**: YAML, TOML, JSON auto-detection
//! - **Type-safe**: Compile-time validation
//! - **CSV point tables**: Support for loading point definitions from CSV files
//! - **SQLite database**: Support for loading configuration from SQLite
//! - **Environment variables**: Override configuration with environment variables
//!
//! ## Architecture
//!
//! ```text
//! ConfigManager
//!   ├── Service Configuration
//!   ├── Channel Configuration
//!   └── Point Tables (CSV/SQLite)
//! ```

#![allow(ambiguous_glob_reexports)]

pub mod manager;
pub mod sqlite_loader;

// Re-export from modules
pub use manager::*;
pub use sqlite_loader::ComsrvSqliteLoader;

// Re-export comsrv configuration types from voltage-config
pub use voltage_config::comsrv::{
    AdjustmentPoint, CanMapping, ChannelConfig, ChannelLoggingConfig, ComsrvConfig, ControlPoint,
    ModbusMapping, Point, RuntimeChannelConfig, SignalPoint, TelemetryPoint, VirtualMapping,
};

// Re-export common configuration types
pub use voltage_config::common::{
    ApiConfig, BaseServiceConfig, FourRemote, LoggingConfig, RedisConfig,
};

// Legacy aliases for backward compatibility
pub type AppConfig = ComsrvConfig;
pub type ServiceConfig = BaseServiceConfig;
