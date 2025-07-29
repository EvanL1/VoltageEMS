//! Protocol Plugins
//!
//! This module contains all protocol implementations as plugins.

#[cfg(feature = "modbus")]
pub mod modbus;

pub mod virt; // Virtual protocol is always available
