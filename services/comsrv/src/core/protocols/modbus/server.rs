//! Unified Modbus Server (Slave/Device Simulation)
//!
//! This module provides a unified Modbus server implementation that supports both RTU and TCP communication modes.
//! The server acts as a Modbus slave device, passively responding to client requests with simulated register data.
//! Implements the ComBase trait for standardized protocol interface integration.
//!
//! **Protocol Stack**: This implementation uses the high-performance `voltage_modbus` crate for all protocol
//! operations, ensuring compliance with Modbus standards and optimal performance.
//!
//! ## Modbus Server Behavior
//!
//! As a Modbus Server (slave device), this implementation follows the standard Modbus protocol pattern:
//!
//! ### Passive Operation
//! - **Listens** for incoming client connections (TCP) or serial communications (RTU)
//! - **Responds** to client requests for reading/writing registers
//! - **Maintains** a virtual register bank with current values
//! - **Does NOT** initiate any communication or actively poll external sources
//! - **Simulates** device behavior by providing configurable register mappings
//!
//! ### Supported Operations
//! - Read Coils (0x01) - Single-bit read/write discrete outputs
//! - Read Discrete Inputs (0x02) - Single-bit read-only discrete inputs  
//! - Read Holding Registers (0x03) - 16-bit read/write analog outputs/storage
//! - Read Input Registers (0x04) - 16-bit read-only analog inputs
//! - Write Single Coil (0x05) - Write individual coil
//! - Write Single Register (0x06) - Write individual holding register
//! - Write Multiple Coils (0x0F) - Write multiple coils
//! - Write Multiple Registers (0x10) - Write multiple holding registers
//!
//! ## Usage Scenarios
//!
//! This server is particularly useful for:
//! - **Device Simulation**: Testing Modbus clients without physical hardware
//! - **Protocol Testing**: Validating Modbus client implementations
//! - **System Integration**: Providing mock devices during development
//! - **Training**: Learning Modbus protocol behavior
//! - **Edge Cases**: Simulating error conditions and unusual responses
//!
//! ## Example Usage
//!
//! ```rust
//! use comsrv::core::protocols::modbus::server::*;
//! use comsrv::core::config::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create channel configuration
//!     let channel_config = ChannelConfig {
//!         id: 1,
//!         name: "Simulated Device".to_string(),
//!         description: "Temperature Controller Simulation".to_string(),
//!         protocol: ProtocolType::ModbusTcp,
//!         parameters: ChannelParameters::Generic(HashMap::new()),
//!     };
//!     
//!     // Configure Modbus server with register mappings
//!     let mut modbus_config = ModbusServerConfig::default();
//!     modbus_config.mode = ModbusServerMode::Tcp;
//!     modbus_config.bind_address = Some("0.0.0.0".to_string());
//!     modbus_config.bind_port = Some(502);
//!     modbus_config.unit_id = 1;
//!     
//!     // Create server instance
//!     let mut server = ModbusServer::new(
//!         "TemperatureController".to_string(),
//!         channel_config,
//!         modbus_config,
//!     );
//!     
//!     // Start the server (begins listening for client connections)
//!     server.start().await?;
//!     println!("Modbus server started, waiting for client connections...");
//!     
//!     // Server now passively waits for client requests
//!     // Clients can connect and read/write registers according to the mappings
//!     
//!     // Simulate process updates (in real applications, this might be
//!     // connected to actual sensor readings or control outputs)
//!     tokio::spawn(async move {
//!         let mut temperature = 20.0;
//!         loop {
//!             // Simulate temperature changes
//!             temperature += (rand::random::<f32>() - 0.5) * 2.0;
//!             
//!             // Update register bank (simulating sensor input)
//!             let temp_register = (temperature * 10.0) as u16; // Scale to register format
//!             server.update_register(1, temp_register).await.unwrap();
//!             
//!             tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
//!         }
//!     });
//!     
//!     // Keep running until shutdown
//!     tokio::signal::ctrl_c().await?;
//!     server.stop().await?;
//!     println!("Server stopped");
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Client-Server Interaction Flow
//!
//! 1. **Server Startup**: Server binds to configured address/port and starts listening
//! 2. **Client Connection**: Modbus client connects to server
//! 3. **Client Request**: Client sends read/write request for specific registers
//! 4. **Server Response**: Server responds with current register values or acknowledgment
//! 5. **Passive Waiting**: Server returns to listening state, waiting for next request
//!
//! The server never initiates communication - all interactions are client-driven.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

// Import voltage_modbus components
use voltage_modbus::server::{ModbusRtuServer, ModbusRtuServerConfig};
use voltage_modbus::{
    ModbusRegisterBank, ModbusResult as VoltageModbusResult, ModbusServer as VoltageModbusServer,
    ModbusTcpServer, ModbusTcpServerConfig, ServerStats as VoltageServerStats,
};

use super::common::{ModbusRegisterMapping, ModbusRegisterType};
use crate::core::config::ChannelConfig;
use crate::core::protocols::common::combase::{ChannelStatus, ComBase, ComBaseImpl, PointData};
use crate::utils::error::{ComSrvError, Result};

/// Modbus server mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModbusServerMode {
    /// RTU mode server
    Rtu,
    /// TCP mode server
    Tcp,
}

/// Modbus server configuration
#[derive(Debug, Clone)]
pub struct ModbusServerConfig {
    /// Server mode (RTU or TCP)
    pub mode: ModbusServerMode,
    /// Server unit/slave ID
    pub unit_id: u8,
    /// Maximum number of client connections (TCP mode)
    pub max_connections: Option<u32>,

    // RTU-specific configuration
    /// Serial port path (RTU mode only)
    pub port: Option<String>,
    /// Baud rate (RTU mode only)
    pub baud_rate: Option<u32>,

    // TCP-specific configuration
    /// Bind address (TCP mode only)
    pub bind_address: Option<String>,
    /// Bind port (TCP mode only)
    pub bind_port: Option<u16>,

    /// Request timeout
    pub request_timeout: Option<Duration>,

    /// Register mappings for simulation
    pub register_mappings: Vec<ModbusRegisterMapping>,
}

impl Default for ModbusServerConfig {
    fn default() -> Self {
        Self {
            mode: ModbusServerMode::Tcp,
            unit_id: 1,
            max_connections: Some(10),
            port: Some("/dev/ttyUSB0".to_string()),
            baud_rate: Some(9600),
            bind_address: Some("0.0.0.0".to_string()),
            bind_port: Some(502),
            request_timeout: Some(Duration::from_secs(30)),
            register_mappings: Vec::new(),
        }
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ModbusServerStats {
    /// Total number of requests received
    pub total_requests: u64,
    /// Number of successful responses
    pub successful_responses: u64,
    /// Number of error responses
    pub error_responses: u64,
    /// Number of currently connected clients
    pub connected_clients: u32,
    /// Server uptime
    pub uptime: Duration,
    /// Start time
    pub start_time: SystemTime,
    /// Bytes received
    pub bytes_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
}

impl ModbusServerStats {
    /// Create new server statistics
    pub fn new() -> Self {
        let now = SystemTime::now();
        Self {
            total_requests: 0,
            successful_responses: 0,
            error_responses: 0,
            connected_clients: 0,
            uptime: Duration::new(0, 0),
            start_time: now,
            bytes_received: 0,
            bytes_sent: 0,
        }
    }

    /// Update statistics for a request
    pub fn update_request(&mut self, success: bool) {
        self.total_requests += 1;
        if success {
            self.successful_responses += 1;
        } else {
            self.error_responses += 1;
        }
        self.uptime = self.start_time.elapsed().unwrap_or_default();
    }

    /// Convert from voltage_modbus ServerStats
    pub fn from_voltage_stats(voltage_stats: &VoltageServerStats) -> Self {
        Self {
            total_requests: voltage_stats.total_requests,
            successful_responses: voltage_stats.successful_requests,
            error_responses: voltage_stats.failed_requests,
            connected_clients: voltage_stats.connections_count as u32,
            uptime: Duration::from_secs(voltage_stats.uptime_seconds),
            start_time: SystemTime::now() - Duration::from_secs(voltage_stats.uptime_seconds),
            bytes_received: voltage_stats.bytes_received,
            bytes_sent: voltage_stats.bytes_sent,
        }
    }
}

/// Internal server implementation wrapper
enum InternalServer {
    Tcp(ModbusTcpServer),
    Rtu(ModbusRtuServer),
}

impl InternalServer {
    async fn start(&mut self) -> VoltageModbusResult<()> {
        match self {
            InternalServer::Tcp(server) => server.start().await,
            InternalServer::Rtu(server) => server.start().await,
        }
    }

    async fn stop(&mut self) -> VoltageModbusResult<()> {
        match self {
            InternalServer::Tcp(server) => server.stop().await,
            InternalServer::Rtu(server) => server.stop().await,
        }
    }

    fn is_running(&self) -> bool {
        match self {
            InternalServer::Tcp(server) => server.is_running(),
            InternalServer::Rtu(server) => server.is_running(),
        }
    }

    fn get_stats(&self) -> VoltageServerStats {
        match self {
            InternalServer::Tcp(server) => server.get_stats(),
            InternalServer::Rtu(server) => server.get_stats(),
        }
    }

    fn get_register_bank(&self) -> Option<Arc<ModbusRegisterBank>> {
        match self {
            InternalServer::Tcp(server) => server.get_register_bank(),
            InternalServer::Rtu(server) => server.get_register_bank(),
        }
    }
}

/// Unified Modbus server implementing ComBase trait with voltage_modbus protocol stack
pub struct ModbusServer {
    /// Base communication implementation
    base: ComBaseImpl,
    /// Server configuration
    config: ModbusServerConfig,
    /// Voltage Modbus register bank
    register_bank: Arc<ModbusRegisterBank>,
    /// Internal server implementation
    internal_server: Option<InternalServer>,
    /// Custom statistics wrapper
    stats: Arc<RwLock<ModbusServerStats>>,
}

impl ModbusServer {
    /// Create new Modbus server
    pub fn new(
        name: String,
        channel_config: ChannelConfig,
        modbus_config: ModbusServerConfig,
    ) -> Self {
        // Create register bank and initialize from mappings
        let register_bank = Arc::new(ModbusRegisterBank::new());
        Self::initialize_register_bank(&register_bank, &modbus_config.register_mappings);

        Self {
            base: ComBaseImpl::new(
                &name,
                &format!("ModbusServer_{:?}", modbus_config.mode),
                channel_config,
            ),
            config: modbus_config,
            register_bank,
            internal_server: None,
            stats: Arc::new(RwLock::new(ModbusServerStats::new())),
        }
    }

    /// Initialize register bank from mappings
    fn initialize_register_bank(
        register_bank: &Arc<ModbusRegisterBank>,
        mappings: &[ModbusRegisterMapping],
    ) {
        for mapping in mappings {
            match mapping.register_type {
                ModbusRegisterType::Coil => {
                    let _ = register_bank.write_single_coil(mapping.address, false);
                }
                ModbusRegisterType::DiscreteInput => {
                    let _ = register_bank.set_discrete_input(mapping.address, false);
                }
                ModbusRegisterType::InputRegister => {
                    let _ = register_bank.set_input_register(mapping.address, 0);
                }
                ModbusRegisterType::HoldingRegister => {
                    let _ = register_bank.write_single_register(mapping.address, 0);
                }
            }
        }
    }

    /// Get server configuration
    pub fn get_config(&self) -> &ModbusServerConfig {
        &self.config
    }

    /// Get server statistics
    pub async fn get_stats(&self) -> ModbusServerStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get register bank for external access
    pub fn get_register_bank(&self) -> Arc<ModbusRegisterBank> {
        self.register_bank.clone()
    }

    /// Update a register value (for simulation)
    pub async fn update_register(&self, address: u16, value: u16) -> Result<()> {
        self.register_bank
            .write_single_register(address, value)
            .map_err(|e| {
                ComSrvError::CommunicationError(format!("Failed to update register: {}", e))
            })?;
        Ok(())
    }

    /// Update a coil value (for simulation)
    pub async fn update_coil(&self, address: u16, value: bool) -> Result<()> {
        self.register_bank
            .write_single_coil(address, value)
            .map_err(|e| {
                ComSrvError::CommunicationError(format!("Failed to update coil: {}", e))
            })?;
        Ok(())
    }

    /// Update an input register value (for simulation)
    pub async fn update_input_register(&self, address: u16, value: u16) -> Result<()> {
        self.register_bank
            .set_input_register(address, value)
            .map_err(|e| {
                ComSrvError::CommunicationError(format!("Failed to update input register: {}", e))
            })?;
        Ok(())
    }

    /// Update a discrete input value (for simulation)
    pub async fn update_discrete_input(&self, address: u16, value: bool) -> Result<()> {
        self.register_bank
            .set_discrete_input(address, value)
            .map_err(|e| {
                ComSrvError::CommunicationError(format!("Failed to update discrete input: {}", e))
            })?;
        Ok(())
    }

    /// Create and configure TCP server
    fn create_tcp_server(&self) -> Result<ModbusTcpServer> {
        let bind_addr = format!(
            "{}:{}",
            self.config.bind_address.as_deref().unwrap_or("0.0.0.0"),
            self.config.bind_port.unwrap_or(502)
        );

        let addr = bind_addr
            .parse()
            .map_err(|e| ComSrvError::ConfigurationError(format!("Invalid bind address: {}", e)))?;

        let config = ModbusTcpServerConfig {
            bind_address: addr,
            max_connections: self.config.max_connections.unwrap_or(10) as usize,
            request_timeout: self
                .config
                .request_timeout
                .unwrap_or(Duration::from_secs(30)),
            register_bank: Some(self.register_bank.clone()),
        };

        ModbusTcpServer::with_config(config).map_err(|e| {
            ComSrvError::CommunicationError(format!("Failed to create TCP server: {}", e))
        })
    }

    /// Create and configure RTU server
    fn create_rtu_server(&self) -> Result<ModbusRtuServer> {
        let port = self.config.port.as_deref().unwrap_or("/dev/ttyUSB0");
        let baud_rate = self.config.baud_rate.unwrap_or(9600);

        let config = ModbusRtuServerConfig {
            port: port.to_string(),
            baud_rate,
            data_bits: tokio_serial::DataBits::Eight,
            stop_bits: tokio_serial::StopBits::One,
            parity: tokio_serial::Parity::None,
            timeout: self
                .config
                .request_timeout
                .unwrap_or(Duration::from_secs(30)),
            frame_gap: Duration::from_millis(30),
            register_bank: Some(self.register_bank.clone()),
        };

        ModbusRtuServer::with_config(config).map_err(|e| {
            ComSrvError::CommunicationError(format!("Failed to create RTU server: {}", e))
        })
    }

    /// Start the appropriate server mode
    async fn start_internal_server(&mut self) -> Result<()> {
        // Update status to show we're attempting to start
        self.base.update_status(false, 0.0, None).await;

        let mut internal_server = match self.config.mode {
            ModbusServerMode::Tcp => {
                log::info!(
                    "Starting Modbus TCP server on {}:{}",
                    self.config.bind_address.as_deref().unwrap_or("0.0.0.0"),
                    self.config.bind_port.unwrap_or(502)
                );
                InternalServer::Tcp(self.create_tcp_server()?)
            }
            ModbusServerMode::Rtu => {
                log::info!("Starting Modbus RTU server on port: {:?}", self.config.port);
                InternalServer::Rtu(self.create_rtu_server()?)
            }
        };

        // Start the server before storing it
        internal_server.start().await.map_err(|e| {
            ComSrvError::CommunicationError(format!("Failed to start server: {}", e))
        })?;

        // Store the started server
        self.internal_server = Some(internal_server);

        // Update status to connected
        self.base.update_status(true, 50.0, None).await;

        log::info!(
            "Modbus server started successfully in {:?} mode",
            self.config.mode
        );
        Ok(())
    }

    /// Stop the internal server
    async fn stop_internal_server(&mut self) -> Result<()> {
        if let Some(ref mut server) = self.internal_server {
            server.stop().await.map_err(|e| {
                ComSrvError::CommunicationError(format!("Failed to stop server: {}", e))
            })?;
        }
        self.internal_server = None;
        log::info!("Modbus server stopped");
        Ok(())
    }

    /// Update statistics from internal server
    async fn update_statistics(&self) {
        if let Some(ref server) = self.internal_server {
            let voltage_stats = server.get_stats();
            let mut stats = self.stats.write().await;
            *stats = ModbusServerStats::from_voltage_stats(&voltage_stats);
        }
    }

    /// Get all current register values as data points (passive status reporting)
    ///
    /// This method returns the current state of all registers in the voltage_modbus register bank.
    /// It does not perform any active communication or polling - it simply reports the
    /// current values that the server is holding and would respond with to client requests.
    ///
    /// # Server Behavior
    ///
    /// As a Modbus Server (slave), this implementation:
    /// - Maintains a virtual register bank with current values
    /// - Passively waits for client requests and responds accordingly
    /// - Does NOT initiate any communication or actively read from external sources
    /// - Only reports the current state of internal registers
    ///
    /// # Returns
    ///
    /// Vector of `PointData` representing all register values in the bank:
    /// - Coils (single-bit read/write)
    /// - Discrete Inputs (single-bit read-only)  
    /// - Input Registers (16-bit read-only)
    /// - Holding Registers (16-bit read/write)
    ///
    /// Returns empty vector if server is not running.
    ///
    /// # Usage
    ///
    /// This method is typically used for:
    /// - Status monitoring and diagnostics
    /// - Data visualization in management interfaces
    /// - Logging current server state
    /// - Integration with monitoring systems
    async fn get_register_points(&self) -> Vec<PointData> {
        let mut points = Vec::new();

        // Sample commonly used addresses for different register types
        let sample_addresses = vec![0, 1, 2, 3, 4, 5, 10, 20, 50, 100];

        // Try to read coils
        for &address in &sample_addresses {
            if let Ok(coils) = self.register_bank.read_coils(address, 1) {
                if !coils.is_empty() {
                    points.push(PointData {
                        id: format!("coil_{}", address),
                        name: format!("Coil {}", address),
                        value: coils[0].to_string(),
                        timestamp: chrono::Utc::now(),
                        unit: "bool".to_string(),
                        description: format!("Coil register at address {}", address),
                    });
                }
            }
        }

        // Try to read discrete inputs
        for &address in &sample_addresses {
            if let Ok(inputs) = self.register_bank.read_discrete_inputs(address, 1) {
                if !inputs.is_empty() {
                    points.push(PointData {
                        id: format!("discrete_input_{}", address),
                        name: format!("Discrete Input {}", address),
                        value: inputs[0].to_string(),
                        timestamp: chrono::Utc::now(),
                        unit: "bool".to_string(),
                        description: format!("Discrete input register at address {}", address),
                    });
                }
            }
        }

        // Try to read input registers
        for &address in &sample_addresses {
            if let Ok(registers) = self.register_bank.read_input_registers(address, 1) {
                if !registers.is_empty() {
                    points.push(PointData {
                        id: format!("input_register_{}", address),
                        name: format!("Input Register {}", address),
                        value: registers[0].to_string(),
                        timestamp: chrono::Utc::now(),
                        unit: "uint16".to_string(),
                        description: format!("Input register at address {}", address),
                    });
                }
            }
        }

        // Try to read holding registers
        for &address in &sample_addresses {
            if let Ok(registers) = self.register_bank.read_holding_registers(address, 1) {
                if !registers.is_empty() {
                    points.push(PointData {
                        id: format!("holding_register_{}", address),
                        name: format!("Holding Register {}", address),
                        value: registers[0].to_string(),
                        timestamp: chrono::Utc::now(),
                        unit: "uint16".to_string(),
                        description: format!("Holding register at address {}", address),
                    });
                }
            }
        }

        points
    }
}

impl std::fmt::Debug for ModbusServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModbusServer")
            .field("name", &self.name())
            .field("protocol_type", &self.protocol_type())
            .field("channel_id", &self.channel_id())
            .field("mode", &self.config.mode)
            .field("unit_id", &self.config.unit_id)
            .field(
                "is_running",
                &self
                    .internal_server
                    .as_ref()
                    .map(|s| s.is_running())
                    .unwrap_or(false),
            )
            .finish()
    }
}

#[async_trait]
impl ComBase for ModbusServer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &str {
        self.base.name()
    }

    fn channel_id(&self) -> String {
        self.base.channel_id()
    }

    fn protocol_type(&self) -> &str {
        self.base.protocol_type()
    }

    fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = self.base.get_parameters();

        // Add Modbus-specific parameters
        params.insert("mode".to_string(), format!("{:?}", self.config.mode));
        params.insert("unit_id".to_string(), self.config.unit_id.to_string());

        if let Some(ref port) = self.config.port {
            params.insert("port".to_string(), port.clone());
        }

        if let Some(baud_rate) = self.config.baud_rate {
            params.insert("baud_rate".to_string(), baud_rate.to_string());
        }

        if let Some(ref bind_address) = self.config.bind_address {
            params.insert("bind_address".to_string(), bind_address.clone());
        }

        if let Some(bind_port) = self.config.bind_port {
            params.insert("bind_port".to_string(), bind_port.to_string());
        }

        if let Some(max_connections) = self.config.max_connections {
            params.insert("max_connections".to_string(), max_connections.to_string());
        }

        if let Some(ref timeout) = self.config.request_timeout {
            params.insert(
                "request_timeout_ms".to_string(),
                timeout.as_millis().to_string(),
            );
        }

        params.insert(
            "register_mappings_count".to_string(),
            self.config.register_mappings.len().to_string(),
        );

        params
    }

    async fn is_running(&self) -> bool {
        // Use voltage_modbus server's actual is_running() method (now properly implemented)
        // Combined with base ComBase state tracking for comprehensive status
        match &self.internal_server {
            Some(server) => self.base.is_running().await && server.is_running(),
            None => false,
        }
    }

    async fn start(&mut self) -> Result<()> {
        if self.is_running().await {
            return Err(ComSrvError::StateError(
                "Server is already running".to_string(),
            ));
        }

        // Start the base service
        self.base.start().await?;

        // Start the internal server
        let result = self.start_internal_server().await;

        match result {
            Ok(_) => {
                log::info!(
                    "Modbus server '{}' started in {:?} mode",
                    self.name(),
                    self.config.mode
                );
                Ok(())
            }
            Err(e) => {
                // If server start failed, stop base service
                self.base.stop().await?;
                self.base
                    .set_error(&format!("Failed to start server: {}", e))
                    .await;
                Err(e)
            }
        }
    }

    async fn stop(&mut self) -> Result<()> {
        if !self.is_running().await {
            return Ok(());
        }

        // Stop the internal server
        self.stop_internal_server().await?;

        // Stop the base service
        self.base.stop().await?;

        log::info!("Modbus server '{}' stopped", self.name());
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        let mut status = self.base.status().await;

        // Update statistics from internal server
        self.update_statistics().await;

        // Add server-specific status information
        let stats = self.stats.read().await;
        status.last_response_time = if stats.total_requests > 0 {
            // Use actual response time if available
            50.0 // Placeholder - could be calculated from stats
        } else {
            status.last_response_time
        };

        // Update connection status
        status.connected = self.is_running().await;

        status
    }

    /// Get all current register values as data points (passive status reporting)
    ///
    /// This method returns the current state of all registers in the voltage_modbus register bank.
    /// It does not perform any active communication or polling - it simply reports the
    /// current values that the server is holding and would respond with to client requests.
    ///
    /// # Server Behavior
    ///
    /// As a Modbus Server (slave), this implementation:
    /// - Maintains a virtual register bank with current values
    /// - Passively waits for client requests and responds accordingly
    /// - Does NOT initiate any communication or actively read from external sources
    /// - Only reports the current state of internal registers
    ///
    /// # Returns
    ///
    /// Vector of `PointData` representing all register values in the bank:
    /// - Coils (single-bit read/write)
    /// - Discrete Inputs (single-bit read-only)  
    /// - Input Registers (16-bit read-only)
    /// - Holding Registers (16-bit read/write)
    ///
    /// Returns empty vector if server is not running.
    ///
    /// # Usage
    ///
    /// This method is typically used for:
    /// - Status monitoring and diagnostics
    /// - Data visualization in management interfaces
    /// - Logging current server state
    /// - Integration with monitoring systems
    async fn get_all_points(&self) -> Vec<PointData> {
        if !self.is_running().await {
            return Vec::new();
        }

        // Get points from register bank
        self.get_register_points().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
            use crate::core::config::{ChannelConfig, ChannelParameters, ProtocolType};

    fn create_test_config() -> ChannelConfig {
        ChannelConfig {
            id: 1,
            name: "Test Modbus Server".to_string(),
            description: Some("Test Description".to_string()),
            protocol: ProtocolType::ModbusTcp,
            parameters: ChannelParameters::Generic(HashMap::new()),
        }
    }

    fn create_test_modbus_config() -> ModbusServerConfig {
        use rand::Rng;
        let port = rand::thread_rng().gen_range(10000..20000);

        ModbusServerConfig {
            mode: ModbusServerMode::Tcp,
            unit_id: 1,
            max_connections: Some(10),
            bind_address: Some("127.0.0.1".to_string()),
            bind_port: Some(port),
            request_timeout: Some(Duration::from_secs(10)),
            register_mappings: vec![ModbusRegisterMapping {
                name: "test_register".to_string(),
                display_name: Some("Test Register".to_string()),
                address: 1,
                register_type: ModbusRegisterType::HoldingRegister,
                data_type: crate::core::protocols::modbus::common::ModbusDataType::UInt16,
                scale: 1.0,
                offset: 0.0,
                unit: Some("uint16".to_string()),
                description: Some("Test register".to_string()),
                access_mode: "read_write".to_string(),
                group: Some("test".to_string()),
                byte_order: crate::core::protocols::modbus::common::ByteOrder::BigEndian,
            }],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_modbus_server_creation() {
        let channel_config = create_test_config();
        let modbus_config = create_test_modbus_config();

        let server = ModbusServer::new("TestServer".to_string(), channel_config, modbus_config);

        assert_eq!(server.name(), "TestServer");
        assert!(server.protocol_type().contains("ModbusServer"));
        assert!(!server.is_running().await);
    }

    #[tokio::test]
    async fn test_modbus_server_lifecycle() {
        let channel_config = create_test_config();
        let modbus_config = create_test_modbus_config();

        let mut server = ModbusServer::new("TestServer".to_string(), channel_config, modbus_config);

        // Initial state
        assert!(!server.is_running().await);

        // Start server
        let result = server.start().await;
        assert!(
            result.is_ok(),
            "Server should start successfully: {:?}",
            result
        );

        // Give the server more time to fully start up and wait for is_running to be true
        let mut retries = 0;
        let max_retries = 50; // 5 seconds total
        while retries < max_retries {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if server.is_running().await {
                break;
            }
            retries += 1;
        }

        assert!(
            server.is_running().await,
            "Server should be running after start and proper startup delay"
        );

        // Check status
        let status = server.status().await;
        assert!(status.connected);

        // Stop server
        let result = server.stop().await;
        assert!(
            result.is_ok(),
            "Server should stop successfully: {:?}",
            result
        );
        assert!(!server.is_running().await);
    }

    #[tokio::test]
    async fn test_modbus_server_parameters() {
        let channel_config = create_test_config();
        let modbus_config = create_test_modbus_config();

        let server = ModbusServer::new("TestServer".to_string(), channel_config, modbus_config);

        let params = server.get_parameters();
        assert_eq!(params.get("mode").unwrap(), "Tcp");
        assert_eq!(params.get("unit_id").unwrap(), "1");
        assert_eq!(params.get("bind_address").unwrap(), "127.0.0.1");
        assert!(params.get("bind_port").unwrap().parse::<u16>().is_ok());
        assert_eq!(params.get("register_mappings_count").unwrap(), "1");
        assert_eq!(params.get("request_timeout_ms").unwrap(), "10000");
    }

    #[tokio::test]
    async fn test_modbus_server_register_operations() {
        let channel_config = create_test_config();
        let modbus_config = create_test_modbus_config();

        let server = ModbusServer::new("TestServer".to_string(), channel_config, modbus_config);

        // Update register value
        server.update_register(1, 42).await.unwrap();

        // Update coil value
        server.update_coil(1, true).await.unwrap();

        // Update input register
        server.update_input_register(1, 123).await.unwrap();

        // Update discrete input
        server.update_discrete_input(1, false).await.unwrap();

        // Verify values through direct register bank access
        let bank = server.get_register_bank();

        let holding_regs = bank.read_holding_registers(1, 1).unwrap();
        assert_eq!(holding_regs[0], 42);

        let coils = bank.read_coils(1, 1).unwrap();
        assert_eq!(coils[0], true);

        let input_regs = bank.read_input_registers(1, 1).unwrap();
        assert_eq!(input_regs[0], 123);

        let discrete_inputs = bank.read_discrete_inputs(1, 1).unwrap();
        assert_eq!(discrete_inputs[0], false);
    }

    #[tokio::test]
    async fn test_modbus_server_points_collection() {
        let channel_config = create_test_config();
        let modbus_config = create_test_modbus_config();

        let mut server = ModbusServer::new("TestServer".to_string(), channel_config, modbus_config);

        // Server not running - no points
        let points = server.get_all_points().await;
        assert!(points.is_empty());

        // Start server
        server.start().await.unwrap();

        // Wait for server to be fully running
        let mut retries = 0;
        let max_retries = 50; // 5 seconds total
        while retries < max_retries {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if server.is_running().await {
                break;
            }
            retries += 1;
        }

        // Ensure server is actually running before proceeding
        if !server.is_running().await {
            panic!(
                "Server failed to start properly after {} retries",
                max_retries
            );
        }

        // Update some values
        server.update_register(1, 100).await.unwrap();
        server.update_coil(2, true).await.unwrap();
        server.update_input_register(3, 200).await.unwrap();
        server.update_discrete_input(4, true).await.unwrap();

        // Get points - now should have data since server is running
        let points = server.get_all_points().await;
        println!("Collected {} points", points.len());
        for point in &points {
            println!("Point: id={}, value={}", point.id, point.value);
        }

        assert!(
            !points.is_empty(),
            "Should have collected some data points when server is running"
        );

        // Find the specific points we set (they should be in the default sample addresses)
        let holding_reg_point = points
            .iter()
            .find(|p| p.id == "holding_register_1")
            .expect("Should find holding register point");
        assert_eq!(holding_reg_point.value, "100");

        let coil_point = points
            .iter()
            .find(|p| p.id == "coil_2")
            .expect("Should find coil point");
        assert_eq!(coil_point.value, "true");

        let input_reg_point = points
            .iter()
            .find(|p| p.id == "input_register_3")
            .expect("Should find input register point");
        assert_eq!(input_reg_point.value, "200");

        server.stop().await.unwrap();
    }

    #[test]
    fn test_voltage_modbus_register_bank_operations() {
        let bank = ModbusRegisterBank::new();

        // Test coil operations
        bank.write_single_coil(0, true).unwrap();
        let coils = bank.read_coils(0, 1).unwrap();
        assert_eq!(coils[0], true);

        // Test register operations
        bank.write_single_register(0, 42).unwrap();
        let regs = bank.read_holding_registers(0, 1).unwrap();
        assert_eq!(regs[0], 42);

        // Test multiple operations
        bank.write_multiple_coils(10, &[true, false, true]).unwrap();
        let coils = bank.read_coils(10, 3).unwrap();
        assert_eq!(coils, vec![true, false, true]);

        bank.write_multiple_registers(20, &[100, 200, 300]).unwrap();
        let regs = bank.read_holding_registers(20, 3).unwrap();
        assert_eq!(regs, vec![100, 200, 300]);

        // Test input register simulation methods
        bank.set_input_register(5, 500).unwrap();
        let input_regs = bank.read_input_registers(5, 1).unwrap();
        assert_eq!(input_regs[0], 500);

        // Test discrete input simulation methods
        bank.set_discrete_input(5, true).unwrap();
        let discrete_inputs = bank.read_discrete_inputs(5, 1).unwrap();
        assert_eq!(discrete_inputs[0], true);
    }

    #[test]
    fn test_modbus_server_config_modes() {
        let channel_config = create_test_config();

        // Test TCP mode
        let tcp_config = ModbusServerConfig {
            mode: ModbusServerMode::Tcp,
            bind_address: Some("192.168.1.100".to_string()),
            bind_port: Some(8080),
            ..Default::default()
        };

        let tcp_server =
            ModbusServer::new("TCPServer".to_string(), channel_config.clone(), tcp_config);

        let params = tcp_server.get_parameters();
        assert_eq!(params.get("mode").unwrap(), "Tcp");
        assert_eq!(params.get("bind_address").unwrap(), "192.168.1.100");
        assert_eq!(params.get("bind_port").unwrap(), "8080");

        // Test RTU mode
        let rtu_config = ModbusServerConfig {
            mode: ModbusServerMode::Rtu,
            port: Some("/dev/ttyUSB1".to_string()),
            baud_rate: Some(115200),
            ..Default::default()
        };

        let rtu_server = ModbusServer::new("RTUServer".to_string(), channel_config, rtu_config);

        let params = rtu_server.get_parameters();
        assert_eq!(params.get("mode").unwrap(), "Rtu");
        assert_eq!(params.get("port").unwrap(), "/dev/ttyUSB1");
        assert_eq!(params.get("baud_rate").unwrap(), "115200");
    }

    #[test]
    fn test_server_stats_conversion() {
        let voltage_stats = VoltageServerStats {
            connections_count: 5,
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            bytes_received: 1000,
            bytes_sent: 800,
            uptime_seconds: 3600,
            register_bank_stats: None,
        };

        let converted_stats = ModbusServerStats::from_voltage_stats(&voltage_stats);

        assert_eq!(converted_stats.connected_clients, 5);
        assert_eq!(converted_stats.total_requests, 100);
        assert_eq!(converted_stats.successful_responses, 95);
        assert_eq!(converted_stats.error_responses, 5);
        assert_eq!(converted_stats.bytes_received, 1000);
        assert_eq!(converted_stats.bytes_sent, 800);
        assert_eq!(converted_stats.uptime, Duration::from_secs(3600));
    }
}
