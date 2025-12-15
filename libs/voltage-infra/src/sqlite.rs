//! SQLite client module
//!
//! Provides SQLite client with optimized settings for edge deployment.

pub mod client;
pub mod service_config;

pub use client::{SqliteClient, SqlitePool};
pub use service_config::{migrate_yaml_to_db, ServiceConfig, ServiceConfigLoader};
