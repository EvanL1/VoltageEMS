//! Channel configuration types

use super::channel_parameters::ChannelParameters;
use super::logging::ChannelLoggingConfig;
use super::protocol::UnifiedPointMapping;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel ID
    pub id: u16,

    /// Channel name
    pub name: String,

    /// Description
    pub description: Option<String>,

    /// Protocol type
    pub protocol: String,

    /// Protocol parameters
    #[serde(default)]
    pub parameters: HashMap<String, serde_yaml::Value>,

    /// Channel-specific logging configuration
    #[serde(default)]
    pub logging: ChannelLoggingConfig,

    /// Table configuration (new unified approach)
    pub table_config: Option<TableConfig>,

    /// Parsed point mappings (filled after loading, not from YAML)
    #[serde(skip)]
    pub points: Vec<UnifiedPointMapping>,

    /// 加载后的点位数据 (matching figment_demo structure)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub combined_points: Vec<CombinedPoint>,
}

/// Unified table configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableConfig {
    /// Four telemetry route
    pub four_telemetry_route: String,

    /// Four telemetry files
    pub four_telemetry_files: FourTelemetryFiles,

    /// Protocol mapping route
    pub protocol_mapping_route: String,

    /// Protocol mapping files
    pub protocol_mapping_files: ProtocolMappingFiles,
}

/// Four telemetry files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryFiles {
    /// Telemetry file (YC)
    pub telemetry_file: String,

    /// Signal file (YX)
    pub signal_file: String,

    /// Adjustment file (YT)
    pub adjustment_file: String,

    /// Control file (YK)
    pub control_file: String,
}

/// Protocol mapping files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMappingFiles {
    /// Telemetry mapping (YC)
    pub telemetry_mapping: String,

    /// Signal mapping (YX)
    pub signal_mapping: String,

    /// Adjustment mapping (YT)
    pub adjustment_mapping: String,

    /// Control mapping (YK)
    pub control_mapping: String,
}

/// 四遥点位定义 (from figment_demo)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryPoint {
    pub point_id: u32,
    pub signal_name: String,
    pub chinese_name: String,
    pub telemetry_type: String,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    pub unit: Option<String>,
    pub reverse: Option<bool>,
    pub data_type: String,
}

/// 协议映射定义 (from figment_demo)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub protocol_params: HashMap<String, String>,
}

/// 合并后的点位 (from figment_demo)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedPoint {
    pub point_id: u32,
    pub signal_name: String,
    pub chinese_name: String,
    pub telemetry_type: String,
    pub data_type: String,
    pub protocol_params: HashMap<String, String>,
    pub scaling: Option<ScalingInfo>,
}

/// 缩放信息 (from figment_demo)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingInfo {
    pub scale: f64,
    pub offset: f64,
    pub unit: Option<String>,
}

impl ChannelConfig {
    /// Get parameters as ChannelParameters for backward compatibility
    pub fn get_parameters(&self) -> ChannelParameters {
        ChannelParameters::Generic(self.parameters.clone())
    }

    /// Set parameters from ChannelParameters
    pub fn set_parameters(&mut self, params: ChannelParameters) {
        match params {
            ChannelParameters::Generic(map) => {
                self.parameters = map;
            }
            _ => {
                // For other parameter types, we could serialize them to a map
                // For now, just set empty map
                self.parameters = HashMap::new();
            }
        }
    }
}
