//! Binary data processing utilities
//!
//! Re-exports from voltage_comlink::bytes for backward compatibility.
//! The authoritative source is now voltage_comlink.
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

// Re-export everything from voltage_comlink::bytes
pub use voltage_comlink::bytes::*;
