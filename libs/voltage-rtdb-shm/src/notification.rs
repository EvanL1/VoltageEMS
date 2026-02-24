//! UDS Event Notification Protocol
//!
//! Used for M2C command notifications from modsrv → comsrv.
//! Notifications carry only "pointer" information (channel_id, point_type, point_id);
//! actual data is read from SHM.

use bytemuck::{Pod, Zeroable};
use voltage_model::PointType;

/// M2C command notification (12 bytes, fixed size)
///
/// Sent via Unix Domain Socket to notify comsrv of new Control/Adjustment commands.
/// Uses `#[repr(C)]` to ensure C-compatible memory layout for cross-process transport.
///
/// # Safety
/// Deriving `Pod` + `Zeroable` guarantees at compile time:
/// - All fields are POD types (no pointers, no Drop)
/// - `#[repr(C)]` layout with explicit padding has no implicit padding bytes
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct ShmNotification {
    /// Channel ID
    pub channel_id: u32,
    /// Point type (Control=2, Adjustment=3)
    pub point_type: u8,
    /// Alignment padding
    pub _padding: [u8; 3],
    /// Point ID
    pub point_id: u32,
}

impl ShmNotification {
    /// Fixed message size
    pub const SIZE: usize = 12;

    /// Create a new notification
    pub fn new(channel_id: u32, point_type: PointType, point_id: u32) -> Self {
        Self {
            channel_id,
            point_type: point_type as u8,
            _padding: [0; 3],
            point_id,
        }
    }

    /// Convert to byte array (zero-copy, compile-time safety guarantee)
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        *bytemuck::bytes_of(self)
            .first_chunk()
            .expect("size matches")
    }

    /// Create from byte array (zero-copy, compile-time safety guarantee)
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        *bytemuck::from_bytes(bytes)
    }

    /// Get point type
    pub fn get_point_type(&self) -> Option<PointType> {
        match self.point_type {
            2 => Some(PointType::Control),
            3 => Some(PointType::Adjustment),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_size() {
        assert_eq!(
            std::mem::size_of::<ShmNotification>(),
            ShmNotification::SIZE
        );
    }

    #[test]
    fn test_notification_roundtrip() {
        let notif = ShmNotification::new(1001, PointType::Control, 42);
        let bytes = notif.to_bytes();
        let decoded = ShmNotification::from_bytes(&bytes);

        assert_eq!(notif, decoded);
        assert_eq!(decoded.channel_id, 1001);
        assert_eq!(decoded.get_point_type(), Some(PointType::Control));
        assert_eq!(decoded.point_id, 42);
    }
}
