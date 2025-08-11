//! Modbus Protocol Implementation
//!
//! Streamlined Modbus protocol implementation, including:
//! - Core protocol processing (TCP/RTU)
//! - Integrated polling mechanism
//! - Batch read optimization
//! - Plugin interface adaptation

pub mod connection;
pub mod pdu;
pub mod plugin;
pub mod protocol;
pub mod server;
pub mod transport;
pub mod types;

#[cfg(test)]
pub mod simulator;

// Re-export main types
pub use connection::{
    ConnectionParams, ModbusConnection, ModbusConnectionManager, ModbusMode as ConnectionMode,
};
pub use plugin::{ModbusRtuPlugin, ModbusTcpPlugin};
pub use protocol::ModbusProtocol;
pub use server::ModbusServer;
pub use transport::{ModbusFrameProcessor, ModbusMode};
pub use types::{
    DeviceLimit, ModbusBatchConfig, ModbusPoint, ModbusPollingConfig, SlavePollingConfig,
};

// Plugin factory function
pub fn create_plugin() -> Box<dyn crate::plugins::traits::ProtocolPlugin> {
    Box::new(ModbusTcpPlugin)
}
