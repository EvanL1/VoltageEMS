//! Unified Modbus Server
//!
//! This module provides a unified Modbus server that supports both RTU and TCP communication modes.
//! It can handle multiple client connections and provides device simulation capabilities.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{RwLock, Mutex};
use serde::{Deserialize, Serialize};

use crate::utils::error::{ComSrvError, Result};
use super::common::{ModbusRegisterMapping, ModbusDataType, ModbusRegisterType};

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
}

/// Virtual register bank for simulation
#[derive(Debug)]
pub struct RegisterBank {
    /// Coils (single bit read/write)
    coils: HashMap<u16, bool>,
    /// Discrete inputs (single bit read-only)
    discrete_inputs: HashMap<u16, bool>,
    /// Input registers (16-bit read-only)
    input_registers: HashMap<u16, u16>,
    /// Holding registers (16-bit read/write)
    holding_registers: HashMap<u16, u16>,
}

impl RegisterBank {
    /// Create new register bank
    pub fn new() -> Self {
        Self {
            coils: HashMap::new(),
            discrete_inputs: HashMap::new(),
            input_registers: HashMap::new(),
            holding_registers: HashMap::new(),
        }
    }
    
    /// Initialize registers from mappings
    pub fn initialize_from_mappings(&mut self, mappings: &[ModbusRegisterMapping]) {
        for mapping in mappings {
            match mapping.register_type {
                ModbusRegisterType::Coil => {
                    self.coils.insert(mapping.address, false);
                },
                ModbusRegisterType::DiscreteInput => {
                    self.discrete_inputs.insert(mapping.address, false);
                },
                ModbusRegisterType::InputRegister => {
                    self.input_registers.insert(mapping.address, 0);
                },
                ModbusRegisterType::HoldingRegister => {
                    self.holding_registers.insert(mapping.address, 0);
                },
            }
        }
    }
    
    /// Read coils
    pub fn read_coils(&self, address: u16, quantity: u16) -> Result<Vec<bool>> {
        let mut values = Vec::new();
        for i in 0..quantity {
            let addr = address + i;
            values.push(*self.coils.get(&addr).unwrap_or(&false));
        }
        Ok(values)
    }
    
    /// Read discrete inputs
    pub fn read_discrete_inputs(&self, address: u16, quantity: u16) -> Result<Vec<bool>> {
        let mut values = Vec::new();
        for i in 0..quantity {
            let addr = address + i;
            values.push(*self.discrete_inputs.get(&addr).unwrap_or(&false));
        }
        Ok(values)
    }
    
    /// Read input registers
    pub fn read_input_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>> {
        let mut values = Vec::new();
        for i in 0..quantity {
            let addr = address + i;
            values.push(*self.input_registers.get(&addr).unwrap_or(&0));
        }
        Ok(values)
    }
    
    /// Read holding registers
    pub fn read_holding_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>> {
        let mut values = Vec::new();
        for i in 0..quantity {
            let addr = address + i;
            values.push(*self.holding_registers.get(&addr).unwrap_or(&0));
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
    pub fn write_multiple_coils(&mut self, address: u16, values: &[bool]) -> Result<()> {
        for (i, &value) in values.iter().enumerate() {
            self.coils.insert(address + i as u16, value);
        }
        Ok(())
    }
    
    /// Write multiple registers
    pub fn write_multiple_registers(&mut self, address: u16, values: &[u16]) -> Result<()> {
        for (i, &value) in values.iter().enumerate() {
            self.holding_registers.insert(address + i as u16, value);
        }
        Ok(())
    }
}

/// Unified Modbus server
pub struct ModbusServer {
    /// Server configuration
    config: ModbusServerConfig,
    /// Virtual register bank
    register_bank: Arc<RwLock<RegisterBank>>,
    /// Server statistics
    stats: Arc<RwLock<ModbusServerStats>>,
    /// Running state
    is_running: Arc<RwLock<bool>>,
}

impl ModbusServer {
    /// Create new Modbus server
    pub fn new(config: ModbusServerConfig) -> Self {
        let mut register_bank = RegisterBank::new();
        register_bank.initialize_from_mappings(&config.register_mappings);
        
        Self {
            config,
            register_bank: Arc::new(RwLock::new(register_bank)),
            stats: Arc::new(RwLock::new(ModbusServerStats::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Start the server
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                return Err(ComSrvError::StateError("Server is already running".to_string()));
            }
            *running = true;
        }
        
        match self.config.mode {
            ModbusServerMode::Rtu => self.start_rtu_server().await?,
            ModbusServerMode::Tcp => self.start_tcp_server().await?,
        }
        
        log::info!("Modbus server started in {:?} mode", self.config.mode);
        Ok(())
    }
    
    /// Stop the server
    pub async fn stop(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }
        
        log::info!("Modbus server stopped");
        Ok(())
    }
    
    /// Start RTU mode server
    async fn start_rtu_server(&self) -> Result<()> {
        // RTU server implementation would go here
        // This is a placeholder for now
        log::info!("RTU server mode not yet implemented");
        Ok(())
    }
    
    /// Start TCP mode server
    async fn start_tcp_server(&self) -> Result<()> {
        // TCP server implementation would go here
        // This is a placeholder for now
        log::info!("TCP server mode not yet implemented");
        Ok(())
    }
    
    /// Get server statistics
    pub async fn get_stats(&self) -> ModbusServerStats {
        self.stats.read().await.clone()
    }
    
    /// Get register bank for external access
    pub fn get_register_bank(&self) -> Arc<RwLock<RegisterBank>> {
        self.register_bank.clone()
    }
    
    /// Update a register value (for simulation)
    pub async fn update_register(&self, address: u16, value: u16) -> Result<()> {
        let mut bank = self.register_bank.write().await;
        bank.write_single_register(address, value)?;
        Ok(())
    }
    
    /// Update a coil value (for simulation)
    pub async fn update_coil(&self, address: u16, value: bool) -> Result<()> {
        let mut bank = self.register_bank.write().await;
        bank.write_single_coil(address, value)?;
        Ok(())
    }
} 