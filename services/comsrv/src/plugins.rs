//! Protocol Plugin System
//!
//! Provides a flexible plugin architecture, supporting dynamic loading,
//! configuration management and standardized interfaces for protocol implementations

pub mod common;
pub mod protocols;

// Re-export common utilities
pub use common::{telemetry_type_to_redis, PluginPointUpdate};
