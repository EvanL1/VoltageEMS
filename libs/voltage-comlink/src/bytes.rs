//! Binary data processing utilities
//!
//! Provides cross-protocol byte order handling, bit extraction,
//! and numeric type conversions for industrial protocols (Modbus, CAN, IEC104, etc.).
//!
//! # Design Principles
//!
//! - **Protocol-agnostic**: No Modbus/CAN/IEC104-specific logic
//! - **Type-safe**: `ByteOrder` enum prevents string typos
//! - **Well-tested**: Property-based + table-driven tests
//! - **Zero-copy**: Direct byte array operations where possible

pub mod bit_ops;
pub mod byte_order;
pub mod conversions;

pub use bit_ops::*;
pub use byte_order::ByteOrder;
pub use conversions::*;
