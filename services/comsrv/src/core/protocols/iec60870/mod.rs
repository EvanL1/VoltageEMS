pub mod asdu;
pub mod common;
pub mod config;
pub mod iec104;

// Plugin support
pub mod plugin;

pub use config::Iec104Config;

use crate::core::protocols::common::combase::{PacketParseResult, ProtocolPacketParser};
use std::collections::HashMap;

/// IEC60870 protocol packet parser
///
/// Provides minimal packet interpretation. Detailed parsing is available
/// when the `iec60870` feature is enabled.
pub struct Iec60870PacketParser;

impl Iec60870PacketParser {
    /// Create a new IEC60870 packet parser
    pub fn new() -> Self {
        Self
    }
}

impl ProtocolPacketParser for Iec60870PacketParser {
    fn protocol_name(&self) -> &str {
        "IEC60870"
    }

    fn parse_packet(&self, data: &[u8], direction: &str) -> PacketParseResult {
        let hex_data = self.format_hex_data(data);

        #[cfg(feature = "iec60870")]
        {
            let description = format!("IEC60870 packet, {} bytes", data.len());
            return PacketParseResult::success("IEC60870", direction, &hex_data, &description);
        }

        #[cfg(not(feature = "iec60870"))]
        {
            PacketParseResult::success("IEC60870", direction, &hex_data, "IEC60870 parser disabled")
        }
    }
}
