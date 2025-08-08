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
//! ```text
//! ConfigManager
//!   ├── Service Configuration
//!   ├── Channel Configuration
//!   └── Point Tables (CSV)
//! ```

#![allow(ambiguous_glob_reexports)]

pub mod loaders;
pub mod manager;
pub mod parameters;
pub mod point;
pub mod types;

// Re-export from new modules
pub use loaders::*;
pub use manager::*;
pub use parameters::*;
pub use point::Point;
pub use types::*;
