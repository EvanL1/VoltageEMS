//! Optimized Modbus PDU data structure
//!
//! Use a fixed-size stack array to avoid heap allocation and improve performance.

use crate::utils::error::{ComSrvError, Result};
use tracing::debug;

use super::constants;

/// Maximum PDU size (re-exported from constants for backward compatibility)
pub use constants::MAX_PDU_SIZE;

/// Maximum MBAP length field value (re-exported from constants for backward compatibility)
pub use constants::MAX_MBAP_LENGTH;

/// High-performance PDU with stack-allocated fixed array
#[derive(Debug, Clone)]
pub struct ModbusPdu {
    /// Fixed-size buffer (stack)
    data: [u8; MAX_PDU_SIZE],
    /// Actual data length
    len: usize,
}

impl ModbusPdu {
    /// Create an empty PDU
    #[inline]
    pub fn new() -> Self {
        Self {
            data: [0; MAX_PDU_SIZE],
            len: 0,
        }
    }

    /// Create a PDU from a byte slice
    #[inline]
    pub fn from_slice(data: &[u8]) -> Result<Self> {
        debug!("Parsing PDU from slice: {} bytes", data.len());

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

        // Log function code details
        if let Some(fc) = pdu.function_code() {
            let fc_desc = Self::function_code_description(fc);
            if pdu.is_exception() {
                let exc_code = pdu.exception_code().unwrap_or(0);
                debug!(
                    "PDU parsed: FC={:02X} (Exception: {}), exception_code={:02X}",
                    fc, fc_desc, exc_code
                );
            } else {
                debug!(
                    "PDU parsed: FC={:02X} ({}), data_len={}",
                    fc,
                    fc_desc,
                    pdu.len - 1
                );
            }
        } else {
            debug!("PDU parsed: empty PDU");
        }

        Ok(pdu)
    }

    /// Push a single byte
    #[inline]
    pub fn push(&mut self, byte: u8) -> Result<()> {
        if self.len >= MAX_PDU_SIZE {
            return Err(ComSrvError::ProtocolError("PDU buffer full".to_string()));
        }
        self.data[self.len] = byte;
        self.len += 1;
        Ok(())
    }

    /// Push u16 in big-endian
    #[inline]
    pub fn push_u16(&mut self, value: u16) -> Result<()> {
        self.push((value >> 8) as u8)?;
        self.push((value & 0xFF) as u8)?;
        Ok(())
    }

    /// Extend with a byte slice
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

    /// Get immutable data slice
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// Get mutable data slice
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data[..self.len]
    }

    /// Get current length
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear PDU
    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
        // Optional: zero data for security
        // self.data[..].fill(0);
    }

    /// Get function code (first byte)
    #[inline]
    pub fn function_code(&self) -> Option<u8> {
        if self.len > 0 {
            Some(self.data[0])
        } else {
            None
        }
    }

    /// Check if exception response
    #[inline]
    pub fn is_exception(&self) -> bool {
        self.function_code()
            .map(|fc| fc & 0x80 != 0)
            .unwrap_or(false)
    }

    /// Get exception code
    #[inline]
    pub fn exception_code(&self) -> Option<u8> {
        if self.is_exception() && self.len > 1 {
            Some(self.data[1])
        } else {
            None
        }
    }

    /// Get human-readable function code description
    fn function_code_description(fc: u8) -> &'static str {
        match fc & 0x7F {
            // Remove exception bit for lookup
            0x01 => "Read Coils",
            0x02 => "Read Discrete Inputs",
            0x03 => "Read Holding Registers",
            0x04 => "Read Input Registers",
            0x05 => "Write Single Coil",
            0x06 => "Write Single Register",
            0x0F => "Write Multiple Coils",
            0x10 => "Write Multiple Registers",
            0x17 => "Read/Write Multiple Registers",
            _ => "Unknown Function",
        }
    }
}

impl Default for ModbusPdu {
    fn default() -> Self {
        Self::new()
    }
}

/// PDU builder - fluent API
pub struct PduBuilder {
    pdu: ModbusPdu,
}

impl Default for PduBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PduBuilder {
    /// Create a new builder
    #[inline]
    pub fn new() -> Self {
        Self {
            pdu: ModbusPdu::new(),
        }
    }

    /// Set function code
    #[inline]
    pub fn function_code(mut self, fc: u8) -> Result<Self> {
        self.pdu.push(fc)?;
        Ok(self)
    }

    /// Add address
    #[inline]
    pub fn address(mut self, addr: u16) -> Result<Self> {
        self.pdu.push_u16(addr)?;
        Ok(self)
    }

    /// Add quantity
    #[inline]
    pub fn quantity(mut self, qty: u16) -> Result<Self> {
        self.pdu.push_u16(qty)?;
        Ok(self)
    }

    /// Add a byte
    #[inline]
    pub fn byte(mut self, b: u8) -> Result<Self> {
        self.pdu.push(b)?;
        Ok(self)
    }

    /// Add data
    #[inline]
    pub fn data(mut self, data: &[u8]) -> Result<Self> {
        self.pdu.extend(data)?;
        Ok(self)
    }

    /// Build the PDU
    #[inline]
    pub fn build(self) -> ModbusPdu {
        // Log PDU construction details
        if let Some(fc) = self.pdu.function_code() {
            let fc_desc = ModbusPdu::function_code_description(fc);
            debug!(
                "PDU built: FC={:02X} ({}), total_len={}",
                fc,
                fc_desc,
                self.pdu.len()
            );
        } else {
            debug!("PDU built: empty PDU");
        }

        self.pdu
    }
}

/// Pre-allocated PDU pool (optional optimization)
pub struct PduPool {
    pool: Vec<ModbusPdu>,
    max_size: usize,
}

impl PduPool {
    /// Create PDU pool
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Get a PDU (from pool or new)
    pub fn get(&mut self) -> ModbusPdu {
        self.pool.pop().unwrap_or_default()
    }

    /// Return PDU to pool
    pub fn put(&mut self, mut pdu: ModbusPdu) {
        if self.pool.len() < self.max_size {
            pdu.clear();
            self.pool.push(pdu);
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_pdu_basic_operations() {
        let mut pdu = ModbusPdu::new();
        assert_eq!(pdu.len(), 0);
        assert!(pdu.is_empty());

        // Add function code
        pdu.push(0x03).unwrap();
        assert_eq!(pdu.function_code(), Some(0x03));
        assert!(!pdu.is_exception());

        // Add address and quantity
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

        // Get PDU
        let mut pdu = pool.get();
        pdu.push(0x03).unwrap();
        assert_eq!(pdu.len(), 1);

        // Return PDU
        pool.put(pdu);

        // Getting again should return a cleared PDU
        let pdu2 = pool.get();
        assert!(pdu2.is_empty());
    }

    #[test]
    fn test_pdu_from_slice_valid() {
        let data = vec![0x03, 0x01, 0x00, 0x00, 0x0A];
        let pdu = ModbusPdu::from_slice(&data).unwrap();

        assert_eq!(pdu.len(), 5);
        assert_eq!(pdu.as_slice(), &data[..]);
        assert_eq!(pdu.function_code(), Some(0x03));
    }

    #[test]
    fn test_pdu_from_slice_too_large() {
        let large_data = vec![0xFF; MAX_PDU_SIZE + 1];
        let result = ModbusPdu::from_slice(&large_data);

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("PDU too large"));
        }
    }

    #[test]
    fn test_pdu_from_slice_empty() {
        let pdu = ModbusPdu::from_slice(&[]).unwrap();
        assert_eq!(pdu.len(), 0);
        assert!(pdu.is_empty());
    }

    #[test]
    fn test_pdu_from_slice_max_size() {
        let data = vec![0x01; MAX_PDU_SIZE];
        let pdu = ModbusPdu::from_slice(&data).unwrap();

        assert_eq!(pdu.len(), MAX_PDU_SIZE);
        assert_eq!(pdu.as_slice().len(), MAX_PDU_SIZE);
    }

    #[test]
    fn test_pdu_push_until_full() {
        let mut pdu = ModbusPdu::new();

        // Fill to capacity
        for i in 0..MAX_PDU_SIZE {
            let result = pdu.push(i as u8);
            assert!(result.is_ok(), "Push #{} should succeed", i);
        }

        assert_eq!(pdu.len(), MAX_PDU_SIZE);

        // Next push should fail
        let result = pdu.push(0xFF);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("buffer full"));
        }
    }

    #[test]
    fn test_pdu_push_u16_boundary_values() {
        let mut pdu = ModbusPdu::new();

        // Test minimum value
        pdu.push_u16(0x0000).unwrap();
        assert_eq!(pdu.as_slice(), &[0x00, 0x00]);

        pdu.clear();

        // Test maximum value
        pdu.push_u16(0xFFFF).unwrap();
        assert_eq!(pdu.as_slice(), &[0xFF, 0xFF]);

        pdu.clear();

        // Test typical value
        pdu.push_u16(0x1234).unwrap();
        assert_eq!(pdu.as_slice(), &[0x12, 0x34]);
    }

    #[test]
    fn test_pdu_push_u16_near_capacity() {
        let mut pdu = ModbusPdu::new();

        // Fill to near capacity (leave room for 1 byte)
        for _ in 0..(MAX_PDU_SIZE - 1) {
            pdu.push(0x00).unwrap();
        }

        // push_u16 needs 2 bytes, should fail
        let result = pdu.push_u16(0x1234);
        assert!(result.is_err());
    }

    #[test]
    fn test_pdu_extend_multiple_times() {
        let mut pdu = ModbusPdu::new();

        pdu.extend(&[0x01, 0x02]).unwrap();
        assert_eq!(pdu.len(), 2);

        pdu.extend(&[0x03, 0x04, 0x05]).unwrap();
        assert_eq!(pdu.len(), 5);

        pdu.extend(&[0x06]).unwrap();
        assert_eq!(pdu.len(), 6);

        assert_eq!(pdu.as_slice(), &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    }

    #[test]
    fn test_pdu_extend_exact_capacity() {
        let mut pdu = ModbusPdu::new();
        let data = vec![0xAA; MAX_PDU_SIZE];

        let result = pdu.extend(&data);
        assert!(result.is_ok());
        assert_eq!(pdu.len(), MAX_PDU_SIZE);
    }

    #[test]
    fn test_pdu_extend_exceed_capacity() {
        let mut pdu = ModbusPdu::new();
        pdu.extend(&[0x01, 0x02]).unwrap();

        let large_data = vec![0xFF; MAX_PDU_SIZE];
        let result = pdu.extend(&large_data);

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("exceed max size"));
        }

        // Original data should remain unchanged
        assert_eq!(pdu.len(), 2);
        assert_eq!(pdu.as_slice(), &[0x01, 0x02]);
    }

    #[test]
    fn test_pdu_clear() {
        let mut pdu = ModbusPdu::new();
        pdu.push(0x03).unwrap();
        pdu.push_u16(0x1234).unwrap();

        assert_eq!(pdu.len(), 3);
        assert!(!pdu.is_empty());

        pdu.clear();

        assert_eq!(pdu.len(), 0);
        assert!(pdu.is_empty());
        assert_eq!(pdu.function_code(), None);
    }

    #[test]
    fn test_pdu_as_mut_slice() {
        let mut pdu = ModbusPdu::new();
        pdu.extend(&[0x01, 0x02, 0x03]).unwrap();

        // Modify via mutable slice
        let slice = pdu.as_mut_slice();
        slice[1] = 0xFF;

        assert_eq!(pdu.as_slice(), &[0x01, 0xFF, 0x03]);
    }

    #[test]
    fn test_pdu_function_code_empty() {
        let pdu = ModbusPdu::new();
        assert_eq!(pdu.function_code(), None);
    }

    #[test]
    fn test_pdu_function_code_various() {
        let test_cases = vec![
            (0x01, false), // Read Coils
            (0x03, false), // Read Holding Registers
            (0x05, false), // Write Single Coil
            (0x06, false), // Write Single Register
            (0x0F, false), // Write Multiple Coils
            (0x10, false), // Write Multiple Registers
            (0x81, true),  // Exception response
            (0x83, true),  // Exception response
            (0x8F, true),  // Exception response
        ];

        for (fc, is_exception) in test_cases {
            let mut pdu = ModbusPdu::new();
            pdu.push(fc).unwrap();

            assert_eq!(pdu.function_code(), Some(fc));
            assert_eq!(pdu.is_exception(), is_exception);
        }
    }

    #[test]
    fn test_pdu_exception_code_valid() {
        let exception_codes = vec![
            0x01, // Illegal Function
            0x02, // Illegal Data Address
            0x03, // Illegal Data Value
            0x04, // Server Device Failure
        ];

        for exc_code in exception_codes {
            let mut pdu = ModbusPdu::new();
            pdu.push(0x83).unwrap(); // Exception FC03
            pdu.push(exc_code).unwrap();

            assert!(pdu.is_exception());
            assert_eq!(pdu.exception_code(), Some(exc_code));
        }
    }

    #[test]
    fn test_pdu_exception_code_without_code_byte() {
        let mut pdu = ModbusPdu::new();
        pdu.push(0x83).unwrap(); // Exception but no code byte

        assert!(pdu.is_exception());
        assert_eq!(pdu.exception_code(), None);
    }

    #[test]
    fn test_pdu_exception_code_normal_response() {
        let mut pdu = ModbusPdu::new();
        pdu.push(0x03).unwrap(); // Normal FC03
        pdu.push(0x02).unwrap();

        assert!(!pdu.is_exception());
        assert_eq!(pdu.exception_code(), None);
    }

    #[test]
    fn test_pdu_default() {
        let pdu = ModbusPdu::default();
        assert_eq!(pdu.len(), 0);
        assert!(pdu.is_empty());
    }

    #[test]
    fn test_pdu_clone() {
        let mut original = ModbusPdu::new();
        original.extend(&[0x03, 0x01, 0x00]).unwrap();

        let cloned = original.clone();

        assert_eq!(cloned.len(), original.len());
        assert_eq!(cloned.as_slice(), original.as_slice());

        // Ensure deep copy (modify original shouldn't affect clone)
        original.push(0xFF).unwrap();
        assert_ne!(cloned.len(), original.len());
    }

    #[test]
    fn test_pdu_builder_empty() {
        let builder = PduBuilder::new();
        let pdu = builder.build();

        assert!(pdu.is_empty());
        assert_eq!(pdu.len(), 0);
    }

    #[test]
    fn test_pdu_builder_chained() {
        let pdu = PduBuilder::new()
            .function_code(0x10)
            .unwrap()
            .address(0x0100)
            .unwrap()
            .quantity(0x0002)
            .unwrap()
            .byte(0x04)
            .unwrap()
            .data(&[0x00, 0x0A, 0x01, 0x02])
            .unwrap()
            .build();

        assert_eq!(pdu.len(), 10);
        assert_eq!(
            pdu.as_slice(),
            &[0x10, 0x01, 0x00, 0x00, 0x02, 0x04, 0x00, 0x0A, 0x01, 0x02]
        );
    }

    #[test]
    fn test_pdu_builder_error_propagation() {
        // Try to overflow via builder
        let result = PduBuilder::new()
            .function_code(0x03)
            .and_then(|b| b.data(&vec![0xFF; MAX_PDU_SIZE]));

        assert!(result.is_err());
    }

    #[test]
    fn test_pdu_builder_default() {
        let builder = PduBuilder::default();
        let pdu = builder.build();
        assert!(pdu.is_empty());
    }

    #[test]
    fn test_pdu_pool_multiple_get_put() {
        let mut pool = PduPool::new(3);

        // Get multiple PDUs
        let mut pdu1 = pool.get();
        let mut pdu2 = pool.get();
        let mut pdu3 = pool.get();

        pdu1.push(0x01).unwrap();
        pdu2.push(0x02).unwrap();
        pdu3.push(0x03).unwrap();

        // Return to pool
        pool.put(pdu1);
        pool.put(pdu2);
        pool.put(pdu3);

        // Get again - should be cleared
        let pdu_new = pool.get();
        assert!(pdu_new.is_empty());
    }

    #[test]
    fn test_pdu_pool_max_size_limit() {
        let mut pool = PduPool::new(2);

        let pdu1 = pool.get();
        let pdu2 = pool.get();
        let pdu3 = pool.get();

        // Return 3 PDUs to pool with max_size=2
        pool.put(pdu1);
        pool.put(pdu2);
        pool.put(pdu3); // This should be discarded

        // Only 2 should be in pool
        let retrieved1 = pool.get();
        let retrieved2 = pool.get();
        let retrieved3 = pool.get(); // This creates a new one

        assert!(retrieved1.is_empty());
        assert!(retrieved2.is_empty());
        assert!(retrieved3.is_empty());
    }

    #[test]
    fn test_pdu_pool_concurrent_usage() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let pool = Arc::new(Mutex::new(PduPool::new(10)));
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let pool_clone = Arc::clone(&pool);
                thread::spawn(move || {
                    let mut pdu = pool_clone.lock().unwrap().get();
                    pdu.push(i as u8).unwrap();
                    assert_eq!(pdu.len(), 1);
                    pool_clone.lock().unwrap().put(pdu);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All PDUs should be returned and cleared
        let pdu = pool.lock().unwrap().get();
        assert!(pdu.is_empty());
    }
}
