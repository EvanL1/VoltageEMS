use std::sync::Arc;
use tokio::sync::RwLock;
use dashmap::DashMap;
use ahash::AHashMap;
use async_trait::async_trait;

use crate::core::config::config_manager::{ChannelConfig, ProtocolType, ConfigManager};
use crate::core::protocols::common::ComBase;
use crate::core::protocols::iec60870::iec104::Iec104Client;
use crate::core::protocols::modbus::{ModbusClient, ModbusCommunicationMode};
use crate::utils::{ComSrvError, Result};

/// Dynamic communication client type for type-erased storage
pub type DynComClient = Arc<RwLock<Box<dyn ComBase>>>;

/// Protocol client factory trait for extensible protocol support
/// 
/// Implement this trait for each protocol to enable factory-based creation
/// and registration with validation support.
#[async_trait]
pub trait ProtocolClientFactory: Send + Sync {
    /// Get the protocol type this factory handles
    fn protocol_type(&self) -> ProtocolType;
    
    /// Create a new client instance for the given configuration
    /// 
    /// # Arguments
    /// 
    /// * `config` - Channel configuration containing protocol parameters
    /// * `config_manager` - Optional config manager for loading point tables
    /// 
    /// # Returns
    /// 
    /// `Ok(client)` if successful, `Err` if creation failed
    fn create_client(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>>;
    
    /// Validate configuration before client creation
    /// 
    /// # Arguments
    /// 
    /// * `config` - Channel configuration to validate
    /// 
    /// # Returns
    /// 
    /// `Ok(())` if configuration is valid, `Err` with details if invalid
    fn validate_config(&self, config: &ChannelConfig) -> Result<()>;
    
    /// Get default configuration template for this protocol
    fn default_config(&self) -> ChannelConfig;
    
    /// Get configuration schema for validation and UI generation
    fn config_schema(&self) -> serde_json::Value;
}

/// Built-in Modbus TCP factory implementation
pub struct ModbusTcpFactory;

impl ProtocolClientFactory for ModbusTcpFactory {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::ModbusTcp
    }
    
    fn create_client(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        let mut modbus_config: crate::core::protocols::modbus::ModbusClientConfig = config.clone().into();
        
        // Load point mappings from config manager if available
        if let Some(cm) = config_manager {
            match cm.get_modbus_mappings_for_channel(config.id) {
                Ok(mappings) => {
                    tracing::info!("Loaded {} point mappings for channel {}", mappings.len(), config.id);
                    modbus_config.point_mappings = mappings;
                }
                Err(e) => {
                    tracing::warn!("Failed to load point mappings for channel {}: {}", config.id, e);
                }
            }
        }
        
        let client = ModbusClient::new(modbus_config, ModbusCommunicationMode::Tcp)?;
        Ok(Box::new(client))
    }
    
    fn validate_config(&self, config: &ChannelConfig) -> Result<()> {
        use crate::core::config::config_manager::ChannelParameters;
        
        let params = match &config.parameters {
            ChannelParameters::Generic(map) => map,
            _ => return Err(ComSrvError::ConfigError("Invalid parameter format for Modbus TCP".to_string())),
        };
        
        // Validate required parameters
        if let Some(address) = params.get("address") {
            if address.as_str().unwrap_or("").is_empty() {
                return Err(ComSrvError::InvalidParameter("address cannot be empty".to_string()));
            }
        } else {
            return Err(ComSrvError::InvalidParameter("address parameter is required".to_string()));
        }
        
        // Validate port
        if let Some(port) = params.get("port") {
            if let Some(port_num) = port.as_u64() {
                if port_num == 0 || port_num > 65535 {
                    return Err(ComSrvError::InvalidParameter("port must be between 1 and 65535".to_string()));
                }
            }
        }
        
        // Validate timeout
        if let Some(timeout) = params.get("timeout") {
            if let Some(timeout_ms) = timeout.as_u64() {
                if timeout_ms < 100 || timeout_ms > 30000 {
                    return Err(ComSrvError::InvalidParameter("timeout must be between 100 and 30000 ms".to_string()));
                }
            }
        }
        
        Ok(())
    }
    
    fn default_config(&self) -> ChannelConfig {
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
        parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
        parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
        
        ChannelConfig {
            id: 0,
            name: "Modbus TCP Channel".to_string(),
            description: "Modbus TCP communication channel".to_string(),
            protocol: ProtocolType::ModbusTcp,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(parameters),
        }
    }
    
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "address": {
                    "type": "string",
                    "description": "Target device IP address or hostname",
                    "example": "192.168.1.100",
                    "required": true
                },
                "port": {
                    "type": "integer",
                    "description": "TCP port number",
                    "minimum": 1,
                    "maximum": 65535,
                    "default": 502
                },
                "timeout": {
                    "type": "integer",
                    "description": "Communication timeout in milliseconds",
                    "minimum": 100,
                    "maximum": 30000,
                    "default": 5000
                }
            },
            "required": ["address"]
        })
    }
}

/// Built-in IEC 104 factory implementation
pub struct Iec104Factory;

impl ProtocolClientFactory for Iec104Factory {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Iec104
    }
    
    fn create_client(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        let client = Iec104Client::new(config);
        Ok(Box::new(client))
    }
    
    fn validate_config(&self, config: &ChannelConfig) -> Result<()> {
        use crate::core::config::config_manager::ChannelParameters;
        
        let params = match &config.parameters {
            ChannelParameters::Generic(map) => map,
            _ => return Err(ComSrvError::ConfigError("Invalid parameter format for IEC 104".to_string())),
        };
        
        // Validate required parameters
        if let Some(address) = params.get("address") {
            if address.as_str().unwrap_or("").is_empty() {
                return Err(ComSrvError::InvalidParameter("address cannot be empty".to_string()));
            }
        } else {
            return Err(ComSrvError::InvalidParameter("address parameter is required".to_string()));
        }
        
        // Validate port
        if let Some(port) = params.get("port") {
            if let Some(port_num) = port.as_u64() {
                if port_num == 0 || port_num > 65535 {
                    return Err(ComSrvError::InvalidParameter("port must be between 1 and 65535".to_string()));
                }
            }
        }
        
        Ok(())
    }
    
    fn default_config(&self) -> ChannelConfig {
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
        parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(2404)));
        parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
        
        ChannelConfig {
            id: 0,
            name: "IEC 104 Channel".to_string(),
            description: "IEC 60870-5-104 communication channel".to_string(),
            protocol: ProtocolType::Iec104,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(parameters),
        }
    }
    
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "address": {
                    "type": "string",
                    "description": "Target device IP address or hostname",
                    "example": "192.168.1.100",
                    "required": true
                },
                "port": {
                    "type": "integer",
                    "description": "TCP port number",
                    "minimum": 1,
                    "maximum": 65535,
                    "default": 2404
                },
                "timeout": {
                    "type": "integer",
                    "description": "Communication timeout in milliseconds",
                    "minimum": 100,
                    "maximum": 30000,
                    "default": 5000
                }
            },
            "required": ["address"]
        })
    }
}

/// Built-in Modbus RTU factory implementation
pub struct ModbusRtuFactory;

impl ProtocolClientFactory for ModbusRtuFactory {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::ModbusRtu
    }
    
    fn create_client(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        let mut modbus_config: crate::core::protocols::modbus::ModbusClientConfig = config.clone().into();
        
        // Load point mappings from config manager if available
        if let Some(cm) = config_manager {
            match cm.get_modbus_mappings_for_channel(config.id) {
                Ok(mappings) => {
                    tracing::info!("Loaded {} point mappings for RTU channel {}", mappings.len(), config.id);
                    modbus_config.point_mappings = mappings;
                }
                Err(e) => {
                    tracing::warn!("Failed to load point mappings for RTU channel {}: {}", config.id, e);
                }
            }
        }
        
        let client = ModbusClient::new(modbus_config, ModbusCommunicationMode::Rtu)?;
        Ok(Box::new(client))
    }
    
    fn validate_config(&self, config: &ChannelConfig) -> Result<()> {
        use crate::core::config::config_manager::ChannelParameters;
        
        let params = match &config.parameters {
            ChannelParameters::Generic(map) => map,
            _ => return Err(ComSrvError::ConfigError("Invalid parameter format for Modbus RTU".to_string())),
        };
        
        // Validate required parameters
        if let Some(port) = params.get("port") {
            if port.as_str().unwrap_or("").is_empty() {
                return Err(ComSrvError::InvalidParameter("port cannot be empty".to_string()));
            }
        } else {
            return Err(ComSrvError::InvalidParameter("port parameter is required".to_string()));
        }
        
        // Validate baud rate
        if let Some(baud_rate) = params.get("baud_rate") {
            if let Some(baud) = baud_rate.as_u64() {
                let valid_baud_rates = [300, 600, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                if !valid_baud_rates.contains(&(baud as u32)) {
                    return Err(ComSrvError::InvalidParameter(
                        format!("Invalid baud rate: {}. Valid rates: {:?}", baud, valid_baud_rates)
                    ));
                }
            }
        }
        
        // Validate data bits
        if let Some(data_bits) = params.get("data_bits") {
            if let Some(bits) = data_bits.as_u64() {
                if bits != 7 && bits != 8 {
                    return Err(ComSrvError::InvalidParameter("data_bits must be 7 or 8".to_string()));
                }
            }
        }
        
        // Validate stop bits
        if let Some(stop_bits) = params.get("stop_bits") {
            if let Some(bits) = stop_bits.as_u64() {
                if bits != 1 && bits != 2 {
                    return Err(ComSrvError::InvalidParameter("stop_bits must be 1 or 2".to_string()));
                }
            }
        }
        
        // Validate parity
        if let Some(parity) = params.get("parity") {
            if let Some(parity_str) = parity.as_str() {
                match parity_str.to_lowercase().as_str() {
                    "none" | "even" | "odd" => {},
                    _ => return Err(ComSrvError::InvalidParameter(
                        "parity must be 'None', 'Even', or 'Odd'".to_string()
                    )),
                }
            }
        }
        
        // Validate slave ID
        if let Some(slave_id) = params.get("slave_id") {
            if let Some(id) = slave_id.as_u64() {
                if id == 0 || id > 247 {
                    return Err(ComSrvError::InvalidParameter("slave_id must be between 1 and 247".to_string()));
                }
            }
        }
        
        Ok(())
    }
    
    fn default_config(&self) -> ChannelConfig {
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("port".to_string(), serde_yaml::Value::String("/dev/ttyUSB0".to_string()));
        parameters.insert("baud_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(9600)));
        parameters.insert("data_bits".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(8)));
        parameters.insert("stop_bits".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
        parameters.insert("parity".to_string(), serde_yaml::Value::String("None".to_string()));
        parameters.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
        parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1000)));
        parameters.insert("retry_count".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(3)));
        
        ChannelConfig {
            id: 0,
            name: "Modbus RTU Channel".to_string(),
            description: "Modbus RTU serial communication channel".to_string(),
            protocol: ProtocolType::ModbusRtu,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(parameters),
        }
    }
    
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "port": {
                    "type": "string",
                    "description": "Serial port device path",
                    "example": "/dev/ttyUSB0",
                    "required": true
                },
                "baud_rate": {
                    "type": "integer",
                    "description": "Serial communication baud rate",
                    "enum": [300, 600, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200],
                    "default": 9600
                },
                "data_bits": {
                    "type": "integer",
                    "description": "Number of data bits",
                    "enum": [7, 8],
                    "default": 8
                },
                "stop_bits": {
                    "type": "integer",
                    "description": "Number of stop bits",
                    "enum": [1, 2],
                    "default": 1
                },
                "parity": {
                    "type": "string",
                    "description": "Parity checking mode",
                    "enum": ["None", "Even", "Odd"],
                    "default": "None"
                },
                "slave_id": {
                    "type": "integer",
                    "description": "Modbus slave device ID",
                    "minimum": 1,
                    "maximum": 247,
                    "default": 1
                },
                "timeout": {
                    "type": "integer",
                    "description": "Communication timeout in milliseconds",
                    "minimum": 100,
                    "maximum": 10000,
                    "default": 1000
                },
                "retry_count": {
                    "type": "integer",
                    "description": "Number of retry attempts on communication failure",
                    "minimum": 0,
                    "maximum": 10,
                    "default": 3
                },
                "character_timeout": {
                    "type": "integer",
                    "description": "Character timeout in microseconds (1.5 character times default)",
                    "minimum": 100,
                    "maximum": 100000
                },
                "frame_timeout": {
                    "type": "integer",
                    "description": "Frame timeout in microseconds (3.5 character times default)",
                    "minimum": 100,
                    "maximum": 100000
                }
            },
            "required": ["port"]
        })
    }
}

/// High-performance protocol factory for creating communication protocol instances
pub struct ProtocolFactory {
    /// Store created channels using DashMap for concurrent access
    channels: DashMap<u16, Arc<RwLock<Box<dyn ComBase>>>, ahash::RandomState>,
    /// Channel metadata cache
    channel_metadata: DashMap<u16, ChannelMetadata, ahash::RandomState>,
    /// Registry of protocol factories by protocol type
    protocol_factories: DashMap<ProtocolType, Arc<dyn ProtocolClientFactory>, ahash::RandomState>,
}

/// Channel metadata for quick access
#[derive(Debug, Clone)]
struct ChannelMetadata {
    pub name: String,
    pub protocol_type: ProtocolType,
    pub created_at: std::time::Instant,
    pub last_accessed: Arc<RwLock<std::time::Instant>>,
}

impl ProtocolFactory {
    /// Create a new protocol factory with optimized data structures
    pub fn new() -> Self {
        let factory = Self {
            channels: DashMap::with_hasher(ahash::RandomState::new()),
            channel_metadata: DashMap::with_hasher(ahash::RandomState::new()),
            protocol_factories: DashMap::with_hasher(ahash::RandomState::new()),
        };
        
        // Register built-in protocol factories by default
        factory.register_builtin_factories();
        factory
    }
    
    /// Register built-in protocol factories
    fn register_builtin_factories(&self) {
        self.register_protocol_factory(Arc::new(ModbusTcpFactory));
        self.register_protocol_factory(Arc::new(Iec104Factory));
        self.register_protocol_factory(Arc::new(ModbusRtuFactory));
        tracing::info!("Registered built-in protocol factories");
    }
    
    /// Register a protocol factory
    /// 
    /// # Arguments
    /// 
    /// * `factory` - Protocol factory implementation to register
    pub fn register_protocol_factory(&self, factory: Arc<dyn ProtocolClientFactory>) {
        let protocol_type = factory.protocol_type();
        self.protocol_factories.insert(protocol_type.clone(), factory);
        tracing::info!("Registered protocol factory for: {:?}", protocol_type);
    }
    
    /// Get list of supported protocol types
    pub fn supported_protocols(&self) -> Vec<ProtocolType> {
        self.protocol_factories.iter().map(|entry| entry.key().clone()).collect()
    }
    
    /// Check if a protocol is supported
    pub fn is_protocol_supported(&self, protocol_type: &ProtocolType) -> bool {
        self.protocol_factories.contains_key(protocol_type)
    }
    
    /// Validate configuration for a specific protocol
    /// 
    /// # Arguments
    /// 
    /// * `config` - Channel configuration to validate
    pub fn validate_config(&self, config: &ChannelConfig) -> Result<()> {
        match self.protocol_factories.get(&config.protocol) {
            Some(factory) => factory.validate_config(config),
            None => Err(ComSrvError::ProtocolNotSupported(format!(
                "Protocol type not supported: {:?}", 
                config.protocol
            ))),
        }
    }
    
    /// Get default configuration for a protocol
    /// 
    /// # Arguments
    /// 
    /// * `protocol_type` - Protocol type to get default configuration for
    pub fn get_default_config(&self, protocol_type: &ProtocolType) -> Option<ChannelConfig> {
        self.protocol_factories.get(protocol_type).map(|factory| factory.default_config())
    }
    
    /// Get configuration schema for a protocol
    /// 
    /// # Arguments
    /// 
    /// * `protocol_type` - Protocol type to get schema for
    pub fn get_config_schema(&self, protocol_type: &ProtocolType) -> Option<serde_json::Value> {
        self.protocol_factories.get(protocol_type).map(|factory| factory.config_schema())
    }

    /// Check if the factory has no channels
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Create a protocol instance using registered factories
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub fn create_protocol(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        self.create_protocol_with_config_manager(config, None)
    }

    /// Create a protocol instance using registered factories with config manager
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    /// * `config_manager` - Config manager for loading point tables
    pub fn create_protocol_with_config_manager(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        // First validate the configuration
        self.validate_config(&config)?;
        
        // Try to use registered factory first
        if let Some(factory) = self.protocol_factories.get(&config.protocol) {
            tracing::info!("Creating protocol instance using registered factory: {:?}", config.protocol);
            return factory.create_client(config, config_manager);
        }
        
        // Fallback to legacy implementation for backward compatibility
        match config.protocol {
            ProtocolType::ModbusRtu => self.create_modbus_rtu_with_config_manager(config, config_manager),
            ProtocolType::Virtual => self.create_virtual(config),
            // For other protocol types that don't have registered factories
            ProtocolType::Dio | 
            ProtocolType::Can | 
            ProtocolType::Iec61850 => {
                Err(ComSrvError::ProtocolNotSupported(format!(
                    "Protocol type not supported: {:?}", 
                    config.protocol
                )))
            }
            // ModbusTcp and Iec104 should be handled by registered factories
            _ => {
                Err(ComSrvError::ProtocolNotSupported(format!(
                    "Protocol factory not found: {:?}", 
                    config.protocol
                )))
            }
        }
    }

    // Create Modbus RTU client (now using factory)
    #[inline]
    fn create_modbus_rtu_with_config_manager(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        // Try to use registered factory first
        if let Some(factory) = self.protocol_factories.get(&ProtocolType::ModbusRtu) {
            return factory.create_client(config, config_manager);
        }
        
        // Fallback error
        Err(ComSrvError::ProtocolNotSupported(
            "Modbus RTU factory not registered".to_string()
        ))
    }

    // Create virtual channel
    #[inline]
    fn create_virtual(&self, _config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        Err(ComSrvError::ProtocolNotSupported(
            "Virtual protocol not implemented yet".to_string()
        ))
    }

    /// Create protocol instances from configurations with parallel processing
    ///
    /// # Arguments
    ///
    /// * `configs` - Channel configurations
    pub async fn create_protocols_parallel(&self, configs: Vec<ChannelConfig>) -> Vec<Result<Box<dyn ComBase>>> {
        use futures::future::join_all;
        
        let futures = configs.into_iter().map(|config| {
            let factory = self;
            async move { factory.create_protocol(config) }
        });
        
        join_all(futures).await
    }
    
    /// Create and register a channel with optimized performance and validation
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub fn create_channel(&self, config: ChannelConfig) -> Result<()> {
        self.create_channel_with_config_manager(config, None)
    }

    /// Create and register a channel with config manager support for point table loading
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    /// * `config_manager` - Optional config manager for loading point tables
    pub fn create_channel_with_config_manager(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<()> {
        let channel_id = config.id;
        
        // Validate configuration using registered factories
        self.validate_config(&config)?;

        // Create protocol instance with config manager support
        let protocol =
            self.create_protocol_with_config_manager(config.clone(), config_manager)?;

        // Create metadata and channel wrapper
        let metadata = ChannelMetadata {
            name: config.name.clone(),
            protocol_type: config.protocol.clone(),
            created_at: std::time::Instant::now(),
            last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
        };
        let channel_wrapper = Arc::new(RwLock::new(protocol));

        // Try to insert the channel; fail if the ID already exists
        if self
            .channels
            .try_insert(channel_id, channel_wrapper)
            .is_err()
        {
            return Err(ComSrvError::ConfigError(format!(
                "Channel ID already exists: {}",
                channel_id
            )));
        }
        // Insert metadata after channel insertion
        self.channel_metadata.insert(channel_id, metadata);
        
        tracing::info!("Created channel {} with protocol {:?}", channel_id, config.protocol);
        Ok(())
    }
    
    /// Start all channels with improved error handling and parallel execution
    pub async fn start_all_channels(&self) -> Result<()> {
        use futures::future::join_all;
        
        let start_futures = self.channels.iter().map(|entry| {
            let (id, channel_wrapper) = (entry.key(), entry.value());
            let id = *id;
            let channel_wrapper = channel_wrapper.clone();
            let channel_metadata = self.channel_metadata.clone();
            
            async move {
                let mut channel = channel_wrapper.write().await;
                match channel.start().await {
                    Ok(_) => {
                        tracing::info!("Channel {} started successfully", id);
                        // Update last accessed time
                        if let Some(metadata) = channel_metadata.get(&id) {
                            *metadata.last_accessed.write().await = std::time::Instant::now();
                        }
                        Ok(())
                    },
                    Err(e) => {
                        tracing::error!("Failed to start channel {}: {}", id, e);
                        Err(ComSrvError::ChannelError(format!(
                            "Failed to start channel {}: {}", id, e
                        )))
                    }
                }
            }
        });
        
        let results = join_all(start_futures).await;
        
        // Check if any channels failed to start
        let mut failed_channels = Vec::new();
        for result in results {
            if let Err(e) = result {
                failed_channels.push(e);
            }
        }
        
        if !failed_channels.is_empty() {
            return Err(ComSrvError::ChannelError(format!(
                "Failed to start {} channels", failed_channels.len()
            )));
        }
        
        Ok(())
    }
    
    /// Stop all channels with improved performance
    pub async fn stop_all_channels(&self) -> Result<()> {
        use futures::future::join_all;
        
        let stop_futures = self.channels.iter().map(|entry| {
            let (id, channel_wrapper) = (entry.key(), entry.value());
            let id = *id;
            let channel_wrapper = channel_wrapper.clone();
            
            async move {
                let mut channel = channel_wrapper.write().await;
                match channel.stop().await {
                    Ok(_) => {
                        tracing::info!("Channel {} stopped successfully", id);
                    },
                    Err(e) => {
                        tracing::error!("Failed to stop channel {}: {}", id, e);
                        // Continue stopping other channels even if one fails
                    }
                }
            }
        });
        
        join_all(stop_futures).await;
        
        Ok(())
    }
    
    /// Get channel by ID with optimized access
    pub async fn get_channel(&self, id: u16) -> Option<Arc<RwLock<Box<dyn ComBase>>>> {
        let channel = self.channels.get(&id).map(|entry| entry.value().clone());
        
        // Update last accessed time asynchronously if channel exists
        if channel.is_some() {
            if let Some(metadata_entry) = self.channel_metadata.get(&id) {
                let metadata = metadata_entry.value().clone();
                tokio::spawn(async move {
                    *metadata.last_accessed.write().await = std::time::Instant::now();
                });
            }
        }
        
        channel
    }
    
    /// Get all channels as a vector of (id, channel) pairs
    pub fn get_all_channels(&self) -> Vec<(u16, Arc<RwLock<Box<dyn ComBase>>>)> {
        self.channels.iter().map(|entry| {
            let id = *entry.key();
            let channel = entry.value().clone();
            (id, channel)
        }).collect()
    }
    
    /// Get mutable channel by ID
    pub async fn get_channel_mut(&self, id: u16) -> Option<Arc<RwLock<Box<dyn ComBase>>>> {
        // For thread-safe access, we still return Arc<RwLock<_>>
        // The caller is responsible for acquiring write lock
        self.get_channel(id).await
    }
    
    /// Get all channel IDs efficiently
    pub fn get_channel_ids(&self) -> Vec<u16> {
        self.channels.iter().map(|entry| *entry.key()).collect()
    }
    
    /// Get the number of registered channels
    #[inline]
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
    
    /// Get the number of channels that are currently running
    pub async fn running_channel_count(&self) -> usize {
        use futures::future::join_all;

        let running_futures = self.channels.iter().map(|entry| {
            let channel = entry.value().clone();
            async move {
                let ch = channel.read().await;
                ch.is_running().await
            }
        });

        join_all(running_futures)
            .await
            .into_iter()
            .filter(|running| *running)
            .count()
    }

    /// Get channel statistics
    pub async fn get_channel_stats(&self) -> ChannelStats {
        let total_channels = self.channels.len();
        let running_channels = self.running_channel_count().await;
        let mut protocol_counts = AHashMap::new();
        
        // Count channels by protocol based on cached metadata
        for entry in self.channel_metadata.iter() {
            let metadata = entry.value();
            let protocol_name = metadata.protocol_type.as_str();
            *protocol_counts.entry(protocol_name.to_string()).or_insert(0) += 1;
        }
        
        ChannelStats {
            total_channels,
            running_channels,
            protocol_counts,
        }
    }
    
    /// Remove expired or unused channels
    pub async fn cleanup_channels(&self, max_idle_time: std::time::Duration) {
        let now = std::time::Instant::now();
        let mut to_remove = Vec::new();
        
        for entry in self.channel_metadata.iter() {
            let (id, metadata) = (entry.key(), entry.value());
            let last_accessed = *metadata.last_accessed.read().await;
            
            if now.duration_since(last_accessed) > max_idle_time {
                to_remove.push(*id);
            }
        }
        
        for id in to_remove {
            if let Some((_, channel_wrapper)) = self.channels.remove(&id) {
                // Stop the channel before removing it
                if let Ok(mut channel) = channel_wrapper.try_write() {
                    let _ = channel.stop().await;
                }
                tracing::info!("Removed idle channel: {}", id);
            }
            self.channel_metadata.remove(&id);
        }
    }
}

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub total_channels: usize,
    pub running_channels: usize,
    pub protocol_counts: AHashMap<String, usize>,
}

/// Create and configure a default protocol factory with all built-in protocols
/// 
/// # Returns
/// 
/// Configured `ProtocolFactory` with all supported protocols registered
pub fn create_default_factory() -> ProtocolFactory {
    // The new() method already registers built-in factories
    ProtocolFactory::new()
}

/// Create a factory with custom protocol factories
/// 
/// # Arguments
/// 
/// * `custom_factories` - Vector of custom protocol factories to register
/// 
/// # Returns
/// 
/// Configured `ProtocolFactory` with built-in and custom protocols registered
pub fn create_factory_with_custom_protocols(
    custom_factories: Vec<Arc<dyn ProtocolClientFactory>>
) -> ProtocolFactory {
    let factory = ProtocolFactory::new();
    
    // Register custom factories
    for custom_factory in custom_factories {
        factory.register_protocol_factory(custom_factory);
    }
    
    factory
}

impl Default for ProtocolFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::config_manager::ProtocolType;

    fn create_test_channel_config(id: u16, protocol: ProtocolType) -> ChannelConfig {
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
        parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
        parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
        
        ChannelConfig {
            id,
            name: format!("Test Channel {}", id),
            description: format!("Test channel {} description", id),
            protocol,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(parameters),
        }
    }

    /// Create test configuration for Modbus RTU protocol with default serial parameters
    fn create_modbus_rtu_test_config(id: u16) -> ChannelConfig {
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("port".to_string(), serde_yaml::Value::String("/dev/ttyUSB0".to_string()));
        parameters.insert("baud_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(9600)));
        parameters.insert("data_bits".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(8)));
        parameters.insert("stop_bits".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
        parameters.insert("parity".to_string(), serde_yaml::Value::String("None".to_string()));
        parameters.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
        parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1000)));
        parameters.insert("retry_count".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(3)));
        
        ChannelConfig {
            id,
            name: "Test Modbus RTU Channel".to_string(),
            description: "Test Modbus RTU channel description".to_string(),
            protocol: ProtocolType::ModbusRtu,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(parameters),
        }
    }

    #[test]
    fn test_protocol_factory_new() {
        let factory = ProtocolFactory::new();
        assert_eq!(factory.channel_count(), 0);
        assert!(factory.is_empty());
        // Should have built-in factories registered
        assert!(!factory.supported_protocols().is_empty());
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusTcp));
        assert!(factory.is_protocol_supported(&ProtocolType::Iec104));
    }
    
    #[test]
    fn test_supported_protocols() {
        let factory = ProtocolFactory::new();
        let protocols = factory.supported_protocols();
        assert!(protocols.contains(&ProtocolType::ModbusTcp));
        assert!(protocols.contains(&ProtocolType::Iec104));
    }
    
    #[test]
    fn test_config_validation() {
        let factory = ProtocolFactory::new();
        
        // Test valid Modbus TCP config
        let valid_config = create_test_channel_config(1, ProtocolType::ModbusTcp);
        assert!(factory.validate_config(&valid_config).is_ok());
        
        // Test invalid config (missing address)
        let mut invalid_config = valid_config.clone();
        if let crate::core::config::config_manager::ChannelParameters::Generic(ref mut params) = invalid_config.parameters {
            params.remove("address");
        }
        assert!(factory.validate_config(&invalid_config).is_err());
    }
    
    #[test]
    fn test_get_default_config() {
        let factory = ProtocolFactory::new();
        
        let modbus_config = factory.get_default_config(&ProtocolType::ModbusTcp);
        assert!(modbus_config.is_some());
        assert_eq!(modbus_config.unwrap().protocol, ProtocolType::ModbusTcp);
        
        let iec104_config = factory.get_default_config(&ProtocolType::Iec104);
        assert!(iec104_config.is_some());
        assert_eq!(iec104_config.unwrap().protocol, ProtocolType::Iec104);
        
        // Unsupported protocol should return None
        let unsupported = factory.get_default_config(&ProtocolType::Can);
        assert!(unsupported.is_none());
    }
    
    #[test]
    fn test_get_config_schema() {
        let factory = ProtocolFactory::new();
        
        let modbus_schema = factory.get_config_schema(&ProtocolType::ModbusTcp);
        assert!(modbus_schema.is_some());
        
        let iec104_schema = factory.get_config_schema(&ProtocolType::Iec104);
        assert!(iec104_schema.is_some());
        
        // Unsupported protocol should return None
        let unsupported_schema = factory.get_config_schema(&ProtocolType::Can);
        assert!(unsupported_schema.is_none());
    }

    #[test]
    fn test_create_modbus_tcp_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(1, ProtocolType::ModbusTcp);
        
        let result = factory.create_protocol(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_iec104_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(2, ProtocolType::Iec104);
        
        let result = factory.create_protocol(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_unsupported_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(3, ProtocolType::Can);
        
        let result = factory.create_protocol(config);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::ProtocolNotSupported(_)));
    }

    #[test]
    fn test_create_modbus_rtu_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_modbus_rtu_test_config(4);
        
        let result = factory.create_protocol(config);
        assert!(result.is_ok(), "Modbus RTU protocol should be supported");
    }

    #[test]
    fn test_create_virtual_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(5, ProtocolType::Virtual);
        
        let result = factory.create_protocol(config);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::ProtocolNotSupported(_)));
    }

    #[test]
    fn test_create_channel() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(10, ProtocolType::ModbusTcp);
        
        let result = factory.create_channel(config);
        assert!(result.is_ok());
        assert_eq!(factory.channel_count(), 1);
        assert!(!factory.is_empty());
    }

    #[test]
    fn test_create_duplicate_channel() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(20, ProtocolType::ModbusTcp);
        let config2 = create_test_channel_config(20, ProtocolType::Iec104);
        
        let result1 = factory.create_channel(config1);
        assert!(result1.is_ok());
        
        let result2 = factory.create_channel(config2);
        assert!(result2.is_err());
        assert!(matches!(result2.unwrap_err(), ComSrvError::ConfigError(_)));
    }

    #[tokio::test]
    async fn test_concurrent_duplicate_channel_creation() {
        let factory = Arc::new(ProtocolFactory::new());
        let config1 = create_test_channel_config(21, ProtocolType::ModbusTcp);
        let config2 = create_test_channel_config(21, ProtocolType::Iec104);

        let factory_clone = factory.clone();
        let handle1 = tokio::spawn(async move { factory_clone.create_channel(config1) });
        let factory_clone = factory.clone();
        let handle2 = tokio::spawn(async move { factory_clone.create_channel(config2) });

        let res1 = handle1.await.unwrap();
        let res2 = handle2.await.unwrap();

        assert!(res1.is_ok() ^ res2.is_ok(), "one creation must fail");
        assert_eq!(factory.channel_count(), 1);
    }

    #[tokio::test]
    async fn test_get_channel() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(30, ProtocolType::ModbusTcp);
        
        factory.create_channel(config).unwrap();
        
        let channel = factory.get_channel(30).await;
        assert!(channel.is_some());
        
        let non_existent = factory.get_channel(999).await;
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_get_channel_mut() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(40, ProtocolType::ModbusTcp);
        
        factory.create_channel(config).unwrap();
        
        let channel = factory.get_channel_mut(40).await;
        assert!(channel.is_some());
        
        let non_existent = factory.get_channel_mut(999).await;
        assert!(non_existent.is_none());
    }

    #[test]
    fn test_get_all_channels() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(50, ProtocolType::ModbusTcp);
        let config2 = create_test_channel_config(51, ProtocolType::Iec104);
        
        factory.create_channel(config1).unwrap();
        factory.create_channel(config2).unwrap();
        
        let all_channels = factory.get_all_channels();
        assert_eq!(all_channels.len(), 2);
    }

    #[test]
    fn test_get_channel_ids() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(60, ProtocolType::ModbusTcp);
        let config2 = create_test_channel_config(61, ProtocolType::Iec104);
        
        factory.create_channel(config1).unwrap();
        factory.create_channel(config2).unwrap();
        
        let ids = factory.get_channel_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&60));
        assert!(ids.contains(&61));
    }

    #[tokio::test]
    async fn test_get_channel_stats() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(70, ProtocolType::ModbusTcp);
        let config2 = create_test_channel_config(71, ProtocolType::Iec104);
        let config3 = create_test_channel_config(72, ProtocolType::ModbusTcp);
        
        factory.create_channel(config1).unwrap();
        factory.create_channel(config2).unwrap();
        factory.create_channel(config3).unwrap();
        
        let stats = factory.get_channel_stats().await;
        assert_eq!(stats.total_channels, 3);
        assert_eq!(stats.running_channels, 0); // Channels not started yet

        // Start channels and verify running count
        factory.start_all_channels().await.unwrap();
        let stats = factory.get_channel_stats().await;
        assert_eq!(stats.running_channels, 3);
        
        // Check protocol counts
        assert_eq!(stats.protocol_counts.get("ModbusTcp"), Some(&2));
        assert_eq!(stats.protocol_counts.get("Iec104"), Some(&1));
    }

    #[tokio::test]
    async fn test_create_protocols_parallel() {
        let factory = ProtocolFactory::new();
        let configs = vec![
            create_test_channel_config(80, ProtocolType::ModbusTcp),
            create_test_channel_config(81, ProtocolType::Iec104),
            create_test_channel_config(82, ProtocolType::Can), // Should fail
        ];
        
        let results = factory.create_protocols_parallel(configs).await;
        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert!(results[2].is_err());
    }

    #[tokio::test]
    async fn test_cleanup_channels() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(90, ProtocolType::ModbusTcp);
        
        factory.create_channel(config).unwrap();
        
        // Test cleanup with very short idle time (should not remove channels immediately)
        factory.cleanup_channels(std::time::Duration::from_millis(1)).await;
        
        // Channel should still exist
        assert_eq!(factory.channel_count(), 1);
    }

    #[test]
    fn test_channel_metadata() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(100, ProtocolType::ModbusTcp);
        
        factory.create_channel(config.clone()).unwrap();
        
        // Verify metadata was stored
        let metadata = factory.channel_metadata.get(&100).unwrap();
        assert_eq!(metadata.name, config.name);
        assert_eq!(metadata.protocol_type, config.protocol);
    }

    #[test]
    fn test_default_implementation() {
        let factory = ProtocolFactory::default();
        assert_eq!(factory.channel_count(), 0);
        assert!(factory.is_empty());
    }
} 