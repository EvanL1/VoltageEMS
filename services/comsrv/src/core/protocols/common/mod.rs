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
//! 
//! These components provide the foundation for all protocol implementations
//! and ensure consistency across different protocol types.

pub mod combase;
pub mod protocol_factory;
pub mod connection_pool;

// Re-export commonly used items
pub use combase::*;
pub use protocol_factory::*;
pub use connection_pool::*;