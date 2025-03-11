//! Binary data processing utilities
//!
//! Provides cross-protocol byte order handling, bit extraction,
//! and numeric type conversions for industrial protocols (Modbus, CAN, IEC104, etc.).
//!
//! # Use Cases
//!
//! ## Modbus: Convert 2 registers to float32
//! ```rust
//! use comsrv::utils::bytes::{ByteOrder, regs_to_f32};
//!
//! let regs = [0x41C8, 0x0000]; // IEEE754: 25.0
//! let value = regs_to_f32(&regs, ByteOrder::BigEndian);
//! assert!((value - 25.0).abs() < f32::EPSILON);
//! ```
//!
//! ## CAN: Extract 12-bit signal from byte array
//! ```rust
//! use comsrv::utils::bytes::extract_bits;
//!
//! let data = [0xAB, 0xCD, 0xEF];
//! let signal = extract_bits(&data, 4, 12); // Start bit 4, length 12
//! assert_eq!(signal, 0xBCD);
//! ```
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
