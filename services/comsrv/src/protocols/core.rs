//! Core abstractions for the Industrial Gateway.
//!
//! This module provides the foundational types and traits that all protocols implement.

pub mod data;
pub mod error;
pub mod logging;
pub mod metadata;
pub mod point;
pub mod quality;
pub mod slot;
pub mod traits;

pub use data::*;
pub use error::{GatewayError, Result};
pub use metadata::{
    get_protocol_registry, DriverMetadata, HasMetadata, ParameterMetadata, ParameterType,
    ProtocolMetadata, ProtocolRegistry,
};
pub use point::*;
pub use quality::*;
pub use slot::{AtomicBoolStore, DataSlot, ShardedSlotStore, SlotStore};
pub use traits::*;
