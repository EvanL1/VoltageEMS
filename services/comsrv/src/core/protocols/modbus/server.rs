//! Modbus Server Implementation
//!
//! This module provides a basic Modbus server implementation that can simulate
//! Modbus devices and respond to client requests.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::core::protocols::common::combase::transport_bridge::UniversalTransportBridge;
use crate::core::protocols::modbus::{
    common::{ModbusConfig, ModbusFunctionCode},
    frame::{ModbusFrameProcessor, ModbusMode},
    pdu::{ModbusPduProcessor, PduParseResult},
};
use crate::core::transport::traits::Transport;
use crate::utils::error::{ComSrvError, Result};

/// Modbus server device simulation
pub struct ModbusDevice {
    /// Unit ID for this device
    unit_id: u8,
    /// Coils (read/write bits) - address space 0x0000-0xFFFF
    coils: HashMap<u16, bool>,
    /// Discrete inputs (read-only bits) - address space 0x0000-0xFFFF
    discrete_inputs: HashMap<u16, bool>,
    /// Holding registers (read/write words) - address space 0x0000-0xFFFF
    holding_registers: HashMap<u16, u16>,
    /// Input registers (read-only words) - address space 0x0000-0xFFFF
    input_registers: HashMap<u16, u16>,
}

impl ModbusDevice {
    /// Create new Modbus device
    pub fn new(unit_id: u8) -> Self {
        Self {
            unit_id,
            coils: HashMap::new(),
            discrete_inputs: HashMap::new(),
            holding_registers: HashMap::new(),
            input_registers: HashMap::new(),
        }
    }

    /// Initialize device with some default values for testing
    pub fn init_test_data(&mut self) {
        // Set some coils
        for i in 0..16 {
            self.coils.insert(i, i % 2 == 0);
        }

        // Set some discrete inputs
        for i in 0..16 {
            self.discrete_inputs.insert(i, i % 3 == 0);
        }

        // Set some holding registers
        for i in 0..16 {
            self.holding_registers.insert(i, (i * 100) as u16);
        }

        // Set some input registers
        for i in 0..16 {
            self.input_registers.insert(i, (i * 200 + 1000) as u16);
        }
    }

    /// Read coils
    pub fn read_coils(&self, start_address: u16, quantity: u16) -> Result<Vec<bool>> {
        let mut values = Vec::new();
        for i in 0..quantity {
            let address = start_address + i;
            values.push(self.coils.get(&address).copied().unwrap_or(false));
        }
        Ok(values)
    }

    /// Read discrete inputs
    pub fn read_discrete_inputs(&self, start_address: u16, quantity: u16) -> Result<Vec<bool>> {
        let mut values = Vec::new();
        for i in 0..quantity {
            let address = start_address + i;
            values.push(self.discrete_inputs.get(&address).copied().unwrap_or(false));
        }
        Ok(values)
    }

    /// Read holding registers
    pub fn read_holding_registers(&self, start_address: u16, quantity: u16) -> Result<Vec<u16>> {
        let mut values = Vec::new();
        for i in 0..quantity {
            let address = start_address + i;
            values.push(self.holding_registers.get(&address).copied().unwrap_or(0));
        }
        Ok(values)
    }

    /// Read input registers
    pub fn read_input_registers(&self, start_address: u16, quantity: u16) -> Result<Vec<u16>> {
        let mut values = Vec::new();
        for i in 0..quantity {
            let address = start_address + i;
            values.push(self.input_registers.get(&address).copied().unwrap_or(0));
        }
        Ok(values)
    }

    /// Write single coil
    pub fn write_single_coil(&mut self, address: u16, value: bool) -> Result<()> {
        self.coils.insert(address, value);
        Ok(())
    }

    /// Write single register
    pub fn write_single_register(&mut self, address: u16, value: u16) -> Result<()> {
        self.holding_registers.insert(address, value);
        Ok(())
    }

    /// Write multiple coils
    pub fn write_multiple_coils(&mut self, start_address: u16, values: &[bool]) -> Result<()> {
        for (i, &value) in values.iter().enumerate() {
            let address = start_address + i as u16;
            self.coils.insert(address, value);
        }
        Ok(())
    }

    /// Write multiple registers
    pub fn write_multiple_registers(&mut self, start_address: u16, values: &[u16]) -> Result<()> {
        for (i, &value) in values.iter().enumerate() {
            let address = start_address + i as u16;
            self.holding_registers.insert(address, value);
        }
        Ok(())
    }
}

/// Modbus server implementation
pub struct ModbusServer {
    /// Server configuration
    config: ModbusConfig,
    /// PDU processor
    pdu_processor: ModbusPduProcessor,
    /// Frame processor
    frame_processor: Arc<Mutex<ModbusFrameProcessor>>,
    /// Transport bridge
    transport_bridge: Arc<Mutex<UniversalTransportBridge>>,
    /// Simulated devices (unit_id -> device)
    devices: HashMap<u8, ModbusDevice>,
    /// Server running state
    running: bool,
}

impl ModbusServer {
    /// Create new Modbus server
    pub fn new(config: ModbusConfig, transport: Box<dyn Transport>) -> Result<Self> {
        let mode = if config.is_tcp() {
            ModbusMode::Tcp
        } else {
            ModbusMode::Rtu
        };

        let pdu_processor = ModbusPduProcessor::new();
        let frame_processor = Arc::new(Mutex::new(ModbusFrameProcessor::new(mode)));
        let transport_bridge =
            Arc::new(Mutex::new(UniversalTransportBridge::new_modbus(transport)));

        Ok(Self {
            config,
            pdu_processor,
            frame_processor,
            transport_bridge,
            devices: HashMap::new(),
            running: false,
        })
    }

    /// Add a device to the server
    pub fn add_device(&mut self, mut device: ModbusDevice) {
        device.init_test_data(); // Initialize with test data
        let unit_id = device.unit_id; // Save unit_id before moving device
        self.devices.insert(device.unit_id, device);
        info!("Added Modbus device with unit ID {}", unit_id);
    }

    /// Start the server
    pub async fn start(&mut self) -> Result<()> {
        self.running = true;
        info!("Modbus server started");

        // Add default device if none exists
        if self.devices.is_empty() {
            self.add_device(ModbusDevice::new(1));
        }

        // Server main loop would go here in a real implementation
        // For now, this is just a basic structure

        Ok(())
    }

    /// Stop the server
    pub async fn stop(&mut self) -> Result<()> {
        self.running = false;
        info!("Modbus server stopped");
        Ok(())
    }

    /// Process a received request and generate response
    pub async fn process_request(&mut self, request_data: &[u8]) -> Result<Vec<u8>> {
        // Parse frame
        let parsed_frame = self
            .frame_processor
            .lock()
            .unwrap()
            .parse_frame(request_data)?;

        // Parse PDU
        let pdu_result = self.pdu_processor.parse_pdu(&parsed_frame.pdu)?;

        let response_pdu = match pdu_result {
            PduParseResult::Request(request) => {
                // Find target device
                let device = self.devices.get_mut(&parsed_frame.unit_id).ok_or_else(|| {
                    ComSrvError::ProtocolError(format!(
                        "Device not found: {}",
                        parsed_frame.unit_id
                    ))
                })?;

                // Handle request without borrowing self
                Self::handle_request_static(&self.pdu_processor, device, &request).await?
            }
            _ => {
                return Err(ComSrvError::ProtocolError(
                    "Expected request PDU".to_string(),
                ));
            }
        };

        // Build response frame
        let response_frame = self.frame_processor.lock().unwrap().build_frame(
            parsed_frame.unit_id,
            response_pdu,
            parsed_frame.transaction_id,
        );

        Ok(response_frame)
    }

    /// Handle a specific Modbus request (static version to avoid borrowing issues)
    async fn handle_request_static(
        pdu_processor: &ModbusPduProcessor,
        device: &mut ModbusDevice,
        request: &crate::core::protocols::modbus::pdu::ModbusPduRequest,
    ) -> Result<Vec<u8>> {
        match request.function_code {
            ModbusFunctionCode::Read01 => {
                let read_req = pdu_processor.parse_read_request(&request.data)?;
                let values = device.read_coils(read_req.start_address, read_req.quantity)?;
                let data = pdu_processor.build_coil_response_data(&values);
                Ok(pdu_processor.build_read_response(ModbusFunctionCode::Read01, &data))
            }
            ModbusFunctionCode::Read02 => {
                let read_req = pdu_processor.parse_read_request(&request.data)?;
                let values =
                    device.read_discrete_inputs(read_req.start_address, read_req.quantity)?;
                let data = pdu_processor.build_coil_response_data(&values);
                Ok(pdu_processor.build_read_response(ModbusFunctionCode::Read02, &data))
            }
            ModbusFunctionCode::Read03 => {
                let read_req = pdu_processor.parse_read_request(&request.data)?;
                let values =
                    device.read_holding_registers(read_req.start_address, read_req.quantity)?;
                let data = pdu_processor.build_register_response_data(&values);
                Ok(pdu_processor.build_read_response(ModbusFunctionCode::Read03, &data))
            }
            ModbusFunctionCode::Read04 => {
                let read_req = pdu_processor.parse_read_request(&request.data)?;
                let values =
                    device.read_input_registers(read_req.start_address, read_req.quantity)?;
                let data = pdu_processor.build_register_response_data(&values);
                Ok(pdu_processor.build_read_response(ModbusFunctionCode::Read04, &data))
            }
            ModbusFunctionCode::Write05 => {
                let write_req = pdu_processor.parse_write_single_request(&request.data)?;
                let coil_value = write_req.value == 0xFF00;
                device.write_single_coil(write_req.address, coil_value)?;
                Ok(pdu_processor.build_write_single_response(
                    ModbusFunctionCode::Write05,
                    write_req.address,
                    write_req.value,
                ))
            }
            ModbusFunctionCode::Write06 => {
                let write_req = pdu_processor.parse_write_single_request(&request.data)?;
                device.write_single_register(write_req.address, write_req.value)?;
                Ok(pdu_processor.build_write_single_response(
                    ModbusFunctionCode::Write06,
                    write_req.address,
                    write_req.value,
                ))
            }
            ModbusFunctionCode::Write0F => {
                let write_req = pdu_processor.parse_write_multiple_coils_request(&request.data)?;
                device.write_multiple_coils(write_req.start_address, &write_req.values)?;
                Ok(pdu_processor.build_write_multiple_response(
                    ModbusFunctionCode::Write0F,
                    write_req.start_address,
                    write_req.quantity,
                ))
            }
            ModbusFunctionCode::Write10 => {
                let write_req =
                    pdu_processor.parse_write_multiple_registers_request(&request.data)?;
                device.write_multiple_registers(write_req.start_address, &write_req.values)?;
                Ok(pdu_processor.build_write_multiple_response(
                    ModbusFunctionCode::Write10,
                    write_req.start_address,
                    write_req.quantity,
                ))
            }
            ModbusFunctionCode::Custom(code) => Err(ComSrvError::ProtocolError(format!(
                "Unsupported function code: 0x{:02X}",
                code
            ))),
        }
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transport::mock::MockTransport;

    fn create_test_config() -> ModbusConfig {
        ModbusConfig {
            protocol_type: "modbus_tcp".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(502),
            device_path: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout_ms: Some(5000),
            points: vec![],
        }
    }

    #[test]
    fn test_modbus_device_creation() {
        let mut device = ModbusDevice::new(1);
        device.init_test_data();

        assert_eq!(device.unit_id, 1);
        assert!(device.coils.len() > 0);
        assert!(device.holding_registers.len() > 0);
    }

    #[test]
    fn test_device_read_operations() {
        let mut device = ModbusDevice::new(1);
        device.init_test_data();

        // Test reading coils
        let coils = device.read_coils(0, 8).unwrap();
        assert_eq!(coils.len(), 8);

        // Test reading holding registers
        let registers = device.read_holding_registers(0, 4).unwrap();
        assert_eq!(registers.len(), 4);
        assert_eq!(registers[0], 0); // 0 * 100
        assert_eq!(registers[1], 100); // 1 * 100
    }

    #[test]
    fn test_device_write_operations() {
        let mut device = ModbusDevice::new(1);

        // Test writing single coil
        device.write_single_coil(10, true).unwrap();
        let coils = device.read_coils(10, 1).unwrap();
        assert_eq!(coils[0], true);

        // Test writing single register
        device.write_single_register(10, 12345).unwrap();
        let registers = device.read_holding_registers(10, 1).unwrap();
        assert_eq!(registers[0], 12345);
    }

    #[tokio::test]
    async fn test_modbus_server_creation() {
        let config = create_test_config();
        let mock_config = crate::core::transport::mock::MockTransportConfig::default();
        let transport = Box::new(MockTransport::new(mock_config).unwrap());

        let mut server = ModbusServer::new(config, transport).unwrap();
        server.add_device(ModbusDevice::new(1));

        assert_eq!(server.device_count(), 1);
        assert!(!server.is_running());
    }
}
