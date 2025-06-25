//! Common Protocol Components
//!
//! This module contains shared components used across all protocol implementations,
//! including the base communication interface, protocol factory, connection pooling,
//! and other common utilities.
//!
//! # Components
//!
//! - **combase**: Base communication interface and traits
//! - **protocol_factory**: Factory pattern for creating protocol instances
//! - **connection_pool**: Connection pooling and management
//! - **stats**: Unified statistics structures for all protocols
//! - **errors**: Common error types and categories
//!
//! Note: Configuration structures have been moved to `core::config::protocol_config`
//! to avoid duplication and provide a unified configuration management system.

pub mod combase;
pub mod connection_pool;
pub mod errors;
pub mod protocol_factory;
pub mod stats;

// Re-export commonly used items (avoiding duplicates)
pub use combase::ComBase;
pub use protocol_factory::*;

// Configuration is now managed centrally in core::config
// Use: `use crate::core::config::{BaseCommConfig, NetworkConfig, SerialConfig};`
