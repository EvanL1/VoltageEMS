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
pub mod types;

// Re-export from new modules
pub use manager::*;
pub use sqlite_loader::ComsrvSqliteLoader;
pub use types::*;
