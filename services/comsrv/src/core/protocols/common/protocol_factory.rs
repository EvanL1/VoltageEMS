use std::sync::Arc;
use tokio::sync::RwLock;
use dashmap::DashMap;
use ahash::AHashMap;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::config::config_manager::{ChannelConfig, ProtocolType, ConfigManager};
use crate::core::protocols::common::ComBase;
// TODO: 暂时屏蔽，等核心组件稳定后再启用
// use crate::core::protocols::iec60870::iec104::Iec104Client;
use crate::core::protocols::modbus::{ModbusClient, ModbusCommunicationMode};
use crate::utils::{ComSrvError, Result};

/// Configuration value type - using JSON internally for better ergonomics
/// YAML files are converted to JSON at entry/exit points
pub type ConfigValue = serde_json::Value;

/// Convert YAML string to JSON Value for internal processing
pub fn yaml_to_json(yaml_str: &str) -> Result<ConfigValue> {
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml_str)
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse YAML: {}", e)))?;
    
    yaml_value_to_json(yaml_value)
}

/// Convert JSON Value back to YAML string for file output
pub fn json_to_yaml(json_value: &ConfigValue) -> Result<String> {
    let yaml_value = json_value_to_yaml(json_value.clone())?;
    serde_yaml::to_string(&yaml_value)
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize to YAML: {}", e)))
}

/// Convert serde_yaml::Value to serde_json::Value
fn yaml_value_to_json(yaml: serde_yaml::Value) -> Result<ConfigValue> {
    match yaml {
        serde_yaml::Value::Null => Ok(serde_json::Value::Null),
        serde_yaml::Value::Bool(b) => Ok(serde_json::Value::Bool(b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(serde_json::Value::Number(serde_json::Number::from(i)))
            } else if let Some(f) = n.as_f64() {
                Ok(serde_json::Value::Number(serde_json::Number::from_f64(f)
                    .ok_or_else(|| ComSrvError::ConfigError("Invalid float number".to_string()))?))
            } else {
                Err(ComSrvError::ConfigError("Unsupported number format".to_string()))
            }
        },
        serde_yaml::Value::String(s) => Ok(serde_json::Value::String(s)),
        serde_yaml::Value::Sequence(seq) => {
            let json_array: Result<Vec<_>> = seq.into_iter()
                .map(yaml_value_to_json)
                .collect();
            Ok(serde_json::Value::Array(json_array?))
        },
        serde_yaml::Value::Mapping(map) => {
            let mut json_obj = serde_json::Map::new();
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s,
                    _ => return Err(ComSrvError::ConfigError("Non-string key in YAML mapping".to_string())),
                };
                json_obj.insert(key, yaml_value_to_json(v)?);
            }
            Ok(serde_json::Value::Object(json_obj))
        },
        serde_yaml::Value::Tagged(_) => {
            Err(ComSrvError::ConfigError("Tagged YAML values not supported".to_string()))
        },
    }
}

/// Convert serde_json::Value to serde_yaml::Value
fn json_value_to_yaml(json: ConfigValue) -> Result<serde_yaml::Value> {
    match json {
        serde_json::Value::Null => Ok(serde_yaml::Value::Null),
        serde_json::Value::Bool(b) => Ok(serde_yaml::Value::Bool(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(serde_yaml::Value::Number(serde_yaml::Number::from(i)))
            } else if let Some(f) = n.as_f64() {
                Ok(serde_yaml::Value::Number(serde_yaml::Number::from(f)))
            } else {
                Err(ComSrvError::ConfigError("Unsupported number format".to_string()))
            }
        },
        serde_json::Value::String(s) => Ok(serde_yaml::Value::String(s)),
        serde_json::Value::Array(arr) => {
            let yaml_seq: Result<Vec<_>> = arr.into_iter()
                .map(json_value_to_yaml)
                .collect();
            Ok(serde_yaml::Value::Sequence(yaml_seq?))
        },
        serde_json::Value::Object(obj) => {
            let mut yaml_map = serde_yaml::Mapping::new();
            for (k, v) in obj {
                yaml_map.insert(
                    serde_yaml::Value::String(k),
                    json_value_to_yaml(v)?
                );
            }
            Ok(serde_yaml::Value::Mapping(yaml_map))
        },
    }
}

/// Convert JSON Value to HashMap<String, serde_yaml::Value> for ChannelParameters
fn json_to_yaml_params(json: ConfigValue) -> std::collections::HashMap<String, serde_yaml::Value> {
    let mut param_map = std::collections::HashMap::new();
    
    if let serde_json::Value::Object(obj) = json {
        for (key, value) in obj {
            let yaml_value = match value {
                serde_json::Value::String(s) => serde_yaml::Value::String(s),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        serde_yaml::Value::Number(serde_yaml::Number::from(i))
                    } else if let Some(f) = n.as_f64() {
                        serde_yaml::Value::Number(serde_yaml::Number::from(f))
                    } else {
                        serde_yaml::Value::Null
                    }
                },
                serde_json::Value::Bool(b) => serde_yaml::Value::Bool(b),
                serde_json::Value::Null => serde_yaml::Value::Null,
                _ => serde_yaml::Value::Null,
            };
            param_map.insert(key, yaml_value);
        }
    }
    
    param_map
}

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
    /// Made async to support protocols that require async initialization
    /// (e.g., TLS handshake, port detection, network discovery)
    /// 
    /// # Arguments
    /// 
    /// * `config` - Channel configuration containing protocol parameters
    /// * `config_manager` - Optional config manager for loading point tables
    /// 
    /// # Returns
    /// 
    /// `Ok(client)` if successful, `Err` if creation failed
    async fn create_client(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>>;
    
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
    /// 
    /// Returns configuration in YAML format for consistency with config files
    fn default_config(&self) -> ChannelConfig;
    
    /// Get configuration schema for validation and UI generation
    /// 
    /// Returns YAML schema for frontend consumption
    fn config_schema(&self) -> ConfigValue;
}

/// Built-in Modbus TCP factory implementation
pub struct ModbusTcpFactory;

#[async_trait]
impl ProtocolClientFactory for ModbusTcpFactory {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::ModbusTcp
    }
    
    async fn create_client(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        let mut modbus_config: crate::core::protocols::modbus::ModbusClientConfig = config.clone().into();
        
        // Load point mappings from config manager if available
        if let Some(cm) = config_manager {
            match cm.get_modbus_mappings_for_channel(config.id) {
                Ok(mappings) => {
                    log::info!("Loaded {} point mappings for channel {}", mappings.len(), config.id);
                    modbus_config.point_mappings = mappings;
                }
                Err(e) => {
                    log::warn!("Failed to load point mappings for channel {}: {}", config.id, e);
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
        let parameters = serde_json::json!({
            "address": "127.0.0.1",
            "port": 502,
            "timeout": 5000
        });
        
        // Convert JSON to HashMap<String, serde_yaml::Value> for compatibility
        let param_map = json_to_yaml_params(parameters);
        
        ChannelConfig {
            id: 0,
            name: "Modbus TCP Channel".to_string(),
            description: "Modbus TCP communication channel".to_string(),
            protocol: ProtocolType::ModbusTcp,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(param_map),
        }
    }
    
    fn config_schema(&self) -> ConfigValue {
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

/// Built-in Modbus RTU factory implementation
pub struct ModbusRtuFactory;

#[async_trait]
impl ProtocolClientFactory for ModbusRtuFactory {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::ModbusRtu
    }
    
    async fn create_client(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        let mut modbus_config: crate::core::protocols::modbus::ModbusClientConfig = config.clone().into();
        
        // Load point mappings from config manager if available
        if let Some(cm) = config_manager {
            match cm.get_modbus_mappings_for_channel(config.id) {
                Ok(mappings) => {
                    log::info!("Loaded {} point mappings for RTU channel {}", mappings.len(), config.id);
                    modbus_config.point_mappings = mappings;
                }
                Err(e) => {
                    log::warn!("Failed to load point mappings for RTU channel {}: {}", config.id, e);
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
        let parameters = serde_json::json!({
            "port": "/dev/ttyUSB0",
            "baud_rate": 9600,
            "data_bits": 8,
            "stop_bits": 1,
            "parity": "None",
            "slave_id": 1,
            "timeout": 1000,
            "retry_count": 3
        });
        
        // Convert JSON to HashMap<String, serde_yaml::Value> for compatibility
        let param_map = json_to_yaml_params(parameters);
        
        ChannelConfig {
            id: 0,
            name: "Modbus RTU Channel".to_string(),
            description: "Modbus RTU serial communication channel".to_string(),
            protocol: ProtocolType::ModbusRtu,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(param_map),
        }
    }
    
    fn config_schema(&self) -> ConfigValue {
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
                    "enum": [9600, 19200, 38400, 57600, 115200],
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
                }
            },
            "required": ["port"]
        })
    }
}

/// Mock ComBase implementation for testing
#[derive(Debug)]
struct MockComBase {
    name: String,
    channel_id: u16,
    protocol_type: String,
    running: AtomicBool,
    should_fail_start: AtomicBool,
    should_fail_stop: AtomicBool,
}

impl MockComBase {
    fn new(name: &str, channel_id: u16, protocol_type: &str) -> Self {
        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            running: AtomicBool::new(false),
            should_fail_start: AtomicBool::new(false),
            should_fail_stop: AtomicBool::new(false),
        }
    }
    
    fn with_start_failure(name: &str, channel_id: u16, protocol_type: &str) -> Self {
        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            running: AtomicBool::new(false),
            should_fail_start: AtomicBool::new(true),
            should_fail_stop: AtomicBool::new(false),
        }
    }
    
    fn with_stop_failure(name: &str, channel_id: u16, protocol_type: &str) -> Self {
        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            running: AtomicBool::new(false),
            should_fail_start: AtomicBool::new(false),
            should_fail_stop: AtomicBool::new(true),
        }
    }
}

#[async_trait::async_trait]
impl ComBase for MockComBase {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn channel_id(&self) -> String {
        self.channel_id.to_string()
    }
    
    fn protocol_type(&self) -> &str {
        &self.protocol_type
    }
    
    fn get_parameters(&self) -> std::collections::HashMap<String, String> {
        let mut params = std::collections::HashMap::new();
        params.insert("mock".to_string(), "true".to_string());
        params.insert("channel_id".to_string(), self.channel_id.to_string());
        params
    }
    
    async fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
    
    async fn start(&mut self) -> Result<()> {
        if self.should_fail_start.load(Ordering::Relaxed) {
            return Err(crate::utils::ComSrvError::InvalidOperation("Mock start failure".to_string()));
        }
        self.running.store(true, Ordering::Relaxed);
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        if self.should_fail_stop.load(Ordering::Relaxed) {
            return Err(crate::utils::ComSrvError::InvalidOperation("Mock stop failure".to_string()));
        }
        self.running.store(false, Ordering::Relaxed);
        Ok(())
    }
    
    async fn status(&self) -> crate::core::protocols::common::combase::ChannelStatus {
        crate::core::protocols::common::combase::ChannelStatus::new(&self.channel_id())
    }
    
    async fn get_all_points(&self) -> Vec<crate::core::protocols::common::combase::PointData> {
        Vec::new()
    }
}

/// Channel entry combining channel and metadata for atomic operations
#[derive(Clone)]
struct ChannelEntry {
    channel: Arc<RwLock<Box<dyn ComBase>>>,
    metadata: ChannelMetadata,
}

/// High-performance protocol factory for creating communication protocol instances
pub struct ProtocolFactory {
    /// Store created channels using DashMap for concurrent access
    /// Now stores ChannelEntry for atomic operations
    channels: DashMap<u16, ChannelEntry, ahash::RandomState>,
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
            protocol_factories: DashMap::with_hasher(ahash::RandomState::new()),
        };
        
        // Register built-in protocol factories by default
        factory.register_builtin_factories();
        factory
    }
    
    /// Register built-in protocol factories
    fn register_builtin_factories(&self) {
        self.register_protocol_factory(Arc::new(ModbusTcpFactory));
        self.register_protocol_factory(Arc::new(ModbusRtuFactory));
        log::info!("Registered built-in protocol factories");
    }
    
    /// Register a protocol factory
    /// 
    /// # Arguments
    /// 
    /// * `factory` - Protocol factory implementation to register
    pub fn register_protocol_factory(&self, factory: Arc<dyn ProtocolClientFactory>) {
        let protocol_type = factory.protocol_type();
        self.protocol_factories.insert(protocol_type.clone(), factory);
        log::info!("Registered protocol factory for {:?}", protocol_type);
    }
    
    /// Unregister a protocol factory for hot-swappable protocol support
    /// 
    /// Removes the factory for the specified protocol type. This enables
    /// runtime plugin management and protocol hot-swapping capabilities.
    /// 
    /// **Important**: This method will fail if there are active channels
    /// using the specified protocol. All channels of this protocol type
    /// must be stopped and removed before unregistering the factory.
    /// 
    /// # Arguments
    /// 
    /// * `protocol_type` - The protocol type to unregister
    /// 
    /// # Returns
    /// 
    /// `Ok(true)` if the factory was found and removed successfully,
    /// `Ok(false)` if no factory was found for the protocol type,
    /// `Err` if there are active channels using this protocol
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::protocol_factory::ProtocolFactory;
    /// use comsrv::core::config::config_manager::ProtocolType;
    /// 
    /// let factory = ProtocolFactory::new();
    /// 
    /// // This will succeed if no channels are using ModbusTcp
    /// match factory.unregister_protocol_factory(&ProtocolType::ModbusTcp) {
    ///     Ok(true) => println!("Factory unregistered successfully"),
    ///     Ok(false) => println!("No factory found for this protocol"),
    ///     Err(e) => println!("Cannot unregister: {}", e),
    /// }
    /// ```
    pub fn unregister_protocol_factory(&self, protocol_type: &ProtocolType) -> Result<bool> {
        // Check if there are any active channels using this protocol
        let active_channels: Vec<u16> = self.channels.iter()
            .filter_map(|entry| {
                let (id, channel_entry) = (entry.key(), entry.value());
                if channel_entry.metadata.protocol_type == *protocol_type {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        
        if !active_channels.is_empty() {
            return Err(ComSrvError::ConfigError(format!(
                "Cannot unregister protocol factory for {:?}: {} active channels found (IDs: {:?}). \
                Please stop and remove all channels of this protocol type first.",
                protocol_type, active_channels.len(), active_channels
            )));
        }
        
        // Safe to remove the factory
        match self.protocol_factories.remove(protocol_type) {
            Some(_) => {
                log::info!("Protocol factory unregistered successfully: {:?}", protocol_type);
                Ok(true)
            },
            None => {
                log::warn!("Attempted to unregister non-existent protocol factory: {:?}", protocol_type);
                Ok(false)
            }
        }
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
            None => {
                // Allow certain protocols that have fallback implementations
                match config.protocol {
                    ProtocolType::Virtual => {
                        // Virtual protocol has basic validation - just check required fields
                        if config.name.is_empty() {
                            return Err(ComSrvError::ConfigError("Channel name cannot be empty".to_string()));
                        }
                        Ok(())
                    },
                    _ => Err(ComSrvError::ProtocolNotSupported(format!(
                        "Protocol type not supported: {:?}", 
                        config.protocol
                    ))),
                }
            }
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
    pub fn get_config_schema(&self, protocol_type: &ProtocolType) -> Option<ConfigValue> {
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
    pub async fn create_protocol(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        self.create_protocol_with_config_manager(config, None).await
    }

    /// Create a protocol instance using registered factories with config manager
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    /// * `config_manager` - Config manager for loading point tables
    pub async fn create_protocol_with_config_manager(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        // First validate the configuration
        self.validate_config(&config)?;
        
        // Try to use registered factory first
        if let Some(factory) = self.protocol_factories.get(&config.protocol) {
            log::info!("Creating protocol instance using registered factory: {:?}", config.protocol);
            return factory.create_client(config, config_manager).await;
        }
        
        // Fallback to legacy implementation for backward compatibility
        match config.protocol {
            ProtocolType::ModbusRtu => self.create_modbus_rtu_with_config_manager(config, config_manager).await,
            ProtocolType::Virtual => self.create_virtual(config).await,
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
    async fn create_modbus_rtu_with_config_manager(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<Box<dyn ComBase>> {
        // Try to use registered factory first
        if let Some(factory) = self.protocol_factories.get(&ProtocolType::ModbusRtu) {
            return factory.create_client(config, config_manager).await;
        }
        
        // Fallback error
        Err(ComSrvError::ProtocolNotSupported(
            "Modbus RTU factory not registered".to_string()
        ))
    }

    // Create virtual channel
    #[inline]
    async fn create_virtual(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        // Create a virtual mock channel for testing purposes
        let mock = MockComBase::new(
            &config.name,
            config.id,
            "Virtual"
        );
        Ok(Box::new(mock) as Box<dyn ComBase>)
    }

    /// Create multiple protocols in parallel for improved performance
    /// 
    /// # Arguments
    /// 
    /// * `configs` - Channel configurations
    pub async fn create_protocols_parallel(&self, configs: Vec<ChannelConfig>) -> Vec<Result<Box<dyn ComBase>>> {
        use futures::future::join_all;
        
        // Fix closure capture issue by creating futures directly
        let futures: Vec<_> = configs.into_iter().map(|config| {
            let fut = self.create_protocol(config);
            fut
        }).collect();
        
        join_all(futures).await
    }
    
    /// Create and register a channel with optimized performance and validation
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub async fn create_channel(&self, config: ChannelConfig) -> Result<()> {
        self.create_channel_with_config_manager(config, None).await
    }

    /// Create and register a channel with config manager support for point table loading
    /// 
    /// Uses atomic insertion to prevent race conditions between channel and metadata
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    /// * `config_manager` - Optional config manager for loading point tables
    pub async fn create_channel_with_config_manager(&self, config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<()> {
        let channel_id = config.id;
        
        // Validate configuration using registered factories
        self.validate_config(&config)?;

        // Create protocol instance with config manager support
        let protocol = self.create_protocol_with_config_manager(config.clone(), config_manager).await?;

        // Create metadata
        let metadata = ChannelMetadata {
            name: config.name.clone(),
            protocol_type: config.protocol.clone(),
            created_at: std::time::Instant::now(),
            last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
        };
      
        let channel_wrapper = Arc::new(RwLock::new(protocol));
        
        // Atomic insertion using DashMap's entry API to prevent race conditions
        let entry = ChannelEntry {
            channel: channel_wrapper,
            metadata,
        };
        
        // Use entry API for atomic operation
        match self.channels.entry(channel_id) {
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                vacant.insert(entry);
                log::info!("Created channel {} with protocol {:?}", channel_id, config.protocol);
                Ok(())
            }
            dashmap::mapref::entry::Entry::Occupied(_) => {
                Err(ComSrvError::ConfigError(format!(
                    "Channel ID already exists: {}",
                    channel_id
                )))
            }
        }
    }
    
    /// Start all channels with improved performance and non-blocking operations
    /// 
    /// Uses snapshot approach to avoid issues with concurrent channel insertion/removal
    /// during the startup process.
    pub async fn start_all_channels(&self) -> Result<()> {
        use futures::future::join_all;
        
        // Take a snapshot of channel IDs to avoid concurrent modification issues
        let channel_ids: Vec<u16> = self.channels.iter()
            .map(|entry| *entry.key())
            .collect();
        
        if channel_ids.is_empty() {
            log::info!("No channels to start");
            return Ok(());
        }
        
        log::info!("Starting all channels with snapshot approach: total_channels={}", channel_ids.len());
        
        let start_futures = channel_ids.into_iter().map(|id| {
            async move {
                // Re-check if channel still exists (might have been removed concurrently)
                if let Some(channel_entry) = self.channels.get(&id) {
                    let mut channel = channel_entry.channel.write().await;
                    match channel.start().await {
                        Ok(_) => {
                            log::info!("Channel started successfully: channel_id={}", id);
                            Ok(())
                        },
                        Err(e) => {
                            log::error!("Failed to start channel {}: {}", id, e);
                            Err(e)
                        }
                    }
                } else {
                    log::debug!("Channel {} was removed during startup, skipping", id);
                    Ok(())
                }
            }
        });
        
        let results = join_all(start_futures).await;
        
        // Collect and report results
        let mut successful_starts = 0;
        let mut failed_starts = 0;
        let mut errors = Vec::new();
        
        for result in results {
            match result {
                Ok(_) => successful_starts += 1,
                Err(e) => {
                    failed_starts += 1;
                    errors.push(e);
                }
            }
        }
        
        log::info!("Channel startup completed: {} successful, {} failed, {} total attempted", 
                   successful_starts, failed_starts, successful_starts + failed_starts);
        
        if failed_starts > 0 {
            let error_msg = format!(
                "Failed to start {} out of {} channels. First error: {}",
                failed_starts,
                successful_starts + failed_starts,
                errors.first().map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string())
            );
            return Err(ComSrvError::InvalidOperation(error_msg));
        }
        
        Ok(())
    }
    
    /// Stop all channels with improved performance and non-blocking cleanup
    pub async fn stop_all_channels(&self) -> Result<()> {
        use futures::future::join_all;
        
        let stop_futures = self.channels.iter().map(|entry| {
            let (id, channel_entry) = (entry.key(), entry.value());
            let id = *id;
            let channel_wrapper = channel_entry.channel.clone();
            
            async move {
                let mut channel = channel_wrapper.write().await;
                match channel.stop().await {
                    Ok(_) => {
                        log::info!("Channel {} stopped successfully", id);
                    },
                    Err(e) => {
                        log::warn!("Channel {}: Failed to stop channel - continuing with other channels: {}", id, e);
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
        let channel_entry = self.channels.get(&id)?;
        
        // Update last accessed time
        *channel_entry.metadata.last_accessed.write().await = std::time::Instant::now();
        
        Some(channel_entry.channel.clone())
    }
    
    /// Get all channels as a vector of (id, channel) pairs
    pub fn get_all_channels(&self) -> Vec<(u16, Arc<RwLock<Box<dyn ComBase>>>)> {
        self.channels.iter().map(|entry| {
            let id = *entry.key();
            let channel = entry.value().channel.clone();
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
            let channel = entry.value().channel.clone();
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

    /// Get channel statistics with real-time protocol counts
    pub async fn get_channel_stats(&self) -> ChannelStats {
        let total_channels = self.channels.len();
        let running_channels = self.running_channel_count().await;
        let mut protocol_counts = AHashMap::new();
        
        // Count channels by protocol from current data
        for entry in self.channels.iter() {
            let metadata = &entry.value().metadata;
            let protocol_name = metadata.protocol_type.as_str();
            *protocol_counts.entry(protocol_name.to_string()).or_insert(0) += 1;
        }
        
        ChannelStats {
            total_channels,
            running_channels,
            protocol_counts,
        }
    }
    
    /// Clean up idle channels with parallel non-blocking operations
    /// 
    /// Removes channels that have been idle for longer than the specified duration.
    /// Uses parallel processing for efficient cleanup of multiple channels.
    /// 
    /// # Arguments
    /// 
    /// * `max_idle_time` - Maximum idle time before a channel is considered for cleanup
    pub async fn cleanup_channels(&self, max_idle_time: std::time::Duration) {
        use futures::future::join_all;
        
        let now = std::time::Instant::now();
        let mut channels_to_remove = Vec::new();
        
        // Identify idle channels
        for entry in self.channels.iter() {
            let (id, channel_entry) = (entry.key(), entry.value());
            let last_accessed = *channel_entry.metadata.last_accessed.read().await;
            
            if now.duration_since(last_accessed) > max_idle_time {
                channels_to_remove.push(*id);
                log::info!("Channel {} marked for cleanup due to inactivity", id);
            }
        }
        
        if channels_to_remove.is_empty() {
            log::debug!("No idle channels found for cleanup");
            return;
        }
        
        // Clone self reference to avoid borrow across await issues
        let channels_ref = &self.channels;
        
        // Stop channels in parallel with proper error handling
        let stop_futures = channels_to_remove.iter().map(|&id| {
            async move {
                if let Some(channel_entry) = channels_ref.get(&id) {
                    let mut channel = channel_entry.channel.write().await;
                    match channel.stop().await {
                        Ok(_) => {
                            log::info!("Channel {} stopped successfully", id);
                            Ok(id)
                        },
                        Err(e) => {
                            log::warn!("Channel {}: Failed to stop - continuing with cleanup: {}", id, e);
                            Err((id, e))
                        }
                    }
                } else {
                    log::debug!("Channel {} already removed during cleanup", id);
                    Ok(id)
                }
            }
        });
        
        let stop_results = join_all(stop_futures).await;
        
        // Remove channels from the map
        let mut successfully_stopped = 0;
        let mut failed_to_stop = 0;
        
        for result in stop_results {
            match result {
                Ok(id) => {
                    self.channels.remove(&id);
                    successfully_stopped += 1;
                    log::debug!("Channel {} removed from factory during cleanup", id);
                },
                Err((id, e)) => {
                    failed_to_stop += 1;
                    log::error!("Failed to cleanup channel {}, keeping in factory: {}", id, e);
                }
            }
        }
        
        log::info!("Channel cleanup completed: {} successfully cleaned, {} failed cleanup", 
                   successfully_stopped, failed_to_stop);
    }
    
    /// Hot update channel configuration
    /// 
    /// Provides seamless configuration updates without service interruption
    /// 
    /// # Arguments
    /// 
    /// * `id` - Channel ID to update
    /// * `new_config` - New configuration to apply
    /// * `config_manager` - Optional config manager for point table loading
    pub async fn update_channel_config(&self, id: u16, new_config: ChannelConfig, config_manager: Option<&ConfigManager>) -> Result<()> {
        // Validate new configuration first
        self.validate_config(&new_config)?;
        
        // Get existing channel
        let channel_entry = self.channels.get(&id)
            .ok_or_else(|| ComSrvError::ChannelError(format!("Channel {} not found", id)))?;
        
        let old_channel = channel_entry.channel.clone();
        
        // Stop the old channel
        {
            let mut channel = old_channel.write().await;
            if let Err(e) = channel.stop().await {
                log::warn!("Failed to stop channel {} during update: {}", id, e);
            }
        }
        
        // Create new protocol instance
        let new_protocol = self.create_protocol_with_config_manager(new_config.clone(), config_manager).await?;
        
        // Create new metadata
        let new_metadata = ChannelMetadata {
            name: new_config.name.clone(),
            protocol_type: new_config.protocol.clone(),
            created_at: std::time::Instant::now(),
            last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
        };
        
        let new_channel_wrapper = Arc::new(RwLock::new(new_protocol));
        
        // Atomic replacement
        let new_entry = ChannelEntry {
            channel: new_channel_wrapper.clone(),
            metadata: new_metadata,
        };
        
        self.channels.insert(id, new_entry);
        
        // Start the new channel
        {
            let mut channel = new_channel_wrapper.write().await;
            if let Err(e) = channel.start().await {
                log::error!("Failed to start updated channel {}: {}", id, e);
                return Err(ComSrvError::ChannelError(format!(
                    "Failed to start updated channel {}: {}", id, e
                )));
            }
        }
        
        log::info!("Successfully updated channel {} configuration", id);
        Ok(())
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
        let parameters = serde_json::json!({
            "address": "127.0.0.1",
            "port": 502,
            "timeout": 5000
        });
        
        // Convert JSON to HashMap<String, serde_yaml::Value> for compatibility
        let param_map = json_to_yaml_params(parameters);
        
        ChannelConfig {
            id,
            name: format!("Test Channel {}", id),
            description: "Test channel configuration".to_string(),
            protocol,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(param_map),
        }
    }

    fn create_modbus_rtu_test_config(id: u16) -> ChannelConfig {
        let parameters = serde_json::json!({
            "port": "/dev/ttyUSB0",
            "baud_rate": 9600,
            "data_bits": 8,
            "stop_bits": 1,
            "parity": "None",
            "slave_id": 1,
            "timeout": 1000,
            "retry_count": 3
        });
        
        // Convert JSON to HashMap<String, serde_yaml::Value> for compatibility
        let param_map = json_to_yaml_params(parameters);
        
        ChannelConfig {
            id,
            name: format!("Modbus RTU Channel {}", id),
            description: "Modbus RTU test channel".to_string(),
            protocol: ProtocolType::ModbusRtu,
            parameters: crate::core::config::config_manager::ChannelParameters::Generic(param_map),
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
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusRtu));
    }
    
    #[test]
    fn test_supported_protocols() {
        let factory = ProtocolFactory::new();
        let protocols = factory.supported_protocols();
        assert!(protocols.contains(&ProtocolType::ModbusTcp));
        assert!(protocols.contains(&ProtocolType::ModbusRtu));
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
        
        let modbus_rtu_config = factory.get_default_config(&ProtocolType::ModbusRtu);
        assert!(modbus_rtu_config.is_some());
        assert_eq!(modbus_rtu_config.unwrap().protocol, ProtocolType::ModbusRtu);
        
        // Unsupported protocol should return None
        let unsupported = factory.get_default_config(&ProtocolType::Can);
        assert!(unsupported.is_none());
    }
    
    #[test]
    fn test_get_config_schema() {
        let factory = ProtocolFactory::new();
        
        let modbus_schema = factory.get_config_schema(&ProtocolType::ModbusTcp);
        assert!(modbus_schema.is_some());
        
        let modbus_rtu_schema = factory.get_config_schema(&ProtocolType::ModbusRtu);
        assert!(modbus_rtu_schema.is_some());
        
        // Unsupported protocol should return None
        let unsupported_schema = factory.get_config_schema(&ProtocolType::Can);
        assert!(unsupported_schema.is_none());
    }

    #[tokio::test]
    async fn test_create_modbus_tcp_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(1, ProtocolType::ModbusTcp);
        
        let result = factory.create_protocol(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_modbus_rtu_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_modbus_rtu_test_config(4);
        
        let result = factory.create_protocol(config).await;
        assert!(result.is_ok(), "Modbus RTU protocol should be supported");
    }

    #[tokio::test]
    async fn test_create_virtual_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(5, ProtocolType::Virtual);
        
        let result = factory.create_protocol(config).await;
        assert!(result.is_ok(), "Virtual protocol should be supported");
        
        // Verify it's a MockComBase instance
        let protocol = result.unwrap();
        let protocol_type = protocol.protocol_type();
        assert_eq!(protocol_type, "Virtual");
    }

    #[tokio::test]
    async fn test_create_channel() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(10, ProtocolType::ModbusTcp);
        
        let result = factory.create_channel(config).await;
        assert!(result.is_ok());
        assert_eq!(factory.channel_count(), 1);
        assert!(!factory.is_empty());
    }

    #[tokio::test]
    async fn test_create_duplicate_channel() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(20, ProtocolType::ModbusTcp);
        let config2 = create_modbus_rtu_test_config(20); // Same ID, different protocol
        
        let result1 = factory.create_channel(config1).await;
        assert!(result1.is_ok());
        
        let result2 = factory.create_channel(config2).await;
        assert!(result2.is_err());
        assert!(matches!(result2.unwrap_err(), ComSrvError::ConfigError(_)));
    }

    #[tokio::test]
    async fn test_concurrent_duplicate_channel_creation() {
        let factory = Arc::new(ProtocolFactory::new());
        let config1 = create_test_channel_config(21, ProtocolType::ModbusTcp);
        let config2 = create_test_channel_config(21, ProtocolType::ModbusRtu);

        let factory_clone = factory.clone();
        let handle1 = tokio::spawn(async move { factory_clone.create_channel(config1).await });
        let factory_clone = factory.clone();
        let handle2 = tokio::spawn(async move { factory_clone.create_channel(config2).await });

        let res1 = handle1.await.unwrap();
        let res2 = handle2.await.unwrap();

        assert!(res1.is_ok() ^ res2.is_ok(), "one creation must fail");
        assert_eq!(factory.channel_count(), 1);
    }

    #[tokio::test]
    async fn test_get_channel() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(30, ProtocolType::ModbusTcp);
        
        factory.create_channel(config).await.unwrap();
        
        let channel = factory.get_channel(30).await;
        assert!(channel.is_some());
        
        let non_existent = factory.get_channel(999).await;
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_get_channel_mut() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(40, ProtocolType::ModbusTcp);
        
        factory.create_channel(config).await.unwrap();
        
        let channel = factory.get_channel_mut(40).await;
        assert!(channel.is_some());
        
        let non_existent = factory.get_channel_mut(999).await;
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_get_all_channels() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(50, ProtocolType::ModbusTcp);
        let config2 = create_modbus_rtu_test_config(51);
        
        factory.create_channel(config1).await.unwrap();
        factory.create_channel(config2).await.unwrap();
        
        let all_channels = factory.get_all_channels();
        assert_eq!(all_channels.len(), 2);
    }

    #[tokio::test]
    async fn test_get_channel_ids() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(60, ProtocolType::ModbusTcp);
        let config2 = create_modbus_rtu_test_config(61);
        
        factory.create_channel(config1).await.unwrap();
        factory.create_channel(config2).await.unwrap();
        
        let ids = factory.get_channel_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&60));
        assert!(ids.contains(&61));
    }

    #[tokio::test]
    async fn test_get_channel_stats() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(70, ProtocolType::ModbusTcp);
        let config2 = create_modbus_rtu_test_config(71);
        let config3 = create_test_channel_config(72, ProtocolType::ModbusTcp);
        
        factory.create_channel(config1).await.unwrap();
        factory.create_channel(config2).await.unwrap();
        factory.create_channel(config3).await.unwrap();
        
        let stats = factory.get_channel_stats().await;
        assert_eq!(stats.total_channels, 3);
        assert_eq!(stats.running_channels, 0); // Channels not started yet

        // Note: We skip starting channels in tests since they would fail without actual devices
        // In a real environment, channels would be started successfully
        
        // Check protocol counts
        assert_eq!(stats.protocol_counts.get("ModbusTcp"), Some(&2));
        assert_eq!(stats.protocol_counts.get("ModbusRtu"), Some(&1));
    }

    #[tokio::test]
    async fn test_create_protocols_parallel() {
        let factory = ProtocolFactory::new();
        let configs = vec![
            create_test_channel_config(80, ProtocolType::ModbusTcp),
            create_modbus_rtu_test_config(81),
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
        
        factory.create_channel(config).await.unwrap();
        
        // Test cleanup with very short idle time (should not remove channels immediately)
        factory.cleanup_channels(std::time::Duration::from_millis(1)).await;
        
        // Channel should still exist
        assert_eq!(factory.channel_count(), 1);
    }

    #[tokio::test]
    async fn test_channel_metadata() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(100, ProtocolType::ModbusTcp);
        
        factory.create_channel(config.clone()).await.unwrap();
        
        // Verify metadata was stored
        let metadata = factory.channels.get(&100).unwrap().metadata.clone();
        assert_eq!(metadata.name, config.name);
        assert_eq!(metadata.protocol_type, config.protocol);
    }

    #[test]
    fn test_default_implementation() {
        let factory = ProtocolFactory::default();
        assert_eq!(factory.channel_count(), 0);
        assert!(factory.is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_channel_creation() {
        let factory = Arc::new(ProtocolFactory::new());
        let config = create_test_channel_config(200, ProtocolType::ModbusTcp);

        let f1 = {
            let factory = factory.clone();
            let cfg = config.clone();
            tokio::spawn(async move { factory.create_channel(cfg).await })
        };

        let f2 = {
            let factory = factory.clone();
            let cfg = config.clone();
            tokio::spawn(async move { factory.create_channel(cfg).await })
        };

        let r1 = f1.await.unwrap();
        let r2 = f2.await.unwrap();

        assert!(r1.is_ok() ^ r2.is_ok(), "only one creation should succeed");
        assert_eq!(factory.channel_count(), 1);
    }

    #[test]
    fn test_unregister_protocol_factory() {
        let factory = ProtocolFactory::new();
        
        // Test unregistering existing factory
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusTcp));
        let removed = factory.unregister_protocol_factory(&ProtocolType::ModbusTcp);
        assert!(removed.is_ok());
        assert!(!factory.is_protocol_supported(&ProtocolType::ModbusTcp));
        
        // Test unregistering non-existent factory
        let removed_again = factory.unregister_protocol_factory(&ProtocolType::ModbusTcp);
        assert!(removed_again.is_ok());
        
        // Test unregistering different protocol
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusRtu));
        let removed_rtu = factory.unregister_protocol_factory(&ProtocolType::ModbusRtu);
        assert!(removed_rtu.is_ok());
        assert!(!factory.is_protocol_supported(&ProtocolType::ModbusRtu));
    }

    #[test]
    fn test_hot_swappable_protocol_factory() {
        let factory = ProtocolFactory::new();
        
        // Verify initial state
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusTcp));
        
        // Unregister and re-register
        let _ = factory.unregister_protocol_factory(&ProtocolType::ModbusTcp);
        assert!(!factory.is_protocol_supported(&ProtocolType::ModbusTcp));
        
        // Re-register with new factory
        let new_factory = Arc::new(ModbusTcpFactory);
        factory.register_protocol_factory(new_factory);
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusTcp));
    }

    #[tokio::test]
    async fn test_mock_combase_functionality() {
        let mut mock = MockComBase::new("test_mock", 999, "Mock");
        
        // Test initial state
        assert_eq!(mock.name(), "test_mock");
        assert_eq!(mock.channel_id(), "999");
        assert_eq!(mock.protocol_type(), "Mock");
        assert!(!mock.is_running().await);
        
        // Test successful start
        assert!(mock.start().await.is_ok());
        assert!(mock.is_running().await);
        
        // Test successful stop
        assert!(mock.stop().await.is_ok());
        assert!(!mock.is_running().await);
        
        // Test start failure
        let mut failing_mock = MockComBase::with_start_failure("failing_mock", 998, "Mock");
        assert!(failing_mock.start().await.is_err());
        assert!(!failing_mock.is_running().await);
        
        // Test stop failure
        let mut stop_failing_mock = MockComBase::with_stop_failure("stop_failing_mock", 997, "Mock");
        assert!(stop_failing_mock.start().await.is_ok());
        assert!(stop_failing_mock.is_running().await);
        assert!(stop_failing_mock.stop().await.is_err());
        // Should still be running after failed stop
        assert!(stop_failing_mock.is_running().await);
    }
    
    #[tokio::test]
    async fn test_start_all_channels_with_mock() {
        let factory = ProtocolFactory::new();
        
        // Create mock channels directly in the factory
        let mock1 = Box::new(MockComBase::new("mock1", 1001, "Mock"));
        let mock2 = Box::new(MockComBase::new("mock2", 1002, "Mock"));
        let mock3 = Box::new(MockComBase::with_start_failure("mock3", 1003, "Mock"));
        
        let entry1 = ChannelEntry {
            channel: Arc::new(RwLock::new(mock1)),
            metadata: ChannelMetadata {
                name: "Mock Channel 1".to_string(),
                protocol_type: ProtocolType::Virtual,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
        };
        
        let entry2 = ChannelEntry {
            channel: Arc::new(RwLock::new(mock2)),
            metadata: ChannelMetadata {
                name: "Mock Channel 2".to_string(),
                protocol_type: ProtocolType::Virtual,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
        };
        
        let entry3 = ChannelEntry {
            channel: Arc::new(RwLock::new(mock3)),
            metadata: ChannelMetadata {
                name: "Mock Channel 3 (Failing)".to_string(),
                protocol_type: ProtocolType::Virtual,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
        };
        
        factory.channels.insert(1001, entry1);
        factory.channels.insert(1002, entry2);
        factory.channels.insert(1003, entry3);
        
        // Test start_all_channels - should fail due to mock3
        let result = factory.start_all_channels().await;
        assert!(result.is_err());
        
        // Verify that successful channels were started
        let channel1 = factory.channels.get(&1001).unwrap();
        assert!(channel1.channel.read().await.is_running().await);
        
        let channel2 = factory.channels.get(&1002).unwrap();
        assert!(channel2.channel.read().await.is_running().await);
        
        // Verify that failing channel was not started
        let channel3 = factory.channels.get(&1003).unwrap();
        assert!(!channel3.channel.read().await.is_running().await);
    }
} 