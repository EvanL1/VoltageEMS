//! UDS 事件通知协议
//!
//! 用于 modsrv → comsrv 的 M2C 命令通知。
//! 通知只携带"指针"信息（channel_id, point_type, point_id），
//! 实际数据从 SHM 读取。

use voltage_model::PointType;

/// M2C 命令通知（12 字节，固定大小）
///
/// 通过 Unix Domain Socket 发送，通知 comsrv 有新的 Control/Adjustment 命令。
/// 使用 `#[repr(C)]` 确保内存布局与 C 兼容，方便跨进程传输。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShmNotification {
    /// 通道 ID
    pub channel_id: u32,
    /// 点类型 (Control=2, Adjustment=3)
    pub point_type: u8,
    /// 对齐填充
    pub _padding: [u8; 3],
    /// 点 ID
    pub point_id: u32,
}

impl ShmNotification {
    /// 固定消息大小
    pub const SIZE: usize = 12;

    /// 创建新的通知
    pub fn new(channel_id: u32, point_type: PointType, point_id: u32) -> Self {
        Self {
            channel_id,
            point_type: point_type as u8,
            _padding: [0; 3],
            point_id,
        }
    }

    /// 转换为字节数组（零拷贝）
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        // Safety: ShmNotification 是 #[repr(C)]，大小固定为 12 字节
        unsafe { std::mem::transmute(*self) }
    }

    /// 从字节数组创建（零拷贝）
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        // Safety: 字节数组大小匹配，结构体是 POD 类型
        unsafe { std::mem::transmute(*bytes) }
    }

    /// 获取点类型
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
