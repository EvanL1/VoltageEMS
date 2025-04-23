use std::collections::HashMap;
use std::sync::Arc;
// Removing unused imports
// use tokio::sync::RwLock;
// use std::borrow::Borrow;

use crate::core::config::config_manager::{ChannelConfig, ProtocolType};
use crate::core::protocols::common::ComBase;
use crate::core::protocols::modbus::tcp::ModbusTcpClient;
use crate::core::protocols::modbus::rtu::ModbusRtuClient; 
use crate::utils::{ComSrvError, Result};

/// Protocol factory for creating communication protocol instances
pub struct ProtocolFactory {
    /// Store created channels
    channels: HashMap<u16, Box<dyn ComBase>>,
}

impl ProtocolFactory {
    /// Create a new protocol factory
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// Create a protocol instance
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub fn create_protocol(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        match config.protocol {
            // Static matching of various protocol types to avoid dynamic lookups with HashMap
            ProtocolType::ModbusTcp => self.create_modbus_tcp(config),
            ProtocolType::ModbusRtu => self.create_modbus_rtu(config),
            ProtocolType::Virtual => self.create_virtual(config),
            // For other protocol types, return not implemented error
            ProtocolType::Dio | 
            ProtocolType::Can | 
            ProtocolType::Iec104 | 
            ProtocolType::Iec61850 => {
                Err(ComSrvError::ProtocolNotSupported(format!(
                    "Protocol type not supported: {:?}", 
                    config.protocol
                )))
            }
        }
    }

    // Create Modbus TCP client
    fn create_modbus_tcp(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        // Create Modbus TCP client instance
        let client = ModbusTcpClient::new(config);
        Ok(Box::new(client))
    }

    // Create Modbus RTU client
    fn create_modbus_rtu(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        // Create Modbus RTU client instance
        let client = ModbusRtuClient::new(config);
        Ok(Box::new(client))
    }
    
    // Create virtual channel
    fn create_virtual(&self, _config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        // Virtual channel not implemented yet
        Err(ComSrvError::ProtocolNotSupported(
            "Virtual protocol not implemented yet".to_string()
        ))
    }

    /// Create protocol instances from configurations
    ///
    /// # Arguments
    ///
    /// * `configs` - Channel configurations
    pub fn create_protocols(&self, configs: Vec<ChannelConfig>) -> Result<Vec<Box<dyn ComBase>>> {
        let mut protocols = Vec::with_capacity(configs.len());
        
        for config in configs {
            let protocol = self.create_protocol(config)?;
            protocols.push(protocol);
        }
        
        Ok(protocols)
    }
    
    /// Create and register a channel
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub fn create_channel(&mut self, config: ChannelConfig) -> Result<()> {
        // Check if the channel ID already exists
        if self.channels.contains_key(&config.id) {
            return Err(ComSrvError::ConfigError(format!(
                "Channel ID already exists: {}",
                config.id
            )));
        }
        
        // Create protocol instance
        let protocol = self.create_protocol(config.clone())?;
        
        // Add to channel mapping
        self.channels.insert(config.id, protocol);
        Ok(())
    }
    
    /// Start all channels
    ///
    /// Starts all registered communication channels
    pub async fn start_all_channels(&mut self) -> Result<()> {
        for (id, channel) in self.channels.iter_mut() {
            match channel.start().await {
                Ok(_) => tracing::info!("Channel {} started successfully", id),
                Err(e) => {
                    tracing::error!("Failed to start channel {}: {}", id, e);
                    return Err(ComSrvError::ChannelError(format!(
                        "Failed to start channel {}: {}", id, e
                    )));
                }
            }
        }
        Ok(())
    }
    
    /// Stop all channels
    ///
    /// Stops all registered communication channels
    pub async fn stop_all_channels(&mut self) -> Result<()> {
        for (id, channel) in self.channels.iter_mut() {
            match channel.stop().await {
                Ok(_) => tracing::info!("Channel {} stopped successfully", id),
                Err(e) => {
                    tracing::error!("Failed to stop channel {}: {}", id, e);
                    // Continue stopping other channels even if one fails
                }
            }
        }
        Ok(())
    }
    
    /// Get all channels mutable reference
    pub fn get_all_channels_mut(&mut self) -> &mut HashMap<u16, Box<dyn ComBase>> {
        &mut self.channels
    }
    
    /// Get all channels
    pub fn get_all_channels(&self) -> &HashMap<u16, Box<dyn ComBase>> {
        &self.channels
    }
    
    /// Get channel by ID
    pub fn get_channel(&self, id: u16) -> Option<&Box<dyn ComBase>> {
        self.channels.get(&id)
    }
    
    /// Get the mutable reference of the channel by ID
    pub fn get_channel_mut(&mut self, id: u16) -> Option<&mut Box<dyn ComBase>> {
        self.channels.get_mut(&id)
    }
}

// Global protocol factory instance
lazy_static::lazy_static! {
    static ref PROTOCOL_FACTORY: Arc<ProtocolFactory> = Arc::new(ProtocolFactory::new());
}

/// Get the global protocol factory instance
pub fn protocol_factory() -> &'static Arc<ProtocolFactory> {
    &PROTOCOL_FACTORY
}

/// Initialize protocol factory with default configurations
pub fn init_protocol_factory() -> Result<()> {
    tracing::info!("Protocol factory initialized");
    Ok(())
}