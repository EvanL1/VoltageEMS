//! `ComBase` Module
//!
//! This module provides the base infrastructure for communication protocol implementations.
//! Actual protocol implementations are provided as plugins.

// 核心modular
pub mod core; // 核心trait和typedefinition
pub mod factory; // protocol工厂
pub mod manager; // 点位managing器
pub mod storage;
pub mod trigger; // 命令trigger器 // storage和synchronous

// 重新export常用type
pub use crate::core::config::types::TelemetryType;
pub use core::{
    ChannelCommand, ChannelStatus, ComBase, ConfigValidator, ConnectionManager, DefaultProtocol,
    ExtendedPointData, FourTelemetryOperations, PacketParseResult, PointData, PointDataMap,
    ProtocolPacketParser, RedisValue, TestChannelParams,
};
pub use factory::{
    create_default_factory, create_factory_with_custom_protocols, ChannelStats, ConfigValue,
    DynComClient, ProtocolClientFactory, ProtocolFactory,
};
pub use manager::{OptimizedPointManager, PointManagerStats, PollingPoint};
pub use storage::{ComBaseStorage, StorageStats};
pub use trigger::{
    CommandStatus, CommandTrigger, CommandTriggerConfig, ControlCommand, TriggerMode,
};

/// Initialize combase module
pub fn init_combase() {
    tracing::info!("ComBase module initialized");
}
