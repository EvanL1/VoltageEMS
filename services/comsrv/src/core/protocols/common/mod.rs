//! Common Protocol Components
//!
//! This module contains shared components used across all protocol implementations,
//! including the base communication interface, protocol factory, and other utilities.
//!
//! # Components
//!
//! - **combase**: Base communication interface and traits
//! - **protocol_factory**: Factory pattern for creating protocol instances
//! - **stats**: Unified statistics structures for all protocols
//!
//! Note: Configuration structures have been moved to `core::config::protocol_config`
//! to avoid duplication and provide a unified configuration management system.

pub mod combase;

// Re-export commonly used items (avoiding duplicates)
pub use combase::{ComBase, ProtocolFactory};

// Configuration is now managed centrally in core::config
// Use: `use crate::core::config::{BaseCommConfig, NetworkConfig, SerialConfig};`
