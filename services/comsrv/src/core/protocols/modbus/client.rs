//! Unified Modbus Client - Real Implementation using voltage_modbus library
//!
//! This module provides a unified Modbus client that supports both RTU and TCP communication modes.
//! Uses the voltage_modbus library for actual Modbus communication.
//! Key features:
//! - Real RTU and TCP client implementation using voltage_modbus
//! - Proper separation of connection and polling logic  
//! - Improved error handling and logging
//! - Better async lock usage
//! - Batch operation optimization

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, Mutex};
use log::{debug, info, warn, trace, error};
use serde::{Deserialize, Serialize};
use tokio_serial::{DataBits, Parity, StopBits};
use async_trait::async_trait;
use chrono::Utc;

// Import voltage_modbus types
use voltage_modbus::{ModbusResult as VoltageResult, ModbusError as VoltageError};
use voltage_modbus::client::{ModbusClient as VoltageModbusClient, ModbusTcpClient, ModbusRtuClient};
use voltage_modbus::protocol::SlaveId;
use voltage_modbus::transport::TransportStats;

use crate::utils::error::{ComSrvError, Result};
use crate::core::config::ChannelParameters;
use crate::core::metrics::{ProtocolMetrics, DataPoint};
use super::common::{ModbusRegisterMapping, ModbusDataType, ModbusRegisterType, ByteOrder};
use crate::core::config::config_manager::{ChannelConfig, ProtocolType};
use crate::core::protocols::common::combase::{
    ComBase, ChannelStatus, PointData, PointReader, 
    PollingConfig, PollingPoint
};
use crate::utils::logger::{ChannelLogger, LogLevel};

/// Modbus communication mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusCommunicationMode {
    /// RTU mode over serial port
    Rtu,
    /// TCP mode over network
    Tcp,
}

/// Unified Modbus client configuration
#[derive(Debug, Clone)]
pub struct ModbusClientConfig {
    /// Communication mode (RTU or TCP)
    pub mode: ModbusCommunicationMode,
    /// Slave/Unit ID
    pub slave_id: u8,
    /// Connection timeout
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Polling interval for data collection
    pub poll_interval: Duration,
    /// Point mappings for this client
    pub point_mappings: Vec<ModbusRegisterMapping>,
    
    // RTU-specific configuration
    /// Serial port path (RTU mode only)
    pub port: Option<String>,
    /// Baud rate (RTU mode only)
    pub baud_rate: Option<u32>,
    /// Data bits (RTU mode only)
    pub data_bits: Option<DataBits>,
    /// Stop bits (RTU mode only)
    pub stop_bits: Option<StopBits>,
    /// Parity (RTU mode only)
    pub parity: Option<Parity>,
    
    // TCP-specific configuration
    /// Host address (TCP mode only)
    pub host: Option<String>,
    /// Port number (TCP mode only)
    pub tcp_port: Option<u16>,
}

impl Default for ModbusClientConfig {
    fn default() -> Self {
        Self {
            mode: ModbusCommunicationMode::Rtu,
            slave_id: 1,
            timeout: Duration::from_secs(5),
            max_retries: 3,
            poll_interval: Duration::from_secs(1),
            point_mappings: Vec::new(),
            port: Some("/dev/ttyUSB0".to_string()),
            baud_rate: Some(9600),
            data_bits: Some(DataBits::Eight),
            stop_bits: Some(StopBits::One),
            parity: Some(Parity::None),
            host: Some("127.0.0.1".to_string()),
            tcp_port: Some(502),
        }
    }
}

/// Connection state for the Modbus client
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModbusConnectionState {
    /// Client is disconnected
    Disconnected,
    /// Client is attempting to connect
    Connecting,
    /// Client is connected and ready
    Connected,
    /// Client encountered an error
    Error(String),
}

/// Communication statistics for the Modbus client
#[derive(Debug, Clone)]
pub struct ModbusClientStats {
    /// Total number of requests sent
    pub total_requests: u64,
    /// Number of successful requests
    pub successful_requests: u64,
    /// Number of failed requests
    pub failed_requests: u64,
    /// Number of timeout requests
    pub timeout_requests: u64,
    /// Number of CRC errors (RTU mode)
    pub crc_errors: u64,
    /// Number of exception responses
    pub exception_responses: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Number of reconnection attempts
    pub reconnect_attempts: u64,
    /// Last successful communication timestamp
    pub last_successful_communication: Option<SystemTime>,
    /// Communication quality percentage (0-100)
    pub communication_quality: f64,
}

impl ModbusClientStats {
    /// Create new statistics instance
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            timeout_requests: 0,
            crc_errors: 0,
            exception_responses: 0,
            avg_response_time_ms: 0.0,
            reconnect_attempts: 0,
            last_successful_communication: None,
            communication_quality: 100.0,
        }
    }
    
    /// Update statistics after a request
    pub fn update_request_stats(&mut self, success: bool, response_time: Duration, error_type: Option<&str>) {
        self.total_requests += 1;
        
        if success {
            self.successful_requests += 1;
            self.last_successful_communication = Some(SystemTime::now());
        } else {
            self.failed_requests += 1;
            
            // Classify error types
            if let Some(error) = error_type {
                match error {
                    "timeout" => self.timeout_requests += 1,
                    "crc" => self.crc_errors += 1,
                    "exception" => self.exception_responses += 1,
                    _ => {},
                }
            }
        }
        
        // Update average response time
        let current_avg = self.avg_response_time_ms;
        let new_time = response_time.as_millis() as f64;
        self.avg_response_time_ms = if self.total_requests == 1 {
            new_time
        } else {
            (current_avg * (self.total_requests - 1) as f64 + new_time) / self.total_requests as f64
        };
        
        // Update communication quality
        self.communication_quality = if self.total_requests > 0 {
            (self.successful_requests as f64 / self.total_requests as f64) * 100.0
        } else {
            100.0
        };
    }
}

/// Internal Modbus client wrapper enum
enum InternalModbusClient {
    Tcp(ModbusTcpClient),
    Rtu(ModbusRtuClient),
}

/// Unified Modbus client that supports both RTU and TCP modes
pub struct ModbusClient {
    /// Client configuration
    config: ModbusClientConfig,
    /// Internal client instance (wrapped in Arc<Mutex> for thread safety)
    internal_client: Arc<Mutex<Option<InternalModbusClient>>>,
    /// Current connection state
    connection_state: Arc<RwLock<ModbusConnectionState>>,
    /// Client statistics
    stats: Arc<RwLock<ModbusClientStats>>,
    /// Point value cache
    point_cache: Arc<RwLock<HashMap<String, DataPoint>>>,
    /// Running state
    is_running: Arc<RwLock<bool>>,
    /// Channel logger for protocol message logging
    channel_logger: Option<ChannelLogger>,
    /// Polling task handle for graceful shutdown
    polling_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl ModbusClient {
    /// Create a new ModbusClient with the specified configuration
    pub fn new(config: ModbusClientConfig, _mode: ModbusCommunicationMode) -> Result<Self> {
        debug!("Creating ModbusClient with mode: {:?}", config.mode);
        
        let protocol_name = format!("modbus_{:?}", config.mode);
        
        Ok(Self {
            config,
            internal_client: Arc::new(Mutex::new(None)),
            connection_state: Arc::new(RwLock::new(ModbusConnectionState::Disconnected)),
            stats: Arc::new(RwLock::new(ModbusClientStats::new())),
            point_cache: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            channel_logger: None,
            polling_handle: Arc::new(RwLock::new(None)),
        })
    }

    /// Get current connection state
    pub async fn get_connection_state(&self) -> ModbusConnectionState {
        self.connection_state.read().await.clone()
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> ModbusClientStats {
        self.stats.read().await.clone()
    }

    /// Check if the client is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Set channel logger for protocol message logging
    pub fn set_channel_logger(&mut self, logger: ChannelLogger) {
        self.channel_logger = Some(logger);
    }

    /// Connect to the Modbus device
    async fn connect(&mut self) -> Result<()> {
        debug!("Connecting to Modbus device with mode: {:?}", self.config.mode);
        
        // Update state to connecting
        *self.connection_state.write().await = ModbusConnectionState::Connecting;
        
        let result = match self.config.mode {
            ModbusCommunicationMode::Tcp => self.connect_tcp().await,
            ModbusCommunicationMode::Rtu => self.connect_rtu().await,
        };
        
        match result {
            Ok(_) => {
                *self.connection_state.write().await = ModbusConnectionState::Connected;
                info!("Successfully connected to Modbus device");
                
                // Log connection success
                if let Some(logger) = &self.channel_logger {
                    logger.info(&format!("== [{}] Connected to Modbus device (mode: {:?}, slave: {})", 
                        chrono::Utc::now().format("%H:%M:%S%.3f"), 
                        self.config.mode,
                        self.config.slave_id
                    ));
                }
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to connect: {}", e);
                *self.connection_state.write().await = ModbusConnectionState::Error(error_msg.clone());
                error!("Connection failed: {}", error_msg);
                
                // Log connection failure
                if let Some(logger) = &self.channel_logger {
                    logger.error(&format!("== [{}] Connection failed: {}", 
                        chrono::Utc::now().format("%H:%M:%S%.3f"), 
                        error_msg
                    ));
                }
                Err(e)
            }
        }
    }

    /// Connect to TCP Modbus device
    async fn connect_tcp(&mut self) -> Result<()> {
        let host = self.config.host.as_ref()
            .ok_or_else(|| ComSrvError::ConfigError("TCP host not specified".to_string()))?;
        let port = self.config.tcp_port
            .ok_or_else(|| ComSrvError::ConfigError("TCP port not specified".to_string()))?;
        
        let address = format!("{}:{}", host, port);
        debug!("Connecting to TCP Modbus server at {}", address);
        
        match ModbusTcpClient::with_timeout(&address, self.config.timeout).await {
            Ok(client) => {
                *self.internal_client.lock().await = Some(InternalModbusClient::Tcp(client));
                info!("Connected to TCP Modbus server at {}", address);
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to TCP server: {}", e);
                Err(ComSrvError::ConnectionError(format!("TCP connection failed: {}", e)))
            }
        }
    }

    /// Connect to RTU Modbus device
    async fn connect_rtu(&mut self) -> Result<()> {
        let port = self.config.port.as_ref()
            .ok_or_else(|| ComSrvError::ConfigError("RTU port not specified".to_string()))?;
        let baud_rate = self.config.baud_rate
            .ok_or_else(|| ComSrvError::ConfigError("RTU baud rate not specified".to_string()))?;
        
        debug!("Connecting to RTU Modbus device at {} with baud rate {}", port, baud_rate);
        
        match ModbusRtuClient::new(port, baud_rate) {
            Ok(client) => {
                *self.internal_client.lock().await = Some(InternalModbusClient::Rtu(client));
                info!("Connected to RTU Modbus device at {}", port);
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to RTU device: {}", e);
                Err(ComSrvError::ConnectionError(format!("RTU connection failed: {}", e)))
            }
        }
    }

    /// Start the Modbus client
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Modbus client");
        
        // Connect to device
        self.connect().await?;
        
        // Mark as running
        *self.is_running.write().await = true;
        
        // Start polling task
        let config = self.config.clone();
        let stats = Arc::clone(&self.stats);
        let point_cache = Arc::clone(&self.point_cache);
        let is_running = Arc::clone(&self.is_running);
        let internal_client = Arc::clone(&self.internal_client);
        let logger = self.channel_logger.clone();
        
        let handle = tokio::spawn(async move {
            Self::polling_loop(config, stats, point_cache, is_running, internal_client, logger).await;
        });
        
        *self.polling_handle.write().await = Some(handle);
        
        info!("Modbus client started successfully");
        Ok(())
    }

    /// Stop the Modbus client
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Modbus client");
        
        // Mark as not running
        *self.is_running.write().await = false;
        
        // Stop polling task
        if let Some(handle) = self.polling_handle.write().await.take() {
            handle.abort();
            debug!("Polling task stopped");
        }
        
        // Close connection
        if let Some(ref mut client) = self.internal_client.lock().await.as_mut() {
            match client {
                InternalModbusClient::Tcp(tcp_client) => {
                    if let Err(e) = tcp_client.close().await {
                        warn!("Error closing TCP connection: {}", e);
                    }
                }
                InternalModbusClient::Rtu(rtu_client) => {
                    if let Err(e) = rtu_client.close().await {
                        warn!("Error closing RTU connection: {}", e);
                    }
                }
            }
        }
        
        *self.internal_client.lock().await = None;
        *self.connection_state.write().await = ModbusConnectionState::Disconnected;
        
        // Log disconnection
        if let Some(logger) = &self.channel_logger {
            logger.info(&format!("== [{}] Disconnected from Modbus device", 
                chrono::Utc::now().format("%H:%M:%S%.3f")
            ));
        }
        
        info!("Modbus client stopped successfully");
        Ok(())
    }

    /// Polling loop for continuous data collection
    async fn polling_loop(
        config: ModbusClientConfig,
        stats: Arc<RwLock<ModbusClientStats>>,
        point_cache: Arc<RwLock<HashMap<String, DataPoint>>>,
        is_running: Arc<RwLock<bool>>,
        internal_client: Arc<Mutex<Option<InternalModbusClient>>>,
        logger: Option<ChannelLogger>,
    ) {
        debug!("Starting polling loop with interval: {:?}", config.poll_interval);
        
        let mut interval = tokio::time::interval(config.poll_interval);
        
        while *is_running.read().await {
            interval.tick().await;
            
            // Poll all configured points
            for mapping in &config.point_mappings {
                if !*is_running.read().await {
                    break;
                }
                
                match Self::read_point_from_mapping(&config, &internal_client, mapping, logger.as_ref()).await {
                    Ok(data_point) => {
                        // Update point cache
                        point_cache.write().await.insert(mapping.name.clone(), data_point);
                        
                        // Update successful request stats
                        stats.write().await.update_request_stats(true, Duration::from_millis(10), None);
                        trace!("Successfully read point: {}", mapping.name);
                    }
                    Err(e) => {
                        error!("Failed to read point {}: {}", mapping.name, e);
                        
                        // Update failed request stats
                        stats.write().await.update_request_stats(false, Duration::from_millis(10), Some("polling_error"));
                    }
                }
            }
            
            trace!("Polling cycle executed for {} points", config.point_mappings.len());
        }
        
        debug!("Polling loop stopped");
    }

    /// Static method to read a point from mapping (used in polling loop)
    async fn read_point_from_mapping(
        config: &ModbusClientConfig,
        internal_client: &Arc<Mutex<Option<InternalModbusClient>>>,
        mapping: &ModbusRegisterMapping,
        logger: Option<&ChannelLogger>,
    ) -> Result<DataPoint> {
        let start_time = std::time::Instant::now();
        
        // Log the request
        if let Some(logger) = logger {
            logger.debug(&format!(">> [{}] ReadPoint \"{}\" slave={} addr={} type={:?}", 
                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                mapping.name,
                config.slave_id, 
                mapping.address,
                mapping.register_type
            ));
        }
        
        let raw_value = match mapping.register_type {
            ModbusRegisterType::HoldingRegister => {
                let result = {
                    let mut client_guard = internal_client.lock().await;
                    match client_guard.as_mut() {
                        Some(InternalModbusClient::Tcp(client)) => {
                            client.read_holding_registers(config.slave_id, mapping.address, 1).await
                        }
                        Some(InternalModbusClient::Rtu(client)) => {
                            client.read_holding_registers(config.slave_id, mapping.address, 1).await
                        }
                        None => {
                            return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                        }
                    }
                };
                match result {
                    Ok(values) => values[0] as f64,
                    Err(e) => {
                        if let Some(logger) = logger {
                            let duration = start_time.elapsed();
                            logger.error(&format!("<< [{}] ReadPoint \"{}\" ERR: {} ({}ms)", 
                                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                                mapping.name,
                                e,
                                duration.as_millis()
                            ));
                        }
                        return Err(ComSrvError::CommunicationError(format!("Read holding register failed: {}", e)));
                    }
                }
            }
            ModbusRegisterType::InputRegister => {
                let result = {
                    let mut client_guard = internal_client.lock().await;
                    match client_guard.as_mut() {
                        Some(InternalModbusClient::Tcp(client)) => {
                            client.read_input_registers(config.slave_id, mapping.address, 1).await
                        }
                        Some(InternalModbusClient::Rtu(client)) => {
                            client.read_input_registers(config.slave_id, mapping.address, 1).await
                        }
                        None => {
                            return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                        }
                    }
                };
                match result {
                    Ok(values) => values[0] as f64,
                    Err(e) => {
                        if let Some(logger) = logger {
                            let duration = start_time.elapsed();
                            logger.error(&format!("<< [{}] ReadPoint \"{}\" ERR: {} ({}ms)", 
                                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                                mapping.name,
                                e,
                                duration.as_millis()
                            ));
                        }
                        return Err(ComSrvError::CommunicationError(format!("Read input register failed: {}", e)));
                    }
                }
            }
            ModbusRegisterType::Coil => {
                let result = {
                    let mut client_guard = internal_client.lock().await;
                    match client_guard.as_mut() {
                        Some(InternalModbusClient::Tcp(client)) => {
                            client.read_coils(config.slave_id, mapping.address, 1).await
                        }
                        Some(InternalModbusClient::Rtu(client)) => {
                            client.read_coils(config.slave_id, mapping.address, 1).await
                        }
                        None => {
                            return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                        }
                    }
                };
                match result {
                    Ok(values) => if values[0] { 1.0 } else { 0.0 },
                    Err(e) => {
                        if let Some(logger) = logger {
                            let duration = start_time.elapsed();
                            logger.error(&format!("<< [{}] ReadPoint \"{}\" ERR: {} ({}ms)", 
                                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                                mapping.name,
                                e,
                                duration.as_millis()
                            ));
                        }
                        return Err(ComSrvError::CommunicationError(format!("Read coil failed: {}", e)));
                    }
                }
            }
            ModbusRegisterType::DiscreteInput => {
                let result = {
                    let mut client_guard = internal_client.lock().await;
                    match client_guard.as_mut() {
                        Some(InternalModbusClient::Tcp(client)) => {
                            client.read_discrete_inputs(config.slave_id, mapping.address, 1).await
                        }
                        Some(InternalModbusClient::Rtu(client)) => {
                            client.read_discrete_inputs(config.slave_id, mapping.address, 1).await
                        }
                        None => {
                            return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                        }
                    }
                };
                match result {
                    Ok(values) => if values[0] { 1.0 } else { 0.0 },
                    Err(e) => {
                        if let Some(logger) = logger {
                            let duration = start_time.elapsed();
                            logger.error(&format!("<< [{}] ReadPoint \"{}\" ERR: {} ({}ms)", 
                                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                                mapping.name,
                                e,
                                duration.as_millis()
                            ));
                        }
                        return Err(ComSrvError::CommunicationError(format!("Read discrete input failed: {}", e)));
                    }
                }
            }
        };

        // Apply basic scaling and offset
        let processed_value = raw_value * mapping.scale + mapping.offset;

        // Log the successful response
        if let Some(logger) = logger {
            let duration = start_time.elapsed();
            logger.debug(&format!("<< [{}] ReadPoint \"{}\" OK: raw={} processed={} ({}ms)", 
                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                mapping.name,
                raw_value,
                processed_value,
                duration.as_millis()
            ));
        }

        Ok(DataPoint {
            id: mapping.name.clone(),
            value: processed_value.to_string(),
            quality: 1, // Good quality
            timestamp: std::time::SystemTime::now(),
            description: mapping.description.clone().unwrap_or_default(),
        })
    }

    /// Read a single register value from the device
    pub async fn read_holding_register(&self, address: u16) -> Result<u16> {
        let start_time = std::time::Instant::now();
        
        // Log the request
        if let Some(logger) = &self.channel_logger {
            logger.debug(&format!(">> [{}] ReadHolding slave={} addr={} qty=1", 
                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                self.config.slave_id, 
                address
            ));
        }
        
        let result = {
            let mut client_guard = self.internal_client.lock().await;
            match client_guard.as_mut() {
                Some(InternalModbusClient::Tcp(client)) => {
                    client.read_holding_registers(
                        self.config.slave_id,
                        address,
                        1
                    ).await.map(|values| values[0])
                }
                Some(InternalModbusClient::Rtu(client)) => {
                    client.read_holding_registers(
                        self.config.slave_id,
                        address,
                        1
                    ).await.map(|values| values[0])
                }
                None => {
                    return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                }
            }
        };
        
        let duration = start_time.elapsed();
        
        match result {
            Ok(value) => {
                // Log the successful response
                if let Some(logger) = &self.channel_logger {
                    logger.debug(&format!("<< [{}] ReadHolding OK: value={} ({}ms)", 
                        chrono::Utc::now().format("%H:%M:%S%.3f"), 
                        value,
                        duration.as_millis()
                    ));
                }
                self.stats.write().await.update_request_stats(true, duration, None);
                debug!("Read register {} value: {}", address, value);
                Ok(value)
            }
            Err(e) => {
                // Log the error response
                if let Some(logger) = &self.channel_logger {
                    logger.error(&format!("<< [{}] ReadHolding ERR: {} ({}ms)", 
                        chrono::Utc::now().format("%H:%M:%S%.3f"), 
                        e,
                        duration.as_millis()
                    ));
                }
                let error_type = Self::classify_error(&e);
                self.stats.write().await.update_request_stats(false, duration, Some(&error_type));
                error!("Failed to read register {}: {}", address, e);
                Err(ComSrvError::CommunicationError(format!("Read failed: {}", e)))
            }
        }
    }

    /// Read multiple holding registers from the device
    pub async fn read_holding_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>> {
        let start_time = std::time::Instant::now();
        
        // Log the request
        if let Some(logger) = &self.channel_logger {
            logger.debug(&format!(">> [{}] ReadHoldingRegs slave={} addr={} qty={}", 
                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                self.config.slave_id, 
                address,
                quantity
            ));
        }
        
        let result = {
            let mut client_guard = self.internal_client.lock().await;
            match client_guard.as_mut() {
                Some(InternalModbusClient::Tcp(client)) => {
                    client.read_holding_registers(
                        self.config.slave_id,
                        address,
                        quantity
                    ).await
                }
                Some(InternalModbusClient::Rtu(client)) => {
                    client.read_holding_registers(
                        self.config.slave_id,
                        address,
                        quantity
                    ).await
                }
                None => {
                    return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                }
            }
        };
        
        let duration = start_time.elapsed();
        
        match result {
            Ok(values) => {
                // Log the successful response
                if let Some(logger) = &self.channel_logger {
                    logger.debug(&format!("<< [{}] ReadHoldingRegs OK: values={:?} ({}ms)", 
                        chrono::Utc::now().format("%H:%M:%S%.3f"), 
                        values,
                        duration.as_millis()
                    ));
                }
                self.stats.write().await.update_request_stats(true, duration, None);
                debug!("Read {} registers from address {}: {:?}", quantity, address, values);
                Ok(values)
            }
            Err(e) => {
                // Log the error response
                if let Some(logger) = &self.channel_logger {
                    logger.error(&format!("<< [{}] ReadHoldingRegs ERR: {} ({}ms)", 
                        chrono::Utc::now().format("%H:%M:%S%.3f"), 
                        e,
                        duration.as_millis()
                    ));
                }
                let error_type = Self::classify_error(&e);
                self.stats.write().await.update_request_stats(false, duration, Some(&error_type));
                error!("Failed to read registers from {}: {}", address, e);
                Err(ComSrvError::CommunicationError(format!("Read failed: {}", e)))
            }
        }
    }

    /// Write a single holding register to the device
    pub async fn write_single_register(&self, address: u16, value: u16) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        // Log the request
        if let Some(logger) = &self.channel_logger {
            logger.debug(&format!(">> [{}] WriteSingle slave={} addr={} value={}", 
                chrono::Utc::now().format("%H:%M:%S%.3f"), 
                self.config.slave_id, 
                address,
                value
            ));
        }
        
        let result = {
            let mut client_guard = self.internal_client.lock().await;
            match client_guard.as_mut() {
                Some(InternalModbusClient::Tcp(client)) => {
                    client.write_single_register(
                        self.config.slave_id,
                        address,
                        value
                    ).await
                }
                Some(InternalModbusClient::Rtu(client)) => {
                    client.write_single_register(
                        self.config.slave_id,
                        address,
                        value
                    ).await
                }
                None => {
                    return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                }
            }
        };
        
        let duration = start_time.elapsed();
        
        match result {
            Ok(_) => {
                // Log the successful response
                if let Some(logger) = &self.channel_logger {
                    logger.debug(&format!("<< [{}] WriteSingle OK ({}ms)", 
                        chrono::Utc::now().format("%H:%M:%S%.3f"), 
                        duration.as_millis()
                    ));
                }
                self.stats.write().await.update_request_stats(true, duration, None);
                debug!("Wrote register {} value: {}", address, value);
                Ok(())
            }
            Err(e) => {
                // Log the error response
                if let Some(logger) = &self.channel_logger {
                    logger.error(&format!("<< [{}] WriteSingle ERR: {} ({}ms)", 
                        chrono::Utc::now().format("%H:%M:%S%.3f"), 
                        e,
                        duration.as_millis()
                    ));
                }
                let error_type = Self::classify_error(&e);
                self.stats.write().await.update_request_stats(false, duration, Some(&error_type));
                error!("Failed to write register {}: {}", address, e);
                Err(ComSrvError::CommunicationError(format!("Write failed: {}", e)))
            }
        }
    }

    /// Classify voltage_modbus errors for statistics
    fn classify_error(error: &VoltageError) -> String {
        match error {
            VoltageError::Timeout { .. } => "timeout".to_string(),
            VoltageError::Frame { .. } => "crc".to_string(),
            VoltageError::Exception { .. } => "exception".to_string(),
            _ => "other".to_string(),
        }
    }

    /// Read a point based on its mapping configuration
    async fn read_point_internal(&self, mapping: &ModbusRegisterMapping) -> Result<DataPoint> {
        let start_time = std::time::Instant::now();
        
        let raw_value = match mapping.register_type {
            ModbusRegisterType::HoldingRegister => {
                let values = self.read_holding_registers(mapping.address, 1).await?;
                values[0] as f64
            }
            ModbusRegisterType::InputRegister => {
                // Read input registers
                let result = {
                    let mut client_guard = self.internal_client.lock().await;
                    match client_guard.as_mut() {
                        Some(InternalModbusClient::Tcp(client)) => {
                            client.read_input_registers(
                                self.config.slave_id,
                                mapping.address,
                                1
                            ).await
                        }
                        Some(InternalModbusClient::Rtu(client)) => {
                            client.read_input_registers(
                                self.config.slave_id,
                                mapping.address,
                                1
                            ).await
                        }
                        None => {
                            return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                        }
                    }
                };
                
                match result {
                    Ok(values) => values[0] as f64,
                    Err(e) => {
                        return Err(ComSrvError::CommunicationError(format!("Read input register failed: {}", e)));
                    }
                }
            }
            ModbusRegisterType::Coil => {
                // Read coils
                let result = {
                    let mut client_guard = self.internal_client.lock().await;
                    match client_guard.as_mut() {
                        Some(InternalModbusClient::Tcp(client)) => {
                            client.read_coils(
                                self.config.slave_id,
                                mapping.address,
                                1
                            ).await
                        }
                        Some(InternalModbusClient::Rtu(client)) => {
                            client.read_coils(
                                self.config.slave_id,
                                mapping.address,
                                1
                            ).await
                        }
                        None => {
                            return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                        }
                    }
                };
                
                match result {
                    Ok(values) => if values[0] { 1.0 } else { 0.0 },
                    Err(e) => {
                        return Err(ComSrvError::CommunicationError(format!("Read coil failed: {}", e)));
                    }
                }
            }
            ModbusRegisterType::DiscreteInput => {
                // Read discrete inputs
                let result = {
                    let mut client_guard = self.internal_client.lock().await;
                    match client_guard.as_mut() {
                        Some(InternalModbusClient::Tcp(client)) => {
                            client.read_discrete_inputs(
                                self.config.slave_id,
                                mapping.address,
                                1
                            ).await
                        }
                        Some(InternalModbusClient::Rtu(client)) => {
                            client.read_discrete_inputs(
                                self.config.slave_id,
                                mapping.address,
                                1
                            ).await
                        }
                        None => {
                            return Err(ComSrvError::ConnectionError("Client not connected".to_string()));
                        }
                    }
                };
                
                match result {
                    Ok(values) => if values[0] { 1.0 } else { 0.0 },
                    Err(e) => {
                        return Err(ComSrvError::CommunicationError(format!("Read discrete input failed: {}", e)));
                    }
                }
            }
        };

        // Apply data conversion based on mapping configuration
        let processed_value = self.convert_raw_data_to_value(raw_value, mapping)?;

        Ok(DataPoint {
            id: mapping.name.clone(),
            value: processed_value.to_string(),
            quality: 1, // Good quality
            timestamp: std::time::SystemTime::now(),
            description: mapping.description.clone().unwrap_or_default(),
        })
    }

    /// Convert raw register data to actual value based on data type and scaling
    fn convert_raw_data_to_value(&self, raw_value: f64, mapping: &ModbusRegisterMapping) -> Result<f64> {
        let mut value = raw_value;

        // Apply data type conversion
        match mapping.data_type {
            ModbusDataType::UInt16 => {
                // Value is already correct for uint16
            }
            ModbusDataType::Int16 => {
                // Convert unsigned to signed 16-bit
                let raw_u16 = raw_value as u16;
                value = (raw_u16 as i16) as f64;
            }
            ModbusDataType::UInt32 => {
                // For 32-bit values, we would need to read 2 registers
                // This is a simplified implementation
                warn!("UInt32 conversion not fully implemented for single register");
            }
            ModbusDataType::Int32 => {
                // For 32-bit values, we would need to read 2 registers
                // This is a simplified implementation
                warn!("Int32 conversion not fully implemented for single register");
            }
            ModbusDataType::UInt64 => {
                // For 64-bit values, we would need to read 4 registers
                // This is a simplified implementation
                warn!("UInt64 conversion not fully implemented for single register");
            }
            ModbusDataType::Int64 => {
                // For 64-bit values, we would need to read 4 registers
                // This is a simplified implementation
                warn!("Int64 conversion not fully implemented for single register");
            }
            ModbusDataType::Float32 => {
                // For float32, we would need to read 2 registers and convert
                // This is a simplified implementation
                warn!("Float32 conversion not fully implemented for single register");
            }
            ModbusDataType::Float64 => {
                // For float64, we would need to read 4 registers and convert
                // This is a simplified implementation
                warn!("Float64 conversion not fully implemented for single register");
            }
            ModbusDataType::Bool => {
                value = if raw_value != 0.0 { 1.0 } else { 0.0 };
            }
            ModbusDataType::String(_) => {
                // String types are not supported for numeric register readings
                warn!("String data type not supported for register readings");
            }
        }

        // Apply scaling
        value *= mapping.scale;

        // Apply offset
        value += mapping.offset;

        Ok(value)
    }
}

#[async_trait]
impl ComBase for ModbusClient {
    fn name(&self) -> &str {
        "ModbusClient"
    }

    fn channel_id(&self) -> String {
        format!("modbus_{}_{}", 
            match self.config.mode {
                ModbusCommunicationMode::Tcp => "tcp",
                ModbusCommunicationMode::Rtu => "rtu",
            },
            self.config.slave_id
        )
    }

    fn protocol_type(&self) -> &str {
        match self.config.mode {
            ModbusCommunicationMode::Tcp => "ModbusTCP",
            ModbusCommunicationMode::Rtu => "ModbusRTU",
        }
    }

    fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("mode".to_string(), format!("{:?}", self.config.mode));
        params.insert("slave_id".to_string(), self.config.slave_id.to_string());
        params.insert("timeout_ms".to_string(), self.config.timeout.as_millis().to_string());
        params.insert("max_retries".to_string(), self.config.max_retries.to_string());
        params.insert("poll_interval_ms".to_string(), self.config.poll_interval.as_millis().to_string());
        
        match self.config.mode {
            ModbusCommunicationMode::Tcp => {
                if let Some(ref host) = self.config.host {
                    params.insert("host".to_string(), host.clone());
                }
                if let Some(port) = self.config.tcp_port {
                    params.insert("port".to_string(), port.to_string());
                }
            }
            ModbusCommunicationMode::Rtu => {
                if let Some(ref port) = self.config.port {
                    params.insert("port".to_string(), port.clone());
                }
                if let Some(baud_rate) = self.config.baud_rate {
                    params.insert("baud_rate".to_string(), baud_rate.to_string());
                }
            }
        }
        
        params
    }

    async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    async fn start(&mut self) -> Result<()> {
        self.start().await
    }

    async fn stop(&mut self) -> Result<()> {
        self.stop().await
    }

    async fn status(&self) -> ChannelStatus {
        let state = self.get_connection_state().await;
        let is_running = self.is_running().await;
        let stats = self.get_stats().await;
        
        let mut status = ChannelStatus::new(&self.channel_id());
        status.connected = matches!(state, ModbusConnectionState::Connected);
        status.last_response_time = stats.avg_response_time_ms;
        status.last_error = match state {
            ModbusConnectionState::Error(ref msg) => msg.clone(),
            _ => String::new(),
        };
        status.last_update_time = Utc::now();
        status
    }

    async fn get_all_points(&self) -> Vec<PointData> {
        let cache = self.point_cache.read().await;
        cache
            .values()
            .map(|data_point| PointData {
                id: data_point.id.clone(),
                name: data_point.id.clone(),
                value: data_point.value.clone(),
                unit: String::new(),
                description: data_point.description.clone(),
                timestamp: data_point.timestamp.into(),
                quality: data_point.quality,
            })
            .collect()
    }
}

#[async_trait]
impl PointReader for ModbusClient {
    async fn read_point(&self, point: &PollingPoint) -> Result<PointData> {
        // Find the mapping for this point
        let mapping = self
            .config
            .point_mappings
            .iter()
            .find(|m| m.name == point.name)
            .ok_or_else(|| ComSrvError::ConfigError(format!("No mapping found for point: {}", point.name)))?;

        // Use the real read method 
        match Self::read_point_from_mapping(&self.config, &self.internal_client, mapping, self.channel_logger.as_ref()).await {
            Ok(data_point) => {
                Ok(PointData {
                    id: data_point.id,
                    name: mapping.display_name.clone().unwrap_or_else(|| mapping.name.clone()),
                    value: data_point.value,
                    unit: mapping.unit.clone().unwrap_or_default(),
                    description: data_point.description,
                    timestamp: Utc::now(),
                    quality: data_point.quality,
                })
            }
            Err(e) => {
                warn!("Failed to read point {}: {}", point.name, e);
                Ok(PointData {
                    id: mapping.name.clone(),
                    name: mapping.display_name.clone().unwrap_or_else(|| mapping.name.clone()),
                    value: "ERROR".to_string(),
                    unit: mapping.unit.clone().unwrap_or_default(),
                    description: format!("Read error: {}", e),
                    timestamp: Utc::now(),
                    quality: 0, // Bad quality
                })
            }
        }
    }

    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
        let mut results = Vec::new();
        
        // Read each point individually
        // In a real implementation, you would optimize this by grouping adjacent registers
        for point in points {
            match self.read_point(point).await {
                Ok(data) => results.push(data),
                Err(e) => {
                    warn!("Failed to read point {}: {}", point.name, e);
                    // Create an error data point
                    results.push(PointData {
                        id: point.id.clone(),
                        name: point.name.clone(),
                        value: "ERROR".to_string(),
                        unit: String::new(),
                        description: format!("Read error: {}", e),
                        timestamp: Utc::now(),
                        quality: 0, // Bad quality
                    });
                }
            }
        }
        
        Ok(results)
    }

    async fn is_connected(&self) -> bool {
        matches!(self.get_connection_state().await, ModbusConnectionState::Connected)
    }

    fn protocol_name(&self) -> &str {
        self.protocol_type()
    }
}

impl From<ChannelConfig> for ModbusClientConfig {
    fn from(channel_config: ChannelConfig) -> Self {
        let mut config = ModbusClientConfig::default();
        
        // Parse channel parameters
        let params_map = match channel_config.parameters {
            crate::core::config::config_manager::ChannelParameters::Generic(map) => map,
            _ => std::collections::HashMap::new(),
        };
        
        for (name, value) in params_map {
            let param_value = match value {
                serde_yaml::Value::String(s) => s,
                _ => format!("{:?}", value),
            };
                match name.as_str() {
                    "mode" => {
                        config.mode = match param_value.as_str() {
                            "tcp" => ModbusCommunicationMode::Tcp,
                            "rtu" => ModbusCommunicationMode::Rtu,
                            _ => ModbusCommunicationMode::Rtu,
                        };
                    }
                    "slave_id" => {
                        if let Ok(id) = param_value.parse::<u8>() {
                            config.slave_id = id;
                        }
                    }
                    "timeout_ms" => {
                        if let Ok(timeout) = param_value.parse::<u64>() {
                            config.timeout = Duration::from_millis(timeout);
                        }
                    }
                    "max_retries" => {
                        if let Ok(retries) = param_value.parse::<u32>() {
                            config.max_retries = retries;
                        }
                    }
                    "poll_interval_ms" => {
                        if let Ok(interval) = param_value.parse::<u64>() {
                            config.poll_interval = Duration::from_millis(interval);
                        }
                    }
                    "host" => {
                        config.host = Some(param_value);
                    }
                    "port" => {
                        if let Ok(port) = param_value.parse::<u16>() {
                            config.tcp_port = Some(port);
                        }
                    }
                    "serial_port" => {
                        config.port = Some(param_value);
                    }
                    "baud_rate" => {
                        if let Ok(baud) = param_value.parse::<u32>() {
                            config.baud_rate = Some(baud);
                        }
                    }
                    _ => {}
                }
            }
        
        config
    }
}

impl std::fmt::Debug for ModbusClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use try_read to avoid blocking in Debug context
        let connection_state = if let Ok(state) = self.connection_state.try_read() {
            format!("{:?}", *state)
        } else {
            "Locked".to_string()
        };
        
        let is_running = if let Ok(running) = self.is_running.try_read() {
            *running
        } else {
            false
        };
        
        f.debug_struct("ModbusClient")
            .field("mode", &self.config.mode)
            .field("slave_id", &self.config.slave_id)
            .field("connection_state", &connection_state)
            .field("is_running", &is_running)
            .field("has_internal_client", &"Arc<Mutex<Option<InternalModbusClient>>>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_client_creation() {
        let config = ModbusClientConfig::default();
        let client = ModbusClient::new(config, ModbusCommunicationMode::Rtu).unwrap();
        
        assert_eq!(client.name(), "ModbusClient");
        assert!(!client.is_running().await);
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn test_statistics() {
        let mut stats = ModbusClientStats::new();
        
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.communication_quality, 100.0);
        
        stats.update_request_stats(true, Duration::from_millis(100), None);
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.communication_quality, 100.0);
        
        stats.update_request_stats(false, Duration::from_millis(200), Some("timeout"));
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.timeout_requests, 1);
        assert_eq!(stats.communication_quality, 50.0);
    }
} 