//! # rust-iec60870
//!
//! A Rust implementation of the IEC 60870-5 protocols for SCADA communications.
//!
//! This crate provides functionality for implementing IEC 60870-5-101 and IEC 60870-5-104
//! protocols, which are widely used in power utility automation systems.
//!
//! ## Features
//!
//! - IEC 60870-5-104 protocol implementation (TCP/IP-based)
//! - Asynchronous API using Tokio
//! - Type-safe ASDU handling
//! - Comprehensive error handling
//! - Well-documented codebase
//! - Designed for embedded and server-side applications
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use rust_iec60870::iec104::{Iec104Client, Iec104ClientConfig};
//! use rust_iec60870::common::CauseOfTransmission;
//! use rust_iec60870::asdu::{ASDU, TypeId};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure client
//!     let config = Iec104ClientConfig::new()
//!         .host("192.168.1.100")
//!         .port(2404)
//!         .timeout(Duration::from_secs(5))
//!         .max_retries(3)
//!         .build()?;
//!     
//!     // Create client
//!     let mut client = Iec104Client::new(config);
//!     
//!     // Connect to server
//!     client.connect().await?;
//!     
//!     // Start data transfer
//!     client.start_data_transfer().await?;
//!     
//!     // Send a general interrogation command
//!     let common_addr = 1;
//!     let asdu = ASDU::new(
//!         TypeId::InterrogationCommand,
//!         0x01, // Single sequence
//!         CauseOfTransmission::Activation,
//!         0, // Originator address
//!         common_addr,
//!         vec![20], // 20 = general interrogation
//!     );
//!     
//!     client.send_asdu(asdu).await?;
//!     
//!     // Process received data
//!     let data = client.receive().await?;
//!     println!("Received ASDU: {:?}", data);
//!     
//!     Ok(())
//! }
//! ```

pub mod common;
pub mod asdu;
pub mod iec104;
#[cfg(feature = "iec101")]
pub mod iec101;

// Re-export common types for convenience
pub use crate::common::{IecError, IecResult, CauseOfTransmission, QualityDescriptor};
pub use crate::asdu::{ASDU, TypeId, CommonAddrSize, InfoObjAddrSize};
pub use crate::iec104::{Iec104Client, Iec104ClientConfig, Apdu, ApciType}; 