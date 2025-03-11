//! Runtime Orchestration Layer
//!
//! Provides runtime lifecycle management, service orchestration, reconnection mechanisms,
//! and maintenance tasks for the communication service

pub mod lifecycle;
pub mod reconnect;

// Re-export common types
pub use lifecycle::{shutdown_handler, start_cleanup_task, start_communication_service};
pub use reconnect::{ReconnectContext, ReconnectError, ReconnectHelper, ReconnectPolicy};
