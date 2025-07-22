//! Unified Modbus Client Implementation
//!
//! This module provides a unified implementation of the ModbusClient trait
//! that works with both TCP and RTU transports, maximizing PDU code reuse.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::core::transport::Transport;
use crate::utils::error::{ComSrvError, Result};

use super::client_trait::{
    ExtendedModbusClient, ModbusClient, ModbusDataOperations, StringEncoding,
};
use super::common::ByteOrder;
use super::frame::{ModbusFrameProcessor, ModbusMode};
use super::pdu::{ModbusPduProcessor, PduParseResult};

/// Unified Modbus client implementation that abstracts TCP/RTU differences
///
/// This implementation uses the same PDU processing logic for both TCP and RTU,
/// with only the framing layer being different between the two protocols.
pub struct ModbusClientImpl<T: Transport> {
    /// Transport layer (TCP or Serial)
    transport: Arc<RwLock<T>>,
    /// PDU processor (shared between TCP/RTU)
    pdu_processor: Arc<ModbusPduProcessor>,
    /// Frame processor (handles TCP/RTU differences)
    frame_processor: Arc<RwLock<ModbusFrameProcessor>>,
    /// Protocol mode (TCP or RTU)
    mode: ModbusMode,
    /// Request timeout
    timeout: Duration,
    /// Connection state
    connected: Arc<RwLock<bool>>,
    /// Transaction ID counter (for TCP)
    transaction_id: Arc<RwLock<u16>>,
    /// Client statistics
    stats: Arc<RwLock<ClientStatistics>>,
}

/// Client statistics for monitoring and diagnostics
#[derive(Debug, Default)]
pub struct ClientStatistics {
    /// Total requests sent
    pub total_requests: u64,
    /// Total successful responses
    pub successful_responses: u64,
    /// Total failed requests
    pub failed_requests: u64,
    /// Total timeout errors
    pub timeout_errors: u64,
    /// Total exception responses
    pub exception_responses: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Last request time
    pub last_request_time: Option<Instant>,
    /// Last response time
    pub last_response_time: Option<Instant>,
}

impl<T: Transport> ModbusClientImpl<T> {
    /// Create a new Modbus client implementation
    ///
    /// # Arguments
    /// * `transport` - Transport layer (TCP or Serial)
    /// * `mode` - Protocol mode (TCP or RTU)
    /// * `timeout` - Default request timeout
    ///
    /// # Returns
    /// * `Self` - New client instance
    pub fn new(transport: T, mode: ModbusMode, timeout: Duration) -> Self {
        let frame_processor = match mode {
            ModbusMode::Tcp => ModbusFrameProcessor::new(ModbusMode::Tcp),
            ModbusMode::Rtu => ModbusFrameProcessor::new(ModbusMode::Rtu),
        };

        Self {
            transport: Arc::new(RwLock::new(transport)),
            pdu_processor: Arc::new(ModbusPduProcessor::new()),
            frame_processor: Arc::new(RwLock::new(frame_processor)),
            mode,
            timeout,
            connected: Arc::new(RwLock::new(false)),
            transaction_id: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(ClientStatistics::default())),
        }
    }

    /// Send a request and receive response
    async fn send_request(&mut self, slave_id: u8, pdu: Vec<u8>) -> Result<Vec<u8>> {
        let start_time = Instant::now();

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
            stats.last_request_time = Some(start_time);
        }

        // Get next transaction ID for TCP
        let transaction_id = if self.mode == ModbusMode::Tcp {
            let mut tx_id = self.transaction_id.write().await;
            *tx_id = tx_id.wrapping_add(1);
            Some(*tx_id)
        } else {
            None
        };

        // Build frame
        let frame_data = {
            let frame_processor = self.frame_processor.read().await;
            frame_processor.build_frame(slave_id, pdu, transaction_id)
        };

        debug!(
            "[ModbusClient] Sending {} bytes to slave {}: {:02X?}",
            frame_data.len(),
            slave_id,
            frame_data
        );

        // Send frame
        {
            let mut transport = self.transport.write().await;
            transport.send(&frame_data).await.map_err(|e| {
                error!("[ModbusClient] Failed to send frame: {}", e);
                ComSrvError::connection(format!("Send failed: {}", e))
            })?;
        }

        // Receive response with timeout
        let response_data = {
            let mut transport = self.transport.write().await;
            let mut buffer = vec![0u8; 2048]; // 足够大的缓冲区
            let bytes_received = tokio::time::timeout(
                self.timeout,
                transport.receive(&mut buffer, Some(self.timeout)),
            )
            .await
            .map_err(|_| {
                warn!("[ModbusClient] Request timeout after {:?}", self.timeout);
                ComSrvError::timeout(format!("Request timeout after {:?}", self.timeout))
            })?
            .map_err(|e| {
                error!("[ModbusClient] Failed to receive response: {}", e);
                ComSrvError::connection(format!("Receive failed: {}", e))
            })?;
            buffer.truncate(bytes_received);
            buffer
        };

        debug!(
            "[ModbusClient] Received {} bytes: {:02X?}",
            response_data.len(),
            response_data
        );

        // Parse frame
        let parsed_frame = {
            let mut frame_processor = self.frame_processor.write().await;
            frame_processor.parse_frame(&response_data).map_err(|e| {
                error!("[ModbusClient] Failed to parse frame: {}", e);
                e
            })?
        };

        // Validate slave ID
        if parsed_frame.unit_id != slave_id {
            warn!(
                "[ModbusClient] Slave ID mismatch: expected {}, got {}",
                slave_id, parsed_frame.unit_id
            );
            return Err(ComSrvError::ProtocolError(format!(
                "Slave ID mismatch: expected {}, got {}",
                slave_id, parsed_frame.unit_id
            )));
        }

        // Parse PDU
        let pdu_result = self.pdu_processor.parse_response_pdu(&parsed_frame.pdu)?;

        // Update statistics
        let response_time = start_time.elapsed();
        {
            let mut stats = self.stats.write().await;
            stats.last_response_time = Some(Instant::now());

            match &pdu_result {
                PduParseResult::Response(_) => {
                    stats.successful_responses += 1;
                }
                PduParseResult::Exception(_) => {
                    stats.exception_responses += 1;
                }
                _ => {
                    stats.failed_requests += 1;
                }
            }

            // Update average response time
            let total_successful = stats.successful_responses + stats.exception_responses;
            if total_successful > 0 {
                let current_avg = stats.avg_response_time_ms;
                let new_time = response_time.as_millis() as f64;
                stats.avg_response_time_ms = (current_avg * (total_successful - 1) as f64
                    + new_time)
                    / total_successful as f64;
            }
        }

        // Handle response
        match pdu_result {
            PduParseResult::Response(response) => {
                debug!("[ModbusClient] Received valid response");
                Ok(response.data)
            }
            PduParseResult::Exception(exception) => {
                warn!(
                    "[ModbusClient] Received exception response: {:?}",
                    exception.exception_code
                );
                Err(ComSrvError::ProtocolError(format!(
                    "Modbus exception: {:?}",
                    exception.exception_code
                )))
            }
            PduParseResult::Request(_) => {
                error!("[ModbusClient] Unexpected request PDU in response");
                Err(ComSrvError::ProtocolError(
                    "Unexpected request PDU in response".to_string(),
                ))
            }
        }
    }

    /// Parse register response data
    fn parse_register_response(&self, data: &[u8]) -> Result<Vec<u16>> {
        if data.is_empty() {
            return Err(ComSrvError::ProtocolError(
                "Empty register response data".to_string(),
            ));
        }

        let byte_count = data[0] as usize;
        if data.len() < 1 + byte_count {
            return Err(ComSrvError::ProtocolError(
                "Invalid register response length".to_string(),
            ));
        }

        let register_data = &data[1..1 + byte_count];
        if register_data.len() % 2 != 0 {
            return Err(ComSrvError::ProtocolError(
                "Invalid register data length (not multiple of 2)".to_string(),
            ));
        }

        let mut registers = Vec::new();
        for chunk in register_data.chunks_exact(2) {
            let value = u16::from_be_bytes([chunk[0], chunk[1]]);
            registers.push(value);
        }

        Ok(registers)
    }

    /// Parse coil response data
    fn parse_coil_response(&self, data: &[u8], expected_count: u16) -> Result<Vec<bool>> {
        if data.is_empty() {
            return Err(ComSrvError::ProtocolError(
                "Empty coil response data".to_string(),
            ));
        }

        let byte_count = data[0] as usize;
        if data.len() < 1 + byte_count {
            return Err(ComSrvError::ProtocolError(
                "Invalid coil response length".to_string(),
            ));
        }

        let coil_data = &data[1..1 + byte_count];
        let mut coils = Vec::new();

        for (_byte_idx, &byte) in coil_data.iter().enumerate() {
            for bit_idx in 0..8 {
                if coils.len() >= expected_count as usize {
                    break;
                }
                let bit_value = (byte >> bit_idx) & 1 != 0;
                coils.push(bit_value);
            }
        }

        // Trim to expected count
        coils.truncate(expected_count as usize);
        Ok(coils)
    }
}

#[async_trait]
impl<T: Transport> ModbusClient for ModbusClientImpl<T> {
    async fn read_holding_registers(
        &mut self,
        slave_id: u8,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<u16>> {
        let pdu = self.pdu_processor.build_read_request(
            super::common::ModbusFunctionCode::Read03,
            start_address,
            count,
        );

        let response_data = self.send_request(slave_id, pdu).await?;
        self.parse_register_response(&response_data)
    }

    async fn read_input_registers(
        &mut self,
        slave_id: u8,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<u16>> {
        let pdu = self.pdu_processor.build_read_request(
            super::common::ModbusFunctionCode::Read04,
            start_address,
            count,
        );

        let response_data = self.send_request(slave_id, pdu).await?;
        self.parse_register_response(&response_data)
    }

    async fn read_coils(
        &mut self,
        slave_id: u8,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<bool>> {
        let pdu = self.pdu_processor.build_read_request(
            super::common::ModbusFunctionCode::Read01,
            start_address,
            count,
        );

        let response_data = self.send_request(slave_id, pdu).await?;
        self.parse_coil_response(&response_data, count)
    }

    async fn read_discrete_inputs(
        &mut self,
        slave_id: u8,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<bool>> {
        let pdu = self.pdu_processor.build_read_request(
            super::common::ModbusFunctionCode::Read02,
            start_address,
            count,
        );

        let response_data = self.send_request(slave_id, pdu).await?;
        self.parse_coil_response(&response_data, count)
    }

    async fn write_single_coil(&mut self, slave_id: u8, address: u16, value: bool) -> Result<()> {
        let coil_value = if value { 0xFF00 } else { 0x0000 };
        let pdu = self.pdu_processor.build_write_single_request(
            super::common::ModbusFunctionCode::Write05,
            address,
            coil_value,
        );

        let _response_data = self.send_request(slave_id, pdu).await?;
        // For write operations, we just need to verify no exception occurred
        Ok(())
    }

    async fn write_single_register(
        &mut self,
        slave_id: u8,
        address: u16,
        value: u16,
    ) -> Result<()> {
        let pdu = self.pdu_processor.build_write_single_request(
            super::common::ModbusFunctionCode::Write06,
            address,
            value,
        );

        let _response_data = self.send_request(slave_id, pdu).await?;
        Ok(())
    }

    async fn write_multiple_coils(
        &mut self,
        slave_id: u8,
        start_address: u16,
        values: &[bool],
    ) -> Result<()> {
        let pdu = self
            .pdu_processor
            .build_write_multiple_coils_request(start_address, values);

        let _response_data = self.send_request(slave_id, pdu).await?;
        Ok(())
    }

    async fn write_multiple_registers(
        &mut self,
        slave_id: u8,
        start_address: u16,
        values: &[u16],
    ) -> Result<()> {
        let pdu = self
            .pdu_processor
            .build_write_multiple_registers_request(start_address, values);

        let _response_data = self.send_request(slave_id, pdu).await?;
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    async fn connect(&mut self) -> Result<()> {
        {
            let mut transport = self.transport.write().await;
            transport.connect().await.map_err(|e| {
                error!("[ModbusClient] Failed to connect: {}", e);
                ComSrvError::connection(format!("Connection failed: {}", e))
            })?;
        }

        *self.connected.write().await = true;
        info!("[ModbusClient] Connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        {
            let mut transport = self.transport.write().await;
            transport.disconnect().await.map_err(|e| {
                error!("[ModbusClient] Failed to disconnect: {}", e);
                ComSrvError::connection(format!("Disconnect failed: {}", e))
            })?;
        }

        *self.connected.write().await = false;
        info!("[ModbusClient] Disconnected successfully");
        Ok(())
    }

    async fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
        debug!("[ModbusClient] Timeout set to {:?}", timeout);
    }

    fn get_timeout(&self) -> Duration {
        self.timeout
    }

    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diagnostics = HashMap::new();
        let stats = self.stats.read().await;

        diagnostics.insert("protocol_mode".to_string(), format!("{:?}", self.mode));
        diagnostics.insert(
            "connected".to_string(),
            self.is_connected().await.to_string(),
        );
        diagnostics.insert(
            "timeout_ms".to_string(),
            self.timeout.as_millis().to_string(),
        );
        diagnostics.insert(
            "total_requests".to_string(),
            stats.total_requests.to_string(),
        );
        diagnostics.insert(
            "successful_responses".to_string(),
            stats.successful_responses.to_string(),
        );
        diagnostics.insert(
            "failed_requests".to_string(),
            stats.failed_requests.to_string(),
        );
        diagnostics.insert(
            "timeout_errors".to_string(),
            stats.timeout_errors.to_string(),
        );
        diagnostics.insert(
            "exception_responses".to_string(),
            stats.exception_responses.to_string(),
        );
        diagnostics.insert(
            "avg_response_time_ms".to_string(),
            format!("{:.2}", stats.avg_response_time_ms),
        );

        if let Some(last_request) = stats.last_request_time {
            diagnostics.insert(
                "last_request_ago_ms".to_string(),
                last_request.elapsed().as_millis().to_string(),
            );
        }

        if let Some(last_response) = stats.last_response_time {
            diagnostics.insert(
                "last_response_ago_ms".to_string(),
                last_response.elapsed().as_millis().to_string(),
            );
        }

        // Add success rate
        let total_responses =
            stats.successful_responses + stats.exception_responses + stats.failed_requests;
        if total_responses > 0 {
            let success_rate = (stats.successful_responses as f64 / total_responses as f64) * 100.0;
            diagnostics.insert(
                "success_rate_percent".to_string(),
                format!("{:.2}", success_rate),
            );
        }

        diagnostics
    }

    async fn read_multiple_register_ranges(
        &mut self,
        slave_id: u8,
        ranges: &[(u16, u16)],
    ) -> Result<Vec<Vec<u16>>> {
        let mut results = Vec::new();

        for &(start_address, count) in ranges {
            match self
                .read_holding_registers(slave_id, start_address, count)
                .await
            {
                Ok(registers) => results.push(registers),
                Err(e) => {
                    warn!(
                        "[ModbusClient] Failed to read range {}-{}: {}",
                        start_address,
                        start_address + count - 1,
                        e
                    );
                    return Err(e);
                }
            }
        }

        Ok(results)
    }

    async fn read_multiple_coil_ranges(
        &mut self,
        slave_id: u8,
        ranges: &[(u16, u16)],
    ) -> Result<Vec<Vec<bool>>> {
        let mut results = Vec::new();

        for &(start_address, count) in ranges {
            match self.read_coils(slave_id, start_address, count).await {
                Ok(coils) => results.push(coils),
                Err(e) => {
                    warn!(
                        "[ModbusClient] Failed to read coil range {}-{}: {}",
                        start_address,
                        start_address + count - 1,
                        e
                    );
                    return Err(e);
                }
            }
        }

        Ok(results)
    }
}

#[async_trait]
impl<T: Transport> ExtendedModbusClient for ModbusClientImpl<T> {
    async fn read_device_identification(&mut self, slave_id: u8, object_id: u8) -> Result<String> {
        // Function code 0x2B/0x0E for device identification
        let pdu = vec![0x2B, 0x0E, 0x01, object_id];

        let response_data = self.send_request(slave_id, pdu).await?;

        // Parse device identification response (simplified)
        if response_data.len() > 6 {
            let object_length = response_data[6] as usize;
            if response_data.len() >= 7 + object_length {
                let object_data = &response_data[7..7 + object_length];
                return Ok(String::from_utf8_lossy(object_data).to_string());
            }
        }

        Err(ComSrvError::ProtocolError(
            "Invalid device identification response".to_string(),
        ))
    }

    async fn read_exception_status(&mut self, slave_id: u8) -> Result<u8> {
        let pdu = vec![0x07]; // Function code 0x07
        let response_data = self.send_request(slave_id, pdu).await?;

        if response_data.len() >= 1 {
            Ok(response_data[0])
        } else {
            Err(ComSrvError::ProtocolError(
                "Invalid exception status response".to_string(),
            ))
        }
    }

    async fn diagnostics(
        &mut self,
        slave_id: u8,
        sub_function: u16,
        data: &[u8],
    ) -> Result<Vec<u8>> {
        let mut pdu = vec![0x08]; // Function code 0x08
        pdu.extend_from_slice(&sub_function.to_be_bytes());
        pdu.extend_from_slice(data);

        let response_data = self.send_request(slave_id, pdu).await?;
        Ok(response_data)
    }

    async fn get_comm_event_counter(&mut self, slave_id: u8) -> Result<u16> {
        let pdu = vec![0x0B]; // Function code 0x0B
        let response_data = self.send_request(slave_id, pdu).await?;

        if response_data.len() >= 4 {
            let counter = u16::from_be_bytes([response_data[2], response_data[3]]);
            Ok(counter)
        } else {
            Err(ComSrvError::ProtocolError(
                "Invalid comm event counter response".to_string(),
            ))
        }
    }

    async fn get_comm_event_log(&mut self, slave_id: u8) -> Result<Vec<u8>> {
        let pdu = vec![0x0C]; // Function code 0x0C
        let response_data = self.send_request(slave_id, pdu).await?;
        Ok(response_data)
    }
}

#[async_trait]
impl<T: Transport> ModbusDataOperations for ModbusClientImpl<T> {
    async fn read_float32(
        &mut self,
        slave_id: u8,
        address: u16,
        byte_order: ByteOrder,
    ) -> Result<f32> {
        let registers = self.read_holding_registers(slave_id, address, 2).await?;
        if registers.len() < 2 {
            return Err(ComSrvError::ProtocolError(
                "Insufficient registers for float32".to_string(),
            ));
        }

        let bytes = match byte_order {
            ByteOrder::ABCD => {
                let mut bytes = [0u8; 4];
                bytes[0..2].copy_from_slice(&registers[0].to_be_bytes());
                bytes[2..4].copy_from_slice(&registers[1].to_be_bytes());
                bytes
            }
            ByteOrder::CDAB => {
                let mut bytes = [0u8; 4];
                bytes[0..2].copy_from_slice(&registers[1].to_be_bytes());
                bytes[2..4].copy_from_slice(&registers[0].to_be_bytes());
                bytes
            }
            ByteOrder::BADC => {
                let mut bytes = [0u8; 4];
                let reg0_bytes = registers[0].to_le_bytes();
                let reg1_bytes = registers[1].to_le_bytes();
                bytes[0] = reg0_bytes[0];
                bytes[1] = reg0_bytes[1];
                bytes[2] = reg1_bytes[0];
                bytes[3] = reg1_bytes[1];
                bytes
            }
            ByteOrder::DCBA => {
                let mut bytes = [0u8; 4];
                bytes[0..2].copy_from_slice(&registers[1].to_le_bytes());
                bytes[2..4].copy_from_slice(&registers[0].to_le_bytes());
                bytes
            }
            _ => {
                return Err(ComSrvError::ProtocolError(
                    "Invalid byte order for float32".to_string(),
                ))
            }
        };

        Ok(f32::from_be_bytes(bytes))
    }

    async fn write_float32(
        &mut self,
        slave_id: u8,
        address: u16,
        value: f32,
        byte_order: ByteOrder,
    ) -> Result<()> {
        let bytes = value.to_be_bytes();
        let registers = match byte_order {
            ByteOrder::ABCD => {
                vec![
                    u16::from_be_bytes([bytes[0], bytes[1]]),
                    u16::from_be_bytes([bytes[2], bytes[3]]),
                ]
            }
            ByteOrder::CDAB => {
                vec![
                    u16::from_be_bytes([bytes[2], bytes[3]]),
                    u16::from_be_bytes([bytes[0], bytes[1]]),
                ]
            }
            _ => {
                return Err(ComSrvError::ProtocolError(
                    "Unsupported byte order for float32 write".to_string(),
                ))
            }
        };

        self.write_multiple_registers(slave_id, address, &registers)
            .await
    }

    async fn read_float64(
        &mut self,
        slave_id: u8,
        address: u16,
        byte_order: ByteOrder,
    ) -> Result<f64> {
        let registers = self.read_holding_registers(slave_id, address, 4).await?;
        if registers.len() < 4 {
            return Err(ComSrvError::ProtocolError(
                "Insufficient registers for float64".to_string(),
            ));
        }

        // Simplified implementation for ABCDEFGH byte order
        let mut bytes = [0u8; 8];
        for (i, &reg) in registers.iter().enumerate() {
            let reg_bytes = reg.to_be_bytes();
            bytes[i * 2] = reg_bytes[0];
            bytes[i * 2 + 1] = reg_bytes[1];
        }

        Ok(f64::from_be_bytes(bytes))
    }

    async fn write_float64(
        &mut self,
        slave_id: u8,
        address: u16,
        value: f64,
        byte_order: ByteOrder,
    ) -> Result<()> {
        let bytes = value.to_be_bytes();
        let registers = (0..4)
            .map(|i| u16::from_be_bytes([bytes[i * 2], bytes[i * 2 + 1]]))
            .collect::<Vec<_>>();

        self.write_multiple_registers(slave_id, address, &registers)
            .await
    }

    async fn read_int32(
        &mut self,
        slave_id: u8,
        address: u16,
        signed: bool,
        byte_order: ByteOrder,
    ) -> Result<i32> {
        let registers = self.read_holding_registers(slave_id, address, 2).await?;
        if registers.len() < 2 {
            return Err(ComSrvError::ProtocolError(
                "Insufficient registers for int32".to_string(),
            ));
        }

        let value = match byte_order {
            ByteOrder::ABCD => ((registers[0] as u32) << 16) | (registers[1] as u32),
            ByteOrder::CDAB => ((registers[1] as u32) << 16) | (registers[0] as u32),
            _ => {
                return Err(ComSrvError::ProtocolError(
                    "Unsupported byte order for int32".to_string(),
                ))
            }
        };

        if signed {
            Ok(value as i32)
        } else {
            Ok(value as i32) // For compatibility, always return i32
        }
    }

    async fn write_int32(
        &mut self,
        slave_id: u8,
        address: u16,
        value: i32,
        byte_order: ByteOrder,
    ) -> Result<()> {
        let value_u32 = value as u32;
        let registers = match byte_order {
            ByteOrder::ABCD => {
                vec![(value_u32 >> 16) as u16, (value_u32 & 0xFFFF) as u16]
            }
            ByteOrder::CDAB => {
                vec![(value_u32 & 0xFFFF) as u16, (value_u32 >> 16) as u16]
            }
            _ => {
                return Err(ComSrvError::ProtocolError(
                    "Unsupported byte order for int32 write".to_string(),
                ))
            }
        };

        self.write_multiple_registers(slave_id, address, &registers)
            .await
    }

    async fn read_string(
        &mut self,
        slave_id: u8,
        address: u16,
        length: u16,
        encoding: StringEncoding,
    ) -> Result<String> {
        let register_count = (length + 1) / 2; // Round up to nearest register
        let registers = self
            .read_holding_registers(slave_id, address, register_count)
            .await?;

        let mut bytes = Vec::new();
        for &reg in &registers {
            bytes.extend_from_slice(&reg.to_be_bytes());
        }

        // Trim to requested length
        bytes.truncate(length as usize);

        match encoding {
            StringEncoding::Ascii => {
                let string = String::from_utf8_lossy(&bytes);
                Ok(string.trim_end_matches('\0').to_string())
            }
            StringEncoding::Utf8 => String::from_utf8(bytes)
                .map_err(|e| ComSrvError::ProtocolError(format!("Invalid UTF-8 string: {}", e))),
            StringEncoding::Latin1 => {
                let string: String = bytes.iter().map(|&b| b as char).collect();
                Ok(string.trim_end_matches('\0').to_string())
            }
            StringEncoding::Custom(_) => Err(ComSrvError::ProtocolError(
                "Custom encoding not implemented".to_string(),
            )),
        }
    }

    async fn write_string(
        &mut self,
        slave_id: u8,
        address: u16,
        value: &str,
        encoding: StringEncoding,
    ) -> Result<()> {
        let bytes = match encoding {
            StringEncoding::Ascii | StringEncoding::Latin1 => value.as_bytes().to_vec(),
            StringEncoding::Utf8 => value.as_bytes().to_vec(),
            StringEncoding::Custom(_) => {
                return Err(ComSrvError::ProtocolError(
                    "Custom encoding not implemented".to_string(),
                ))
            }
        };

        // Pad to even length
        let mut padded_bytes = bytes;
        if padded_bytes.len() % 2 != 0 {
            padded_bytes.push(0);
        }

        // Convert to registers
        let registers: Vec<u16> = padded_bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();

        self.write_multiple_registers(slave_id, address, &registers)
            .await
    }
}
