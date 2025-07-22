//! 设备模型类型定义
//!
//! 提供设备模型相关的类型和工具函数

use crate::comsrv_interface::ControlCommand;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 设备实例状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    /// 在线
    Online,
    /// 离线
    Offline,
    /// 故障
    Fault,
    /// 维护中
    Maintenance,
    /// 未知
    Unknown,
}

/// 数据质量
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DataQuality {
    /// 良好
    Good,
    /// 不确定
    Uncertain,
    /// 坏值
    Bad,
    /// 未采集
    NotCollected,
}

/// 设备实例数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceData {
    /// 设备实例ID
    pub instance_id: String,
    /// 时间戳
    pub timestamp: i64,
    /// 属性值
    pub properties: HashMap<String, PropertyValue>,
    /// 遥测值
    pub telemetry: HashMap<String, TelemetryValue>,
    /// 设备状态
    pub status: DeviceStatus,
}

/// 属性值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyValue {
    pub value: serde_json::Value,
    pub timestamp: i64,
    pub quality: DataQuality,
}

/// 遥测值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryValue {
    pub value: serde_json::Value,
    pub timestamp: i64,
    pub quality: DataQuality,
    pub raw_value: Option<f64>,
}

/// 命令请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    /// 请求ID
    pub request_id: String,
    /// 设备实例ID
    pub instance_id: String,
    /// 命令标识
    pub command: String,
    /// 命令参数
    pub params: HashMap<String, serde_json::Value>,
    /// 请求时间戳
    pub timestamp: i64,
}

/// 命令响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    /// 请求ID
    pub request_id: String,
    /// 是否成功
    pub success: bool,
    /// 结果数据
    pub result: Option<HashMap<String, serde_json::Value>>,
    /// 错误信息
    pub error: Option<String>,
    /// 响应时间戳
    pub timestamp: i64,
}

/// 设备事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEvent {
    /// 事件ID
    pub event_id: String,
    /// 设备实例ID
    pub instance_id: String,
    /// 事件标识
    pub event: String,
    /// 事件类型
    pub event_type: crate::device_model::EventType,
    /// 事件参数
    pub params: HashMap<String, serde_json::Value>,
    /// 事件时间戳
    pub timestamp: i64,
}

/// 计算结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationResult {
    /// 计算标识
    pub calculation_id: String,
    /// 输出值
    pub outputs: HashMap<String, serde_json::Value>,
    /// 计算时间戳
    pub timestamp: i64,
    /// 执行时长（毫秒）
    pub duration_ms: u64,
}

/// 模型元数据键
pub mod metadata_keys {
    /// 制造商
    pub const MANUFACTURER: &str = "manufacturer";
    /// 型号
    pub const MODEL_NUMBER: &str = "model_number";
    /// 协议版本
    pub const PROTOCOL_VERSION: &str = "protocol_version";
    /// 创建时间
    pub const CREATED_AT: &str = "created_at";
    /// 更新时间
    pub const UPDATED_AT: &str = "updated_at";
    /// 作者
    pub const AUTHOR: &str = "author";
}

/// 数据类型转换工具
impl crate::device_model::DataType {
    /// 验证值是否符合数据类型
    pub fn validate_value(&self, value: &serde_json::Value) -> bool {
        match (self, value) {
            (crate::device_model::DataType::Bool, serde_json::Value::Bool(_)) => true,
            (crate::device_model::DataType::Int32, serde_json::Value::Number(n)) => n
                .as_i64()
                .map(|v| v >= i32::MIN as i64 && v <= i32::MAX as i64)
                .unwrap_or(false),
            (crate::device_model::DataType::Int64, serde_json::Value::Number(n)) => {
                n.as_i64().is_some()
            }
            (
                crate::device_model::DataType::Float32 | crate::device_model::DataType::Float64,
                serde_json::Value::Number(_),
            ) => true,
            (crate::device_model::DataType::String, serde_json::Value::String(_)) => true,
            (crate::device_model::DataType::Array(inner), serde_json::Value::Array(arr)) => {
                arr.iter().all(|v| inner.validate_value(v))
            }
            (crate::device_model::DataType::Object, serde_json::Value::Object(_)) => true,
            (crate::device_model::DataType::Timestamp, serde_json::Value::Number(n)) => {
                n.as_i64().is_some()
            }
            (crate::device_model::DataType::Binary, serde_json::Value::String(s)) => {
                // 检查是否是有效的base64编码
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.decode(s).is_ok()
            }
            _ => false,
        }
    }

    /// 获取默认值
    pub fn default_value(&self) -> serde_json::Value {
        match self {
            crate::device_model::DataType::Bool => serde_json::Value::Bool(false),
            crate::device_model::DataType::Int32 | crate::device_model::DataType::Int64 => {
                serde_json::Value::Number(0.into())
            }
            crate::device_model::DataType::Float32 | crate::device_model::DataType::Float64 => {
                serde_json::Value::Number(serde_json::Number::from_f64(0.0).unwrap())
            }
            crate::device_model::DataType::String => serde_json::Value::String(String::new()),
            crate::device_model::DataType::Array(_) => serde_json::Value::Array(vec![]),
            crate::device_model::DataType::Object => {
                serde_json::Value::Object(serde_json::Map::new())
            }
            crate::device_model::DataType::Timestamp => serde_json::Value::Number(0.into()),
            crate::device_model::DataType::Binary => serde_json::Value::String(String::new()),
        }
    }
}

/// 值转换工具
pub struct ValueTransformer;

impl ValueTransformer {
    /// 应用转换规则
    pub fn apply_transform(
        value: f64,
        transform: &crate::device_model::TransformRule,
    ) -> Result<serde_json::Value, String> {
        match transform {
            crate::device_model::TransformRule::Linear { scale, offset } => {
                let result = value * scale + offset;
                Ok(serde_json::Value::Number(
                    serde_json::Number::from_f64(result)
                        .ok_or_else(|| "Invalid float value".to_string())?,
                ))
            }
            crate::device_model::TransformRule::Expression(expr) => {
                // TODO: 实现表达式计算
                Err(format!("Expression evaluation not implemented: {}", expr))
            }
            crate::device_model::TransformRule::LookupTable(table) => {
                let key = value.to_string();
                table
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| format!("Value {} not found in lookup table", key))
            }
            crate::device_model::TransformRule::Function(name) => {
                // TODO: 实现自定义函数调用
                Err(format!("Function execution not implemented: {}", name))
            }
        }
    }
}

/// 设备模型命令映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMapping {
    /// 命令名称
    pub command_name: String,
    /// 目标通道ID
    pub channel_id: u16,
    /// 点位类型 (c=control, a=adjustment)
    pub point_type: String,
    /// 点位ID
    pub point_id: u32,
    /// 参数映射
    pub parameter_mapping: HashMap<String, String>,
    /// 默认值
    pub default_values: HashMap<String, serde_json::Value>,
}

/// 设备模型到comsrv的命令转换器
pub struct CommandTransformer;

impl CommandTransformer {
    /// 将设备模型CommandRequest转换为comsrv ControlCommand
    pub fn transform_to_control_command(
        request: &CommandRequest,
        mapping: &CommandMapping,
    ) -> Result<ControlCommand, String> {
        // 验证命令名称
        if request.command != mapping.command_name {
            return Err(format!(
                "Command name mismatch: expected '{}', got '{}'",
                mapping.command_name, request.command
            ));
        }

        // 提取控制值
        let control_value = Self::extract_control_value(request, mapping)?;

        // 创建控制命令
        let control_command = ControlCommand::new(
            mapping.channel_id,
            &mapping.point_type,
            mapping.point_id,
            control_value,
        );

        Ok(control_command)
    }

    /// 从命令请求中提取控制值
    fn extract_control_value(
        request: &CommandRequest,
        mapping: &CommandMapping,
    ) -> Result<f64, String> {
        // 根据命令类型提取值
        match request.command.as_str() {
            "start_motor" => {
                // 启动电机通常发送1.0
                Ok(1.0)
            }
            "stop_motor" => {
                // 停止电机通常发送0.0
                Ok(0.0)
            }
            "change_speed" => {
                // 变速需要从参数中提取目标速度
                request
                    .params
                    .get("target_speed")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| "Missing or invalid target_speed parameter".to_string())
            }
            "set_position" => {
                // 位置设定需要从参数中提取目标位置
                request
                    .params
                    .get("target_position")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| "Missing or invalid target_position parameter".to_string())
            }
            _ => {
                // 通用参数提取：尝试从参数中提取"value"字段
                request
                    .params
                    .get("value")
                    .and_then(|v| v.as_f64())
                    .or_else(|| {
                        // 如果没有value字段，尝试从默认值获取
                        mapping.default_values.get("value").and_then(|v| v.as_f64())
                    })
                    .ok_or_else(|| {
                        format!(
                            "Unable to extract control value for command '{}'",
                            request.command
                        )
                    })
            }
        }
    }

    /// 创建标准的命令映射配置
    pub fn create_standard_mappings() -> HashMap<String, CommandMapping> {
        let mut mappings = HashMap::new();

        // 电机启动命令
        mappings.insert(
            "start_motor".to_string(),
            CommandMapping {
                command_name: "start_motor".to_string(),
                channel_id: 1001,
                point_type: "c".to_string(),
                point_id: 30001,
                parameter_mapping: HashMap::new(),
                default_values: [("value".to_string(), serde_json::json!(1.0))]
                    .into_iter()
                    .collect(),
            },
        );

        // 电机停止命令
        mappings.insert(
            "stop_motor".to_string(),
            CommandMapping {
                command_name: "stop_motor".to_string(),
                channel_id: 1001,
                point_type: "c".to_string(),
                point_id: 30001,
                parameter_mapping: HashMap::new(),
                default_values: [("value".to_string(), serde_json::json!(0.0))]
                    .into_iter()
                    .collect(),
            },
        );

        // 速度调节命令
        mappings.insert(
            "change_speed".to_string(),
            CommandMapping {
                command_name: "change_speed".to_string(),
                channel_id: 1001,
                point_type: "a".to_string(),
                point_id: 30002,
                parameter_mapping: [("target_speed".to_string(), "value".to_string())]
                    .into_iter()
                    .collect(),
                default_values: HashMap::new(),
            },
        );

        // 位置设定命令
        mappings.insert(
            "set_position".to_string(),
            CommandMapping {
                command_name: "set_position".to_string(),
                channel_id: 1001,
                point_type: "a".to_string(),
                point_id: 30003,
                parameter_mapping: [("target_position".to_string(), "value".to_string())]
                    .into_iter()
                    .collect(),
                default_values: HashMap::new(),
            },
        );

        mappings
    }

    /// 批量转换命令
    pub fn batch_transform_commands(
        requests: Vec<CommandRequest>,
        mappings: &HashMap<String, CommandMapping>,
    ) -> Result<Vec<ControlCommand>, Vec<String>> {
        let mut control_commands = Vec::new();
        let mut errors = Vec::new();

        for request in requests {
            match mappings.get(&request.command) {
                Some(mapping) => match Self::transform_to_control_command(&request, mapping) {
                    Ok(control_command) => control_commands.push(control_command),
                    Err(e) => errors.push(format!(
                        "Failed to transform command '{}': {}",
                        request.command, e
                    )),
                },
                None => errors.push(format!(
                    "No mapping found for command '{}'",
                    request.command
                )),
            }
        }

        if errors.is_empty() {
            Ok(control_commands)
        } else {
            Err(errors)
        }
    }
}

/// 数据格式转换器
pub struct DataFormatConverter;

impl DataFormatConverter {
    /// 将comsrv的点位数据转换为设备模型的遥测数据
    pub fn convert_comsrv_to_telemetry(
        comsrv_data: &str,
        point_type: &str,
    ) -> Result<TelemetryValue, String> {
        // 解析comsrv格式: value:timestamp
        let parts: Vec<&str> = comsrv_data.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid comsrv data format: {}", comsrv_data));
        }

        let value = parts[0]
            .parse::<f64>()
            .map_err(|e| format!("Invalid value: {}", e))?;
        let timestamp = parts[1]
            .parse::<i64>()
            .map_err(|e| format!("Invalid timestamp: {}", e))?;

        // 根据点位类型确定数据质量
        let quality = match point_type {
            "m" | "s" => DataQuality::Good, // 遥测和遥信通常质量良好
            "c" | "a" => DataQuality::Good, // 控制和调节点位
            _ => DataQuality::Uncertain,
        };

        Ok(TelemetryValue {
            value: serde_json::Value::Number(
                serde_json::Number::from_f64(value)
                    .ok_or_else(|| "Invalid float value".to_string())?,
            ),
            timestamp,
            quality,
            raw_value: Some(value),
        })
    }

    /// 将设备模型的遥测数据转换为comsrv格式
    pub fn convert_telemetry_to_comsrv(telemetry: &TelemetryValue) -> Result<String, String> {
        let value = telemetry.raw_value.unwrap_or_else(|| {
            // 如果没有原始值，尝试从JSON值提取
            match &telemetry.value {
                serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
                serde_json::Value::Bool(b) => {
                    if *b {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            }
        });

        Ok(format!("{}:{}", value, telemetry.timestamp))
    }

    /// 批量转换comsrv数据
    pub fn batch_convert_comsrv_to_telemetry(
        comsrv_data: HashMap<String, (String, String)>, // key -> (data, point_type)
    ) -> HashMap<String, Result<TelemetryValue, String>> {
        comsrv_data
            .into_iter()
            .map(|(key, (data, point_type))| {
                let result = Self::convert_comsrv_to_telemetry(&data, &point_type);
                (key, result)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_validation() {
        let int_type = crate::device_model::DataType::Int32;
        assert!(int_type.validate_value(&serde_json::json!(42)));
        assert!(!int_type.validate_value(&serde_json::json!("string")));

        let array_type =
            crate::device_model::DataType::Array(Box::new(crate::device_model::DataType::String));
        assert!(array_type.validate_value(&serde_json::json!(["a", "b"])));
        assert!(!array_type.validate_value(&serde_json::json!([1, 2])));
    }

    #[test]
    fn test_value_transformer() {
        let transform = crate::device_model::TransformRule::Linear {
            scale: 2.0,
            offset: 10.0,
        };

        let result = ValueTransformer::apply_transform(5.0, &transform).unwrap();
        assert_eq!(result, serde_json::json!(20.0));
    }

    #[test]
    fn test_command_transformer() {
        let request = CommandRequest {
            request_id: "test_request".to_string(),
            instance_id: "test_instance".to_string(),
            command: "start_motor".to_string(),
            params: HashMap::new(),
            timestamp: 123456789,
        };

        let mapping = CommandMapping {
            command_name: "start_motor".to_string(),
            channel_id: 1001,
            point_type: "c".to_string(),
            point_id: 30001,
            parameter_mapping: HashMap::new(),
            default_values: HashMap::new(),
        };

        let control_command =
            CommandTransformer::transform_to_control_command(&request, &mapping).unwrap();
        assert_eq!(control_command.channel_id, 1001);
        assert_eq!(control_command.point_type, "c");
        assert_eq!(control_command.point_id, 30001);
        assert_eq!(control_command.value, 1.0);
    }

    #[test]
    fn test_data_format_converter() {
        // 测试comsrv到遥测数据转换
        let comsrv_data = "25.6:1234567890";
        let telemetry = DataFormatConverter::convert_comsrv_to_telemetry(comsrv_data, "m").unwrap();

        assert_eq!(telemetry.raw_value, Some(25.6));
        assert_eq!(telemetry.timestamp, 1234567890);
        assert_eq!(telemetry.quality, DataQuality::Good);

        // 测试遥测数据到comsrv格式转换
        let converted_back = DataFormatConverter::convert_telemetry_to_comsrv(&telemetry).unwrap();
        assert_eq!(converted_back, "25.6:1234567890");
    }

    #[test]
    fn test_standard_mappings() {
        let mappings = CommandTransformer::create_standard_mappings();

        // 测试电机启动命令映射
        let start_mapping = mappings.get("start_motor").unwrap();
        assert_eq!(start_mapping.channel_id, 1001);
        assert_eq!(start_mapping.point_type, "c");
        assert_eq!(start_mapping.point_id, 30001);

        // 测试速度调节命令映射
        let speed_mapping = mappings.get("change_speed").unwrap();
        assert_eq!(speed_mapping.channel_id, 1001);
        assert_eq!(speed_mapping.point_type, "a");
        assert_eq!(speed_mapping.point_id, 30002);
    }
}
