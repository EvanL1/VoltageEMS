//! Modbus协议引擎
//!
//! 这个模块提供了优化的Modbus协议处理引擎，包含：
//! - 零拷贝数据处理
//! - 批量请求优化
//! - 智能缓存机制
//! - 并发请求管理

use serde_json;
use std::collections::HashMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn};

use crate::core::protocols::common::combase::transport_bridge::UniversalTransportBridge;
use crate::core::protocols::common::data_types::PointData;
use crate::core::protocols::modbus::{
    common::{ModbusConfig, ModbusFunctionCode},
    frame::{ModbusFrameProcessor, ModbusMode},
    pdu::ModbusPduProcessor,
};
// 简化的映射类型定义 - 替代已删除的复杂trait系统
#[derive(Debug, Clone)]
pub struct ModbusTelemetryMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub address: u16,
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
}

#[derive(Debug, Clone)]
pub struct ModbusSignalMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub address: u16,
    pub bit_location: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct ModbusAdjustmentMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub address: u16,
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
}

#[derive(Debug, Clone)]
pub struct ModbusControlMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub address: u16,
    pub bit_location: Option<u8>,
    pub coil_number: Option<u16>,
}
use crate::utils::error::{ComSrvError, Result};

/// 批量请求信息
#[derive(Debug, Clone)]
pub struct BatchRequest {
    pub slave_id: u8,
    pub function_code: ModbusFunctionCode,
    pub start_address: u16,
    pub quantity: u16,
    pub point_ids: Vec<u32>,
}

/// 请求缓存项
#[derive(Debug, Clone)]
pub struct CacheItem {
    pub data: Vec<u8>,
    pub timestamp: Instant,
    pub ttl: Duration,
}

impl CacheItem {
    pub fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > self.ttl
    }
}

/// 协议引擎配置
#[derive(Debug, Clone)]
pub struct ProtocolEngineConfig {
    pub max_concurrent_requests: usize,
    pub batch_optimization: bool,
    pub cache_enabled: bool,
    pub cache_ttl: Duration,
    pub max_cache_size: usize,
}

impl Default for ProtocolEngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            batch_optimization: true,
            cache_enabled: true,
            cache_ttl: Duration::from_millis(500),
            max_cache_size: 1000,
        }
    }
}

/// Modbus协议引擎
pub struct ModbusProtocolEngine {
    /// PDU处理器
    pdu_processor: Arc<ModbusPduProcessor>,
    /// 帧处理器
    frame_processor: Arc<RwLock<ModbusFrameProcessor>>,
    /// 并发控制
    semaphore: Arc<Semaphore>,
    /// 事务ID计数器
    transaction_id: Arc<std::sync::atomic::AtomicU16>,
    /// 请求缓存
    cache: Arc<RwLock<HashMap<String, CacheItem>>>,
    /// 引擎配置
    config: ProtocolEngineConfig,
    /// 性能统计
    stats: Arc<RwLock<EngineStats>>,
    /// 通道ID（用于日志）
    channel_id: Option<u16>,
    /// 通道名称（用于日志）
    channel_name: Option<String>,
    /// 通道日志文件句柄
    channel_log_file: Option<Arc<RwLock<std::fs::File>>>,
}

/// 引擎性能统计
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub batch_optimizations: u64,
    pub concurrent_requests: u64,
    pub zero_copy_operations: u64,
}

impl ModbusProtocolEngine {
    /// 创建新的协议引擎
    pub async fn new(modbus_config: &ModbusConfig) -> Result<Self> {
        tracing::debug!(
            "Creating ModbusProtocolEngine with protocol_type: {}",
            modbus_config.protocol_type
        );
        let mode = if modbus_config.is_tcp() {
            tracing::debug!("Selected ModbusMode::Tcp");
            ModbusMode::Tcp
        } else {
            tracing::debug!("Selected ModbusMode::Rtu");
            ModbusMode::Rtu
        };

        let config = ProtocolEngineConfig::default();
        let pdu_processor = Arc::new(ModbusPduProcessor::new());
        let frame_processor = Arc::new(RwLock::new(ModbusFrameProcessor::new(mode)));
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));

        Ok(Self {
            pdu_processor,
            frame_processor,
            semaphore,
            transaction_id: Arc::new(std::sync::atomic::AtomicU16::new(1)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(EngineStats::default())),
            channel_id: None,
            channel_name: None,
            channel_log_file: None,
        })
    }

    /// 设置通道信息（用于日志）
    pub fn set_channel_info(&mut self, channel_id: u16, channel_name: String) {
        self.channel_id = Some(channel_id);
        self.channel_name = Some(channel_name.clone());

        // 创建通道日志文件
        let log_dir = Path::new("logs").join(&channel_name);
        if let Err(e) = create_dir_all(&log_dir) {
            warn!(
                "Failed to create channel log directory {:?}: {}",
                log_dir, e
            );
            return;
        }

        let log_file_path = log_dir.join(format!("channel_{}.log", channel_id));
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
        {
            Ok(file) => {
                self.channel_log_file = Some(Arc::new(RwLock::new(file)));
                info!("Created channel log file: {:?}", log_file_path);
            }
            Err(e) => {
                warn!(
                    "Failed to create channel log file {:?}: {}",
                    log_file_path, e
                );
            }
        }
    }

    /// 写入通道日志
    async fn write_channel_log(&self, log_entry: &str) {
        if let Some(log_file) = &self.channel_log_file {
            let mut file = log_file.write().await;
            let _ = writeln!(file, "{}", log_entry);
            let _ = file.flush();
        }
    }

    /// 读取遥测点位
    pub async fn read_telemetry_point(
        &self,
        mapping: &ModbusTelemetryMapping,
        transport: &UniversalTransportBridge,
    ) -> Result<PointData> {
        // 遥测点默认使用功能码03（读保持寄存器）
        let function_code = ModbusFunctionCode::Read03;

        // 根据数据类型计算寄存器数量
        let register_count = match mapping.data_type.as_str() {
            "float32" | "int32" | "uint32" => 2,
            "float64" | "int64" | "uint64" => 4,
            _ => 1, // int16, uint16
        };

        let response_data = self
            .send_optimized_request(
                mapping.slave_id,
                function_code,
                mapping.address,
                register_count,
                transport,
            )
            .await?;

        let value = self.parse_telemetry_value(&response_data, mapping)?;

        Ok(PointData {
            id: mapping.point_id.to_string(),
            name: format!("Telemetry_Point_{}", mapping.point_id),
            value: value.to_string(),
            timestamp: chrono::Utc::now(),
            unit: String::new(),
            description: format!("Modbus telemetry, address: {}", mapping.address),
            telemetry_type: None,
            channel_id: None,
        })
    }

    /// 读取遥信点位
    pub async fn read_signal_point(
        &self,
        mapping: &ModbusSignalMapping,
        transport: &UniversalTransportBridge,
    ) -> Result<PointData> {
        // 遥信点默认使用功能码01（读线圈）
        let function_code = ModbusFunctionCode::Read01;

        let quantity = 1; // 读取一个位
        let response_data = self
            .send_optimized_request(
                mapping.slave_id,
                function_code,
                mapping.address,
                quantity,
                transport,
            )
            .await?;

        let value = self.parse_signal_value(&response_data, mapping)?;

        Ok(PointData {
            id: mapping.point_id.to_string(),
            name: format!("Signal_Point_{}", mapping.point_id),
            value: value.to_string(),
            timestamp: chrono::Utc::now(),
            unit: String::new(),
            description: format!(
                "Modbus signal, address: {}, bit: {}",
                mapping.address,
                mapping.bit_location.unwrap_or(0)
            ),
            telemetry_type: None,
            channel_id: None,
        })
    }

    /// 写入遥调点位
    pub async fn write_adjustment_point(
        &self,
        mapping: &ModbusAdjustmentMapping,
        value: f64,
        transport: &UniversalTransportBridge,
    ) -> Result<()> {
        let write_data = self.convert_adjustment_value(value, mapping)?;

        // 根据数据类型决定使用单寄存器还是多寄存器写入
        let register_count = match mapping.data_type.as_str() {
            "float32" | "int32" | "uint32" => 2,
            "float64" | "int64" | "uint64" => 4,
            _ => 1, // int16, uint16
        };

        if register_count == 1 {
            let register_value = u16::from_be_bytes([write_data[0], write_data[1]]);
            self.send_write_single_register(
                mapping.slave_id,
                mapping.address,
                register_value,
                transport,
            )
            .await?;
        } else {
            self.send_write_multiple_registers(
                mapping.slave_id,
                mapping.address,
                &write_data,
                transport,
            )
            .await?;
        }

        info!(
            "Successfully wrote adjustment point {}: {}",
            mapping.point_id, value
        );
        Ok(())
    }

    /// 执行遥控操作
    pub async fn execute_control_point(
        &self,
        mapping: &ModbusControlMapping,
        command: bool,
        transport: &UniversalTransportBridge,
    ) -> Result<()> {
        // 遥控默认使用功能码05（写单个线圈）
        if mapping.coil_number.is_some() {
            // 如果有线圈号，使用写单个线圈
            self.send_write_single_coil(mapping.slave_id, mapping.address, command, transport)
                .await?;
        } else {
            // 否则写寄存器
            let value = if command { 1u16 } else { 0u16 };
            self.send_write_single_register(mapping.slave_id, mapping.address, value, transport)
                .await?;
        }

        info!(
            "Successfully executed control point {}: {}",
            mapping.point_id, command
        );
        Ok(())
    }

    /// 优化的请求发送（包含缓存和并发控制）
    pub async fn send_optimized_request(
        &self,
        slave_id: u8,
        function_code: ModbusFunctionCode,
        address: u16,
        quantity: u16,
        transport: &UniversalTransportBridge,
    ) -> Result<Vec<u8>> {
        // 生成缓存键
        let cache_key = format!(
            "{}:{}:{}:{}",
            slave_id,
            u8::from(function_code),
            address,
            quantity
        );

        // 检查缓存
        if self.config.cache_enabled {
            let cache = self.cache.read().await;
            if let Some(item) = cache.get(&cache_key) {
                if !item.is_expired() {
                    let mut stats = self.stats.write().await;
                    stats.cache_hits += 1;
                    debug!("Cache hit: {}", cache_key);
                    return Ok(item.data.clone());
                }
            }

            let mut stats = self.stats.write().await;
            stats.cache_misses += 1;
        }

        // 获取并发许可
        let _permit = self.semaphore.acquire().await.unwrap();
        {
            let mut stats = self.stats.write().await;
            stats.concurrent_requests += 1;
        }

        // 执行实际请求
        let result = self
            .send_raw_request(slave_id, function_code, address, quantity, transport)
            .await;

        // 更新缓存
        if self.config.cache_enabled && result.is_ok() {
            let response_data = result.as_ref().unwrap();
            let cache_item = CacheItem {
                data: response_data.clone(),
                timestamp: Instant::now(),
                ttl: self.config.cache_ttl,
            };

            let mut cache = self.cache.write().await;
            // 检查缓存大小限制
            if cache.len() >= self.config.max_cache_size {
                // 简单的LRU：清除一半缓存
                let keys_to_remove: Vec<String> =
                    cache.keys().take(cache.len() / 2).cloned().collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }
            cache.insert(cache_key, cache_item);
        }

        result
    }

    /// 原始请求发送
    async fn send_raw_request(
        &self,
        slave_id: u8,
        function_code: ModbusFunctionCode,
        address: u16,
        quantity: u16,
        transport: &UniversalTransportBridge,
    ) -> Result<Vec<u8>> {
        // Zero-copy PDU construction
        let request_data = self
            .pdu_processor
            .build_read_request(function_code, address, quantity);
        debug!(
                "[Protocol Engine] PDU construction completed - Slave: {}, Function code: {:?}, Address: {}, Quantity: {}", 
                slave_id, function_code, address, quantity
            );

        // Get transaction ID
        let transaction_id = self
            .transaction_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        debug!(
            "[Protocol Engine] Transaction ID assigned: {}",
            transaction_id
        );

        // Build frame
        let frame = self.frame_processor.read().await.build_frame(
            slave_id,
            request_data,
            Some(transaction_id),
        );
        debug!(
            "[Protocol Engine] Modbus frame construction completed - Frame length: {} bytes",
            frame.len()
        );

        // Log outgoing request
        let channel_id = self.channel_id.unwrap_or(0);
        let channel_name = self.channel_name.as_deref().unwrap_or("unknown");
        let timestamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.6fZ");
        let hex = Self::format_hex(&frame);

        info!(
            channel_id = channel_id,
            channel_name = %channel_name,
            direction = "request",
            slave_id = slave_id,
            hex = %hex,
            bytes = frame.len(),
            "Modbus packet"
        );

        // Write to channel log file
        let log_entry = serde_json::json!({
            "timestamp": timestamp.to_string(),
            "level": "INFO",
            "channel_id": channel_id,
            "channel_name": channel_name,
            "direction": "request",
            "slave_id": slave_id,
            "hex": hex,
            "bytes": frame.len()
        });
        self.write_channel_log(&log_entry.to_string()).await;

        // Send request
        debug!("[Protocol Engine] Sending Modbus request to transport layer...");
        let response = transport.send_request(&frame).await?;

        // Log incoming response
        let response_hex = Self::format_hex(&response);
        let response_timestamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.6fZ");

        info!(
            channel_id = channel_id,
            channel_name = %channel_name,
            direction = "response",
            slave_id = slave_id,
            hex = %response_hex,
            bytes = response.len(),
            "Modbus packet"
        );

        // Write to channel log file
        let response_log_entry = serde_json::json!({
            "timestamp": response_timestamp.to_string(),
            "level": "INFO",
            "channel_id": channel_id,
            "channel_name": channel_name,
            "direction": "response",
            "slave_id": slave_id,
            "hex": response_hex,
            "bytes": response.len()
        });
        self.write_channel_log(&response_log_entry.to_string())
            .await;
        debug!(
            "[Protocol Engine] Received Modbus response - Response length: {} bytes",
            response.len()
        );

        // Parse response frame
        debug!("[Protocol Engine] Starting response frame parsing...");
        let parsed_frame = {
            let mut processor = self.frame_processor.write().await;
            processor.parse_frame(&response)?
        };
        debug!(
            "[Protocol Engine] Frame parsing completed - PDU length: {} bytes",
            parsed_frame.pdu.len()
        );

        // Parse response PDU
        debug!("[Protocol Engine] Starting response PDU parsing...");
        let pdu_result = self.pdu_processor.parse_response_pdu(&parsed_frame.pdu)?;
        debug!("[Protocol Engine] PDU parsing completed");

        // Extract data
        match pdu_result {
            crate::core::protocols::modbus::pdu::PduParseResult::Response(response) => {
                debug!(
                        "[Protocol Engine] Response data extraction successful - Data length: {} bytes, Data: {:02X?}", 
                        response.data.len(), response.data
                    );

                // Update zero-copy statistics
                let mut stats = self.stats.write().await;
                stats.zero_copy_operations += 1;

                Ok(response.data)
            }
            crate::core::protocols::modbus::pdu::PduParseResult::Exception(exception) => {
                warn!(
                        "[Protocol Engine] Received Modbus exception response - Function code: 0x{:02X}, Exception code: {:?}", 
                        exception.function_code, exception.exception_code
                    );

                Err(ComSrvError::ProtocolError(format!(
                    "Modbus exception response: Function code=0x{:02X}, Exception code={:?}",
                    exception.function_code, exception.exception_code
                )))
            }
            _ => {
                warn!("[Protocol Engine] Invalid PDU response type");
                Err(ComSrvError::ProtocolError("Invalid response".to_string()))
            }
        }
    }

    /// 写单个寄存器
    async fn send_write_single_register(
        &self,
        slave_id: u8,
        address: u16,
        value: u16,
        transport: &UniversalTransportBridge,
    ) -> Result<()> {
        let request_data = self.pdu_processor.build_write_single_request(
            ModbusFunctionCode::Write06,
            address,
            value,
        );
        let transaction_id = self
            .transaction_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let frame = self.frame_processor.read().await.build_frame(
            slave_id,
            request_data,
            Some(transaction_id),
        );

        transport.send_request(&frame).await?;
        Ok(())
    }

    /// 写单个线圈
    async fn send_write_single_coil(
        &self,
        slave_id: u8,
        address: u16,
        value: bool,
        transport: &UniversalTransportBridge,
    ) -> Result<()> {
        let coil_value = if value { 0xFF00 } else { 0x0000 };
        let request_data = self.pdu_processor.build_write_single_request(
            ModbusFunctionCode::Write05,
            address,
            coil_value,
        );
        let transaction_id = self
            .transaction_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let frame = self.frame_processor.read().await.build_frame(
            slave_id,
            request_data,
            Some(transaction_id),
        );

        transport.send_request(&frame).await?;
        Ok(())
    }

    /// 写多个寄存器
    async fn send_write_multiple_registers(
        &self,
        slave_id: u8,
        address: u16,
        data: &[u8],
        transport: &UniversalTransportBridge,
    ) -> Result<()> {
        let values: Vec<u16> = data
            .chunks(2)
            .map(|chunk| {
                if chunk.len() == 2 {
                    u16::from_be_bytes([chunk[0], chunk[1]])
                } else {
                    chunk[0] as u16
                }
            })
            .collect();

        let request_data = self
            .pdu_processor
            .build_write_multiple_registers_request(address, &values);
        let transaction_id = self
            .transaction_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let frame = self.frame_processor.read().await.build_frame(
            slave_id,
            request_data,
            Some(transaction_id),
        );

        transport.send_request(&frame).await?;
        Ok(())
    }

    /// 写多个线圈
    async fn send_write_multiple_coils(
        &self,
        slave_id: u8,
        address: u16,
        data: &[u8],
        transport: &UniversalTransportBridge,
    ) -> Result<()> {
        let values: Vec<bool> = data.iter().map(|&b| b != 0).collect();
        let request_data = self
            .pdu_processor
            .build_write_multiple_coils_request(address, &values);
        let transaction_id = self
            .transaction_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let frame = self.frame_processor.read().await.build_frame(
            slave_id,
            request_data,
            Some(transaction_id),
        );

        transport.send_request(&frame).await?;
        Ok(())
    }

    /// 解析遥测值
    fn parse_telemetry_value(&self, data: &[u8], mapping: &ModbusTelemetryMapping) -> Result<f64> {
        // Get register count based on data type
        let register_count = match mapping.data_type.to_lowercase().as_str() {
            "uint16" | "int16" => 1,
            "uint32" | "int32" | "float32" => 2,
            "uint64" | "int64" | "float64" => 4,
            _ => 1,
        };

        if data.len() < (register_count as usize * 2) {
            return Err(ComSrvError::ProtocolError(
                "Insufficient data length".to_string(),
            ));
        }

        match mapping.data_type.to_lowercase().as_str() {
            "uint16" => {
                let value = u16::from_be_bytes([data[0], data[1]]);
                Ok(value as f64)
            }
            "int16" => {
                let value = i16::from_be_bytes([data[0], data[1]]);
                Ok(value as f64)
            }
            "uint32" => {
                if data.len() < 4 {
                    return Err(ComSrvError::ProtocolError(
                        "Insufficient data length for uint32".to_string(),
                    ));
                }
                let value = match "ABCD" {
                    "ABCD" => u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
                    "DCBA" => u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
                    "BADC" => u32::from_be_bytes([data[1], data[0], data[3], data[2]]),
                    "CDAB" => u32::from_le_bytes([data[2], data[3], data[0], data[1]]),
                    _ => u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
                };
                Ok(value as f64)
            }
            "float32" => {
                if data.len() < 4 {
                    return Err(ComSrvError::ProtocolError(
                        "Insufficient data length for float32".to_string(),
                    ));
                }
                let bytes = match "ABCD" {
                    "ABCD" => [data[0], data[1], data[2], data[3]],
                    "DCBA" => [data[3], data[2], data[1], data[0]],
                    "BADC" => [data[1], data[0], data[3], data[2]],
                    "CDAB" => [data[2], data[3], data[0], data[1]],
                    _ => [data[0], data[1], data[2], data[3]],
                };
                let value = f32::from_be_bytes(bytes);
                Ok(value as f64)
            }
            _ => {
                warn!("Unsupported data format: {}", mapping.data_type);
                let value = u16::from_be_bytes([data[0], data[1]]);
                Ok(value as f64)
            }
        }
    }

    /// 解析遥信值
    fn parse_signal_value(&self, data: &[u8], mapping: &ModbusSignalMapping) -> Result<bool> {
        if data.is_empty() {
            return Err(ComSrvError::ProtocolError(
                "Signal data is empty".to_string(),
            ));
        }

        // 遥信点通常是位值，根据bit_location解析
        if let Some(bit_loc) = mapping.bit_location {
            // 有位位置，从字节中提取位
            let byte_index = (bit_loc / 8) as usize;
            let bit_index = bit_loc % 8;
            if byte_index < data.len() {
                Ok((data[byte_index] & (1 << bit_index)) != 0)
            } else {
                Err(ComSrvError::ProtocolError(
                    "Bit index exceeds data range".to_string(),
                ))
            }
        } else {
            // 没有位位置，整个字节作为布尔值
            if data.len() >= 2 {
                let register_value = u16::from_be_bytes([data[0], data[1]]);
                Ok(register_value != 0)
            } else if !data.is_empty() {
                // 单字节数据
                Ok(data[0] != 0)
            } else {
                Err(ComSrvError::ProtocolError(
                    "Signal data length insufficient".to_string(),
                ))
            }
        }
    }

    /// 转换遥调值
    fn convert_adjustment_value(
        &self,
        value: f64,
        mapping: &ModbusAdjustmentMapping,
    ) -> Result<Vec<u8>> {
        // 应用缩放和偏移的逆操作
        let raw_value = (value - mapping.offset) / mapping.scale;

        match mapping.data_type.to_lowercase().as_str() {
            "uint16" => {
                let int_value = raw_value as u16;
                Ok(int_value.to_be_bytes().to_vec())
            }
            "int16" => {
                let int_value = raw_value as i16;
                Ok(int_value.to_be_bytes().to_vec())
            }
            "uint32" => {
                let int_value = raw_value as u32;
                let bytes = match "ABCD" {
                    "ABCD" => int_value.to_be_bytes(),
                    "DCBA" => int_value.to_le_bytes(),
                    "BADC" => {
                        let be_bytes = int_value.to_be_bytes();
                        [be_bytes[1], be_bytes[0], be_bytes[3], be_bytes[2]]
                    }
                    "CDAB" => {
                        let le_bytes = int_value.to_le_bytes();
                        [le_bytes[2], le_bytes[3], le_bytes[0], le_bytes[1]]
                    }
                    _ => int_value.to_be_bytes(),
                };
                Ok(bytes.to_vec())
            }
            "float32" => {
                let float_value = raw_value as f32;
                let bytes = match "ABCD" {
                    "ABCD" => float_value.to_be_bytes(),
                    "DCBA" => {
                        let be_bytes = float_value.to_be_bytes();
                        [be_bytes[3], be_bytes[2], be_bytes[1], be_bytes[0]]
                    }
                    "BADC" => {
                        let be_bytes = float_value.to_be_bytes();
                        [be_bytes[1], be_bytes[0], be_bytes[3], be_bytes[2]]
                    }
                    "CDAB" => {
                        let be_bytes = float_value.to_be_bytes();
                        [be_bytes[2], be_bytes[3], be_bytes[0], be_bytes[1]]
                    }
                    _ => float_value.to_be_bytes(),
                };
                Ok(bytes.to_vec())
            }
            _ => {
                warn!("Unsupported adjustment data format: {}", mapping.data_type);
                let int_value = raw_value as u16;
                Ok(int_value.to_be_bytes().to_vec())
            }
        }
    }

    /// 清理过期缓存
    pub async fn cleanup_cache(&self) {
        let mut cache = self.cache.write().await;
        let expired_keys: Vec<String> = cache
            .iter()
            .filter(|(_, item)| item.is_expired())
            .map(|(key, _)| key.clone())
            .collect();

        for key in expired_keys {
            cache.remove(&key);
        }

        debug!("Cleaned {} expired cache items", cache.len());
    }

    /// 获取引擎统计信息
    pub async fn get_stats(&self) -> EngineStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = EngineStats::default();
    }

    /// 格式化字节数组为十六进制字符串
    fn format_hex(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<String>>()
            .join(" ")
    }

    /// 获取缓存统计
    pub async fn get_cache_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();
        let cache = self.cache.read().await;
        let engine_stats = self.stats.read().await;

        stats.insert("cache_size".to_string(), cache.len().to_string());
        stats.insert(
            "cache_hits".to_string(),
            engine_stats.cache_hits.to_string(),
        );
        stats.insert(
            "cache_misses".to_string(),
            engine_stats.cache_misses.to_string(),
        );

        let hit_rate = if engine_stats.cache_hits + engine_stats.cache_misses > 0 {
            (engine_stats.cache_hits as f64
                / (engine_stats.cache_hits + engine_stats.cache_misses) as f64)
                * 100.0
        } else {
            0.0
        };
        stats.insert("cache_hit_rate".to_string(), format!("{:.2}%", hit_rate));

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> ModbusConfig {
        ModbusConfig {
            protocol_type: "modbus_tcp".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(502),
            device_path: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout_ms: Some(5000),
            points: vec![],
        }
    }

    #[tokio::test]
    async fn test_protocol_engine_creation() {
        let config = create_test_config();
        let engine = ModbusProtocolEngine::new(&config).await;
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let config = create_test_config();
        let engine = ModbusProtocolEngine::new(&config).await.unwrap();

        // 测试缓存统计
        let cache_stats = engine.get_cache_stats().await;
        assert_eq!(cache_stats.get("cache_size").unwrap(), "0");
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let config = create_test_config();
        let engine = ModbusProtocolEngine::new(&config).await.unwrap();

        let stats = engine.get_stats().await;
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
    }
}
