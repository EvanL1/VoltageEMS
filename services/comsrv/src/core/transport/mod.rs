//! Transport Layer Module
//!
//! This module provides a unified transport layer abstraction that separates
//! physical communication details from protocol logic. It supports various
//! transport mechanisms including TCP, serial, GPIO (DI/DO), CAN bus, etc.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                Protocol Layer                           │
//! │  (Modbus, IEC60870, CAN Protocol Logic)               │
//! └─────────────────────────────────────────────────────────┘
//!                             │
//!                             ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │              Transport Interface (Trait)                │
//! │  connect(), disconnect(), send(), receive()            │
//! └─────────────────────────────────────────────────────────┘
//!                             │
//!     ┌───────────────────────┼───────────────────────┐
//!     ▼               ▼               ▼               ▼
//! ┌─────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
//! │   TCP   │ │   Serial    │ │    GPIO     │ │    CAN      │
//! │Transport│ │  Transport  │ │  Transport  │ │  Transport  │
//! │         │ │             │ │   (DI/DO)   │ │             │
//! └─────────┘ └─────────────┘ └─────────────┘ └─────────────┘
//! ```
//!
//! # Industrial Interface Support
//!
//! The transport layer now supports a comprehensive range of industrial interfaces:
//!
//! ## Network Communications
//! - **TCP** - Ethernet-based protocols (Modbus TCP, etc.)
//! - **Serial** - RS232/RS485 communications (Modbus RTU, etc.)
//!
//! ## Field I/O Interfaces  
//! - **GPIO** - Digital Input/Output (DI/DO), Analog I/O (AI/AO)
//! - **CAN** - Controller Area Network for automotive/industrial
//!
//! ## Testing Support
//! - **Mock** - Controllable mock transport for testing
//!
//! # Benefits
//!
//! - **Reusability**: Transport implementations can be shared across protocols
//! - **Testability**: Transport layer can be mocked for protocol testing
//! - **Maintainability**: Transport bugs fixed once, benefit all protocols
//! - **Extensibility**: New transport types automatically available to all protocols
//! - **Industrial Ready**: Comprehensive support for edge device interfaces

// CAN module is only available on Linux
#[cfg(all(target_os = "linux", feature = "can"))]
pub mod can;

pub mod factory;
pub mod gpio;
#[cfg(any(test, feature = "test-utils"))]
pub mod mock;
pub mod serial;
pub mod tcp;
pub mod traits;

// Re-export commonly used types
#[cfg(all(target_os = "linux", feature = "can"))]
pub use can::CanTransport;

pub use factory::{TransportFactory, TransportType};
pub use gpio::GpioTransport;
#[cfg(any(test, feature = "test-utils"))]
pub use mock::MockTransport;
pub use serial::SerialTransport;
pub use tcp::TcpTransport;
pub use traits::{Transport, TransportConfig, TransportError, TransportStats};

/// Transport module initialization
pub fn init_transport_layer() {
    tracing::info!("Initializing transport layer with layered architecture");
    tracing::info!("Supported transports: TCP, Serial, GPIO (DI/DO), CAN, Mock");
}
