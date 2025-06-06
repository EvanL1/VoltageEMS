//! Modbus RTU Server Mock for Testing
//! 
//! This module provides a mock Modbus RTU server implementation that can
//! respond to client requests for comprehensive integration testing.

use std::collections::HashMap;
// CRC calculation for Modbus RTU

/// Calculate CRC16 for Modbus RTU
fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    
    for byte in data {
        crc ^= *byte as u16;
        for _ in 0..8 {
            if crc & 0x0001 != 0 {
                crc >>= 1;
                crc ^= 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    
    crc
}

/// Mock Modbus RTU server for testing
#[derive(Debug)]
pub struct MockModbusRtuServer {
    /// Server slave ID
    slave_id: u8,
    /// Coil values (addresses 0-65535)
    coils: HashMap<u16, bool>,
    /// Discrete input values
    discrete_inputs: HashMap<u16, bool>,
    /// Holding register values
    holding_registers: HashMap<u16, u16>,
    /// Input register values
    input_registers: HashMap<u16, u16>,
    /// Response delay in milliseconds
    response_delay_ms: u64,
    /// Flag to simulate connection issues
    should_respond: bool,
}

impl MockModbusRtuServer {
    /// Create a new mock RTU server
    pub fn new(slave_id: u8) -> Self {
        Self {
            slave_id,
            coils: HashMap::new(),
            discrete_inputs: HashMap::new(),
            holding_registers: HashMap::new(),
            input_registers: HashMap::new(),
            response_delay_ms: 0,
            should_respond: true,
        }
    }
    
    /// Set response delay for simulating slow devices
    pub fn set_response_delay(&mut self, delay_ms: u64) {
        self.response_delay_ms = delay_ms;
    }
    
    /// Enable or disable responses (simulate device disconnection)
    pub fn set_responding(&mut self, should_respond: bool) {
        self.should_respond = should_respond;
    }
    
    /// Set coil value
    pub fn set_coil(&mut self, address: u16, value: bool) {
        self.coils.insert(address, value);
    }
    
    /// Set discrete input value
    pub fn set_discrete_input(&mut self, address: u16, value: bool) {
        self.discrete_inputs.insert(address, value);
    }
    
    /// Set holding register value
    pub fn set_holding_register(&mut self, address: u16, value: u16) {
        self.holding_registers.insert(address, value);
    }
    
    /// Set input register value
    pub fn set_input_register(&mut self, address: u16, value: u16) {
        self.input_registers.insert(address, value);
    }
    
    /// Initialize server with test data
    pub fn initialize_test_data(&mut self) {
        // Initialize some test coils
        for i in 0..20 {
            self.set_coil(i, i % 2 == 0);
            self.set_discrete_input(i + 100, i % 3 == 0);
        }
        
        // Initialize some test registers
        for i in 0..50 {
            self.set_holding_register(i, 1000 + i);
            self.set_input_register(i + 200, 2000 + i);
        }
    }
    
    /// Process a Modbus RTU request and return response frame
    pub fn process_request(&self, request_frame: &[u8]) -> Option<Vec<u8>> {
        if !self.should_respond {
            return None;
        }
        
        if request_frame.len() < 4 {
            return None;
        }
        
        // Verify CRC
        let data_len = request_frame.len() - 2;
        let data = &request_frame[..data_len];
        let frame_crc = u16::from_le_bytes([request_frame[data_len], request_frame[data_len + 1]]);
        let calculated_crc = crc16_modbus(data);
        
        if frame_crc != calculated_crc {
            return None; // CRC error - no response
        }
        
        let slave_id = request_frame[0];
        if slave_id != self.slave_id {
            return None; // Wrong slave ID - no response
        }
        
        let function_code = request_frame[1];
        
        match function_code {
            0x01 => self.handle_read_coils(&request_frame[2..data_len]),
            0x02 => self.handle_read_discrete_inputs(&request_frame[2..data_len]),
            0x03 => self.handle_read_holding_registers(&request_frame[2..data_len]),
            0x04 => self.handle_read_input_registers(&request_frame[2..data_len]),
            0x05 => self.handle_write_single_coil(&request_frame[2..data_len]),
            0x06 => self.handle_write_single_register(&request_frame[2..data_len]),
            0x0F => self.handle_write_multiple_coils(&request_frame[2..data_len]),
            0x10 => self.handle_write_multiple_registers(&request_frame[2..data_len]),
            _ => self.create_exception_response(function_code, 0x01), // Illegal function
        }
    }
    
    /// Handle read coils request (function 0x01)
    fn handle_read_coils(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 {
            return self.create_exception_response(0x01, 0x03); // Illegal data value
        }
        
        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        
        if quantity == 0 || quantity > 2000 {
            return self.create_exception_response(0x01, 0x03); // Illegal data value
        }
        
        let byte_count = (quantity + 7) / 8;
        let mut response_data = vec![self.slave_id, 0x01, byte_count as u8];
        
        for byte_idx in 0..byte_count {
            let mut byte_value = 0u8;
            for bit_idx in 0..8 {
                let coil_address = start_address + (byte_idx * 8) + bit_idx;
                if coil_address < start_address + quantity {
                    if *self.coils.get(&coil_address).unwrap_or(&false) {
                        byte_value |= 1 << bit_idx;
                    }
                }
            }
            response_data.push(byte_value);
        }
        
        self.add_crc(response_data)
    }
    
    /// Handle read discrete inputs request (function 0x02)
    fn handle_read_discrete_inputs(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 {
            return self.create_exception_response(0x02, 0x03);
        }
        
        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        
        if quantity == 0 || quantity > 2000 {
            return self.create_exception_response(0x02, 0x03);
        }
        
        let byte_count = (quantity + 7) / 8;
        let mut response_data = vec![self.slave_id, 0x02, byte_count as u8];
        
        for byte_idx in 0..byte_count {
            let mut byte_value = 0u8;
            for bit_idx in 0..8 {
                let input_address = start_address + (byte_idx * 8) + bit_idx;
                if input_address < start_address + quantity {
                    if *self.discrete_inputs.get(&input_address).unwrap_or(&false) {
                        byte_value |= 1 << bit_idx;
                    }
                }
            }
            response_data.push(byte_value);
        }
        
        self.add_crc(response_data)
    }
    
    /// Handle read holding registers request (function 0x03)
    fn handle_read_holding_registers(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 {
            return self.create_exception_response(0x03, 0x03);
        }
        
        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        
        if quantity == 0 || quantity > 125 {
            return self.create_exception_response(0x03, 0x03);
        }
        
        let byte_count = quantity * 2;
        let mut response_data = vec![self.slave_id, 0x03, byte_count as u8];
        
        for i in 0..quantity {
            let register_address = start_address + i;
            let register_value = self.holding_registers.get(&register_address).unwrap_or(&0);
            let bytes = register_value.to_be_bytes();
            response_data.push(bytes[0]);
            response_data.push(bytes[1]);
        }
        
        self.add_crc(response_data)
    }
    
    /// Handle read input registers request (function 0x04)
    fn handle_read_input_registers(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 {
            return self.create_exception_response(0x04, 0x03);
        }
        
        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        
        if quantity == 0 || quantity > 125 {
            return self.create_exception_response(0x04, 0x03);
        }
        
        let byte_count = quantity * 2;
        let mut response_data = vec![self.slave_id, 0x04, byte_count as u8];
        
        for i in 0..quantity {
            let register_address = start_address + i;
            let register_value = self.input_registers.get(&register_address).unwrap_or(&0);
            let bytes = register_value.to_be_bytes();
            response_data.push(bytes[0]);
            response_data.push(bytes[1]);
        }
        
        self.add_crc(response_data)
    }
    
    /// Handle write single coil request (function 0x05)
    fn handle_write_single_coil(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 {
            return self.create_exception_response(0x05, 0x03);
        }
        
        let address = u16::from_be_bytes([data[0], data[1]]);
        let value_raw = u16::from_be_bytes([data[2], data[3]]);
        
        if value_raw != 0x0000 && value_raw != 0xFF00 {
            return self.create_exception_response(0x05, 0x03);
        }
        
        // Echo back the request as response for write single coil
        let mut response_data = vec![self.slave_id, 0x05];
        response_data.extend_from_slice(data);
        
        self.add_crc(response_data)
    }
    
    /// Handle write single register request (function 0x06)
    fn handle_write_single_register(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 {
            return self.create_exception_response(0x06, 0x03);
        }
        
        // Echo back the request as response for write single register
        let mut response_data = vec![self.slave_id, 0x06];
        response_data.extend_from_slice(data);
        
        self.add_crc(response_data)
    }
    
    /// Handle write multiple coils request (function 0x0F)
    fn handle_write_multiple_coils(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 5 {
            return self.create_exception_response(0x0F, 0x03);
        }
        
        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        let byte_count = data[4] as usize;
        
        if data.len() < 5 + byte_count {
            return self.create_exception_response(0x0F, 0x03);
        }
        
        // Return confirmation response
        let mut response_data = vec![self.slave_id, 0x0F];
        response_data.extend_from_slice(&data[0..4]); // Address and quantity
        
        self.add_crc(response_data)
    }
    
    /// Handle write multiple registers request (function 0x10)
    fn handle_write_multiple_registers(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 5 {
            return self.create_exception_response(0x10, 0x03);
        }
        
        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        let byte_count = data[4] as usize;
        
        if data.len() < 5 + byte_count || byte_count != (quantity * 2) as usize {
            return self.create_exception_response(0x10, 0x03);
        }
        
        // Return confirmation response
        let mut response_data = vec![self.slave_id, 0x10];
        response_data.extend_from_slice(&data[0..4]); // Address and quantity
        
        self.add_crc(response_data)
    }
    
    /// Create exception response
    fn create_exception_response(&self, function_code: u8, exception_code: u8) -> Option<Vec<u8>> {
        let response_data = vec![self.slave_id, function_code | 0x80, exception_code];
        self.add_crc(response_data)
    }
    
    /// Add CRC to response data
    fn add_crc(&self, mut data: Vec<u8>) -> Option<Vec<u8>> {
        let crc = crc16_modbus(&data);
        data.push((crc & 0xFF) as u8);
        data.push((crc >> 8) as u8);
        Some(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mock_server_creation() {
        let server = MockModbusRtuServer::new(1);
        assert_eq!(server.slave_id, 1);
        assert!(server.should_respond);
    }
    
    #[test]
    fn test_set_data() {
        let mut server = MockModbusRtuServer::new(1);
        
        server.set_coil(10, true);
        server.set_holding_register(100, 1234);
        
        assert_eq!(server.coils.get(&10), Some(&true));
        assert_eq!(server.holding_registers.get(&100), Some(&1234));
    }
    
    #[test]
    fn test_initialize_test_data() {
        let mut server = MockModbusRtuServer::new(1);
        server.initialize_test_data();
        
        // Check some initialized data
        assert!(server.coils.len() > 0);
        assert!(server.holding_registers.len() > 0);
        assert_eq!(server.coils.get(&0), Some(&true)); // 0 % 2 == 0
        assert_eq!(server.coils.get(&1), Some(&false)); // 1 % 2 != 0
    }
    
    #[test]
    fn test_read_holding_registers_request() {
        let mut server = MockModbusRtuServer::new(1);
        server.set_holding_register(0, 0x1234);
        server.set_holding_register(1, 0x5678);
        
        // Create read holding registers request: slave=1, function=3, address=0, quantity=2
        let request_data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x02];
        let crc = crc16_modbus(&request_data);
        let mut request = request_data;
        request.push((crc & 0xFF) as u8);
        request.push((crc >> 8) as u8);
        
        let response = server.process_request(&request);
        assert!(response.is_some());
        
        let response = response.unwrap();
        // Response should be: [Slave][Function][ByteCount][Data...][CRC]
        assert_eq!(response[0], 1);     // Slave ID
        assert_eq!(response[1], 0x03);  // Function code
        assert_eq!(response[2], 4);     // Byte count (2 registers * 2 bytes)
        assert_eq!(response[3], 0x12);  // Register 0 high byte
        assert_eq!(response[4], 0x34);  // Register 0 low byte
        assert_eq!(response[5], 0x56);  // Register 1 high byte
        assert_eq!(response[6], 0x78);  // Register 1 low byte
    }
    
    #[test]
    fn test_invalid_crc_no_response() {
        let server = MockModbusRtuServer::new(1);
        
        // Request with invalid CRC
        let request = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x02, 0xFF, 0xFF];
        let response = server.process_request(&request);
        
        assert!(response.is_none()); // Should not respond to invalid CRC
    }
    
    #[test]
    fn test_wrong_slave_id_no_response() {
        let server = MockModbusRtuServer::new(1);
        
        // Request for slave ID 2 (server is configured for slave ID 1)
        let request_data = vec![0x02, 0x03, 0x00, 0x00, 0x00, 0x02];
        let crc = crc16_modbus(&request_data);
        let mut request = request_data;
        request.push((crc & 0xFF) as u8);
        request.push((crc >> 8) as u8);
        
        let response = server.process_request(&request);
        assert!(response.is_none()); // Should not respond to wrong slave ID
    }
    
    #[test]
    fn test_exception_response() {
        let server = MockModbusRtuServer::new(1);
        
        // Request with illegal function code 99
        let request_data = vec![0x01, 99, 0x00, 0x00, 0x00, 0x02];
        let crc = crc16_modbus(&request_data);
        let mut request = request_data;
        request.push((crc & 0xFF) as u8);
        request.push((crc >> 8) as u8);
        
        let response = server.process_request(&request);
        assert!(response.is_some());
        
        let response = response.unwrap();
        assert_eq!(response[0], 1);           // Slave ID
        assert_eq!(response[1], 99 | 0x80);   // Function code with error bit
        assert_eq!(response[2], 0x01);        // Exception code: illegal function
    }
    
    #[test]
    fn test_server_not_responding() {
        let mut server = MockModbusRtuServer::new(1);
        server.set_responding(false);
        
        let request_data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x02];
        let crc = crc16_modbus(&request_data);
        let mut request = request_data;
        request.push((crc & 0xFF) as u8);
        request.push((crc >> 8) as u8);
        
        let response = server.process_request(&request);
        assert!(response.is_none()); // Should not respond when disabled
    }
} 