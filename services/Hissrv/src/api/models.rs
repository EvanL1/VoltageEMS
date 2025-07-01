use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::storage::{DataPoint, DataValue, QueryFilter, QueryResult, StorageStats};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiDataPoint {
    /// Data point unique key
    pub key: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Data value
    pub value: ApiDataValue,
    /// Tags
    pub tags: HashMap<String, String>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum ApiDataValue {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// JSON value
    Json(serde_json::Value),
    /// Binary data (Base64 encoded)
    Binary(String),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiQueryFilter {
    /// Key pattern (supports wildcards)
    pub key_pattern: Option<String>,
    /// Start time
    pub start_time: Option<DateTime<Utc>>,
    /// End time
    pub end_time: Option<DateTime<Utc>>,
    /// Tag filters
    pub tags: Option<HashMap<String, String>>,
    /// Limit number of results
    pub limit: Option<u32>,
    /// Offset
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiQueryResult {
    /// Data points list
    pub data_points: Vec<ApiDataPoint>,
    /// Total count
    pub total_count: Option<u64>,
    /// Has more data
    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    /// Success status
    pub success: bool,
    /// Response message
    pub message: String,
    /// Response data
    pub data: Option<T>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    /// Service status
    pub status: String,
    /// Version info
    pub version: String,
    /// Uptime in seconds
    pub uptime: u64,
    /// Storage backend status
    pub storage_backends: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StorageStatistics {
    /// Backend name
    pub backend_name: String,
    /// Total data points
    pub total_data_points: u64,
    /// Storage size in bytes
    pub storage_size_bytes: u64,
    /// Last write time
    pub last_write_time: Option<DateTime<Utc>>,
    /// Last read time
    pub last_read_time: Option<DateTime<Utc>>,
    /// Connection status
    pub connection_status: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    /// Error code
    pub code: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

// Conversion implementations
impl From<DataPoint> for ApiDataPoint {
    fn from(dp: DataPoint) -> Self {
        Self {
            key: dp.key,
            timestamp: dp.timestamp,
            value: dp.value.into(),
            tags: dp.tags,
            metadata: dp.metadata,
        }
    }
}

impl From<ApiDataPoint> for DataPoint {
    fn from(api_dp: ApiDataPoint) -> Self {
        Self {
            key: api_dp.key,
            timestamp: api_dp.timestamp,
            value: api_dp.value.into(),
            tags: api_dp.tags,
            metadata: api_dp.metadata,
        }
    }
}

impl From<DataValue> for ApiDataValue {
    fn from(dv: DataValue) -> Self {
        match dv {
            DataValue::String(s) => ApiDataValue::String(s),
            DataValue::Integer(i) => ApiDataValue::Integer(i),
            DataValue::Float(f) => ApiDataValue::Float(f),
            DataValue::Boolean(b) => ApiDataValue::Boolean(b),
            DataValue::Json(j) => ApiDataValue::Json(j),
            DataValue::Binary(b) => ApiDataValue::Binary(base64::encode(b)),
        }
    }
}

impl From<ApiDataValue> for DataValue {
    fn from(api_dv: ApiDataValue) -> Self {
        match api_dv {
            ApiDataValue::String(s) => DataValue::String(s),
            ApiDataValue::Integer(i) => DataValue::Integer(i),
            ApiDataValue::Float(f) => DataValue::Float(f),
            ApiDataValue::Boolean(b) => DataValue::Boolean(b),
            ApiDataValue::Json(j) => DataValue::Json(j),
            ApiDataValue::Binary(b) => {
                match base64::decode(&b) {
                    Ok(bytes) => DataValue::Binary(bytes),
                    Err(_) => DataValue::String(b),
                }
            }
        }
    }
}

impl From<QueryFilter> for ApiQueryFilter {
    fn from(qf: QueryFilter) -> Self {
        Self {
            key_pattern: qf.key_pattern,
            start_time: qf.start_time,
            end_time: qf.end_time,
            tags: Some(qf.tags),
            limit: qf.limit,
            offset: qf.offset,
        }
    }
}

impl From<ApiQueryFilter> for QueryFilter {
    fn from(api_qf: ApiQueryFilter) -> Self {
        Self {
            key_pattern: api_qf.key_pattern,
            start_time: api_qf.start_time,
            end_time: api_qf.end_time,
            tags: api_qf.tags.unwrap_or_default(),
            limit: api_qf.limit,
            offset: api_qf.offset,
        }
    }
}

impl From<QueryResult> for ApiQueryResult {
    fn from(qr: QueryResult) -> Self {
        Self {
            data_points: qr.data_points.into_iter().map(|dp| dp.into()).collect(),
            total_count: qr.total_count,
            has_more: qr.has_more,
        }
    }
}

impl From<StorageStats> for StorageStatistics {
    fn from(ss: StorageStats) -> Self {
        Self {
            backend_name: "unknown".to_string(),
            total_data_points: ss.total_data_points,
            storage_size_bytes: ss.storage_size_bytes,
            last_write_time: ss.last_write_time,
            last_read_time: ss.last_read_time,
            connection_status: ss.connection_status,
        }
    }
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            message: String::from("success"),
            data: Some(data),
            timestamp: Utc::now(),
        }
    }

    pub fn success_with_message(data: T, message: String) -> Self {
        Self {
            success: true,
            message,
            data: Some(data),
            timestamp: Utc::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            message,
            data: None,
            timestamp: Utc::now(),
        }
    }
}