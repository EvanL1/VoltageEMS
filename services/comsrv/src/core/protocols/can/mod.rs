//! CAN (Controller Area Network) Protocol Implementation
//! 
//! This module provides comprehensive CAN bus communication functionality including:
//! - CAN frame handling and parsing
//! - Message filtering and routing
//! - Error detection and handling
//! - Multiple CAN interface support (SocketCAN, Peak CAN, etc.)

pub mod common;
pub mod config;
pub mod client;
pub mod frame;

// Plugin support
pub mod plugin;

pub use config::CanConfig;

use std::collections::HashMap;
use crate::core::protocols::common::combase::{ProtocolPacketParser, PacketParseResult};

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
            PacketParseResult::success(
                "CAN",
                direction,
                &hex_data,
                "CAN parser disabled"
            )
        }
    }
}
