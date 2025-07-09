//! Modbus协议模拟器
//!
//! 提供Modbus TCP/RTU服务器模拟，用于测试Modbus客户端实现

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bytes::{BytesMut, Buf};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, debug, error};

/// Modbus功能码
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
enum FunctionCode {
    ReadCoils = 0x01,
    ReadDiscreteInputs = 0x02,
    ReadHoldingRegisters = 0x03,
    ReadInputRegisters = 0x04,
    WriteSingleCoil = 0x05,
    WriteSingleRegister = 0x06,
    WriteMultipleCoils = 0x0F,
    WriteMultipleRegisters = 0x10,
}

/// Modbus异常码
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum ExceptionCode {
    IllegalFunction = 0x01,
    IllegalDataAddress = 0x02,
    IllegalDataValue = 0x03,
    SlaveDeviceFailure = 0x04,
}

/// Modbus数据模型
#[derive(Debug)]
struct ModbusDataModel {
    coils: HashMap<u16, bool>,              // 0x
    discrete_inputs: HashMap<u16, bool>,    // 1x
    holding_registers: HashMap<u16, u16>,   // 4x
    input_registers: HashMap<u16, u16>,     // 3x
}

impl Default for ModbusDataModel {
    fn default() -> Self {
        let mut model = Self {
            coils: HashMap::new(),
            discrete_inputs: HashMap::new(),
            holding_registers: HashMap::new(),
            input_registers: HashMap::new(),
        };
        
        // 初始化一些测试数据
        for i in 0..100 {
            model.coils.insert(i, i % 2 == 0);
            model.discrete_inputs.insert(i, i % 3 == 0);
            model.holding_registers.insert(i, i * 10);
            model.input_registers.insert(i, i * 100);
        }
        
        model
    }
}

/// Modbus TCP服务器模拟器
pub struct ModbusTcpSimulator {
    addr: SocketAddr,
    data_models: Arc<RwLock<HashMap<u8, ModbusDataModel>>>,
    running: Arc<RwLock<bool>>,
}

impl ModbusTcpSimulator {
    /// 创建新的模拟器
    pub fn new(addr: SocketAddr) -> Self {
        let mut data_models = HashMap::new();
        
        // 为不同的单元ID创建数据模型
        for unit_id in 1..=10 {
            data_models.insert(unit_id, ModbusDataModel::default());
        }
        
        Self {
            addr,
            data_models: Arc::new(RwLock::new(data_models)),
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// 启动模拟器
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.addr).await?;
        *self.running.write().await = true;
        
        info!("Modbus TCP simulator listening on {}", self.addr);
        
        while *self.running.read().await {
            let (stream, addr) = match listener.accept().await {
                Ok(result) => result,
                Err(e) => {
                    error!("Accept error: {}", e);
                    continue;
                }
            };
            
            info!("New connection from {}", addr);
            
            let data_models = Arc::clone(&self.data_models);
            let running = Arc::clone(&self.running);
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, data_models, running).await {
                    error!("Client handler error: {}", e);
                }
            });
        }
        
        Ok(())
    }
    
    /// 停止模拟器
    pub async fn stop(&self) {
        *self.running.write().await = false;
        info!("Modbus TCP simulator stopped");
    }
    
    /// 处理客户端连接
    async fn handle_client(
        mut stream: TcpStream,
        data_models: Arc<RwLock<HashMap<u8, ModbusDataModel>>>,
        running: Arc<RwLock<bool>>
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = BytesMut::with_capacity(1024);
        
        while *running.read().await {
            // 读取请求
            let n = stream.read_buf(&mut buffer).await?;
            if n == 0 {
                break; // 连接关闭
            }
            
            // 处理Modbus TCP帧
            while buffer.len() >= 7 {
                // 检查是否有完整的MBAP头
                let length = ((buffer[4] as usize) << 8) | buffer[5] as usize;
                let frame_length = 6 + length;
                
                if buffer.len() < frame_length {
                    break; // 等待更多数据
                }
                
                // 提取一个完整的帧
                let frame = buffer.split_to(frame_length);
                
                // 处理请求并生成响应
                match Self::process_request(&frame, &data_models).await {
                    Ok(response) => {
                        stream.write_all(&response).await?;
                        stream.flush().await?;
                    }
                    Err(e) => {
                        error!("Process request error: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 处理Modbus请求
    async fn process_request(
        frame: &[u8],
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // 解析MBAP头
        let transaction_id = (frame[0] as u16) << 8 | frame[1] as u16;
        let protocol_id = (frame[2] as u16) << 8 | frame[3] as u16;
        let unit_id = frame[6];
        
        if protocol_id != 0 {
            return Err("Invalid protocol ID".into());
        }
        
        // 获取PDU
        let pdu = &frame[7..];
        if pdu.is_empty() {
            return Err("Empty PDU".into());
        }
        
        let function_code = pdu[0];
        
        debug!("Request: Unit={}, Function=0x{:02X}", unit_id, function_code);
        
        // 处理功能码
        let response_pdu = match function_code {
            0x01 => Self::read_coils(pdu, unit_id, &data_models).await?,
            0x02 => Self::read_discrete_inputs(pdu, unit_id, &data_models).await?,
            0x03 => Self::read_holding_registers(pdu, unit_id, &data_models).await?,
            0x04 => Self::read_input_registers(pdu, unit_id, &data_models).await?,
            0x05 => Self::write_single_coil(pdu, unit_id, &data_models).await?,
            0x06 => Self::write_single_register(pdu, unit_id, &data_models).await?,
            0x0F => Self::write_multiple_coils(pdu, unit_id, &data_models).await?,
            0x10 => Self::write_multiple_registers(pdu, unit_id, &data_models).await?,
            _ => {
                // 不支持的功能码
                vec![function_code | 0x80, ExceptionCode::IllegalFunction as u8]
            }
        };
        
        // 构建响应帧
        let mut response = Vec::with_capacity(7 + response_pdu.len());
        response.extend_from_slice(&frame[0..4]); // Transaction ID + Protocol ID
        response.push(((response_pdu.len() + 1) >> 8) as u8); // Length high
        response.push((response_pdu.len() + 1) as u8); // Length low
        response.push(unit_id);
        response.extend_from_slice(&response_pdu);
        
        Ok(response)
    }
    
    /// 读取线圈（功能码0x01）
    async fn read_coils(
        pdu: &[u8],
        unit_id: u8,
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if pdu.len() < 5 {
            return Ok(vec![0x81, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let start_address = ((pdu[1] as u16) << 8) | pdu[2] as u16;
        let quantity = ((pdu[3] as u16) << 8) | pdu[4] as u16;
        
        if quantity == 0 || quantity > 2000 {
            return Ok(vec![0x81, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let models = data_models.read().await;
        let model = models.get(&unit_id)
            .ok_or("Unit ID not found")?;
        
        let mut response = vec![0x01]; // Function code
        let byte_count = (quantity + 7) / 8;
        response.push(byte_count as u8);
        
        let mut current_byte = 0u8;
        let mut bit_count = 0;
        
        for i in 0..quantity {
            let addr = start_address + i;
            let value = model.coils.get(&addr).copied().unwrap_or(false);
            
            if value {
                current_byte |= 1 << bit_count;
            }
            
            bit_count += 1;
            if bit_count == 8 {
                response.push(current_byte);
                current_byte = 0;
                bit_count = 0;
            }
        }
        
        if bit_count > 0 {
            response.push(current_byte);
        }
        
        Ok(response)
    }
    
    /// 读取离散输入（功能码0x02）
    async fn read_discrete_inputs(
        pdu: &[u8],
        unit_id: u8,
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if pdu.len() < 5 {
            return Ok(vec![0x82, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let start_address = ((pdu[1] as u16) << 8) | pdu[2] as u16;
        let quantity = ((pdu[3] as u16) << 8) | pdu[4] as u16;
        
        if quantity == 0 || quantity > 2000 {
            return Ok(vec![0x82, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let models = data_models.read().await;
        let model = models.get(&unit_id)
            .ok_or("Unit ID not found")?;
        
        let mut response = vec![0x02]; // Function code
        let byte_count = (quantity + 7) / 8;
        response.push(byte_count as u8);
        
        let mut current_byte = 0u8;
        let mut bit_count = 0;
        
        for i in 0..quantity {
            let addr = start_address + i;
            let value = model.discrete_inputs.get(&addr).copied().unwrap_or(false);
            
            if value {
                current_byte |= 1 << bit_count;
            }
            
            bit_count += 1;
            if bit_count == 8 {
                response.push(current_byte);
                current_byte = 0;
                bit_count = 0;
            }
        }
        
        if bit_count > 0 {
            response.push(current_byte);
        }
        
        Ok(response)
    }
    
    /// 读取保持寄存器（功能码0x03）
    async fn read_holding_registers(
        pdu: &[u8],
        unit_id: u8,
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if pdu.len() < 5 {
            return Ok(vec![0x83, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let start_address = ((pdu[1] as u16) << 8) | pdu[2] as u16;
        let quantity = ((pdu[3] as u16) << 8) | pdu[4] as u16;
        
        if quantity == 0 || quantity > 125 {
            return Ok(vec![0x83, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let models = data_models.read().await;
        let model = models.get(&unit_id)
            .ok_or("Unit ID not found")?;
        
        let mut response = vec![0x03]; // Function code
        response.push((quantity * 2) as u8); // Byte count
        
        for i in 0..quantity {
            let addr = start_address + i;
            let value = model.holding_registers.get(&addr).copied().unwrap_or(0);
            response.push((value >> 8) as u8);
            response.push(value as u8);
        }
        
        Ok(response)
    }
    
    /// 读取输入寄存器（功能码0x04）
    async fn read_input_registers(
        pdu: &[u8],
        unit_id: u8,
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if pdu.len() < 5 {
            return Ok(vec![0x84, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let start_address = ((pdu[1] as u16) << 8) | pdu[2] as u16;
        let quantity = ((pdu[3] as u16) << 8) | pdu[4] as u16;
        
        if quantity == 0 || quantity > 125 {
            return Ok(vec![0x84, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let models = data_models.read().await;
        let model = models.get(&unit_id)
            .ok_or("Unit ID not found")?;
        
        let mut response = vec![0x04]; // Function code
        response.push((quantity * 2) as u8); // Byte count
        
        for i in 0..quantity {
            let addr = start_address + i;
            let value = model.input_registers.get(&addr).copied().unwrap_or(0);
            response.push((value >> 8) as u8);
            response.push(value as u8);
        }
        
        Ok(response)
    }
    
    /// 写单个线圈（功能码0x05）
    async fn write_single_coil(
        pdu: &[u8],
        unit_id: u8,
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if pdu.len() < 5 {
            return Ok(vec![0x85, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let address = ((pdu[1] as u16) << 8) | pdu[2] as u16;
        let value = ((pdu[3] as u16) << 8) | pdu[4] as u16;
        
        if value != 0x0000 && value != 0xFF00 {
            return Ok(vec![0x85, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let mut models = data_models.write().await;
        let model = models.get_mut(&unit_id)
            .ok_or("Unit ID not found")?;
        
        model.coils.insert(address, value == 0xFF00);
        
        // Echo back the request
        Ok(pdu.to_vec())
    }
    
    /// 写单个寄存器（功能码0x06）
    async fn write_single_register(
        pdu: &[u8],
        unit_id: u8,
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if pdu.len() < 5 {
            return Ok(vec![0x86, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let address = ((pdu[1] as u16) << 8) | pdu[2] as u16;
        let value = ((pdu[3] as u16) << 8) | pdu[4] as u16;
        
        let mut models = data_models.write().await;
        let model = models.get_mut(&unit_id)
            .ok_or("Unit ID not found")?;
        
        model.holding_registers.insert(address, value);
        
        // Echo back the request
        Ok(pdu.to_vec())
    }
    
    /// 写多个线圈（功能码0x0F）
    async fn write_multiple_coils(
        pdu: &[u8],
        unit_id: u8,
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if pdu.len() < 6 {
            return Ok(vec![0x8F, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let start_address = ((pdu[1] as u16) << 8) | pdu[2] as u16;
        let quantity = ((pdu[3] as u16) << 8) | pdu[4] as u16;
        let byte_count = pdu[5] as usize;
        
        if quantity == 0 || quantity > 1968 {
            return Ok(vec![0x8F, ExceptionCode::IllegalDataValue as u8]);
        }
        
        if byte_count != (quantity + 7) / 8 {
            return Ok(vec![0x8F, ExceptionCode::IllegalDataValue as u8]);
        }
        
        if pdu.len() < 6 + byte_count {
            return Ok(vec![0x8F, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let mut models = data_models.write().await;
        let model = models.get_mut(&unit_id)
            .ok_or("Unit ID not found")?;
        
        let data_bytes = &pdu[6..6 + byte_count];
        for i in 0..quantity {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            let value = (data_bytes[byte_idx as usize] & (1 << bit_idx)) != 0;
            model.coils.insert(start_address + i, value);
        }
        
        // Response: function code + address + quantity
        Ok(vec![0x0F, pdu[1], pdu[2], pdu[3], pdu[4]])
    }
    
    /// 写多个寄存器（功能码0x10）
    async fn write_multiple_registers(
        pdu: &[u8],
        unit_id: u8,
        data_models: &Arc<RwLock<HashMap<u8, ModbusDataModel>>>
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if pdu.len() < 6 {
            return Ok(vec![0x90, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let start_address = ((pdu[1] as u16) << 8) | pdu[2] as u16;
        let quantity = ((pdu[3] as u16) << 8) | pdu[4] as u16;
        let byte_count = pdu[5] as usize;
        
        if quantity == 0 || quantity > 123 {
            return Ok(vec![0x90, ExceptionCode::IllegalDataValue as u8]);
        }
        
        if byte_count != quantity as usize * 2 {
            return Ok(vec![0x90, ExceptionCode::IllegalDataValue as u8]);
        }
        
        if pdu.len() < 6 + byte_count {
            return Ok(vec![0x90, ExceptionCode::IllegalDataValue as u8]);
        }
        
        let mut models = data_models.write().await;
        let model = models.get_mut(&unit_id)
            .ok_or("Unit ID not found")?;
        
        let data_bytes = &pdu[6..6 + byte_count];
        for i in 0..quantity {
            let idx = (i * 2) as usize;
            let value = ((data_bytes[idx] as u16) << 8) | data_bytes[idx + 1] as u16;
            model.holding_registers.insert(start_address + i, value);
        }
        
        // Response: function code + address + quantity
        Ok(vec![0x10, pdu[1], pdu[2], pdu[3], pdu[4]])
    }
    
    /// 设置寄存器值（测试用）
    pub async fn set_register(&self, unit_id: u8, address: u16, value: u16) {
        let mut models = self.data_models.write().await;
        if let Some(model) = models.get_mut(&unit_id) {
            model.holding_registers.insert(address, value);
            model.input_registers.insert(address, value);
        }
    }
    
    /// 设置线圈值（测试用）
    pub async fn set_coil(&self, unit_id: u8, address: u16, value: bool) {
        let mut models = self.data_models.write().await;
        if let Some(model) = models.get_mut(&unit_id) {
            model.coils.insert(address, value);
            model.discrete_inputs.insert(address, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_simulator_basic() {
        let addr = "127.0.0.1:5502".parse().unwrap();
        let simulator = ModbusTcpSimulator::new(addr);
        
        // 测试数据模型初始化
        let models = simulator.data_models.read().await;
        assert!(models.contains_key(&1));
        
        let model = models.get(&1).unwrap();
        assert_eq!(model.holding_registers.get(&10), Some(&100));
    }
}