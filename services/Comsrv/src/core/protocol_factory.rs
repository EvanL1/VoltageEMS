use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::config::config_manager::ChannelConfig;
use crate::core::protocols::common::ComBase;
use crate::utils::{ComSrvError, Result};

/// Protocol creator function type
type ProtocolCreator = fn(ChannelConfig) -> Result<Box<dyn ComBase>>;

/// Protocol factory for creating communication protocol instances
pub struct ProtocolFactory {
    /// Registered protocol creators
    creators: Arc<RwLock<HashMap<String, ProtocolCreator>>>,
    /// 存储已创建的通道
    channels: HashMap<String, Box<dyn ComBase>>,
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
        
        let creator = creators.get(&config.protocol).ok_or_else(|| {
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
    
    /// 创建并注册一个通道
    ///
    /// # Arguments
    ///
    /// * `config` - 通道配置
    pub async fn create_channel(&mut self, config: ChannelConfig) -> Result<()> {
        // 检查通道ID是否已存在
        if self.channels.contains_key(&config.id) {
            return Err(ComSrvError::ConfigError(format!(
                "Channel ID already exists: {}",
                config.id
            )));
        }
        
        // 创建协议实例
        let protocol = self.create_protocol(config.clone()).await?;
        
        // 添加到通道映射
        self.channels.insert(config.id.clone(), protocol);
        Ok(())
    }
    
    /// 获取所有通道的可变引用
    pub async fn get_all_channels_mut(&mut self) -> &mut HashMap<String, Box<dyn ComBase>> {
        &mut self.channels
    }
    
    /// 获取所有通道
    pub async fn get_all_channels(&self) -> &HashMap<String, Box<dyn ComBase>> {
        &self.channels
    }
    
    /// 获取指定ID的通道
    pub async fn get_channel(&self, id: &str) -> Option<&Box<dyn ComBase>> {
        self.channels.get(id)
    }
    
    /// 获取指定ID的通道的可变引用
    pub async fn get_channel_mut(&mut self, id: &str) -> Option<&mut Box<dyn ComBase>> {
        self.channels.get_mut(id)
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