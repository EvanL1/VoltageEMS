//! Common Protocol Module
//!
//! This module provides common functionality for all communication protocols.
//! Consolidated and simplified from the original combase structure.

// Core modules
pub mod data_types;
pub mod traits;
pub mod manager;
pub mod polling;
pub mod redis;

// Legacy combase module (to be removed after migration)
pub mod combase;

// Re-export commonly used types
pub use data_types::*;
pub use traits::{ComBase, FourTelemetryOperations, ConnectionManager, ConfigValidator, ProtocolPacketParser, PointReader};
pub use manager::{OptimizedPointManager, PointManagerStats};
pub use polling::{UniversalPollingEngine, PollingEngine, TimeCache};
pub use redis::{RedisBatchSync, RedisBatchSyncConfig, RedisSyncStats};

// Legacy re-exports for backward compatibility
pub use combase::ProtocolFactory;

/// Initialize common protocol functionality
pub fn init_common_protocols() {
    tracing::info!("Common protocol functionality initialized");
}
