//! Modbus Protocol Implementation
//!
//! 精简的 Modbus 协议实现，包含：
//! - 核心协议处理 (TCP/RTU)
//! - 集成轮询机制
//! - 批量读取优化
//! - Plugin 接口适配

pub mod connection;
pub mod core;
pub mod plugin;
pub mod transport;
pub mod types;

// 重新导出主要类型
pub use connection::{
    ConnectionParams, ModbusConnection, ModbusConnectionManager, ModbusMode as ConnectionMode,
};
pub use core::{ModbusCore, ModbusProtocol};
pub use plugin::{ModbusRtuPlugin, ModbusTcpPlugin};
pub use transport::{ModbusFrameProcessor, ModbusMode};
pub use types::{
    DeviceLimit, ModbusBatchConfig, ModbusPoint, ModbusPollingConfig, SlavePollingConfig,
};

// Plugin 工厂函数
pub fn create_plugin() -> Box<dyn crate::plugins::traits::ProtocolPlugin> {
    Box::new(ModbusTcpPlugin)
}
