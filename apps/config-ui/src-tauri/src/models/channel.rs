use serde::{Deserialize, Serialize};
use crate::models::point_table::PointDefinition;
use crate::models::protocol_mapping::ProtocolMappingEnum;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: u32,
    pub name: String,
    pub protocol: String,
    pub protocol_type: String, // modbus_tcp, modbus_rtu, iec104, iec101, can
    pub enabled: bool,
    pub transport_config: TransportConfig,
    pub protocol_config: ProtocolConfig,
    pub polling_config: PollingConfig,
    pub point_table: PointTableConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub tcp: Option<TcpConfig>,
    pub serial: Option<SerialConfig>,
    pub can: Option<CanTransportConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConfig {
    pub address: String,
    pub timeout_ms: u64,
    pub keepalive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialConfig {
    pub port: String,
    pub baudrate: u32,
    pub data_bits: u8,
    pub stop_bits: String,
    pub parity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanTransportConfig {
    pub interface: String,
    pub bitrate: u32,
    pub use_extended_id: bool,
    pub use_fd: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    pub modbus: Option<ModbusConfig>,
    pub iec60870: Option<IEC60870Config>,
    pub can: Option<CanProtocolConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusConfig {
    pub mode: String, // tcp or rtu
    pub retry_count: u32,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IEC60870Config {
    pub version: String, // 101 or 104
    pub common_address: u16,
    pub link_address: Option<u16>,
    pub k: Option<u8>,
    pub w: Option<u8>,
    pub t1: Option<u32>,
    pub t2: Option<u32>,
    pub t3: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanProtocolConfig {
    pub dbc_file: Option<String>,
    pub messages: Vec<CanMessageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanMessageConfig {
    pub id: u32,
    pub name: String,
    pub cycle_time_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingConfig {
    pub interval_ms: u64,
    pub batch_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTableConfig {
    // 点位定义（四遥）
    pub telemetry: Vec<PointDefinition>,
    pub signal: Vec<PointDefinition>,
    pub control: Vec<PointDefinition>,
    pub adjustment: Vec<PointDefinition>,
    
    // 协议映射（根据协议类型存储不同的映射）
    pub telemetry_mapping: Vec<ProtocolMappingEnum>,
    pub signal_mapping: Vec<ProtocolMappingEnum>,
    pub control_mapping: Vec<ProtocolMappingEnum>,
    pub adjustment_mapping: Vec<ProtocolMappingEnum>,
    
    // CSV配置路径（用于兼容现有系统）
    pub csv_config: Option<CsvPathConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvPathConfig {
    pub four_telemetry_route: String,
    pub four_telemetry_files: FourTelemetryFiles,
    pub protocol_mapping_route: String,
    pub protocol_mapping_files: ProtocolMappingFiles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryFiles {
    pub telemetry_file: String,
    pub signal_file: String,
    pub control_file: String,
    pub adjustment_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMappingFiles {
    pub telemetry_mapping: String,
    pub signal_mapping: String,
    pub control_mapping: String,
    pub adjustment_mapping: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub enabled: bool,
    pub level: String,
    pub file: String,
    pub max_size: u64,
    pub max_files: u32,
}

// 通道列表信息（用于显示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: u32,
    pub name: String,
    pub protocol: String,
    pub protocol_type: String,
    pub enabled: bool,
    pub status: ChannelStatus,
    pub point_counts: ChannelPointCounts,
    pub last_update: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelStatus {
    Online,
    Offline,
    Error,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPointCounts {
    pub telemetry: usize,
    pub signal: usize,
    pub control: usize,
    pub adjustment: usize,
}