//! Modbus 协议的数据类型和配置
//!
//! 包含简化的 Modbus 点定义、轮询配置和批量处理配置

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 简化的 Modbus 点映射
/// 只包含协议相关字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusPoint {
    /// 唯一点标识符（匹配四遥表）
    pub point_id: String,
    /// Modbus 从站ID
    pub slave_id: u8,
    /// 读取功能码
    pub function_code: u8,
    /// 寄存器地址
    pub register_address: u16,
    /// 数据格式 (e.g., "`float32_be`", "uint16", "bool")
    pub data_format: String,
    /// 读取寄存器数量 (e.g., 2 for float32)
    pub register_count: u16,
    /// 多寄存器值的字节序 (e.g., "ABCD", "CDAB")
    pub byte_order: Option<String>,
}

/// Modbus 轮询配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusPollingConfig {
    /// 是否启用轮询
    pub enabled: bool,
    /// 全局默认轮询间隔（毫秒）
    pub default_interval_ms: u64,
    /// 连接超时（毫秒）
    pub connection_timeout_ms: u64,
    /// 读取超时（毫秒）
    pub read_timeout_ms: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 错误后重试间隔（毫秒）
    pub retry_interval_ms: u64,
    /// 批量处理配置
    pub batch_config: ModbusBatchConfig,
    /// 从站特定配置
    pub slaves: HashMap<u8, SlavePollingConfig>,
}

/// 从站轮询配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlavePollingConfig {
    /// 从站ID
    pub slave_id: u8,
    /// 轮询间隔（毫秒）
    pub interval_ms: u64,
    /// 是否启用该从站
    pub enabled: bool,
    /// 从站特定超时
    pub timeout_ms: Option<u64>,
    /// 从站描述
    pub description: Option<String>,
}

/// Modbus 批量读取配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusBatchConfig {
    /// 是否启用批量读取
    pub enabled: bool,
    /// 最大批量大小（寄存器数量）
    pub max_batch_size: u16,
    /// 地址间隙阈值
    pub max_gap: u16,
    /// 是否合并不同功能码
    pub merge_function_codes: bool,
    /// 设备特定限制
    pub device_limits: HashMap<u8, DeviceLimit>,
}

/// 设备特定限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLimit {
    /// 单次读取最大寄存器数
    pub max_registers_per_read: u16,
    /// 设备描述
    pub description: Option<String>,
}

impl Default for ModbusPollingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_interval_ms: 1000,
            connection_timeout_ms: 5000,
            read_timeout_ms: 3000,
            max_retries: 3,
            retry_interval_ms: 1000,
            batch_config: ModbusBatchConfig::default(),
            slaves: HashMap::new(),
        }
    }
}

impl Default for ModbusBatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_batch_size: 100,
            max_gap: 5,
            merge_function_codes: false,
            device_limits: HashMap::new(),
        }
    }
}

impl Default for SlavePollingConfig {
    fn default() -> Self {
        Self {
            slave_id: 1,
            interval_ms: 1000,
            enabled: true,
            timeout_ms: None,
            description: None,
        }
    }
}

impl Default for DeviceLimit {
    fn default() -> Self {
        Self {
            max_registers_per_read: 100,
            description: None,
        }
    }
}
