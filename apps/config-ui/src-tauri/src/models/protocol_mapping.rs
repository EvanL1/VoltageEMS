use serde::{Deserialize, Serialize};

/// 通用的协议映射特征
pub trait ProtocolMapping {
    fn point_id(&self) -> u32;
    fn signal_name(&self) -> &str;
    fn validate(&self) -> Result<(), String>;
}

/// Modbus协议映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_format: String,
    pub byte_order: Option<String>,
    pub register_count: Option<u16>,
    pub bit_position: Option<u8>,
    pub description: Option<String>,
}

impl ProtocolMapping for ModbusMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn validate(&self) -> Result<(), String> {
        if self.slave_id == 0 || self.slave_id > 247 {
            return Err(format!("Invalid Modbus slave ID: {}. Must be 1-247", self.slave_id));
        }
        
        if self.signal_name.is_empty() {
            return Err("Signal name cannot be empty".to_string());
        }

        // 验证功能码
        match self.function_code {
            1..=4 | 5 | 6 | 15 | 16 => Ok(()),
            _ => Err(format!("Invalid function code: {}", self.function_code)),
        }
    }
}

/// IEC60870协议映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IEC60870Mapping {
    pub point_id: u32,
    pub signal_name: String,
    pub common_address: u16,
    pub ioa_address: u32,
    pub type_id: u8,
    pub cause_of_transmission: Option<u8>,
    pub qualifier: Option<u8>,
    pub description: Option<String>,
}

impl ProtocolMapping for IEC60870Mapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn validate(&self) -> Result<(), String> {
        if self.signal_name.is_empty() {
            return Err("Signal name cannot be empty".to_string());
        }

        // 验证类型标识符
        match self.type_id {
            1..=40 | 45..=51 | 58..=64 | 70 | 100..=107 | 110..=113 | 120..=127 => Ok(()),
            _ => Err(format!("Invalid type ID: {}", self.type_id)),
        }
    }
}

/// CAN协议映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanMapping {
    pub point_id: u32,
    pub signal_name: String,
    pub can_id: u32,
    pub start_bit: u8,
    pub bit_length: u8,
    pub byte_order: String, // "motorola" or "intel"
    pub value_type: String, // "unsigned", "signed", "float", "double"
    pub factor: Option<f64>,
    pub offset: Option<f64>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub unit: Option<String>,
    pub description: Option<String>,
}

impl ProtocolMapping for CanMapping {
    fn point_id(&self) -> u32 {
        self.point_id
    }

    fn signal_name(&self) -> &str {
        &self.signal_name
    }

    fn validate(&self) -> Result<(), String> {
        if self.signal_name.is_empty() {
            return Err("Signal name cannot be empty".to_string());
        }

        // 验证CAN ID范围
        if self.can_id > 0x1FFFFFFF {
            return Err(format!("Invalid CAN ID: {}. Must be <= 0x1FFFFFFF", self.can_id));
        }

        // 验证位长度
        if self.bit_length == 0 || self.bit_length > 64 {
            return Err(format!("Invalid bit length: {}. Must be 1-64", self.bit_length));
        }

        // 验证字节序
        match self.byte_order.as_str() {
            "motorola" | "intel" => Ok(()),
            _ => Err(format!("Invalid byte order: {}. Must be 'motorola' or 'intel'", self.byte_order)),
        }
    }
}

/// 统一的协议映射枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "protocol_type")]
pub enum ProtocolMappingEnum {
    Modbus(ModbusMapping),
    IEC60870(IEC60870Mapping),
    CAN(CanMapping),
}

impl ProtocolMappingEnum {
    pub fn point_id(&self) -> u32 {
        match self {
            ProtocolMappingEnum::Modbus(m) => m.point_id(),
            ProtocolMappingEnum::IEC60870(m) => m.point_id(),
            ProtocolMappingEnum::CAN(m) => m.point_id(),
        }
    }

    pub fn signal_name(&self) -> &str {
        match self {
            ProtocolMappingEnum::Modbus(m) => m.signal_name(),
            ProtocolMappingEnum::IEC60870(m) => m.signal_name(),
            ProtocolMappingEnum::CAN(m) => m.signal_name(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            ProtocolMappingEnum::Modbus(m) => m.validate(),
            ProtocolMappingEnum::IEC60870(m) => m.validate(),
            ProtocolMappingEnum::CAN(m) => m.validate(),
        }
    }
}

/// CSV映射头部定义
pub struct CsvHeaders {
    pub modbus: Vec<&'static str>,
    pub iec60870: Vec<&'static str>,
    pub can: Vec<&'static str>,
}

impl Default for CsvHeaders {
    fn default() -> Self {
        Self {
            modbus: vec![
                "point_id",
                "signal_name",
                "slave_id",
                "function_code",
                "register_address",
                "data_format",
                "byte_order",
                "register_count",
                "bit_position",
                "description",
            ],
            iec60870: vec![
                "point_id",
                "signal_name",
                "common_address",
                "ioa_address",
                "type_id",
                "cause_of_transmission",
                "qualifier",
                "description",
            ],
            can: vec![
                "point_id",
                "signal_name",
                "can_id",
                "start_bit",
                "bit_length",
                "byte_order",
                "value_type",
                "factor",
                "offset",
                "min_value",
                "max_value",
                "unit",
                "description",
            ],
        }
    }
}