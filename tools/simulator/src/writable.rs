//! Writable registers storage for Modbus write operations.
//!
//! When a Modbus master writes to registers (FC=0x06/0x10), the values are
//! stored here. Subsequent reads will return the stored values instead of
//! generated values for these addresses.

use std::collections::HashMap;
use std::sync::RwLock;

/// Storage for writable register values.
///
/// This provides a simple key-value store where:
/// - Key: (unit_id, register_address)
/// - Value: u16 register value
///
/// Thread-safe using RwLock for concurrent read/write access.
pub struct WritableRegisters {
    /// Inner storage: (unit_id, address) -> value
    values: RwLock<HashMap<(u8, u16), u16>>,
}

impl WritableRegisters {
    /// Create a new empty writable registers storage.
    pub fn new() -> Self {
        Self {
            values: RwLock::new(HashMap::new()),
        }
    }

    /// Write a single register value.
    pub fn write_single(&self, unit_id: u8, address: u16, value: u16) {
        let mut values = self.values.write().unwrap_or_else(|e| e.into_inner());
        values.insert((unit_id, address), value);
    }

    /// Write multiple consecutive registers.
    pub fn write_multiple(&self, unit_id: u8, start_address: u16, values_to_write: &[u16]) {
        let mut values = self.values.write().unwrap_or_else(|e| e.into_inner());
        for (offset, &value) in values_to_write.iter().enumerate() {
            let addr = start_address.wrapping_add(offset as u16);
            values.insert((unit_id, addr), value);
        }
    }

    /// Read a single register value if it was previously written.
    ///
    /// Returns `Some(value)` if the register was written, `None` otherwise.
    pub fn read(&self, unit_id: u8, address: u16) -> Option<u16> {
        let values = self.values.read().unwrap_or_else(|e| e.into_inner());
        values.get(&(unit_id, address)).copied()
    }

    /// Read multiple consecutive registers.
    ///
    /// Returns a Vec where each element is `Some(value)` if written, `None` if not.
    #[allow(dead_code)]
    pub fn read_multiple(&self, unit_id: u8, start_address: u16, count: u16) -> Vec<Option<u16>> {
        let values = self.values.read().unwrap_or_else(|e| e.into_inner());
        (0..count)
            .map(|offset| {
                let addr = start_address.wrapping_add(offset);
                values.get(&(unit_id, addr)).copied()
            })
            .collect()
    }

    /// Clear all stored values (useful for testing).
    #[allow(dead_code)]
    pub fn clear(&self) {
        let mut values = self.values.write().unwrap_or_else(|e| e.into_inner());
        values.clear();
    }

    /// Get the number of stored values.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let values = self.values.read().unwrap_or_else(|e| e.into_inner());
        values.len()
    }
}

impl Default for WritableRegisters {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_single() {
        let wr = WritableRegisters::new();
        wr.write_single(1, 1024, 4500);

        assert_eq!(wr.read(1, 1024), Some(4500));
        assert_eq!(wr.read(1, 1025), None);
        assert_eq!(wr.read(2, 1024), None); // Different unit_id
    }

    #[test]
    fn test_write_multiple() {
        let wr = WritableRegisters::new();
        wr.write_multiple(1, 100, &[1000, 2000, 3000]);

        assert_eq!(wr.read(1, 100), Some(1000));
        assert_eq!(wr.read(1, 101), Some(2000));
        assert_eq!(wr.read(1, 102), Some(3000));
        assert_eq!(wr.read(1, 103), None);
    }

    #[test]
    fn test_read_multiple() {
        let wr = WritableRegisters::new();
        wr.write_single(1, 101, 500);

        let values = wr.read_multiple(1, 100, 3);
        assert_eq!(values, vec![None, Some(500), None]);
    }

    #[test]
    fn test_overwrite() {
        let wr = WritableRegisters::new();
        wr.write_single(1, 1024, 100);
        wr.write_single(1, 1024, 200);

        assert_eq!(wr.read(1, 1024), Some(200));
    }
}
