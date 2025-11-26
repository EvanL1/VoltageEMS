//! Numeric type conversions with byte order support
//!
//! Provides functions for converting between:
//! - Register arrays (u16[]) ↔ numeric types (f32, f64, u32, i32, etc.)
//! - Byte arrays (u8[]) ↔ numeric types
//! - BCD encoding ↔ decimal values
//!
//! All conversions support configurable byte order via the `ByteOrder` enum.

use super::ByteOrder;

// ============================================================================
// Register to Bytes Conversions
// ============================================================================

/// Convert 2 u16 registers to 4 bytes with specified byte order
pub fn regs_to_bytes_4(regs: &[u16; 2], order: ByteOrder) -> [u8; 4] {
    let [h0, h1] = [regs[0].to_be_bytes(), regs[1].to_be_bytes()];

    match order {
        ByteOrder::BigEndian => [h0[0], h0[1], h1[0], h1[1]], // ABCD
        ByteOrder::LittleEndian => [h1[1], h1[0], h0[1], h0[0]], // DCBA
        ByteOrder::BigEndianSwap => [h1[0], h1[1], h0[0], h0[1]], // CDAB
        ByteOrder::LittleEndianSwap => [h0[1], h0[0], h1[1], h1[0]], // BADC
        _ => panic!("Unsupported byte order for 32-bit conversion: {}", order),
    }
}

/// Convert 4 u16 registers to 8 bytes with specified byte order
pub fn regs_to_bytes_8(regs: &[u16; 4], order: ByteOrder) -> [u8; 8] {
    let [h0, h1, h2, h3] = [
        regs[0].to_be_bytes(),
        regs[1].to_be_bytes(),
        regs[2].to_be_bytes(),
        regs[3].to_be_bytes(),
    ];

    match order {
        ByteOrder::BigEndian => [
            h0[0], h0[1], h1[0], h1[1], h2[0], h2[1], h3[0], h3[1], // ABCDEFGH
        ],
        ByteOrder::LittleEndian => [
            h3[1], h3[0], h2[1], h2[0], h1[1], h1[0], h0[1], h0[0], // HGFEDCBA
        ],
        ByteOrder::BigEndianSwap => [
            h3[0], h3[1], h2[0], h2[1], h1[0], h1[1], h0[0], h0[1], // GHEFCDAB
        ],
        ByteOrder::LittleEndianSwap => [
            h0[1], h0[0], h1[1], h1[0], h2[1], h2[0], h3[1], h3[0], // BADCFEHG
        ],
        _ => panic!("Unsupported byte order for 64-bit conversion: {}", order),
    }
}

// ============================================================================
// Register to Numeric Type Conversions
// ============================================================================

/// Convert 2 u16 registers to f32
pub fn regs_to_f32(regs: &[u16; 2], order: ByteOrder) -> f32 {
    let bytes = regs_to_bytes_4(regs, order);
    f32::from_be_bytes(bytes)
}

/// Convert 4 u16 registers to f64
pub fn regs_to_f64(regs: &[u16; 4], order: ByteOrder) -> f64 {
    let bytes = regs_to_bytes_8(regs, order);
    f64::from_be_bytes(bytes)
}

/// Convert 2 u16 registers to u32
pub fn regs_to_u32(regs: &[u16; 2], order: ByteOrder) -> u32 {
    let bytes = regs_to_bytes_4(regs, order);
    u32::from_be_bytes(bytes)
}

/// Convert 2 u16 registers to i32
pub fn regs_to_i32(regs: &[u16; 2], order: ByteOrder) -> i32 {
    let bytes = regs_to_bytes_4(regs, order);
    i32::from_be_bytes(bytes)
}

/// Convert single u16 register to bytes
pub fn reg_to_bytes_2(reg: u16, order: ByteOrder) -> [u8; 2] {
    match order {
        ByteOrder::BigEndian | ByteOrder::BigEndian16 => reg.to_be_bytes(),
        ByteOrder::LittleEndian | ByteOrder::LittleEndian16 => reg.to_le_bytes(),
        _ => reg.to_be_bytes(),
    }
}

/// Convert single u16 register to u16 (with byte swapping if needed)
pub fn reg_to_u16(reg: u16, order: ByteOrder) -> u16 {
    match order {
        ByteOrder::LittleEndian16 => reg.swap_bytes(),
        _ => reg,
    }
}

/// Convert single u16 register to i16
pub fn reg_to_i16(reg: u16, order: ByteOrder) -> i16 {
    reg_to_u16(reg, order) as i16
}

// ============================================================================
// Byte Array to Numeric Type Conversions
// ============================================================================

/// Convert 4 bytes to f32 with specified byte order
pub fn bytes_to_f32(bytes: &[u8; 4], order: ByteOrder) -> f32 {
    match order {
        ByteOrder::BigEndian | ByteOrder::BigEndianSwap => f32::from_be_bytes(*bytes),
        ByteOrder::LittleEndian | ByteOrder::LittleEndianSwap => f32::from_le_bytes(*bytes),
        _ => f32::from_be_bytes(*bytes),
    }
}

/// Convert 8 bytes to f64 with specified byte order
pub fn bytes_to_f64(bytes: &[u8; 8], order: ByteOrder) -> f64 {
    match order {
        ByteOrder::BigEndian | ByteOrder::BigEndianSwap => f64::from_be_bytes(*bytes),
        ByteOrder::LittleEndian | ByteOrder::LittleEndianSwap => f64::from_le_bytes(*bytes),
        _ => f64::from_be_bytes(*bytes),
    }
}

/// Convert 4 bytes to u32 with specified byte order
pub fn bytes_to_u32(bytes: &[u8; 4], order: ByteOrder) -> u32 {
    match order {
        ByteOrder::BigEndian | ByteOrder::BigEndianSwap => u32::from_be_bytes(*bytes),
        ByteOrder::LittleEndian | ByteOrder::LittleEndianSwap => u32::from_le_bytes(*bytes),
        _ => u32::from_be_bytes(*bytes),
    }
}

/// Convert 4 bytes to i32 with specified byte order
pub fn bytes_to_i32(bytes: &[u8; 4], order: ByteOrder) -> i32 {
    match order {
        ByteOrder::BigEndian | ByteOrder::BigEndianSwap => i32::from_be_bytes(*bytes),
        ByteOrder::LittleEndian | ByteOrder::LittleEndianSwap => i32::from_le_bytes(*bytes),
        _ => i32::from_be_bytes(*bytes),
    }
}

/// Convert 2 bytes to u16 with specified byte order
pub fn bytes_to_u16(bytes: &[u8; 2], order: ByteOrder) -> u16 {
    match order {
        ByteOrder::BigEndian | ByteOrder::BigEndian16 | ByteOrder::BigEndianSwap => {
            u16::from_be_bytes(*bytes)
        },
        ByteOrder::LittleEndian | ByteOrder::LittleEndian16 | ByteOrder::LittleEndianSwap => {
            u16::from_le_bytes(*bytes)
        },
    }
}

/// Convert 2 bytes to i16 with specified byte order
pub fn bytes_to_i16(bytes: &[u8; 2], order: ByteOrder) -> i16 {
    match order {
        ByteOrder::BigEndian | ByteOrder::BigEndian16 | ByteOrder::BigEndianSwap => {
            i16::from_be_bytes(*bytes)
        },
        ByteOrder::LittleEndian | ByteOrder::LittleEndian16 | ByteOrder::LittleEndianSwap => {
            i16::from_le_bytes(*bytes)
        },
    }
}

// ============================================================================
// BCD Conversions
// ============================================================================

/// Convert BCD-encoded bytes to u32
///
/// BCD (Binary-Coded Decimal) represents each decimal digit as 4 bits.
/// Common in industrial devices for displaying human-readable values.
pub fn bcd_to_u32(bytes: &[u8]) -> Option<u32> {
    let mut result = 0u32;
    for byte in bytes {
        let high = (byte >> 4) & 0x0F;
        let low = byte & 0x0F;

        // Validate BCD digits
        if high > 9 || low > 9 {
            return None;
        }

        result = result
            .checked_mul(100)?
            .checked_add((high * 10 + low) as u32)?;
    }
    Some(result)
}

/// Convert u32 to BCD bytes
///
/// # Returns
/// Number of bytes written, or None if value doesn't fit or buffer too small.
pub fn u32_to_bcd(mut value: u32, bytes: &mut [u8]) -> Option<usize> {
    if value == 0 {
        if bytes.is_empty() {
            return None;
        }
        bytes[0] = 0;
        return Some(1);
    }

    let mut digits = Vec::new();
    while value > 0 {
        digits.push((value % 10) as u8);
        value /= 10;
    }

    // Pack digits into BCD bytes (2 digits per byte)
    let byte_count = digits.len().div_ceil(2);
    if byte_count > bytes.len() {
        return None;
    }

    for (i, chunk) in digits.chunks(2).enumerate() {
        let low = chunk[0];
        let high = chunk.get(1).copied().unwrap_or(0);
        bytes[byte_count - 1 - i] = (high << 4) | low;
    }

    Some(byte_count)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_regs_to_bytes_4_all_orders() {
        let regs = [0x1234, 0x5678];

        assert_eq!(
            regs_to_bytes_4(&regs, ByteOrder::BigEndian),
            [0x12, 0x34, 0x56, 0x78]
        );
        assert_eq!(
            regs_to_bytes_4(&regs, ByteOrder::LittleEndian),
            [0x78, 0x56, 0x34, 0x12]
        );
        assert_eq!(
            regs_to_bytes_4(&regs, ByteOrder::BigEndianSwap),
            [0x56, 0x78, 0x12, 0x34]
        );
        assert_eq!(
            regs_to_bytes_4(&regs, ByteOrder::LittleEndianSwap),
            [0x34, 0x12, 0x78, 0x56]
        );
    }

    #[test]
    fn test_regs_to_f32() {
        // 25.0 in IEEE 754: 0x41C80000
        let regs = [0x41C8, 0x0000];
        let value = regs_to_f32(&regs, ByteOrder::BigEndian);
        assert!((value - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_regs_to_u32() {
        let regs = [0x1234, 0x5678];
        assert_eq!(regs_to_u32(&regs, ByteOrder::BigEndian), 0x12345678);
        assert_eq!(regs_to_u32(&regs, ByteOrder::LittleEndian), 0x78563412);
    }

    #[test]
    fn test_bcd_roundtrip() {
        let test_values = [0, 1, 99, 1234, 567890];
        let mut bytes = [0u8; 8];

        for value in test_values {
            let len = u32_to_bcd(value, &mut bytes).unwrap();
            let decoded = bcd_to_u32(&bytes[0..len]).unwrap();
            assert_eq!(value, decoded);
        }
    }
}
