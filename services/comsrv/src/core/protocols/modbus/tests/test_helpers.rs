//! Test helper functions and utilities
//!
//! Common utilities for Modbus protocol testing.

/// Create a test PDU with function code and data
pub fn create_test_pdu(function_code: u8, data: &[u8]) -> Vec<u8> {
    let mut pdu = vec![function_code];
    pdu.extend_from_slice(data);
    pdu
}

/// Create a Modbus TCP frame (MBAP header + PDU)
pub fn create_modbus_tcp_frame(transaction_id: u16, unit_id: u8, pdu: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();

    // Transaction ID (2 bytes)
    frame.extend_from_slice(&transaction_id.to_be_bytes());

    // Protocol ID (2 bytes) - always 0 for Modbus
    frame.extend_from_slice(&0u16.to_be_bytes());

    // Length (2 bytes) - PDU length + 1 for unit ID
    let length = (pdu.len() + 1) as u16;
    frame.extend_from_slice(&length.to_be_bytes());

    // Unit ID (1 byte)
    frame.push(unit_id);

    // PDU
    frame.extend_from_slice(pdu);

    frame
}

/// Create a Modbus RTU frame (PDU + CRC)
pub fn create_modbus_rtu_frame(slave_id: u8, pdu: &[u8]) -> Vec<u8> {
    let mut frame = vec![slave_id];
    frame.extend_from_slice(pdu);

    // Calculate and append CRC
    let crc = calculate_crc16(&frame);
    frame.extend_from_slice(&crc.to_le_bytes());

    frame
}

/// Calculate CRC16 for Modbus RTU
pub fn calculate_crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;

    for byte in data {
        crc ^= *byte as u16;
        for _ in 0..8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }

    crc
}

/// Compare two byte arrays and provide detailed diff
pub fn assert_bytes_eq(actual: &[u8], expected: &[u8]) {
    if actual != expected {
        panic!(
            "Byte arrays differ:\nExpected: {}\nActual:   {}\nDiff:     {}",
            format_bytes(expected),
            format_bytes(actual),
            format_diff(expected, actual)
        );
    }
}

/// Format bytes as hex string
pub fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format diff between two byte arrays
fn format_diff(expected: &[u8], actual: &[u8]) -> String {
    let max_len = expected.len().max(actual.len());
    let mut diff = String::new();

    for i in 0..max_len {
        if i >= expected.len() {
            diff.push_str("++ ");
        } else if i >= actual.len() {
            diff.push_str("-- ");
        } else if expected[i] != actual[i] {
            diff.push_str("!! ");
        } else {
            diff.push_str("   ");
        }
    }

    diff
}

/// Test data generator for various data types
pub struct TestDataGenerator {
    seed: u64,
}

impl TestDataGenerator {
    pub fn new() -> Self {
        Self { seed: 0x1234 }
    }

    /// Generate random u16 values
    pub fn generate_u16_values(&mut self, count: usize) -> Vec<u16> {
        (0..count).map(|_| self.next_u16()).collect()
    }

    /// Generate random bool values
    pub fn generate_bool_values(&mut self, count: usize) -> Vec<bool> {
        (0..count).map(|_| self.next_bool()).collect()
    }

    /// Simple pseudo-random number generator
    fn next_u16(&mut self) -> u16 {
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.seed >> 16) & 0xFFFF) as u16
    }

    fn next_bool(&mut self) -> bool {
        self.next_u16() & 1 == 1
    }
}

/// Test configuration builder
pub struct TestConfigBuilder {
    slave_id: u8,
    timeout_ms: u64,
    max_retries: u32,
}

impl TestConfigBuilder {
    pub fn new() -> Self {
        Self {
            slave_id: 1,
            timeout_ms: 1000,
            max_retries: 3,
        }
    }

    pub fn with_slave_id(mut self, id: u8) -> Self {
        self.slave_id = id;
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn with_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn build_tcp_config(self) -> crate::core::protocols::modbus::common::ModbusConfig {
        crate::core::protocols::modbus::common::ModbusConfig {
            protocol_type: "modbus_tcp".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(502),
            device_path: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout_ms: Some(self.timeout_ms),
            points: vec![],
        }
    }
}

/// Performance measurement helper
pub struct PerfMeasure {
    start: std::time::Instant,
    name: String,
}

impl PerfMeasure {
    pub fn start(name: &str) -> Self {
        Self {
            start: std::time::Instant::now(),
            name: name.to_string(),
        }
    }

    pub fn stop(self) -> std::time::Duration {
        let duration = self.start.elapsed();
        println!("{}: {:?}", self.name, duration);
        duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16_calculation() {
        // Test vector from Modbus specification
        let data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let crc = calculate_crc16(&data);
        assert_eq!(crc, 0xCDC5); // Correct CRC for this data
    }

    #[test]
    fn test_modbus_tcp_frame_creation() {
        let pdu = vec![0x03, 0x00, 0x00, 0x00, 0x0A];
        let frame = create_modbus_tcp_frame(0x1234, 0x01, &pdu);

        assert_eq!(frame[0..2], [0x12, 0x34]); // Transaction ID
        assert_eq!(frame[2..4], [0x00, 0x00]); // Protocol ID
        assert_eq!(frame[4..6], [0x00, 0x06]); // Length (5 + 1)
        assert_eq!(frame[6], 0x01); // Unit ID
        assert_eq!(&frame[7..], &pdu); // PDU
    }

    #[test]
    fn test_data_generator() {
        let mut gen = TestDataGenerator::new();
        let values = gen.generate_u16_values(10);
        assert_eq!(values.len(), 10);

        // Values should be deterministic with same seed
        let mut gen2 = TestDataGenerator::new();
        let values2 = gen2.generate_u16_values(10);
        assert_eq!(values, values2);
    }
}
