use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;
use chrono::Utc;
use log::{debug, error, info, warn};
use crate::core::config::config_manager::ChannelConfig;
use crate::core::protocols::common::{ComBase, ComBaseImpl, ChannelStatus, PointData};
use crate::utils::{ComSrvError, Result};
use super::common::{ModbusFunctionCode, ModbusDataType, ModbusRegisterMapping, ByteOrder};

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