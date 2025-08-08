//! `ComBase` Module
//!
//! This module provides the base infrastructure for communication protocol implementations.
//! Actual protocol implementations are provided as plugins.

// Core modules
pub mod factory; // Protocol factory
pub mod traits; // Core traits and type definitions
pub mod trigger; // Command trigger for storage and synchronization

// Re-export common types
pub use crate::core::config::types::TelemetryType;
pub use factory::{
    create_default_factory, create_factory_with_custom_protocols, ChannelStats, ConfigValue,
    DynComClient, ProtocolClientFactory, ProtocolFactory,
};
pub use traits::{
    ChannelCommand, ChannelStatus, ClientInfo, ComBase, ComClient, ComServer, ConfigValidator,
    ConnectionManager, DefaultProtocol, ExtendedPointData, FourTelemetryOperations,
    PacketParseResult, PointData, PointDataMap, ProtocolPacketParser, RedisValue,
    TestChannelParams,
};
// Storage now in unified module at crate::storage
pub use trigger::{
    CommandStatus, CommandTrigger, CommandTriggerConfig, ControlCommand, TriggerMode,
};

/// Initialize combase module
pub fn init_combase() {
    tracing::info!("ComBase module initialized");
}
