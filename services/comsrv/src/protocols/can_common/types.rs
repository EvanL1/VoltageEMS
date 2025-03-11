//! Common CAN types shared between protocols

// Re-export shared types from voltage-config
pub use voltage_config::protocols::{ByteOrder, SignalDataType};

/// CAN message structure
#[derive(Debug, Clone)]
pub struct CanMessage {
    pub id: u32,
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub is_extended: bool,
    pub is_remote: bool,
    pub is_error: bool,
}

/// CAN ID filter
#[derive(Debug, Clone)]
pub struct CanFilter {
    pub can_id: u32,
    pub mask: u32,
}
