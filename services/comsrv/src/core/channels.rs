//! Channels Module (formerly ComBase)
//!
//! This module provides the base infrastructure for communication protocol implementations.
//! Actual protocol implementations are provided as plugins.

// Core modules
pub mod channel_manager; // Channel lifecycle manager (includes ChannelEntry, ChannelStats)
pub mod point_config; // Point configuration provider
pub mod sync; // Telemetry synchronization and data transformation
pub mod traits; // Core traits and type definitions (re-exports from types)
pub mod trigger; // Command trigger for storage and synchronization
pub mod types; // Channel communication types (owned by comsrv)

// IGW integration
pub mod igw_bridge; // Bridge for IGW protocol clients

// Re-export data types from local types module
// These types are used by TelemetrySync and CommandTrigger
pub use types::{
    ChannelCommand, ChannelLogger, ChannelStatus, ConnectionState, ExtendedPointData, PointData,
    PointDataMap, ProtocolValue, TelemetryBatch, TestChannelParams,
};

// Re-export other types from local modules
pub use crate::core::config::FourRemote;
pub use channel_manager::{ChannelEntry, ChannelManager, ChannelMetadata, ChannelStats};
pub use point_config::RuntimeConfigProvider;
pub use sync::{PointTransformer, TransformDirection};
pub use trigger::{CommandStatus, CommandTrigger, CommandTriggerConfig, ControlCommand};

// IGW bridge types
pub use igw_bridge::{
    convert_to_igw_point_configs, create_virtual_channel, ChannelImpl, IgwChannelWrapper,
};

/// Initialize channels module
pub fn init_channels() {
    tracing::info!("Channels module initialized");
}
