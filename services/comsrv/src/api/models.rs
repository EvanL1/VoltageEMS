use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// service status response
#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub name: String,
    pub version: String,
    pub uptime: u64,
    pub start_time: DateTime<Utc>,
    pub channels: u32,
    pub active_channels: u32,
}

/// channel status response
#[derive(Debug, Clone, Serialize)]
pub struct ChannelStatus {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub connected: bool,
    pub last_response_time: f64,
    pub last_error: String,
    pub last_update_time: DateTime<Utc>,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// service health status
#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub uptime: u64,
    pub memory_usage: u64,
    pub cpu_usage: f64,
}

/// channel operation request
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelOperation {
    pub operation: String,  // "start", "stop", "restart"
}

/// point value read response
#[derive(Debug, Clone, Serialize)]
pub struct PointValue {
    pub name: String,
    pub value: serde_json::Value,
    pub quality: bool,
    pub timestamp: DateTime<Utc>,
}

/// point table data response containing all points
#[derive(Debug, Clone, Serialize)]
pub struct PointTableData {
    pub channel_id: String,
    pub points: Vec<PointValue>,
    pub timestamp: DateTime<Utc>,
}

/// point value write request
#[derive(Debug, Clone, Deserialize)]
pub struct WritePointRequest {
    pub value: serde_json::Value,
}

/// error response
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub status: u16,
    pub message: String,
}

/// API response wrapper
#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
} 