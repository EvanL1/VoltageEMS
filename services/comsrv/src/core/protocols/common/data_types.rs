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
        }
    }

    /// Check if this point represents an error
    pub fn is_error(&self) -> bool {
        self.value == "ERROR" || self.description.contains("error") || self.description.contains("Error")
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
        }
    }
}

/// Telemetry type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
            TelemetryType::Telemetry => write!(f, "YC"),
            TelemetryType::Signal => write!(f, "YX"),
            TelemetryType::Control => write!(f, "YK"),
            TelemetryType::Adjustment => write!(f, "YT"),
        }
    }
}

impl From<&str> for TelemetryType {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "YC" | "TELEMETRY" => TelemetryType::Telemetry,
            "YX" | "SIGNAL" | "SIGNALING" => TelemetryType::Signal,
            "YK" | "CONTROL" => TelemetryType::Control,
            "YT" | "ADJUSTMENT" => TelemetryType::Adjustment,
            _ => TelemetryType::Telemetry, // Default
        }
    }
}

/// Polling context to reduce Arc clones
#[derive(Clone)]
pub struct PollingContext {
    pub config: Arc<PollingConfig>,
    pub transport: Arc<dyn crate::core::transport::traits::Transport>,
    pub point_manager: Arc<crate::core::protocols::common::manager::OptimizedPointManager>,
    pub redis_sync: Option<Arc<crate::core::protocols::common::redis::RedisBatchSync>>,
    pub channel_name: Arc<str>,
    pub stats: Arc<tokio::sync::RwLock<PollingStats>>,
}

/// Polling configuration
#[derive(Debug, Clone)]
pub struct PollingConfig {
    /// Polling interval in milliseconds
    pub interval_ms: u64,
    /// Enable batch reading optimization
    pub enable_batch_reading: bool,
    /// Maximum points per batch
    pub max_batch_size: usize,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Enable statistics collection
    pub enable_stats: bool,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            enable_batch_reading: true,
            max_batch_size: 50,
            request_timeout_ms: 3000,
            max_retries: 3,
            enable_stats: true,
        }
    }
}

/// Polling statistics
#[derive(Debug, Clone, Default)]
pub struct PollingStats {
    pub total_polls: u64,
    pub successful_polls: u64,
    pub failed_polls: u64,
    pub total_points_read: u64,
    pub average_poll_time_ms: f64,
    pub last_poll_time: Option<DateTime<Utc>>,
    pub error_count: u64,
    pub last_error: Option<String>,
}

impl PollingStats {
    /// Record a successful poll
    pub fn record_success(&mut self, points_read: usize, duration_ms: f64) {
        self.total_polls += 1;
        self.successful_polls += 1;
        self.total_points_read += points_read as u64;
        self.last_poll_time = Some(Utc::now());
        
        // Update average poll time
        let total_time = self.average_poll_time_ms * (self.total_polls - 1) as f64 + duration_ms;
        self.average_poll_time_ms = total_time / self.total_polls as f64;
    }

    /// Record a failed poll
    pub fn record_failure(&mut self, error: String) {
        self.total_polls += 1;
        self.failed_polls += 1;
        self.error_count += 1;
        self.last_error = Some(error);
        self.last_poll_time = Some(Utc::now());
    }

    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_polls == 0 {
            0.0
        } else {
            (self.successful_polls as f64 / self.total_polls as f64) * 100.0
        }
    }
}