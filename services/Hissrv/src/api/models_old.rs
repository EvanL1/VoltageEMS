use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::storage::{DataPoint, DataValue, QueryFilter, QueryResult, StorageStats};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiDataPoint {
    /// 数据点的唯一键
    pub key: String,
    
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    
    /// 数据值
    pub value: ApiDataValue,
    
    /// 标签信息
    pub tags: HashMap<String, String>,
    
    /// 元数据
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "datapub enum ApiDataValue {
    /// 字符串值
    String(String),
    
    /// 整数值
    Integer(i64),
    
    /// 浮点数值
    Float(f64),
    
    /// 布尔值
    Boolean(bool),
    
    /// JSON值
    Json(serde_json::Value),
    
    /// 二进制数据(Base64编码)
    Binary(String),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiQueryFilter {
    /// 键匹配模式(支持通配符)    pub key_pattern: Option<String>,
    
    /// 开始时间    pub start_time: Option<DateTime<Utc>>,
    
    /// 结束时间    pub end_time: Option<DateTime<Utc>>,
    
    /// 标签过滤    pub tags: Option<HashMap<String, String>>,
    
    /// 限制返回数量    pub limit: Option<u32>,
    
    /// 偏移量    pub offset: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiQueryResult {
    /// 数据点列表    pub data_points: Vec<ApiDataPoint>,
    
    /// 总数量    pub total_count: Option<u64>,
    
    /// 是否还有更多数据    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    /// 是否成功    pub success: bool,
    
    /// 响应消息    pub message: String,
    
    /// 响应数据    pub data: Option<T>,
    
    /// 时间戳    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    /// 服务状态    pub status: String,
    
    /// 版本信息    pub version: String,
    
    /// 运行时间(秒)    pub uptime: u64,
    
    /// 存储后端状态    pub storage_backends: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StorageStatistics {
    /// 后端名称    pub backend_name: String,
    
    /// 总数据点数    pub total_data_points: u64,
    
    /// 存储大小(字节)    pub storage_size_bytes: u64,
    
    /// 最后写入时间    pub last_write_time: Option<DateTime<Utc>>,
    
    /// 最后读取时间    pub last_read_time: Option<DateTime<Utc>>,
    
    /// 连接状态    pub connection_status: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// 错误消息    pub error: String,
    
    /// 错误代码    pub code: String,
    
    /// 时间戳    pub timestamp: DateTime<Utc>,
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
                    Err(_) => DataValue::String(b), // Fallback to string if decode fails
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
            backend_name: "unknown".to_string(), // Will be set by caller
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