//! Voltage Communication Link Library
//!
//! Core communication abstractions and protocol implementations for VoltageEMS.
//!
//! # Architecture
//!
//! This library provides:
//! - **Core Traits**: `ComBase`, `ComClient` for protocol abstraction
//! - **Bytes Utilities**: Byte order handling, bit operations, type conversions
//! - **Protocol Implementations**: Modbus, CAN, Virtual protocols (feature-gated)
//!
//! # Features
//!
//! - `modbus` - Modbus TCP/RTU protocol support (default)
//! - `can` - CAN bus protocol support with DBC parsing (default)
//! - `can-linux` - Full CAN support on Linux with SocketCAN

pub mod bytes;
pub mod error;
pub mod traits;

// Re-export core types
pub use bytes::ByteOrder;
pub use error::{ComLinkError, Result};
pub use traits::{
    ChannelCommand, ChannelLogger, ChannelStatus, ComBase, ComClient, ConnectionState,
    ExtendedPointData, PointData, PointDataMap, ProtocolValue, RedisValue, TelemetryBatch,
    TestChannelParams,
};
