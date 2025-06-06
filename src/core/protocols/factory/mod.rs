//! Protocol Factory Module
//! 
//! This module contains the protocol factory implementation for creating
//! and managing communication protocol instances. It provides a unified
//! interface for creating different types of protocol clients through
//! the factory pattern.
//! 
//! # Components
//! 
//! - **ProtocolFactory**: Main factory for creating protocol instances
//! - **ProtocolClientFactory**: Trait for implementing protocol-specific factories
//! - **Built-in Factories**: Implementations for Modbus TCP/RTU, IEC60870, etc.
//! 
//! # Architecture
//! 
//! The factory pattern allows for:
//! - Dynamic protocol registration
//! - Configuration validation
//! - Unified instance management
//! - Type-safe protocol creation

pub mod protocol_factory;

// Re-export all public items for convenience
pub use protocol_factory::*; 