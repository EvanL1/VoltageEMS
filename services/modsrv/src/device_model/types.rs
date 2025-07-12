//! 设备模型类型定义
//!
//! 提供设备模型相关的类型和工具函数

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
    pub event_type: super::EventType,
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
impl super::DataType {
    /// 验证值是否符合数据类型
    pub fn validate_value(&self, value: &serde_json::Value) -> bool {
        match (self, value) {
            (super::DataType::Bool, serde_json::Value::Bool(_)) => true,
            (super::DataType::Int32, serde_json::Value::Number(n)) => n
                .as_i64()
                .map(|v| v >= i32::MIN as i64 && v <= i32::MAX as i64)
                .unwrap_or(false),
            (super::DataType::Int64, serde_json::Value::Number(n)) => n.as_i64().is_some(),
            (super::DataType::Float32 | super::DataType::Float64, serde_json::Value::Number(_)) => {
                true
            }
            (super::DataType::String, serde_json::Value::String(_)) => true,
            (super::DataType::Array(inner), serde_json::Value::Array(arr)) => {
                arr.iter().all(|v| inner.validate_value(v))
            }
            (super::DataType::Object, serde_json::Value::Object(_)) => true,
            (super::DataType::Timestamp, serde_json::Value::Number(n)) => n.as_i64().is_some(),
            (super::DataType::Binary, serde_json::Value::String(s)) => {
                // 检查是否是有效的base64编码
                base64::decode(s).is_ok()
            }
            _ => false,
        }
    }

    /// 获取默认值
    pub fn default_value(&self) -> serde_json::Value {
        match self {
            super::DataType::Bool => serde_json::Value::Bool(false),
            super::DataType::Int32 | super::DataType::Int64 => serde_json::Value::Number(0.into()),
            super::DataType::Float32 | super::DataType::Float64 => {
                serde_json::Value::Number(serde_json::Number::from_f64(0.0).unwrap())
            }
            super::DataType::String => serde_json::Value::String(String::new()),
            super::DataType::Array(_) => serde_json::Value::Array(vec![]),
            super::DataType::Object => serde_json::Value::Object(serde_json::Map::new()),
            super::DataType::Timestamp => serde_json::Value::Number(0.into()),
            super::DataType::Binary => serde_json::Value::String(String::new()),
        }
    }
}

/// 值转换工具
pub struct ValueTransformer;

impl ValueTransformer {
    /// 应用转换规则
    pub fn apply_transform(
        value: f64,
        transform: &super::TransformRule,
    ) -> Result<serde_json::Value, String> {
        match transform {
            super::TransformRule::Linear { scale, offset } => {
                let result = value * scale + offset;
                Ok(serde_json::Value::Number(
                    serde_json::Number::from_f64(result)
                        .ok_or_else(|| "Invalid float value".to_string())?,
                ))
            }
            super::TransformRule::Expression(expr) => {
                // TODO: 实现表达式计算
                Err(format!("Expression evaluation not implemented: {}", expr))
            }
            super::TransformRule::LookupTable(table) => {
                let key = value.to_string();
                table
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| format!("Value {} not found in lookup table", key))
            }
            super::TransformRule::Function(name) => {
                // TODO: 实现自定义函数调用
                Err(format!("Function execution not implemented: {}", name))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_validation() {
        let int_type = super::super::DataType::Int32;
        assert!(int_type.validate_value(&serde_json::json!(42)));
        assert!(!int_type.validate_value(&serde_json::json!("string")));

        let array_type = super::super::DataType::Array(Box::new(super::super::DataType::String));
        assert!(array_type.validate_value(&serde_json::json!(["a", "b"])));
        assert!(!array_type.validate_value(&serde_json::json!([1, 2])));
    }

    #[test]
    fn test_value_transformer() {
        let transform = super::super::TransformRule::Linear {
            scale: 2.0,
            offset: 10.0,
        };

        let result = ValueTransformer::apply_transform(5.0, &transform).unwrap();
        assert_eq!(result, serde_json::json!(20.0));
    }
}
