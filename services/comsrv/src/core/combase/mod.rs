//! `ComBase` Module
//!
//! This module provides the base infrastructure for communication protocol implementations.
//! Actual protocol implementations are provided as plugins.

// 核心模块
pub mod command; // Redis命令订阅
pub mod core; // 核心trait和类型定义
pub mod factory; // 协议工厂
pub mod manager; // 点位管理器
pub mod storage; // 存储和同步

// 重新导出常用类型
pub use crate::core::config::types::TelemetryType;
pub use command::{CommandStatus, CommandSubscriber, CommandSubscriberConfig, ControlCommand};
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
pub use storage::{create_combase_storage, ComBaseStorage, DefaultComBaseStorage, StorageStats};

/// Initialize combase module
pub fn init_combase() {
    tracing::info!("ComBase module initialized");
}
