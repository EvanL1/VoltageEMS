//! Gateway module - 通道运行时抽象层
//!
//! 本模块提供：
//! - `ChannelRuntime` trait：统一的协议通道接口
//! - `factory`：根据配置创建通道的工厂函数
//! - 配置类型和地址解析
//!
//! 各协议适配器（如 ModbusChannel, Iec104Channel 等）直接实现
//! `ChannelRuntime` trait，无需额外包装层。

mod address;
mod config;
pub mod factory;
mod runtime;

// Public exports
pub use address::parse_address;
pub use config::{
    ChannelConfig, ChannelModeConfig, ConfigError, GatewayConfig, GatewayGlobalConfig, PointDef,
};
pub use runtime::{ChannelMode, ChannelRuntime};
