//! Common Protocol Module
//!
//! This module provides common functionality for all communication protocols.
//! Consolidated and simplified from the original combase structure.

// Core modules
pub mod data_types;
pub mod manager;
pub mod redis;
pub mod traits;

// Legacy combase module (to be removed after migration)
pub mod combase;

// Re-export commonly used types
pub use data_types::*;
pub use manager::{OptimizedPointManager, PointManagerStats};
pub use redis::{RedisBatchSync, RedisBatchSyncConfig, RedisSyncStats};
pub use traits::{
    ComBase, ConfigValidator, ConnectionManager, FourTelemetryOperations, ProtocolPacketParser,
};

// Legacy re-exports for backward compatibility
pub use combase::ProtocolFactory;

/// Initialize common protocol functionality
pub fn init_common_protocols() {
    tracing::info!("Common protocol functionality initialized");
}
