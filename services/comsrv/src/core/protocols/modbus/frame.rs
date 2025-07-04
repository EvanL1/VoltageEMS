//! Modbus Frame Processing
//!
//! This module implements Modbus frame handling for both TCP (MBAP) and RTU modes,
//! including frame construction, parsing, and validation.

use std::time::{Duration, Instant};
use tracing::{debug, warn};
use crate::utils::error::{ComSrvError, Result};

/// Modbus transmission mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusMode {
    Tcp,
    Rtu,
}

/// MBAP (Modbus Application Protocol) header for TCP mode
#[derive(Debug, Clone)]
pub struct MbapHeader {
    pub transaction_id: u16,
    pub protocol_id: u16,    // Always 0 for Modbus
    pub length: u16,         // Byte count of following fields
    pub unit_id: u8,         // Slave address
}

impl MbapHeader {
    /// Create new MBAP header
    pub fn new(transaction_id: u16, unit_id: u8, pdu_length: u16) -> Self {
        Self {
            transaction_id,
            protocol_id: 0,
            length: pdu_length + 1, // PDU length + unit_id
            unit_id,
        }
    }

    /// Serialize MBAP header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.transaction_id.to_be_bytes());
        bytes.extend_from_slice(&self.protocol_id.to_be_bytes());
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.push(self.unit_id);
        bytes
    }

    /// Parse MBAP header from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 7 {
            return Err(ComSrvError::ProtocolError("Invalid MBAP header length".to_string()));
        }

        let transaction_id = u16::from_be_bytes([data[0], data[1]]);
        let protocol_id = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let unit_id = data[6];

        if protocol_id != 0 {
            return Err(ComSrvError::ProtocolError(format!("Invalid protocol ID: {}", protocol_id)));
        }

        Ok(Self {
            transaction_id,
            protocol_id,
            length,
            unit_id,
        })
    }

    /// Get total frame length including MBAP header
    pub fn frame_length(&self) -> usize {
        7 + self.length as usize - 1 // MBAP header (7 bytes) + PDU length - unit_id (already counted in header)
    }

    /// Get PDU length
    pub fn pdu_length(&self) -> u16 {
        self.length - 1 // Subtract unit_id from length
    }
}

/// RTU frame structure
#[derive(Debug, Clone)]
pub struct RtuFrame {
    pub slave_address: u8,
    pub pdu: Vec<u8>,
    pub crc: u16,
}

impl RtuFrame {
    /// Create new RTU frame
    pub fn new(slave_address: u8, pdu: Vec<u8>) -> Self {
        let crc = Self::calculate_crc(&[&[slave_address], pdu.as_slice()].concat());
        Self {
            slave_address,
            pdu,
            crc,
        }
    }

    /// Serialize RTU frame to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.slave_address);
        bytes.extend_from_slice(&self.pdu);
        bytes.extend_from_slice(&self.crc.to_le_bytes()); // CRC is little-endian in RTU
        bytes
    }

    /// Parse RTU frame from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(ComSrvError::ProtocolError("RTU frame too short".to_string()));
        }

        let slave_address = data[0];
        let pdu_end = data.len() - 2;
        let pdu = data[1..pdu_end].to_vec();
        let crc = u16::from_le_bytes([data[pdu_end], data[pdu_end + 1]]);

        // Verify CRC
        let calculated_crc = Self::calculate_crc(&data[..pdu_end]);
        if crc != calculated_crc {
            return Err(ComSrvError::ProtocolError(format!(
                "CRC mismatch: expected 0x{:04X}, got 0x{:04X}",
                calculated_crc, crc
            )));
        }

        Ok(Self {
            slave_address,
            pdu,
            crc,
        })
    }

    /// Calculate CRC-16 for RTU mode
    pub fn calculate_crc(data: &[u8]) -> u16 {
        const CRC_TABLE: [u16; 256] = [
            0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241,
            0xC601, 0x06C0, 0x0780, 0xC741, 0x0500, 0xC5C1, 0xC481, 0x0440,
            0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1, 0xCE81, 0x0E40,
            0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841,
            0xD801, 0x18C0, 0x1980, 0xD941, 0x1B00, 0xDBC1, 0xDA81, 0x1A40,
            0x1E00, 0xDEC1, 0xDF81, 0x1F40, 0xDD01, 0x1DC0, 0x1C80, 0xDC41,
            0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680, 0xD641,
            0xD201, 0x12C0, 0x1380, 0xD341, 0x1100, 0xD1C1, 0xD081, 0x1040,
            0xF001, 0x30C0, 0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240,
            0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501, 0x35C0, 0x3480, 0xF441,
            0x3C00, 0xFCC1, 0xFD81, 0x3D40, 0xFF01, 0x3FC0, 0x3E80, 0xFE41,
            0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840,
            0x2800, 0xE8C1, 0xE981, 0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41,
            0xEE01, 0x2EC0, 0x2F80, 0xEF41, 0x2D00, 0xEDC1, 0xEC81, 0x2C40,
            0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640,
            0x2200, 0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041,
            0xA001, 0x60C0, 0x6180, 0xA141, 0x6300, 0xA3C1, 0xA281, 0x6240,
            0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480, 0xA441,
            0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41,
            0xAA01, 0x6AC0, 0x6B80, 0xAB41, 0x6900, 0xA9C1, 0xA881, 0x6840,
            0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01, 0x7BC0, 0x7A80, 0xBA41,
            0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40,
            0xB401, 0x74C0, 0x7580, 0xB541, 0x7700, 0xB7C1, 0xB681, 0x7640,
            0x7200, 0xB2C1, 0xB381, 0x7340, 0xB101, 0x71C0, 0x7080, 0xB041,
            0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0, 0x5280, 0x9241,
            0x9601, 0x56C0, 0x5780, 0x9741, 0x5500, 0x95C1, 0x9481, 0x5440,
            0x9C01, 0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40,
            0x5A00, 0x9AC1, 0x9B81, 0x5B40, 0x9901, 0x59C0, 0x5880, 0x9841,
            0x8801, 0x48C0, 0x4980, 0x8941, 0x4B00, 0x8BC1, 0x8A81, 0x4A40,
            0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41,
            0x4400, 0x84C1, 0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641,
            0x8201, 0x42C0, 0x4380, 0x8341, 0x4100, 0x81C1, 0x8081, 0x4040,
        ];

        let mut crc = 0xFFFFu16;
        for &byte in data {
            let table_index = ((crc ^ byte as u16) & 0xFF) as usize;
            crc = (crc >> 8) ^ CRC_TABLE[table_index];
        }
        crc
    }

    /// Verify CRC of the frame
    pub fn verify_crc(&self) -> bool {
        let data = [&[self.slave_address], self.pdu.as_slice()].concat();
        let calculated_crc = Self::calculate_crc(&data);
        self.crc == calculated_crc
    }
}

/// TCP frame structure
#[derive(Debug, Clone)]
pub struct TcpFrame {
    pub mbap_header: MbapHeader,
    pub pdu: Vec<u8>,
}

impl TcpFrame {
    /// Create new TCP frame
    pub fn new(transaction_id: u16, unit_id: u8, pdu: Vec<u8>) -> Self {
        let mbap_header = MbapHeader::new(transaction_id, unit_id, pdu.len() as u16);
        Self {
            mbap_header,
            pdu,
        }
    }

    /// Serialize TCP frame to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.mbap_header.to_bytes();
        bytes.extend_from_slice(&self.pdu);
        bytes
    }

    /// Parse TCP frame from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let mbap_header = MbapHeader::from_bytes(data)?;
        
        if data.len() < mbap_header.frame_length() {
            return Err(ComSrvError::ProtocolError("Incomplete TCP frame".to_string()));
        }

        let pdu = data[7..7 + mbap_header.pdu_length() as usize].to_vec();

        Ok(Self {
            mbap_header,
            pdu,
        })
    }
}

/// Modbus frame processor
#[derive(Debug)]
pub struct ModbusFrameProcessor {
    mode: ModbusMode,
    /// Last frame received time (for RTU timing)
    last_frame_time: Option<Instant>,
    /// RTU frame gap timeout (typically 3.5 character times)
    rtu_frame_gap: Duration,
}

impl ModbusFrameProcessor {
    /// Create new frame processor
    pub fn new(mode: ModbusMode) -> Self {
        // Default RTU frame gap for 9600 baud: 3.5 * (1 start + 8 data + 1 parity + 1 stop) / 9600 = ~3.6ms
        let rtu_frame_gap = Duration::from_millis(4);
        
        Self {
            mode,
            last_frame_time: None,
            rtu_frame_gap,
        }
    }

    /// Create new frame processor with specific baud rate for RTU
    pub fn new_with_baud_rate(mode: ModbusMode, baud_rate: u32) -> Self {
        let rtu_frame_gap = if mode == ModbusMode::Rtu {
            Self::calculate_frame_gap(baud_rate)
        } else {
            Duration::from_millis(4) // Default for TCP mode (not used)
        };
        
        Self {
            mode,
            last_frame_time: None,
            rtu_frame_gap,
        }
    }

    /// Calculate RTU frame gap based on baud rate
    /// Frame gap = 3.5 character times
    /// Character time = (1 start + 8 data + 1 parity + 1 stop) bits / baud_rate
    pub fn calculate_frame_gap(baud_rate: u32) -> Duration {
        if baud_rate == 0 {
            return Duration::from_millis(4); // Default fallback
        }
        
        // For baud rates > 19200, use fixed 1.75ms as per Modbus spec
        if baud_rate > 19200 {
            return Duration::from_micros(1750);
        }
        
        // Calculate based on 3.5 character times
        // 1 character = 11 bits (1 start + 8 data + 1 parity + 1 stop)
        let char_time_us = (11 * 1_000_000) / baud_rate;
        let frame_gap_us = (char_time_us * 35) / 10; // 3.5 character times
        
        // Add small margin for safety
        Duration::from_micros(frame_gap_us as u64 + 100)
    }

    /// Set RTU frame gap timeout
    pub fn set_rtu_frame_gap(&mut self, gap: Duration) {
        self.rtu_frame_gap = gap;
    }

    /// Build frame for transmission
    pub fn build_frame(&self, unit_id: u8, pdu: Vec<u8>, transaction_id: Option<u16>) -> Vec<u8> {
        match self.mode {
            ModbusMode::Tcp => {
                let tcp_frame = TcpFrame::new(transaction_id.unwrap_or(0), unit_id, pdu);
                tcp_frame.to_bytes()
            },
            ModbusMode::Rtu => {
                let rtu_frame = RtuFrame::new(unit_id, pdu);
                rtu_frame.to_bytes()
            },
        }
    }

    /// Parse received frame data
    pub fn parse_frame(&mut self, data: &[u8]) -> Result<ParsedFrame> {
        match self.mode {
            ModbusMode::Tcp => self.parse_tcp_frame(data),
            ModbusMode::Rtu => self.parse_rtu_frame(data),
        }
    }

    /// Parse TCP frame
    fn parse_tcp_frame(&self, data: &[u8]) -> Result<ParsedFrame> {
        let tcp_frame = TcpFrame::from_bytes(data)?;
        
        Ok(ParsedFrame {
            unit_id: tcp_frame.mbap_header.unit_id,
            pdu: tcp_frame.pdu,
            transaction_id: Some(tcp_frame.mbap_header.transaction_id),
            frame_mode: ModbusMode::Tcp,
        })
    }

    /// Parse RTU frame
    fn parse_rtu_frame(&mut self, data: &[u8]) -> Result<ParsedFrame> {
        let now = Instant::now();
        
        // Check frame gap timing for RTU
        if let Some(last_time) = self.last_frame_time {
            let elapsed = now.duration_since(last_time);
            if elapsed < self.rtu_frame_gap {
                return Err(ComSrvError::ProtocolError("RTU frame gap violation".to_string()));
            }
        }
        
        self.last_frame_time = Some(now);
        
        let rtu_frame = RtuFrame::from_bytes(data)?;
        
        Ok(ParsedFrame {
            unit_id: rtu_frame.slave_address,
            pdu: rtu_frame.pdu,
            transaction_id: None,
            frame_mode: ModbusMode::Rtu,
        })
    }

    /// Check if we have a complete frame in the buffer
    pub fn is_frame_complete(&self, buffer: &[u8]) -> Result<Option<usize>> {
        match self.mode {
            ModbusMode::Tcp => self.check_tcp_frame_complete(buffer),
            ModbusMode::Rtu => self.check_rtu_frame_complete(buffer),
        }
    }

    /// Check TCP frame completeness
    fn check_tcp_frame_complete(&self, buffer: &[u8]) -> Result<Option<usize>> {
        if buffer.len() < 7 {
            return Ok(None); // Need at least MBAP header
        }

        let mbap_header = MbapHeader::from_bytes(buffer)?;
        let total_length = mbap_header.frame_length();

        if buffer.len() >= total_length {
            Ok(Some(total_length))
        } else {
            Ok(None)
        }
    }

    /// Check RTU frame completeness
    fn check_rtu_frame_complete(&self, buffer: &[u8]) -> Result<Option<usize>> {
        // For RTU, frame completeness is determined by:
        // 1. Minimum frame size (4 bytes: address + function + CRC)
        // 2. Silent interval detection (3.5 character times)
        // 3. Function code specific length validation
        
        if buffer.len() < 4 {
            return Ok(None);
        }

        // Check if we have a complete frame based on function code
        if let Some(frame_length) = self.estimate_rtu_frame_length(buffer) {
            if buffer.len() >= frame_length {
                // Validate CRC before accepting the frame
                let data_end = frame_length - 2;
                let data = &buffer[..data_end];
                let crc_received = u16::from_le_bytes([buffer[data_end], buffer[data_end + 1]]);
                let crc_calculated = RtuFrame::calculate_crc(data);
                
                if crc_received == crc_calculated {
                    debug!(
                        "[Frame Processor] RTU frame complete - Length: {}, CRC valid: 0x{:04X}", 
                        frame_length, crc_received
                    );
                    Ok(Some(frame_length))
                } else {
                    warn!(
                        "[Frame Processor] RTU frame CRC mismatch - Expected: 0x{:04X}, Got: 0x{:04X}",
                        crc_calculated, crc_received
                    );
                    // CRC mismatch - might be incomplete frame or corruption
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Estimate RTU frame length based on function code
    fn estimate_rtu_frame_length(&self, buffer: &[u8]) -> Option<usize> {
        if buffer.len() < 2 {
            return None;
        }

        let _slave_address = buffer[0];
        let function_code = buffer[1];

        // Check if this is an exception response
        if function_code & 0x80 != 0 {
            return Some(5); // Address + Exception Function + Exception Code + CRC
        }

        match function_code {
            // Read coils/discrete inputs response
            0x01 | 0x02 => {
                if buffer.len() < 3 {
                    return None;
                }
                let byte_count = buffer[2] as usize;
                Some(3 + byte_count + 2) // Address + Function + ByteCount + Data + CRC
            },
            // Read holding/input registers response
            0x03 | 0x04 => {
                if buffer.len() < 3 {
                    return None;
                }
                let byte_count = buffer[2] as usize;
                Some(3 + byte_count + 2) // Address + Function + ByteCount + Data + CRC
            },
            // Write single coil response
            0x05 => Some(8), // Address + Function + Address(2) + Value(2) + CRC(2)
            // Write single register response
            0x06 => Some(8), // Address + Function + Address(2) + Value(2) + CRC(2)
            // Write multiple coils response
            0x0F => Some(8), // Address + Function + Address(2) + Quantity(2) + CRC(2)
            // Write multiple registers response
            0x10 => Some(8), // Address + Function + Address(2) + Quantity(2) + CRC(2)
            // Read device identification (if needed)
            0x2B => {
                if buffer.len() < 3 {
                    return None;
                }
                // This is complex - just return a reasonable estimate
                Some(64) 
            },
            // For requests (when parsing incoming data), we need different logic
            // This simplified version assumes we're parsing responses
            _ => {
                debug!(
                    "[Frame Processor] Unknown function code 0x{:02X}, using default frame length",
                    function_code
                );
                Some(8) // Default minimum for unknown function codes
            }
        }
    }

    /// Get frame gap duration for RTU mode
    pub fn get_rtu_frame_gap(&self) -> Duration {
        self.rtu_frame_gap
    }

    /// Reset frame timing (useful for RTU mode)
    pub fn reset_frame_timing(&mut self) {
        self.last_frame_time = None;
    }
}

/// Parsed frame result
#[derive(Debug, Clone)]
pub struct ParsedFrame {
    pub unit_id: u8,
    pub pdu: Vec<u8>,
    pub transaction_id: Option<u16>, // Only present in TCP mode
    pub frame_mode: ModbusMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbap_header() {
        let header = MbapHeader::new(0x1234, 0x01, 5);
        assert_eq!(header.transaction_id, 0x1234);
        assert_eq!(header.protocol_id, 0);
        assert_eq!(header.length, 6); // PDU length + unit_id
        assert_eq!(header.unit_id, 0x01);

        let bytes = header.to_bytes();
        assert_eq!(bytes, vec![0x12, 0x34, 0x00, 0x00, 0x00, 0x06, 0x01]);

        let parsed = MbapHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.transaction_id, header.transaction_id);
        assert_eq!(parsed.unit_id, header.unit_id);
    }

    #[test]
    fn test_rtu_crc() {
        // Test case 1: Read holding registers request
        let data1 = [0x01, 0x03, 0x00, 0x01, 0x00, 0x02];
        let crc1 = RtuFrame::calculate_crc(&data1);
        assert_eq!(crc1, 0x95C4); // Known CRC for this data
        
        // Test case 2: Write single register request
        let data2 = [0x01, 0x06, 0x00, 0x01, 0x00, 0x03];
        let crc2 = RtuFrame::calculate_crc(&data2);
        assert_eq!(crc2, 0x9A9B); // Known CRC for this data
        
        // Test case 3: Exception response
        let data3 = [0x01, 0x83, 0x02];
        let crc3 = RtuFrame::calculate_crc(&data3);
        assert_eq!(crc3, 0xC0F1); // Known CRC for this data
    }

    #[test]
    fn test_rtu_frame() {
        let pdu = vec![0x03, 0x00, 0x01, 0x00, 0x02];
        let frame = RtuFrame::new(0x01, pdu.clone());
        
        assert_eq!(frame.slave_address, 0x01);
        assert_eq!(frame.pdu, pdu);
        assert!(frame.verify_crc());

        let bytes = frame.to_bytes();
        let parsed = RtuFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.slave_address, frame.slave_address);
        assert_eq!(parsed.pdu, frame.pdu);
        assert!(parsed.verify_crc());
    }

    #[test]
    fn test_tcp_frame() {
        let pdu = vec![0x03, 0x00, 0x01, 0x00, 0x02];
        let frame = TcpFrame::new(0x1234, 0x01, pdu.clone());
        
        assert_eq!(frame.mbap_header.transaction_id, 0x1234);
        assert_eq!(frame.mbap_header.unit_id, 0x01);
        assert_eq!(frame.pdu, pdu);

        let bytes = frame.to_bytes();
        let parsed = TcpFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.mbap_header.transaction_id, frame.mbap_header.transaction_id);
        assert_eq!(parsed.mbap_header.unit_id, frame.mbap_header.unit_id);
        assert_eq!(parsed.pdu, frame.pdu);
    }

    #[test]
    fn test_frame_processor_tcp() {
        let processor = ModbusFrameProcessor::new(ModbusMode::Tcp);
        let pdu = vec![0x03, 0x00, 0x01, 0x00, 0x02];
        
        let frame_bytes = processor.build_frame(0x01, pdu.clone(), Some(0x1234));
        let expected = vec![0x12, 0x34, 0x00, 0x00, 0x00, 0x06, 0x01, 0x03, 0x00, 0x01, 0x00, 0x02];
        assert_eq!(frame_bytes, expected);
    }

    #[test]
    fn test_frame_processor_rtu() {
        let processor = ModbusFrameProcessor::new(ModbusMode::Rtu);
        let pdu = vec![0x03, 0x00, 0x01, 0x00, 0x02];
        
        let frame_bytes = processor.build_frame(0x01, pdu.clone(), None);
        // Should be: address(0x01) + pdu + crc
        assert_eq!(frame_bytes[0], 0x01);
        assert_eq!(frame_bytes[1..6], pdu);
        assert_eq!(frame_bytes.len(), 8); // 1 + 5 + 2
        
        // Verify CRC is correct
        let frame_without_crc = &frame_bytes[..6];
        let crc_calculated = RtuFrame::calculate_crc(frame_without_crc);
        let crc_in_frame = u16::from_le_bytes([frame_bytes[6], frame_bytes[7]]);
        assert_eq!(crc_calculated, crc_in_frame);
    }

    #[test]
    fn test_frame_gap_calculation() {
        // Test 9600 baud
        let gap_9600 = ModbusFrameProcessor::calculate_frame_gap(9600);
        // 3.5 * 11 bits / 9600 baud = ~4.01ms
        assert!(gap_9600.as_millis() >= 4);
        assert!(gap_9600.as_millis() <= 5);
        
        // Test 19200 baud
        let gap_19200 = ModbusFrameProcessor::calculate_frame_gap(19200);
        // 3.5 * 11 bits / 19200 baud = ~2.00ms
        assert!(gap_19200.as_millis() >= 2);
        assert!(gap_19200.as_millis() <= 3);
        
        // Test high baud rate (should use fixed 1.75ms)
        let gap_115200 = ModbusFrameProcessor::calculate_frame_gap(115200);
        assert_eq!(gap_115200.as_micros(), 1750);
    }

    #[test]
    fn test_rtu_frame_completeness_check() {
        let mut processor = ModbusFrameProcessor::new(ModbusMode::Rtu);
        
        // Test incomplete frame
        let incomplete = vec![0x01, 0x03, 0x02];
        assert_eq!(processor.check_rtu_frame_complete(&incomplete).unwrap(), None);
        
        // Test complete frame with valid CRC
        let complete = vec![0x01, 0x03, 0x02, 0x00, 0x64, 0xB9, 0xF9]; // Valid response
        assert_eq!(processor.check_rtu_frame_complete(&complete).unwrap(), Some(7));
        
        // Test frame with invalid CRC
        let invalid_crc = vec![0x01, 0x03, 0x02, 0x00, 0x64, 0x00, 0x00]; // Bad CRC
        assert_eq!(processor.check_rtu_frame_complete(&invalid_crc).unwrap(), None);
    }
} 