use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_modbus::prelude::*;
use log::{debug, error, info, warn, trace};
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use tracing::{field, Span};
use hex;

use crate::core::config::config_manager::ChannelConfig;
use crate::core::protocols::common::{ComBase, ChannelStatus, PointData};
use crate::utils::{ComSrvError, Result};
use super::client::{ModbusClient, ModbusClientBase};
use super::common::{ModbusRegisterMapping, ModbusDataType, ByteOrder};

/// Modbus TCP客户端实现
pub struct ModbusTcpClient {
    /// 基础Modbus客户端
    base: ModbusClientBase,
    /// TCP地址
    host: String,
    /// TCP端口
    port: u16,
    /// Modbus客户端
    client: Arc<Mutex<Option<client::Context>>>,
}

impl ModbusTcpClient {
    /// 创建新的Modbus TCP客户端
    pub fn new(config: ChannelConfig) -> Self {
        // 获取TCP连接参数
        let params = &config.parameters;
        let host = params.get("host")
            .and_then(|v| if let Some(s) = v.as_str() { Some(s.to_string()) } else { None })
            .unwrap_or("127.0.0.1".to_string());
            
        let port = params.get("port")
            .and_then(|v| v.as_u64())
            .unwrap_or(502) as u16;
            
        Self {
            base: ModbusClientBase::new("ModbusTcpClient", config),
            host,
            port,
            client: Arc::new(Mutex::new(None)),
        }
    }
    
    /// 连接到Modbus TCP服务器
    async fn connect(&self) -> Result<()> {
        if self.base.is_connected().await {
            return Ok(());
        }
        
        // 关闭现有连接
        self.disconnect().await?;
        
        // 构建TCP地址
        let socket_addr = format!("{}:{}", self.host, self.port).parse()
            .map_err(|e| ComSrvError::ConnectionError(format!("Invalid address: {}", e)))?;
            
        debug!("Connecting to Modbus TCP server at {}", socket_addr);
        
        // 创建TCP连接
        let timeout_duration = Duration::from_millis(self.base.timeout_ms());
        let _slave = Slave(self.base.slave_id());
        
        let client_result = timeout(
            timeout_duration,
            tokio_modbus::client::tcp::connect(socket_addr)
        ).await;
        
        match client_result {
            Ok(Ok(client)) => {
                // 连接成功
                let mut c = self.client.lock().await;
                *c = Some(client);
                self.base.set_connected(true).await;
                info!("Connected to Modbus TCP server at {}", socket_addr);
                Ok(())
            },
            Ok(Err(e)) => {
                // 连接错误
                let err_msg = format!("Failed to connect to Modbus TCP server: {}", e);
                error!("{}", err_msg);
                self.base.set_error(&err_msg).await;
                Err(ComSrvError::ConnectionError(err_msg))
            },
            Err(_) => {
                // 连接超时
                let err_msg = format!(
                    "Connection to Modbus TCP server timed out after {} ms",
                    self.base.timeout_ms()
                );
                error!("{}", err_msg);
                self.base.set_error(&err_msg).await;
                Err(ComSrvError::TimeoutError(err_msg))
            }
        }
    }
    
    /// 断开连接
    async fn disconnect(&self) -> Result<()> {
        let mut client = self.client.lock().await;
        if client.is_some() {
            *client = None;
            self.base.set_connected(false).await;
            debug!("Disconnected from Modbus TCP server");
        }
        Ok(())
    }
    
    /// 记录Modbus报文
    fn log_modbus_message(&self, direction: &str, data: &[u8]) {
        let hex_str = hex::encode(data);
        debug!("Modbus TCP {} [{}]: {}", direction, self.host, hex_str);
        // 使用跟踪级别记录详细的报文分析
        if log::log_enabled!(log::Level::Trace) {
            if direction == "TX" && data.len() >= 7 {
                // 解析标准Modbus TCP请求
                let transaction_id = u16::from_be_bytes([data[0], data[1]]);
                let protocol_id = u16::from_be_bytes([data[2], data[3]]);
                let length = u16::from_be_bytes([data[4], data[5]]);
                let unit_id = data[6];
                let function_code = if data.len() > 7 { data[7] } else { 0 };
                
                trace!(
                    "Modbus TCP Request: Transaction ID={}, Protocol ID={}, Length={}, Unit ID={}, Function Code={}",
                    transaction_id, protocol_id, length, unit_id, function_code
                );
                
                // 根据功能码进一步解析
                if data.len() > 9 {
                    match function_code {
                        1 | 2 | 3 | 4 => { // 读取功能
                            let address = u16::from_be_bytes([data[8], data[9]]);
                            let quantity = u16::from_be_bytes([data[10], data[11]]);
                            trace!(
                                "Reading: Starting Address={}, Quantity={}",
                                address, quantity
                            );
                        },
                        5 => { // 写单个线圈
                            let address = u16::from_be_bytes([data[8], data[9]]);
                            let value = u16::from_be_bytes([data[10], data[11]]);
                            trace!(
                                "Write Single Coil: Address={}, Value={}",
                                address, value
                            );
                        },
                        6 => { // 写单个寄存器
                            let address = u16::from_be_bytes([data[8], data[9]]);
                            let value = u16::from_be_bytes([data[10], data[11]]);
                            trace!(
                                "Write Single Register: Address={}, Value={}",
                                address, value
                            );
                        },
                        15 | 16 => { // 写多个线圈/寄存器
                            let address = u16::from_be_bytes([data[8], data[9]]);
                            let quantity = u16::from_be_bytes([data[10], data[11]]);
                            trace!(
                                "Write Multiple: Address={}, Quantity={}",
                                address, quantity
                            );
                        },
                        _ => {
                            trace!("Unknown function code: {}", function_code);
                        }
                    }
                }
            } else if direction == "RX" && data.len() >= 7 {
                // 解析标准Modbus TCP响应
                let transaction_id = u16::from_be_bytes([data[0], data[1]]);
                let protocol_id = u16::from_be_bytes([data[2], data[3]]);
                let length = u16::from_be_bytes([data[4], data[5]]);
                let unit_id = data[6];
                let function_code = if data.len() > 7 { data[7] } else { 0 };
                
                if function_code > 0x80 {
                    // 错误响应
                    let error_code = if data.len() > 8 { data[8] } else { 0 };
                    trace!(
                        "Modbus TCP Error Response: Transaction ID={}, Protocol ID={}, Length={}, Unit ID={}, Error Function={}, Exception Code={}",
                        transaction_id, protocol_id, length, unit_id, function_code, error_code
                    );
                } else {
                    trace!(
                        "Modbus TCP Response: Transaction ID={}, Protocol ID={}, Length={}, Unit ID={}, Function Code={}",
                        transaction_id, protocol_id, length, unit_id, function_code
                    );
                    
                    // 针对不同响应类型进行解析
                    if data.len() > 8 {
                        match function_code {
                            1 | 2 => { // 读线圈/离散量输入响应
                                let byte_count = data[8] as usize;
                                if data.len() >= 9 + byte_count {
                                    let values: Vec<u8> = data[9..9+byte_count].to_vec();
                                    trace!("Read Coils/Discrete Inputs Response: Byte Count={}, Values={:?}", byte_count, values);
                                }
                            },
                            3 | 4 => { // 读保持/输入寄存器响应
                                let byte_count = data[8] as usize;
                                if data.len() >= 9 + byte_count && byte_count % 2 == 0 {
                                    let mut registers = Vec::new();
                                    for i in 0..byte_count/2 {
                                        let idx = 9 + i * 2;
                                        let reg = u16::from_be_bytes([data[idx], data[idx+1]]);
                                        registers.push(reg);
                                    }
                                    trace!("Read Registers Response: Byte Count={}, Registers={:?}", byte_count, registers);
                                }
                            },
                            5 | 6 | 15 | 16 => { // 写响应
                                if data.len() >= 11 {
                                    let address = u16::from_be_bytes([data[8], data[9]]);
                                    let value = u16::from_be_bytes([data[10], data[11]]);
                                    trace!("Write Response: Address={}, Value/Quantity={}", address, value);
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    /// 执行Modbus操作
    async fn execute<F, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce(&mut client::Context) -> Result<T> + Send + Clone + 'static,
        T: Send + 'static,
    {
        let retry_count = self.base.retry_count();
        let mut attempts = 0;
        let mut last_error = None;
        
        while attempts < retry_count {
            // 如果已超过最大重试次数，则返回最后一个错误
            if attempts > 0 {
                info!("Retry {}/{} for Modbus TCP operation", attempts, retry_count);
                // 等待一段时间后重试
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            
            attempts += 1;
            
            // 获取锁并执行操作
            let mut client_guard = self.client.lock().await;
            // 克隆操作闭包，以便可以在不同的重试中重用
            let operation_clone = operation.clone();
            
            if let Some(ref mut client) = *client_guard {
                // 直接在已获取的锁上下文中执行操作
                // 注意：由于tokio_modbus库的封装，我们无法直接获取原始报文
                // 但在底层实现中应该已经记录了报文流
                match operation_clone(client) {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        warn!("Modbus TCP operation failed: {}", e);
                        last_error = Some(e);
                        // 释放锁后再等待
                        drop(client_guard);
                        continue;
                    }
                }
            } else {
                // 客户端未连接，尝试连接
                drop(client_guard);
                match self.connect().await {
                    Ok(_) => continue, // 重新尝试操作
                    Err(e) => {
                        warn!("Failed to connect: {}", e);
                        last_error = Some(e);
                        continue;
                    }
                }
            }
        }
        
        // 所有重试都失败
        Err(last_error.unwrap_or_else(|| ComSrvError::ModbusError("Unknown error in Modbus TCP execute".to_string())))
    }
    
    /// 解析寄存器值为JSON
    fn parse_registers(&self, registers: &[u16], mapping: &ModbusRegisterMapping) -> Result<serde_json::Value> {
        if registers.is_empty() {
            return Err(ComSrvError::ParsingError("Empty register data".to_string()));
        }
        
        let byte_order = &mapping.byte_order;
        
        match mapping.data_type {
            ModbusDataType::Bool => {
                Ok(json!(registers[0] != 0))
            },
            ModbusDataType::Int16 => {
                let value = registers[0] as i16;
                Ok(json!(value))
            },
            ModbusDataType::UInt16 => {
                Ok(json!(registers[0]))
            },
            ModbusDataType::Int32 => {
                if registers.len() < 2 {
                    return Err(ComSrvError::ParsingError(
                        "Insufficient registers for Int32".to_string()
                    ));
                }
                
                let value = match byte_order {
                    ByteOrder::BigEndian => {
                        ((registers[0] as i32) << 16) | (registers[1] as i32)
                    },
                    ByteOrder::LittleEndian => {
                        ((registers[1] as i32) << 16) | (registers[0] as i32)
                    },
                    ByteOrder::BigEndianWordSwapped => {
                        ((registers[1] as i32) << 16) | (registers[0] as i32)
                    },
                    ByteOrder::LittleEndianWordSwapped => {
                        ((registers[0] as i32) << 16) | (registers[1] as i32)
                    },
                };
                
                Ok(json!(value))
            },
            ModbusDataType::UInt32 => {
                if registers.len() < 2 {
                    return Err(ComSrvError::ParsingError(
                        "Insufficient registers for UInt32".to_string()
                    ));
                }
                
                let value = match byte_order {
                    ByteOrder::BigEndian => {
                        ((registers[0] as u32) << 16) | (registers[1] as u32)
                    },
                    ByteOrder::LittleEndian => {
                        ((registers[1] as u32) << 16) | (registers[0] as u32)
                    },
                    ByteOrder::BigEndianWordSwapped => {
                        ((registers[1] as u32) << 16) | (registers[0] as u32)
                    },
                    ByteOrder::LittleEndianWordSwapped => {
                        ((registers[0] as u32) << 16) | (registers[1] as u32)
                    },
                };
                
                Ok(json!(value))
            },
            ModbusDataType::Float32 => {
                if registers.len() < 2 {
                    return Err(ComSrvError::ParsingError(
                        "Insufficient registers for Float32".to_string()
                    ));
                }
                
                // 将两个u16转换为u32，然后解释为f32
                let bits = match byte_order {
                    ByteOrder::BigEndian => {
                        ((registers[0] as u32) << 16) | (registers[1] as u32)
                    },
                    ByteOrder::LittleEndian => {
                        ((registers[1] as u32) << 16) | (registers[0] as u32)
                    },
                    ByteOrder::BigEndianWordSwapped => {
                        ((registers[1] as u32) << 16) | (registers[0] as u32)
                    },
                    ByteOrder::LittleEndianWordSwapped => {
                        ((registers[0] as u32) << 16) | (registers[1] as u32)
                    },
                };
                
                let value = f32::from_bits(bits);
                
                // 应用缩放因子和偏移量
                let scaled_value = if let Some(scale) = mapping.scale_factor {
                    value * scale as f32
                } else {
                    value
                };
                
                let final_value = if let Some(offset) = mapping.offset {
                    scaled_value + offset as f32
                } else {
                    scaled_value
                };
                
                Ok(json!(final_value))
            },
            ModbusDataType::Float64 => {
                if registers.len() < 4 {
                    return Err(ComSrvError::ParsingError(
                        "Insufficient registers for Float64".to_string()
                    ));
                }
                
                // 将四个u16转换为u64，然后解释为f64
                let bits = match byte_order {
                    ByteOrder::BigEndian => {
                        ((registers[0] as u64) << 48) | 
                        ((registers[1] as u64) << 32) |
                        ((registers[2] as u64) << 16) |
                        (registers[3] as u64)
                    },
                    ByteOrder::LittleEndian => {
                        ((registers[3] as u64) << 48) | 
                        ((registers[2] as u64) << 32) |
                        ((registers[1] as u64) << 16) |
                        (registers[0] as u64)
                    },
                    ByteOrder::BigEndianWordSwapped => {
                        ((registers[1] as u64) << 48) | 
                        ((registers[0] as u64) << 32) |
                        ((registers[3] as u64) << 16) |
                        (registers[2] as u64)
                    },
                    ByteOrder::LittleEndianWordSwapped => {
                        ((registers[2] as u64) << 48) | 
                        ((registers[3] as u64) << 32) |
                        ((registers[0] as u64) << 16) |
                        (registers[1] as u64)
                    },
                };
                
                let value = f64::from_bits(bits);
                
                // 应用缩放因子和偏移量
                let scaled_value = if let Some(scale) = mapping.scale_factor {
                    value * scale
                } else {
                    value
                };
                
                let final_value = if let Some(offset) = mapping.offset {
                    scaled_value + offset
                } else {
                    scaled_value
                };
                
                Ok(json!(final_value))
            },
            ModbusDataType::String(_) => {
                // 从寄存器构建字符串
                let mut bytes = Vec::with_capacity(registers.len() * 2);
                
                for register in registers {
                    // 根据字节序添加字节
                    match byte_order {
                        ByteOrder::BigEndian | ByteOrder::BigEndianWordSwapped => {
                            bytes.push((register >> 8) as u8);
                            bytes.push((register & 0xFF) as u8);
                        },
                        ByteOrder::LittleEndian | ByteOrder::LittleEndianWordSwapped => {
                            bytes.push((register & 0xFF) as u8);
                            bytes.push((register >> 8) as u8);
                        },
                    }
                }
                
                // 转换为字符串，去除空字节
                let valid_bytes: Vec<u8> = bytes.into_iter()
                    .take_while(|&b| b != 0)
                    .collect();
                
                let string_value = String::from_utf8_lossy(&valid_bytes).to_string();
                
                Ok(json!(string_value))
            },
        }
    }

    /// 获取客户端上下文
    async fn get_client(&self) -> Result<Arc<Mutex<Option<client::Context>>>> {
        // 确保已连接
        self.connect().await?;
        
        // 返回Arc克隆，而不是引用，以避免生命周期问题
        Ok(self.client.clone())
    }
}

#[async_trait]
impl ComBase for ModbusTcpClient {
    fn name(&self) -> &str {
        self.base.name()
    }
    
    fn channel_id(&self) -> &str {
        self.base.channel_id()
    }
    
    fn protocol_type(&self) -> &str {
        "modbus_tcp"
    }
    
    fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("protocol".to_string(), "modbus_tcp".to_string());
        params.insert("host".to_string(), self.host.clone());
        params.insert("port".to_string(), self.port.to_string());
        params.insert("slave_id".to_string(), self.base.slave_id().to_string());
        params.insert("timeout_ms".to_string(), self.base.timeout_ms().to_string());
        params.insert("retry_count".to_string(), self.base.retry_count().to_string());
        params
    }
    
    async fn is_running(&self) -> bool {
        self.base.is_running().await
    }
    
    async fn start(&mut self) -> Result<()> {
        info!("启动Modbus TCP客户端: {}:{}", self.host, self.port);
        
        // 设置运行状态
        self.base.set_running(true).await;
        
        // 连接到服务器
        match self.connect().await {
            Ok(_) => {
                info!("Modbus TCP连接成功: {}:{}", self.host, self.port);
                
                // 加载点表
                let config_path = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "./config".to_string());
                if let Err(e) = self.base.load_point_tables(&config_path).await {
                    error!("加载点表失败: {}", e);
                }
                
                Ok(())
            },
            Err(e) => {
                error!("Modbus TCP连接失败: {}", e);
                Err(e)
            }
        }
    }
    
    async fn stop(&mut self) -> Result<()> {
        info!("停止Modbus TCP客户端: {}:{} (通道: {})", self.host, self.port, self.channel_id());
        self.disconnect().await?;
        self.base.set_running(false).await;
        Ok(())
    }
    
    async fn status(&self) -> ChannelStatus {
        self.base.status().await
    }
    
    async fn get_all_points(&self) -> Vec<PointData> {
        let mut points = self.base.get_all_points().await;
        
        // 确保已连接，如果未连接则重试连接
        if !self.base.is_connected().await {
            if let Err(e) = self.connect().await {
                error!("Failed to connect to Modbus TCP server: {}", e);
                return points; // 连接失败，返回空值点位
            }
        }
        
        // 对每个点位，尝试读取其实际值
        for point in &mut points {
            // 查找对应的寄存器映射
            if let Some(mapping) = self.base.find_mapping(&point.id).await {
                // 尝试读取该点位的值
                match self.read_data(&mapping).await {
                    Ok(value) => {
                        point.value = value;
                        point.quality = true;
                        point.timestamp = Utc::now();
                    },
                    Err(e) => {
                        // 读取失败，记录错误但保持默认值
                        error!("Failed to read point {}: {}", point.id, e);
                        point.quality = false;
                    }
                }
            } else {
                // 如果找不到映射，记录警告
                warn!("No mapping found for point: {}", point.id);
            }
        }
        
        points
    }
}

#[async_trait]
impl ModbusClient for ModbusTcpClient {
    async fn read_coils(&self, address: u16, quantity: u16) -> Result<Vec<bool>> {
        debug!("Reading {} coils from address {}", quantity, address);
        
        self.execute(move |ctx: &mut client::Context| {
            // 使用Ok来包装异步操作返回的结果
            let fut = ctx.read_coils(address, quantity);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to read coils: {}", e)))?)
        }).await
    }
    
    async fn read_discrete_inputs(&self, address: u16, quantity: u16) -> Result<Vec<bool>> {
        debug!("Reading {} discrete inputs from address {}", quantity, address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.read_discrete_inputs(address, quantity);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to read discrete inputs: {}", e)))?)
        }).await
    }
    
    async fn read_holding_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>> {
        debug!("Reading {} holding registers from address {}", quantity, address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.read_holding_registers(address, quantity);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to read holding registers: {}", e)))?)
        }).await
    }
    
    async fn read_input_registers(&self, address: u16, quantity: u16) -> Result<Vec<u16>> {
        debug!("Reading {} input registers from address {}", quantity, address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.read_input_registers(address, quantity);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to read input registers: {}", e)))?)
        }).await
    }
    
    async fn write_single_coil(&self, address: u16, value: bool) -> Result<()> {
        debug!("Writing coil at address {} with value {}", address, value);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.write_single_coil(address, value);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to write single coil: {}", e)))?)
        }).await
    }
    
    async fn write_single_register(&self, address: u16, value: u16) -> Result<()> {
        debug!("Writing register at address {} with value {}", address, value);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.write_single_register(address, value);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to write single register: {}", e)))?)
        }).await
    }
    
    async fn write_multiple_coils(&self, address: u16, values: &[bool]) -> Result<()> {
        let values = values.to_vec(); // Clone for move
        debug!("Writing {} coils at address {}", values.len(), address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.write_multiple_coils(address, &values);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to write multiple coils: {}", e)))?)
        }).await
    }
    
    async fn write_multiple_registers(&self, address: u16, values: &[u16]) -> Result<()> {
        let values = values.to_vec(); // Clone for move
        debug!("Writing {} registers at address {}", values.len(), address);
        
        self.execute(move |ctx: &mut client::Context| {
            let fut = ctx.write_multiple_registers(address, &values);
            Ok(tokio::runtime::Handle::current().block_on(fut)
                .map_err(|e| ComSrvError::ModbusError(format!("Failed to write multiple registers: {}", e)))?)
        }).await
    }
    
    async fn read_data(&self, mapping: &ModbusRegisterMapping) -> Result<serde_json::Value> {
        let address = mapping.address;
        let quantity = mapping.quantity;
        
        debug!(
            "Reading data from address {} with quantity {} and type {:?}",
            address, quantity, mapping.data_type
        );
        
        // 根据数据类型选择读取方法
        match mapping.data_type {
            ModbusDataType::Bool => {
                // 布尔类型读取线圈
                let values = self.read_coils(address, 1).await?;
                if values.is_empty() {
                    return Err(ComSrvError::ModbusError("No coil data received".to_string()));
                }
                Ok(json!(values[0]))
            },
            _ => {
                // 其他类型读取保持寄存器
                let registers = self.read_holding_registers(address, quantity).await?;
                self.parse_registers(&registers, mapping)
            }
        }
    }
    
    async fn write_data(&self, mapping: &ModbusRegisterMapping, value: &serde_json::Value) -> Result<()> {
        if !mapping.writable {
            return Err(ComSrvError::InvalidOperation(
                format!("Register at address {} is not writable", mapping.address)
            ));
        }
        
        let address = mapping.address;
        
        // 根据数据类型执行写入
        match mapping.data_type {
            ModbusDataType::Bool => {
                let bool_value = value.as_bool().ok_or_else(|| {
                    ComSrvError::InvalidParameter(
                        format!("Expected boolean value, got: {}", value)
                    )
                })?;
                
                self.write_single_coil(address, bool_value).await
            },
            ModbusDataType::Int16 | ModbusDataType::UInt16 => {
                let int_value = if value.is_i64() {
                    value.as_i64().unwrap() as u16
                } else if value.is_u64() {
                    value.as_u64().unwrap() as u16
                } else if value.is_f64() {
                    value.as_f64().unwrap() as u16
                } else {
                    return Err(ComSrvError::InvalidParameter(
                        format!("Expected numeric value, got: {}", value)
                    ));
                };
                
                self.write_single_register(address, int_value).await
            },
            _ => {
                // 对于复杂类型暂不支持
                Err(ComSrvError::ProtocolNotSupported(
                    format!("Writing {:?} data type is not implemented yet", mapping.data_type)
                ))
            }
        }
    }
} 