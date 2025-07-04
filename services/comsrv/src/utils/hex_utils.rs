//! Hex formatting utilities for logging
//!
//! This module provides utilities for formatting byte data as hex strings.

/// Format byte slice as hex string (e.g., "00 01 02 03 AB CD EF")
pub fn format_hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format byte slice as hex string with prefix (e.g., "0x00 0x01 0x02")
pub fn format_hex_with_prefix(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("0x{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format byte slice as compact hex string (e.g., "00010203ABCDEF")
pub fn format_hex_compact(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hex() {
        let data = vec![0x00, 0x01, 0xAB, 0xCD, 0xEF];
        assert_eq!(format_hex(&data), "00 01 AB CD EF");
    }

    #[test]
    fn test_format_hex_with_prefix() {
        let data = vec![0x00, 0x01, 0xAB];
        assert_eq!(format_hex_with_prefix(&data), "0x00 0x01 0xAB");
    }

    #[test]
    fn test_format_hex_compact() {
        let data = vec![0x00, 0x01, 0xAB, 0xCD];
        assert_eq!(format_hex_compact(&data), "0001ABCD");
    }

    #[test]
    fn test_empty_data() {
        let data = vec![];
        assert_eq!(format_hex(&data), "");
        assert_eq!(format_hex_with_prefix(&data), "");
        assert_eq!(format_hex_compact(&data), "");
    }
}