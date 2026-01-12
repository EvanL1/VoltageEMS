//! Channels Module (formerly ComBase)
//!
//! This module provides the base infrastructure for communication protocol implementations.
//! Actual protocol implementations are provided as plugins.

// Core modules
pub mod channel_manager; // Channel lifecycle manager (includes ChannelEntry, ChannelStats)
pub mod traits; // Core traits and type definitions (re-exports from types)
pub mod trigger; // Command trigger for storage and synchronization
pub mod types; // Channel communication types (owned by comsrv)

// Configuration conversion and protocol factory (split from former bridge.rs)
pub mod converters; // Config converters: comsrv config → IGW PointConfig
pub mod factory; // Protocol client factory: create_*_channel() functions

// Re-export data types from local types module
pub use types::{ChannelCommand, ChannelStatus, ConnectionState, ProtocolValue};

// Re-export other types from local modules
pub use crate::core::config::FourRemote;
pub use channel_manager::{ChannelEntry, ChannelManager, ChannelMetadata, ChannelStats};
pub use trigger::{CommandStatus, CommandTrigger, CommandTriggerConfig, ControlCommand};

// Re-export converters
#[cfg(feature = "modbus")]
pub use converters::convert_to_modbus_point_configs;
pub use converters::convert_to_point_configs;
#[cfg(all(feature = "can", target_os = "linux"))]
pub use converters::{convert_can_to_point_configs, convert_to_can_point_configs};

// Re-export factory functions
#[cfg(all(feature = "can", target_os = "linux"))]
pub use factory::create_can_channel;
#[cfg(all(target_os = "linux", feature = "gpio"))]
pub use factory::create_gpio_channel;
pub use factory::create_virtual_channel;
#[cfg(feature = "modbus")]
pub use factory::{create_modbus_channel, create_modbus_rtu_channel};

/// Initialize channels module
pub fn init_channels() {
    tracing::info!("Channels module initialized");
}
