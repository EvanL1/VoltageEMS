//! Protocol Plugins
//!
//! This module contains all protocol implementations as plugins.

#[cfg(feature = "modbus")]
pub mod modbus;

#[cfg(feature = "iec60870")]
pub mod iec60870;

#[cfg(feature = "can")]
pub mod can;

pub mod virt; // Virtual protocol is always available
