//! Unified byte order representation for cross-protocol data conversion
//!
//! Provides a type-safe enum for handling different byte/word ordering patterns
//! commonly used in industrial protocols (Modbus, CAN, IEC104, etc.).

/// Unified byte/word order representation for 16/32/64-bit values
///
/// # Terminology
/// - **Byte order**: Order of bytes within multi-byte values (endianness)
/// - **Word order**: Order of 16-bit words when combining to form 32/64-bit values
///
/// # Naming Convention
/// Uses ABCD notation where:
/// - A = Most significant byte (MSB)
/// - B = Second byte
/// - C = Third byte
/// - D = Least significant byte (LSB)
///
/// For 32-bit value `0x12345678`:
/// - `BigEndian (ABCD)`: [0x12, 0x34, 0x56, 0x78]
/// - `LittleEndian (DCBA)`: [0x78, 0x56, 0x34, 0x12]
/// - `BigEndianSwap (CDAB)`: [0x56, 0x78, 0x12, 0x34] (Modbus common)
/// - `LittleEndianSwap (BADC)`: [0x34, 0x12, 0x78, 0x56]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ByteOrder {
    /// Big-endian: ABCD (most significant byte first)
    ///
    /// Network byte order, used in most protocols.
    /// Example: 0x12345678 → [0x12, 0x34, 0x56, 0x78]
    BigEndian,

    /// Little-endian: DCBA (least significant byte first)
    ///
    /// Intel x86 native byte order.
    /// Example: 0x12345678 → [0x78, 0x56, 0x34, 0x12]
    LittleEndian,

    /// Big-endian with swapped words: CDAB
    ///
    /// Common in Modbus and some PLCs. Words are big-endian but swapped.
    /// Example: 0x12345678 → [0x56, 0x78, 0x12, 0x34]
    BigEndianSwap,

    /// Little-endian with swapped words: BADC
    ///
    /// Rare, but exists in some devices.
    /// Example: 0x12345678 → [0x34, 0x12, 0x78, 0x56]
    LittleEndianSwap,

    /// 16-bit big-endian: AB
    ///
    /// For 16-bit values only.
    /// Example: 0x1234 → [0x12, 0x34]
    BigEndian16,

    /// 16-bit little-endian: BA
    ///
    /// For 16-bit values only.
    /// Example: 0x1234 → [0x34, 0x12]
    LittleEndian16,
}

impl ByteOrder {
    /// Convert from legacy string formats
    ///
    /// Supports various common string representations:
    /// - "ABCD", "AB-CD" → BigEndian
    /// - "DCBA", "DC-BA" → LittleEndian
    /// - "CDAB", "CD-AB" → BigEndianSwap
    /// - "BADC", "BA-DC" → LittleEndianSwap
    /// - "BE", "BIG_ENDIAN" → BigEndian
    /// - "LE", "LITTLE_ENDIAN" → LittleEndian
    /// - "AB" → BigEndian16
    /// - "BA" → LittleEndian16
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        let normalized = s.to_uppercase().replace('-', "");
        match normalized.as_str() {
            // 32/64-bit patterns
            "ABCD" | "BE" | "BIG_ENDIAN" | "BIGENDIAN" | "ABCDEFGH" => Some(Self::BigEndian),
            "DCBA" | "LE" | "LITTLE_ENDIAN" | "LITTLEENDIAN" | "HGFEDCBA" => {
                Some(Self::LittleEndian)
            },
            "CDAB" | "BIG_ENDIAN_SWAP" | "BIGENDIANSWAP" => Some(Self::BigEndianSwap),
            "BADC" | "LITTLE_ENDIAN_SWAP" | "LITTLEENDIANSWAP" => Some(Self::LittleEndianSwap),

            // 16-bit patterns
            "AB" => Some(Self::BigEndian16),
            "BA" => Some(Self::LittleEndian16),

            _ => None,
        }
    }

    /// Get descriptive name
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BigEndian => "ABCD (Big-Endian)",
            Self::LittleEndian => "DCBA (Little-Endian)",
            Self::BigEndianSwap => "CDAB (Big-Endian Swap)",
            Self::LittleEndianSwap => "BADC (Little-Endian Swap)",
            Self::BigEndian16 => "AB (Big-Endian 16)",
            Self::LittleEndian16 => "BA (Little-Endian 16)",
        }
    }

    /// Check if this is a 16-bit only byte order
    pub fn is_16bit_only(&self) -> bool {
        matches!(self, Self::BigEndian16 | Self::LittleEndian16)
    }

    /// Check if this is a big-endian variant
    pub fn is_big_endian(&self) -> bool {
        matches!(
            self,
            Self::BigEndian | Self::BigEndianSwap | Self::BigEndian16
        )
    }

    /// Check if this is a little-endian variant
    pub fn is_little_endian(&self) -> bool {
        matches!(
            self,
            Self::LittleEndian | Self::LittleEndianSwap | Self::LittleEndian16
        )
    }

    /// Check if words are swapped (for 32/64-bit values)
    pub fn has_word_swap(&self) -> bool {
        matches!(self, Self::BigEndianSwap | Self::LittleEndianSwap)
    }
}

impl std::fmt::Display for ByteOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for ByteOrder {
    /// Default to big-endian (network byte order)
    fn default() -> Self {
        Self::BigEndian
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str_valid() {
        assert_eq!(ByteOrder::from_str("ABCD"), Some(ByteOrder::BigEndian));
        assert_eq!(ByteOrder::from_str("AB-CD"), Some(ByteOrder::BigEndian));
        assert_eq!(ByteOrder::from_str("be"), Some(ByteOrder::BigEndian));

        assert_eq!(ByteOrder::from_str("DCBA"), Some(ByteOrder::LittleEndian));
        assert_eq!(ByteOrder::from_str("LE"), Some(ByteOrder::LittleEndian));

        assert_eq!(ByteOrder::from_str("CDAB"), Some(ByteOrder::BigEndianSwap));
        assert_eq!(
            ByteOrder::from_str("BADC"),
            Some(ByteOrder::LittleEndianSwap)
        );

        assert_eq!(ByteOrder::from_str("AB"), Some(ByteOrder::BigEndian16));
        assert_eq!(ByteOrder::from_str("BA"), Some(ByteOrder::LittleEndian16));
    }

    #[test]
    fn test_from_str_invalid() {
        assert_eq!(ByteOrder::from_str("invalid"), None);
        assert_eq!(ByteOrder::from_str(""), None);
    }

    #[test]
    fn test_properties() {
        assert!(ByteOrder::BigEndian16.is_16bit_only());
        assert!(!ByteOrder::BigEndian.is_16bit_only());

        assert!(ByteOrder::BigEndian.is_big_endian());
        assert!(!ByteOrder::LittleEndian.is_big_endian());

        assert!(ByteOrder::BigEndianSwap.has_word_swap());
        assert!(!ByteOrder::BigEndian.has_word_swap());
    }

    #[test]
    fn test_default() {
        assert_eq!(ByteOrder::default(), ByteOrder::BigEndian);
    }
}
