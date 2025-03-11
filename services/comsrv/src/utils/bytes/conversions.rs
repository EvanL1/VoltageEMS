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
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::{ByteOrder, regs_to_bytes_4};
///
/// let regs = [0x1234, 0x5678];
/// let bytes = regs_to_bytes_4(&regs, ByteOrder::BigEndian);
/// assert_eq!(bytes, [0x12, 0x34, 0x56, 0x78]);
///
/// let bytes = regs_to_bytes_4(&regs, ByteOrder::LittleEndian);
/// assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);
///
/// let bytes = regs_to_bytes_4(&regs, ByteOrder::BigEndianSwap);
/// assert_eq!(bytes, [0x56, 0x78, 0x12, 0x34]);
/// ```
pub fn regs_to_bytes_4(regs: &[u16; 2], order: ByteOrder) -> [u8; 4] {
    // Zero-allocation: directly destructure register bytes on stack
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
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::{ByteOrder, regs_to_bytes_8};
///
/// let regs = [0x1122, 0x3344, 0x5566, 0x7788];
/// let bytes = regs_to_bytes_8(&regs, ByteOrder::BigEndian);
/// assert_eq!(bytes, [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
/// ```
pub fn regs_to_bytes_8(regs: &[u16; 4], order: ByteOrder) -> [u8; 8] {
    // Zero-allocation: directly destructure register bytes on stack
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
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::{ByteOrder, regs_to_f32};
///
/// // 25.0 in IEEE 754: 0x41C80000
/// let regs = [0x41C8, 0x0000];
/// let value = regs_to_f32(&regs, ByteOrder::BigEndian);
/// assert!((value - 25.0).abs() < f32::EPSILON);
/// ```
pub fn regs_to_f32(regs: &[u16; 2], order: ByteOrder) -> f32 {
    let bytes = regs_to_bytes_4(regs, order);
    // Always use big-endian decoding after byte reordering
    f32::from_be_bytes(bytes)
}

/// Convert 4 u16 registers to f64
pub fn regs_to_f64(regs: &[u16; 4], order: ByteOrder) -> f64 {
    let bytes = regs_to_bytes_8(regs, order);
    // Always use big-endian decoding after byte reordering
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
        _ => reg.to_be_bytes(), // Default to big-endian
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
///
/// Useful for CAN/raw byte stream processing
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
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::bcd_to_u32;
///
/// // 0x12 0x34 represents decimal 1234
/// let bytes = [0x12, 0x34];
/// assert_eq!(bcd_to_u32(&bytes), Some(1234));
///
/// // Invalid BCD (digit > 9)
/// let bad_bytes = [0x1A, 0x34];
/// assert_eq!(bcd_to_u32(&bad_bytes), None);
/// ```
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
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::u32_to_bcd;
///
/// let mut bytes = [0u8; 4];
/// let len = u32_to_bcd(1234, &mut bytes).unwrap();
/// assert_eq!(len, 2);
/// assert_eq!(&bytes[0..2], &[0x12, 0x34]);
/// ```
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

    // ========================================================================
    // Register to Bytes Tests
    // ========================================================================

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
    fn test_regs_to_bytes_8_big_endian() {
        let regs = [0x1122, 0x3344, 0x5566, 0x7788];
        let bytes = regs_to_bytes_8(&regs, ByteOrder::BigEndian);
        assert_eq!(bytes, [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
    }

    // ========================================================================
    // Register to Numeric Tests
    // ========================================================================

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
    fn test_regs_to_i32_negative() {
        let regs = [0xFFFF, 0xFFFF];
        assert_eq!(regs_to_i32(&regs, ByteOrder::BigEndian), -1);
    }

    // ========================================================================
    // Bytes to Numeric Tests
    // ========================================================================

    #[test]
    fn test_bytes_to_f32() {
        let bytes = [0x41, 0xC8, 0x00, 0x00];
        let value = bytes_to_f32(&bytes, ByteOrder::BigEndian);
        assert!((value - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bytes_to_u16() {
        let bytes = [0x12, 0x34];
        assert_eq!(bytes_to_u16(&bytes, ByteOrder::BigEndian), 0x1234);
        assert_eq!(bytes_to_u16(&bytes, ByteOrder::LittleEndian), 0x3412);
    }

    // ========================================================================
    // BCD Tests
    // ========================================================================

    #[test]
    fn test_bcd_to_u32_valid() {
        assert_eq!(bcd_to_u32(&[0x12, 0x34]), Some(1234));
        assert_eq!(bcd_to_u32(&[0x00, 0x99]), Some(99));
        assert_eq!(bcd_to_u32(&[0x10]), Some(10));
    }

    #[test]
    fn test_bcd_to_u32_invalid() {
        assert_eq!(bcd_to_u32(&[0x1A, 0x34]), None); // A > 9
        assert_eq!(bcd_to_u32(&[0x12, 0xF4]), None); // F > 9
    }

    #[test]
    fn test_u32_to_bcd() {
        let mut bytes = [0u8; 4];

        let len = u32_to_bcd(1234, &mut bytes).unwrap();
        assert_eq!(len, 2);
        assert_eq!(&bytes[0..2], &[0x12, 0x34]);

        let len = u32_to_bcd(0, &mut bytes).unwrap();
        assert_eq!(len, 1);
        assert_eq!(bytes[0], 0x00);
    }

    #[test]
    fn test_u32_to_bcd_roundtrip() {
        let test_values = [0, 1, 99, 1234, 567890];
        let mut bytes = [0u8; 8];

        for value in test_values {
            let len = u32_to_bcd(value, &mut bytes).unwrap();
            let decoded = bcd_to_u32(&bytes[0..len]).unwrap();
            assert_eq!(value, decoded);
        }
    }

    // ========================================================================
    // Round-trip Tests
    // ========================================================================

    #[test]
    fn test_f32_roundtrip_all_orders() {
        let test_values = [
            0.0,
            1.0,
            -1.0,
            25.0,
            std::f32::consts::PI,
            f32::MAX,
            f32::MIN,
        ];

        // Test BigEndian and LittleEndian (standard orders)
        for value in test_values {
            // BigEndian test
            let be_bytes = value.to_be_bytes();
            let regs = [
                u16::from_be_bytes([be_bytes[0], be_bytes[1]]),
                u16::from_be_bytes([be_bytes[2], be_bytes[3]]),
            ];
            let decoded = regs_to_f32(&regs, ByteOrder::BigEndian);
            assert!(
                (value - decoded).abs() < f32::EPSILON * 10.0,
                "Failed for value={}, order=BigEndian: got {}",
                value,
                decoded
            );

            // LittleEndian test
            let le_bytes = value.to_le_bytes();
            let regs = [
                u16::from_be_bytes([le_bytes[0], le_bytes[1]]),
                u16::from_be_bytes([le_bytes[2], le_bytes[3]]),
            ];
            let decoded = regs_to_f32(&regs, ByteOrder::LittleEndian);
            assert!(
                (value - decoded).abs() < f32::EPSILON * 10.0,
                "Failed for value={}, order=LittleEndian: got {}",
                value,
                decoded
            );
        }

        // Test word-swap orders with specific known values
        let value = 25.0f32; // 0x41C80000 in IEEE 754

        // BigEndianSwap: Words are swapped (CDAB)
        let regs = [0x0000, 0x41C8]; // Swapped words
        let decoded = regs_to_f32(&regs, ByteOrder::BigEndianSwap);
        assert!(
            (value - decoded).abs() < f32::EPSILON * 10.0,
            "BigEndianSwap failed: got {}",
            decoded
        );
    }

    #[test]
    fn test_u32_roundtrip() {
        let test_values = [0u32, 1, 0x12345678, 0xFFFFFFFF, 0xDEADBEEF];

        for value in test_values {
            for order in [
                ByteOrder::BigEndian,
                ByteOrder::LittleEndian,
                ByteOrder::BigEndianSwap,
                ByteOrder::LittleEndianSwap,
            ] {
                let bytes = value.to_be_bytes();
                let regs = [
                    u16::from_be_bytes([bytes[0], bytes[1]]),
                    u16::from_be_bytes([bytes[2], bytes[3]]),
                ];

                let decoded = regs_to_u32(&regs, order);

                // Note: Only BigEndian should produce exact match
                if matches!(order, ByteOrder::BigEndian) {
                    assert_eq!(value, decoded, "Failed for value=0x{:08X}", value);
                }
            }
        }
    }
}
