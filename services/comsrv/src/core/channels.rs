//! Channels Module (formerly ComBase)
//!
//! This module provides the base infrastructure for communication protocol implementations.
//! Actual protocol implementations are provided as plugins.

// Core modules
pub mod channel_manager; // Channel lifecycle manager (includes ChannelEntry, ChannelStats)
pub mod point_config; // Point configuration provider
pub mod sync; // Telemetry synchronization and data transformation
pub mod traits; // Core traits and type definitions
pub mod trigger; // Command trigger for storage and synchronization

// Re-export data types from voltage_comlink (primary source for data types)
// Note: ComBase and ComClient traits are still from local traits.rs because they use ComSrvError
pub use voltage_comlink::{
    ChannelCommand, ChannelLogger, ChannelStatus, ConnectionState, ExtendedPointData, PointData,
    PointDataMap, ProtocolValue, TelemetryBatch, TestChannelParams,
};

// Re-export traits from local module (uses ComSrvError)
pub use traits::{ComBase, ComClient};

// Re-export other types from local modules
pub use crate::core::config::FourRemote;
pub use channel_manager::{
    ChannelEntry, ChannelManager, ChannelMetadata, ChannelStats, DynComClient,
};
pub use point_config::RuntimeConfigProvider;
pub use sync::{PointTransformer, TransformDirection};
pub use trigger::{CommandStatus, CommandTrigger, CommandTriggerConfig, ControlCommand};

/// Initialize channels module
pub fn init_channels() {
    tracing::info!("Channels module initialized");
}
