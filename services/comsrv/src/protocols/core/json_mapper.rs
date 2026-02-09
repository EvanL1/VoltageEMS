//! JSON Payload Mapping Engine for MQTT/HTTP Protocols
//!
//! TODO: 实现 JSONPath 数据提取功能
//!
//! 设计目标：
//! - 从 JSON payload 中使用 JSONPath 提取数据点
//! - 支持时间戳格式转换（Unix/ISO8601）
//! - 支持数据类型转换和缩放

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use voltage_model::PointType;

use super::data::DataBatch;
use super::error::{GatewayError, Result};

/// Timestamp format in JSON payload
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    UnixSeconds,
    #[default]
    UnixMillis,
    Iso8601,
    Now,
}

/// Data type for JSON value extraction
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JsonDataType {
    #[default]
    Float,
    Int,
    Bool,
    String,
}

/// A pre-compiled point mapping (stub)
#[derive(Debug)]
pub struct CompiledMapping {
    pub point_id: u32,
    pub point_type: PointType,
    // TODO: 添加 JSONPath 编译后的表达式
}

/// JSON mapping configuration for a channel
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JsonMappingConfig {
    #[serde(default)]
    pub device_id_path: Option<String>,
    #[serde(default)]
    pub timestamp_path: Option<String>,
    #[serde(default)]
    pub timestamp_format: TimestampFormat,
}

/// JSON payload mapper for a channel (stub)
#[derive(Debug)]
pub struct JsonMapper {
    pub channel_id: u32,
    pub mappings: Vec<CompiledMapping>,
}

impl JsonMapper {
    /// Create a new empty mapper
    pub fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            mappings: Vec::new(),
        }
    }

    /// Load mapper from database
    ///
    /// TODO: 实现从数据库加载 JSONPath 映射配置
    pub async fn from_database(_pool: &SqlitePool, channel_id: u32) -> Result<Self> {
        Ok(Self::new(channel_id))
    }

    /// Configure timestamp extraction
    ///
    /// TODO: 实现时间戳配置
    pub fn with_config(self, _config: &JsonMappingConfig) -> Result<Self> {
        Ok(self)
    }

    /// Parse JSON payload and extract data points
    ///
    /// TODO: 实现 JSON 解析和数据提取
    pub fn parse(&self, _payload: &[u8]) -> Result<DataBatch> {
        Err(GatewayError::Config(
            "JSON mapping not implemented yet".to_string(),
        ))
    }

    /// Parse from already-parsed JSON value
    ///
    /// TODO: 实现
    pub fn parse_value(&self, _json: &serde_json::Value) -> Result<DataBatch> {
        Err(GatewayError::Config(
            "JSON mapping not implemented yet".to_string(),
        ))
    }

    /// Extract device ID from payload
    ///
    /// TODO: 实现
    pub fn extract_device_id(&self, _json: &serde_json::Value) -> Option<String> {
        None
    }

    /// Extract timestamp from JSON
    ///
    /// TODO: 实现
    #[allow(dead_code)]
    fn extract_timestamp(&self, _json: &serde_json::Value) -> Option<DateTime<Utc>> {
        None
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    pub fn len(&self) -> usize {
        self.mappings.len()
    }
}

/// Thread-safe shared mapper reference
pub type SharedJsonMapper = Arc<JsonMapper>;
