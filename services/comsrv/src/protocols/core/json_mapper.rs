//! JSON Payload Mapping Engine for MQTT/HTTP Protocols
//!
//! TODO: Implement JSONPath data extraction
//!
//! Design goals:
//! - Extract data points from JSON payloads using JSONPath
//! - Support timestamp format conversion (Unix/ISO8601)
//! - Support data type conversion and scaling

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
    // TODO: Add compiled JSONPath expressions
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
    /// TODO: Implement loading JSONPath mapping config from database
    pub async fn from_database(_pool: &SqlitePool, channel_id: u32) -> Result<Self> {
        Ok(Self::new(channel_id))
    }

    /// Configure timestamp extraction
    ///
    /// TODO: Implement timestamp configuration
    pub fn with_config(self, _config: &JsonMappingConfig) -> Result<Self> {
        Ok(self)
    }

    /// Parse JSON payload and extract data points
    ///
    /// TODO: Implement JSON parsing and data extraction
    pub fn parse(&self, _payload: &[u8]) -> Result<DataBatch> {
        Err(GatewayError::Config(
            "JSON mapping not implemented yet".to_string(),
        ))
    }

    /// Parse from already-parsed JSON value
    ///
    /// TODO: Implement
    pub fn parse_value(&self, _json: &serde_json::Value) -> Result<DataBatch> {
        Err(GatewayError::Config(
            "JSON mapping not implemented yet".to_string(),
        ))
    }

    /// Extract device ID from payload
    ///
    /// TODO: Implement
    pub fn extract_device_id(&self, _json: &serde_json::Value) -> Option<String> {
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
