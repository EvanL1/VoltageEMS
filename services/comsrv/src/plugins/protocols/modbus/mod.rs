//! Modbus Protocol Implementation
//!
//! 精简的 Modbus protocolimplement，package含：
//! - 核心protocolprocessing (TCP/RTU)
//! - 集成轮询机制
//! - batchreadoptimization
//! - Plugin interface适配

pub mod connection;
pub mod core;
pub mod plugin;
pub mod transport;
pub mod types;

// 重新exportmaster要type
pub use connection::{
    ConnectionParams, ModbusConnection, ModbusConnectionManager, ModbusMode as ConnectionMode,
};
pub use core::{ModbusCore, ModbusProtocol};
pub use plugin::{ModbusRtuPlugin, ModbusTcpPlugin};
pub use transport::{ModbusFrameProcessor, ModbusMode};
pub use types::{
    DeviceLimit, ModbusBatchConfig, ModbusPoint, ModbusPollingConfig, SlavePollingConfig,
};

// Plugin 工厂function
pub fn create_plugin() -> Box<dyn crate::plugins::traits::ProtocolPlugin> {
    Box::new(ModbusTcpPlugin)
}
