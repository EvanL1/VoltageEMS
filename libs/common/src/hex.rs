//! Hex encoding utility
//! Only provides uppercase hex encoding which is used by comsrv

use std::fmt::Write;

/// Encode bytes to uppercase hex string
/// Example: [0x12, 0x34, 0xAB] -> "1234AB"
pub fn encode_upper(data: &[u8]) -> String {
    let mut result = String::with_capacity(data.len() * 2);
    for byte in data {
        // Writing to String buffer is infallible - no need for expect
        let _ = write!(&mut result, "{:02X}", byte);
    }
    result
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_encode_upper_basic() {
        assert_eq!(encode_upper(&[0x12, 0x34, 0xAB]), "1234AB");
    }

    #[test]
    fn test_encode_upper_empty() {
        assert_eq!(encode_upper(&[]), "");
    }

    #[test]
    fn test_encode_upper_single_byte() {
        assert_eq!(encode_upper(&[0xFF]), "FF");
        assert_eq!(encode_upper(&[0x00]), "00");
        assert_eq!(encode_upper(&[0x0F]), "0F");
    }

    #[test]
    fn test_encode_upper_all_zeros() {
        assert_eq!(encode_upper(&[0x00, 0x00, 0x00]), "000000");
    }

    #[test]
    fn test_encode_upper_all_ones() {
        assert_eq!(encode_upper(&[0xFF, 0xFF, 0xFF]), "FFFFFF");
    }

    #[test]
    fn test_encode_upper_mixed() {
        assert_eq!(
            encode_upper(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF]),
            "0123456789ABCDEF"
        );
    }
}
