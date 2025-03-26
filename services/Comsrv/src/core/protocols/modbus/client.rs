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

/// Modbus客户端抽象接口
///
/// 定义所有Modbus客户端必须实现的方法
#[async_trait]
pub trait ModbusClient: ComBase {
    /// 读取线圈
    async fn read_coils(&self, address: u16, quantity: u16) -> Result<Vec<bool>>;
    
    /// 读取离散输入
    async fn read_discrete_inputs(&self, address: u16, quantity: u16) -> Result<Vec<bool>>;
    
    /// 读取保持寄存器
    async fn read_holding_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>>;
    
    /// 读取输入寄存器
    async fn read_input_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>>;
    
    /// 写入单个线圈
    async fn write_single_coil(&self, address: u16, value: bool) -> Result<()>;
    
    /// 写入单个寄存器
    async fn write_single_register(&self, address: u16, value: u16) -> Result<()>;
    
    /// 写入多个线圈
    async fn write_multiple_coils(&self, address: u16, values: &[bool]) -> Result<()>;
    
    /// 写入多个寄存器
    async fn write_multiple_registers(&self, address: u16, values: &[u16]) -> Result<()>;
    
    /// 读取指定类型的数据
    async fn read_data(&self, mapping: &ModbusRegisterMapping) -> Result<serde_json::Value>;
    
    /// 写入指定类型的数据
    async fn write_data(&self, mapping: &ModbusRegisterMapping, value: &serde_json::Value) -> Result<()>;
}

/// Modbus客户端基础实现
pub struct ModbusClientBase {
    /// 基础通信实现
    pub base: ComBaseImpl,
    /// Modbus设备ID
    slave_id: u8,
    /// 连接超时(毫秒)
    timeout_ms: u64,
    /// 重试次数
    retry_count: u8,
    /// 是否连接
    connected: Arc<RwLock<bool>>,
    /// 寄存器映射
    register_mappings: Arc<RwLock<Vec<ModbusRegisterMapping>>>,
}

impl ModbusClientBase {
    /// 创建新的Modbus客户端基础实现
    pub fn new(name: &str, config: ChannelConfig) -> Self {
        // 从配置中获取设备参数
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
            
        // 创建对象    
        Self {
            base: ComBaseImpl::new(name, &config.protocol.clone(), config),
            slave_id,
            timeout_ms,
            retry_count,
            connected: Arc::new(RwLock::new(false)),
            register_mappings: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// 获取设备ID
    pub fn slave_id(&self) -> u8 {
        self.slave_id
    }
    
    /// 获取超时时间
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
    
    /// 获取重试次数
    pub fn retry_count(&self) -> u8 {
        self.retry_count
    }
    
    /// 获取名称
    pub fn name(&self) -> &str {
        self.base.name()
    }
    
    /// 获取通道ID
    pub fn channel_id(&self) -> &str {
        self.base.channel_id()
    }
    
    /// 获取运行状态
    pub async fn is_running(&self) -> bool {
        self.base.is_running().await
    }
    
    /// 设置运行状态
    pub async fn set_running(&self, running: bool) {
        self.base.set_running(running).await;
    }
    
    /// 获取当前状态
    pub async fn status(&self) -> ChannelStatus {
        let mut status = self.base.status().await;
        status.connected = self.is_connected().await;
        status
    }
    
    /// 设置连接状态
    pub async fn set_connected(&self, connected: bool) {
        let mut c = self.connected.write().await;
        *c = connected;
        
        // 更新通道状态
        let mut status = ChannelStatus::new(self.base.channel_id());
        status.connected = connected;
        status.last_update_time = Utc::now();
        
        if !connected {
            status.last_error = "设备已断开连接".to_string();
        }
    }
    
    /// 获取连接状态
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
    
    /// 设置错误信息
    pub async fn set_error(&self, error: &str) {
        // 更新通道状态
        let mut status = ChannelStatus::new(self.base.channel_id());
        status.connected = false;
        status.last_error = error.to_string();
        status.last_update_time = Utc::now();
    }
    
    /// 加载寄存器映射
    pub async fn load_register_mappings(&self, mappings: Vec<ModbusRegisterMapping>) {
        let mut reg_mappings = self.register_mappings.write().await;
        *reg_mappings = mappings;
    }
    
    /// 获取寄存器映射
    pub async fn get_register_mappings(&self) -> Vec<ModbusRegisterMapping> {
        self.register_mappings.read().await.clone()
    }
    
    /// 根据点ID查找寄存器映射
    pub async fn find_mapping(&self, point_id: &str) -> Option<ModbusRegisterMapping> {
        let mappings = self.register_mappings.read().await;
        for mapping in mappings.iter() {
            if mapping.point_id == point_id {
                return Some(mapping.clone());
            }
        }
        None
    }
    
    /// 获取所有点位实时数据
    pub async fn get_all_points(&self) -> Vec<PointData> {
        let mappings = self.register_mappings.read().await;
        let mut points = Vec::new();
        
        for mapping in mappings.iter() {
            // 创建点位数据对象
            let point_data = PointData {
                id: mapping.point_id.clone(),
                value: serde_json::Value::Null, // 初始化为空，后续在各实现类中填充实际值
                quality: false,
                timestamp: Utc::now(),
            };
            
            points.push(point_data);
        }
        
        // 返回点位列表，实际值会在具体的实现类中填充
        points
    }

    /// 加载点表
    pub async fn load_point_tables(&self, config_dir: &str) -> Result<()> {
        // 获取通道配置
        let channel_config = self.base.config();
        let params = &channel_config.parameters;
        
        // 尝试获取点表配置
        if let Some(point_tables) = params.get("point_tables") {
            if let Some(tables) = point_tables.as_mapping() {
                // 创建点表管理器
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
                
                // 更新点表
                info!("Total loaded {} point mappings", all_mappings.len());
                self.load_register_mappings(all_mappings).await;
                return Ok(());
            }
        }
        
        // 尝试获取内嵌点位配置
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

/// Modbus客户端工厂
/// 
/// 用于根据配置创建不同类型的Modbus客户端
pub struct ModbusClientFactory;

impl ModbusClientFactory {
    /// 创建Modbus客户端
    /// 
    /// 根据配置中的协议类型创建相应的客户端实例
    pub fn create_client(config: ChannelConfig) -> Result<Box<dyn ModbusClient>> {
        // 根据协议类型创建不同的客户端
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