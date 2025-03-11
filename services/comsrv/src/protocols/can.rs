//! CAN Protocol Implementation
//!
//! Supports CAN bus communication with DBC-based signal extraction

pub mod protocol;
pub mod transport;
pub mod types;

// Re-export commonly used types
pub use protocol::CanProtocol;
