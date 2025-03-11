//! `ComBase` Module
//!
//! This module provides the base infrastructure for communication protocol implementations.
//! Actual protocol implementations are provided as plugins.

// Core modules
pub mod channel; // Channel management
pub mod channel_manager; // Channel lifecycle manager
pub mod point_config; // Point configuration provider
pub mod point_transformer; // Point data transformers
pub mod sync; // Telemetry synchronization
pub mod traits; // Core traits and type definitions
pub mod trigger; // Command trigger for storage and synchronization

// Re-export common types
pub use crate::core::config::types::FourRemote;
pub use channel::{ChannelEntry, ChannelMetadata, ChannelStats};
pub use channel_manager::{ChannelManager, DynComClient};
pub use point_config::RuntimeConfigProvider;
pub use point_transformer::{PointTransformer, TransformDirection};
pub use traits::{
    ChannelCommand, ChannelStatus, ComBase, ComClient, ExtendedPointData, PointData, PointDataMap,
    RedisValue, TestChannelParams,
};
// Storage now in unified module at crate::storage
pub use trigger::{CommandStatus, CommandTrigger, CommandTriggerConfig, ControlCommand};

/// Initialize combase module
pub fn init_combase() {
    tracing::info!("ComBase module initialized");
}
