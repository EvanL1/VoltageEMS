//! CAN (Controller Area Network) Protocol Implementation
//!
//! This module provides comprehensive CAN bus communication functionality including:
//! - CAN frame handling and parsing
//! - Message filtering and routing
//! - Error detection and handling
//! - Multiple CAN interface support (SocketCAN, Peak CAN, etc.)

pub mod client;
pub mod common;
pub mod config;
pub mod frame;

// Plugin support
pub mod plugin;

pub use config::CanConfig;

use crate::core::protocols::common::combase::{PacketParseResult, ProtocolPacketParser};
use std::collections::HashMap;

/// CAN protocol packet parser
///
/// Provides basic hex logging of CAN frames. Detailed parsing can be
/// implemented when the `can` feature is enabled.
pub struct CanPacketParser;

impl CanPacketParser {
    /// Create a new CAN packet parser
    pub fn new() -> Self {
        Self
    }
}

impl ProtocolPacketParser for CanPacketParser {
    fn protocol_name(&self) -> &str {
        "CAN"
    }

    fn parse_packet(&self, data: &[u8], direction: &str) -> PacketParseResult {
        let hex_data = self.format_hex_data(data);

        #[cfg(feature = "can")]
        {
            let description = format!("CAN frame, {} bytes", data.len());
            return PacketParseResult::success("CAN", direction, &hex_data, &description);
        }

        #[cfg(not(feature = "can"))]
        {
            PacketParseResult::success("CAN", direction, &hex_data, "CAN parser disabled")
        }
    }
}
