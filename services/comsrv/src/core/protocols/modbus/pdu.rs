//! Modbus PDU (Protocol Data Unit) Implementation
//!
//! This module implements the Modbus Protocol Data Unit handling,
//! including parsing requests and building responses for all standard Modbus function codes.

use serde::{Deserialize, Serialize};
use crate::utils::error::{ComSrvError, Result};

/// Modbus function codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModbusFunctionCode {
    ReadCoils = 0x01,
    ReadDiscreteInputs = 0x02,
    ReadHoldingRegisters = 0x03,
    ReadInputRegisters = 0x04,
    WriteSingleCoil = 0x05,
    WriteSingleRegister = 0x06,
    WriteMultipleCoils = 0x0F,
    WriteMultipleRegisters = 0x10,
}

impl From<ModbusFunctionCode> for u8 {
    fn from(code: ModbusFunctionCode) -> u8 {
        code as u8
    }
}

impl TryFrom<u8> for ModbusFunctionCode {
    type Error = ComSrvError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0x01 => Ok(ModbusFunctionCode::ReadCoils),
            0x02 => Ok(ModbusFunctionCode::ReadDiscreteInputs),
            0x03 => Ok(ModbusFunctionCode::ReadHoldingRegisters),
            0x04 => Ok(ModbusFunctionCode::ReadInputRegisters),
            0x05 => Ok(ModbusFunctionCode::WriteSingleCoil),
            0x06 => Ok(ModbusFunctionCode::WriteSingleRegister),
            0x0F => Ok(ModbusFunctionCode::WriteMultipleCoils),
            0x10 => Ok(ModbusFunctionCode::WriteMultipleRegisters),
            _ => Err(ComSrvError::ProtocolError(format!("Invalid function code: 0x{:02X}", value))),
        }
    }
}

/// Modbus exception codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModbusExceptionCode {
    IllegalFunction = 0x01,
    IllegalDataAddress = 0x02,
    IllegalDataValue = 0x03,
    SlaveDeviceFailure = 0x04,
    Acknowledge = 0x05,
    SlaveDeviceBusy = 0x06,
    NegativeAcknowledge = 0x07,
    MemoryParityError = 0x08,
    GatewayPathUnavailable = 0x0A,
    GatewayTargetDeviceFailedToRespond = 0x0B,
}

impl From<ModbusExceptionCode> for u8 {
    fn from(code: ModbusExceptionCode) -> u8 {
        code as u8
    }
}

/// Modbus PDU request
#[derive(Debug, Clone)]
pub struct ModbusPduRequest {
    pub function_code: ModbusFunctionCode,
    pub data: Vec<u8>,
}

/// Modbus PDU response
#[derive(Debug, Clone)]
pub struct ModbusPduResponse {
    pub function_code: ModbusFunctionCode,
    pub data: Vec<u8>,
}

/// Modbus PDU exception response
#[derive(Debug, Clone)]
pub struct ModbusPduException {
    pub function_code: u8, // Function code with 0x80 bit set
    pub exception_code: ModbusExceptionCode,
}

/// PDU parsing result
#[derive(Debug, Clone)]
pub enum PduParseResult {
    Request(ModbusPduRequest),
    Response(ModbusPduResponse),
    Exception(ModbusPduException),
}

/// Read request data (for function codes 0x01, 0x02, 0x03, 0x04)
#[derive(Debug, Clone)]
pub struct ReadRequest {
    pub start_address: u16,
    pub quantity: u16,
}

/// Write single request data (for function codes 0x05, 0x06)
#[derive(Debug, Clone)]
pub struct WriteSingleRequest {
    pub address: u16,
    pub value: u16,
}

/// Write multiple coils request data (for function code 0x0F)
#[derive(Debug, Clone)]
pub struct WriteMultipleCoilsRequest {
    pub start_address: u16,
    pub quantity: u16,
    pub byte_count: u8,
    pub values: Vec<bool>,
}

/// Write multiple registers request data (for function code 0x10)
#[derive(Debug, Clone)]
pub struct WriteMultipleRegistersRequest {
    pub start_address: u16,
    pub quantity: u16,
    pub byte_count: u8,
    pub values: Vec<u16>,
}

/// Modbus PDU processor
#[derive(Debug)]
pub struct ModbusPduProcessor;

impl ModbusPduProcessor {
    pub fn new() -> Self {
        Self
    }

    /// Parse PDU from byte slice
    pub fn parse_pdu(&self, data: &[u8]) -> Result<PduParseResult> {
        if data.is_empty() {
            return Err(ComSrvError::ProtocolError("Empty PDU".to_string()));
        }

        let function_code_raw = data[0];
        
        // Check if this is an exception response
        if function_code_raw & 0x80 != 0 {
            return self.parse_exception_response(data);
        }

        let function_code = ModbusFunctionCode::try_from(function_code_raw)?;
        let pdu_data = &data[1..];

        let request = ModbusPduRequest {
            function_code,
            data: pdu_data.to_vec(),
        };

        Ok(PduParseResult::Request(request))
    }

    /// Parse exception response
    fn parse_exception_response(&self, data: &[u8]) -> Result<PduParseResult> {
        if data.len() < 2 {
            return Err(ComSrvError::ProtocolError("Invalid exception response length".to_string()));
        }

        let function_code = data[0];
        let exception_code_raw = data[1];

        let exception_code = match exception_code_raw {
            0x01 => ModbusExceptionCode::IllegalFunction,
            0x02 => ModbusExceptionCode::IllegalDataAddress,
            0x03 => ModbusExceptionCode::IllegalDataValue,
            0x04 => ModbusExceptionCode::SlaveDeviceFailure,
            0x05 => ModbusExceptionCode::Acknowledge,
            0x06 => ModbusExceptionCode::SlaveDeviceBusy,
            0x07 => ModbusExceptionCode::NegativeAcknowledge,
            0x08 => ModbusExceptionCode::MemoryParityError,
            0x0A => ModbusExceptionCode::GatewayPathUnavailable,
            0x0B => ModbusExceptionCode::GatewayTargetDeviceFailedToRespond,
            _ => return Err(ComSrvError::ProtocolError(format!("Invalid exception code: 0x{:02X}", exception_code_raw))),
        };

        let exception = ModbusPduException {
            function_code,
            exception_code,
        };

        Ok(PduParseResult::Exception(exception))
    }

    /// Parse read request (0x01, 0x02, 0x03, 0x04)
    pub fn parse_read_request(&self, data: &[u8]) -> Result<ReadRequest> {
        if data.len() < 4 {
            return Err(ComSrvError::ProtocolError("Invalid read request length".to_string()));
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);

        Ok(ReadRequest {
            start_address,
            quantity,
        })
    }

    /// Parse write single request (0x05, 0x06)
    pub fn parse_write_single_request(&self, data: &[u8]) -> Result<WriteSingleRequest> {
        if data.len() < 4 {
            return Err(ComSrvError::ProtocolError("Invalid write single request length".to_string()));
        }

        let address = u16::from_be_bytes([data[0], data[1]]);
        let value = u16::from_be_bytes([data[2], data[3]]);

        Ok(WriteSingleRequest {
            address,
            value,
        })
    }

    /// Parse write multiple coils request (0x0F)
    pub fn parse_write_multiple_coils_request(&self, data: &[u8]) -> Result<WriteMultipleCoilsRequest> {
        if data.len() < 5 {
            return Err(ComSrvError::ProtocolError("Invalid write multiple coils request length".to_string()));
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        let byte_count = data[4];

        if data.len() < (5 + byte_count as usize) {
            return Err(ComSrvError::ProtocolError("Invalid write multiple coils data length".to_string()));
        }

        let coil_bytes = &data[5..5 + byte_count as usize];
        let mut values = Vec::new();

        // Convert bytes to individual coil values
        for (byte_idx, &byte) in coil_bytes.iter().enumerate() {
            for bit_idx in 0..8 {
                if byte_idx * 8 + bit_idx < quantity as usize {
                    values.push((byte >> bit_idx) & 1 != 0);
                }
            }
        }

        Ok(WriteMultipleCoilsRequest {
            start_address,
            quantity,
            byte_count,
            values,
        })
    }

    /// Parse write multiple registers request (0x10)
    pub fn parse_write_multiple_registers_request(&self, data: &[u8]) -> Result<WriteMultipleRegistersRequest> {
        if data.len() < 5 {
            return Err(ComSrvError::ProtocolError("Invalid write multiple registers request length".to_string()));
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        let byte_count = data[4];

        if data.len() < (5 + byte_count as usize) {
            return Err(ComSrvError::ProtocolError("Invalid write multiple registers data length".to_string()));
        }

        let register_bytes = &data[5..5 + byte_count as usize];
        let mut values = Vec::new();

        // Convert bytes to register values
        for chunk in register_bytes.chunks(2) {
            if chunk.len() == 2 {
                values.push(u16::from_be_bytes([chunk[0], chunk[1]]));
            }
        }

        Ok(WriteMultipleRegistersRequest {
            start_address,
            quantity,
            byte_count,
            values,
        })
    }

    /// Build read response PDU (0x01, 0x02, 0x03, 0x04)
    pub fn build_read_response(&self, function_code: ModbusFunctionCode, data: &[u8]) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.push(function_code.into());
        pdu.push(data.len() as u8); // Byte count
        pdu.extend_from_slice(data);
        pdu
    }

    /// Build read coils/discrete inputs response data from boolean values
    pub fn build_coil_response_data(&self, values: &[bool]) -> Vec<u8> {
        let byte_count = (values.len() + 7) / 8; // Round up to nearest byte
        let mut data = vec![0u8; byte_count];

        for (i, &value) in values.iter().enumerate() {
            if value {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                data[byte_idx] |= 1 << bit_idx;
            }
        }

        data
    }

    /// Build read registers response data from u16 values
    pub fn build_register_response_data(&self, values: &[u16]) -> Vec<u8> {
        let mut data = Vec::new();
        for &value in values {
            data.extend_from_slice(&value.to_be_bytes());
        }
        data
    }

    /// Build write single response PDU (0x05, 0x06)
    pub fn build_write_single_response(&self, function_code: ModbusFunctionCode, address: u16, value: u16) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.push(function_code.into());
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&value.to_be_bytes());
        pdu
    }

    /// Build write multiple response PDU (0x0F, 0x10)
    pub fn build_write_multiple_response(&self, function_code: ModbusFunctionCode, start_address: u16, quantity: u16) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.push(function_code.into());
        pdu.extend_from_slice(&start_address.to_be_bytes());
        pdu.extend_from_slice(&quantity.to_be_bytes());
        pdu
    }

    /// Build exception response PDU
    pub fn build_exception_response(&self, function_code: ModbusFunctionCode, exception_code: ModbusExceptionCode) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.push(u8::from(function_code) | 0x80); // Set error bit
        pdu.push(exception_code.into());
        pdu
    }

    /// Build request PDU for read operations (0x01, 0x02, 0x03, 0x04)
    pub fn build_read_request(&self, function_code: ModbusFunctionCode, start_address: u16, quantity: u16) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.push(function_code.into());
        pdu.extend_from_slice(&start_address.to_be_bytes());
        pdu.extend_from_slice(&quantity.to_be_bytes());
        pdu
    }

    /// Build request PDU for write single operations (0x05, 0x06)
    pub fn build_write_single_request(&self, function_code: ModbusFunctionCode, address: u16, value: u16) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.push(function_code.into());
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&value.to_be_bytes());
        pdu
    }

    /// Build request PDU for write multiple coils (0x0F)
    pub fn build_write_multiple_coils_request(&self, start_address: u16, values: &[bool]) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.push(ModbusFunctionCode::WriteMultipleCoils.into());
        pdu.extend_from_slice(&start_address.to_be_bytes());
        pdu.extend_from_slice(&(values.len() as u16).to_be_bytes());
        
        let coil_data = self.build_coil_response_data(values);
        pdu.push(coil_data.len() as u8);
        pdu.extend_from_slice(&coil_data);
        
        pdu
    }

    /// Build request PDU for write multiple registers (0x10)
    pub fn build_write_multiple_registers_request(&self, start_address: u16, values: &[u16]) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.push(ModbusFunctionCode::WriteMultipleRegisters.into());
        pdu.extend_from_slice(&start_address.to_be_bytes());
        pdu.extend_from_slice(&(values.len() as u16).to_be_bytes());
        
        let register_data = self.build_register_response_data(values);
        pdu.push(register_data.len() as u8);
        pdu.extend_from_slice(&register_data);
        
        pdu
    }
}

impl Default for ModbusPduProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_code_conversion() {
        assert_eq!(u8::from(ModbusFunctionCode::ReadCoils), 0x01);
        assert_eq!(u8::from(ModbusFunctionCode::ReadHoldingRegisters), 0x03);
        
        assert_eq!(ModbusFunctionCode::try_from(0x01).unwrap(), ModbusFunctionCode::ReadCoils);
        assert_eq!(ModbusFunctionCode::try_from(0x03).unwrap(), ModbusFunctionCode::ReadHoldingRegisters);
        
        assert!(ModbusFunctionCode::try_from(0xFF).is_err());
    }

    #[test]
    fn test_read_request_parsing() {
        let processor = ModbusPduProcessor::new();
        let data = [0x00, 0x01, 0x00, 0x0A]; // Start address 1, quantity 10
        
        let request = processor.parse_read_request(&data).unwrap();
        assert_eq!(request.start_address, 1);
        assert_eq!(request.quantity, 10);
    }

    #[test]
    fn test_write_single_request_parsing() {
        let processor = ModbusPduProcessor::new();
        let data = [0x00, 0x01, 0x00, 0xFF]; // Address 1, value 255
        
        let request = processor.parse_write_single_request(&data).unwrap();
        assert_eq!(request.address, 1);
        assert_eq!(request.value, 255);
    }

    #[test]
    fn test_coil_response_data_building() {
        let processor = ModbusPduProcessor::new();
        let values = [true, false, true, true, false, false, true, false, true];
        
        let data = processor.build_coil_response_data(&values);
        
        // First byte: 11011001 = 0xD9 (LSB first)
        // Second byte: 00000001 = 0x01
        assert_eq!(data, vec![0xCD, 0x01]); // 11001101, 00000001
    }

    #[test]
    fn test_register_response_data_building() {
        let processor = ModbusPduProcessor::new();
        let values = [0x1234, 0x5678];
        
        let data = processor.build_register_response_data(&values);
        assert_eq!(data, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_exception_response_building() {
        let processor = ModbusPduProcessor::new();
        
        let pdu = processor.build_exception_response(
            ModbusFunctionCode::ReadCoils,
            ModbusExceptionCode::IllegalDataAddress
        );
        
        assert_eq!(pdu, vec![0x81, 0x02]); // 0x01 | 0x80, 0x02
    }

    #[test]
    fn test_read_request_building() {
        let processor = ModbusPduProcessor::new();
        
        let pdu = processor.build_read_request(
            ModbusFunctionCode::ReadHoldingRegisters,
            0x0001,
            0x000A
        );
        
        assert_eq!(pdu, vec![0x03, 0x00, 0x01, 0x00, 0x0A]);
    }
} 