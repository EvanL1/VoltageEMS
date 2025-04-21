use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;
use chrono::Utc;
use log::{debug, error, info, warn};
use crate::core::config::config_manager::ChannelConfig;
use crate::core::protocols::common::{ComBase, ComBaseImpl, ChannelStatus, PointData};
use crate::utils::{ComSrvError, Result};
use super::common::{ModbusFunctionCode, ModbusDataType, ModbusRegisterMapping, ByteOrder};
use crate::core::config::PointTableManager;
use std::path::Path;
use std::collections::HashMap;
use serde_yaml;

/// Modbus client abstract interface
///
/// Define all methods that Modbus clients must implement
#[async_trait]
pub trait ModbusClient: ComBase {
    /// Read coils
    async fn read_coils(&self, address: u16, quantity: u16) -> Result<Vec<bool>>;
    
    /// Read discrete inputs
    async fn read_discrete_inputs(&self, address: u16, quantity: u16) -> Result<Vec<bool>>;
    
    /// Read holding registers
    async fn read_holding_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>>;
    
    /// Read input registers
    async fn read_input_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>>;
    
    /// Write single coil
    async fn write_single_coil(&self, address: u16, value: bool) -> Result<()>;
    
    /// Write single register
    async fn write_single_register(&self, address: u16, value: u16) -> Result<()>;
    
    /// Write multiple coils
    async fn write_multiple_coils(&self, address: u16, values: &[bool]) -> Result<()>;
    
    /// Write multiple registers
    async fn write_multiple_registers(&self, address: u16, values: &[u16]) -> Result<()>;
    
    /// Read data of specified type
    async fn read_data(&self, mapping: &ModbusRegisterMapping) -> Result<serde_json::Value>;
    
    /// Write data of specified type
    async fn write_data(&self, mapping: &ModbusRegisterMapping, value: &serde_json::Value) -> Result<()>;
}

/// Modbus client base implementation
pub struct ModbusClientBase {
    /// Base communication implementation
    pub base: ComBaseImpl,
    /// Modbus device ID
    slave_id: u8,
    /// Connection timeout (milliseconds)
    timeout_ms: u64,
    /// Retry count
    retry_count: u8,
    /// Whether connected
    connected: Arc<RwLock<bool>>,
    /// Register mappings
    register_mappings: Arc<RwLock<Vec<ModbusRegisterMapping>>>,
}

impl ModbusClientBase {
    /// Create a new Modbus client base implementation
    pub fn new(name: &str, config: ChannelConfig) -> Self {
        // Get device parameters from configuration
        let params = &config.parameters;
        let slave_id = params.get("slave_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u8;
            
        let timeout_ms = params.get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);
            
        let retry_count = params.get("retry_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as u8;
            
        // Create object    
        Self {
            base: ComBaseImpl::new(name, &config.protocol.clone(), config),
            slave_id,
            timeout_ms,
            retry_count,
            connected: Arc::new(RwLock::new(false)),
            register_mappings: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Get device ID
    pub fn slave_id(&self) -> u8 {
        self.slave_id
    }
    
    /// Get timeout time
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
    
    /// Get retry count
    pub fn retry_count(&self) -> u8 {
        self.retry_count
    }
    
    /// Get name
    pub fn name(&self) -> &str {
        self.base.name()
    }
    
    /// Get channel ID
    pub fn channel_id(&self) -> &str {
        self.base.channel_id()
    }
    
    /// Get running status
    pub async fn is_running(&self) -> bool {
        self.base.is_running().await
    }
    
    /// Set running status
    pub async fn set_running(&self, running: bool) {
        self.base.set_running(running).await;
    }
    
    /// Get current status
    pub async fn status(&self) -> ChannelStatus {
        let mut status = self.base.status().await;
        status.connected = self.is_connected().await;
        status
    }
    
    /// Set connected status
    pub async fn set_connected(&self, connected: bool) {
        let mut c = self.connected.write().await;
        *c = connected;
        
        // Update channel status
        let mut status = ChannelStatus::new(self.base.channel_id());
        status.connected = connected;
        status.last_update_time = Utc::now();
        
        if !connected {
            status.last_error = "Device disconnected".to_string();
        }
    }
    
    /// Get connected status
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
    
    /// Set error information
    pub async fn set_error(&self, error: &str) {
        // Update channel status
        let mut status = ChannelStatus::new(self.base.channel_id());
        status.connected = false;
        status.last_error = error.to_string();
        status.last_update_time = Utc::now();
    }
    
    /// Load register mappings
    pub async fn load_register_mappings(&self, mappings: Vec<ModbusRegisterMapping>) {
        let mut reg_mappings = self.register_mappings.write().await;
        *reg_mappings = mappings;
    }
    
    /// Get register mappings
    pub async fn get_register_mappings(&self) -> Vec<ModbusRegisterMapping> {
        self.register_mappings.read().await.clone()
    }
    
    /// Find mapping by point ID
    pub async fn find_mapping(&self, point_id: &str) -> Option<ModbusRegisterMapping> {
        let mappings = self.register_mappings.read().await;
        for mapping in mappings.iter() {
            if mapping.point_id == point_id {
                return Some(mapping.clone());
            }
        }
        None
    }
    
    /// Get all points real-time data
    pub async fn get_all_points(&self) -> Vec<PointData> {
        let mappings = self.register_mappings.read().await;
        let mut points = Vec::new();
        
        for mapping in mappings.iter() {
            // Create point data object
            let point_data = PointData {
                id: mapping.point_id.clone(),
                value: serde_json::Value::Null, // Initialize to null, fill actual value in the specific implementation class
                quality: false,
                timestamp: Utc::now(),
            };
            
            points.push(point_data);
        }
        
        // Return point list, actual value will be filled in the specific implementation class
        points
    }

    /// Load point tables
    pub async fn load_point_tables(&self, config_dir: &str) -> Result<()> {
        // Get channel configuration
        let channel_config = self.base.config();
        let params = &channel_config.parameters;
        
        // Try to get point table configuration
        if let Some(point_tables) = params.get("point_tables") {
            if let Some(tables) = point_tables.as_mapping() {
                // Create point table manager
                let point_table_manager = PointTableManager::new(config_dir);
                let mut all_mappings = Vec::new();
                
                // Load all point tables
                for (table_name, path) in tables {
                    if let Some(path_str) = path.as_str() {
                        let table_name_str = if let Some(s) = table_name.as_str() {
                            s
                        } else {
                            "unknown"
                        };
                        
                        info!("Loading point table {}: {}", table_name_str, path_str);
                        match point_table_manager.load_point_table(path_str) {
                            Ok(mappings) => {
                                info!("Successfully loaded point table {}: {} points", table_name_str, mappings.len());
                                // Add all mappings
                                all_mappings.extend(mappings);
                            },
                            Err(e) => {
                                error!("Failed to load point table {}: {}", table_name_str, e);
                                return Err(e);
                            }
                        }
                    }
                }
                
                // Update point table
                info!("Total loaded {} point mappings", all_mappings.len());
                self.load_register_mappings(all_mappings).await;
                return Ok(());
            }
        }
        
        // Try to get embedded point configuration
        if let Some(points) = params.get("points") {
            if let Some(points_array) = points.as_sequence() {
                let mut mappings = Vec::new();
                
                for point in points_array {
                    if let Some(point_obj) = point.as_mapping() {
                        // Extract required fields
                        let id = point_obj.get(&serde_yaml::Value::String("id".to_string()))
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ComSrvError::ConfigError("Point missing required 'id' field".to_string())
                            })?;
                        
                        let address = point_obj.get(&serde_yaml::Value::String("address".to_string()))
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| {
                                ComSrvError::ConfigError(format!("Point {} missing required 'address' field", id))
                            })? as u16;
                        
                        let point_type = point_obj.get(&serde_yaml::Value::String("type".to_string()))
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ComSrvError::ConfigError(format!("Point {} missing required 'type' field", id))
                            })?;
                        
                        // Determine data type and quantity
                        let (data_type, quantity) = match point_type {
                            "coil" | "discrete_input" => (ModbusDataType::Bool, 1),
                            "holding_register" | "input_register" => {
                                let dt = point_obj.get(&serde_yaml::Value::String("data_type".to_string()))
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| {
                                        ComSrvError::ConfigError(format!("Register point {} missing required 'data_type' field", id))
                                    })?;
                                
                                match dt {
                                    "bool" => (ModbusDataType::Bool, 1),
                                    "int16" => (ModbusDataType::Int16, 1),
                                    "uint16" => (ModbusDataType::UInt16, 1),
                                    "int32" => (ModbusDataType::Int32, 2),
                                    "uint32" => (ModbusDataType::UInt32, 2),
                                    "float32" => (ModbusDataType::Float32, 2),
                                    "float64" => (ModbusDataType::Float64, 4),
                                    s if s.starts_with("string") => {
                                        let len = s.trim_start_matches("string")
                                            .trim_start_matches(|c: char| !c.is_digit(10))
                                            .parse::<usize>()
                                            .unwrap_or(10);
                                        let registers = (len + 1) / 2;
                                        (ModbusDataType::String(len), registers as u16)
                                    },
                                    _ => return Err(ComSrvError::ConfigError(format!(
                                        "Point {} has unsupported data_type: {}", id, dt
                                    ))),
                                }
                            },
                            _ => return Err(ComSrvError::ConfigError(format!(
                                "Point {} has unsupported type: {}", id, point_type
                            ))),
                        };
                        
                        // Writability
                        let writable = point_obj.get(&serde_yaml::Value::String("writable".to_string()))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                            
                        // Byte order
                        let byte_order = match point_obj.get(&serde_yaml::Value::String("byte_order".to_string())).and_then(|v| v.as_str()) {
                            Some("ABCD") => ByteOrder::BigEndian,
                            Some("DCBA") => ByteOrder::LittleEndian,
                            Some("BADC") => ByteOrder::BigEndianWordSwapped,
                            Some("CDAB") => ByteOrder::LittleEndianWordSwapped,
                            _ => ByteOrder::BigEndian, // Default
                        };
                        
                        // Scale factor
                        let scale_factor = point_obj.get(&serde_yaml::Value::String("scale".to_string()))
                            .and_then(|v| v.as_f64());
                            
                        // Offset
                        let offset = point_obj.get(&serde_yaml::Value::String("offset".to_string()))
                            .and_then(|v| v.as_f64());
                            
                        // Create mapping
                        let mapping = ModbusRegisterMapping {
                            point_id: id.to_string(),
                            address,
                            quantity,
                            data_type,
                            writable,
                            byte_order,
                            scale_factor,
                            offset,
                        };
                        
                        mappings.push(mapping);
                    }
                }
                
                // Update point table
                info!("Loaded {} point mappings from embedded configuration", mappings.len());
                self.load_register_mappings(mappings).await;
                return Ok(());
            }
        }
        
        // If no point table configuration is found
        warn!("No point table configuration found");
        Ok(())
    }
}

/// Modbus client factory
/// 
/// Used to create different types of Modbus clients based on configuration
pub struct ModbusClientFactory;

impl ModbusClientFactory {
    /// Create Modbus client
    /// 
    /// Create a client instance based on the protocol type in the configuration
    pub fn create_client(config: ChannelConfig) -> Result<Box<dyn ModbusClient>> {
        // Create different clients based on the protocol type
        let client: Box<dyn ModbusClient> = match config.protocol.as_str() {
            "modbus_tcp" => {
                info!("Creating Modbus TCP client for channel: {}", config.id);
                Box::new(super::tcp::ModbusTcpClient::new(config))
            },
            "modbus_rtu" => {
                info!("Creating Modbus RTU client for channel: {}", config.id);
                Box::new(super::rtu::ModbusRtuClient::new(config))
            },
            _ => {
                return Err(ComSrvError::ProtocolNotSupported(format!(
                    "Unsupported Modbus protocol type: {}",
                    config.protocol
                )));
            }
        };
        
        Ok(client)
    }
} 