//! `VoltageEMS` basic library (basic library)
//!
//! Provides basic functions shared by all services, including:
//! - Redis client
//! - monitoring and health checking
//! - error processing
//! - logging functions

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(feature = "sqlite")]
pub mod sqlite;

// Common modules
pub mod config_loader;
pub mod error;
pub mod logging;
pub mod service_bootstrap;
pub mod system_metrics;
pub mod warning_monitor;

// Re-export commonly used csv types (previously in csv.rs module)
pub use csv::{Reader, ReaderBuilder, StringRecord, Writer, WriterBuilder};

// Bootstrap modules
pub mod bootstrap_args;
pub mod bootstrap_database;
pub mod bootstrap_system;
pub mod bootstrap_validation;

// Test utilities (for use in test code only)
pub mod test_utils;

// Re-export common dependencies
pub use anyhow;
pub use serde;
pub use serde_json;
pub use tokio;

// Re-export CLI dependencies when cli feature is enabled
#[cfg(feature = "cli")]
pub use clap;

// Re-export clap derive macros separately for proper macro resolution
#[cfg(feature = "cli")]
pub use clap::{Args, Parser, Subcommand, ValueEnum};

#[cfg(feature = "cli")]
pub use reqwest;

// Pre-import common types
pub mod prelude {
    pub use crate::error::{Error, Result};

    #[cfg(feature = "redis")]
    pub use crate::redis::RedisClient;
}
