//! Optimized Modbus PDU data structure
//!
//! 采用固定大小栈上数组，避免堆分配，提高性能

use crate::utils::error::{ComSrvError, Result};

/// 最大PDU长度（Modbus规范）
const MAX_PDU_SIZE: usize = 253;

/// 高性能PDU结构 - 栈上固定数组
#[derive(Debug, Clone)]
pub struct ModbusPdu {
    /// 固定大小缓冲区（栈上）
    data: [u8; MAX_PDU_SIZE],
    /// 实际数据长度
    len: usize,
}

impl ModbusPdu {
    /// 创建空PDU
    #[inline]
    pub fn new() -> Self {
        Self {
            data: [0; MAX_PDU_SIZE],
            len: 0,
        }
    }

    /// 从切片创建PDU
    #[inline]
    pub fn from_slice(data: &[u8]) -> Result<Self> {
        if data.len() > MAX_PDU_SIZE {
            return Err(ComSrvError::ProtocolError(format!(
                "PDU too large: {} bytes (max {})",
                data.len(),
                MAX_PDU_SIZE
            )));
        }

        let mut pdu = Self::new();
        pdu.data[..data.len()].copy_from_slice(data);
        pdu.len = data.len();
        Ok(pdu)
    }

    /// 添加单个字节
    #[inline]
    pub fn push(&mut self, byte: u8) -> Result<()> {
        if self.len >= MAX_PDU_SIZE {
            return Err(ComSrvError::ProtocolError("PDU buffer full".to_string()));
        }
        self.data[self.len] = byte;
        self.len += 1;
        Ok(())
    }

    /// 添加u16（大端序）
    #[inline]
    pub fn push_u16(&mut self, value: u16) -> Result<()> {
        self.push((value >> 8) as u8)?;
        self.push((value & 0xFF) as u8)?;
        Ok(())
    }

    /// 批量添加数据
    #[inline]
    pub fn extend(&mut self, data: &[u8]) -> Result<()> {
        if self.len + data.len() > MAX_PDU_SIZE {
            return Err(ComSrvError::ProtocolError(format!(
                "PDU would exceed max size: {} + {} > {}",
                self.len,
                data.len(),
                MAX_PDU_SIZE
            )));
        }
        self.data[self.len..self.len + data.len()].copy_from_slice(data);
        self.len += data.len();
        Ok(())
    }

    /// 获取数据切片
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// 获取可变数据切片
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data[..self.len]
    }

    /// 获取长度
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// 是否为空
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 清空PDU
    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
        // 可选：清零数据（安全考虑）
        // self.data[..].fill(0);
    }

    /// 获取功能码（第一个字节）
    #[inline]
    pub fn function_code(&self) -> Option<u8> {
        if self.len > 0 {
            Some(self.data[0])
        } else {
            None
        }
    }

    /// 检查是否为异常响应
    #[inline]
    pub fn is_exception(&self) -> bool {
        self.function_code()
            .map(|fc| fc & 0x80 != 0)
            .unwrap_or(false)
    }

    /// 获取异常码
    #[inline]
    pub fn exception_code(&self) -> Option<u8> {
        if self.is_exception() && self.len > 1 {
            Some(self.data[1])
        } else {
            None
        }
    }
}

impl Default for ModbusPdu {
    fn default() -> Self {
        Self::new()
    }
}

/// PDU构建器 - 流式API
pub struct PduBuilder {
    pdu: ModbusPdu,
}

impl Default for PduBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PduBuilder {
    /// 创建新构建器
    #[inline]
    pub fn new() -> Self {
        Self {
            pdu: ModbusPdu::new(),
        }
    }

    /// 设置功能码
    #[inline]
    pub fn function_code(mut self, fc: u8) -> Result<Self> {
        self.pdu.push(fc)?;
        Ok(self)
    }

    /// 添加地址
    #[inline]
    pub fn address(mut self, addr: u16) -> Result<Self> {
        self.pdu.push_u16(addr)?;
        Ok(self)
    }

    /// 添加数量
    #[inline]
    pub fn quantity(mut self, qty: u16) -> Result<Self> {
        self.pdu.push_u16(qty)?;
        Ok(self)
    }

    /// 添加字节
    #[inline]
    pub fn byte(mut self, b: u8) -> Result<Self> {
        self.pdu.push(b)?;
        Ok(self)
    }

    /// 添加数据
    #[inline]
    pub fn data(mut self, data: &[u8]) -> Result<Self> {
        self.pdu.extend(data)?;
        Ok(self)
    }

    /// 构建PDU
    #[inline]
    pub fn build(self) -> ModbusPdu {
        self.pdu
    }
}

/// 预分配的PDU池（可选优化）
pub struct PduPool {
    pool: Vec<ModbusPdu>,
    max_size: usize,
}

impl PduPool {
    /// 创建PDU池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// 获取PDU（从池中或新建）
    pub fn get(&mut self) -> ModbusPdu {
        self.pool.pop().unwrap_or_default()
    }

    /// 归还PDU到池中
    pub fn put(&mut self, mut pdu: ModbusPdu) {
        if self.pool.len() < self.max_size {
            pdu.clear();
            self.pool.push(pdu);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdu_basic_operations() {
        let mut pdu = ModbusPdu::new();
        assert_eq!(pdu.len(), 0);
        assert!(pdu.is_empty());

        // 添加功能码
        pdu.push(0x03).unwrap();
        assert_eq!(pdu.function_code(), Some(0x03));
        assert!(!pdu.is_exception());

        // 添加地址和数量
        pdu.push_u16(0x0100).unwrap();
        pdu.push_u16(0x000A).unwrap();

        assert_eq!(pdu.len(), 5);
        assert_eq!(pdu.as_slice(), &[0x03, 0x01, 0x00, 0x00, 0x0A]);
    }

    #[test]
    fn test_pdu_builder() {
        let pdu = PduBuilder::new()
            .function_code(0x03)
            .unwrap()
            .address(0x0100)
            .unwrap()
            .quantity(0x000A)
            .unwrap()
            .build();

        assert_eq!(pdu.len(), 5);
        assert_eq!(pdu.as_slice(), &[0x03, 0x01, 0x00, 0x00, 0x0A]);
    }

    #[test]
    fn test_exception_response() {
        let mut pdu = ModbusPdu::new();
        pdu.push(0x83).unwrap(); // FC 03 + 0x80
        pdu.push(0x02).unwrap(); // Exception code 2

        assert!(pdu.is_exception());
        assert_eq!(pdu.exception_code(), Some(0x02));
    }

    #[test]
    fn test_pdu_overflow() {
        let mut pdu = ModbusPdu::new();
        let large_data = vec![0xFF; MAX_PDU_SIZE + 1];

        let result = pdu.extend(&large_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_pdu_pool() {
        let mut pool = PduPool::new(5);

        // 获取PDU
        let mut pdu = pool.get();
        pdu.push(0x03).unwrap();
        assert_eq!(pdu.len(), 1);

        // 归还PDU
        pool.put(pdu);

        // 再次获取应该是清空的PDU
        let pdu2 = pool.get();
        assert!(pdu2.is_empty());
    }
}
