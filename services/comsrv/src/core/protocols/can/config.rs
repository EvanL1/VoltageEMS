//! CAN Protocol Configuration

use serde::{Deserialize, Serialize};

/// CAN protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanConfig {
    /// CAN interface name
    pub interface: String,
    /// Bitrate in bits per second
    pub bitrate: u32,
    /// Use extended (29-bit) CAN IDs
    pub use_extended_id: bool,
    /// Use CAN FD (Flexible Data-rate)
    pub use_fd: bool,
    /// Read timeout in milliseconds
    pub timeout_ms: u64,
    /// Size of the transmit queue
    pub send_queue_size: usize,
}

impl Default for CanConfig {
    fn default() -> Self {
        Self {
            interface: "can0".to_string(),
            bitrate: 500000,
            use_extended_id: false,
            use_fd: false,
            timeout_ms: 1000,
            send_queue_size: 100,
        }
    }
}
