use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::borrow::Borrow;

use crate::core::config::config_manager::{ChannelConfig, ProtocolType};
use crate::core::protocols::common::ComBase;
use crate::utils::{ComSrvError, Result};

/// Protocol creator function type
type ProtocolCreator = fn(ChannelConfig) -> Result<Box<dyn ComBase>>;

/// Protocol factory for creating communication protocol instances
pub struct ProtocolFactory {
    /// Registered protocol creators
    creators: Arc<RwLock<HashMap<String, ProtocolCreator>>>,
    /// Store created channels
    channels: HashMap<u16, Box<dyn ComBase>>,
}

impl ProtocolFactory {
    /// Create a new protocol factory
    pub fn new() -> Self {
        Self {
            creators: Arc::new(RwLock::new(HashMap::new())),
            channels: HashMap::new(),
        }
    }

    /// Register a protocol creator function
    ///
    /// # Arguments
    ///
    /// * `protocol_type` - Protocol type identifier
    /// * `creator` - Creator function that takes a ChannelConfig and returns a ComBase implementation
    pub async fn register_protocol(&self, protocol_type: &str, creator: ProtocolCreator) -> Result<()> {
        let mut creators = self.creators.write().await;
        
        if creators.contains_key(protocol_type) {
            return Err(ComSrvError::ConfigError(format!(
                "Protocol type already registered: {}",
                protocol_type
            )));
        }
        
        creators.insert(protocol_type.to_string(), creator);
        Ok(())
    }

    /// Create a protocol instance
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub async fn create_protocol(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        let creators = self.creators.read().await;
        
        let protocol_str = config.protocol.to_string();
        let creator = creators.get(&protocol_str).ok_or_else(|| {
            ComSrvError::ConfigError(format!(
                "Unknown protocol type: {}",
                config.protocol
            ))
        })?;
        
        creator(config)
    }

    /// Create protocol instances from configurations
    ///
    /// # Arguments
    ///
    /// * `configs` - Channel configurations
    pub async fn create_protocols(&self, configs: Vec<ChannelConfig>) -> Result<Vec<Box<dyn ComBase>>> {
        let mut protocols = Vec::with_capacity(configs.len());
        
        for config in configs {
            let protocol = self.create_protocol(config).await?;
            protocols.push(protocol);
        }
        
        Ok(protocols)
    }
    
    /// Create and register a channel
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub async fn create_channel(&mut self, config: ChannelConfig) -> Result<()> {
        // Check if the channel ID already exists
        if self.channels.contains_key(&config.id) {
            return Err(ComSrvError::ConfigError(format!(
                "Channel ID already exists: {}",
                config.id
            )));
        }
        
        // Create protocol instance
        let protocol = self.create_protocol(config.clone()).await?;
        
        // Add to channel mapping
        self.channels.insert(config.id, protocol);
        Ok(())
    }
    
    /// Get all channels mutable reference
    pub async fn get_all_channels_mut(&mut self) -> &mut HashMap<u16, Box<dyn ComBase>> {
        &mut self.channels
    }
    
    /// Get all channels
    pub async fn get_all_channels(&self) -> &HashMap<u16, Box<dyn ComBase>> {
        &self.channels
    }
    
    /// Get channel by ID
    pub async fn get_channel(&self, id: u16) -> Option<&Box<dyn ComBase>> {
        self.channels.get(&id)
    }
    
    /// Get the mutable reference of the channel by ID
    pub async fn get_channel_mut(&mut self, id: u16) -> Option<&mut Box<dyn ComBase>> {
        self.channels.get_mut(&id)
    }
}

// Global protocol factory instance
lazy_static::lazy_static! {
    static ref PROTOCOL_FACTORY: ProtocolFactory = ProtocolFactory::new();
}

/// Get the global protocol factory instance
pub fn protocol_factory() -> &'static ProtocolFactory {
    &PROTOCOL_FACTORY
}

/// Register all supported protocol types
pub async fn register_supported_protocols() -> Result<()> {
    let _factory = protocol_factory();
    
    // We'll register protocol creators here
    // These will be added as we implement the actual protocol handlers
    tracing::info!("Supported protocols registered");
    
    Ok(())
}