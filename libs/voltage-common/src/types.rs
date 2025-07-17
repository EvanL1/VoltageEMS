//! Common types used across VoltageEMS services

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Point ID type used throughout the system
pub type PointId = u32;

/// Device ID type
pub type DeviceId = String;

/// Channel ID type
pub type ChannelId = u16;

/// Service name type
pub type ServiceName = String;

/// Timestamp type
pub type Timestamp = DateTime<Utc>;

/// Common point value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PointValue {
    /// Boolean value (for digital points)
    Bool(bool),
    /// Integer value
    Int(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
    /// Binary data
    Binary(Vec<u8>),
    /// Null/undefined value
    Null,
}

impl PointValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PointValue::Bool(v) => Some(*v),
            PointValue::Int(v) => Some(*v != 0),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            PointValue::Float(v) => Some(*v),
            PointValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            PointValue::Int(v) => Some(*v),
            PointValue::Float(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match self {
            PointValue::String(v) => Some(v.clone()),
            PointValue::Bool(v) => Some(v.to_string()),
            PointValue::Int(v) => Some(v.to_string()),
            PointValue::Float(v) => Some(v.to_string()),
            _ => None,
        }
    }
}

/// Point data with timestamp and quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    /// Point ID
    pub point_id: PointId,
    /// Point value
    pub value: PointValue,
    /// Timestamp when the value was read/generated
    pub timestamp: Timestamp,
    /// Quality flag (optional, defaults to Good if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Data quality indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// Good quality data
    #[default]
    Good,
    /// Uncertain quality (e.g., extrapolated)
    Uncertain,
    /// Bad quality (e.g., sensor failure)
    Bad,
    /// No data available
    NoData,
}

/// Point type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointType {
    /// Telemetry/Measurement (YC)
    Telemetry,
    /// Remote signaling (YX)
    Signal,
    /// Remote control (YK)
    Control,
    /// Remote adjustment (YT)
    Adjustment,
}

/// Service status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    /// Service is starting up
    Starting,
    /// Service is running normally
    Running,
    /// Service is degraded but operational
    Degraded,
    /// Service is stopping
    Stopping,
    /// Service is stopped
    Stopped,
    /// Service has encountered an error
    Error,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Service name
    pub service: ServiceName,
    /// Current status
    pub status: ServiceStatus,
    /// Service version
    pub version: String,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Additional health details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
}

/// Common request/response envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Unique request ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: Timestamp,
    /// Source service
    pub source: ServiceName,
    /// Target service (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ServiceName>,
    /// Payload
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(source: ServiceName, payload: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source,
            target: None,
            payload,
        }
    }

    pub fn with_target(mut self, target: ServiceName) -> Self {
        self.target = Some(target);
        self
    }
}

/// Pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Page number (1-based)
    pub page: u32,
    /// Items per page
    pub per_page: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 100,
        }
    }
}

/// Paginated response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    /// Data items
    pub data: Vec<T>,
    /// Current page
    pub page: u32,
    /// Items per page
    pub per_page: u32,
    /// Total items
    pub total: u64,
    /// Total pages
    pub total_pages: u32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, pagination: Pagination, total: u64) -> Self {
        let total_pages = ((total as f64) / (pagination.per_page as f64)).ceil() as u32;
        Self {
            data,
            page: pagination.page,
            per_page: pagination.per_page,
            total,
            total_pages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_value_conversions() {
        let bool_val = PointValue::Bool(true);
        assert_eq!(bool_val.as_bool(), Some(true));
        assert_eq!(bool_val.as_string(), Some("true".to_string()));

        let int_val = PointValue::Int(42);
        assert_eq!(int_val.as_i64(), Some(42));
        assert_eq!(int_val.as_f64(), Some(42.0));
        assert_eq!(int_val.as_bool(), Some(true));

        let float_val = PointValue::Float(std::f64::consts::PI);
        assert_eq!(float_val.as_f64(), Some(std::f64::consts::PI));
        assert_eq!(float_val.as_i64(), Some(3));
    }

    #[test]
    fn test_envelope() {
        let payload = "test_payload";
        let envelope =
            Envelope::new("service1".to_string(), payload).with_target("service2".to_string());

        assert_eq!(envelope.source, "service1");
        assert_eq!(envelope.target, Some("service2".to_string()));
        assert_eq!(envelope.payload, "test_payload");
    }
}
