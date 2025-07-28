//! Modbus 传输层实现
//!
//! 支持 TCP 和 RTU 两种传输模式的帧处理

use super::connection::ConnectionParams;
use crate::core::config::types::ChannelConfig;
use crate::utils::error::{ComSrvError, Result};
use tracing::debug;

/// Modbus 传输模式
#[derive(Debug, Clone, PartialEq)]
pub enum ModbusMode {
    /// TCP 模式 (使用 MBAP 头)
    Tcp,
    /// RTU 模式 (使用 CRC 校验)
    Rtu,
}

/// Modbus TCP MBAP 头
#[derive(Debug, Clone)]
pub struct MbapHeader {
    /// 事务标识符
    pub transaction_id: u16,
    /// 协议标识符 (固定为 0)
    pub protocol_id: u16,
    /// 长度字段
    pub length: u16,
    /// 单元标识符 (从站ID)
    pub unit_id: u8,
}

/// Modbus 帧处理器
#[derive(Debug)]
pub struct ModbusFrameProcessor {
    mode: ModbusMode,
    next_transaction_id: u16,
}

impl ModbusFrameProcessor {
    /// 创建新的帧处理器
    pub fn new(mode: ModbusMode) -> Self {
        Self {
            mode,
            next_transaction_id: 1,
        }
    }

    /// 获取下一个事务ID（仅TCP模式使用）
    pub fn next_transaction_id(&mut self) -> u16 {
        let id = self.next_transaction_id;
        self.next_transaction_id = self.next_transaction_id.wrapping_add(1);
        if self.next_transaction_id == 0 {
            self.next_transaction_id = 1;
        }
        id
    }

    /// 构建完整的Modbus帧
    pub fn build_frame(&mut self, unit_id: u8, pdu: &[u8]) -> Vec<u8> {
        match self.mode {
            ModbusMode::Tcp => self.build_tcp_frame(unit_id, pdu),
            ModbusMode::Rtu => self.build_rtu_frame(unit_id, pdu),
        }
    }

    /// 解析接收到的帧
    pub fn parse_frame(&self, data: &[u8]) -> Result<(u8, Vec<u8>)> {
        match self.mode {
            ModbusMode::Tcp => self.parse_tcp_frame(data),
            ModbusMode::Rtu => self.parse_rtu_frame(data),
        }
    }

    /// 构建TCP帧 (MBAP + PDU)
    fn build_tcp_frame(&mut self, unit_id: u8, pdu: &[u8]) -> Vec<u8> {
        let transaction_id = self.next_transaction_id();
        let length = (pdu.len() + 1) as u16; // PDU长度 + unit_id

        let mut frame = Vec::with_capacity(6 + pdu.len());

        // MBAP 头
        frame.extend_from_slice(&transaction_id.to_be_bytes());
        frame.extend_from_slice(&0u16.to_be_bytes()); // protocol_id
        frame.extend_from_slice(&length.to_be_bytes());
        frame.push(unit_id);

        // PDU
        frame.extend_from_slice(pdu);

        frame
    }

    /// 构建RTU帧 (`unit_id` + PDU + CRC)
    fn build_rtu_frame(&self, unit_id: u8, pdu: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(1 + pdu.len() + 2);

        // Unit ID
        frame.push(unit_id);

        // PDU
        frame.extend_from_slice(pdu);

        // CRC
        let crc = self.calculate_crc16(&frame);
        frame.extend_from_slice(&crc.to_le_bytes());

        frame
    }

    /// 解析TCP帧
    fn parse_tcp_frame(&self, data: &[u8]) -> Result<(u8, Vec<u8>)> {
        if data.len() < 8 {
            return Err(ComSrvError::ProtocolError(
                "TCP frame too short".to_string(),
            ));
        }

        // 解析MBAP头
        let _transaction_id = u16::from_be_bytes([data[0], data[1]]);
        let _protocol_id = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let unit_id = data[6];

        // 验证长度
        if data.len() != (6 + length as usize) {
            return Err(ComSrvError::ProtocolError(
                "Invalid TCP frame length".to_string(),
            ));
        }

        // 提取PDU
        let pdu = data[7..].to_vec();

        debug!(
            "Parsed TCP frame: trans_id={:04X}, length={}, unit_id={}, pdu_len={}",
            _transaction_id,
            length,
            unit_id,
            pdu.len()
        );

        Ok((unit_id, pdu))
    }

    /// 解析RTU帧
    fn parse_rtu_frame(&self, data: &[u8]) -> Result<(u8, Vec<u8>)> {
        if data.len() < 4 {
            return Err(ComSrvError::ProtocolError(
                "RTU frame too short".to_string(),
            ));
        }

        let frame_len = data.len();
        let unit_id = data[0];
        let pdu_data = &data[1..frame_len - 2];
        let received_crc = u16::from_le_bytes([data[frame_len - 2], data[frame_len - 1]]);

        // 验证CRC
        let calculated_crc = self.calculate_crc16(&data[..frame_len - 2]);
        if received_crc != calculated_crc {
            return Err(ComSrvError::ProtocolError(format!(
                "CRC mismatch: expected 0x{calculated_crc:04X}, got 0x{received_crc:04X}"
            )));
        }

        Ok((unit_id, pdu_data.to_vec()))
    }

    /// 计算CRC16校验码 (Modbus RTU标准)
    fn calculate_crc16(&self, data: &[u8]) -> u16 {
        let mut crc: u16 = 0xFFFF;

        for &byte in data {
            crc ^= u16::from(byte);
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc >>= 1;
                    crc ^= 0xA001;
                } else {
                    crc >>= 1;
                }
            }
        }

        crc
    }

    /// 验证PDU是否为异常响应
    pub fn is_exception_response(pdu: &[u8]) -> bool {
        !pdu.is_empty() && (pdu[0] & 0x80) != 0
    }

    /// 解析异常响应
    pub fn parse_exception(pdu: &[u8]) -> Result<(u8, u8)> {
        if pdu.len() < 2 {
            return Err(ComSrvError::ProtocolError(
                "Invalid exception response".to_string(),
            ));
        }

        let function_code = pdu[0] & 0x7F; // 移除错误位
        let exception_code = pdu[1];

        Ok((function_code, exception_code))
    }

    /// 获取异常描述
    pub fn exception_description(exception_code: u8) -> &'static str {
        match exception_code {
            0x01 => "Illegal Function",
            0x02 => "Illegal Data Address",
            0x03 => "Illegal Data Value",
            0x04 => "Slave Device Failure",
            0x05 => "Acknowledge",
            0x06 => "Slave Device Busy",
            0x07 => "Negative Acknowledge",
            0x08 => "Memory Parity Error",
            0x0A => "Gateway Path Unavailable",
            0x0B => "Gateway Target Device Failed to Respond",
            _ => "Unknown Exception",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_frame_build_parse() {
        let mut processor = ModbusFrameProcessor::new(ModbusMode::Tcp);
        let pdu = vec![0x03, 0x00, 0x01, 0x00, 0x02]; // Read holding registers

        let frame = processor.build_frame(1, &pdu);
        assert_eq!(frame.len(), 12); // 7 bytes header (2 trans_id + 2 proto + 2 len + 1 unit) + 5 bytes PDU

        let (unit_id, parsed_pdu) = processor.parse_frame(&frame).unwrap();
        assert_eq!(unit_id, 1);
        assert_eq!(parsed_pdu, pdu);
    }

    #[test]
    fn test_rtu_frame_build_parse() {
        let mut processor = ModbusFrameProcessor::new(ModbusMode::Rtu);
        let pdu = vec![0x03, 0x00, 0x01, 0x00, 0x02]; // Read holding registers

        let frame = processor.build_frame(1, &pdu);
        assert_eq!(frame.len(), 8); // 1 byte unit_id + 5 bytes PDU + 2 bytes CRC

        let (unit_id, parsed_pdu) = processor.parse_frame(&frame).unwrap();
        assert_eq!(unit_id, 1);
        assert_eq!(parsed_pdu, pdu);
    }

    #[test]
    fn test_crc16_calculation() {
        let processor = ModbusFrameProcessor::new(ModbusMode::Rtu);
        let data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x01];
        let crc = processor.calculate_crc16(&data);
        // CRC 计算结果应该是 0x0A84 (2692 in decimal)
        assert_eq!(crc, 0x0A84);
    }

    #[test]
    fn test_exception_response() {
        let exception_pdu = vec![0x83, 0x02]; // Function 03 with exception code 02

        assert!(ModbusFrameProcessor::is_exception_response(&exception_pdu));

        let (func_code, exc_code) = ModbusFrameProcessor::parse_exception(&exception_pdu).unwrap();
        assert_eq!(func_code, 0x03);
        assert_eq!(exc_code, 0x02);

        let desc = ModbusFrameProcessor::exception_description(0x02);
        assert_eq!(desc, "Illegal Data Address");
    }
}

/// 创建连接参数
/// 从通道配置中提取连接参数
pub fn create_connection_params(config: &ChannelConfig) -> Result<ConnectionParams> {
    match config.protocol.as_str() {
        "modbus_tcp" => {
            // 从parameters中提取TCP配置
            let host = config
                .parameters
                .get("host")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);
            let port = config
                .parameters
                .get("port")
                .and_then(serde_yaml::Value::as_u64)
                .map(|p| p as u16);

            Ok(ConnectionParams {
                host,
                port,
                device: None,
                baud_rate: None,
                data_bits: None,
                stop_bits: None,
                parity: None,
                timeout: std::time::Duration::from_secs(5),
            })
        }
        "modbus_rtu" => {
            // 从parameters中提取串口配置
            let device = config
                .parameters
                .get("device")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);
            let baud_rate = config
                .parameters
                .get("baud_rate")
                .and_then(serde_yaml::Value::as_u64)
                .map(|b| b as u32);
            let data_bits = config
                .parameters
                .get("data_bits")
                .and_then(serde_yaml::Value::as_u64)
                .map(|d| d as u8);
            let stop_bits = config
                .parameters
                .get("stop_bits")
                .and_then(serde_yaml::Value::as_u64)
                .map(|s| s as u8);
            let parity = config
                .parameters
                .get("parity")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            Ok(ConnectionParams {
                host: None,
                port: None,
                device,
                baud_rate,
                data_bits,
                stop_bits,
                parity,
                timeout: std::time::Duration::from_secs(1),
            })
        }
        _ => Err(ComSrvError::ConfigError(format!(
            "Unsupported protocol type: {}",
            config.protocol
        ))),
    }
}
