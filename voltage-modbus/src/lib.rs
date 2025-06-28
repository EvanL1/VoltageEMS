//! # Voltage Modbus - High-Performance Modbus TCP/RTU/ASCII Library
//! 
//! **Author:** Evan Liu <evan.liu@voltageenergy.com>  
//! **Version:** 0.2.0  
//! **License:** MIT
//! 
//! A comprehensive, high-performance Modbus TCP/RTU/ASCII implementation in pure Rust
//! designed for industrial automation, IoT applications, and smart grid systems.
//! 
//! ## Features
//! 
//! - **🚀 High Performance**: Async/await support with Tokio for maximum throughput
//! - **🔧 Complete Protocol Support**: Modbus TCP, RTU, and ASCII protocols
//! - **🛡️ Memory Safe**: Pure Rust implementation with zero unsafe code
//! - **⚡ Zero-Copy Operations**: Optimized for minimal memory allocations
//! - **🔄 Concurrent Processing**: Multi-client server support
//! - **📊 Built-in Monitoring**: Comprehensive statistics and metrics
//! - **🏭 Production Ready**: Extensive testing and error handling
//! 
//! ## Supported Function Codes
//! 
//! | Code | Function | Client | Server |
//! |------|----------|--------|--------|
//! | 0x01 | Read Coils | ✅ | ✅ |
//! | 0x02 | Read Discrete Inputs | ✅ | ✅ |
//! | 0x03 | Read Holding Registers | ✅ | ✅ |
//! | 0x04 | Read Input Registers | ✅ | ✅ |
//! | 0x05 | Write Single Coil | ✅ | ✅ |
//! | 0x06 | Write Single Register | ✅ | ✅ |
//! | 0x0F | Write Multiple Coils | ✅ | ✅ |
//! | 0x10 | Write Multiple Registers | ✅ | ✅ |
//! 
//! ## Quick Start
//! 
//! ### Client Example
//! 
//! ```rust,no_run
//! use voltage_modbus::{ModbusTcpClient, ModbusClient, ModbusResult};
//! use std::time::Duration;
//! 
//! #[tokio::main]
//! async fn main() -> ModbusResult<()> {
//!     // Connect to Modbus TCP server
//!     let mut client = ModbusTcpClient::from_address("127.0.0.1:502", Duration::from_secs(5)).await?;
//!     
//!     // Read holding registers
//!     let values = client.read_03(1, 0, 10).await?;
//!     println!("Read registers: {:?}", values);
//!     
//!     // Write single register
//!     client.write_06(1, 100, 0x1234).await?;
//!     
//!     client.close().await?;
//!     Ok(())
//! }
//! ```
//! 
//! ### Server Example
//! 
//! ```rust,no_run
//! use voltage_modbus::{ModbusTcpServer, ModbusTcpServerConfig, ModbusServer, ModbusRegisterBank};
//! use std::sync::Arc;
//! use std::time::Duration;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create server configuration
//!     let config = ModbusTcpServerConfig {
//!         bind_address: "127.0.0.1:502".parse().unwrap(),
//!         max_connections: 50,
//!         request_timeout: Duration::from_secs(30),
//!         register_bank: Some(Arc::new(ModbusRegisterBank::new())),
//!     };
//!     
//!     // Start server
//!     let mut server = ModbusTcpServer::with_config(config)?;
//!     server.start().await?;
//!     
//!     // Server is now running...
//!     Ok(())
//! }
//! ```
//! 
//! ## Architecture
//! 
//! ```text
//! ┌─────────────────┐    ┌─────────────────┐
//! │   Application   │    │   Application   │
//! └─────────────────┘    └─────────────────┘
//!          │                       │
//! ┌─────────────────┐    ┌─────────────────┐
//! │  Modbus Client  │    │  Modbus Server  │
//! └─────────────────┘    └─────────────────┘
//!          │                       │
//! ┌─────────────────┐    ┌─────────────────┐
//! │   Protocol      │    │ Register Bank   │
//! │   (TCP/RTU)     │    │   (Storage)     │
//! └─────────────────┘    └─────────────────┘
//!          │                       │
//! ┌─────────────────┐    ┌─────────────────┐
//! │   Transport     │◄──►│   Transport     │
//! │   (Async I/O)   │    │   (Async I/O)   │
//! └─────────────────┘    └─────────────────┘
//! ```

/// Core error types and result handling
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
pub mod error;

/// Modbus protocol definitions and message handling
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
pub mod protocol;

/// Network transport layer for TCP and RTU communication
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
pub mod transport;

/// Modbus client implementations
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
pub mod client;

/// Modbus server implementations
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
pub mod server;

/// Thread-safe register storage for server applications
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
pub mod register_bank;

/// Utility functions and performance monitoring
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
pub mod utils;

/// Logging system for the library
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
pub mod logging;

// Re-export main types for convenience
pub use error::{ModbusError, ModbusResult};
pub use protocol::{ModbusRequest, ModbusResponse, ModbusFunction};
pub use transport::{ModbusTransport, TcpTransport, RtuTransport, AsciiTransport, TransportStats};
pub use client::{ModbusClient, ModbusTcpClient, ModbusRtuClient};
pub use server::{ModbusServer, ModbusTcpServer, ModbusTcpServerConfig, ServerStats};
pub use register_bank::{ModbusRegisterBank, RegisterBankStats};
pub use utils::{PerformanceMetrics, OperationTimer};
pub use logging::{LogLevel, LogCallback, CallbackLogger, LoggingMode};

/// Default timeout for operations (5 seconds)
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Maximum number of coils that can be read/written in a single request
pub const MAX_COILS_PER_REQUEST: u16 = 2000;

/// Maximum number of registers that can be read/written in a single request  
pub const MAX_REGISTERS_PER_REQUEST: u16 = 125;

/// Maximum Modbus TCP frame size (MBAP header + PDU)
pub const MAX_TCP_FRAME_SIZE: usize = 260;

/// Maximum Modbus RTU frame size
pub const MAX_RTU_FRAME_SIZE: usize = 256;

/// Modbus TCP default port
pub const DEFAULT_TCP_PORT: u16 = 502;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get library information
pub fn info() -> String {
    format!("Voltage Modbus v{} - High-performance Modbus TCP/RTU/ASCII library by Evan Liu", VERSION)
} 