//! Bit-level operations for industrial protocol data
//!
//! Provides functions for extracting and inserting bits from/to byte arrays.
//! Common use cases:
//! - CAN signal extraction from message data
//! - Modbus coil/discrete input processing
//! - Status register bit manipulation

/// Extract single bit from u16 value
#[inline]
pub fn extract_bit_u16(value: u16, bit_index: u8) -> bool {
    debug_assert!(bit_index < 16, "Bit index out of range: {}", bit_index);
    (value & (1 << bit_index)) != 0
}

/// Extract single bit from u8 value
#[inline]
pub fn extract_bit_u8(value: u8, bit_index: u8) -> bool {
    debug_assert!(bit_index < 8, "Bit index out of range: {}", bit_index);
    (value & (1 << bit_index)) != 0
}

/// Extract multi-bit value from byte array (LSB-first bit ordering)
///
/// Extracts a value of up to 64 bits from a byte array, starting at any bit position.
/// Uses LSB-first (Intel/little-endian) bit ordering within each byte.
///
/// # Arguments
/// * `bytes` - Source byte array
/// * `start_bit` - Starting bit position (0-indexed)
/// * `bit_length` - Number of bits to extract (1-64)
///
/// # Returns
/// Extracted value as u64 (use type casting for smaller types)
pub fn extract_bits(bytes: &[u8], start_bit: u16, bit_length: u8) -> u64 {
    debug_assert!(bit_length <= 64, "Bit length out of range: {}", bit_length);
    debug_assert!(bit_length > 0, "Bit length must be greater than 0");

    let mut result = 0u64;

    for i in 0..bit_length {
        let bit_position = start_bit + i as u16;
        let byte_index = (bit_position / 8) as usize;
        let bit_index = (bit_position % 8) as u8;

        if byte_index < bytes.len() {
            let bit = (bytes[byte_index] >> bit_index) & 0x01;
            result |= (bit as u64) << i;
        }
    }

    result
}

/// Extract signed multi-bit value from byte array
///
/// Similar to `extract_bits`, but performs sign extension for signed values.
pub fn extract_bits_signed(bytes: &[u8], start_bit: u16, bit_length: u8) -> i64 {
    debug_assert!(bit_length <= 64, "Bit length out of range: {}", bit_length);
    debug_assert!(bit_length > 0, "Bit length must be greater than 0");

    let unsigned = extract_bits(bytes, start_bit, bit_length);

    // Check sign bit
    let sign_bit = 1u64 << (bit_length - 1);
    if unsigned & sign_bit != 0 {
        // Negative value - sign extend
        let mask = !((1u64 << bit_length) - 1);
        (unsigned | mask) as i64
    } else {
        unsigned as i64
    }
}

/// Insert bits into byte array (for write operations)
///
/// Inserts a value of up to 64 bits into a byte array at any bit position.
/// Uses LSB-first bit ordering to match `extract_bits`.
///
/// # Arguments
/// * `bytes` - Target byte array (modified in-place)
/// * `start_bit` - Starting bit position (0-indexed)
/// * `bit_length` - Number of bits to insert (1-64)
/// * `value` - Value to insert
pub fn insert_bits(bytes: &mut [u8], start_bit: u16, bit_length: u8, value: u64) {
    debug_assert!(bit_length <= 64, "Bit length out of range: {}", bit_length);
    debug_assert!(bit_length > 0, "Bit length must be greater than 0");

    for i in 0..bit_length {
        let bit_position = start_bit + i as u16;
        let byte_index = (bit_position / 8) as usize;
        let bit_index = (bit_position % 8) as u8;

        if byte_index < bytes.len() {
            let bit_value = ((value >> i) & 0x01) as u8;

            // Clear the target bit first
            bytes[byte_index] &= !(1 << bit_index);

            // Set the bit if value is 1
            if bit_value == 1 {
                bytes[byte_index] |= 1 << bit_index;
            }
        }
    }
}

/// Set a single bit in a byte array
#[inline]
pub fn set_bit(bytes: &mut [u8], bit_position: u16) {
    let byte_index = (bit_position / 8) as usize;
    let bit_index = (bit_position % 8) as u8;

    if byte_index < bytes.len() {
        bytes[byte_index] |= 1 << bit_index;
    }
}

/// Clear a single bit in a byte array
#[inline]
pub fn clear_bit(bytes: &mut [u8], bit_position: u16) {
    let byte_index = (bit_position / 8) as usize;
    let bit_index = (bit_position % 8) as u8;

    if byte_index < bytes.len() {
        bytes[byte_index] &= !(1 << bit_index);
    }
}

/// Toggle a single bit in a byte array
#[inline]
pub fn toggle_bit(bytes: &mut [u8], bit_position: u16) {
    let byte_index = (bit_position / 8) as usize;
    let bit_index = (bit_position % 8) as u8;

    if byte_index < bytes.len() {
        bytes[byte_index] ^= 1 << bit_index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bit_u16() {
        let value = 0b1010_1100u16;
        assert!(!extract_bit_u16(value, 0));
        assert!(extract_bit_u16(value, 2));
        assert!(extract_bit_u16(value, 3));
    }

    #[test]
    fn test_extract_bits_basic() {
        let data = [0b10110101, 0b11001100];

        let result = extract_bits(&data, 2, 4);
        assert_eq!(result, 0b1101);

        let result = extract_bits(&data, 4, 8);
        assert_eq!(result, 0b11001011);
    }

    #[test]
    fn test_extract_bits_signed() {
        let data = [0b11111111];
        let result = extract_bits_signed(&data, 0, 4);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_insert_bits_basic() {
        let mut data = [0u8; 2];
        insert_bits(&mut data, 2, 4, 0b1101);
        assert_eq!(data[0], 0b00110100);
    }

    #[test]
    fn test_roundtrip_extract_insert() {
        let original = [0b10110101u8, 0b11001100];
        let mut data = [0u8; 2];

        let value = extract_bits(&original, 2, 10);
        insert_bits(&mut data, 2, 10, value);

        assert_eq!(extract_bits(&data, 2, 10), extract_bits(&original, 2, 10));
    }

    #[test]
    fn test_set_clear_toggle_bit() {
        let mut data = [0u8; 2];

        set_bit(&mut data, 3);
        assert_eq!(data[0], 0b00001000);

        clear_bit(&mut data, 3);
        assert_eq!(data[0], 0);

        toggle_bit(&mut data, 3);
        assert_eq!(data[0], 0b00001000);
    }
}
