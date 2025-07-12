use serde::{Deserialize, Serialize};
use crate::models::protocol_mapping::ProtocolMappingEnum;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointDefinition {
    pub point_id: u32,
    pub signal_name: String,
    pub chinese_name: Option<String>,
    pub data_type: String,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    pub reverse: Option<bool>,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub group: Option<String>,
}

// 使用 protocol_mapping 模块中的映射类型
// pub use crate::models::protocol_mapping::{ModbusMapping as PointMapping};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTable {
    pub id: String,
    pub name: String,
    pub protocol_type: String,
    pub telemetry: Vec<PointDefinition>,
    pub signal: Vec<PointDefinition>,
    pub control: Vec<PointDefinition>,
    pub adjustment: Vec<PointDefinition>,
    // 使用枚举类型支持多协议映射
    pub telemetry_mapping: Vec<ProtocolMappingEnum>,
    pub signal_mapping: Vec<ProtocolMappingEnum>,
    pub control_mapping: Vec<ProtocolMappingEnum>,
    pub adjustment_mapping: Vec<ProtocolMappingEnum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTableMetadata {
    pub id: String,
    pub name: String,
    pub protocol_type: String,
    pub channel_id: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
    pub point_counts: PointCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointCounts {
    pub telemetry: usize,
    pub signal: usize,
    pub control: usize,
    pub adjustment: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CsvType {
    Telemetry,
    Signal,
    Control,
    Adjustment,
    TelemetryMapping,
    SignalMapping,
    ControlMapping,
    AdjustmentMapping,
}

impl CsvType {
    pub fn file_name(&self) -> &str {
        match self {
            CsvType::Telemetry => "telemetry.csv",
            CsvType::Signal => "signal.csv",
            CsvType::Control => "control.csv",
            CsvType::Adjustment => "adjustment.csv",
            CsvType::TelemetryMapping => "mapping_telemetry.csv",
            CsvType::SignalMapping => "mapping_signal.csv",
            CsvType::ControlMapping => "mapping_control.csv",
            CsvType::AdjustmentMapping => "mapping_adjustment.csv",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub row: Option<usize>,
    pub column: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}