//! Byte order utilities for CAN signal extraction
//!
//! Implements ABCD notation for byte ordering matching Modbus conventions

use super::types::ByteOrder;

/// Reorder bytes based on ABCD notation
pub fn reorder_bytes(data: &[u8], byte_order: &ByteOrder) -> Vec<u8> {
    match byte_order {
        ByteOrder::ABCD => {
            // Big Endian - no change needed
            data.to_vec()
        },
        ByteOrder::DCBA => {
            // Little Endian - reverse all bytes
            data.iter().rev().cloned().collect()
        },
        ByteOrder::CDAB => {
            // Middle Endian - swap 16-bit pairs
            if data.len() == 4 {
                vec![data[2], data[3], data[0], data[1]]
            } else {
                data.to_vec()
            }
        },
        ByteOrder::BADC => {
            // Middle Endian - swap bytes in pairs
            if data.len() == 4 {
                vec![data[1], data[0], data[3], data[2]]
            } else {
                let mut result = Vec::with_capacity(data.len());
                for chunk in data.chunks(2) {
                    if chunk.len() == 2 {
                        result.push(chunk[1]);
                        result.push(chunk[0]);
                    } else {
                        result.extend_from_slice(chunk);
                    }
                }
                result
            }
        },
        ByteOrder::BA => {
            // 16-bit Little Endian
            if data.len() >= 2 {
                vec![data[1], data[0]]
            } else {
                data.to_vec()
            }
        },
        ByteOrder::AB => {
            // 16-bit Big Endian - no change
            data[..2.min(data.len())].to_vec()
        },
    }
}

/// Convert bytes to u32 based on byte order
pub fn bytes_to_u32(data: &[u8], byte_order: &ByteOrder) -> u32 {
    let reordered = reorder_bytes(data, byte_order);
    let mut result = 0u32;

    for (i, &byte) in reordered.iter().take(4).enumerate() {
        match byte_order {
            ByteOrder::ABCD | ByteOrder::AB => {
                // Big Endian
                result |= (byte as u32) << (8 * (3 - i));
            },
            _ => {
                // Already reordered, treat as big endian
                result |= (byte as u32) << (8 * (3 - i));
            },
        }
    }

    result
}

/// Convert bytes to u64 based on byte order
pub fn bytes_to_u64(data: &[u8], byte_order: &ByteOrder) -> u64 {
    let reordered = reorder_bytes(data, byte_order);
    let mut result = 0u64;

    for (i, &byte) in reordered.iter().take(8).enumerate() {
        result |= (byte as u64) << (8 * (7 - i));
    }

    result
}

/// Convert bytes to f32 based on byte order
pub fn bytes_to_f32(data: &[u8], byte_order: &ByteOrder) -> f32 {
    if data.len() < 4 {
        return 0.0;
    }

    let reordered = reorder_bytes(&data[..4], byte_order);
    let bytes: [u8; 4] = [reordered[0], reordered[1], reordered[2], reordered[3]];
    f32::from_be_bytes(bytes)
}

/// Convert bytes to f64 based on byte order
pub fn bytes_to_f64(data: &[u8], byte_order: &ByteOrder) -> f64 {
    if data.len() < 8 {
        return 0.0;
    }

    let reordered = reorder_bytes(&data[..8], byte_order);
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&reordered[..8]);
    f64::from_be_bytes(bytes)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_byte_order_abcd() {
        let data = vec![0x12, 0x34, 0x56, 0x78];
        let reordered = reorder_bytes(&data, &ByteOrder::ABCD);
        assert_eq!(reordered, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_byte_order_dcba() {
        let data = vec![0x12, 0x34, 0x56, 0x78];
        let reordered = reorder_bytes(&data, &ByteOrder::DCBA);
        assert_eq!(reordered, vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_byte_order_cdab() {
        let data = vec![0x12, 0x34, 0x56, 0x78];
        let reordered = reorder_bytes(&data, &ByteOrder::CDAB);
        assert_eq!(reordered, vec![0x56, 0x78, 0x12, 0x34]);
    }

    #[test]
    fn test_byte_order_badc() {
        let data = vec![0x12, 0x34, 0x56, 0x78];
        let reordered = reorder_bytes(&data, &ByteOrder::BADC);
        assert_eq!(reordered, vec![0x34, 0x12, 0x78, 0x56]);
    }
}
