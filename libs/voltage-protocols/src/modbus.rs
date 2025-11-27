//! Modbus Protocol Implementation
//!
//! This module provides Modbus TCP and RTU protocol core components for VoltageEMS.
//! The implementation supports:
//! - Multiple function codes (FC01-04, FC05-06, FC15-16)
//! - TCP and RTU transport modes
//! - Frame encoding/decoding with MBAP header and CRC
//! - Command batching for write optimization
//!
//! # Architecture
//!
//! This library provides protocol core components:
//!
//! ```text
//! voltage-protocols/modbus
//!     ├── ModbusConnectionManager (TCP/RTU connection handling)
//!     ├── ModbusFrameProcessor (MBAP header / CRC handling)
//!     ├── ModbusCodec (value encoding/decoding)
//!     ├── CommandBatcher (write optimization)
//!     └── types (ModbusPoint, polling config, etc.)
//! ```
//!
//! The `ModbusProtocol` (ComClient implementation) is in comsrv, which integrates
//! these components with service-layer concerns (config, four-telemetry mapping, Redis).

mod codec;
mod command_batcher;
mod connection;
mod constants;
mod pdu;
mod transport;
mod types;

// Re-export main types
pub use codec::{clamp_to_data_type, decode_register_value, parse_modbus_pdu, ModbusCodec};
pub use command_batcher::{BatchCommand, CommandBatcher, BATCH_WINDOW_MS, MAX_BATCH_SIZE};
pub use connection::ModbusMode as ConnectionMode;
pub use connection::{ConnectionParams, ModbusConnection, ModbusConnectionManager};
pub use constants::{
    MAX_MBAP_LENGTH, MAX_PDU_SIZE, MBAP_HEADER_LEN, MODBUS_MAX_READ_COILS,
    MODBUS_MAX_READ_REGISTERS, MODBUS_MAX_WRITE_COILS, MODBUS_MAX_WRITE_REGISTERS,
    MODBUS_RESPONSE_BUFFER_SIZE,
};
pub use pdu::{ModbusPdu, PduBuilder};
pub use transport::{MbapHeader, ModbusFrameProcessor, ModbusMode};
pub use types::{
    DeviceLimit, ModbusBatchConfig, ModbusPoint, ModbusPollingConfig, SlavePollingConfig,
};
