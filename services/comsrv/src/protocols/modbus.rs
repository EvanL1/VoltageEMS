//! Modbus Protocol Implementation
//!
//! Supports both Modbus TCP and Modbus RTU protocols

pub mod codec;
pub mod command_batcher;
pub mod connection;
pub mod constants;
pub mod pdu;
pub mod protocol;
pub mod transport;
pub mod types;

// Re-export commonly used types
pub use protocol::ModbusProtocol;
pub use transport::{ModbusFrameProcessor, ModbusMode};
pub use types::*;
