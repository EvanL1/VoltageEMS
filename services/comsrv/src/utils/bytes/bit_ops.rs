//! Bit-level operations for industrial protocol data
//!
//! Provides functions for extracting and inserting bits from/to byte arrays.
//! Common use cases:
//! - CAN signal extraction from message data
//! - Modbus coil/discrete input processing
//! - Status register bit manipulation

/// Extract single bit from u16 value
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::extract_bit_u16;
///
/// let value = 0b1010_1100u16;
/// assert_eq!(extract_bit_u16(value, 0), false); // Bit 0
/// assert_eq!(extract_bit_u16(value, 2), true);  // Bit 2
/// assert_eq!(extract_bit_u16(value, 3), true);  // Bit 3
/// ```
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
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::extract_bits;
///
/// // Byte array: [0b10110101, 0b11001100]
/// let data = [0b10110101, 0b11001100];
///
/// // Extract 4 bits starting at bit 2
/// let result = extract_bits(&data, 2, 4);
/// assert_eq!(result, 0b1101);
///
/// // Extract 8 bits starting at bit 4 (spans two bytes)
/// let result = extract_bits(&data, 4, 8);
/// assert_eq!(result, 0b11001011);
/// ```
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
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::extract_bits_signed;
///
/// let data = [0xFF, 0xFF]; // All bits set
///
/// // Extract 4-bit signed value
/// let result = extract_bits_signed(&data, 0, 4);
/// assert_eq!(result, -1); // Sign extended
/// ```
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
///
/// # Examples
/// ```
/// use comsrv::utils::bytes::insert_bits;
///
/// let mut data = [0u8; 2];
///
/// // Insert 4-bit value 0b1101 at bit 2
/// insert_bits(&mut data, 2, 4, 0b1101);
/// assert_eq!(data[0], 0b00110100);
/// ```
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
        assert!(!extract_bit_u16(value, 0)); // Bit 0 = 0
        assert!(!extract_bit_u16(value, 1)); // Bit 1 = 0
        assert!(extract_bit_u16(value, 2)); // Bit 2 = 1
        assert!(extract_bit_u16(value, 3)); // Bit 3 = 1
        assert!(!extract_bit_u16(value, 4)); // Bit 4 = 0
        assert!(extract_bit_u16(value, 5)); // Bit 5 = 1
        assert!(!extract_bit_u16(value, 6)); // Bit 6 = 0
        assert!(extract_bit_u16(value, 7)); // Bit 7 = 1
    }

    #[test]
    fn test_extract_bits_basic() {
        let data = [0b10110101, 0b11001100];

        // Extract 4 bits starting at bit 2
        let result = extract_bits(&data, 2, 4);
        assert_eq!(result, 0b1101);

        // Extract 8 bits starting at bit 4 (spans two bytes)
        let result = extract_bits(&data, 4, 8);
        assert_eq!(result, 0b11001011);
    }

    #[test]
    fn test_extract_bits_single_byte() {
        let data = [0b10110101];

        // Extract all 8 bits
        let result = extract_bits(&data, 0, 8);
        assert_eq!(result, 0b10110101);

        // Extract upper 4 bits
        let result = extract_bits(&data, 4, 4);
        assert_eq!(result, 0b1011);
    }

    #[test]
    fn test_extract_bits_signed_positive() {
        let data = [0b00000111];

        // Extract 4-bit signed value (positive: 0b0111 = 7)
        let result = extract_bits_signed(&data, 0, 4);
        assert_eq!(result, 7);
    }

    #[test]
    fn test_extract_bits_signed_negative() {
        let data = [0b11111111];

        // Extract 4-bit signed value (all 1s = -1)
        let result = extract_bits_signed(&data, 0, 4);
        assert_eq!(result, -1);

        // Extract 8-bit signed value
        let result = extract_bits_signed(&data, 0, 8);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_insert_bits_basic() {
        let mut data = [0u8; 2];

        // Insert 4-bit value 0b1101 at bit 2
        insert_bits(&mut data, 2, 4, 0b1101);
        assert_eq!(data[0], 0b00110100);

        // Insert 8-bit value spanning two bytes
        let mut data = [0u8; 2];
        insert_bits(&mut data, 4, 8, 0b11001011);
        assert_eq!(data[0], 0b10110000);
        assert_eq!(data[1], 0b00001100);
    }

    #[test]
    fn test_insert_bits_overwrites() {
        let mut data = [0xFFu8; 2];

        // Overwrite with zeros
        insert_bits(&mut data, 2, 4, 0b0000);
        assert_eq!(data[0], 0b11000011);
    }

    #[test]
    fn test_roundtrip_extract_insert() {
        let original = [0b10110101u8, 0b11001100];
        let mut data = [0u8; 2];

        // Extract and reinsert the same value
        let value = extract_bits(&original, 2, 10);
        insert_bits(&mut data, 2, 10, value);

        // Verify extracted bits match
        assert_eq!(extract_bits(&data, 2, 10), extract_bits(&original, 2, 10));
    }

    #[test]
    fn test_set_bit() {
        let mut data = [0u8; 2];

        set_bit(&mut data, 3);
        assert_eq!(data[0], 0b00001000);

        set_bit(&mut data, 10);
        assert_eq!(data[1], 0b00000100);
    }

    #[test]
    fn test_clear_bit() {
        let mut data = [0xFFu8; 2];

        clear_bit(&mut data, 3);
        assert_eq!(data[0], 0b11110111);

        clear_bit(&mut data, 10);
        assert_eq!(data[1], 0b11111011);
    }

    #[test]
    fn test_toggle_bit() {
        let mut data = [0b10101010u8];

        toggle_bit(&mut data, 0);
        assert_eq!(data[0], 0b10101011);

        toggle_bit(&mut data, 0);
        assert_eq!(data[0], 0b10101010);
    }

    #[test]
    fn test_extract_bits_boundary() {
        let data = [0xAB, 0xCD, 0xEF];

        // Extract exactly 8 bits at byte boundary
        assert_eq!(extract_bits(&data, 0, 8), 0xAB);
        assert_eq!(extract_bits(&data, 8, 8), 0xCD);

        // Extract spanning 12 bits (1.5 bytes)
        assert_eq!(extract_bits(&data, 4, 12), 0xCDA);
    }
}
