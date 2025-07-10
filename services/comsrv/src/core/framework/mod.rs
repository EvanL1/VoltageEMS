//! Protocol Framework Module
//!
//! This module provides the framework and base infrastructure for protocol implementations.
//! Actual protocol implementations are provided as plugins.

// Core modules
pub mod base; // Basic protocol implementation (was default_protocol)
pub mod factory; // Protocol factory (was protocol_factory)
pub mod manager; // Point manager
pub mod traits; // Core traits
pub mod types; // Data types (was data_types)

// Data handling modules
pub mod optimized_sync; // Optimized sync mechanisms
pub mod realtime_data; // Real-time data structures
pub mod redis; // Redis integration

// Re-export commonly used types
pub use base::{DefaultProtocol, PacketParseResult};
pub use factory::{ConfigValue, ProtocolFactory};
pub use manager::{OptimizedPointManager, PointManagerStats};
pub use traits::{
    ComBase, ConfigValidator, ConnectionManager, FourTelemetryOperations, ProtocolPacketParser,
};
pub use types::*;

// Data handling exports
pub use optimized_sync::{OptimizedBatchSync, OptimizedSyncConfig, SyncStats};
pub use realtime_data::{ChannelConfig, PointConfig, RealtimeBatch, RealtimeValue};
pub use redis::{RedisBatchSync, RedisBatchSyncConfig, RedisSyncStats};

/// Initialize protocol framework
pub fn init_framework() {
    tracing::info!("Protocol framework initialized");
}
