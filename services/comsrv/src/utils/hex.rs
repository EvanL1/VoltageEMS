//! Hexadecimal Utilities
//!
//! This module provides utilities for hexadecimal encoding, decoding, and formatting.
//! These functions are commonly used in protocol implementations for data visualization
//! and debugging.
//!
//! # Features
//!
//! - Byte array to hex string conversion
//! - Hex string to byte array conversion
//! - Pretty formatting with separators
//! - Uppercase/lowercase options
//! - Safe parsing with error handling
//!
//! # Examples
//!
//! ```rust
//! use comsrv::utils::hex::{bytes_to_hex, hex_to_bytes, format_hex_pretty};
//!
//! let data = &[0x01, 0x02, 0x03, 0xFF];
//!
//! // Convert to hex string
//! let hex = bytes_to_hex(data);
//! assert_eq!(hex, "010203ff");
//!
//! // Convert back to bytes
//! let bytes = hex_to_bytes(&hex)?;
//! assert_eq!(bytes, vec![0x01, 0x02, 0x03, 0xFF]);
//!
//! // Pretty format with spaces
//! let pretty = format_hex_pretty(data, " ");
//! assert_eq!(pretty, "01 02 03 ff");
//! ```

use crate::utils::error::{ComSrvError, Result};

/// Convert byte array to lowercase hexadecimal string
///
/// # Arguments
///
/// * `data` - Byte array to convert
///
/// # Returns
///
/// Lowercase hexadecimal string without separators
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::bytes_to_hex;
///
/// let data = &[0x01, 0x02, 0xFF];
/// let hex = bytes_to_hex(data);
/// assert_eq!(hex, "0102ff");
/// ```
pub fn bytes_to_hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

/// Convert byte array to uppercase hexadecimal string
///
/// # Arguments
///
/// * `data` - Byte array to convert
///
/// # Returns
///
/// Uppercase hexadecimal string without separators
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::bytes_to_hex_upper;
///
/// let data = &[0x01, 0x02, 0xFF];
/// let hex = bytes_to_hex_upper(data);
/// assert_eq!(hex, "0102FF");
/// ```
pub fn bytes_to_hex_upper(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join("")
}

/// Convert hexadecimal string to byte array
///
/// # Arguments
///
/// * `hex` - Hexadecimal string (with or without separators)
///
/// # Returns
///
/// Result containing byte vector or parsing error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::hex_to_bytes;
///
/// let hex = "0102ff";
/// let bytes = hex_to_bytes(hex)?;
/// assert_eq!(bytes, vec![0x01, 0x02, 0xFF]);
///
/// // Also works with separators
/// let hex_with_spaces = "01 02 ff";
/// let bytes = hex_to_bytes(hex_with_spaces)?;
/// assert_eq!(bytes, vec![0x01, 0x02, 0xFF]);
/// ```
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>> {
    // Remove common separators
    let cleaned = hex
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>();

    if cleaned.len() % 2 != 0 {
        return Err(ComSrvError::ParsingError(
            "Hex string must have even length".to_string(),
        ));
    }

    cleaned
        .chars()
        .collect::<Vec<char>>()
        .chunks(2)
        .map(|chunk| {
            let hex_byte = chunk.iter().collect::<String>();
            u8::from_str_radix(&hex_byte, 16).map_err(|e| {
                ComSrvError::ParsingError(format!("Invalid hex byte '{}': {}", hex_byte, e))
            })
        })
        .collect()
}

/// Format byte array as hex string with custom separator
///
/// # Arguments
///
/// * `data` - Byte array to format
/// * `separator` - String to use between hex bytes
///
/// # Returns
///
/// Formatted hexadecimal string with separators
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::format_hex_pretty;
///
/// let data = &[0x01, 0x02, 0xFF];
/// let pretty = format_hex_pretty(data, " ");
/// assert_eq!(pretty, "01 02 ff");
///
/// let colon_separated = format_hex_pretty(data, ":");
/// assert_eq!(colon_separated, "01:02:ff");
/// ```
pub fn format_hex_pretty(data: &[u8], separator: &str) -> String {
    data.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(separator)
}

/// Format byte array as uppercase hex string with custom separator
///
/// # Arguments
///
/// * `data` - Byte array to format
/// * `separator` - String to use between hex bytes
///
/// # Returns
///
/// Formatted uppercase hexadecimal string with separators
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::format_hex_pretty_upper;
///
/// let data = &[0x01, 0x02, 0xFF];
/// let pretty = format_hex_pretty_upper(data, " ");
/// assert_eq!(pretty, "01 02 FF");
/// ```
pub fn format_hex_pretty_upper(data: &[u8], separator: &str) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(separator)
}

/// Format byte array with standard space separation (lowercase)
///
/// Convenience function for the most common hex formatting use case.
///
/// # Arguments
///
/// * `data` - Byte array to format
///
/// # Returns
///
/// Space-separated lowercase hexadecimal string
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::format_hex_spaced;
///
/// let data = &[0x01, 0x02, 0xFF];
/// let hex = format_hex_spaced(data);
/// assert_eq!(hex, "01 02 ff");
/// ```
pub fn format_hex_spaced(data: &[u8]) -> String {
    format_hex_pretty(data, " ")
}

/// Format byte array with standard space separation (uppercase)
///
/// Convenience function for uppercase hex formatting.
///
/// # Arguments
///
/// * `data` - Byte array to format
///
/// # Returns
///
/// Space-separated uppercase hexadecimal string
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::format_hex_spaced_upper;
///
/// let data = &[0x01, 0x02, 0xFF];
/// let hex = format_hex_spaced_upper(data);
/// assert_eq!(hex, "01 02 FF");
/// ```
pub fn format_hex_spaced_upper(data: &[u8]) -> String {
    format_hex_pretty_upper(data, " ")
}

/// Validate if a string is valid hexadecimal
///
/// # Arguments
///
/// * `hex` - String to validate
///
/// # Returns
///
/// True if the string contains only valid hex characters and separators
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::is_valid_hex;
///
/// assert!(is_valid_hex("01ff"));
/// assert!(is_valid_hex("01 FF"));
/// assert!(is_valid_hex("01:ff:AA"));
/// assert!(!is_valid_hex("01gg"));
/// assert!(!is_valid_hex("xyz"));
/// ```
pub fn is_valid_hex(hex: &str) -> bool {
    let cleaned = hex
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>();
    
    !cleaned.is_empty() && cleaned.len() % 2 == 0 && hex.chars().all(|c| {
        c.is_ascii_hexdigit() || c.is_ascii_whitespace() || c == ':' || c == '-' || c == '_'
    })
}

/// Convert single byte to hex string
///
/// # Arguments
///
/// * `byte` - Byte to convert
///
/// # Returns
///
/// Two-character lowercase hex string
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::byte_to_hex;
///
/// assert_eq!(byte_to_hex(0x01), "01");
/// assert_eq!(byte_to_hex(0xFF), "ff");
/// ```
pub fn byte_to_hex(byte: u8) -> String {
    format!("{:02x}", byte)
}

/// Convert single byte to uppercase hex string
///
/// # Arguments
///
/// * `byte` - Byte to convert
///
/// # Returns
///
/// Two-character uppercase hex string
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::byte_to_hex_upper;
///
/// assert_eq!(byte_to_hex_upper(0x01), "01");
/// assert_eq!(byte_to_hex_upper(0xFF), "FF");
/// ```
pub fn byte_to_hex_upper(byte: u8) -> String {
    format!("{:02X}", byte)
}

/// Parse single hex byte from string
///
/// # Arguments
///
/// * `hex` - Two-character hex string
///
/// # Returns
///
/// Result containing parsed byte or error
///
/// # Example
///
/// ```rust
/// use comsrv::utils::hex::hex_byte_to_u8;
///
/// assert_eq!(hex_byte_to_u8("01")?, 0x01);
/// assert_eq!(hex_byte_to_u8("FF")?, 0xFF);
/// assert_eq!(hex_byte_to_u8("ff")?, 0xFF);
/// ```
pub fn hex_byte_to_u8(hex: &str) -> Result<u8> {
    if hex.len() != 2 {
        return Err(ComSrvError::ParsingError(
            format!("Hex byte must be exactly 2 characters, got: '{}'", hex)
        ));
    }

    u8::from_str_radix(hex, 16).map_err(|e| {
        ComSrvError::ParsingError(format!("Invalid hex byte '{}': {}", hex, e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_hex() {
        let data = &[0x00, 0x01, 0x02, 0xFF];
        assert_eq!(bytes_to_hex(data), "000102ff");
        assert_eq!(bytes_to_hex_upper(data), "000102FF");
    }

    #[test]
    fn test_hex_to_bytes() {
        // Test basic conversion
        let hex = "000102ff";
        let bytes = hex_to_bytes(hex).unwrap();
        assert_eq!(bytes, vec![0x00, 0x01, 0x02, 0xFF]);

        // Test with separators
        let hex_spaced = "00 01 02 ff";
        let bytes = hex_to_bytes(hex_spaced).unwrap();
        assert_eq!(bytes, vec![0x00, 0x01, 0x02, 0xFF]);

        let hex_colon = "00:01:02:ff";
        let bytes = hex_to_bytes(hex_colon).unwrap();
        assert_eq!(bytes, vec![0x00, 0x01, 0x02, 0xFF]);

        // Test uppercase
        let hex_upper = "000102FF";
        let bytes = hex_to_bytes(hex_upper).unwrap();
        assert_eq!(bytes, vec![0x00, 0x01, 0x02, 0xFF]);
    }

    #[test]
    fn test_hex_to_bytes_errors() {
        // Test odd length
        let result = hex_to_bytes("0");
        assert!(result.is_err());

        // Test invalid characters
        let result = hex_to_bytes("0g");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_hex_pretty() {
        let data = &[0x01, 0x02, 0xFF];
        assert_eq!(format_hex_pretty(data, " "), "01 02 ff");
        assert_eq!(format_hex_pretty(data, ":"), "01:02:ff");
        assert_eq!(format_hex_pretty(data, "-"), "01-02-ff");
        assert_eq!(format_hex_pretty_upper(data, " "), "01 02 FF");
    }

    #[test]
    fn test_format_hex_spaced() {
        let data = &[0x01, 0x02, 0xFF];
        assert_eq!(format_hex_spaced(data), "01 02 ff");
        assert_eq!(format_hex_spaced_upper(data), "01 02 FF");
    }

    #[test]
    fn test_is_valid_hex() {
        assert!(is_valid_hex("01ff"));
        assert!(is_valid_hex("01 FF"));
        assert!(is_valid_hex("01:ff:AA"));
        assert!(is_valid_hex("01-ff-AA"));
        assert!(is_valid_hex("01_ff_AA"));
        
        assert!(!is_valid_hex("01gg"));
        assert!(!is_valid_hex("xyz"));
        assert!(!is_valid_hex("1")); // Odd length
        assert!(!is_valid_hex(""));
    }

    #[test]
    fn test_byte_conversions() {
        assert_eq!(byte_to_hex(0x01), "01");
        assert_eq!(byte_to_hex(0xFF), "ff");
        assert_eq!(byte_to_hex_upper(0x01), "01");
        assert_eq!(byte_to_hex_upper(0xFF), "FF");

        assert_eq!(hex_byte_to_u8("01").unwrap(), 0x01);
        assert_eq!(hex_byte_to_u8("FF").unwrap(), 0xFF);
        assert_eq!(hex_byte_to_u8("ff").unwrap(), 0xFF);

        // Test errors
        assert!(hex_byte_to_u8("1").is_err());
        assert!(hex_byte_to_u8("123").is_err());
        assert!(hex_byte_to_u8("gg").is_err());
    }

    #[test]
    fn test_roundtrip() {
        let original = vec![0x00, 0x01, 0x10, 0xFF, 0xAB, 0xCD];
        let hex = bytes_to_hex(&original);
        let recovered = hex_to_bytes(&hex).unwrap();
        assert_eq!(original, recovered);

        // Test with pretty formatting
        let pretty = format_hex_spaced(&original);
        let recovered_pretty = hex_to_bytes(&pretty).unwrap();
        assert_eq!(original, recovered_pretty);
    }

    #[test]
    fn test_empty_data() {
        let empty: &[u8] = &[];
        assert_eq!(bytes_to_hex(empty), "");
        assert_eq!(format_hex_spaced(empty), "");
        assert_eq!(hex_to_bytes("").unwrap(), Vec::<u8>::new());
    }
} 