use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_modbus::prelude::*;
use log::{debug, error, info, warn, trace};
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use tracing::{field, Span};
use hex;
use std::sync::atomic::{AtomicU16, Ordering};

use crate::core::config::config_manager::ChannelConfig;
use crate::core::protocols::common::{ComBase, ChannelStatus, PointData};
use crate::utils::{ComSrvError, Result};
use super::client::{ModbusClient, ModbusClientBase, ModbusOptimizer, ModbusReadGroupType};
use super::common::{ModbusRegisterMapping, ModbusDataType, ByteOrder};

/// Modbus TCP client implementation
pub struct ModbusTcpClient {
    /// Base Modbus client
    base: ModbusClientBase,
    /// TCP address
    host: String,
    /// TCP port
    port: u16,
    /// Modbus client context
    client: Arc<Mutex<Option<client::Context>>>,
    transaction_id: AtomicU16, // Simple transaction ID counter
    channel_id_str: String,
}

impl ModbusTcpClient {
    /// Create a new Modbus TCP client
    pub fn new(config: ChannelConfig) -> Self {
        // Get parameters from configuration
        let params = &config.parameters;
        let host = params.get("host")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "localhost".to_string());
            
        let port = params.get("port")
            .and_then(|v| v.as_u64())
            .unwrap_or(502) as u16;
            
        let channel_id_str = config.id.to_string();
        
        Self {
            base: ModbusClientBase::new("ModbusTcpClient", config),
            host: host,
            port,
            client: Arc::new(Mutex::new(None)),
            transaction_id: AtomicU16::new(0), // Initialize counter
            channel_id_str,
        }
    }
    
    /// Connect to the Modbus TCP server
    async fn connect(&self) -> Result<()> {
        if self.base.is_connected().await {
            return Ok(());
        }
        
        // Close existing connection
        self.disconnect().await?;
        
        // Build TCP address
        let socket_addr = format!("{}:{}", self.host, self.port).parse()
            .map_err(|e| ComSrvError::ConnectionError(format!("Invalid address: {}", e)))?;
            
        debug!("Connecting to Modbus TCP server at {}", socket_addr);
        
        // Create TCP connection
        let timeout_duration = Duration::from_millis(self.base.timeout_ms());
        let _slave = Slave(self.base.slave_id());
        
        let client_result = timeout(
            timeout_duration,
            tokio_modbus::client::tcp::connect(socket_addr)
        ).await;
        
        match client_result {
            Ok(Ok(client)) => {
                // Connection successful
                let mut c = self.client.lock().await;
                *c = Some(client);
                self.base.set_connected(true).await;
                info!("Connected to Modbus TCP server at {}", socket_addr);
                Ok(())
            },
            Ok(Err(e)) => {
                // Connection error
                let err_msg = format!("Failed to connect to Modbus TCP server: {}", e);
                error!("{}", err_msg);
                self.base.set_error(&err_msg).await;
                Err(ComSrvError::ConnectionError(err_msg))
            },
            Err(_) => {
                // Connection timeout
                let err_msg = format!(
                    "Connection to Modbus TCP server timed out after {} ms",
                    self.base.timeout_ms()
                );
                error!("{}", err_msg);
                self.base.set_error(&err_msg).await;
                Err(ComSrvError::TimeoutError(err_msg))
            }
        }
    }
    
    /// Disconnect from the server
    async fn disconnect(&self) -> Result<()> {
        let mut client = self.client.lock().await;
        if client.is_some() {
            *client = None;
            self.base.set_connected(false).await;
            debug!("Disconnected from Modbus TCP server");
        }
        Ok(())
    }
    
    /// Log Modbus messages
    fn log_modbus_message(&self, direction: &str, data: &[u8]) {
        let hex_str = hex::encode(data);
        debug!("Modbus TCP {} [{}]: {}", direction, self.host, hex_str);
        // Log detailed message analysis at trace level
        if log::log_enabled!(log::Level::Trace) {
            if direction == "TX" && data.len() >= 7 {
                // Parse standard Modbus TCP request
                let transaction_id = u16::from_be_bytes([data[0], data[1]]);
                let protocol_id = u16::from_be_bytes([data[2], data[3]]);
                let length = u16::from_be_bytes([data[4], data[5]]);
                let unit_id = data[6];
                let function_code = if data.len() > 7 { data[7] } else { 0 };
                
                trace!(
                    "Modbus TCP Request: Transaction ID={}, Protocol ID={}, Length={}, Unit ID={}, Function Code={}",
                    transaction_id, protocol_id, length, unit_id, function_code
                );
                
                // Further parse based on function code
                if data.len() > 9 {
                    match function_code {
                        1 | 2 | 3 | 4 => { // Read function
                            let address = u16::from_be_bytes([data[8], data[9]]);
                            let quantity = u16::from_be_bytes([data[10], data[11]]);
                            trace!(
                                "Reading: Starting Address={}, Quantity={}",
                                address, quantity
                            );
                        },
                        5 => { // Write single coil
                            let address = u16::from_be_bytes([data[8], data[9]]);
                            let value = u16::from_be_bytes([data[10], data[11]]);
                            trace!(
                                "Write Single Coil: Address={}, Value={}",
                                address, value
                            );
                        },
                        6 => { // Write single register
                            let address = u16::from_be_bytes([data[8], data[9]]);
                            let value = u16::from_be_bytes([data[10], data[11]]);
                            trace!(
                                "Write Single Register: Address={}, Value={}",
                                address, value
                            );
                        },
                        15 | 16 => { // Write multiple coils/registers
                            let address = u16::from_be_bytes([data[8], data[9]]);
                            let quantity = u16::from_be_bytes([data[10], data[11]]);
                            trace!(
                                "Write Multiple: Address={}, Quantity={}",
                                address, quantity
                            );
                        },
                        _ => {
                            trace!("Unknown function code: {}", function_code);
                        }
                    }
                }
            } else if direction == "RX" && data.len() >= 7 {
                // Parse standard Modbus TCP response
                let transaction_id = u16::from_be_bytes([data[0], data[1]]);
                let protocol_id = u16::from_be_bytes([data[2], data[3]]);
                let length = u16::from_be_bytes([data[4], data[5]]);
                let unit_id = data[6];
                let function_code = if data.len() > 7 { data[7] } else { 0 };
                
                if function_code > 0x80 {
                    // Error response
                    let error_code = if data.len() > 8 { data[8] } else { 0 };
                    trace!(
                        "Modbus TCP Error Response: Transaction ID={}, Protocol ID={}, Length={}, Unit ID={}, Error Function={}, Exception Code={}",
                        transaction_id, protocol_id, length, unit_id, function_code, error_code
                    );
                } else {
                    trace!(
                        "Modbus TCP Response: Transaction ID={}, Protocol ID={}, Length={}, Unit ID={}, Function Code={}",
                        transaction_id, protocol_id, length, unit_id, function_code
                    );
                    
                    // Parse based on response type
                    if data.len() > 8 {
                        match function_code {
                            1 | 2 => { // Read coils/discrete inputs response
                                let byte_count = data[8] as usize;
                                if data.len() >= 9 + byte_count {
                                    let values: Vec<u8> = data[9..9+byte_count].to_vec();
                                    trace!("Read Coils/Discrete Inputs Response: Byte Count={}, Values={:?}", byte_count, values);
                                }
                            },
                            3 | 4 => { // Read holding/input registers response
                                let byte_count = data[8] as usize;
                                if data.len() >= 9 + byte_count && byte_count % 2 == 0 {
                                    let mut registers = Vec::new();
                                    for i in 0..byte_count/2 {
                                        let idx = 9 + i * 2;
                                        let reg = u16::from_be_bytes([data[idx], data[idx+1]]);
                                        registers.push(reg);
                                    }
                                    trace!("Read Registers Response: Byte Count={}, Registers={:?}", byte_count, registers);
                                }
                            },
                            5 | 6 | 15 | 16 => { // Write response
                                if data.len() >= 11 {
                                    let address = u16::from_be_bytes([data[8], data[9]]);
                                    let value = u16::from_be_bytes([data[10], data[11]]);
                                    trace!("Write Response: Address={}, Value/Quantity={}", address, value);
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    /// Execute Modbus operation
    async fn execute<F, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce(&mut client::Context) -> Result<T> + Send + Clone + 'static,
        T: Send + 'static,
    {
        let retry_count = self.base.retry_count();
        let mut attempts = 0;
        let mut last_error = None;
        
        while attempts < retry_count {
            // If max retries exceeded, return last error
            if attempts > 0 {
                info!("Retry {}/{} for Modbus TCP operation", attempts, retry_count);
                // Wait a bit before retrying
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            
            attempts += 1;
            
            // Get lock and execute operation
            let mut client_guard = self.client.lock().await;
            // Clone the operation closure to reuse in different retries
            let operation_clone = operation.clone();
            
            if let Some(ref mut client) = *client_guard {
                // Execute operation directly within the locked context
                // Note: Due to tokio_modbus library encapsulation, we can't directly get the raw message
                // But the message stream should be logged in the underlying implementation
                match operation_clone(client) {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        warn!("Modbus TCP operation failed: {}", e);
                        last_error = Some(e);
                        // Release lock before waiting
                        drop(client_guard);
                        continue;
                    }
                }
            } else {
                // Client not connected, try to connect
                drop(client_guard);
                match self.connect().await {
                    Ok(_) => continue, // Retry operation
                    Err(e) => {
                        warn!("Failed to connect: {}", e);
                        last_error = Some(e);
                        continue;
                    }
                }
            }
        }
        
        // All retries failed
        Err(last_error.unwrap_or_else(|| ComSrvError::ModbusError("Unknown error in Modbus TCP execute".to_string())))
    }
    
    /// Parse register values to JSON
    fn parse_registers(&self, registers: &[u16], mapping: &ModbusRegisterMapping) -> Result<serde_json::Value> {
        if registers.is_empty() {
            return Err(ComSrvError::ParsingError("Empty register data".to_string()));
        }
        
        let byte_order = &mapping.byte_order;
        
        match mapping.data_type {
            ModbusDataType::Bool => {
                Ok(json!(registers[0] != 0))
            },
            ModbusDataType::Int16 => {
                let value = registers[0] as i16;
                Ok(json!(value))
            },
            ModbusDataType::UInt16 => {
                Ok(json!(registers[0]))
            },
            ModbusDataType::Int32 => {
                if registers.len() < 2 {
                    return Err(ComSrvError::ParsingError(
                        "Insufficient registers for Int32".to_string()
                    ));
                }
                
                let value = match byte_order {
                    ByteOrder::BigEndian => {
                        ((registers[0] as i32) << 16) | (registers[1] as i32)
                    },
                    ByteOrder::LittleEndian => {
                        ((registers[1] as i32) << 16) | (registers[0] as i32)
                    },
                    ByteOrder::BigEndianWordSwapped => {
                        ((registers[1] as i32) << 16) | (registers[0] as i32)
                    },
                    ByteOrder::LittleEndianWordSwapped => {
                        ((registers[0] as i32) << 16) | (registers[1] as i32)
                    },
                };
                
                Ok(json!(value))
            },
            ModbusDataType::UInt32 => {
                if registers.len() < 2 {
                    return Err(ComSrvError::ParsingError(
                        "Insufficient registers for UInt32".to_string()
                    ));
                }
                
                let value = match byte_order {
                    ByteOrder::BigEndian => {
                        ((registers[0] as u32) << 16) | (registers[1] as u32)
                    },
                    ByteOrder::LittleEndian => {
                        ((registers[1] as u32) << 16) | (registers[0] as u32)
                    },
                    ByteOrder::BigEndianWordSwapped => {
                        ((registers[1] as u32) << 16) | (registers[0] as u32)
                    },
                    ByteOrder::LittleEndianWordSwapped => {
                        ((registers[0] as u32) << 16) | (registers[1] as u32)
                    },
                };
                
                Ok(json!(value))
            },
            ModbusDataType::Float32 => {
                if registers.len() < 2 {
                    return Err(ComSrvError::ParsingError(
                        "Insufficient registers for Float32".to_string()
                    ));
                }
                
                // Convert two u16 to u32, then interpret as f32
                let bits = match byte_order {
                    ByteOrder::BigEndian => {
                        ((registers[0] as u32) << 16) | (registers[1] as u32)
                    },
                    ByteOrder::LittleEndian => {
                        ((registers[1] as u32) << 16) | (registers[0] as u32)
                    },
                    ByteOrder::BigEndianWordSwapped => {
                        ((registers[1] as u32) << 16) | (registers[0] as u32)
                    },
                    ByteOrder::LittleEndianWordSwapped => {
                        ((registers[0] as u32) << 16) | (registers[1] as u32)
                    },
                };
                
                let value = f32::from_bits(bits);
                
                // Apply scale factor and offset
                let scaled_value = if let Some(scale) = mapping.scale_factor {
                    value * scale as f32
                } else {
                    value
                };
                
                let final_value = if let Some(offset) = mapping.offset {
                    scaled_value + offset as f32
                } else {
                    scaled_value
                };
                
                Ok(json!(final_value))
            },
            ModbusDataType::Float64 => {
                if registers.len() < 4 {
                    return Err(ComSrvError::ParsingError(
                        "Insufficient registers for Float64".to_string()
                    ));
                }
                
                // Convert four u16 to u64, then interpret as f64
                let bits = match byte_order {
                    ByteOrder::BigEndian => {
                        ((registers[0] as u64) << 48) | 
                        ((registers[1] as u64) << 32) |
                        ((registers[2] as u64) << 16) |
                        (registers[3] as u64)
                    },
                    ByteOrder::LittleEndian => {
                        ((registers[3] as u64) << 48) | 
                        ((registers[2] as u64) << 32) |
                        ((registers[1] as u64) << 16) |
                        (registers[0] as u64)
                    },
                    ByteOrder::BigEndianWordSwapped => {
                        ((registers[1] as u64) << 48) | 
                        ((registers[0] as u64) << 32) |
                        ((registers[3] as u64) << 16) |
                        (registers[2] as u64)
                    },
                    ByteOrder::LittleEndianWordSwapped => {
                        ((registers[2] as u64) << 48) | 
                        ((registers[3] as u64) << 32) |
                        ((registers[0] as u64) << 16) |
                        (registers[1] as u64)
                    },
                };
                
                let value = f64::from_bits(bits);
                
                // Apply scale factor and offset
                let scaled_value = if let Some(scale) = mapping.scale_factor {
                    value * scale
                } else {
                    value
                };
                
                let final_value = if let Some(offset) = mapping.offset {
                    scaled_value + offset
                } else {
                    scaled_value
                };
                
                Ok(json!(final_value))
            },
            ModbusDataType::String(_) => {
                // Build string from registers
                let mut bytes = Vec::with_capacity(registers.len() * 2);
                
                for register in registers {
                    // Add bytes according to byte order
                    match byte_order {
                        ByteOrder::BigEndian | ByteOrder::BigEndianWordSwapped => {
                            bytes.push((register >> 8) as u8);
                            bytes.push((register & 0xFF) as u8);
                        },
                        ByteOrder::LittleEndian | ByteOrder::LittleEndianWordSwapped => {
                            bytes.push((register & 0xFF) as u8);
                            bytes.push((register >> 8) as u8);
                        },
                    }
                }
                
                // Convert to string, remove null bytes
                let valid_bytes: Vec<u8> = bytes.into_iter()
                    .take_while(|&b| b != 0)
                    .collect();
                
                let string_value = String::from_utf8_lossy(&valid_bytes).to_string();
                
                Ok(json!(string_value))
            },
        }
    }

    /// Get client context
    async fn get_client(&self) -> Result<Arc<Mutex<Option<client::Context>>>> {
        // Ensure connected
        self.connect().await?;
        
        // Return Arc clone, not reference, to avoid lifetime issues
        Ok(self.client.clone())
    }
}

#[async_trait]
impl ComBase for ModbusTcpClient {
    fn name(&self) -> &str {
        self.base.name()
    }
    
    fn channel_id(&self) -> &str {
        &self.channel_id_str
    }
    
    fn protocol_type(&self) -> &str {
        "modbus_tcp"
    }
    
    fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("protocol".to_string(), "modbus_tcp".to_string());
        params.insert("host".to_string(), self.host.clone());
        params.insert("port".to_string(), self.port.to_string());
        params.insert("slave_id".to_string(), self.base.slave_id().to_string());
        params.insert("timeout_ms".to_string(), self.base.timeout_ms().to_string());
        params.insert("retry_count".to_string(), self.base.retry_count().to_string());
        params
    }
    
    async fn is_running(&self) -> bool {
        self.base.is_running().await
    }
    
    async fn start(&mut self) -> Result<()> {
        info!("Starting Modbus TCP client: {}:{}", self.host, self.port);
        
        // Set running state
        self.base.set_running(true).await;
        
        // Connect to server
        match self.connect().await {
            Ok(_) => {
                info!("Modbus TCP connection successful: {}:{}", self.host, self.port);
                
                // Load point tables
                let config_path = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "./config".to_string());
                if let Err(e) = self.base.load_point_tables(&config_path).await {
                    error!("Failed to load point tables: {}", e);
                }
                
                Ok(())
            },
            Err(e) => {
                error!("Modbus TCP connection failed: {}", e);
                Err(e)
            }
        }
    }
    
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Modbus TCP client: {}:{} (Channel: {})", self.host, self.port, self.channel_id());
        self.disconnect().await?;
        self.base.set_running(false).await;
        Ok(())
    }
    
    async fn status(&self) -> ChannelStatus {
        self.base.status().await
    }
    
    async fn get_all_points(&self) -> Vec<PointData> {
        let mut points = self.base.get_all_points().await;
        
        // Ensure connected, retry connection if not
        if !self.base.is_connected().await {
            if let Err(e) = self.connect().await {
                error!("Failed to connect to Modbus TCP server: {}", e);
                return points; // Return empty points on connection failure
            }
        }
        
        // Create a map from point ID to points array index for updating
        let mut point_id_to_index = HashMap::new();
        for (i, point) in points.iter().enumerate() {
            point_id_to_index.insert(point.id.clone(), i);
        }
        
        // Get all register mappings from the base implementation
        let register_mappings = self.base.get_all_mappings().await;
        if register_mappings.is_empty() {
            warn!("No register mappings found for channel {}", self.base.channel_id());
            return points;
        }
        
        // Create optimizer for reading registers
        let optimizer = ModbusOptimizer::from_channel(&self.channel_id_str, &self.base.config());
        let read_groups = optimizer.optimize(&register_mappings, self.base.slave_id());
        
        info!("Using optimized batch read with {} read groups", read_groups.len());
        
        // Execute batch read for each read group
        for group in read_groups {
            
            // --- Start: Demonstrate TCP Frame (ADU) Construction --- 
            let pdu = group.build_request_pdu();
            let transaction_id = self.transaction_id.fetch_add(1, Ordering::Relaxed); // Increment and get current ID
            let pdu_length = pdu.len() as u16;
            
            // Build MBAP Header (7 bytes) + PDU
            let mut adu_builder = Vec::with_capacity(7 + pdu.len());
            
            // MBAP Header:
            adu_builder.extend_from_slice(&transaction_id.to_be_bytes()); // Transaction Identifier (2 bytes)
            adu_builder.extend_from_slice(&0u16.to_be_bytes()); // Protocol Identifier (2 bytes, 0 for Modbus)
            adu_builder.extend_from_slice(&(pdu_length + 1).to_be_bytes()); // Length (2 bytes, PDU length + Unit ID byte)
            adu_builder.push(group.slave_id); // Unit Identifier (1 byte)
            
            // Append PDU
            adu_builder.extend_from_slice(&pdu);
            
            // Example: Now `adu_builder` holds the complete TCP ADU frame bytes
            // In a full implementation, these bytes would be sent over the TCP socket,
            // and the response would be read and parsed.
            trace!("Generated TCP ADU for group (addr={}, qty={}, TxID={}): {:02X?}",
                   group.start_address, group.quantity, transaction_id, adu_builder);
            // --- End: Demonstrate TCP Frame (ADU) Construction --- 

            // NOTE: The following code still uses high-level tokio_modbus functions for reading.
            // To use the generated `adu_builder`, the communication logic in `execute`
            // would need to be refactored to handle raw byte transmission and reception.

            // Select read method based on group type
            let result: Result<Vec<u16>> = match group.group_type {
                ModbusReadGroupType::Coil => {
                    self.read_coils(group.start_address, group.quantity).await
                        .map(|bools| bools.into_iter().map(|b| if b { 1u16 } else { 0u16 }).collect())
                },
                ModbusReadGroupType::DiscreteInput => {
                    self.read_discrete_inputs(group.start_address, group.quantity).await
                        .map(|bools| bools.into_iter().map(|b| if b { 1u16 } else { 0u16 }).collect())
                },
                ModbusReadGroupType::HoldingRegister => self.read_holding_registers(group.start_address, group.quantity).await,
                ModbusReadGroupType::InputRegister => self.read_input_registers(group.start_address, group.quantity).await,
            };
            
            // Process the read values and update corresponding points
            match result {
                Ok(data) => {
                    // Process the read values and update corresponding points
                    for (point_id, (offset, mapping)) in &group.mappings {
                        // Check if there is enough register data
                        if *offset + mapping.quantity as usize > data.len() {
                            warn!("Insufficient register values for point {}", point_id);
                            continue;
                        }
                        
                        // Extract register values for the point
                        let registers = &data[*offset..*offset + mapping.quantity as usize];
                        
                        // Parse register values
                        match self.parse_registers(registers, mapping) {
                            Ok(value) => {
                                // Update point value
                                if let Some(&index) = point_id_to_index.get(point_id) {
                                    points[index].value = value;
                                    points[index].quality = true;
                                    points[index].timestamp = Utc::now();
                                }
                            },
                            Err(e) => {
                                error!("Failed to parse registers for point {}: {}", point_id, e);
                                if let Some(&index) = point_id_to_index.get(point_id) {
                                    points[index].quality = false;
                                }
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to read data: {}", e);
                    // 修改：标记所有与该组关联的点位为质量不好
                    for (point_id, _) in &group.mappings {
                        if let Some(&index) = point_id_to_index.get(point_id) {
                            points[index].quality = false;
                        }
                    }
                }
            }
        }
        
        points
    }
}

#[async_trait]
impl ModbusClient for ModbusTcpClient {
    async fn read_coils(&self, address: u16, quantity: u16) -> Result<Vec<bool>> {
        debug!("Reading {} coils from address {}", quantity, address);
        
        self.execute(move |ctx: &mut client::Context| {
            // Wrap the result of the async operation with Ok
            let fut = ctx.read_coils(address, quantity);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to read coils: {}", e)))?)
        }).await
    }
    
    async fn read_discrete_inputs(&self, address: u16, quantity: u16) -> Result<Vec<bool>> {
        debug!("Reading {} discrete inputs from address {}", quantity, address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.read_discrete_inputs(address, quantity);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to read discrete inputs: {}", e)))?)
        }).await
    }
    
    async fn read_holding_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>> {
        debug!("Reading {} holding registers from address {}", quantity, address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.read_holding_registers(address, quantity);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to read holding registers: {}", e)))?)
        }).await
    }
    
    async fn read_input_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>> {
        debug!("Reading {} input registers from address {}", quantity, address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.read_input_registers(address, quantity);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to read input registers: {}", e)))?)
        }).await
    }
    
    async fn write_single_coil(&self, address: u16, value: bool) -> Result<()> {
        debug!("Writing coil at address {} with value {}", address, value);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.write_single_coil(address, value);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to write single coil: {}", e)))?)
        }).await
    }
    
    async fn write_single_register(&self, address: u16, value: u16) -> Result<()> {
        debug!("Writing register at address {} with value {}", address, value);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.write_single_register(address, value);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to write single register: {}", e)))?)
        }).await
    }
    
    async fn write_multiple_coils(&self, address: u16, values: &[bool]) -> Result<()> {
        let values = values.to_vec(); // Clone for move
        debug!("Writing {} coils at address {}", values.len(), address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.write_multiple_coils(address, &values);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to write multiple coils: {}", e)))?)
        }).await
    }
    
    async fn write_multiple_registers(&self, address: u16, values: &[u16]) -> Result<()> {
        let values = values.to_vec(); // Clone for move
        debug!("Writing {} registers at address {}", values.len(), address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.write_multiple_registers(address, &values);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to write multiple registers: {}", e)))?)
        }).await
    }
    
    async fn read_data(&self, mapping: &ModbusRegisterMapping) -> Result<serde_json::Value> {
        let address = mapping.address;
        let quantity = mapping.quantity;
        
        debug!(
            "Reading data from address {} with quantity {} and type {:?}",
            address, quantity, mapping.data_type
        );
        
        // Select read method based on data type
        match mapping.data_type {
            ModbusDataType::Bool => {
                // Read coils for boolean type
                let values = self.read_coils(address, 1).await?;
                if values.is_empty() {
                    return Err(ComSrvError::ModbusError("No coil data received".to_string()));
                }
                Ok(json!(values[0]))
            },
            _ => {
                // Read holding registers for other types
                let registers = self.read_holding_registers(address, quantity).await?;
                self.parse_registers(&registers, mapping)
            }
        }
    }
    
    async fn write_data(&self, mapping: &ModbusRegisterMapping, value: &serde_json::Value) -> Result<()> {
        if !mapping.writable {
            return Err(ComSrvError::InvalidOperation(
                format!("Register at address {} is not writable", mapping.address)
            ));
        }
        
        let address = mapping.address;
        
        // Execute write based on data type
        match mapping.data_type {
            ModbusDataType::Bool => {
                let bool_value = value.as_bool().ok_or_else(|| {
                    ComSrvError::InvalidParameter(
                        format!("Expected boolean value, got: {}", value)
                    )
                })?;
                
                self.write_single_coil(address, bool_value).await
            },
            ModbusDataType::Int16 | ModbusDataType::UInt16 => {
                let int_value = if value.is_i64() {
                    value.as_i64().unwrap() as u16
                } else if value.is_u64() {
                    value.as_u64().unwrap() as u16
                } else if value.is_f64() {
                    value.as_f64().unwrap() as u16
                } else {
                    return Err(ComSrvError::InvalidParameter(
                        format!("Expected numeric value, got: {}", value)
                    ));
                };
                
                self.write_single_register(address, int_value).await
            },
            _ => {
                // Writing complex types is not yet supported
                Err(ComSrvError::ProtocolNotSupported(
                    format!("Writing {:?} data type is not implemented yet", mapping.data_type)
                ))
            }
        }
    }
} 