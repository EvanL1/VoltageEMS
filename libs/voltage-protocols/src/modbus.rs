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
//!
//! # Re-exports from voltage-modbus
//!
//! Core Modbus types are re-exported from the standalone `voltage-modbus` library
//! for independent use and testing.

mod codec;
mod command_batcher;
mod connection;
mod constants;
mod transport;
mod types;

// ============================================================================
// EMS-specific types (local modules)
// ============================================================================

// Codec: parse_modbus_pdu has EMS-specific graceful degradation logic
pub use codec::{clamp_to_data_type, decode_register_value, parse_modbus_pdu, ModbusCodec};

// Command batcher: EMS-specific version using ProtocolValue and Option<String> byte_order
pub use command_batcher::{BatchCommand, CommandBatcher, BATCH_WINDOW_MS, MAX_BATCH_SIZE};

// Connection: ModbusConnectionManager depends on ChannelLogger
pub use connection::ModbusMode as ConnectionMode;
pub use connection::{ConnectionParams, ModbusConnection, ModbusConnectionManager};

// Transport: ModbusFrameProcessor is EMS-specific request tracking layer
pub use transport::{MbapHeader, ModbusFrameProcessor, ModbusMode};

// Types: EMS configuration types
pub use types::{
    DeviceLimit, ModbusBatchConfig, ModbusPoint, ModbusPollingConfig, SlavePollingConfig,
};

// ============================================================================
// Re-exports from voltage-modbus (standalone library)
// ============================================================================

// Core types
pub use voltage_modbus::{
    ByteOrder, DeviceLimits, ModbusClient, ModbusError, ModbusResult, ModbusTcpClient, ModbusValue,
};

// PDU types (no conflict after removing local pdu.rs)
pub use voltage_modbus::{ModbusPdu, PduBuilder};

// Constants - keep local constants.rs for MODBUS_ prefixed names used by comsrv
pub use constants::{
    MAX_MBAP_LENGTH, MAX_PDU_SIZE, MBAP_HEADER_LEN, MODBUS_MAX_READ_COILS,
    MODBUS_MAX_READ_REGISTERS, MODBUS_MAX_WRITE_COILS, MODBUS_MAX_WRITE_REGISTERS,
    MODBUS_RESPONSE_BUFFER_SIZE,
};
