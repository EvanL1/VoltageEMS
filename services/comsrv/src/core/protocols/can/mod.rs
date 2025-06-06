//! CAN (Controller Area Network) Protocol Implementation
//! 
//! This module provides comprehensive CAN bus communication functionality including:
//! - CAN frame handling and parsing
//! - Message filtering and routing
//! - Error detection and handling
//! - Multiple CAN interface support (SocketCAN, Peak CAN, etc.)

pub mod common;
pub mod client;
pub mod frame;

pub use common::*;
pub use client::*;
pub use frame::*; 