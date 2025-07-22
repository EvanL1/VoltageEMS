//! hissrv 内部数据类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// 通用的点数据结构，用于解析来自不同服务的数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericPointData {
    /// 通道 ID
    pub channel_id: u32,
    /// 点位 ID
    pub point_id: u32,
    /// 通用值，可以是任何 JSON 类型
    pub value: JsonValue,
    /// 质量标识（可选）
    #[serde(default)]
    pub quality: Option<u8>,
    /// 时间戳
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    /// 额外的元数据
    #[serde(default)]
    pub metadata: HashMap<String, JsonValue>,
}

impl GenericPointData {
    /// 创建新的通用点数据
    pub fn new(channel_id: u32, point_id: u32, value: JsonValue) -> Self {
        Self {
            channel_id,
            point_id,
            value,
            quality: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// 设置质量标识
    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = Some(quality);
        self
    }

    /// 设置时间戳
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: String, value: JsonValue) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// 尝试从 JSON 字符串解析
    pub fn from_json_str(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// 转换为 JSON 字符串
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// 点值类型，用于内部处理
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PointValue {
    /// 浮点数值
    Float(f64),
    /// 整数值
    Integer(i64),
    /// 字符串值
    String(String),
    /// 布尔值
    Boolean(bool),
    /// 二进制数据（base64 编码）
    Binary(String),
    /// 空值
    Null,
}

impl From<JsonValue> for PointValue {
    fn from(value: JsonValue) -> Self {
        match value {
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    PointValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    PointValue::Float(f)
                } else {
                    PointValue::Null
                }
            }
            JsonValue::String(s) => PointValue::String(s),
            JsonValue::Bool(b) => PointValue::Boolean(b),
            JsonValue::Null => PointValue::Null,
            _ => PointValue::String(value.to_string()),
        }
    }
}

impl From<PointValue> for JsonValue {
    fn from(value: PointValue) -> Self {
        match value {
            PointValue::Float(f) => JsonValue::from(f),
            PointValue::Integer(i) => JsonValue::from(i),
            PointValue::String(s) => JsonValue::String(s),
            PointValue::Boolean(b) => JsonValue::Bool(b),
            PointValue::Binary(b) => JsonValue::String(b),
            PointValue::Null => JsonValue::Null,
        }
    }
}

/// 数据质量常量
pub struct Quality;

impl Quality {
    /// 良好
    pub const GOOD: u8 = 192;
    /// 不良
    pub const BAD: u8 = 0;
    /// 不确定
    pub const UNCERTAIN: u8 = 64;
}

/// 消息格式类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageFormat {
    /// comsrv 格式
    ComSrv,
    /// modsrv 格式
    ModSrv,
    /// 通用 JSON 格式
    Generic,
    /// 自定义格式
    Custom(String),
}

impl Default for MessageFormat {
    fn default() -> Self {
        MessageFormat::Generic
    }
}

/// Redis 键解析配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyParseConfig {
    /// 键的分隔符
    #[serde(default = "default_separator")]
    pub separator: String,
    /// 键的格式模板，例如 "{channel}:{type}:{point}"
    pub key_pattern: String,
    /// 字段映射
    pub field_mapping: HashMap<String, usize>,
}

fn default_separator() -> String {
    ":".to_string()
}

impl Default for KeyParseConfig {
    fn default() -> Self {
        let mut field_mapping = HashMap::new();
        field_mapping.insert("channel".to_string(), 0);
        field_mapping.insert("type".to_string(), 1);
        field_mapping.insert("point".to_string(), 2);

        Self {
            separator: default_separator(),
            key_pattern: "{channel}:{type}:{point}".to_string(),
            field_mapping,
        }
    }
}

impl KeyParseConfig {
    /// 解析 Redis 键
    pub fn parse_key(&self, key: &str) -> Option<HashMap<String, String>> {
        let parts: Vec<&str> = key.split(&self.separator).collect();
        let mut result = HashMap::new();

        for (field, index) in &self.field_mapping {
            if let Some(value) = parts.get(*index) {
                result.insert(field.clone(), value.to_string());
            }
        }

        if result.len() == self.field_mapping.len() {
            Some(result)
        } else {
            None
        }
    }
}
