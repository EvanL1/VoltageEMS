//! Modbus protocol constants based on official specification
//!
//! These constants are derived from the official Modbus specification:
//! - Maximum PDU size: 253 bytes (inherited from RS485 ADU limit of 256 bytes)
//! - Register/coil limits are calculated to fit within the PDU size constraint

// ============================================================================
// Frame Size Constants
// ============================================================================

/// Modbus MBAP header length for TCP
/// Format: Transaction ID(2) + Protocol ID(2) + Length(2) + Unit ID(1) = 7 bytes
/// Note: Length field itself is not counted in MBAP_HEADER_LEN for frame parsing
pub const MBAP_HEADER_LEN: usize = 6;

/// Maximum PDU (Protocol Data Unit) size per Modbus specification
/// This is the fundamental limit inherited from RS485 implementation:
/// RS485 ADU (256 bytes) - Slave Address (1 byte) - CRC (2 bytes) = 253 bytes
pub const MAX_PDU_SIZE: usize = 253;

/// Maximum MBAP length field value (Unit ID + PDU)
/// Used for validating the Length field in MBAP header
/// = 1 (Unit ID) + 253 (Max PDU) = 254 bytes
pub const MAX_MBAP_LENGTH: usize = 1 + MAX_PDU_SIZE;

/// Response buffer size for receiving Modbus frames
///
/// Calculation:
/// - MBAP Header: 6 bytes (MBAP_HEADER_LEN)
/// - Max MBAP Length (Unit ID + PDU): 254 bytes (MAX_MBAP_LENGTH)
/// - Theoretical max frame: 6 + 254 = 260 bytes
/// - Buffer size: 512 bytes (provides safety margin and prevents "PDU too short" errors)
///
/// Note: Previous 256-byte buffer was insufficient for maximum-sized responses,
/// causing "PDU too short" errors when batch reading large numbers of registers.
pub const MODBUS_RESPONSE_BUFFER_SIZE: usize = 512;

// ============================================================================
// Register Operation Limits
// ============================================================================

/// Maximum number of registers for FC03/FC04 (Read Holding/Input Registers)
///
/// Calculation for response PDU:
/// - Function Code: 1 byte
/// - Byte Count: 1 byte
/// - Register Data: N × 2 bytes
/// - Total: 1 + 1 + (N × 2) ≤ 253
/// - Therefore: N ≤ (253 - 2) / 2 = 125.5 → 125 registers
pub const MODBUS_MAX_READ_REGISTERS: usize = 125;

/// Maximum number of registers for FC16 (Write Multiple Registers)
///
/// Calculation for request PDU:
/// - Function Code: 1 byte
/// - Starting Address: 2 bytes
/// - Quantity of Registers: 2 bytes
/// - Byte Count: 1 byte
/// - Register Values: N × 2 bytes
/// - Total: 1 + 2 + 2 + 1 + (N × 2) ≤ 253
/// - Therefore: N ≤ (253 - 6) / 2 = 123.5 → 123 registers
pub const MODBUS_MAX_WRITE_REGISTERS: usize = 123;

// ============================================================================
// Coil Operation Limits
// ============================================================================

/// Maximum number of coils for FC01/FC02 (Read Coils/Discrete Inputs)
///
/// Calculation for response PDU:
/// - Function Code: 1 byte
/// - Byte Count: 1 byte
/// - Coil Data: ceil(N / 8) bytes
/// - Total: 1 + 1 + ceil(N / 8) ≤ 253
/// - Therefore: ceil(N / 8) ≤ 251, N ≤ 251 × 8 = 2008
/// - Spec defines: N ≤ 2000 (rounded for practical use)
pub const MODBUS_MAX_READ_COILS: usize = 2000;

/// Maximum number of coils for FC15 (Write Multiple Coils)
///
/// Calculation for request PDU:
/// - Function Code: 1 byte
/// - Starting Address: 2 bytes
/// - Quantity of Outputs: 2 bytes
/// - Byte Count: 1 byte
/// - Coil Values: ceil(N / 8) bytes
/// - Total: 1 + 2 + 2 + 1 + ceil(N / 8) ≤ 253
/// - Therefore: ceil(N / 8) ≤ 247, N ≤ 247 × 8 = 1976
/// - Spec defines: N ≤ 1968 (0x7B0, conservative practical limit)
pub const MODBUS_MAX_WRITE_COILS: usize = 1968;

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate total Modbus TCP frame size (MBAP header + PDU)
///
/// # Parameters
/// - `pdu_len`: Length of the PDU in bytes
///
/// # Returns
/// Total frame size in bytes
///
/// # Example
/// ```
/// use comsrv::plugins::protocols::modbus::constants::mbap_frame_size;
/// assert_eq!(mbap_frame_size(5), 11); // 6-byte MBAP + 5-byte PDU
/// ```
#[inline]
pub const fn mbap_frame_size(pdu_len: usize) -> usize {
    MBAP_HEADER_LEN + pdu_len
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_frame_size_constants() {
        // Verify basic frame size relationships
        assert_eq!(MBAP_HEADER_LEN, 6);
        assert_eq!(MAX_PDU_SIZE, 253);
        assert_eq!(MAX_MBAP_LENGTH, 254);
    }

    #[test]
    fn test_register_limits() {
        // Verify read register limit calculation
        let read_pdu_size = 1 + 1 + (MODBUS_MAX_READ_REGISTERS * 2);
        assert!(read_pdu_size <= MAX_PDU_SIZE);
        assert_eq!(MODBUS_MAX_READ_REGISTERS, 125);

        // Verify write register limit calculation
        let write_pdu_size = 1 + 2 + 2 + 1 + (MODBUS_MAX_WRITE_REGISTERS * 2);
        assert!(write_pdu_size <= MAX_PDU_SIZE);
        assert_eq!(MODBUS_MAX_WRITE_REGISTERS, 123);
    }

    #[test]
    fn test_coil_limits() {
        // Verify read coil limit
        let read_coil_bytes = MODBUS_MAX_READ_COILS.div_ceil(8);
        let read_coil_pdu = 1 + 1 + read_coil_bytes;
        assert!(read_coil_pdu <= MAX_PDU_SIZE);
        assert_eq!(MODBUS_MAX_READ_COILS, 2000);

        // Verify write coil limit
        let write_coil_bytes = MODBUS_MAX_WRITE_COILS.div_ceil(8);
        let write_coil_pdu = 1 + 2 + 2 + 1 + write_coil_bytes;
        assert!(write_coil_pdu <= MAX_PDU_SIZE);
        assert_eq!(MODBUS_MAX_WRITE_COILS, 1968);
    }

    #[test]
    fn test_mbap_frame_size_helper() {
        assert_eq!(mbap_frame_size(0), 6);
        assert_eq!(mbap_frame_size(5), 11);
        assert_eq!(mbap_frame_size(MAX_PDU_SIZE), 6 + 253);
    }
}
