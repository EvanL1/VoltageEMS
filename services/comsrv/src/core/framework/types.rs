//! Basic Data Types and Structures
//!
//! This module contains the fundamental data structures used throughout
//! the communication service. Consolidated from combase module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Channel operational status and health information
#[derive(Debug, Clone)]
pub struct ChannelStatus {
    /// Channel identifier
    pub id: String,
    /// Connection status
    pub connected: bool,
    /// Last response time in milliseconds
    pub last_response_time: f64,
    /// Last error message
    pub last_error: String,
    /// Last status update time
    pub last_update_time: DateTime<Utc>,
}

impl ChannelStatus {
    /// Create a new channel status with default values
    pub fn new(channel_id: &str) -> Self {
        Self {
            id: channel_id.to_string(),
            connected: false,
            last_response_time: 0.0,
            last_error: String::new(),
            last_update_time: Utc::now(),
        }
    }

    /// Check if the channel has any error
    pub fn has_error(&self) -> bool {
        !self.last_error.is_empty()
    }

    /// Get error message by reference to avoid cloning
    pub fn error_ref(&self) -> &str {
        &self.last_error
    }

    /// Get channel ID by reference to avoid cloning
    pub fn id_ref(&self) -> &str {
        &self.id
    }

    /// Update connection status
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
        self.last_update_time = Utc::now();
    }

    /// Update error status
    pub fn set_error(&mut self, error: String) {
        self.last_error = error;
        self.last_update_time = Utc::now();
    }

    /// Clear error status
    pub fn clear_error(&mut self) {
        self.last_error.clear();
        self.last_update_time = Utc::now();
    }

    /// Check connection status
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get last response time
    pub fn response_time(&self) -> f64 {
        self.last_response_time
    }

    /// Get last update timestamp
    pub fn last_update(&self) -> DateTime<Utc> {
        self.last_update_time
    }
}

/// Point data structure for telemetry values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    /// Point identifier
    pub id: String,
    /// Point name
    pub name: String,
    /// Value as string (universal representation)
    pub value: String,
    /// Timestamp of the reading
    pub timestamp: DateTime<Utc>,
    /// Unit of measurement
    pub unit: String,
    /// Description or additional information
    pub description: String,
    /// Telemetry type (YC/YX/YT/YK)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_type: Option<TelemetryType>,
    /// Channel ID (for multi-channel systems)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<u16>,
}

impl PointData {
    /// Create new point data
    pub fn new(id: String, name: String, value: String, unit: String) -> Self {
        Self {
            id,
            name,
            value,
            timestamp: Utc::now(),
            unit,
            description: String::new(),
            telemetry_type: None,
            channel_id: None,
        }
    }

    /// Create point data with error value
    pub fn with_error(id: String, name: String, error: String) -> Self {
        Self {
            id,
            name,
            value: "ERROR".to_string(),
            timestamp: Utc::now(),
            unit: String::new(),
            description: error,
            telemetry_type: None,
            channel_id: None,
        }
    }

    /// Check if this point represents an error
    pub fn is_error(&self) -> bool {
        self.value == "ERROR"
            || self.description.contains("error")
            || self.description.contains("Error")
    }
}

/// Optimized polling point structure
#[derive(Debug, Clone)]
pub struct PollingPoint {
    /// Point identifier - kept as Arc for high-frequency sharing
    pub id: Arc<str>,
    /// Point name - kept as Arc for frequent logging
    pub name: Arc<str>,
    /// Register address
    pub address: u32,
    /// Data type representation
    pub data_type: String,
    /// Telemetry type
    pub telemetry_type: TelemetryType,
    /// Scaling factor
    pub scale: f64,
    /// Offset value
    pub offset: f64,
    /// Unit of measurement
    pub unit: String,
    /// Description
    pub description: String,
    /// Access mode
    pub access_mode: String,
    /// Point group - kept as Arc for grouping operations
    pub group: Arc<str>,
    /// Protocol-specific parameters
    pub protocol_params: HashMap<String, serde_json::Value>,
}

impl PollingPoint {
    /// Create a new polling point
    pub fn new(id: String, name: String, address: u32) -> Self {
        Self {
            id: Arc::from(id),
            name: Arc::from(name),
            address,
            data_type: "float".to_string(),
            telemetry_type: TelemetryType::Telemetry,
            scale: 1.0,
            offset: 0.0,
            unit: String::new(),
            description: String::new(),
            access_mode: "r".to_string(),
            group: Arc::from("default"),
            protocol_params: HashMap::new(),
        }
    }

    /// Convert to PointData
    pub fn to_point_data(&self, value: String) -> PointData {
        PointData {
            id: self.id.to_string(),
            name: self.name.to_string(),
            value,
            timestamp: Utc::now(),
            unit: self.unit.clone(),
            description: self.description.clone(),
            telemetry_type: Some(self.telemetry_type),
            channel_id: None,
        }
    }
}

/// Telemetry type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TelemetryType {
    /// 遥测 - Analog measurements
    Telemetry,
    /// 遥信 - Digital signals
    Signal,
    /// 遥控 - Control commands
    Control,
    /// 遥调 - Analog adjustments
    Adjustment,
}

impl std::fmt::Display for TelemetryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TelemetryType::Telemetry => write!(f, "Measurement"),
            TelemetryType::Signal => write!(f, "Signal"),
            TelemetryType::Control => write!(f, "Control"),
            TelemetryType::Adjustment => write!(f, "Adjustment"),
        }
    }
}

impl From<&str> for TelemetryType {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "YC" | "TELEMETRY" | "MEASUREMENT" => TelemetryType::Telemetry,
            "YX" | "SIGNAL" | "SIGNALING" => TelemetryType::Signal,
            "YK" | "CONTROL" => TelemetryType::Control,
            "YT" | "ADJUSTMENT" => TelemetryType::Adjustment,
            _ => TelemetryType::Telemetry, // Default
        }
    }
}

impl TelemetryType {
    /// Get Chinese name
    pub fn chinese_name(&self) -> &'static str {
        match self {
            TelemetryType::Telemetry => "遥测",
            TelemetryType::Signal => "遥信",
            TelemetryType::Control => "遥控",
            TelemetryType::Adjustment => "遥调",
        }
    }

    /// Get English name
    pub fn english_name(&self) -> &'static str {
        match self {
            TelemetryType::Telemetry => "Measurement",
            TelemetryType::Signal => "Signaling",
            TelemetryType::Control => "Control",
            TelemetryType::Adjustment => "Regulation",
        }
    }

    /// Check if this type is analog
    pub fn is_analog(&self) -> bool {
        matches!(self, TelemetryType::Telemetry | TelemetryType::Adjustment)
    }

    /// Check if this type is digital
    pub fn is_digital(&self) -> bool {
        matches!(self, TelemetryType::Signal | TelemetryType::Control)
    }

    /// Check if this type is readable
    pub fn is_readable(&self) -> bool {
        matches!(self, TelemetryType::Telemetry | TelemetryType::Signal)
    }

    /// Check if this type is writable
    pub fn is_writable(&self) -> bool {
        matches!(self, TelemetryType::Control | TelemetryType::Adjustment)
    }
}

// Note: Polling-related structures have been removed from the common layer.
// Each protocol should implement its own data collection mechanism:
// - Modbus/IEC60870: Polling-based with protocol-specific intervals
// - CAN: Event-driven with message filtering
// - GPIO: Interrupt-driven with state change detection

/// Connection state enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Channel is disconnected.
    Disconnected,
    /// Channel is attempting to establish a connection.
    Connecting,
    /// Channel is connected and operational.
    Connected,
}

/// Point value type for unified handling
#[derive(Debug, Clone)]
pub enum PointValueType {
    /// Analog measurements (遥测/遥调)
    Analog(f64),
    /// Digital status (遥信/遥控)
    Digital(bool),
}

/// Remote operation type definition (simplified)
#[derive(Debug, Clone)]
pub enum RemoteOperationType {
    /// Digital control (遥控)
    Control { value: bool },
    /// Analog regulation (遥调)
    Regulation { value: f64 },
}

/// Remote operation request
#[derive(Debug, Clone)]
pub struct RemoteOperationRequest {
    /// Operation ID
    pub operation_id: String,
    /// Point name
    pub point_name: String,
    /// Operation type
    pub operation_type: RemoteOperationType,
}

/// Remote operation response
#[derive(Debug, Clone)]
pub struct RemoteOperationResponse {
    /// Operation ID
    pub operation_id: String,
    /// Success status
    pub success: bool,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Response timestamp
    pub timestamp: DateTime<Utc>,
}
