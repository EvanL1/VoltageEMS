//! Hex formatting utilities

/// Format bytes as hex string with spaces between each byte
/// Example: [0x12, 0x34, 0xAB] -> "12 34 AB"
#[inline]
pub fn format_hex_pretty(data: &[u8]) -> String {
    let hex_str = hex::encode_upper(data);
    // Convert "1234AB" to "12 34 AB"
    hex_str
        .chars()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format bytes as compact hex string
/// Example: [0x12, 0x34, 0xAB] -> "1234AB"
#[inline]
pub fn format_hex(data: &[u8]) -> String {
    hex::encode_upper(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hex_pretty() {
        assert_eq!(format_hex_pretty(&[0x00]), "00");
        assert_eq!(format_hex_pretty(&[0x12, 0x34]), "12 34");
        assert_eq!(format_hex_pretty(&[0xAB, 0xCD, 0xEF]), "AB CD EF");
        assert_eq!(format_hex_pretty(&[]), "");
    }

    #[test]
    fn test_format_hex() {
        assert_eq!(format_hex(&[0x00]), "00");
        assert_eq!(format_hex(&[0x12, 0x34]), "1234");
        assert_eq!(format_hex(&[0xAB, 0xCD, 0xEF]), "ABCDEF");
        assert_eq!(format_hex(&[]), "");
    }
}