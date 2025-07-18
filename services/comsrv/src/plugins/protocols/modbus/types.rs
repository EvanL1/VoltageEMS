//! Common types for Modbus protocol implementation

use std::collections::HashMap;
use std::time::Duration;

use super::common::ModbusConfig;
use super::modbus_polling::ModbusPollingConfig;
use crate::plugins::protocols::modbus::protocol_engine::{
    ModbusAdjustmentMapping, ModbusControlMapping, ModbusSignalMapping, ModbusTelemetryMapping,
};

/// Modbus channel configuration
#[derive(Debug, Clone)]
pub struct ModbusChannelConfig {
    pub channel_id: u16,
    pub channel_name: String,
    pub connection: ModbusConfig,
    pub request_timeout: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
    /// Polling configuration (protocol-specific)
    pub polling: ModbusPollingConfig,
}

/// Protocol mapping table
#[derive(Debug, Clone, Default)]
pub struct ProtocolMappingTable {
    pub telemetry_mappings: HashMap<u32, ModbusTelemetryMapping>,
    pub signal_mappings: HashMap<u32, ModbusSignalMapping>,
    pub adjustment_mappings: HashMap<u32, ModbusAdjustmentMapping>,
    pub control_mappings: HashMap<u32, ModbusControlMapping>,
}

/// Connection state
#[derive(Debug, Clone, Default)]
pub struct ConnectionState {
    pub connected: bool,
    pub last_connect_time: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
    pub retry_count: u32,
}

/// Client statistics
#[derive(Debug, Clone, Default)]
pub struct ClientStatistics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub average_response_time_ms: f64,
    pub last_request_time: Option<chrono::DateTime<chrono::Utc>>,
}
