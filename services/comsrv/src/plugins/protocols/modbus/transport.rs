//! Modbus transport layer implementation
//!
//! Supports frame processing for both TCP and RTU transport modes

use super::connection::ConnectionParams;
use crate::core::config::types::ChannelConfig;
use crate::plugins::protocols::modbus::pdu::ModbusPdu;
use crate::utils::error::{ComSrvError, Result};
use std::collections::HashMap;
use tracing::{debug, error};

/// Modbus transport mode
#[derive(Debug, Clone, PartialEq)]
pub enum ModbusMode {
    /// TCP mode (using MBAP header)
    Tcp,
    /// RTU mode (using CRC check)
    Rtu,
}

/// Modbus TCP MBAP header
#[derive(Debug, Clone)]
pub struct MbapHeader {
    /// Transaction identifier
    pub transaction_id: u16,
    /// Protocol identifier (fixed to 0)
    pub protocol_id: u16,
    /// Length field
    pub length: u16,
    /// Unit identifier (slave ID)
    pub unit_id: u8,
}

/// Composite key for request tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RequestKey {
    transaction_id: u16,
    function_code: u8,
    slave_id: u8,
}

/// Request tracking information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RequestInfo {
    transaction_id: u16,
    function_code: u8,
    slave_id: u8,
    timestamp: std::time::Instant,
}

/// Modbus frame processor
#[derive(Debug)]
pub struct ModbusFrameProcessor {
    mode: ModbusMode,
    /// Store pending requests indexed by composite key (transaction_id, function_code, slave_id)
    pending_requests: HashMap<RequestKey, RequestInfo>,
    /// Channel-local transaction ID counter (for TCP mode)
    next_transaction_id: u16,
    /// Sequential request ID for RTU mode tracking
    next_rtu_request_id: u16,
    /// Maximum pending requests to prevent memory growth
    max_pending_requests: usize,
}

impl ModbusFrameProcessor {
    /// Create new frame processor
    pub fn new(mode: ModbusMode) -> Self {
        Self {
            mode,
            pending_requests: HashMap::new(),
            next_transaction_id: 1,
            next_rtu_request_id: 1,
            max_pending_requests: 1000,
        }
    }

    /// Get next transaction ID (TCP mode only)
    pub fn next_transaction_id(&mut self) -> u16 {
        // Use channel-local counter
        let id = self.next_transaction_id;

        // Increment for next call - wraps naturally from 0xFFFF to 0x0000
        self.next_transaction_id = self.next_transaction_id.wrapping_add(1);

        id
    }

    /// Get next RTU request ID
    fn next_rtu_request_id(&mut self) -> u16 {
        let id = self.next_rtu_request_id;
        self.next_rtu_request_id = self.next_rtu_request_id.wrapping_add(1);
        if self.next_rtu_request_id == 0 {
            self.next_rtu_request_id = 1;
        }
        id
    }

    /// Build complete Modbus frame
    pub fn build_frame(&mut self, unit_id: u8, pdu: &ModbusPdu) -> Vec<u8> {
        // Extract function code from PDU
        let function_code = pdu.function_code().unwrap_or(0);

        match self.mode {
            ModbusMode::Tcp => {
                let transaction_id = self.next_transaction_id();

                // Create composite key
                let key = RequestKey {
                    transaction_id,
                    function_code,
                    slave_id: unit_id,
                };

                // Store request info with composite key for validation
                self.pending_requests.insert(
                    key,
                    RequestInfo {
                        transaction_id,
                        function_code,
                        slave_id: unit_id,
                        timestamp: std::time::Instant::now(),
                    },
                );

                // Clean up old requests if we exceed the limit
                if self.pending_requests.len() > self.max_pending_requests {
                    self.cleanup_old_requests();
                }

                self.build_tcp_frame_with_id(unit_id, pdu, transaction_id)
            },
            ModbusMode::Rtu => {
                let request_id = self.next_rtu_request_id();

                // For RTU, we use request_id as transaction_id in the key
                let key = RequestKey {
                    transaction_id: request_id,
                    function_code,
                    slave_id: unit_id,
                };

                // Store request info for RTU validation
                self.pending_requests.insert(
                    key,
                    RequestInfo {
                        transaction_id: request_id,
                        function_code,
                        slave_id: unit_id,
                        timestamp: std::time::Instant::now(),
                    },
                );

                // Clean up old requests if we exceed the limit
                if self.pending_requests.len() > self.max_pending_requests {
                    self.cleanup_old_requests();
                }

                self.build_rtu_frame(unit_id, pdu)
            },
        }
    }

    /// Clean up old requests based on timestamp
    fn cleanup_old_requests(&mut self) {
        let now = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(30);

        self.pending_requests
            .retain(|_, info| now.duration_since(info.timestamp) < timeout);

        // If still too many, remove oldest entries
        if self.pending_requests.len() > self.max_pending_requests / 2 {
            let mut entries: Vec<_> = self
                .pending_requests
                .iter()
                .map(|(k, v)| (k.clone(), v.timestamp))
                .collect();
            entries.sort_by_key(|(_, timestamp)| *timestamp);

            let to_remove = entries.len() - self.max_pending_requests / 2;
            let keys_to_remove: Vec<_> = entries
                .iter()
                .take(to_remove)
                .map(|(k, _)| k.clone())
                .collect();

            for key in keys_to_remove {
                self.pending_requests.remove(&key);
            }
        }
    }

    /// Parse received frame
    pub fn parse_frame(&mut self, data: &[u8]) -> Result<(u8, ModbusPdu)> {
        match self.mode {
            ModbusMode::Tcp => self.parse_tcp_frame(data),
            ModbusMode::Rtu => self.parse_rtu_frame(data),
        }
    }

    /// Build TCP frame with specific transaction ID (MBAP + PDU)
    fn build_tcp_frame_with_id(
        &self,
        unit_id: u8,
        pdu: &ModbusPdu,
        transaction_id: u16,
    ) -> Vec<u8> {
        let length = (pdu.len() + 1) as u16; // PDU length + unit_id

        let mut frame = Vec::with_capacity(7 + pdu.len());

        // MBAP header
        frame.extend_from_slice(&transaction_id.to_be_bytes());
        frame.extend_from_slice(&0u16.to_be_bytes()); // protocol_id
        frame.extend_from_slice(&length.to_be_bytes());
        frame.push(unit_id);

        // PDU
        frame.extend_from_slice(pdu.as_slice());

        debug!(
            "Built TCP frame: transaction_id={:04X}, slave_id={}, function_code={:02X}, frame={:02X?}",
            transaction_id,
            unit_id,
            pdu.function_code().unwrap_or(0),
            frame
        );

        frame
    }

    /// Build RTU frame (`unit_id` + PDU + CRC)
    fn build_rtu_frame(&self, unit_id: u8, pdu: &ModbusPdu) -> Vec<u8> {
        let mut frame = Vec::with_capacity(1 + pdu.len() + 2);

        // Unit ID
        frame.push(unit_id);

        // PDU
        frame.extend_from_slice(pdu.as_slice());

        // CRC
        let crc = self.calculate_crc16(&frame);
        frame.extend_from_slice(&crc.to_le_bytes());

        frame
    }

    /// Parse TCP frame
    fn parse_tcp_frame(&mut self, data: &[u8]) -> Result<(u8, ModbusPdu)> {
        if data.len() < 8 {
            return Err(ComSrvError::ProtocolError(
                "TCP frame too short".to_string(),
            ));
        }

        // Parse MBAP header
        let transaction_id = u16::from_be_bytes([data[0], data[1]]);
        let protocol_id = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let unit_id = data[6];

        // Validate protocol ID
        if protocol_id != 0 {
            return Err(ComSrvError::ProtocolError(format!(
                "Invalid protocol ID: expected 0, got {}",
                protocol_id
            )));
        }

        // Validate length
        if data.len() != (6 + length as usize) {
            return Err(ComSrvError::ProtocolError(format!(
                "Invalid TCP frame length: expected {}, got {}",
                6 + length as usize,
                data.len()
            )));
        }

        // Extract PDU
        let pdu = ModbusPdu::from_slice(&data[7..])?;

        // Find the request by transaction ID only (transaction ID is unique per channel)
        let matching_request = self
            .pending_requests
            .iter()
            .find(|(k, _)| k.transaction_id == transaction_id);

        if let Some((key, _info)) = matching_request {
            // Found a matching request, validate it matches the response
            let response_fc = pdu.function_code()
                .map(|fc| fc & 0x7F) // Remove potential error bit
                .unwrap_or(0);

            // Check if the response matches our request
            if key.function_code == response_fc && key.slave_id == unit_id {
                debug!(
                    "Validated TCP response: trans_id={:04X}, slave_id={}, func_code={:02X}",
                    transaction_id, unit_id, response_fc
                );

                // Remove the pending request as it's been fulfilled
                let key_to_remove = key.clone();
                self.pending_requests.remove(&key_to_remove);
            } else {
                // Transaction ID matches but FC/slave doesn't - this response is not for us
                debug!(
                    "Ignoring response: trans_id={:04X} matches but FC/slave mismatch. Expected FC={:02X}/slave={}, Got FC={:02X}/slave={}",
                    transaction_id, key.function_code, key.slave_id, response_fc, unit_id
                );
                // Don't process this response - it might belong to another channel or be a delayed response
                return Err(ComSrvError::ProtocolError(
                    "Response ignored - FC/slave mismatch".to_string(),
                ));
            }
        } else {
            // No matching transaction ID - this response is not for us (might be from another channel)
            debug!(
                "Ignoring response with unknown transaction ID: {:04X}. Active transactions: {:?}",
                transaction_id,
                self.pending_requests
                    .keys()
                    .map(|k| k.transaction_id)
                    .collect::<Vec<_>>()
            );
            // Don't process this response
            return Err(ComSrvError::ProtocolError(
                "Response ignored - unknown transaction ID".to_string(),
            ));
        }

        debug!(
            "Parsed TCP frame: trans_id={:04X}, length={}, unit_id={}, pdu={:02X?}",
            transaction_id, length, unit_id, pdu
        );

        Ok((unit_id, pdu))
    }

    /// Parse RTU frame
    fn parse_rtu_frame(&mut self, data: &[u8]) -> Result<(u8, ModbusPdu)> {
        if data.len() < 4 {
            return Err(ComSrvError::ProtocolError(
                "RTU frame too short".to_string(),
            ));
        }

        let frame_len = data.len();
        let unit_id = data[0];
        let pdu_bytes = &data[1..frame_len - 2];
        let received_crc = u16::from_le_bytes([data[frame_len - 2], data[frame_len - 1]]);

        // Validate CRC
        let calculated_crc = self.calculate_crc16(&data[..frame_len - 2]);
        if received_crc != calculated_crc {
            return Err(ComSrvError::ProtocolError(format!(
                "CRC mismatch: expected 0x{calculated_crc:04X}, got 0x{received_crc:04X}"
            )));
        }

        // Convert bytes to ModbusPdu
        let pdu = ModbusPdu::from_slice(pdu_bytes)?;

        // For RTU, we need to validate against pending requests
        if !pdu.is_empty() {
            let response_fc = pdu.function_code()
                .map(|fc| fc & 0x7F) // Remove potential error bit
                .unwrap_or(0);

            // Find the most recent matching request by looking for matching slave_id and function_code
            let matching_keys: Vec<_> = self
                .pending_requests
                .keys()
                .filter(|k| k.slave_id == unit_id && k.function_code == response_fc)
                .cloned()
                .collect();

            if !matching_keys.is_empty() {
                // Sort by timestamp to get the most recent
                let most_recent_key = matching_keys
                    .iter()
                    .max_by_key(|k| self.pending_requests.get(k).map(|info| info.timestamp))
                    .unwrap();

                debug!(
                    "Validated RTU response: slave_id={}, func_code={:02X}, removing request key: {:?}",
                    unit_id, response_fc, most_recent_key
                );

                // Remove the fulfilled request
                self.pending_requests.remove(most_recent_key);
            } else {
                // Check if we have any request with matching slave ID
                let slave_requests: Vec<_> = self
                    .pending_requests
                    .keys()
                    .filter(|k| k.slave_id == unit_id)
                    .collect();

                if !slave_requests.is_empty() {
                    error!(
                        "RTU function code mismatch for slave {}: expected one of {:?}, got {:02X}",
                        unit_id,
                        slave_requests
                            .iter()
                            .map(|k| format!("{:02X}", k.function_code))
                            .collect::<Vec<_>>(),
                        response_fc
                    );
                    return Err(ComSrvError::ProtocolError(format!(
                        "Function code mismatch for slave {}: got {:02X}",
                        unit_id, response_fc
                    )));
                } else {
                    error!(
                        "RTU response from unexpected slave: {} (no pending requests)",
                        unit_id
                    );
                    return Err(ComSrvError::ProtocolError(format!(
                        "Unexpected response from slave {}",
                        unit_id
                    )));
                }
            }
        }

        Ok((unit_id, pdu))
    }

    /// Calculate CRC16 checksum (Modbus RTU standard)
    fn calculate_crc16(&self, data: &[u8]) -> u16 {
        let mut crc: u16 = 0xFFFF;

        for &byte in data {
            crc ^= u16::from(byte);
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc >>= 1;
                    crc ^= 0xA001;
                } else {
                    crc >>= 1;
                }
            }
        }

        crc
    }

    /// Verify if PDU is exception response
    pub fn is_exception_response(pdu: &[u8]) -> bool {
        !pdu.is_empty() && (pdu[0] & 0x80) != 0
    }

    /// Parse exception response
    pub fn parse_exception(pdu: &[u8]) -> Result<(u8, u8)> {
        if pdu.len() < 2 {
            return Err(ComSrvError::ProtocolError(
                "Invalid exception response".to_string(),
            ));
        }

        let function_code = pdu[0] & 0x7F; // Remove error bit
        let exception_code = pdu[1];

        Ok((function_code, exception_code))
    }

    /// Get exception description
    pub fn exception_description(exception_code: u8) -> &'static str {
        match exception_code {
            0x01 => "Illegal Function",
            0x02 => "Illegal Data Address",
            0x03 => "Illegal Data Value",
            0x04 => "Slave Device Failure",
            0x05 => "Acknowledge",
            0x06 => "Slave Device Busy",
            0x07 => "Negative Acknowledge",
            0x08 => "Memory Parity Error",
            0x0A => "Gateway Path Unavailable",
            0x0B => "Gateway Target Device Failed to Respond",
            _ => "Unknown Exception",
        }
    }

    /// Clear the stored request information (useful for testing or reset)
    pub fn clear_request_info(&mut self) {
        self.pending_requests.clear();
    }
}

/// Create connection parameters
/// Extract connection parameters from channel configuration
pub fn create_connection_params(config: &ChannelConfig) -> Result<ConnectionParams> {
    match config.protocol.as_str() {
        "modbus_tcp" => {
            // Extract TCP configuration from parameters
            let host = config
                .parameters
                .get("host")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);
            let port = config
                .parameters
                .get("port")
                .and_then(serde_yaml::Value::as_u64)
                .map(|p| p as u16);

            Ok(ConnectionParams {
                host,
                port,
                device: None,
                baud_rate: None,
                data_bits: None,
                stop_bits: None,
                parity: None,
                timeout: std::time::Duration::from_secs(5),
            })
        },
        "modbus_rtu" => {
            // Extract serial port configuration from parameters
            let device = config
                .parameters
                .get("device")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);
            let baud_rate = config
                .parameters
                .get("baud_rate")
                .and_then(serde_yaml::Value::as_u64)
                .map(|b| b as u32);
            let data_bits = config
                .parameters
                .get("data_bits")
                .and_then(serde_yaml::Value::as_u64)
                .map(|d| d as u8);
            let stop_bits = config
                .parameters
                .get("stop_bits")
                .and_then(serde_yaml::Value::as_u64)
                .map(|s| s as u8);
            let parity = config
                .parameters
                .get("parity")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            Ok(ConnectionParams {
                host: None,
                port: None,
                device,
                baud_rate,
                data_bits,
                stop_bits,
                parity,
                timeout: std::time::Duration::from_secs(1),
            })
        },
        _ => Err(ComSrvError::ConfigError(format!(
            "Unsupported protocol type: {}",
            config.protocol
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_frame_build_parse() {
        let mut processor = ModbusFrameProcessor::new(ModbusMode::Tcp);
        let pdu_bytes = vec![0x03, 0x00, 0x01, 0x00, 0x02]; // Read holding registers
        let pdu = ModbusPdu::from_slice(&pdu_bytes).unwrap();
        let slave_id = 1;

        let frame = processor.build_frame(slave_id, &pdu);
        assert_eq!(frame.len(), 12); // 7 bytes header (2 trans_id + 2 proto + 2 len + 1 unit) + 5 bytes PDU

        // The processor should have stored the request info
        assert_eq!(processor.pending_requests.len(), 1);

        let (unit_id, parsed_pdu) = processor
            .parse_frame(&frame)
            .expect("TCP frame parsing should succeed");
        assert_eq!(unit_id, slave_id);
        assert_eq!(parsed_pdu.as_slice(), pdu.as_slice());
    }

    #[test]
    fn test_rtu_frame_build_parse() {
        let mut processor = ModbusFrameProcessor::new(ModbusMode::Rtu);
        let pdu_bytes = vec![0x03, 0x00, 0x01, 0x00, 0x02]; // Read holding registers
        let pdu = ModbusPdu::from_slice(&pdu_bytes).unwrap();
        let slave_id = 1;

        let frame = processor.build_frame(slave_id, &pdu);
        assert_eq!(frame.len(), 8); // 1 byte unit_id + 5 bytes PDU + 2 bytes CRC

        // The processor should have stored the request info
        assert_eq!(processor.pending_requests.len(), 1);

        let (unit_id, parsed_pdu) = processor
            .parse_frame(&frame)
            .expect("RTU frame parsing should succeed");
        assert_eq!(unit_id, slave_id);
        assert_eq!(parsed_pdu.as_slice(), pdu.as_slice());
    }

    #[test]
    fn test_crc16_calculation() {
        let processor = ModbusFrameProcessor::new(ModbusMode::Rtu);
        let data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x01];
        let crc = processor.calculate_crc16(&data);
        // CRC calculation result should be 0x0A84 (2692 in decimal)
        assert_eq!(crc, 0x0A84);
    }

    #[test]
    fn test_exception_response() {
        let exception_pdu = vec![0x83, 0x02]; // Function 03 with exception code 02

        assert!(ModbusFrameProcessor::is_exception_response(&exception_pdu));

        let (func_code, exc_code) = ModbusFrameProcessor::parse_exception(&exception_pdu)
            .expect("exception parsing should succeed");
        assert_eq!(func_code, 0x03);
        assert_eq!(exc_code, 0x02);

        let desc = ModbusFrameProcessor::exception_description(0x02);
        assert_eq!(desc, "Illegal Data Address");
    }

    #[test]
    fn test_tcp_validation_errors() {
        let mut processor = ModbusFrameProcessor::new(ModbusMode::Tcp);

        // Build a request for slave 1, function code 3
        let request_pdu_bytes = vec![0x03, 0x00, 0x01, 0x00, 0x02];
        let request_pdu = ModbusPdu::from_slice(&request_pdu_bytes).unwrap();
        let request_frame = processor.build_frame(1, &request_pdu);

        // Extract the transaction ID from the request
        let transaction_id = u16::from_be_bytes([request_frame[0], request_frame[1]]);

        // Test 1: Response with wrong transaction ID
        let response_pdu_bytes = vec![0x03, 0x04, 0x00, 0x01, 0x00, 0x02];
        let mut wrong_tid_frame = vec![0; 7 + response_pdu_bytes.len()];
        wrong_tid_frame[0] = ((transaction_id + 1) >> 8) as u8;
        wrong_tid_frame[1] = ((transaction_id + 1) & 0xFF) as u8;
        wrong_tid_frame[2..4].copy_from_slice(&[0x00, 0x00]); // Protocol ID
        let len = (1 + response_pdu_bytes.len()) as u16;
        wrong_tid_frame[4..6].copy_from_slice(&len.to_be_bytes());
        wrong_tid_frame[6] = 1; // Slave ID
        wrong_tid_frame[7..].copy_from_slice(&response_pdu_bytes);

        let result = processor.parse_frame(&wrong_tid_frame);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unknown transaction ID"));

        // Test 2: Response with wrong slave ID (same transaction ID)
        let mut different_slave_frame = vec![0; 7 + response_pdu_bytes.len()];
        different_slave_frame[0..2].copy_from_slice(&transaction_id.to_be_bytes());
        different_slave_frame[2..4].copy_from_slice(&[0x00, 0x00]); // Protocol ID
        different_slave_frame[4..6].copy_from_slice(&len.to_be_bytes());
        different_slave_frame[6] = 2; // Different slave ID
        different_slave_frame[7..].copy_from_slice(&response_pdu_bytes);

        let result = processor.parse_frame(&different_slave_frame);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("FC/slave mismatch"));

        // Test 3: Multiple requests with different function codes
        // Build another request with different FC
        let request_pdu2_bytes = vec![0x01, 0x00, 0x00, 0x00, 0x08]; // FC01
        let request_pdu2 = ModbusPdu::from_slice(&request_pdu2_bytes).unwrap();
        let request_frame2 = processor.build_frame(1, &request_pdu2);
        let transaction_id2 = u16::from_be_bytes([request_frame2[0], request_frame2[1]]);

        // Both requests should be tracked
        assert_eq!(processor.pending_requests.len(), 2);

        // Valid response for FC03 request
        let mut response1_frame = vec![0; 7 + response_pdu_bytes.len()];
        response1_frame[0..2].copy_from_slice(&transaction_id.to_be_bytes());
        response1_frame[2..4].copy_from_slice(&[0x00, 0x00]); // Protocol ID
        response1_frame[4..6].copy_from_slice(&len.to_be_bytes());
        response1_frame[6] = 1; // Same slave ID
        response1_frame[7..].copy_from_slice(&response_pdu_bytes);

        let result = processor.parse_frame(&response1_frame);
        assert!(result.is_ok(), "Failed to parse response1: {:?}", result);

        // Valid response for FC01 request
        let response2_pdu_bytes = vec![0x01, 0x01, 0xFF]; // FC01 response
        let mut response2_frame = vec![0; 7 + response2_pdu_bytes.len()];
        response2_frame[0..2].copy_from_slice(&transaction_id2.to_be_bytes());
        response2_frame[2..4].copy_from_slice(&[0x00, 0x00]); // Protocol ID
        let len2 = (1 + response2_pdu_bytes.len()) as u16;
        response2_frame[4..6].copy_from_slice(&len2.to_be_bytes());
        response2_frame[6] = 1; // Same slave ID
        response2_frame[7..].copy_from_slice(&response2_pdu_bytes);

        let result = processor.parse_frame(&response2_frame);
        assert!(result.is_ok(), "Failed to parse response2: {:?}", result);
    }

    #[test]
    fn test_concurrent_requests() {
        let mut processor = ModbusFrameProcessor::new(ModbusMode::Tcp);

        // Build multiple requests
        let pdu1 = ModbusPdu::from_slice(&[0x03, 0x00, 0x00, 0x00, 0x01]).unwrap();
        let request1 = processor.build_frame(1, &pdu1);
        let tid1 = u16::from_be_bytes([request1[0], request1[1]]);

        let pdu2 = ModbusPdu::from_slice(&[0x01, 0x00, 0x00, 0x00, 0x08]).unwrap();
        let request2 = processor.build_frame(2, &pdu2);
        let tid2 = u16::from_be_bytes([request2[0], request2[1]]);

        let pdu3 = ModbusPdu::from_slice(&[0x04, 0x00, 0x10, 0x00, 0x02]).unwrap();
        let request3 = processor.build_frame(1, &pdu3);
        let tid3 = u16::from_be_bytes([request3[0], request3[1]]);

        // Verify all requests are tracked with composite keys
        assert_eq!(processor.pending_requests.len(), 3);

        // Check that the composite keys exist
        let key1 = RequestKey {
            transaction_id: tid1,
            function_code: 0x03,
            slave_id: 1,
        };
        let key2 = RequestKey {
            transaction_id: tid2,
            function_code: 0x01,
            slave_id: 2,
        };
        let key3 = RequestKey {
            transaction_id: tid3,
            function_code: 0x04,
            slave_id: 1,
        };

        assert!(processor.pending_requests.contains_key(&key1));
        assert!(processor.pending_requests.contains_key(&key2));
        assert!(processor.pending_requests.contains_key(&key3));

        // Responses can come in any order
        // Response for request2 comes first
        let (unit_id, _pdu) = processor
            .parse_frame(&request2)
            .expect("Should parse request2 response");
        assert_eq!(unit_id, 2);

        // Response for request1
        let (unit_id, _pdu) = processor
            .parse_frame(&request1)
            .expect("Should parse request1 response");
        assert_eq!(unit_id, 1);

        // Response for request3
        let (unit_id, _pdu) = processor
            .parse_frame(&request3)
            .expect("Should parse request3 response");
        assert_eq!(unit_id, 1);

        // All requests should have been removed
        assert_eq!(processor.pending_requests.len(), 0);
    }

    #[test]
    fn test_composite_key_validation() {
        let mut processor = ModbusFrameProcessor::new(ModbusMode::Tcp);

        // Force same transaction ID for multiple requests
        processor.next_transaction_id = 100;

        // Request 1: Slave 1, FC03
        let pdu1 = ModbusPdu::from_slice(&[0x03, 0x00, 0x00, 0x00, 0x01]).unwrap();
        let req1 = processor.build_frame(1, &pdu1);

        // Request 2: Slave 2, FC03 (same FC, different slave)
        let pdu2 = ModbusPdu::from_slice(&[0x03, 0x00, 0x00, 0x00, 0x01]).unwrap();
        let req2 = processor.build_frame(2, &pdu2);

        // Request 3: Slave 1, FC04 (same slave, different FC)
        let pdu3 = ModbusPdu::from_slice(&[0x04, 0x00, 0x00, 0x00, 0x01]).unwrap();
        let req3 = processor.build_frame(1, &pdu3);

        // All three should be tracked despite having same transaction ID
        assert_eq!(processor.pending_requests.len(), 3);

        // Each response should be matched correctly
        assert!(processor.parse_frame(&req1).is_ok());
        assert!(processor.parse_frame(&req2).is_ok());
        assert!(processor.parse_frame(&req3).is_ok());
    }
}
