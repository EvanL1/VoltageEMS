//! Protocol implementations.
//!
//! This module contains adapters that integrate protocol crates with the protocol layer.

pub mod virtual_channel;

// Cross-platform CAN types and decoder (no hardware dependency)
pub mod can_decoder;
pub mod can_types;

// Modbus TCP + RTU support
#[cfg(feature = "modbus")]
pub mod modbus;

#[cfg(feature = "modbus")]
pub mod command_batcher;

// Mock Modbus server for testing (available in both test and non-test builds for integration tests)
#[cfg(feature = "modbus")]
pub mod modbus_mock;

#[cfg(feature = "iec104")]
pub mod iec104;

#[cfg(feature = "opcua")]
pub mod opcua;

#[cfg(all(feature = "can", target_os = "linux"))]
pub mod can;

#[cfg(all(feature = "gpio", target_os = "linux"))]
pub mod gpio;

#[cfg(feature = "dl645")]
pub mod dl645;
