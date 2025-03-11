//! Protocol implementations
//!
//! This module contains all communication protocol implementations.

#[cfg(feature = "modbus")]
pub mod modbus;

#[cfg(feature = "can")]
pub mod can_common;

#[cfg(feature = "can")]
pub mod can;

pub mod virt;
