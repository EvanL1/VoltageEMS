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

pub mod core;
pub mod loaders;
pub mod parameters;
pub mod point;
pub mod types;

// Re-export from new modules
pub use core::*;
pub use loaders::*;
pub use parameters::*;
pub use point::Point;
pub use types::*;
