//! Frame processing tests
//!
//! Tests for Modbus TCP (MBAP) and RTU frame handling.

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    #[test]
    fn test_crc16_calculation() {
        // Test vector from Modbus specification
        let data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let crc = calculate_crc16(&data);
        assert_eq!(crc, 0xCDC5); // Correct CRC for this data
    }

    #[test]
    fn test_modbus_tcp_frame_helper() {
        let pdu = vec![0x03, 0x00, 0x00, 0x00, 0x0A];
        let frame = create_modbus_tcp_frame(0x1234, 0x01, &pdu);

        assert_eq!(frame[0..2], [0x12, 0x34]); // Transaction ID
        assert_eq!(frame[2..4], [0x00, 0x00]); // Protocol ID
        assert_eq!(frame[4..6], [0x00, 0x06]); // Length (5 + 1)
        assert_eq!(frame[6], 0x01); // Unit ID
        assert_eq!(&frame[7..], &pdu); // PDU
    }

    // Additional frame tests will be added as frame processor evolves
}
