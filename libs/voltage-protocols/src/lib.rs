//! VoltageEMS Protocol Implementations
//!
//! This library provides protocol client implementations for VoltageEMS.
//! Protocols are feature-gated for selective compilation.
//!
//! # Features
//!
//! - `virt` - Virtual protocol for testing
//! - `modbus` - Modbus TCP/RTU protocol
//! - `dido` - DI/DO (GPIO) protocol for local hardware
//!
//! # Architecture
//!
//! All protocols implement the `ComClient` trait from `voltage-comlink`.
//! Each protocol provides a `from_runtime_config()` constructor for
//! instantiation from channel configuration.

#[cfg(feature = "dido")]
pub mod dido;

#[cfg(feature = "modbus")]
pub mod modbus;

// Re-export common types for convenience
pub use voltage_comlink::{
    ComBase, ComClient, ConnectionState, PointData, PointDataMap, ProtocolValue,
};
