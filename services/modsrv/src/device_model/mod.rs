//! 物理设备模型定义
//!
//! 提供设备物模型的核心定义和管理功能

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod calculation;
pub mod dataflow;
pub mod instance;
pub mod integration;
pub mod registry;
pub mod types;

pub use calculation::*;
pub use dataflow::{DataFlowConfig, DataFlowProcessor};
pub use instance::*;
pub use integration::DeviceModelSystem;
pub use dataflow::DataUpdate;
pub use registry::*;
pub use types::*;

/// 设备模型定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceModel {
    /// 模型ID
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 模型版本
    pub version: String,
    /// 模型描述
    pub description: String,
    /// 设备类型
    pub device_type: DeviceType,
    /// 属性定义
    pub properties: Vec<PropertyDefinition>,
    /// 遥测点定义
    pub telemetry: Vec<TelemetryDefinition>,
    /// 命令定义
    pub commands: Vec<CommandDefinition>,
    /// 事件定义
    pub events: Vec<EventDefinition>,
    /// 计算模型
    pub calculations: Vec<CalculationDefinition>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// Device type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    /// Sensor device
    Sensor,
    /// Actuator device
    Actuator,
    /// Gateway device
    Gateway,
    /// Edge computing device
    Edge,
    /// Industrial equipment
    Industrial,
    /// Energy storage system
    EnergyStorage,
    /// Diesel generator
    DieselGenerator,
    /// Power meter
    PowerMeter,
    /// Photovoltaic system
    Photovoltaic,
    /// Custom device type
    Custom(String),
}

/// 属性定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDefinition {
    /// 属性标识
    pub identifier: String,
    /// 属性名称
    pub name: String,
    /// 数据类型
    pub data_type: DataType,
    /// 是否必需
    pub required: bool,
    /// 默认值
    pub default_value: Option<serde_json::Value>,
    /// 约束条件
    pub constraints: Option<Constraints>,
    /// 单位
    pub unit: Option<String>,
    /// 描述
    pub description: Option<String>,
}

/// 遥测点定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryDefinition {
    /// 遥测标识
    pub identifier: String,
    /// 遥测名称
    pub name: String,
    /// 数据类型
    pub data_type: DataType,
    /// 采集方式
    pub collection_type: CollectionType,
    /// 映射配置
    pub mapping: TelemetryMapping,
    /// 转换规则
    pub transform: Option<TransformRule>,
    /// 单位
    pub unit: Option<String>,
    /// 描述
    pub description: Option<String>,
}

/// 命令定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
    /// 命令标识
    pub identifier: String,
    /// 命令名称
    pub name: String,
    /// 命令类型
    pub command_type: CommandType,
    /// 输入参数
    pub input_params: Vec<ParamDefinition>,
    /// 输出参数
    pub output_params: Vec<ParamDefinition>,
    /// 映射配置
    pub mapping: CommandMapping,
    /// 描述
    pub description: Option<String>,
}

/// 事件定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDefinition {
    /// 事件标识
    pub identifier: String,
    /// 事件名称
    pub name: String,
    /// 事件类型
    pub event_type: EventType,
    /// 触发条件
    pub trigger: TriggerCondition,
    /// 事件参数
    pub params: Vec<ParamDefinition>,
    /// 描述
    pub description: Option<String>,
}

/// 计算定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationDefinition {
    /// 计算标识
    pub identifier: String,
    /// 计算名称
    pub name: String,
    /// 输入变量
    pub inputs: Vec<String>,
    /// 输出变量
    pub outputs: Vec<String>,
    /// 计算表达式或脚本
    pub expression: CalculationExpression,
    /// 执行条件
    pub condition: Option<String>,
    /// 描述
    pub description: Option<String>,
}

/// 数据类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Bool,
    Int32,
    Int64,
    Float32,
    Float64,
    String,
    Binary,
    Array(Box<DataType>),
    Object,
    Timestamp,
}

/// 采集类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionType {
    /// 周期采集
    Periodic { interval_ms: u64 },
    /// 变化采集
    OnChange { threshold: Option<f64> },
    /// 事件驱动
    EventDriven,
    /// 混合模式
    Hybrid {
        interval_ms: u64,
        change_threshold: Option<f64>,
    },
}

/// 遥测映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryMapping {
    /// 通道ID
    pub channel_id: u16,
    /// 点类型
    pub point_type: String,
    /// 点ID
    pub point_id: u32,
    /// 缩放因子
    pub scale: Option<f64>,
    /// 偏移量
    pub offset: Option<f64>,
}

/// 命令映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMapping {
    /// 通道ID
    pub channel_id: u16,
    /// 命令类型
    pub command_type: String,
    /// 点ID
    pub point_id: u32,
    /// 值映射
    pub value_mapping: Option<HashMap<String, f64>>,
}

/// 参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDefinition {
    pub name: String,
    pub data_type: DataType,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
}

/// 约束条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub enum_values: Option<Vec<serde_json::Value>>,
    pub pattern: Option<String>,
}

/// 转换规则
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformRule {
    /// 线性变换
    Linear { scale: f64, offset: f64 },
    /// 表达式
    Expression(String),
    /// 查找表
    LookupTable(HashMap<String, serde_json::Value>),
    /// 自定义函数
    Function(String),
}

/// 命令类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    /// 控制命令
    Control,
    /// 设置命令
    Setting,
    /// 查询命令
    Query,
    /// 动作命令
    Action,
}

/// 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// 信息事件
    Info,
    /// 告警事件
    Alarm,
    /// 故障事件
    Fault,
    /// 状态变化
    StateChange,
}

/// 触发条件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerCondition {
    /// 阈值触发
    Threshold {
        variable: String,
        operator: String,
        value: f64,
    },
    /// 表达式触发
    Expression(String),
    /// 状态变化触发
    StateChange {
        variable: String,
        from: Option<serde_json::Value>,
        to: Option<serde_json::Value>,
    },
    /// 定时触发
    Scheduled { cron: String },
}

/// 计算表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculationExpression {
    /// 数学表达式
    Math(String),
    /// JavaScript代码
    JavaScript(String),
    /// Python代码
    Python(String),
    /// 内置函数
    BuiltIn { function: String, args: Vec<String> },
}

impl DeviceModel {
    /// 验证模型定义
    pub fn validate(&self) -> Result<(), String> {
        // 检查ID和名称
        if self.id.is_empty() {
            return Err("Model ID cannot be empty".to_string());
        }
        if self.name.is_empty() {
            return Err("Model name cannot be empty".to_string());
        }


        // 检查属性定义
        let mut identifiers = std::collections::HashSet::<String>::new();
        for prop in &self.properties {
            if !identifiers.insert(prop.identifier.clone()) {
                return Err(format!(
                    "Duplicate property identifier: {}",
                    prop.identifier
                ));
            }
        }

        // 检查遥测定义
        for telemetry in &self.telemetry {
            if !identifiers.insert(telemetry.identifier.clone()) {
                return Err(format!(
                    "Duplicate telemetry identifier: {}",
                    telemetry.identifier
                ));
            }
        }

        // 检查命令定义
        for command in &self.commands {
            if !identifiers.insert(command.identifier.clone()) {
                return Err(format!(
                    "Duplicate command identifier: {}",
                    command.identifier
                ));
            }
        }

        // 检查计算定义的输入输出
        for calc in &self.calculations {
            for input in &calc.inputs {
                if !identifiers.contains(input) {
                    return Err(format!("Calculation input '{}' not found in model", input));
                }
            }
        }

        Ok(())
    }

    /// 获取所有数据点
    pub fn get_all_points(&self) -> Vec<&str> {
        let mut points = Vec::new();

        for prop in &self.properties {
            points.push(prop.identifier.as_str());
        }

        for telemetry in &self.telemetry {
            points.push(telemetry.identifier.as_str());
        }

        points
    }

    /// Check if device is an energy device
    pub fn is_energy_device(&self) -> bool {
        matches!(
            self.device_type,
            DeviceType::EnergyStorage
                | DeviceType::DieselGenerator
                | DeviceType::PowerMeter
                | DeviceType::Photovoltaic
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_model_validation() {
        let mut model = DeviceModel {
            id: "test_model".to_string(),
            name: "Test Model".to_string(),
            version: "1.0.0".to_string(),
            description: "Test device model".to_string(),
            device_type: DeviceType::Sensor,
            properties: vec![PropertyDefinition {
                identifier: "prop1".to_string(),
                name: "Property 1".to_string(),
                data_type: DataType::Float64,
                required: true,
                default_value: None,
                constraints: None,
                unit: Some("kW".to_string()),
                description: None,
            }],
            telemetry: vec![],
            commands: vec![],
            events: vec![],
            calculations: vec![],
            metadata: HashMap::new(),
        };

        assert!(model.validate().is_ok());

        // 测试重复标识符
        model.telemetry.push(TelemetryDefinition {
            identifier: "prop1".to_string(),
            name: "Telemetry 1".to_string(),
            data_type: DataType::Float64,
            collection_type: CollectionType::Periodic { interval_ms: 1000 },
            mapping: TelemetryMapping {
                channel_id: 1,
                point_type: "YC".to_string(),
                point_id: 1001,
                scale: None,
                offset: None,
            },
            transform: None,
            unit: None,
            description: None,
        });

        assert!(model.validate().is_err());
    }
}
