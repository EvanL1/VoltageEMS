//! # Communication Base Module
//!
//! This module provides the foundational traits and types for implementing
//! communication protocols in the Voltage EMS Communication Service. It defines
//! the common interface that all protocol implementations must satisfy.
//!
//! ## Overview
//!
//! The Communication Service supports multiple industrial communication protocols
//! through a unified interface. This module defines:
//!
//! - **ComBase Trait**: The primary interface all protocols must implement
//! - **ChannelStatus**: Status reporting and health monitoring
//! - **PointData**: Real-time data point representation
//! - **ComBaseImpl**: Reference implementation with common functionality
//!
//! ## Key Components
//!
//! ### ComBase Trait
//!
//! The `ComBase` trait provides a standardized interface for:
//! - Protocol lifecycle management (start/stop)
//! - Status monitoring and error reporting
//! - Real-time data collection
//! - Configuration parameter access
//!
//! ### Channel Status Monitoring
//!
//! The status system provides:
//! - Connection state tracking
//! - Performance metrics (response times)
//! - Error condition reporting
//! - Timestamped status updates
//!
//! ## Usage Example
//!
//! ```rust
//! use comsrv::core::protocols::common::combase::{ComBase, ChannelStatus, PointData};
//! use comsrv::utils::Result;
//!
//! // Example usage of the communication base interface
//! async fn example_usage(mut service: Box<dyn ComBase>) -> Result<()> {
//!     // Start the communication service
//!     service.start().await?;
//!     
//!     // Check operational status
//!     let status = service.status().await;
//!     println!("Service {}: connected={}", service.name(), status.connected);
//!     
//!     // Collect data points
//!     let points = service.get_all_points().await;
//!     for point in points {
//!         println!("Point {}: {}", point.id, point.value);
//!     }
//!     
//!     // Graceful shutdown
//!     service.stop().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Protocol Implementation Guide
//!
//! To implement a new communication protocol:
//!
//! 1. **Create a Protocol Struct**: Define your protocol's specific configuration and state
//! 2. **Implement ComBase**: Provide implementations for all required methods
//! 3. **Handle Lifecycle**: Properly manage connection setup and teardown
//! 4. **Report Status**: Keep channel status updated with current information
//! 5. **Error Handling**: Use the unified error system for consistent reporting
//!
//! ## Example Implementation Structure
//!
//! ```rust
//! use comsrv::core::protocols::common::combase::ComBaseImpl;
//! use comsrv::core::config::ChannelConfig;
//!
//! // Example: Creating a communication service base
//! fn create_service_base(name: &str, protocol: &str, config: ChannelConfig) -> ComBaseImpl {
//!     ComBaseImpl::new(name, protocol, config)
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;

use tracing::{
    debug, debug as log_debug, error, error as log_error, info, info as log_info, trace, warn,
    warn as log_warn,
};

use crate::core::config::ChannelConfig;
use crate::utils::error::{ComSrvError, Result};

/// Channel operational status and health information
///
/// Provides comprehensive status information for a communication channel,
/// including connection state, performance metrics, and error conditions.
/// This structure is used for monitoring and diagnostics of communication channels.
///
/// # Fields
///
/// * `id` - Unique identifier for the channel
/// * `connected` - Whether the channel is currently connected
/// * `last_response_time` - Most recent response time in milliseconds
/// * `last_error` - Description of the most recent error (empty if no error)
/// * `last_update_time` - Timestamp of the last status update
///
/// # Examples
///
/// ```
/// use comsrv::core::protocols::common::combase::ChannelStatus;
///
/// let status = ChannelStatus::new("modbus_001");
/// assert!(!status.connected);
/// assert!(!status.has_error());
/// assert!(status.last_error.is_empty());
/// ```
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
    ///
    /// Initializes a new channel status with disconnected state, zero response time,
    /// no error message, and current timestamp.
    ///
    /// # Arguments
    ///
    /// * `channel_id` - Unique identifier for the channel
    ///
    /// # Returns
    ///
    /// New `ChannelStatus` instance with default values
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ChannelStatus;
    ///
    /// let status = ChannelStatus::new("modbus_001");
    /// assert_eq!(status.id, "modbus_001");
    /// assert!(!status.connected);
    /// assert_eq!(status.last_response_time, 0.0);
    /// assert!(status.last_error.is_empty());
    /// ```
    pub fn new(channel_id: &str) -> Self {
        Self {
            id: channel_id.to_string(),
            connected: false,
            last_response_time: 0.0,
            last_error: String::new(),
            last_update_time: Utc::now(),
        }
    }

    /// Check if the channel has an error condition
    ///
    /// Determines whether the channel currently has an error by checking
    /// if the error message is non-empty.
    ///
    /// # Returns
    ///
    /// `true` if there is an active error condition, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use comsrv::core::protocols::common::combase::ChannelStatus;
    ///
    /// let mut status = ChannelStatus::new("test_channel");
    /// assert!(!status.has_error());
    ///
    /// // Simulate an error condition
    /// status.last_error = "Connection failed".to_string();
    /// assert!(status.has_error());
    /// ```
    pub fn has_error(&self) -> bool {
        !self.last_error.is_empty()
    }
}

/// Real-time data point structure
///
/// Represents a single data point from a communication channel with
/// associated metadata and timestamps. This structure is used for
/// real-time data collection and monitoring.
///
/// # Fields
///
/// * `id` - Unique identifier for the data point
/// * `name` - Human-readable name for the data point
/// * `value` - Current value as a string
/// * `timestamp` - Time when the data was collected
/// * `unit` - Engineering unit for the value
/// * `description` - Detailed description of the data point
///
/// # Examples
///
/// ```
/// use comsrv::core::protocols::common::combase::PointData;
/// use chrono::Utc;
///
/// let point = PointData {
///     id: "voltage_1".to_string(),
///     name: "Main Bus Voltage".to_string(),
///     value: "230.5".to_string(),
///     timestamp: Utc::now(),
///     unit: "V".to_string(),
///     description: "Primary electrical bus voltage measurement".to_string(),
/// };
///
/// println!("Point {}: {} {}", point.name, point.value, point.unit);
/// ```
#[derive(Debug, Clone)]
pub struct PointData {
    /// Point ID
    pub id: String,
    /// Point name
    pub name: String,
    /// Point value as string
    pub value: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Engineering unit
    pub unit: String,
    /// Point description
    pub description: String,
}

/// Universal Polling Configuration
///
/// This configuration is protocol-agnostic and can be used by any communication protocol
/// that requires periodic data collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingConfig {
    /// Enable or disable polling for this channel
    pub enabled: bool,
    /// Polling interval in milliseconds
    pub interval_ms: u64,
    /// Maximum number of points to read per polling cycle
    pub max_points_per_cycle: u32,
    /// Timeout for each polling operation
    pub timeout_ms: u64,
    /// Number of retry attempts on failure
    pub max_retries: u32,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Enable batch reading optimization (protocol-specific)
    pub enable_batch_reading: bool,
    /// Minimum delay between individual point reads in milliseconds
    pub point_read_delay_ms: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000,
            max_points_per_cycle: 1000,
            timeout_ms: 5000,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_batch_reading: true,
            point_read_delay_ms: 10,
        }
    }
}

/// Universal Polling Statistics
///
/// These statistics are collected for any protocol that uses the polling system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingStats {
    /// Total number of polling cycles executed
    pub total_cycles: u64,
    /// Number of successful polling cycles
    pub successful_cycles: u64,
    /// Number of failed polling cycles
    pub failed_cycles: u64,
    /// Total data points read successfully
    pub total_points_read: u64,
    /// Total data points that failed to read
    pub total_points_failed: u64,
    /// Average polling cycle time in milliseconds
    pub avg_cycle_time_ms: f64,
    /// Current polling rate (cycles per second)
    pub current_polling_rate: f64,
    /// Last successful polling timestamp
    pub last_successful_polling: Option<DateTime<Utc>>,
    /// Last polling error message
    pub last_polling_error: Option<String>,

}

/// 四遥数据类型分类枚举
/// Four-telemetry data type classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TelemetryType {
    /// 遥测 - Analog measurements (temperature, pressure, flow, etc.)
    Telemetry,
    /// 遥信 - Digital status signals (switch status, alarm status, etc.)
    Signaling,
    /// 遥控 - Digital control commands (start/stop, on/off, etc.)
    Control,
    /// 遥调 - Analog regulation commands (setpoint adjustment, etc.)
    Setpoint,
}

impl TelemetryType {
    /// Get the Chinese name of the telemetry type
    pub fn chinese_name(&self) -> &'static str {
        match self {
            TelemetryType::Telemetry => "遥测",
            TelemetryType::Signaling => "遥信",
            TelemetryType::Control => "遥控",
            TelemetryType::Setpoint => "遥调",
        }
    }

    /// Get the English name of the telemetry type
    pub fn english_name(&self) -> &'static str {
        match self {
            TelemetryType::Telemetry => "Remote Measurement",
            TelemetryType::Signaling => "Remote Signaling",
            TelemetryType::Control => "Remote Control",
            TelemetryType::Setpoint => "Remote Regulation",
        }
    }

    /// Check if this telemetry type is readable (measurement/signaling)
    pub fn is_readable(&self) -> bool {
        matches!(self, TelemetryType::Telemetry | TelemetryType::Signaling)
    }

    /// Check if this telemetry type is writable (control/regulation)
    pub fn is_writable(&self) -> bool {
        matches!(self, TelemetryType::Control | TelemetryType::Setpoint)
    }

    /// Check if this telemetry type handles analog values
    pub fn is_analog(&self) -> bool {
        matches!(self, TelemetryType::Telemetry | TelemetryType::Setpoint)
    }

    /// Check if this telemetry type handles digital values
    pub fn is_digital(&self) -> bool {
        matches!(self, TelemetryType::Signaling | TelemetryType::Control)
    }
}

/// 遥测点数据结构 - Remote Measurement Point (Processed by Universal Layer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementPoint {
    /// Current analog value (in engineering units)
    pub value: f64,
    /// Engineering unit (℃, bar, m³/h, etc.)
    pub unit: String,
    /// Measurement timestamp
    pub timestamp: DateTime<Utc>,
}

/// 遥信点数据结构 - Remote Signaling Point (Processed by Universal Layer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalingPoint {
    /// Current digital status
    pub status: bool,
    /// Status description text
    pub status_text: String,
    /// Status change timestamp
    pub timestamp: DateTime<Utc>,
}

/// 遥控点数据结构 - Remote Control Point (Processed by Universal Layer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPoint {
    /// Current control state
    pub current_state: bool,
    /// Command description text
    pub command_text: String,
    /// Control execution status
    pub execution_status: ExecutionStatus,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// 遥调点数据结构 - Remote Regulation Point (Processed by Universal Layer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulationPoint {
    /// Current setpoint value (in engineering units)
    pub current_value: f64,
    /// Engineering unit
    pub unit: String,
    /// Whether the value is within regulation range
    pub in_range: bool,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// 执行状态 - Execution Status (Simplified)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    /// 待机 - Standby
    Standby,
    /// 执行中 - Executing
    Executing,
    /// 成功 - Completed
    Completed,
    /// 失败 - Failed
    Failed(String),
}

/// 控制执行状态 - Control Execution Status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ControlExecutionStatus {
    /// 待机 - Standby
    Standby,
    /// 执行中 - Executing
    Executing,
    /// 成功 - Success
    Success,
    /// 失败 - Failed
    Failed(String),
    /// 超时 - Timeout
    Timeout,
}

/// 调节执行状态 - Regulation Execution Status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegulationExecutionStatus {
    /// 待机 - Standby
    Standby,
    /// 调节中 - Regulating
    Regulating,
    /// 成功 - Success
    Success,
    /// 失败 - Failed
    Failed(String),
    /// 超时 - Timeout
    Timeout,
    /// 超出范围 - Out of Range
    OutOfRange,
}

/// Point value type enumeration for four-telemetry operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PointValueType {
    /// Analog measurements (遥测/遥调)
    Analog(f64),
    /// Digital status (遥信/遥控)
    Digital(bool),
    /// Extended measurement with metadata
    Measurement(MeasurementPoint),
    /// Extended signaling with state descriptions
    Signaling(SignalingPoint),
    /// Extended control with execution status
    Control(ControlPoint),
    /// Extended regulation with range validation
    Regulation(RegulationPoint),
}

impl PointValueType {
    /// Get the telemetry type classification
    pub fn telemetry_type(&self) -> TelemetryType {
        match self {
            PointValueType::Analog(_) | PointValueType::Measurement(_) => {
                TelemetryType::Telemetry
            }
            PointValueType::Digital(_) | PointValueType::Signaling(_) => TelemetryType::Signaling,
            PointValueType::Control(_) => TelemetryType::Control,
            PointValueType::Regulation(_) => TelemetryType::Setpoint,
        }
    }

    /// Get the raw value as f64 (for analog types)
    pub fn as_analog(&self) -> Option<f64> {
        match self {
            PointValueType::Analog(v) => Some(*v),
            PointValueType::Measurement(m) => Some(m.value),
            PointValueType::Regulation(r) => Some(r.current_value),
            _ => None,
        }
    }

    /// Get the raw value as bool (for digital types)
    pub fn as_digital(&self) -> Option<bool> {
        match self {
            PointValueType::Digital(v) => Some(*v),
            PointValueType::Signaling(s) => Some(s.status),
            PointValueType::Control(c) => Some(c.current_state),
            _ => None,
        }
    }
}

/// Point operation type for remote control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteOperationType {
    /// Digital control (遥控)
    Control { value: bool },
    /// Analog regulation (遥调)
    Regulation { value: f64 },
    /// Extended control with validation
    ExtendedControl {
        target_state: bool,
        operator: String,
        description: Option<String>,
        confirmation_required: bool,
    },
    /// Extended regulation with range checking
    ExtendedRegulation {
        target_value: f64,
        operator: String,
        description: Option<String>,
        min_value: Option<f64>,
        max_value: Option<f64>,
        step_size: Option<f64>,
    },
}

impl RemoteOperationType {
    /// Get the telemetry type for this operation
    pub fn telemetry_type(&self) -> TelemetryType {
        match self {
            RemoteOperationType::Control { .. } | RemoteOperationType::ExtendedControl { .. } => {
                TelemetryType::Control
            }
            RemoteOperationType::Regulation { .. }
            | RemoteOperationType::ExtendedRegulation { .. } => TelemetryType::Setpoint,
        }
    }

    /// Validate the operation parameters
    pub fn validate(&self) -> Result<()> {
        match self {
            RemoteOperationType::ExtendedRegulation {
                target_value,
                min_value,
                max_value,
                ..
            } => {
                if let (Some(min), Some(max)) = (min_value, max_value) {
                    if *target_value < *min || *target_value > *max {
                        return Err(ComSrvError::ConfigError(format!(
                            "Target value {} is out of range [{}, {}]",
                            target_value, min, max
                        )));
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Command execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteOperationRequest {
    /// Operation ID
    pub operation_id: String,
    /// Point name
    pub point_name: String,
    /// Operation type
    pub operation_type: RemoteOperationType,
    /// Operator information
    pub operator: Option<String>,
    /// Operation description
    pub description: Option<String>,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
}

/// Command execution response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteOperationResponse {
    /// Operation ID (corresponds to request ID)
    pub operation_id: String,
    /// Execution success
    pub success: bool,
    /// Error message (if any)
    pub error_message: Option<String>,
    /// Actual value after execution
    pub actual_value: Option<PointValueType>,
    /// Execution completion timestamp
    pub execution_time: DateTime<Utc>,
}

/// Define the standard four-telemetry interface for SCADA systems
///
/// This trait provides a unified interface for the four fundamental telemetry operations
/// in industrial control systems: measurement (遥测), signaling (遥信), control (遥控), and regulation (遥调).
#[async_trait]
pub trait FourTelemetryOperations: Send + Sync {
    /// Remote Measurement (遥测) - Read analog measurement values from remote devices
    ///
    /// Reads analog measurement values such as temperature, pressure, flow rate, etc.
    /// from remote devices and returns them with timestamps.
    ///
    /// # Arguments
    /// * `point_names` - List of measurement point names to read
    ///
    /// # Returns
    /// * `Ok(Vec<(String, PointValueType)>)` - Successfully read values with point names
    /// * `Err(ComSrvError)` - Read operation failed
    ///
    /// # Examples
    /// ```rust
    /// let measurements = device.remote_measurement(&["temperature_01", "pressure_02"]).await?;
    /// for (name, value) in measurements {
    ///     if let Some(analog_val) = value.as_analog() {
    ///         println!("Point {}: {}", name, analog_val);
    ///     }
    /// }
    /// ```
    async fn remote_measurement(
        &self,
        point_names: &[String],
    ) -> Result<Vec<(String, PointValueType)>>;

    /// Remote Signaling (遥信) - Read digital status values from remote devices
    ///
    /// Reads digital status values such as switch positions, alarm states, equipment status, etc.
    /// from remote devices and returns them with timestamps.
    ///
    /// # Arguments
    /// * `point_names` - List of signaling point names to read
    ///
    /// # Returns
    /// * `Ok(Vec<(String, PointValueType)>)` - Successfully read values with point names
    /// * `Err(ComSrvError)` - Read operation failed
    ///
    /// # Examples
    /// ```rust
    /// let signals = device.remote_signaling(&["pump_status", "alarm_high_temp"]).await?;
    /// for (name, value) in signals {
    ///     if let Some(digital_val) = value.as_digital() {
    ///         println!("Point {}: {}", name, if digital_val { "ON" } else { "OFF" });
    ///     }
    /// }
    /// ```
    async fn remote_signaling(
        &self,
        point_names: &[String],
    ) -> Result<Vec<(String, PointValueType)>>;

    /// Remote Control (遥控) - Execute digital control operations on remote devices
    ///
    /// Executes digital control commands such as start/stop, on/off, open/close operations
    /// on remote devices and returns the execution result with status information.
    ///
    /// # Arguments
    /// * `request` - Remote control operation request containing target point and command
    ///
    /// # Returns
    /// * `Ok(RemoteOperationResponse)` - Control operation result with execution status
    /// * `Err(ComSrvError)` - Control operation failed
    ///
    /// # Examples
    /// ```rust
    /// let request = RemoteOperationRequest {
    ///     operation_id: "ctrl_001".to_string(),
    ///     point_name: "pump_01_start".to_string(),
    ///     operation_type: RemoteOperationType::Control { value: true },
    ///     operator: Some("operator_01".to_string()),
    ///     description: Some("Start main pump".to_string()),
    ///     timestamp: Utc::now(),
    /// };
    /// let response = device.remote_control(request).await?;
    /// ```
    async fn remote_control(
        &self,
        request: RemoteOperationRequest,
    ) -> Result<RemoteOperationResponse>;

    /// Remote Regulation (遥调) - Execute analog regulation operations on remote devices
    ///
    /// Executes analog regulation commands such as setpoint adjustments, flow rate control,
    /// temperature control, etc. on remote devices and returns the execution result.
    ///
    /// # Arguments
    /// * `request` - Remote regulation operation request containing target point and setpoint
    ///
    /// # Returns
    /// * `Ok(RemoteOperationResponse)` - Regulation operation result with execution status
    /// * `Err(ComSrvError)` - Regulation operation failed
    ///
    /// # Examples
    /// ```rust
    /// let request = RemoteOperationRequest {
    ///     operation_id: "reg_001".to_string(),
    ///     point_name: "temp_setpoint_01".to_string(),
    ///     operation_type: RemoteOperationType::Regulation { value: 75.5 },
    ///     operator: Some("operator_01".to_string()),
    ///     description: Some("Adjust temperature setpoint".to_string()),
    ///     timestamp: Utc::now(),
    /// };
    /// let response = device.remote_regulation(request).await?;
    /// ```
    async fn remote_regulation(
        &self,
        request: RemoteOperationRequest,
    ) -> Result<RemoteOperationResponse>;

    /// Get all available remote control points (遥控点)
    ///
    /// Returns a list of all control points that can be operated through remote_control().
    /// These are typically digital control points like pump start/stop, valve open/close, etc.
    ///
    /// # Returns
    /// * `Vec<String>` - List of control point names
    async fn get_control_points(&self) -> Vec<String>;

    /// Get all available remote regulation points (遥调点)
    ///
    /// Returns a list of all regulation points that can be operated through remote_regulation().
    /// These are typically analog setpoints like temperature control, flow rate control, etc.
    ///
    /// # Returns
    /// * `Vec<String>` - List of regulation point names
    async fn get_regulation_points(&self) -> Vec<String>;

    /// Get all available measurement points (遥测点)
    ///
    /// Returns a list of all measurement points that can be read through remote_measurement().
    /// These are typically analog sensors like temperature, pressure, flow sensors, etc.
    ///
    /// # Returns
    /// * `Vec<String>` - List of measurement point names
    async fn get_measurement_points(&self) -> Vec<String>;

    /// Get all available signaling points (遥信点)
    ///
    /// Returns a list of all signaling points that can be read through remote_signaling().
    /// These are typically digital status points like switch positions, alarm states, etc.
    ///
    /// # Returns
    /// * `Vec<String>` - List of signaling point names
    async fn get_signaling_points(&self) -> Vec<String>;

    /// Get points by telemetry type
    ///
    /// Returns a list of points filtered by their telemetry type classification.
    ///
    /// # Arguments
    /// * `telemetry_type` - The type of telemetry points to retrieve
    ///
    /// # Returns
    /// * `Vec<String>` - List of point names matching the specified type
    async fn get_points_by_type(&self, telemetry_type: TelemetryType) -> Vec<String> {
        match telemetry_type {
            TelemetryType::Telemetry => self.get_measurement_points().await,
            TelemetryType::Signaling => self.get_signaling_points().await,
            TelemetryType::Control => self.get_control_points().await,
            TelemetryType::Setpoint => self.get_regulation_points().await,
        }
    }

    /// Batch read points by telemetry type
    ///
    /// Efficiently reads multiple points of the same telemetry type in a single operation.
    ///
    /// # Arguments
    /// * `telemetry_type` - The type of telemetry points to read
    /// * `point_names` - Optional list of specific point names (if None, reads all points of this type)
    ///
    /// # Returns
    /// * `Ok(Vec<(String, PointValueType)>)` - Successfully read values with point names
    /// * `Err(ComSrvError)` - Read operation failed
    async fn batch_read_by_type(
        &self,
        telemetry_type: TelemetryType,
        point_names: Option<&[String]>,
    ) -> Result<Vec<(String, PointValueType)>> {
        let points_to_read = if let Some(names) = point_names {
            names.to_vec()
        } else {
            self.get_points_by_type(telemetry_type).await
        };

        match telemetry_type {
            TelemetryType::Telemetry => self.remote_measurement(&points_to_read).await,
            TelemetryType::Signaling => self.remote_signaling(&points_to_read).await,
            _ => Err(ComSrvError::InvalidOperation(
                "Batch read not supported for control/regulation points".to_string(),
            )),
        }
    }
}

/// Universal Redis Command Manager
/// Universal Redis command manager for handling four-telemetry commands across all protocols
#[derive(Clone)]
pub struct UniversalCommandManager {
    /// Redis store for command handling
    redis_store: Option<crate::core::storage::redis_storage::RedisStore>,
    /// Channel ID for this communication instance
    channel_id: String,
    /// Command listener task handle
    command_listener_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Running state
    is_running: Arc<RwLock<bool>>,
}

impl UniversalCommandManager {
    /// Create a new command manager
    pub fn new(channel_id: String) -> Self {
        Self {
            redis_store: None,
            channel_id,
            command_listener_handle: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Initialize with Redis store
    pub fn with_redis_store(
        mut self,
        redis_store: crate::core::storage::redis_storage::RedisStore,
    ) -> Self {
        self.redis_store = Some(redis_store);
        self
    }

    /// Start command listener
    pub async fn start<T>(&self, four_telemetry_impl: Arc<T>) -> Result<()>
    where
        T: FourTelemetryOperations + 'static,
    {
        if self.redis_store.is_none() {
            // No Redis integration, skip command listener
            return Ok(());
        }

        *self.is_running.write().await = true;

        let redis_store = self.redis_store.as_ref().unwrap().clone();
        let channel_id = self.channel_id.clone();
        let is_running = Arc::clone(&self.is_running);

        let handle = tokio::spawn(async move {
            Self::command_listener_loop(redis_store, four_telemetry_impl, channel_id, is_running)
                .await;
        });

        *self.command_listener_handle.write().await = Some(handle);
        info!(
            "Universal command manager started for channel: {}",
            self.channel_id
        );
        Ok(())
    }

    /// Stop command listener
    pub async fn stop(&self) -> Result<()> {
        *self.is_running.write().await = false;

        if let Some(handle) = self.command_listener_handle.write().await.take() {
            handle.abort();
        }

        info!(
            "Universal command manager stopped for channel: {}",
            self.channel_id
        );
        Ok(())
    }

    /// Redis command listener loop
    async fn command_listener_loop<T>(
        redis_store: crate::core::storage::redis_storage::RedisStore,
        four_telemetry_impl: Arc<T>,
        channel_id: String,
        is_running: Arc<RwLock<bool>>,
    ) where
        T: FourTelemetryOperations + 'static,
    {
        use futures::StreamExt;

        info!(
            "Starting Redis command listener for channel: {}",
            channel_id
        );

        // Create PubSub connection
        let mut pubsub = match redis_store.create_pubsub().await {
            Ok(pubsub) => pubsub,
            Err(e) => {
                error!("Failed to create Redis PubSub connection: {}", e);
                return;
            }
        };

        // Subscribe to command channel
        let command_channel = format!("commands:{}", channel_id);
        if let Err(e) = pubsub.subscribe(&command_channel).await {
            error!(
                "Failed to subscribe to command channel {}: {}",
                command_channel, e
            );
            return;
        }

        info!("Subscribed to Redis command channel: {}", command_channel);

        // Listen for commands
        while *is_running.read().await {
            match pubsub.on_message().next().await {
                Some(msg) => {
                    let command_id: String = match msg.get_payload() {
                        Ok(payload) => payload,
                        Err(e) => {
                            warn!("Failed to parse command notification payload: {}", e);
                            continue;
                        }
                    };

                    debug!("Received command notification: {}", command_id);

                    // Process command
                    if let Err(e) = Self::process_redis_command(
                        &redis_store,
                        &four_telemetry_impl,
                        &channel_id,
                        &command_id,
                    )
                    .await
                    {
                        error!("Failed to process command {}: {}", command_id, e);
                    }
                }
                None => {
                    trace!("No message received from Redis PubSub");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        debug!("Redis command listener loop stopped");
    }

    /// Process a Redis command using four-telemetry operations
    async fn process_redis_command<T>(
        redis_store: &crate::core::storage::redis_storage::RedisStore,
        four_telemetry_impl: &Arc<T>,
        channel_id: &str,
        command_id: &str,
    ) -> Result<()>
    where
        T: FourTelemetryOperations + 'static,
    {
        use crate::core::storage::redis_storage::{CommandResult, CommandType};

        // Get command from Redis
        let command = match redis_store.get_command(channel_id, command_id).await? {
            Some(cmd) => cmd,
            None => {
                warn!("Command {} not found in Redis", command_id);
                return Ok(());
            }
        };

        info!(
            "Processing command: {} for point: {} with value: {}",
            command_id, command.point_name, command.value
        );

        // Convert Redis command to four-telemetry request
        let request = RemoteOperationRequest {
            operation_id: command.command_id.clone(),
            point_name: command.point_name.clone(),
            operation_type: match command.command_type {
                CommandType::RemoteControl => RemoteOperationType::Control {
                    value: command.value != 0.0,
                },
                CommandType::RemoteRegulation => RemoteOperationType::Regulation {
                    value: command.value,
                },
            },
            operator: None,
            description: None,
            timestamp: Utc::now(),
        };

        // Execute command using four-telemetry interface
        let response = match command.command_type {
            CommandType::RemoteControl => four_telemetry_impl.remote_control(request).await,
            CommandType::RemoteRegulation => four_telemetry_impl.remote_regulation(request).await,
        };

        // Convert four-telemetry response to Redis result
        let result = match response {
            Ok(resp) => {
                info!("Command {} executed successfully", command_id);

                CommandResult {
                    command_id: resp.operation_id,
                    success: resp.success,
                    error_message: resp.error_message,
                    execution_time: resp
                        .execution_time
                        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                        .to_string(),
                    actual_value: resp.actual_value.map(|v| match v {
                        PointValueType::Analog(val) => val,
                        PointValueType::Digital(val) => {
                            if val {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        PointValueType::Measurement(m) => m.value,
                        PointValueType::Signaling(s) => {
                            if s.status {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        PointValueType::Control(c) => {
                            if c.current_state {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        PointValueType::Regulation(r) => r.current_value,
                    }),
                }
            }
            Err(e) => {
                error!("Command {} execution failed: {}", command_id, e);

                CommandResult {
                    command_id: command.command_id.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                    execution_time: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    actual_value: None,
                }
            }
        };

        // Save result to Redis
        if let Err(e) = redis_store.set_command_result(channel_id, &result).await {
            warn!("Failed to save command result: {}", e);
        }

        // Delete processed command
        if let Err(e) = redis_store.delete_command(channel_id, command_id).await {
            warn!("Failed to delete processed command: {}", e);
        }

        Ok(())
    }

    /// Sync real-time data to Redis
    pub async fn sync_data_to_redis(&self, data_points: &[PointData]) -> Result<()> {
        if let Some(ref redis_store) = self.redis_store {
            for point in data_points {
                let realtime_value = crate::core::storage::redis_storage::RealtimeValue {
                    raw: point.value.parse::<f64>().unwrap_or(0.0),
                    processed: point.value.parse::<f64>().unwrap_or(0.0),
                    timestamp: point.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                };

                let redis_key = format!("realtime:{}:{}", self.channel_id, point.id);

                if let Err(e) = redis_store
                    .set_realtime_value_with_expire(&redis_key, &realtime_value, 3600)
                    .await
                {
                    warn!("Failed to sync point {} to Redis: {}", point.id, e);
                } else {
                    trace!("Successfully synced point {} to Redis", point.id);
                }
            }

            debug!(
                "Synced {} points to Redis for channel {}",
                data_points.len(),
                self.channel_id
            );
        }
        Ok(())
    }
}

impl Default for PollingStats {
    fn default() -> Self {
        Self {
            total_cycles: 0,
            successful_cycles: 0,
            failed_cycles: 0,
            total_points_read: 0,
            total_points_failed: 0,
            avg_cycle_time_ms: 0.0,
            current_polling_rate: 0.0,
            last_successful_polling: None,
            last_polling_error: None,

        }
    }
}

/// Point Definition for Polling
///
/// Defines a data point that should be read during polling cycles.
/// This is protocol-agnostic and can represent any type of data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingPoint {
    /// Unique point identifier
    pub id: String,
    /// Human-readable point name
    pub name: String,
    /// Protocol-specific address (e.g., Modbus register address, IEC60870 IOA)
    pub address: u32,
    /// Data type for value interpretation
    pub data_type: String,
    /// Four-telemetry type classification
    pub telemetry_type: TelemetryType,
    /// Scaling factor applied to raw values
    pub scale: f64,
    /// Offset applied after scaling
    pub offset: f64,
    /// Engineering unit
    pub unit: String,
    /// Point description
    pub description: String,
    /// Access mode (read, write, read-write)
    pub access_mode: String,
    /// Point group for batch operations
    pub group: String,
    /// Protocol-specific parameters
    pub protocol_params: HashMap<String, serde_json::Value>,
    /// Extended telemetry metadata (optional)
    pub telemetry_metadata: Option<TelemetryMetadata>,
}

/// Extended telemetry metadata for specialized point types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryMetadata {
    /// For signaling points: state descriptions and invert option
    pub true_text: Option<String>,
    pub false_text: Option<String>,
    pub invert_signal: Option<bool>,

    /// For control points: command descriptions and invert option
    pub true_command: Option<String>,
    pub false_command: Option<String>,
    pub invert_control: Option<bool>,
}

/// Protocol response parsing trait for data conversion
/// Protocol layer should implement this to provide parsed data
pub trait ProtocolResponse {
    /// Parse response data as registers (u16 values)
    fn parse_registers(&self) -> Result<Vec<u16>>;

    /// Parse response data as bits (bool values)
    fn parse_bits(&self) -> Result<Vec<bool>>;
}

/// Raw protocol value types from parsed protocol response
/// 从协议解析响应中获得的原始协议值类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawProtocolValue {
    /// Register values (u16) - from parse_registers()
    Registers(Vec<u16>),
    /// Bit values (bool) - from parse_bits()  
    Bits(Vec<bool>),
    /// Single register value
    SingleRegister(u16),
    /// Single bit value
    SingleBit(bool),
}

impl RawProtocolValue {
    /// Create from ProtocolResponse register parsing
    pub fn from_registers(response: &dyn ProtocolResponse) -> Result<Self> {
        let registers = response.parse_registers()?;
        Ok(RawProtocolValue::Registers(registers))
    }

    /// Create from ProtocolResponse bit parsing
    pub fn from_bits(response: &dyn ProtocolResponse) -> Result<Self> {
        let bits = response.parse_bits()?;
        Ok(RawProtocolValue::Bits(bits))
    }

    /// Extract a specific register value by index
    pub fn get_register(&self, index: usize) -> Result<u16> {
        match self {
            RawProtocolValue::Registers(registers) => {
                registers.get(index).copied().ok_or_else(|| {
                    ComSrvError::DataConversionError(format!(
                        "Register index {} out of range",
                        index
                    ))
                })
            }
            RawProtocolValue::SingleRegister(value) => {
                if index == 0 {
                    Ok(*value)
                } else {
                    Err(ComSrvError::DataConversionError(format!(
                        "Single register but requested index {}",
                        index
                    )))
                }
            }
            _ => Err(ComSrvError::DataConversionError(
                "Cannot extract register from non-register data".to_string(),
            )),
        }
    }

    /// Extract a specific bit value by index
    pub fn get_bit(&self, index: usize) -> Result<bool> {
        match self {
            RawProtocolValue::Bits(bits) => bits.get(index).copied().ok_or_else(|| {
                ComSrvError::DataConversionError(format!("Bit index {} out of range", index))
            }),
            RawProtocolValue::SingleBit(value) => {
                if index == 0 {
                    Ok(*value)
                } else {
                    Err(ComSrvError::DataConversionError(format!(
                        "Single bit but requested index {}",
                        index
                    )))
                }
            }
            _ => Err(ComSrvError::DataConversionError(
                "Cannot extract bit from non-bit data".to_string(),
            )),
        }
    }

    /// Extract a bit from a register at specified bit position (0-15)
    pub fn get_register_bit(&self, register_index: usize, bit_position: u8) -> Result<bool> {
        if bit_position > 15 {
            return Err(ComSrvError::DataConversionError(format!(
                "Bit position {} out of range (0-15)",
                bit_position
            )));
        }

        let register_value = self.get_register(register_index)?;
        let mask = 1u16 << bit_position;
        Ok((register_value & mask) != 0)
    }

    /// Convert to f64 for analog processing
    pub fn to_f64(&self, index: usize) -> Result<f64> {
        match self {
            RawProtocolValue::Registers(_registers) => {
                let value = self.get_register(index)?;
                Ok(value as f64)
            }
            RawProtocolValue::SingleRegister(value) => {
                if index == 0 {
                    Ok(*value as f64)
                } else {
                    Err(ComSrvError::DataConversionError(format!(
                        "Single register but requested index {}",
                        index
                    )))
                }
            }
            _ => Err(ComSrvError::DataConversionError(
                "Cannot convert non-register data to float".to_string(),
            )),
        }
    }

    /// Convert to bool for digital processing
    pub fn to_bool(&self, index: usize) -> Result<bool> {
        match self {
            RawProtocolValue::Bits(_bits) => self.get_bit(index),
            RawProtocolValue::SingleBit(value) => {
                if index == 0 {
                    Ok(*value)
                } else {
                    Err(ComSrvError::DataConversionError(format!(
                        "Single bit but requested index {}",
                        index
                    )))
                }
            }
            RawProtocolValue::Registers(_) | RawProtocolValue::SingleRegister(_) => {
                // For registers, treat non-zero as true
                let register_value = self.get_register(index)?;
                Ok(register_value != 0)
            }
        }
    }
}

/// Generic connection state used by [`ConnectionManager`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Channel is disconnected.
    Disconnected,
    /// Channel is attempting to establish a connection.
    Connecting,
    /// Channel is connected and operational.
    Connected,
    /// Channel encountered an error during connection.
    Error(String),
}

/// Unified trait for connection management across protocols.
///
/// Implementors should handle protocol specific connect/disconnect logic
/// while updating the provided [`ConnectionState`] information.
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    /// Connect to the remote endpoint.
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the remote endpoint.
    async fn disconnect(&mut self) -> Result<()>;

    /// Attempt to reconnect using protocol specific strategy.
    async fn reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    /// Retrieve the current connection state.
    async fn connection_state(&self) -> ConnectionState;
}

/// Trait for configuration validation of protocol implementations.
#[async_trait]
pub trait ConfigValidator: Send + Sync {
    /// Validate configuration parameters.
    async fn validate_config(&self) -> Result<()> {
        Ok(())
    }
}

/// Trait representing protocol specific statistics collection.
pub trait ProtocolStats: Send + Sync {
    /// Reset all statistic counters.
    fn reset(&mut self);
}

/// Primary communication interface for all protocol implementations
///
/// This trait defines the standard interface that all communication protocols
/// must implement to integrate with the Communication Service. It provides
/// a consistent API for managing protocol lifecycle, monitoring status,
/// and accessing real-time data.
///
/// ## Design Principles
///
/// - **Protocol Agnostic**: Works with any communication protocol
/// - **Async by Default**: All operations are asynchronous for scalability
/// - **Status Monitoring**: Built-in status reporting and error tracking
/// - **Type Safety**: Strongly typed interfaces with clear error handling
///
/// ## Implementation Requirements
///
/// All implementing types must:
/// - Be `Send + Sync` for thread safety
/// - Implement `Debug` for logging and debugging
/// - Handle errors gracefully without panicking
/// - Provide accurate status information
///
/// ## Lifecycle Management
///
/// The typical lifecycle of a communication service:
/// 1. Creation and configuration
/// 2. Start operation (`start()`)
/// 3. Normal operation with data collection
/// 4. Status monitoring and error handling
/// 5. Graceful shutdown (`stop()`)
///
/// # Examples
///
/// ```
/// use comsrv::core::protocols::common::combase::{ComBase, ChannelStatus};
/// use comsrv::utils::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create a mock service that implements ComBase
///     struct MockService;
///     
///     // Implementation would go here
///     println!("ComBase usage example");
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait ComBase: Send + Sync + std::fmt::Debug {
    /// Downcast helper for dynamic protocol access
    fn as_any(&self) -> &dyn std::any::Any;
    /// Get the human-readable name of the communication service
    ///
    /// Returns a descriptive name for this communication service instance,
    /// typically used for logging, monitoring, and user interfaces.
    ///
    /// # Returns
    ///
    /// Service name as a string slice
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # fn example(service: &dyn ComBase) {
    /// println!("Service name: {}", service.name());
    /// # }
    /// ```
    fn name(&self) -> &str;

    /// Get the unique channel identifier
    ///
    /// Returns a unique identifier for this communication channel,
    /// used for distinguishing between multiple channels and for
    /// configuration management.
    ///
    /// # Returns
    ///
    /// Channel ID as a string
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # fn example(service: &dyn ComBase) {
    /// let channel_id = service.channel_id();
    /// println!("Channel: {}", channel_id);
    /// # }
    /// ```
    fn channel_id(&self) -> String;

    /// Get the protocol type identifier
    ///
    /// Returns the type of communication protocol implemented by this service,
    /// such as "ModbusTcp", "ModbusRtu", "IEC60870", etc.
    ///
    /// # Returns
    ///
    /// Protocol type as a string slice
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # fn example(service: &dyn ComBase) {
    /// match service.protocol_type() {
    ///     "ModbusTcp" => println!("Using Modbus TCP protocol"),
    ///     "IEC60870" => println!("Using IEC 60870 protocol"),
    ///     _ => println!("Unknown protocol"),
    /// }
    /// # }
    /// ```
    fn protocol_type(&self) -> &str;

    /// Get protocol-specific parameters and configuration
    ///
    /// Returns a map of configuration parameters specific to this protocol
    /// implementation. These parameters can be used for diagnostics,
    /// monitoring, or dynamic reconfiguration.
    ///
    /// # Returns
    ///
    /// HashMap containing parameter names and values as strings
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # fn example(service: &dyn ComBase) {
    /// let params = service.get_parameters();
    /// if let Some(host) = params.get("host") {
    ///     println!("Connected to host: {}", host);
    /// }
    /// # }
    /// ```
    fn get_parameters(&self) -> HashMap<String, String>;

    /// Check if the communication service is currently running
    ///
    /// Determines whether the service is in an active, running state
    /// and capable of processing communication requests.
    ///
    /// # Returns
    ///
    /// `true` if the service is running, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # async fn example(service: &dyn ComBase) {
    /// if service.is_running().await {
    ///     println!("Service is active");
    /// } else {
    ///     println!("Service is stopped");
    /// }
    /// # }
    /// ```
    async fn is_running(&self) -> bool;

    /// Start the communication service
    ///
    /// Initiates the communication service and sets the running state to true.
    /// This base implementation only manages the running state; protocol-specific
    /// implementations should override this method to perform actual connection setup.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Service started successfully
    /// * `Err(error)` - Failure during startup
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<()> {
    /// // Service startup is handled by ComBaseImpl
    /// service.start().await?;
    /// assert!(service.is_running().await);
    /// # Ok(())
    /// # }
    /// ```
    async fn start(&mut self) -> Result<()>;

    /// Stop the communication service gracefully
    ///
    /// Stops the communication service and sets the running state to false.
    /// This base implementation only manages the running state; protocol-specific
    /// implementations should override this method to perform actual connection cleanup.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Service stopped successfully
    /// * `Err(error)` - Failure during shutdown
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<()> {
    /// // Service shutdown is handled by ComBaseImpl
    /// service.stop().await?;
    /// assert!(!service.is_running().await);
    /// # Ok(())
    /// # }
    /// ```
    async fn stop(&mut self) -> Result<()>;

    /// Get the current status of the communication channel
    ///
    /// Returns a snapshot of the current channel status including connection state,
    /// response time metrics, error conditions, and last update timestamp.
    ///
    /// # Returns
    ///
    /// Current `ChannelStatus` with up-to-date information
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) {
    /// let status = service.status().await;
    /// println!("Channel {}: connected={}", status.id, status.connected);
    /// # }
    /// ```
    async fn status(&self) -> ChannelStatus;

    /// Check if the channel currently has an error condition
    ///
    /// Convenience method that checks the current status for error conditions.
    /// This provides a quick way to determine if the channel is experiencing
    /// problems without retrieving the full status.
    ///
    /// # Returns
    ///
    /// `true` if there is an active error condition, `false` otherwise
    ///
    /// # Default Implementation
    ///
    /// The default implementation calls `status().await.has_error()`,
    /// but implementations may override this for better performance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # async fn example(service: &dyn ComBase) {
    /// if service.has_error().await {
    ///     println!("Channel has active errors");
    ///     let error_msg = service.last_error().await;
    ///     println!("Error details: {}", error_msg);
    /// }
    /// # }
    /// ```
    async fn has_error(&self) -> bool {
        self.status().await.has_error()
    }

    /// Get the most recent error message from the channel
    ///
    /// Returns the error message from the most recent error condition.
    /// If there are no current errors, returns an empty string.
    ///
    /// # Returns
    ///
    /// Error message as a string (empty if no errors)
    ///
    /// # Default Implementation
    ///
    /// The default implementation calls `status().await.last_error`,
    /// but implementations may override this for better performance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # async fn example(service: &dyn ComBase) {
    /// let error_msg = service.last_error().await;
    /// if !error_msg.is_empty() {
    ///     eprintln!("Channel error: {}", error_msg);
    /// }
    /// # }
    /// ```
    async fn last_error(&self) -> String {
        self.status().await.last_error
    }

    /// Get all real-time data points from the communication channel
    ///
    /// Retrieves all available data points from the channel with their
    /// current values and timestamps. This method is used for bulk data 
    /// collection and monitoring.
    ///
    /// # Returns
    ///
    /// Vector of `PointData` structures containing all available data points
    ///
    /// # Default Implementation
    ///
    /// The default implementation returns an empty vector. Protocol
    /// implementations should override this method to provide actual
    /// data point collection.
    ///
    /// # Performance Considerations
    ///
    /// This method may involve network communication and should be
    /// called with appropriate frequency based on system requirements.
    /// Consider caching strategies for high-frequency access.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # async fn example(service: &dyn ComBase) {
    /// let points = service.get_all_points().await;
    ///
    /// for point in points {
    ///     println!("Point {}: {}", point.id, point.value);
    /// }
    /// # }
    /// ```
    async fn get_all_points(&self) -> Vec<PointData> {
        Vec::new() // Default implementation returns an empty vector
    }

    /// Log channel connection events with mapping context
    ///
    /// Protocol-specific implementations should override this to provide
    /// detailed logging with channel-specific information and mapping details.
    ///
    /// # Arguments
    /// * `action` - The action being performed (e.g., "connecting", "connected", "connect_failed")
    /// * `mapping_info` - Protocol-specific mapping information (e.g., "slave_id=1, address=127.0.0.1:502")
    /// * `details` - Additional details about the connection
    fn log_connection(&self, action: &str, mapping_info: &str, details: &str) {
        info!(
            channel = %self.channel_id(),
            protocol = %self.protocol_type(),
            action = action,
            mapping = mapping_info,
            details = details,
            "Channel connection event"
        );
    }

    /// Log data operation with mapping information
    ///
    /// Protocol-specific implementations should override this to provide
    /// detailed logging with data source, mapping, and transformation details.
    ///
    /// # Arguments
    /// * `action` - The action being performed (e.g., "Read_03", "Write_06", "data_received")
    /// * `point_name` - Name of the data point
    /// * `mapping_info` - Protocol-specific mapping information (e.g., "slave_id=1, address=40001, type=holding")
    /// * `value_info` - Value information (e.g., "raw=1234, scaled=12.34")
    /// * `details` - Additional details about the operation
    fn log_data_operation(&self, action: &str, point_name: &str, mapping_info: &str, value_info: &str, details: &str) {
        info!(
            channel = %self.channel_id(),
            protocol = %self.protocol_type(),
            action = action,
            point_name = point_name,
            mapping = mapping_info,
            value = value_info,
            details = details,
            "Data operation event"
        );
    }

    /// Log error with mapping context
    ///
    /// Protocol-specific implementations should override this to provide
    /// detailed error logging with mapping and context information.
    ///
    /// # Arguments
    /// * `action` - The action that failed (e.g., "Read_03", "Write_06", "connect")
    /// * `point_name` - Name of the data point (if applicable)
    /// * `mapping_info` - Protocol-specific mapping information
    /// * `error` - Error details
    fn log_error(&self, action: &str, point_name: Option<&str>, mapping_info: &str, error: &str) {
        error!(
            channel = %self.channel_id(),
            protocol = %self.protocol_type(),
            action = action,
            point_name = point_name.unwrap_or("N/A"),
            mapping = mapping_info,
            error = error,
            "Channel operation error"
        );
    }
}

/// Protocol logging trait for unified logging across all communication protocols
///
/// This trait provides standardized logging methods that can be used by all protocol
/// implementations. It's separate from ComBase to maintain object safety while
/// providing rich logging capabilities.
pub trait ProtocolLogger: Send + Sync {
    /// Get the channel ID for logging context
    fn channel_id(&self) -> String;

    /// Get the protocol type for logging context  
    fn protocol_type(&self) -> &str;

    /// Log protocol connection events with standardized format
    ///
    /// Provides a unified way to log connection-related events across all protocols.
    /// Uses the channel ID as the log target for filtering.
    ///
    /// # Arguments
    ///
    /// * `event` - Connection event ("connecting", "connected", "disconnected", "reconnecting")
    /// * `details` - Optional additional details about the connection event
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ProtocolLogger;
    /// # async fn example(logger: &dyn ProtocolLogger) {
    /// logger.log_connection("connecting", Some("192.168.1.100:502")).await;
    /// logger.log_connection("connected", None).await;
    /// logger.log_connection("disconnected", Some("Connection timeout")).await;
    /// # }
    /// ```
    async fn log_connection(&self, event: &str, details: Option<&str>) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");

        let message = match details {
            Some(detail) => format!("== [{}] {} {} ({})", timestamp, event, protocol, detail),
            None => format!("== [{}] {} {}", timestamp, event, protocol),
        };

        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);

        match event {
            "connected" | "reconnected" => log_info!("[{}] {}", target, message),
            "connecting" | "reconnecting" => log_info!("[{}] {}", target, message),
            "disconnected" => log_warn!("[{}] {}", target, message),
            _ => log_debug!("[{}] {}", target, message),
        }
    }

    /// Log protocol operation success
    ///
    /// Logs successful protocol operations with timing information.
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation type ("read", "write", "batch_read", etc.)
    /// * `direction` - Direction indicator (">>" for request, "<<" for response)
    /// * `details` - Operation details (address, value, etc.)
    /// * `result_value` - Success result value
    /// * `duration_ms` - Operation duration in milliseconds
    async fn log_operation_success(
        &self,
        operation: &str,
        direction: &str,
        details: &str,
        result_value: &str,
        duration_ms: u128,
    ) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");

        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!(
            "{} [{}] {} {} OK: {} ({}ms)",
            direction, timestamp, operation, details, result_value, duration_ms
        );

        log_debug!("{}", message);
    }

    /// Log protocol operation failure
    ///
    /// Logs failed protocol operations with timing information.
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation type
    /// * `direction` - Direction indicator  
    /// * `details` - Operation details
    /// * `error_msg` - Error message
    /// * `duration_ms` - Operation duration in milliseconds
    async fn log_operation_error(
        &self,
        operation: &str,
        direction: &str,
        details: &str,
        error_msg: &str,
        duration_ms: u128,
    ) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");

        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!(
            "{} [{}] {} {} ERR: {} ({}ms)",
            direction, timestamp, operation, details, error_msg, duration_ms
        );

        log_error!("{}", message);
    }

    /// Log protocol operation request
    ///
    /// Logs the start of a protocol operation with request details.
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation type
    /// * `details` - Request details
    async fn log_request(&self, operation: &str, details: &str) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");

        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!(">> [{}] {} {}", timestamp, operation, details);

        log_debug!("{}", message);
    }

    /// Log protocol data synchronization success
    ///
    /// Logs successful data synchronization activities like Redis updates, batch operations, etc.
    ///
    /// # Arguments
    ///
    /// * `sync_type` - Type of synchronization ("redis_sync", "batch_update", etc.)
    /// * `count` - Number of items synchronized
    async fn log_data_sync_success(&self, sync_type: &str, count: usize) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");

        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!(
            "== [{}] {} completed: {} items",
            timestamp, sync_type, count
        );

        log_debug!("{}", message);
    }

    /// Log protocol data synchronization failure
    ///
    /// Logs failed data synchronization activities.
    ///
    /// # Arguments
    ///
    /// * `sync_type` - Type of synchronization
    /// * `count` - Number of items attempted
    /// * `error_msg` - Error message
    async fn log_data_sync_error(&self, sync_type: &str, count: usize, error_msg: &str) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");

        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!(
            "== [{}] {} failed: {} (attempted {} items)",
            timestamp, sync_type, error_msg, count
        );

        log_error!("{}", message);
    }

    /// Convenience method to log operation results with automatic timing
    ///
    /// This method handles both success and error cases with proper timing calculation.
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation type
    /// * `direction` - Direction indicator
    /// * `details` - Operation details
    /// * `result` - Operation result
    /// * `start_time` - Operation start time
    async fn log_operation_result<T, E>(
        &self,
        operation: &str,
        direction: &str,
        details: &str,
        result: &std::result::Result<T, E>,
        start_time: Instant,
    ) where
        T: std::fmt::Display,
        E: std::fmt::Display,
    {
        let duration_ms = start_time.elapsed().as_millis();

        match result {
            Ok(value) => {
                self.log_operation_success(
                    operation,
                    direction,
                    details,
                    &value.to_string(),
                    duration_ms,
                )
                .await;
            }
            Err(error) => {
                self.log_operation_error(
                    operation,
                    direction,
                    details,
                    &error.to_string(),
                    duration_ms,
                )
                .await;
            }
        }
    }

    /// Convenience method to log data sync results
    ///
    /// # Arguments
    ///
    /// * `sync_type` - Type of synchronization
    /// * `count` - Number of items
    /// * `result` - Synchronization result
    async fn log_data_sync_result<E>(
        &self,
        sync_type: &str,
        count: usize,
        result: &std::result::Result<(), E>,
    ) where
        E: std::fmt::Display,
    {
        match result {
            Ok(()) => {
                self.log_data_sync_success(sync_type, count).await;
            }
            Err(error) => {
                self.log_data_sync_error(sync_type, count, &error.to_string())
                    .await;
            }
        }
    }
}

/// Base implementation of the ComBase trait
///
/// `ComBaseImpl` provides a reference implementation of the `ComBase` trait
/// with common functionality that can be used by protocol implementations.
/// It handles status management, error tracking, and performance monitoring.
///
/// # Features
///
/// - **Status Management**: Automatic status tracking and updates
/// - **Error Handling**: Built-in error tracking and reporting
/// - **Performance Monitoring**: Response time measurement utilities
/// - **Thread Safety**: All operations are thread-safe using Arc and RwLock
///
/// # Usage
///
/// This implementation can be used as a base for custom protocol implementations
/// or as a standalone service for testing and development.
///
/// # Examples
///
/// ```rust
/// use comsrv::core::protocols::common::combase::ComBaseImpl;
/// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
/// use std::collections::HashMap;
///
/// // Create a test configuration
/// let config = ChannelConfig {
///     id: 1,
///     name: "Test Channel".to_string(),
///     description: "Test Description".to_string(),
///     protocol: ProtocolType::ModbusTcp,
///     parameters: ChannelParameters::Generic(HashMap::new()),
/// };
///
/// let service = ComBaseImpl::new("test_service", "modbus_tcp", config);
/// assert_eq!(service.name(), "test_service");
/// assert_eq!(service.protocol_type(), "modbus_tcp");
/// ```
#[derive(Debug)]
pub struct ComBaseImpl {
    /// Service name
    name: String,
    /// Channel ID
    channel_id: u16,
    /// Protocol type
    protocol_type: String,
    /// Channel configuration
    config: ChannelConfig,
    /// Channel status
    status: Arc<RwLock<ChannelStatus>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Last error message
    last_error: Arc<RwLock<String>>,
}

impl ComBaseImpl {
    /// Create a new ComBaseImpl instance
    ///
    /// Initializes a new base implementation with the specified name, protocol type,
    /// and configuration. The instance starts in a stopped state with no errors.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the service
    /// * `protocol_type` - Protocol type identifier (e.g., "ModbusTcp", "ModbusRtu")
    /// * `config` - Channel configuration with protocol-specific parameters
    ///
    /// # Returns
    ///
    /// New `ComBaseImpl` instance ready for use
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// let config = ChannelConfig {
    ///     id: 1,
    ///     name: "Test Channel".to_string(),
    ///     description: "Test Description".to_string(),
    ///     protocol: ProtocolType::ModbusTcp,
    ///     parameters: ChannelParameters::Generic(HashMap::new()),
    /// };
    ///
    /// let service = ComBaseImpl::new("ModbusService", "ModbusTcp", config);
    /// assert_eq!(service.name(), "ModbusService");
    /// assert_eq!(service.protocol_type(), "ModbusTcp");
    /// ```
    pub fn new(name: &str, protocol_type: &str, config: ChannelConfig) -> Self {
        let channel_id = config.id;
        let status = ChannelStatus::new(&channel_id.to_string());

        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            config,
            status: Arc::new(RwLock::new(status)),
            running: Arc::new(RwLock::new(false)),
            last_error: Arc::new(RwLock::new(String::new())),
        }
    }

    /// Get the human-readable name of the communication service
    ///
    /// # Returns
    ///
    /// Service name as a string slice
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the unique channel identifier as a string
    ///
    /// # Returns
    ///
    /// Channel ID converted to string format
    pub fn channel_id(&self) -> String {
        self.channel_id.to_string()
    }

    /// Get the protocol type identifier
    ///
    /// # Returns
    ///
    /// Protocol type as a string slice
    pub fn protocol_type(&self) -> &str {
        &self.protocol_type
    }

    /// Get protocol parameters as a HashMap
    ///
    /// Converts the configuration to a string-based parameter map
    /// for use in monitoring, diagnostics, or dynamic configuration.
    ///
    /// # Returns
    ///
    /// HashMap containing basic protocol parameters
    ///
    /// # Note
    ///
    /// Additional parameters can be added by extending this method
    /// in protocol-specific implementations.
    pub fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        // Convert configuration to HashMap
        params.insert("protocol".to_string(), self.protocol_type.clone());
        params.insert("channel_id".to_string(), self.channel_id.to_string());
        // More parameters extracted from config can be added in actual implementation
        params
    }

    /// Get a reference to the channel configuration
    ///
    /// Provides read-only access to the complete channel configuration
    /// for use by protocol implementations.
    ///
    /// # Returns
    ///
    /// Immutable reference to the channel configuration
    pub fn config(&self) -> &ChannelConfig {
        &self.config
    }

    /// Check if the communication service is currently running
    ///
    /// Thread-safe check of the current running state.
    ///
    /// # Returns
    ///
    /// `true` if the service is running, `false` otherwise
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Start the communication service
    ///
    /// Initiates the communication service and sets the running state to true.
    /// This base implementation only manages the running state; protocol-specific
    /// implementations should override this method to perform actual connection setup.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Service started successfully
    /// * `Err(error)` - Failure during startup
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<()> {
    /// // Service startup is handled by ComBaseImpl
    /// service.start().await?;
    /// assert!(service.is_running().await);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(&self) -> Result<()> {
        self.set_running(true).await;
        self.update_status(false, 0.0, None).await;
        Ok(())
    }

    /// Stop the communication service gracefully
    ///
    /// Stops the communication service and sets the running state to false.
    /// This base implementation only manages the running state; protocol-specific
    /// implementations should override this method to perform actual connection cleanup.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Service stopped successfully
    /// * `Err(error)` - Failure during shutdown
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<()> {
    /// // Service shutdown is handled by ComBaseImpl
    /// service.stop().await?;
    /// assert!(!service.is_running().await);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stop(&self) -> Result<()> {
        self.set_running(false).await;
        self.update_status(false, 0.0, None).await;
        Ok(())
    }

    /// Get the current status of the communication channel
    ///
    /// Returns a snapshot of the current channel status including connection state,
    /// response time metrics, error conditions, and last update timestamp.
    ///
    /// # Returns
    ///
    /// Current `ChannelStatus` with up-to-date information
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) {
    /// let status = service.status().await;
    /// println!("Channel {}: connected={}", status.id, status.connected);
    /// # }
    /// ```
    pub async fn status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }

    /// Update the channel status with new information
    ///
    /// Updates the channel status with connection state, response time, and
    /// optional error information. This method is typically called by
    /// protocol implementations to report status changes.
    ///
    /// # Arguments
    ///
    /// * `connected` - Current connection state
    /// * `response_time` - Response time in milliseconds
    /// * `error` - Optional error message (None to clear errors)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) {
    /// // Update status after successful operation
    /// service.update_status(true, 150.5, None).await;
    ///
    /// // Update status with error condition
    /// service.update_status(false, 0.0, Some("Connection failed")).await;
    /// # }
    /// ```
    pub async fn update_status(&self, connected: bool, response_time: f64, error: Option<&str>) {
        let mut status = self.status.write().await;
        status.connected = connected;
        status.last_response_time = response_time;
        status.last_update_time = Utc::now();

        if let Some(err) = error {
            status.last_error = err.to_string();
            // Also update the separate error field
            *self.last_error.write().await = err.to_string();
        } else if !connected {
            // Clear error when disconnected normally
            status.last_error.clear();
            self.last_error.write().await.clear();
        }
    }

    /// Measure execution time of a synchronous operation
    ///
    /// Executes the provided function and measures its execution time.
    /// The execution time is automatically reported to the channel status.
    ///
    /// # Arguments
    ///
    /// * `f` - Function to execute and measure
    ///
    /// # Returns
    ///
    /// Result of the executed function
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<String> {
    /// let result = service.measure_execution(|| {
    ///     // Simulate some work
    ///     std::thread::sleep(std::time::Duration::from_millis(100));
    ///     Ok("Operation completed".to_string())
    /// }).await?;
    /// # Ok(result)
    /// # }
    /// ```
    pub async fn measure_execution<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();

        // Update status based on the result
        match &result {
            Ok(_) => {
                self.update_status(true, duration.as_secs_f64() * 1000.0, None)
                    .await;
            }
            Err(e) => {
                self.update_status(false, duration.as_secs_f64() * 1000.0, Some(&e.to_string()))
                    .await;
            }
        }

        result
    }

    /// Measure execution time of a synchronous operation that returns a Result
    ///
    /// Executes the provided function and measures its execution time.
    /// Updates the channel status based on the operation result.
    ///
    /// # Arguments
    ///
    /// * `f` - Synchronous function to execute and measure
    ///
    /// # Returns
    ///
    /// Result of the executed function
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) -> std::result::Result<String, String> {
    /// let result = service.measure_result_execution(|| {
    ///     // Simulate sync work that returns a Result
    ///     sync_operation()
    /// }).await?;
    /// # Ok(result)
    /// # }
    /// # fn sync_operation() -> std::result::Result<String, String> { Ok("Done".to_string()) }
    /// ```
    pub async fn measure_result_execution<F, T, E>(&self, f: F) -> std::result::Result<T, E>
    where
        F: FnOnce() -> std::result::Result<T, E> + Send,
        E: ToString,
    {
        let start = std::time::Instant::now();
        let result = f();
        let duration = start.elapsed();
        let response_time = duration.as_millis() as f64;

        match &result {
            Ok(_) => {
                self.update_status(true, response_time, None).await;
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.update_status(false, response_time, Some(&error_msg))
                    .await;
            }
        }

        result
    }

    /// Set an error condition for the channel
    ///
    /// Records an error message and updates the channel status to reflect
    /// the error condition. This method is used by protocol implementations
    /// to report error states.
    ///
    /// # Arguments
    ///
    /// * `error` - Error message to record
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) {
    /// service.set_error("Connection timeout occurred").await;
    /// assert!(service.status().await.has_error());
    /// # }
    /// ```
    pub async fn set_error(&self, error: &str) {
        *self.last_error.write().await = error.to_string();

        // Also update the status
        let mut status = self.status.write().await;
        status.last_error = error.to_string();
        status.last_update_time = Utc::now();
    }

    /// Set the running state of the service
    ///
    /// Updates the internal running state. This method is used internally
    /// by start/stop operations and can be used by protocol implementations
    /// for state management.
    ///
    /// # Arguments
    ///
    /// * `running` - New running state
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    ///
    /// # async fn example(service: &ComBaseImpl) {
    /// service.set_running(true).await;
    /// assert!(service.is_running().await);
    ///
    /// service.set_running(false).await;
    /// assert!(!service.is_running().await);
    /// # }
    /// ```
    pub async fn set_running(&self, running: bool) {
        *self.running.write().await = running;
    }
}

/// Protocol packet parsing result
///
/// Contains human-readable interpretation of protocol packets,
/// including packet structure and data content.
#[derive(Debug, Clone)]
pub struct PacketParseResult {
    /// Protocol type (e.g., "Modbus", "IEC60870", "CAN")
    pub protocol: String,
    /// Packet direction ("send" or "receive")
    pub direction: String,
    /// Hexadecimal representation of raw data
    pub hex_data: String,
    /// Human-readable description of packet structure
    pub description: String,
    /// Parsed data fields
    pub fields: HashMap<String, String>,
    /// Whether parsing was successful
    pub success: bool,
    /// Error message if parsing failed
    pub error: Option<String>,
}

impl PacketParseResult {
    /// Create a new successful parse result
    pub fn success(
        protocol: &str,
        direction: &str,
        hex_data: &str,
        description: &str,
        fields: HashMap<String, String>,
    ) -> Self {
        Self {
            protocol: protocol.to_string(),
            direction: direction.to_string(),
            hex_data: hex_data.to_string(),
            description: description.to_string(),
            fields,
            success: true,
            error: None,
        }
    }

    /// Create a new failed parse result
    pub fn failure(protocol: &str, direction: &str, hex_data: &str, error: &str) -> Self {
        Self {
            protocol: protocol.to_string(),
            direction: direction.to_string(),
            hex_data: hex_data.to_string(),
            description: format!("Parse error: {}", error),
            fields: HashMap::new(),
            success: false,
            error: Some(error.to_string()),
        }
    }

    /// Format as debug log entry
    pub fn format_debug_log(&self) -> String {
        if self.success {
            format!(
                "[{}] {} | {}",
                self.direction.to_uppercase(),
                self.hex_data,
                self.description
            )
        } else {
            format!(
                "[{}] {} | {} ({})",
                self.direction.to_uppercase(),
                self.hex_data,
                self.description,
                self.error.as_ref().unwrap_or(&"Unknown error".to_string())
            )
        }
    }
}

/// Protocol packet parser trait
///
/// Defines the interface for parsing protocol-specific packets.
/// Each protocol implementation should provide its own parser.
pub trait ProtocolPacketParser: Send + Sync {
    /// Get the protocol name
    fn protocol_name(&self) -> &str;

    /// Parse a packet and return human-readable interpretation
    fn parse_packet(&self, data: &[u8], direction: &str) -> PacketParseResult;

    /// Convert bytes to hexadecimal string
    fn format_hex_data(&self, data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Protocol packet parser registry
///
/// Manages multiple protocol parsers and routes packets to the appropriate parser.
pub struct ProtocolParserRegistry {
    parsers: HashMap<String, Box<dyn ProtocolPacketParser>>,
}

impl ProtocolParserRegistry {
    /// Create a new parser registry
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    /// Register a parser for a specific protocol
    pub fn register_parser<P>(&mut self, parser: P)
    where
        P: ProtocolPacketParser + 'static,
    {
        let protocol_name = parser.protocol_name().to_string();
        self.parsers.insert(protocol_name, Box::new(parser));
    }

    /// Parse a packet using the appropriate protocol parser
    pub fn parse_packet(&self, protocol: &str, data: &[u8], direction: &str) -> PacketParseResult {
        if let Some(parser) = self.parsers.get(protocol) {
            parser.parse_packet(data, direction)
        } else {
            // Fallback to basic hex representation
            let hex_data = data
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");

            PacketParseResult::failure(
                protocol,
                direction,
                &hex_data,
                &format!("No parser registered for protocol: {}", protocol),
            )
        }
    }

    /// Get list of registered protocols
    pub fn registered_protocols(&self) -> Vec<String> {
        self.parsers.keys().cloned().collect()
    }
}

use once_cell::sync::Lazy;
use parking_lot::RwLock as ParkingLotRwLock;

/// Global protocol parser registry protected by a read-write lock
static GLOBAL_PARSER_REGISTRY: Lazy<ParkingLotRwLock<ProtocolParserRegistry>> =
    Lazy::new(|| ParkingLotRwLock::new(ProtocolParserRegistry::new()));

/// Get the global protocol parser registry
pub fn get_global_parser_registry() -> &'static ParkingLotRwLock<ProtocolParserRegistry> {
    &GLOBAL_PARSER_REGISTRY
}

/// Parse a protocol packet using the global registry
pub fn parse_protocol_packet(protocol: &str, data: &[u8], direction: &str) -> PacketParseResult {
    let registry = get_global_parser_registry();
    let registry = registry.read();
    registry.parse_packet(protocol, data, direction)
}

/// Polling Engine Trait
///
/// This trait abstracts the polling functionality and can be implemented
/// by any communication protocol. It provides a unified interface for
/// data collection across different protocols.
#[async_trait]
pub trait PollingEngine: Send + Sync {
    /// Start the polling engine
    async fn start_polling(&self, config: PollingConfig, points: Vec<PollingPoint>) -> Result<()>;

    /// Stop the polling engine
    async fn stop_polling(&self) -> Result<()>;

    /// Get current polling statistics
    async fn get_polling_stats(&self) -> PollingStats;

    /// Check if polling is currently active
    async fn is_polling_active(&self) -> bool;

    /// Update polling configuration at runtime
    async fn update_polling_config(&self, config: PollingConfig) -> Result<()>;

    /// Add or update polling points
    async fn update_polling_points(&self, points: Vec<PollingPoint>) -> Result<()>;

    /// Read a single point (protocol-specific implementation)
    async fn read_point(&self, point: &PollingPoint) -> Result<PointData>;

    /// Read multiple points in batch (protocol-specific optimization)
    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>>;
}

/// Universal Polling Engine Implementation
///
/// This is a generic polling engine that can be used by any protocol.
/// It handles the polling loop, statistics, and delegates actual reading
/// to protocol-specific implementations.
pub struct UniversalPollingEngine {
    /// Protocol name for logging
    protocol_name: String,
    /// Polling configuration
    config: Arc<RwLock<PollingConfig>>,
    /// Points to be polled
    points: Arc<RwLock<Vec<PollingPoint>>>,
    /// Polling statistics
    stats: Arc<RwLock<PollingStats>>,
    /// Running state
    is_running: Arc<RwLock<bool>>,
    /// Point reader implementation (protocol-specific)
    point_reader: Arc<dyn PointReader>,
    /// Data callback for storing read values
    data_callback: Option<Arc<dyn Fn(Vec<PointData>) + Send + Sync>>,
    /// Task handle for polling task
    task_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

/// Point Reader Trait
///
/// This trait must be implemented by each protocol to provide the actual
/// point reading functionality. The universal polling engine uses this
/// to delegate protocol-specific operations.
#[async_trait]
pub trait PointReader: Send + Sync {
    /// Read a single point using protocol-specific logic
    async fn read_point(&self, point: &PollingPoint) -> Result<PointData>;

    /// Read multiple points in batch (optional optimization)
    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
        // Default implementation: read points individually
        let mut results = Vec::new();
        for point in points {
            match self.read_point(point).await {
                Ok(data) => results.push(data),
                Err(e) => {
                    // Create error data point
                    results.push(PointData {
                        id: point.id.clone(),
                        name: point.name.clone(),
                        value: "null".to_string(),
                        timestamp: Utc::now(),
                        unit: point.unit.clone(),
                        description: format!("Failed to read point {}: {}", point.id, e),
                    });
                    warn!("Failed to read point {}: {}", point.id, e);
                }
            }
        }
        Ok(results)
    }

    /// Check if the connection is healthy
    async fn is_connected(&self) -> bool;

    /// Get protocol name for logging
    fn protocol_name(&self) -> &str;
}

impl UniversalPollingEngine {
    /// Create a new universal polling engine
    pub fn new(protocol_name: String, point_reader: Arc<dyn PointReader>) -> Self {
        Self {
            protocol_name,
            config: Arc::new(RwLock::new(PollingConfig::default())),
            points: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(PollingStats::default())),
            is_running: Arc::new(RwLock::new(false)),
            point_reader,
            data_callback: None,
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Set data callback for handling read data
    pub fn set_data_callback<F>(&mut self, callback: F)
    where
        F: Fn(Vec<PointData>) + Send + Sync + 'static,
    {
        self.data_callback = Some(Arc::new(callback));
    }
}

#[async_trait]
impl PollingEngine for UniversalPollingEngine {
    async fn start_polling(&self, config: PollingConfig, points: Vec<PollingPoint>) -> Result<()> {
        // Update configuration and points
        {
            let mut config_guard = self.config.write().await;
            *config_guard = config.clone();
        }
        {
            let mut points_guard = self.points.write().await;
            *points_guard = points;
        }

        // Check if already running
        {
            let mut running = self.is_running.write().await;
            if *running {
                return Err(ComSrvError::StateError(
                    "Polling engine already running".to_string(),
                ));
            }
            *running = true;
        }

        if !config.enabled {
            info!("Polling disabled for {} protocol", self.protocol_name);
            return Ok(());
        }

        info!(
            "Starting universal polling engine for {} protocol",
            self.protocol_name
        );
        info!(
            "Polling interval: {}ms, Max points per cycle: {}",
            config.interval_ms, config.max_points_per_cycle
        );

        // Start the polling task
        let handle = self.start_polling_task().await;

        // Store the task handle for cleanup
        {
            let mut task_handle = self.task_handle.write().await;
            *task_handle = Some(handle);
        }

        Ok(())
    }

    async fn stop_polling(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Abort the polling task if it's running
        {
            let mut handle = self.task_handle.write().await;
            if let Some(task) = handle.take() {
                task.abort();
            }
        }

        info!(
            "Stopped universal polling engine for {} protocol",
            self.protocol_name
        );
        Ok(())
    }

    async fn get_polling_stats(&self) -> PollingStats {
        self.stats.read().await.clone()
    }

    async fn is_polling_active(&self) -> bool {
        *self.is_running.read().await
    }

    async fn update_polling_config(&self, config: PollingConfig) -> Result<()> {
        {
            let mut config_guard = self.config.write().await;
            *config_guard = config;
        }
        info!(
            "Updated polling configuration for {} protocol",
            self.protocol_name
        );
        Ok(())
    }

    async fn update_polling_points(&self, points: Vec<PollingPoint>) -> Result<()> {
        {
            let mut points_guard = self.points.write().await;
            *points_guard = points;
        }
        info!("Updated polling points for {} protocol", self.protocol_name);
        Ok(())
    }

    async fn read_point(&self, point: &PollingPoint) -> Result<PointData> {
        self.point_reader.read_point(point).await
    }

    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
        self.point_reader.read_points_batch(points).await
    }
}

impl UniversalPollingEngine {
    /// Start the main polling task
    async fn start_polling_task(&self) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let points = self.points.clone();
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();
        let point_reader = self.point_reader.clone();
        let data_callback = self.data_callback.clone();
        let protocol_name = self.protocol_name.clone();

        return tokio::spawn(async move {
            let mut cycle_counter = 0u64;

            let mut current_interval_ms = config.read().await.interval_ms;
            let mut poll_interval = interval(Duration::from_millis(current_interval_ms));

            while *is_running.read().await {
                let config_snapshot = config.read().await.clone();

                if !config_snapshot.enabled {
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }

                if config_snapshot.interval_ms != current_interval_ms {
                    current_interval_ms = config_snapshot.interval_ms;
                    poll_interval = interval(Duration::from_millis(current_interval_ms));
                }

                poll_interval.tick().await;
                cycle_counter += 1;

                // Check connection before polling
                if !point_reader.is_connected().await {
                    debug!(
                        "Skipping polling cycle {} for {} - not connected",
                        cycle_counter, protocol_name
                    );
                    continue;
                }

                let cycle_start = Instant::now();

                // Execute polling cycle
                match Self::execute_polling_cycle(
                    &config_snapshot,
                    &points,
                    &point_reader,
                    &protocol_name,
                    cycle_counter,
                )
                .await
                {
                    Ok(read_data) => {
                        // Update statistics
                        let cycle_time = cycle_start.elapsed().as_millis() as f64;
                        Self::update_stats(&stats, true, read_data.len(), cycle_time).await;

                        // Call data callback if set
                        if let Some(ref callback) = data_callback {
                            callback(read_data);
                        }

                        debug!(
                            "Polling cycle {} completed for {} in {:.2}ms",
                            cycle_counter, protocol_name, cycle_time
                        );
                    }
                    Err(e) => {
                        // Update statistics for failed cycle
                        let cycle_time = cycle_start.elapsed().as_millis() as f64;
                        Self::update_stats(&stats, false, 0, cycle_time).await;

                        error!(
                            "Polling cycle {} failed for {}: {}",
                            cycle_counter, protocol_name, e
                        );
                    }
                }

                // Log periodic statistics
                if cycle_counter % 50 == 0 {
                    let current_stats = stats.read().await;
                    info!(
                        "Polling stats for {}: {}/{} successful, avg {:.2}ms",
                        protocol_name,
                        current_stats.successful_cycles,
                        current_stats.total_cycles,
                        current_stats.avg_cycle_time_ms
                    );
                }
            }

            info!("Polling task stopped for {} protocol", protocol_name);
        });
    }

    /// Execute a single polling cycle
    async fn execute_polling_cycle(
        config: &PollingConfig,
        points: &Arc<RwLock<Vec<PollingPoint>>>,
        point_reader: &Arc<dyn PointReader>,
        protocol_name: &str,
        cycle_number: u64,
    ) -> Result<Vec<PointData>> {
        let points_snapshot = points.read().await.clone();

        if points_snapshot.is_empty() {
            debug!(
                "No points configured for polling in {} protocol",
                protocol_name
            );
            return Ok(Vec::new());
        }

        debug!(
            "Starting polling cycle {} for {} protocol with {} points",
            cycle_number,
            protocol_name,
            points_snapshot.len()
        );

        let mut all_data = Vec::new();

        // Batch points by group if batch reading is enabled
        if config.enable_batch_reading {
            let grouped_points = Self::group_points_for_batch_reading(&points_snapshot);

            for (group_name, group_points) in grouped_points {
                debug!(
                    "Reading batch group '{}' with {} points",
                    group_name,
                    group_points.len()
                );

                match point_reader.read_points_batch(&group_points).await {
                    Ok(mut batch_data) => {
                        all_data.append(&mut batch_data);
                    }
                    Err(e) => {
                        warn!("Batch read failed for group '{}': {}", group_name, e);
                        // Fall back to individual reads
                        for point in group_points {
                            match point_reader.read_point(&point).await {
                                Ok(data) => all_data.push(data),
                                Err(e) => {
                                    warn!("Individual read failed for point {}: {}", point.id, e);
                                    // Add error data point
                                    all_data.push(PointData {
                                        id: point.id.clone(),
                                        name: point.name.clone(),
                                        value: "null".to_string(),
                                        timestamp: Utc::now(),
                                        unit: point.unit.clone(),
                                        description: format!(
                                            "Failed to read point {}: {}",
                                            point.id, e
                                        ),
                                    });
                                }
                            }

                            // Delay between individual reads
                            if config.point_read_delay_ms > 0 {
                                tokio::time::sleep(Duration::from_millis(
                                    config.point_read_delay_ms,
                                ))
                                .await;
                            }
                        }
                    }
                }
            }
        } else {
            // Read points individually
            for point in points_snapshot {
                match point_reader.read_point(&point).await {
                    Ok(data) => all_data.push(data),
                    Err(e) => {
                        warn!("Failed to read point {}: {}", point.id, e);
                        // Add error data point
                        all_data.push(PointData {
                            id: point.id.clone(),
                            name: point.name.clone(),
                            value: "null".to_string(),
                            timestamp: Utc::now(),
                            unit: point.unit.clone(),
                            description: format!("Failed to read point {}: {}", point.id, e),
                        });
                    }
                }

                // Delay between reads
                if config.point_read_delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(config.point_read_delay_ms)).await;
                }
            }
        }

        Ok(all_data)
    }

    /// Group points by their group name for batch reading
    fn group_points_for_batch_reading(
        points: &[PollingPoint],
    ) -> HashMap<String, Vec<PollingPoint>> {
        let mut grouped = HashMap::new();

        for point in points {
            let group_name = if point.group.is_empty() {
                "default".to_string()
            } else {
                point.group.clone()
            };

            grouped
                .entry(group_name)
                .or_insert_with(Vec::new)
                .push(point.clone());
        }

        grouped
    }

    /// Update polling statistics
    async fn update_stats(
        stats: &Arc<RwLock<PollingStats>>,
        success: bool,
        points_read: usize,
        cycle_time_ms: f64,
    ) {
        let mut stats_guard = stats.write().await;

        stats_guard.total_cycles += 1;

        if success {
            stats_guard.successful_cycles += 1;
            stats_guard.total_points_read += points_read as u64;
            stats_guard.last_successful_polling = Some(Utc::now());
            stats_guard.last_polling_error = None;
        } else {
            stats_guard.failed_cycles += 1;
        }

        // Update average cycle time
        let total_time =
            stats_guard.avg_cycle_time_ms * (stats_guard.total_cycles - 1) as f64 + cycle_time_ms;
        stats_guard.avg_cycle_time_ms = total_time / stats_guard.total_cycles as f64;



        // Update polling rate (approximate)
        if stats_guard.total_cycles > 1 && stats_guard.avg_cycle_time_ms > 0.0 {
            stats_guard.current_polling_rate = 1000.0 / stats_guard.avg_cycle_time_ms;
        } else {
            stats_guard.current_polling_rate = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{ChannelConfig, ChannelParameters, ProtocolType};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use tokio::time::{sleep, Duration, Instant};

    // Mock implementations for testing
    struct MockReader {
        connected: Arc<Mutex<bool>>,
        fail_reads: Arc<Mutex<bool>>,
        read_delay: Arc<Mutex<Option<Duration>>>,
    }

    impl MockReader {
        fn new() -> Self {
            Self {
                connected: Arc::new(Mutex::new(true)),
                fail_reads: Arc::new(Mutex::new(false)),
                read_delay: Arc::new(Mutex::new(None)),
            }
        }

        fn set_connected(&self, connected: bool) {
            *self.connected.lock().unwrap() = connected;
        }

        fn set_fail_reads(&self, fail: bool) {
            *self.fail_reads.lock().unwrap() = fail;
        }

        fn set_read_delay(&self, delay: Option<Duration>) {
            *self.read_delay.lock().unwrap() = delay;
        }
    }

    #[async_trait]
    impl PointReader for MockReader {
        async fn read_point(&self, point: &PollingPoint) -> Result<PointData> {
            let delay = *self.read_delay.lock().unwrap();
            if let Some(delay) = delay {
                sleep(delay).await;
            }

            let should_fail = *self.fail_reads.lock().unwrap();
            if should_fail {
                return Err(ComSrvError::CommunicationError(
                    "Mock read failure".to_string(),
                ));
            }

            Ok(PointData {
                id: point.id.clone(),
                name: point.name.clone(),
                value: format!("value_{}", point.address),
                timestamp: Utc::now(),
                unit: point.unit.clone(),
                description: point.description.clone(),
            })
        }

        async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
            let should_fail = *self.fail_reads.lock().unwrap();
            if should_fail {
                return Err(ComSrvError::CommunicationError(
                    "Mock batch read failure".to_string(),
                ));
            }

            let mut results = Vec::new();
            for point in points {
                results.push(self.read_point(point).await?);
            }
            Ok(results)
        }

        async fn is_connected(&self) -> bool {
            *self.connected.lock().unwrap()
        }

        fn protocol_name(&self) -> &str {
            "mock"
        }
    }

    struct MockParser {
        protocol: String,
    }

    impl MockParser {
        fn new(protocol: &str) -> Self {
            Self {
                protocol: protocol.to_string(),
            }
        }
    }

    impl ProtocolPacketParser for MockParser {
        fn protocol_name(&self) -> &str {
            &self.protocol
        }

        fn parse_packet(&self, data: &[u8], direction: &str) -> PacketParseResult {
            let hex_data = self.format_hex_data(data);
            let mut fields = HashMap::new();
            fields.insert("length".to_string(), data.len().to_string());
            fields.insert(
                "first_byte".to_string(),
                format!("0x{:02x}", data.first().unwrap_or(&0)),
            );

            PacketParseResult::success(
                &self.protocol,
                direction,
                &hex_data,
                &format!("{} packet with {} bytes", self.protocol, data.len()),
                fields,
            )
        }
    }

    fn create_test_config() -> ChannelConfig {
        ChannelConfig {
            id: 1,
            name: "Test Channel".to_string(),
            description: Some("Test Description".to_string()),
            protocol: ProtocolType::ModbusTcp,
            parameters: ChannelParameters::Generic(HashMap::new()),
            logging: crate::core::config::types::ChannelLoggingConfig::default(),
        }
    }

    fn create_test_point(id: &str, address: u32) -> PollingPoint {
        PollingPoint {
            id: id.to_string(),
            name: format!("Point {}", id),
            address,
            data_type: "u16".to_string(),
            telemetry_type: TelemetryType::Telemetry,
            scale: 1.0,
            offset: 0.0,
            unit: "V".to_string(),
            description: format!("Test point {}", id),
            access_mode: "read".to_string(),
            group: "default".to_string(),
            protocol_params: HashMap::new(),
            telemetry_metadata: None,
        }
    }

    // ChannelStatus Tests
    #[test]
    fn test_channel_status_new() {
        let status = ChannelStatus::new("test_channel");
        assert_eq!(status.id, "test_channel");
        assert!(!status.connected);
        assert_eq!(status.last_response_time, 0.0);
        assert!(status.last_error.is_empty());
        assert!(!status.has_error());
    }

    #[test]
    fn test_channel_status_has_error() {
        let mut status = ChannelStatus::new("test_channel");
        assert!(!status.has_error());

        status.last_error = "Connection failed".to_string();
        assert!(status.has_error());

        status.last_error.clear();
        assert!(!status.has_error());
    }

    // PointData Tests
    #[test]
    fn test_point_data_creation() {
        let now = Utc::now();
        let point = PointData {
            id: "test_point".to_string(),
            name: "Test Point".to_string(),
            value: "123.45".to_string(),
            timestamp: now,
            unit: "V".to_string(),
            description: "Test voltage point".to_string(),
        };

        assert_eq!(point.id, "test_point");
        assert_eq!(point.name, "Test Point");
        assert_eq!(point.value, "123.45");
        assert_eq!(point.timestamp, now);
        assert_eq!(point.unit, "V");
        assert_eq!(point.description, "Test voltage point");
    }

    // PollingConfig Tests
    #[test]
    fn test_polling_config_default() {
        let config = PollingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.max_points_per_cycle, 1000);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
        assert!(config.enable_batch_reading);
        assert_eq!(config.point_read_delay_ms, 10);
    }

    #[test]
    fn test_polling_config_custom() {
        let config = PollingConfig {
            enabled: false,
            interval_ms: 500,
            max_points_per_cycle: 100,
            timeout_ms: 2000,
            max_retries: 1,
            retry_delay_ms: 500,
            enable_batch_reading: false,
            point_read_delay_ms: 50,
        };

        assert!(!config.enabled);
        assert_eq!(config.interval_ms, 500);
        assert_eq!(config.max_points_per_cycle, 100);
        assert_eq!(config.timeout_ms, 2000);
        assert_eq!(config.max_retries, 1);
        assert_eq!(config.retry_delay_ms, 500);
        assert!(!config.enable_batch_reading);
        assert_eq!(config.point_read_delay_ms, 50);
    }

    // PollingStats Tests
    #[test]
    fn test_polling_stats_default() {
        let stats = PollingStats::default();
        assert_eq!(stats.total_cycles, 0);
        assert_eq!(stats.successful_cycles, 0);
        assert_eq!(stats.failed_cycles, 0);
        assert_eq!(stats.total_points_read, 0);
        assert_eq!(stats.total_points_failed, 0);
        assert_eq!(stats.avg_cycle_time_ms, 0.0);
        assert_eq!(stats.current_polling_rate, 0.0);
        assert!(stats.last_successful_polling.is_none());
        assert!(stats.last_polling_error.is_none());
    }

    // ConnectionState Tests
    #[test]
    fn test_connection_state_equality() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Connecting, ConnectionState::Connecting);
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_eq!(
            ConnectionState::Error("test".to_string()),
            ConnectionState::Error("test".to_string())
        );

        assert_ne!(ConnectionState::Connected, ConnectionState::Disconnected);
        assert_ne!(
            ConnectionState::Error("a".to_string()),
            ConnectionState::Error("b".to_string())
        );
    }

    // PacketParseResult Tests
    #[test]
    fn test_packet_parse_result_success() {
        let mut fields = HashMap::new();
        fields.insert("function_code".to_string(), "0x03".to_string());
        fields.insert("data_length".to_string(), "4".to_string());

        let result = PacketParseResult::success(
            "Modbus",
            "send",
            "01 03 00 00 00 02 c4 0b",
            "Read holding registers request",
            fields.clone(),
        );

        assert_eq!(result.protocol, "Modbus");
        assert_eq!(result.direction, "send");
        assert_eq!(result.hex_data, "01 03 00 00 00 02 c4 0b");
        assert_eq!(result.description, "Read holding registers request");
        assert_eq!(result.fields, fields);
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_packet_parse_result_failure() {
        let result =
            PacketParseResult::failure("Modbus", "receive", "01 83 02", "Invalid function code");

        assert_eq!(result.protocol, "Modbus");
        assert_eq!(result.direction, "receive");
        assert_eq!(result.hex_data, "01 83 02");
        assert!(result.description.contains("Parse error"));
        assert!(result.fields.is_empty());
        assert!(!result.success);
        assert!(result.error.is_some());
        assert_eq!(result.error.unwrap(), "Invalid function code");
    }

    #[test]
    fn test_packet_parse_result_format_debug_log() {
        let mut fields = HashMap::new();
        fields.insert("test".to_string(), "value".to_string());

        let success_result =
            PacketParseResult::success("Test", "send", "01 02 03", "Test packet", fields);

        let log = success_result.format_debug_log();
        assert!(log.contains("SEND"));
        assert!(log.contains("01 02 03"));
        assert!(log.contains("Test packet"));

        let failure_result =
            PacketParseResult::failure("Test", "receive", "04 05 06", "Parse failed");

        let log = failure_result.format_debug_log();
        assert!(log.contains("RECEIVE"));
        assert!(log.contains("04 05 06"));
        assert!(log.contains("Parse failed"));
    }

    // ProtocolParserRegistry Tests
    #[test]
    fn test_protocol_parser_registry() {
        let mut registry = ProtocolParserRegistry::new();
        assert!(registry.registered_protocols().is_empty());

        // Register parsers
        registry.register_parser(MockParser::new("Modbus"));
        registry.register_parser(MockParser::new("IEC60870"));

        let protocols = registry.registered_protocols();
        assert_eq!(protocols.len(), 2);
        assert!(protocols.contains(&"Modbus".to_string()));
        assert!(protocols.contains(&"IEC60870".to_string()));

        // Test parsing with registered protocol
        let data = [0x01, 0x03, 0x00, 0x00];
        let result = registry.parse_packet("Modbus", &data, "send");
        assert!(result.success);
        assert_eq!(result.protocol, "Modbus");
        assert_eq!(result.direction, "send");

        // Test parsing with unregistered protocol
        let result = registry.parse_packet("Unknown", &data, "send");
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("No parser registered"));
    }

    // ComBaseImpl Tests
    #[tokio::test]
    async fn test_combase_impl_creation() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        assert_eq!(service.name(), "TestService");
        assert_eq!(service.channel_id(), "1");
        assert_eq!(service.protocol_type(), "TestProtocol");
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_combase_impl_lifecycle() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Initial state
        assert!(!service.is_running().await);
        let status = service.status().await;
        assert!(!status.connected);
        assert!(!status.has_error());

        // Start service
        service.start().await.unwrap();
        assert!(service.is_running().await);

        // Stop service
        service.stop().await.unwrap();
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_combase_impl_status_updates() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Update status with success
        service.update_status(true, 123.45, None).await;
        let status = service.status().await;
        assert!(status.connected);
        assert_eq!(status.last_response_time, 123.45);
        assert!(!status.has_error());

        // Update status with error
        service
            .update_status(false, 0.0, Some("Connection failed"))
            .await;
        let status = service.status().await;
        assert!(!status.connected);
        assert_eq!(status.last_response_time, 0.0);
        assert!(status.has_error());
        assert_eq!(status.last_error, "Connection failed");
    }

    #[tokio::test]
    async fn test_combase_impl_error_handling() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Set error
        service.set_error("Test error message").await;
        let status = service.status().await;
        assert!(status.has_error());
        assert_eq!(status.last_error, "Test error message");

        // Clear error by updating status without error
        service.update_status(false, 0.0, None).await;
        let status = service.status().await;
        assert!(!status.has_error());
        assert!(status.last_error.is_empty());
    }

    #[tokio::test]
    async fn test_combase_impl_measure_execution() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Test successful execution
        let result = service
            .measure_execution(|| {
                std::thread::sleep(std::time::Duration::from_millis(10));
                Ok("success".to_string())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        let status = service.status().await;
        assert!(status.connected);
        assert!(status.last_response_time > 0.0);

        // Test failed execution
        let result = service
            .measure_execution(|| {
                Err::<String, ComSrvError>(ComSrvError::CommunicationError(
                    "Test error".to_string(),
                ))
            })
            .await;

        assert!(result.is_err());
        let status = service.status().await;
        assert!(!status.connected);
        assert!(status.has_error());
    }

    #[tokio::test]
    async fn test_combase_impl_measure_result_execution() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Test successful execution
        let result = service
            .measure_result_execution(|| {
                std::thread::sleep(std::time::Duration::from_millis(5));
                Ok::<String, String>("success".to_string())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        let status = service.status().await;
        assert!(status.connected);
        assert!(status.last_response_time > 0.0);

        // Test failed execution
        let result = service
            .measure_result_execution(|| Err::<String, String>("Test error".to_string()))
            .await;

        assert!(result.is_err());
        let status = service.status().await;
        assert!(!status.connected);
        assert!(status.has_error());
        assert_eq!(status.last_error, "Test error");
    }

    #[test]
    fn test_combase_impl_parameters() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        let params = service.get_parameters();
        assert_eq!(params.get("protocol").unwrap(), "TestProtocol");
        assert_eq!(params.get("channel_id").unwrap(), "1");
    }

    // Universal Polling Engine Tests
    #[tokio::test]
    async fn test_universal_polling_engine_creation() {
        let reader = Arc::new(MockReader::new());
        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        assert!(!engine.is_polling_active().await);
        let stats = engine.get_polling_stats().await;
        assert_eq!(stats.total_cycles, 0);
    }

    #[tokio::test]
    async fn test_universal_polling_engine_disabled() {
        let reader = Arc::new(MockReader::new());
        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let config = PollingConfig {
            enabled: false,
            ..Default::default()
        };

        let points = vec![create_test_point("p1", 1)];
        engine.start_polling(config, points).await.unwrap();

        assert!(engine.is_polling_active().await);

        // Wait a bit and check stats - should remain zero since polling is disabled
        sleep(Duration::from_millis(100)).await;
        let stats = engine.get_polling_stats().await;
        assert_eq!(stats.total_cycles, 0);

        engine.stop_polling().await.unwrap();
        assert!(!engine.is_polling_active().await);
    }

    #[tokio::test]
    async fn test_universal_polling_engine_successful_polling() {
        let reader = Arc::new(MockReader::new());
        let mut engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let collected_data: Arc<Mutex<Vec<Vec<PointData>>>> = Arc::new(Mutex::new(Vec::new()));
        let data_clone = collected_data.clone();
        engine.set_data_callback(move |data| {
            data_clone.lock().unwrap().push(data);
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 50,
            max_points_per_cycle: 10,
            timeout_ms: 1000,
            max_retries: 1,
            retry_delay_ms: 100,
            enable_batch_reading: false,
            point_read_delay_ms: 1,
        };

        let points = vec![create_test_point("p1", 1), create_test_point("p2", 2)];

        engine.start_polling(config, points).await.unwrap();
        assert!(engine.is_polling_active().await);

        // Wait for some polling cycles
        sleep(Duration::from_millis(200)).await;

        engine.stop_polling().await.unwrap();
        assert!(!engine.is_polling_active().await);

        // Check statistics
        let stats = engine.get_polling_stats().await;
        assert!(stats.total_cycles > 0);
        assert!(stats.successful_cycles > 0);
        assert_eq!(stats.failed_cycles, 0);
        assert!(stats.total_points_read > 0);
        assert_eq!(stats.total_points_failed, 0);
        assert!(stats.avg_cycle_time_ms > 0.0);

        // Check collected data
        let data = collected_data.lock().unwrap();
        assert!(!data.is_empty());
        for batch in data.iter() {
            assert_eq!(batch.len(), 2); // Two points
            for point in batch {
                assert!(point.value.starts_with("value_"));
            }
        }
    }

    #[tokio::test]
    async fn test_universal_polling_engine_failed_reads() {
        let reader = Arc::new(MockReader::new());
        reader.set_fail_reads(true); // Make all reads fail

        let mut engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let collected_data: Arc<Mutex<Vec<Vec<PointData>>>> = Arc::new(Mutex::new(Vec::new()));
        let data_clone = collected_data.clone();
        engine.set_data_callback(move |data| {
            data_clone.lock().unwrap().push(data);
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 50,
            max_points_per_cycle: 10,
            timeout_ms: 1000,
            max_retries: 1,
            retry_delay_ms: 10,
            enable_batch_reading: false,
            point_read_delay_ms: 1,
        };

        let points = vec![create_test_point("p1", 1)];

        engine.start_polling(config, points).await.unwrap();

        // Wait for some polling cycles
        sleep(Duration::from_millis(150)).await;

        engine.stop_polling().await.unwrap();

        // Check that data was still collected (with error points)
        let data = collected_data.lock().unwrap();
        assert!(!data.is_empty());
        for batch in data.iter() {
            for point in batch {
                assert_eq!(point.value, "null");
            }
        }
    }

    #[tokio::test]
    async fn test_universal_polling_engine_batch_reading() {
        let reader = Arc::new(MockReader::new());
        let mut engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let collected_data: Arc<Mutex<Vec<Vec<PointData>>>> = Arc::new(Mutex::new(Vec::new()));
        let data_clone = collected_data.clone();
        engine.set_data_callback(move |data| {
            data_clone.lock().unwrap().push(data);
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 50,
            max_points_per_cycle: 10,
            timeout_ms: 1000,
            max_retries: 1,
            retry_delay_ms: 10,
            enable_batch_reading: true, // Enable batch reading
            point_read_delay_ms: 1,
        };

        let points = vec![create_test_point("p1", 1), create_test_point("p2", 2)];

        engine.start_polling(config, points).await.unwrap();

        // Wait for some polling cycles
        sleep(Duration::from_millis(150)).await;

        engine.stop_polling().await.unwrap();

        // Check that data was collected
        let data = collected_data.lock().unwrap();
        assert!(!data.is_empty());
    }

    #[tokio::test]
    async fn test_universal_polling_engine_disconnected() {
        let reader = Arc::new(MockReader::new());
        reader.set_connected(false); // Simulate disconnection

        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let config = PollingConfig {
            enabled: true,
            interval_ms: 50,
            ..Default::default()
        };

        let points = vec![create_test_point("p1", 1)];

        engine.start_polling(config, points).await.unwrap();

        // Wait for some polling attempts
        sleep(Duration::from_millis(150)).await;

        engine.stop_polling().await.unwrap();

        // Statistics should show no successful cycles due to disconnection
        let stats = engine.get_polling_stats().await;
        assert_eq!(stats.successful_cycles, 0);
        assert_eq!(stats.total_points_read, 0);
    }

    #[tokio::test]
    async fn test_universal_polling_engine_config_updates() {
        let reader = Arc::new(MockReader::new());
        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let new_config = PollingConfig {
            enabled: true,
            interval_ms: 100,
            max_points_per_cycle: 50,
            timeout_ms: 2000,
            max_retries: 5,
            retry_delay_ms: 200,
            enable_batch_reading: false,
            point_read_delay_ms: 20,
        };

        engine.update_polling_config(new_config).await.unwrap();

        let new_points = vec![
            create_test_point("new_p1", 10),
            create_test_point("new_p2", 20),
            create_test_point("new_p3", 30),
        ];

        engine.update_polling_points(new_points).await.unwrap();
    }

    #[tokio::test]
    async fn test_universal_polling_engine_double_start() {
        let reader = Arc::new(MockReader::new());
        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let config = PollingConfig::default();
        let points = vec![create_test_point("p1", 1)];

        // First start should succeed
        engine
            .start_polling(config.clone(), points.clone())
            .await
            .unwrap();
        assert!(engine.is_polling_active().await);

        // Second start should fail
        let result = engine.start_polling(config, points).await;
        assert!(result.is_err());

        engine.stop_polling().await.unwrap();
    }

    #[tokio::test]
    async fn test_poll_interval_respected() {
        let reader: Arc<dyn PointReader> = Arc::new(MockReader::new());
        let mut engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let call_times: Arc<Mutex<Vec<Instant>>> = Arc::new(Mutex::new(Vec::new()));
        let times_clone = call_times.clone();
        engine.set_data_callback(move |_| {
            times_clone.lock().unwrap().push(Instant::now());
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 100,
            max_points_per_cycle: 1,
            timeout_ms: 100,
            max_retries: 0,
            retry_delay_ms: 0,
            enable_batch_reading: false,
            point_read_delay_ms: 0,
        };

        let point = create_test_point("p1", 1);

        engine.start_polling(config, vec![point]).await.unwrap();

        sleep(Duration::from_millis(250)).await;
        engine.stop_polling().await.unwrap();

        let times = call_times.lock().unwrap();
        assert!(times.len() >= 2);
        let diff = times[1].duration_since(times[0]);
        assert!(diff >= Duration::from_millis(100));
    }

    // Point reader trait test
    #[tokio::test]
    async fn test_point_reader_default_batch_read() {
        let reader = MockReader::new();

        let points = vec![create_test_point("p1", 1), create_test_point("p2", 2)];

        let results = reader.read_points_batch(&points).await.unwrap();
        assert_eq!(results.len(), 2);

        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.id, points[i].id);
            assert!(result.value.starts_with("value_"));
        }
    }

    #[tokio::test]
    async fn test_point_reader_batch_read_with_failures() {
        let reader = MockReader::new();
        reader.set_fail_reads(true);

        let points = vec![create_test_point("p1", 1)];
        let result = reader.read_points_batch(&points).await;
        assert!(result.is_err());
    }

    // Integration test for polling with various scenarios
    #[tokio::test]
    async fn test_polling_integration_scenarios() {
        let reader = Arc::new(MockReader::new());
        let mut engine = UniversalPollingEngine::new("Integration".to_string(), reader.clone());

        let all_data: Arc<Mutex<Vec<PointData>>> = Arc::new(Mutex::new(Vec::new()));
        let data_clone = all_data.clone();
        engine.set_data_callback(move |data| {
            data_clone.lock().unwrap().extend(data);
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 30,
            max_points_per_cycle: 5,
            timeout_ms: 500,
            max_retries: 2,
            retry_delay_ms: 50,
            enable_batch_reading: true,
            point_read_delay_ms: 5,
        };

        let points = vec![
            create_test_point("voltage", 40001),
            create_test_point("current", 40002),
            create_test_point("power", 40003),
        ];

        engine.start_polling(config, points).await.unwrap();

        // Phase 1: Normal operation
        sleep(Duration::from_millis(100)).await;

        // Phase 2: Simulate connection issues
        reader.set_connected(false);
        sleep(Duration::from_millis(80)).await;

        // Phase 3: Restore connection but with read errors
        reader.set_connected(true);
        reader.set_fail_reads(true);
        sleep(Duration::from_millis(80)).await;

        // Phase 4: Restore normal operation
        reader.set_fail_reads(false);
        sleep(Duration::from_millis(100)).await;

        engine.stop_polling().await.unwrap();

        // Verify we collected some data
        let data = all_data.lock().unwrap();
        assert!(!data.is_empty());

        // Verify statistics show mixed results
        let stats = engine.get_polling_stats().await;
        assert!(stats.total_cycles > 0);
        // Note: Testing that the engine handles the different scenarios
    }
}

/// 通用点位配置 - 所有协议共享的基础配置
/// Universal Point Configuration - Base configuration shared by all protocols
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UniversalPointConfig {
    /// 点位序号 - 全局唯一标识符
    pub point_index: u32,
    /// 点位名称
    pub point_name: String,
    /// 四遥类型分类
    pub telemetry_type: TelemetryType,
    /// 工程单位
    pub unit: String,
    /// 缩放系数
    pub scale: f64,
    /// 偏移量
    pub offset: f64,
    /// 访问模式 (read, write, read_write)
    pub access_mode: String,
    /// 四遥扩展元数据
    pub telemetry_metadata: Option<TelemetryMetadata>,
    /// 是否启用
    pub enabled: bool,
}

impl UniversalPointConfig {
    /// 创建新的通用点位配置
    pub fn new(
        point_index: u32,
        point_name: String,
        telemetry_type: TelemetryType,
        unit: String,
    ) -> Self {
        Self {
            point_index,
            point_name,
            telemetry_type,
            unit,
            scale: 1.0,
            offset: 0.0,
            access_mode: match telemetry_type {
                TelemetryType::Telemetry | TelemetryType::Signaling => "read".to_string(),
                TelemetryType::Control | TelemetryType::Setpoint => "read_write".to_string(),
            },
            telemetry_metadata: None,
            enabled: true,
        }
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<()> {
        if self.point_name.is_empty() {
            return Err(ComSrvError::ConfigError(
                "Point name cannot be empty".to_string(),
            ));
        }

        if self.scale == 0.0 {
            return Err(ComSrvError::ConfigError(
                "Scale factor cannot be zero".to_string(),
            ));
        }

        // 验证访问模式与四遥类型的一致性
        match self.telemetry_type {
            TelemetryType::Telemetry | TelemetryType::Signaling => {
                if self.access_mode != "read" {
                    return Err(ComSrvError::ConfigError(format!(
                        "Measurement and signaling points should be read-only, got: {}",
                        self.access_mode
                    )));
                }
            }
            TelemetryType::Control | TelemetryType::Setpoint => {
                if !["read_write", "write"].contains(&self.access_mode.as_str()) {
                    return Err(ComSrvError::ConfigError(format!(
                        "Control and regulation points should be writable, got: {}",
                        self.access_mode
                    )));
                }
            }
        }

        Ok(())
    }

    /// 应用工程单位转换
    pub fn apply_engineering_conversion(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }

    /// 反向工程单位转换（用于写入操作）
    pub fn reverse_engineering_conversion(&self, engineering_value: f64) -> f64 {
        (engineering_value - self.offset) / self.scale
    }

    /// 检查值是否在有效范围内
    pub fn is_value_in_range(&self, _value: f64) -> bool {
        // Protocol layer does not handle range validation
        true
    }
}

/// 通用点位配置管理器
/// Universal Point Configuration Manager
#[derive(Debug, Clone)]
pub struct UniversalPointManager {
    /// 点位配置映射表 (point_index -> config)
    points: HashMap<u32, UniversalPointConfig>,
    /// 名称到索引的映射 (point_name -> point_index)
    name_to_index: HashMap<String, u32>,
    /// 按四遥类型分组的点位索引
    points_by_type: HashMap<TelemetryType, Vec<u32>>,
}

impl UniversalPointManager {
    /// 创建新的点位管理器
    pub fn new() -> Self {
        Self {
            points: HashMap::new(),
            name_to_index: HashMap::new(),
            points_by_type: HashMap::new(),
        }
    }

    /// 添加点位配置
    pub fn add_point(&mut self, config: UniversalPointConfig) -> Result<()> {
        config.validate()?;

        // 检查点位索引是否已存在
        if self.points.contains_key(&config.point_index) {
            return Err(ComSrvError::ConfigError(format!(
                "Point index {} already exists",
                config.point_index
            )));
        }

        // 检查点位名称是否已存在
        if self.name_to_index.contains_key(&config.point_name) {
            return Err(ComSrvError::ConfigError(format!(
                "Point name '{}' already exists",
                config.point_name
            )));
        }

        let point_index = config.point_index;
        let point_name = config.point_name.clone();
        let telemetry_type = config.telemetry_type.clone();

        // 添加到主映射表
        self.points.insert(point_index, config);

        // 添加到名称映射表
        self.name_to_index.insert(point_name, point_index);

        // 添加到类型分组
        self.points_by_type
            .entry(telemetry_type)
            .or_insert_with(Vec::new)
            .push(point_index);

        Ok(())
    }

    /// 根据索引获取点位配置
    pub fn get_point_by_index(&self, point_index: u32) -> Option<&UniversalPointConfig> {
        self.points.get(&point_index)
    }

    /// 根据名称获取点位配置
    pub fn get_point_by_name(&self, point_name: &str) -> Option<&UniversalPointConfig> {
        self.name_to_index
            .get(point_name)
            .and_then(|index| self.points.get(index))
    }

    /// 获取指定四遥类型的所有点位
    pub fn get_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<&UniversalPointConfig> {
        self.points_by_type
            .get(telemetry_type)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|index| self.points.get(index))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 获取所有点位配置
    pub fn get_all_points(&self) -> Vec<&UniversalPointConfig> {
        self.points.values().collect()
    }

    /// 更新点位配置
    pub fn update_point(&mut self, point_index: u32, config: UniversalPointConfig) -> Result<()> {
        config.validate()?;

        if !self.points.contains_key(&point_index) {
            return Err(ComSrvError::ConfigError(format!(
                "Point index {} not found",
                point_index
            )));
        }

        // Protocol layer doesn't track timestamps

        // 如果名称发生变化，需要更新名称映射
        if let Some(old_config) = self.points.get(&point_index) {
            if old_config.point_name != config.point_name {
                // 检查新名称是否已存在
                if self.name_to_index.contains_key(&config.point_name) {
                    return Err(ComSrvError::ConfigError(format!(
                        "Point name '{}' already exists",
                        config.point_name
                    )));
                }

                // 移除旧名称映射
                self.name_to_index.remove(&old_config.point_name);
                // 添加新名称映射
                self.name_to_index
                    .insert(config.point_name.clone(), point_index);
            }

            // 如果四遥类型发生变化，需要更新类型分组
            if old_config.telemetry_type != config.telemetry_type {
                // 从旧类型分组中移除
                if let Some(old_group) = self.points_by_type.get_mut(&old_config.telemetry_type) {
                    old_group.retain(|&index| index != point_index);
                }
                // 添加到新类型分组
                self.points_by_type
                    .entry(config.telemetry_type.clone())
                    .or_insert_with(Vec::new)
                    .push(point_index);
            }
        }

        self.points.insert(point_index, config);
        Ok(())
    }

    /// 删除点位配置
    pub fn remove_point(&mut self, point_index: u32) -> Result<()> {
        if let Some(config) = self.points.remove(&point_index) {
            // 移除名称映射
            self.name_to_index.remove(&config.point_name);

            // 从类型分组中移除
            if let Some(group) = self.points_by_type.get_mut(&config.telemetry_type) {
                group.retain(|&index| index != point_index);
            }

            Ok(())
        } else {
            Err(ComSrvError::ConfigError(format!(
                "Point index {} not found",
                point_index
            )))
        }
    }

    /// 从YAML文件加载配置
    pub fn load_from_yaml<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to read config file: {}", e)))?;

        let configs: Vec<UniversalPointConfig> = serde_yaml::from_str(&content)
            .map_err(|e| ComSrvError::SerializationError(format!("Failed to parse YAML: {}", e)))?;

        let mut manager = Self::new();
        for config in configs {
            manager.add_point(config)?;
        }

        Ok(manager)
    }

    /// 保存配置到YAML文件
    pub fn save_to_yaml<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let mut configs: Vec<_> = self.points.values().collect();
        configs.sort_by_key(|config| config.point_index);

        let content = serde_yaml::to_string(&configs).map_err(|e| {
            ComSrvError::SerializationError(format!("Failed to serialize YAML: {}", e))
        })?;

        std::fs::write(path, content)
            .map_err(|e| ComSrvError::IoError(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> PointStatistics {
        let mut stats = PointStatistics::default();

        for config in self.points.values() {
            stats.total_points += 1;

            match config.telemetry_type {
                TelemetryType::Telemetry => stats.telemetry_points += 1,
                TelemetryType::Signaling => stats.signaling_points += 1,
                TelemetryType::Control => stats.control_points += 1,
                TelemetryType::Setpoint => stats.setpoint_points += 1,
            }

            if config.enabled {
                stats.enabled_points += 1;
            } else {
                stats.disabled_points += 1;
            }
        }

        stats
    }
}

/// 点位统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PointStatistics {
    /// 总点位数
    pub total_points: u32,
    /// 遥测点数
    pub telemetry_points: u32,
    /// 遥信点数
    pub signaling_points: u32,
    /// 遥控点数
    pub control_points: u32,
    /// 遥调点数
    pub setpoint_points: u32,
    /// 启用的点位数
    pub enabled_points: u32,
    /// 禁用的点位数
    pub disabled_points: u32,
}

/// 协议层原始数据类型 - 协议解析后的原始数据
/// Protocol Raw Data Types - Raw data after protocol parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolRawData {
    /// 十进制数值 (遥测、遥调使用)
    Decimal(f64),
    /// 二进制数值 (遥信、遥控使用)
    Binary(bool),
    /// 无效数据
    Invalid,
}

impl ProtocolRawData {
    /// 获取十进制值
    pub fn as_decimal(&self) -> Option<f64> {
        match self {
            ProtocolRawData::Decimal(val) => Some(*val),
            _ => None,
        }
    }

    /// 获取二进制值
    pub fn as_binary(&self) -> Option<bool> {
        match self {
            ProtocolRawData::Binary(val) => Some(*val),
            _ => None,
        }
    }

    /// 检查数据是否有效
    pub fn is_valid(&self) -> bool {
        !matches!(self, ProtocolRawData::Invalid)
    }
}

/// 协议数据解析接口 - 协议层实现
/// Protocol Data Parsing Interface - Implemented by protocol layer
#[async_trait]
pub trait ProtocolDataParser: Send + Sync {
    /// 解析遥测数据 - 返回十进制原始值
    async fn parse_measurement_data(
        &self,
        point_config: &(dyn std::any::Any + Send + Sync),
    ) -> Result<ProtocolRawData>;

    /// 解析遥信数据 - 返回二进制原始值
    async fn parse_signaling_data(
        &self,
        point_config: &(dyn std::any::Any + Send + Sync),
    ) -> Result<ProtocolRawData>;

    /// 解析遥控数据 - 返回二进制原始值
    async fn parse_control_data(
        &self,
        point_config: &(dyn std::any::Any + Send + Sync),
    ) -> Result<ProtocolRawData>;

    /// 解析遥调数据 - 返回十进制原始值
    async fn parse_regulation_data(
        &self,
        point_config: &(dyn std::any::Any + Send + Sync),
    ) -> Result<ProtocolRawData>;

    /// 写入遥控命令 - 接收二进制值
    async fn write_control_command(
        &self,
        point_config: &(dyn std::any::Any + Send + Sync),
        value: bool,
    ) -> Result<()>;

    /// 写入遥调设定值 - 接收十进制值
    async fn write_regulation_setpoint(
        &self,
        point_config: &(dyn std::any::Any + Send + Sync),
        value: f64,
    ) -> Result<()>;
}

/// 通用数据处理器 - 通用层实现业务逻辑
/// Universal Data Processor - Universal layer implements business logic
pub struct UniversalDataProcessor;

impl UniversalDataProcessor {
    /// Apply invert logic for signaling data (遥信反位)
    ///
    /// Applies invert configuration to digital signal data if enabled.
    /// This is used to reverse the logic of 0/1 signals based on configuration.
    ///
    /// # Arguments
    /// * `value` - Original boolean value
    /// * `metadata` - Telemetry metadata containing invert configuration
    ///
    /// # Returns
    /// Boolean value after applying invert logic if configured
    pub fn apply_signal_invert(&self, value: bool, metadata: &Option<TelemetryMetadata>) -> bool {
        if let Some(meta) = metadata {
            if let Some(true) = meta.invert_signal {
                return !value;
            }
        }
        value
    }

    /// Apply invert logic for control data (遥控反位)
    ///
    /// Applies invert configuration to control command data if enabled.
    /// This is used to reverse the logic of 0/1 control commands based on configuration.
    ///
    /// # Arguments
    /// * `value` - Original boolean command value
    /// * `metadata` - Telemetry metadata containing invert configuration
    ///
    /// # Returns
    /// Boolean command value after applying invert logic if configured
    pub fn apply_control_invert(&self, value: bool, metadata: &Option<TelemetryMetadata>) -> bool {
        if let Some(meta) = metadata {
            if let Some(true) = meta.invert_control {
                return !value;
            }
        }
        value
    }

    /// 处理遥测数据 - 从协议原始数据到工程值
    pub fn process_measurement_data(
        &self,
        raw_data: ProtocolRawData,
        config: &UniversalPointConfig,
    ) -> Result<MeasurementPoint> {
        let decimal_value = raw_data
            .as_decimal()
            .ok_or_else(|| ComSrvError::ConfigError("Invalid measurement data type".to_string()))?;

        // 工程单位转换
        let engineering_value = config.apply_engineering_conversion(decimal_value);

        // 报警判断
        // Protocol layer doesn't handle alarm status

        Ok(MeasurementPoint {
            value: engineering_value,
            unit: config.unit.clone(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// 处理遥信数据 - 从协议原始数据到状态描述
    pub fn process_signaling_data(
        &self,
        raw_data: ProtocolRawData,
        config: &UniversalPointConfig,
    ) -> Result<SignalingPoint> {
        let mut binary_value = raw_data
            .as_binary()
            .ok_or_else(|| ComSrvError::ConfigError("Invalid signaling data type".to_string()))?;

        // 应用遥信反位配置
        binary_value = self.apply_signal_invert(binary_value, &config.telemetry_metadata);

        // 状态描述
        let status_text = self.get_status_description(binary_value, config);

        // Protocol layer doesn't handle alarm status

        Ok(SignalingPoint {
            status: binary_value,
            status_text,
            timestamp: chrono::Utc::now(),
        })
    }

    /// 处理遥控数据 - 从协议原始数据到控制状态
    pub fn process_control_data(
        &self,
        raw_data: ProtocolRawData,
        config: &UniversalPointConfig,
    ) -> Result<ControlPoint> {
        let mut binary_value = raw_data
            .as_binary()
            .ok_or_else(|| ComSrvError::ConfigError("Invalid control data type".to_string()))?;

        // 应用遥控反位配置
        binary_value = self.apply_control_invert(binary_value, &config.telemetry_metadata);

        // 控制状态描述
        let command_text = self.get_command_description(binary_value, config);

        Ok(ControlPoint {
            current_state: binary_value,
            command_text,
            execution_status: ExecutionStatus::Completed,
            timestamp: chrono::Utc::now(),
        })
    }

    /// 处理遥调数据 - 从协议原始数据到调节值
    pub fn process_regulation_data(
        &self,
        raw_data: ProtocolRawData,
        config: &UniversalPointConfig,
    ) -> Result<RegulationPoint> {
        let decimal_value = raw_data
            .as_decimal()
            .ok_or_else(|| ComSrvError::ConfigError("Invalid regulation data type".to_string()))?;

        // 工程单位转换
        let engineering_value = config.apply_engineering_conversion(decimal_value);

        // 范围检查
        let in_range = self.check_regulation_range(engineering_value, config);

        Ok(RegulationPoint {
            current_value: engineering_value,
            unit: config.unit.clone(),
            in_range,
            timestamp: chrono::Utc::now(),
        })
    }

    /// 准备遥控命令 - 从业务逻辑值转换为协议原始值
    pub fn prepare_control_command(
        &self,
        command: bool,
        config: &UniversalPointConfig,
    ) -> Result<ProtocolRawData> {
        self.validate_control_command(command, config)?;

        // 应用遥控反位配置（对要发送的命令进行反位转换）
        let protocol_command = self.apply_control_invert(command, &config.telemetry_metadata);

        Ok(ProtocolRawData::Binary(protocol_command))
    }

    /// 准备遥调设定值 - 从工程值转换为协议原始值
    pub fn prepare_regulation_setpoint(
        &self,
        setpoint: f64,
        config: &UniversalPointConfig,
    ) -> Result<ProtocolRawData> {
        // 范围检查
        self.validate_regulation_setpoint(setpoint, config)?;

        // 反向工程单位转换：原始值 = (工程值 - offset) / scale
        let raw_value = (setpoint - config.offset) / config.scale;

        Ok(ProtocolRawData::Decimal(raw_value))
    }

    // 私有辅助方法

    fn get_status_description(&self, status: bool, config: &UniversalPointConfig) -> String {
        if let Some(metadata) = &config.telemetry_metadata {
            if status {
                metadata
                    .true_text
                    .clone()
                    .unwrap_or_else(|| "True".to_string())
            } else {
                metadata
                    .false_text
                    .clone()
                    .unwrap_or_else(|| "False".to_string())
            }
        } else {
            if status {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
    }

    fn get_command_description(&self, command: bool, config: &UniversalPointConfig) -> String {
        if let Some(metadata) = &config.telemetry_metadata {
            if command {
                metadata
                    .true_command
                    .clone()
                    .unwrap_or_else(|| "Execute".to_string())
            } else {
                metadata
                    .false_command
                    .clone()
                    .unwrap_or_else(|| "Stop".to_string())
            }
        } else {
            if command {
                "Execute".to_string()
            } else {
                "Stop".to_string()
            }
        }
    }

    fn check_regulation_range(&self, _value: f64, _config: &UniversalPointConfig) -> bool {
        // Protocol layer doesn't handle range validation
        true
    }

    fn validate_control_command(
        &self,
        _command: bool,
        _config: &UniversalPointConfig,
    ) -> Result<()> {
        Ok(())
    }

    fn validate_regulation_setpoint(
        &self,
        _setpoint: f64,
        _config: &UniversalPointConfig,
    ) -> Result<()> {
        // Protocol layer doesn't handle validation
        Ok(())
    }
}

/// 四遥点位标识符 - Four-Telemetry Point Identifier
/// 用于精确标识一个四遥点位
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct TelemetryPointId {
    /// 四遥类型
    pub telemetry_type: TelemetryType,
    /// 点位ID
    pub point_id: u32,
}

impl TelemetryPointId {
    /// 创建新的四遥点位标识符
    pub fn new(telemetry_type: TelemetryType, point_id: u32) -> Self {
        Self {
            telemetry_type,
            point_id,
        }
    }

    /// 转换为字符串表示
    pub fn to_string(&self) -> String {
        format!("{}:{}", self.telemetry_type.english_name(), self.point_id)
    }

    /// 从字符串解析四遥点位标识符
    pub fn from_string(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(ComSrvError::ConfigError(
                "Invalid point ID format, expected 'type:id'".to_string(),
            ));
        }

        let telemetry_type = match parts[0].to_lowercase().as_str() {
            "telemetry" => TelemetryType::Telemetry,
            "signaling" => TelemetryType::Signaling,
            "control" => TelemetryType::Control,
            "setpoint" => TelemetryType::Setpoint,
            _ => return Err(ComSrvError::ConfigError(
                format!("Unknown telemetry type: {}", parts[0])
            )),
        };

        let point_id = parts[1].parse::<u32>()
            .map_err(|_| ComSrvError::ConfigError(
                format!("Invalid point ID: {}", parts[1])
            ))?;

        Ok(Self {
            telemetry_type,
            point_id,
        })
    }
}

/// 转发计算配置 - Forward Calculation Configuration
/// 支持四遥类型+点位的精确定义和逻辑运算
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardCalculationConfig {
    /// 计算配置ID
    pub id: String,
    /// 计算名称
    pub name: String,
    /// 目标点位（四遥类型+点位ID）
    pub target_point: TelemetryPointId,
    /// 目标点位名称（可选，用于显示）
    pub target_point_name: Option<String>,
    /// 计算表达式（支持四则运算和逻辑运算）
    pub expression: String,
    /// 源点位映射（变量名 -> 四遥点位标识符）
    pub source_points: HashMap<String, TelemetryPointId>,
    /// 执行间隔（毫秒）
    pub execution_interval_ms: u64,
    /// 是否启用
    pub enabled: bool,
    /// 工程单位
    pub unit: String,
    /// 描述信息
    pub description: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,

    // 向后兼容字段
    /// 向后兼容：目标点位ID
    #[deprecated(note = "Use target_point instead")]
    pub target_point_id: Option<u32>,
    /// 向后兼容：计算结果的四遥类型
    #[deprecated(note = "Use target_point.telemetry_type instead")]
    pub target_telemetry_type: Option<TelemetryType>,
    /// 向后兼容：源点位映射（变量名 -> 点位ID）
    #[deprecated(note = "Use source_points instead")]
    pub legacy_source_points: Option<HashMap<String, u32>>,
}

impl ForwardCalculationConfig {
    /// 创建新的转发计算配置
    pub fn new(
        id: String,
        name: String,
        target_point: TelemetryPointId,
        expression: String,
        source_points: HashMap<String, TelemetryPointId>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            name,
            target_point,
            target_point_name: None,
            expression,
            source_points,
            execution_interval_ms: 1000,
            enabled: true,
            unit: String::new(),
            description: String::new(),
            created_at: now,
            updated_at: now,
            target_point_id: None,
            target_telemetry_type: None,
            legacy_source_points: None,
        }
    }

    /// 从旧格式转换
    pub fn from_legacy(
        id: String,
        name: String,
        target_point_id: u32,
        target_telemetry_type: TelemetryType,
        expression: String,
        legacy_source_points: HashMap<String, u32>,
    ) -> Self {
        let target_point = TelemetryPointId::new(target_telemetry_type, target_point_id);
        
        // 假设旧格式的源点位都是遥测类型
        let source_points = legacy_source_points
            .into_iter()
            .map(|(var, point_id)| (var, TelemetryPointId::new(TelemetryType::Telemetry, point_id)))
            .collect();

        let now = Utc::now();
        Self {
            id,
            name,
            target_point,
            target_point_name: None,
            expression,
            source_points,
            execution_interval_ms: 1000,
            enabled: true,
            unit: String::new(),
            description: String::new(),
            created_at: now,
            updated_at: now,
            target_point_id: Some(target_point_id),
            target_telemetry_type: Some(target_telemetry_type),
            legacy_source_points: None,
        }
    }
}

/// 转发计算结果 - Forward Calculation Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardCalculationResult {
    /// 计算配置ID
    pub config_id: String,
    /// 目标点位
    pub target_point: TelemetryPointId,
    /// 计算结果值（数值或布尔值）
    pub result_value: CalculationValue,
    /// 源点位数据
    pub source_data: HashMap<String, CalculationValue>,
    /// 计算执行时间
    pub execution_time: DateTime<Utc>,
    /// 计算状态
    pub status: CalculationStatus,
    /// 错误信息（如果有）
    pub error_message: Option<String>,
}

/// 计算值类型 - 支持数值和布尔值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CalculationValue {
    /// 数值（用于遥测、遥调）
    Numeric(f64),
    /// 布尔值（用于遥信、遥控）
    Boolean(bool),
}

impl CalculationValue {
    /// 转换为数值
    pub fn as_numeric(&self) -> Option<f64> {
        match self {
            CalculationValue::Numeric(value) => Some(*value),
            CalculationValue::Boolean(value) => Some(if *value { 1.0 } else { 0.0 }),
        }
    }

    /// 转换为布尔值
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            CalculationValue::Boolean(value) => Some(*value),
            CalculationValue::Numeric(value) => Some(*value != 0.0),
        }
    }

    /// 获取显示字符串
    pub fn to_display_string(&self) -> String {
        match self {
            CalculationValue::Numeric(value) => value.to_string(),
            CalculationValue::Boolean(value) => if *value { "1".to_string() } else { "0".to_string() },
        }
    }
}

/// 计算状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CalculationStatus {
    /// 成功
    Success,
    /// 源数据不完整
    IncompleteSourceData,
    /// 表达式错误
    ExpressionError,
    /// 计算错误
    CalculationError,
    /// 目标点位不存在
    TargetPointNotFound,
    /// 数据类型不匹配
    TypeMismatch,
}

/// 增强的表达式求值器 - Enhanced Expression Evaluator
/// 支持数学运算和逻辑运算
pub struct EnhancedExpressionEvaluator {
    /// 变量映射表
    variables: HashMap<String, CalculationValue>,
}

impl EnhancedExpressionEvaluator {
    /// 创建新的增强表达式求值器
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// 设置变量值
    pub fn set_variable(&mut self, name: &str, value: CalculationValue) {
        self.variables.insert(name.to_string(), value);
    }

    /// 批量设置变量
    pub fn set_variables(&mut self, vars: &HashMap<String, CalculationValue>) {
        for (name, value) in vars {
            self.variables.insert(name.clone(), value.clone());
        }
    }

    /// 计算表达式
    /// 支持:
    /// - 数学运算: +, -, *, /, ()
    /// - 逻辑运算: AND, OR, NOT
    /// - 比较运算: >, <, >=, <=, ==, !=
    pub fn evaluate(&self, expression: &str) -> Result<CalculationValue> {
        // 预处理：替换逻辑运算符为符号
        let expr = expression
            .replace(" AND ", " & ")
            .replace(" OR ", " | ")
            .replace(" NOT ", " ! ")
            .replace("AND", "&")
            .replace("OR", "|")
            .replace("NOT", "!");

        // 替换变量
        let expr = self.substitute_variables(&expr)?;
        
        // 解析并计算表达式
        self.parse_expression(&expr)
    }

    /// 替换表达式中的变量
    fn substitute_variables(&self, expression: &str) -> Result<String> {
        let mut result = expression.to_string();
        
        // 按变量名长度排序，避免短变量名覆盖长变量名
        let mut vars: Vec<_> = self.variables.iter().collect();
        vars.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        
        for (name, value) in vars {
            let replacement = match value {
                CalculationValue::Numeric(v) => v.to_string(),
                CalculationValue::Boolean(b) => if *b { "1".to_string() } else { "0".to_string() },
            };
            result = result.replace(name, &replacement);
        }
        
        Ok(result)
    }

    /// 解析并计算表达式
    fn parse_expression(&self, expr: &str) -> Result<CalculationValue> {
        let cleaned = expr.trim().replace(" ", "");
        self.parse_logical_or(&cleaned, &mut 0)
    }

    /// 解析逻辑或运算 (最低优先级)
    fn parse_logical_or(&self, expr: &str, pos: &mut usize) -> Result<CalculationValue> {
        let mut result = self.parse_logical_and(expr, pos)?;
        
        while *pos < expr.len() {
            if expr.chars().nth(*pos) == Some('|') {
                *pos += 1;
                let right = self.parse_logical_and(expr, pos)?;
                result = CalculationValue::Boolean(
                    result.as_boolean().unwrap_or(false) || right.as_boolean().unwrap_or(false)
                );
            } else {
                break;
            }
        }
        
        Ok(result)
    }

    /// 解析逻辑与运算
    fn parse_logical_and(&self, expr: &str, pos: &mut usize) -> Result<CalculationValue> {
        let mut result = self.parse_comparison(expr, pos)?;
        
        while *pos < expr.len() {
            if expr.chars().nth(*pos) == Some('&') {
                *pos += 1;
                let right = self.parse_comparison(expr, pos)?;
                result = CalculationValue::Boolean(
                    result.as_boolean().unwrap_or(false) && right.as_boolean().unwrap_or(false)
                );
            } else {
                break;
            }
        }
        
        Ok(result)
    }

    /// 解析比较运算
    fn parse_comparison(&self, expr: &str, pos: &mut usize) -> Result<CalculationValue> {
        let mut result = self.parse_addition(expr, pos)?;
        
        while *pos < expr.len() {
            let chars: Vec<char> = expr.chars().collect();
            if *pos + 1 < chars.len() {
                let op = format!("{}{}", chars[*pos], chars[*pos + 1]);
                match op.as_str() {
                    ">=" => {
                        *pos += 2;
                        let right = self.parse_addition(expr, pos)?;
                        result = CalculationValue::Boolean(
                            result.as_numeric().unwrap_or(0.0) >= right.as_numeric().unwrap_or(0.0)
                        );
                    }
                    "<=" => {
                        *pos += 2;
                        let right = self.parse_addition(expr, pos)?;
                        result = CalculationValue::Boolean(
                            result.as_numeric().unwrap_or(0.0) <= right.as_numeric().unwrap_or(0.0)
                        );
                    }
                    "==" => {
                        *pos += 2;
                        let right = self.parse_addition(expr, pos)?;
                        result = CalculationValue::Boolean(
                            (result.as_numeric().unwrap_or(0.0) - right.as_numeric().unwrap_or(0.0)).abs() < f64::EPSILON
                        );
                    }
                    "!=" => {
                        *pos += 2;
                        let right = self.parse_addition(expr, pos)?;
                        result = CalculationValue::Boolean(
                            (result.as_numeric().unwrap_or(0.0) - right.as_numeric().unwrap_or(0.0)).abs() >= f64::EPSILON
                        );
                    }
                    _ => {
                        if chars[*pos] == '>' {
                            *pos += 1;
                            let right = self.parse_addition(expr, pos)?;
                            result = CalculationValue::Boolean(
                                result.as_numeric().unwrap_or(0.0) > right.as_numeric().unwrap_or(0.0)
                            );
                        } else if chars[*pos] == '<' {
                            *pos += 1;
                            let right = self.parse_addition(expr, pos)?;
                            result = CalculationValue::Boolean(
                                result.as_numeric().unwrap_or(0.0) < right.as_numeric().unwrap_or(0.0)
                            );
                        } else {
                            break;
                        }
                    }
                }
            } else if *pos < chars.len() {
                if chars[*pos] == '>' {
                    *pos += 1;
                    let right = self.parse_addition(expr, pos)?;
                    result = CalculationValue::Boolean(
                        result.as_numeric().unwrap_or(0.0) > right.as_numeric().unwrap_or(0.0)
                    );
                } else if chars[*pos] == '<' {
                    *pos += 1;
                    let right = self.parse_addition(expr, pos)?;
                    result = CalculationValue::Boolean(
                        result.as_numeric().unwrap_or(0.0) < right.as_numeric().unwrap_or(0.0)
                    );
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        
        Ok(result)
    }

    /// 解析加法和减法
    fn parse_addition(&self, expr: &str, pos: &mut usize) -> Result<CalculationValue> {
        let mut result = self.parse_multiplication(expr, pos)?;
        
        while *pos < expr.len() {
            let ch = expr.chars().nth(*pos).unwrap();
            if ch == '+' {
                *pos += 1;
                let right = self.parse_multiplication(expr, pos)?;
                if let (Some(l), Some(r)) = (result.as_numeric(), right.as_numeric()) {
                    result = CalculationValue::Numeric(l + r);
                } else {
                    return Err(ComSrvError::ConfigError("Type mismatch in addition".to_string()));
                }
            } else if ch == '-' {
                *pos += 1;
                let right = self.parse_multiplication(expr, pos)?;
                if let (Some(l), Some(r)) = (result.as_numeric(), right.as_numeric()) {
                    result = CalculationValue::Numeric(l - r);
                } else {
                    return Err(ComSrvError::ConfigError("Type mismatch in subtraction".to_string()));
                }
            } else {
                break;
            }
        }
        
        Ok(result)
    }

    /// 解析乘法和除法
    fn parse_multiplication(&self, expr: &str, pos: &mut usize) -> Result<CalculationValue> {
        let mut result = self.parse_factor(expr, pos)?;
        
        while *pos < expr.len() {
            let ch = expr.chars().nth(*pos).unwrap();
            if ch == '*' {
                *pos += 1;
                let right = self.parse_factor(expr, pos)?;
                if let (Some(l), Some(r)) = (result.as_numeric(), right.as_numeric()) {
                    result = CalculationValue::Numeric(l * r);
                } else {
                    return Err(ComSrvError::ConfigError("Type mismatch in multiplication".to_string()));
                }
            } else if ch == '/' {
                *pos += 1;
                let right = self.parse_factor(expr, pos)?;
                if let (Some(l), Some(r)) = (result.as_numeric(), right.as_numeric()) {
                    if r == 0.0 {
                        return Err(ComSrvError::ConfigError("Division by zero".to_string()));
                    }
                    result = CalculationValue::Numeric(l / r);
                } else {
                    return Err(ComSrvError::ConfigError("Type mismatch in division".to_string()));
                }
            } else {
                break;
            }
        }
        
        Ok(result)
    }

    /// 解析因子（数字、布尔值、括号表达式、逻辑非）
    fn parse_factor(&self, expr: &str, pos: &mut usize) -> Result<CalculationValue> {
        if *pos >= expr.len() {
            return Err(ComSrvError::ConfigError("Unexpected end of expression".to_string()));
        }
        
        let ch = expr.chars().nth(*pos).unwrap();
        
        if ch == '(' {
            *pos += 1; // 跳过 '('
            let result = self.parse_logical_or(expr, pos)?;
            if *pos >= expr.len() || expr.chars().nth(*pos).unwrap() != ')' {
                return Err(ComSrvError::ConfigError("Missing closing parenthesis".to_string()));
            }
            *pos += 1; // 跳过 ')'
            Ok(result)
        } else if ch == '-' {
            *pos += 1;
            let factor = self.parse_factor(expr, pos)?;
            if let Some(value) = factor.as_numeric() {
                Ok(CalculationValue::Numeric(-value))
            } else {
                Err(ComSrvError::ConfigError("Cannot apply negative to boolean".to_string()))
            }
        } else if ch == '+' {
            *pos += 1;
            self.parse_factor(expr, pos)
        } else if ch == '!' {
            *pos += 1;
            let factor = self.parse_factor(expr, pos)?;
            Ok(CalculationValue::Boolean(!factor.as_boolean().unwrap_or(false)))
        } else {
            self.parse_number_or_boolean(expr, pos)
        }
    }

    /// 解析数字或布尔值
    fn parse_number_or_boolean(&self, expr: &str, pos: &mut usize) -> Result<CalculationValue> {
        let start = *pos;
        let chars: Vec<char> = expr.chars().collect();
        
        // 检查是否是布尔值
        if start + 4 <= chars.len() && &expr[start..start+4] == "true" {
            *pos += 4;
            return Ok(CalculationValue::Boolean(true));
        }
        if start + 5 <= chars.len() && &expr[start..start+5] == "false" {
            *pos += 5;
            return Ok(CalculationValue::Boolean(false));
        }
        
        // 解析数字
        while *pos < chars.len() && (chars[*pos].is_ascii_digit() || chars[*pos] == '.') {
            *pos += 1;
        }
        
        if start == *pos {
            return Err(ComSrvError::ConfigError("Expected number or boolean".to_string()));
        }
        
        let number_str = &expr[start..*pos];
        let value = number_str.parse::<f64>()
            .map_err(|_| ComSrvError::ConfigError(format!("Invalid number: {}", number_str)))?;
        
        Ok(CalculationValue::Numeric(value))
    }
}

/// 转发计算引擎 - Forward Calculation Engine
/// 负责管理和执行转发计算任务
pub struct ForwardCalculationEngine {
    /// 计算配置列表
    configs: HashMap<String, ForwardCalculationConfig>,
    /// 表达式求值器
    evaluator: EnhancedExpressionEvaluator,
    /// 计算结果历史
    results_history: Vec<ForwardCalculationResult>,
    /// 最大历史记录数
    max_history_size: usize,
}

impl ForwardCalculationEngine {
    /// 创建新的转发计算引擎
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            evaluator: EnhancedExpressionEvaluator::new(),
            results_history: Vec::new(),
            max_history_size: 1000,
        }
    }

    /// 添加计算配置
    pub fn add_config(&mut self, config: ForwardCalculationConfig) -> Result<()> {
        // 验证配置
        self.validate_config(&config)?;
        
        self.configs.insert(config.id.clone(), config);
        Ok(())
    }

    /// 移除计算配置
    pub fn remove_config(&mut self, config_id: &str) -> bool {
        self.configs.remove(config_id).is_some()
    }

    /// 获取计算配置
    pub fn get_config(&self, config_id: &str) -> Option<&ForwardCalculationConfig> {
        self.configs.get(config_id)
    }

    /// 获取所有配置
    pub fn get_all_configs(&self) -> Vec<&ForwardCalculationConfig> {
        self.configs.values().collect()
    }

    /// 验证配置有效性
    fn validate_config(&self, config: &ForwardCalculationConfig) -> Result<()> {
        if config.expression.trim().is_empty() {
            return Err(ComSrvError::ConfigError("Expression cannot be empty".to_string()));
        }

        if config.source_points.is_empty() {
            return Err(ComSrvError::ConfigError("Source points cannot be empty".to_string()));
        }

        if config.execution_interval_ms == 0 {
            return Err(ComSrvError::ConfigError("Execution interval must be greater than 0".to_string()));
        }

        // 验证表达式语法（简单检查）
        let mut test_evaluator = EnhancedExpressionEvaluator::new();
        for var_name in config.source_points.keys() {
            test_evaluator.set_variable(var_name, CalculationValue::Numeric(1.0)); // 使用测试值
        }

        match test_evaluator.evaluate(&config.expression) {
            Ok(_) => Ok(()),
            Err(e) => Err(ComSrvError::ConfigError(format!(
                "Invalid expression '{}': {}",
                config.expression, e
            ))),
        }
    }

    /// 执行转发计算
    pub async fn execute_calculation(
        &mut self,
        config_id: &str,
        source_data: &HashMap<String, CalculationValue>,
    ) -> Result<ForwardCalculationResult> {
        let config = self.configs.get(config_id)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Config not found: {}", config_id)))?
            .clone();

        if !config.enabled {
            return Err(ComSrvError::ConfigError(format!("Config is disabled: {}", config_id)));
        }

        let execution_time = Utc::now();
        
        // 检查源数据完整性
        let mut missing_sources = Vec::new();
        let mut available_data = HashMap::new();
        
        for (var_name, point_id) in &config.source_points {
            if let Some(value) = source_data.get(&point_id.to_string()) {
                available_data.insert(var_name.clone(), value.clone());
            } else {
                missing_sources.push(var_name.clone());
            }
        }

        if !missing_sources.is_empty() {
            return Ok(ForwardCalculationResult {
                config_id: config_id.to_string(),
                target_point: config.target_point,
                result_value: CalculationValue::Numeric(0.0),
                source_data: available_data,
                execution_time,
                status: CalculationStatus::IncompleteSourceData,
                error_message: Some(format!("Missing source data for variables: {:?}", missing_sources)),
            });
        }

        // 设置变量并执行计算
        self.evaluator.set_variables(&available_data);
        
        match self.evaluator.evaluate(&config.expression) {
            Ok(result_value) => {
                let result = ForwardCalculationResult {
                    config_id: config_id.to_string(),
                    target_point: config.target_point,
                    result_value,
                    source_data: available_data,
                    execution_time,
                    status: CalculationStatus::Success,
                    error_message: None,
                };

                // 添加到历史记录
                self.add_to_history(result.clone());
                
                Ok(result)
            }
            Err(e) => {
                Ok(ForwardCalculationResult {
                    config_id: config_id.to_string(),
                    target_point: config.target_point,
                    result_value: CalculationValue::Numeric(0.0),
                    source_data: available_data,
                    execution_time,
                    status: CalculationStatus::CalculationError,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }

    /// 批量执行所有启用的计算配置
    pub async fn execute_all_calculations(
        &mut self,
        all_source_data: &HashMap<String, CalculationValue>,
    ) -> Vec<ForwardCalculationResult> {
        let mut results = Vec::new();
        
        for config_id in self.configs.keys().cloned().collect::<Vec<_>>() {
            if let Ok(result) = self.execute_calculation(&config_id, all_source_data).await {
                results.push(result);
            }
        }
        
        results
    }

    /// 添加计算结果到历史记录
    fn add_to_history(&mut self, result: ForwardCalculationResult) {
        self.results_history.push(result);
        
        // 限制历史记录大小
        if self.results_history.len() > self.max_history_size {
            self.results_history.remove(0);
        }
    }

    /// 获取计算历史
    pub fn get_calculation_history(&self, config_id: Option<&str>) -> Vec<&ForwardCalculationResult> {
        match config_id {
            Some(id) => self.results_history.iter()
                .filter(|r| r.config_id == id)
                .collect(),
            None => self.results_history.iter().collect(),
        }
    }

    /// 清空历史记录
    pub fn clear_history(&mut self) {
        self.results_history.clear();
    }
}

impl Default for ForwardCalculationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 转发计算管理器特性 - Forward Calculation Manager Trait
/// 集成到通用数据处理器中
#[async_trait]
pub trait ForwardCalculationManager: Send + Sync {
    /// 添加转发计算配置
    async fn add_forward_calculation(&mut self, config: ForwardCalculationConfig) -> Result<()>;
    
    /// 移除转发计算配置
    async fn remove_forward_calculation(&mut self, config_id: &str) -> Result<bool>;
    
    /// 获取转发计算配置
    async fn get_forward_calculation(&self, config_id: &str) -> Result<Option<ForwardCalculationConfig>>;
    
    /// 执行转发计算
    async fn execute_forward_calculations(
        &mut self,
        source_data: &HashMap<String, CalculationValue>,
    ) -> Result<Vec<ForwardCalculationResult>>;
    
    /// 获取计算历史
    async fn get_calculation_history(
        &self,
        config_id: Option<&str>,
    ) -> Result<Vec<ForwardCalculationResult>>;
}

#[cfg(test)]
mod legacy_forward_calculation_tests {
    use super::*;

    #[test]
    fn test_expression_evaluator_basic_arithmetic() {
        let evaluator = EnhancedExpressionEvaluator::new();
        
        // 测试基本四则运算
        assert_eq!(evaluator.evaluate("2 + 3").unwrap().as_numeric(), Some(5.0));
        assert_eq!(evaluator.evaluate("10 - 4").unwrap().as_numeric(), Some(6.0));
        assert_eq!(evaluator.evaluate("3 * 4").unwrap().as_numeric(), Some(12.0));
        assert_eq!(evaluator.evaluate("15 / 3").unwrap().as_numeric(), Some(5.0));
    }

    #[test]
    fn test_expression_evaluator_precedence() {
        let evaluator = EnhancedExpressionEvaluator::new();
        
        // 测试运算优先级
        assert_eq!(evaluator.evaluate("2 + 3 * 4").unwrap().as_numeric(), Some(14.0));
        assert_eq!(evaluator.evaluate("(2 + 3) * 4").unwrap().as_numeric(), Some(20.0));
        assert_eq!(evaluator.evaluate("10 - 2 * 3").unwrap().as_numeric(), Some(4.0));
        assert_eq!(evaluator.evaluate("(10 - 2) * 3").unwrap().as_numeric(), Some(24.0));
    }

    #[test]
    fn test_expression_evaluator_variables() {
        let mut evaluator = EnhancedExpressionEvaluator::new();
        evaluator.set_variable("x", CalculationValue::Numeric(5.0));
        evaluator.set_variable("y", CalculationValue::Numeric(3.0));
        
        assert_eq!(evaluator.evaluate("x + y").unwrap().as_numeric(), Some(8.0));
        assert_eq!(evaluator.evaluate("x * y + 2").unwrap().as_numeric(), Some(17.0));
        assert_eq!(evaluator.evaluate("(x + y) / 2").unwrap().as_numeric(), Some(4.0));
    }

    #[test]
    fn test_forward_calculation_config_validation() {
        let config = ForwardCalculationConfig {
            id: "test_calc".to_string(),
            name: "Test Calculation".to_string(),
            target_point: TelemetryPointId::new(TelemetryType::Telemetry, 5000),
            target_point_name: None,
            expression: "temp1 + temp2".to_string(),
            source_points: {
                let mut map = HashMap::new();
                map.insert("temp1".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1001));
                map.insert("temp2".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1002));
                map
            },
            execution_interval_ms: 1000,
            enabled: true,
            unit: "°C".to_string(),
            description: "Temperature sum calculation".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),s
        };

        let engine = EnhancedForwardCalculationEngine::new();
        assert!(engine.validate_config(&config).is_ok());
    }

    #[tokio::test]
    async fn test_forward_calculation_execution() {
        let mut engine = EnhancedForwardCalculationEngine::new();
        
        let config = ForwardCalculationConfig {
            id: "test_calc".to_string(),
            name: "Test Calculation".to_string(),
            target_point: TelemetryPointId::new(TelemetryType::Telemetry, 5000),
            target_point_name: None,
            expression: "temp1 * 2 + temp2".to_string(),
            source_points: {
                let mut map = HashMap::new();
                map.insert("temp1".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1001));
                map.insert("temp2".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1002));
                map
            },
            execution_interval_ms: 1000,
            enabled: true,
            unit: "°C".to_string(),
            description: "Temperature calculation".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        engine.add_config(config).unwrap();

        let mut source_data = HashMap::new();
        source_data.insert("telemetry:1001".to_string(), CalculationValue::Numeric(25.0)); // temp1
        source_data.insert("telemetry:1002".to_string(), CalculationValue::Numeric(5.0));  // temp2

        let result = engine.execute_calculation("test_calc", &source_data).await.unwrap();
        
        assert_eq!(result.status, CalculationStatus::Success);
        assert_eq!(result.result_value.as_numeric(), Some(55.0)); // 25 * 2 + 5 = 55
    }
}

/// 增强的转发计算引擎实现
/// Enhanced Forward Calculation Engine Implementation
pub struct EnhancedForwardCalculationEngine {
    /// 计算配置列表
    configs: HashMap<String, ForwardCalculationConfig>,
    /// 增强表达式求值器
    evaluator: EnhancedExpressionEvaluator,
    /// 计算结果历史
    results_history: Vec<ForwardCalculationResult>,
    /// 最大历史记录数
    max_history_size: usize,
}

impl EnhancedForwardCalculationEngine {
    /// 创建新的增强转发计算引擎
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            evaluator: EnhancedExpressionEvaluator::new(),
            results_history: Vec::new(),
            max_history_size: 1000,
        }
    }

    /// 添加配置
    pub fn add_config(&mut self, config: ForwardCalculationConfig) -> Result<()> {
        self.validate_config(&config)?;
        self.configs.insert(config.id.clone(), config);
        Ok(())
    }

    /// 移除配置
    pub fn remove_config(&mut self, config_id: &str) -> bool {
        self.configs.remove(config_id).is_some()
    }

    /// 获取配置
    pub fn get_config(&self, config_id: &str) -> Option<&ForwardCalculationConfig> {
        self.configs.get(config_id)
    }

    /// 获取所有配置
    pub fn get_all_configs(&self) -> Vec<&ForwardCalculationConfig> {
        self.configs.values().collect()
    }

    /// 验证配置
    fn validate_config(&self, config: &ForwardCalculationConfig) -> Result<()> {
        // 验证ID不为空
        if config.id.is_empty() {
            return Err(ComSrvError::ConfigError("Configuration ID cannot be empty".to_string()));
        }

        // 验证表达式不为空
        if config.expression.is_empty() {
            return Err(ComSrvError::ConfigError("Expression cannot be empty".to_string()));
        }

        // 验证源点位不为空
        if config.source_points.is_empty() {
            return Err(ComSrvError::ConfigError("Source points cannot be empty".to_string()));
        }

        // 验证表达式语法（简单检查）
        let mut test_evaluator = EnhancedExpressionEvaluator::new();
        for var_name in config.source_points.keys() {
            // 根据目标点位类型设置合适的测试值
            let test_value = match config.target_point.telemetry_type {
                TelemetryType::Signaling | TelemetryType::Control => CalculationValue::Boolean(true),
                TelemetryType::Telemetry | TelemetryType::Setpoint => CalculationValue::Numeric(1.0),
            };
            test_evaluator.set_variable(var_name, test_value);
        }

        test_evaluator.evaluate(&config.expression)
            .map_err(|e| ComSrvError::ConfigError(format!("Invalid expression: {}", e)))?;

        Ok(())
    }

    /// 执行单个计算
    pub async fn execute_calculation(
        &mut self,
        config_id: &str,
        source_data: &HashMap<String, CalculationValue>,
    ) -> Result<ForwardCalculationResult> {
        let config = self.configs.get(config_id)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Configuration not found: {}", config_id)))?;

        let execution_time = Utc::now();

        // 收集可用的源数据
        let mut available_data = HashMap::new();
        let mut missing_points = Vec::new();

        for (var_name, point_id) in &config.source_points {
            let key = point_id.to_string();
            if let Some(value) = source_data.get(&key) {
                available_data.insert(var_name.clone(), value.clone());
            } else {
                missing_points.push(var_name.clone());
            }
        }

        // 如果有缺失的源数据
        if !missing_points.is_empty() {
            return Ok(ForwardCalculationResult {
                config_id: config_id.to_string(),
                target_point: config.target_point.clone(),
                result_value: match config.target_point.telemetry_type {
                    TelemetryType::Signaling | TelemetryType::Control => CalculationValue::Boolean(false),
                    TelemetryType::Telemetry | TelemetryType::Setpoint => CalculationValue::Numeric(0.0),
                },
                source_data: available_data,
                execution_time,
                status: CalculationStatus::IncompleteSourceData,
                error_message: Some(format!("Missing source data for variables: {}", missing_points.join(", "))),
            });
        }

        // 执行计算
        self.evaluator.set_variables(&available_data);
        match self.evaluator.evaluate(&config.expression) {
            Ok(result_value) => {
                // 验证结果类型是否与目标点位类型匹配
                let type_match = match (&result_value, &config.target_point.telemetry_type) {
                    (CalculationValue::Boolean(_), TelemetryType::Signaling | TelemetryType::Control) => true,
                    (CalculationValue::Numeric(_), TelemetryType::Telemetry | TelemetryType::Setpoint) => true,
                    _ => false,
                };

                if !type_match {
                    return Ok(ForwardCalculationResult {
                        config_id: config_id.to_string(),
                        target_point: config.target_point.clone(),
                        result_value: match config.target_point.telemetry_type {
                            TelemetryType::Signaling | TelemetryType::Control => CalculationValue::Boolean(false),
                            TelemetryType::Telemetry | TelemetryType::Setpoint => CalculationValue::Numeric(0.0),
                        },
                        source_data: available_data,
                        execution_time,
                        status: CalculationStatus::TypeMismatch,
                                                                          error_message: Some(format!("Result type mismatch: expected {:?}, got {:?}", 
                             config.target_point.telemetry_type, result_value)),
                    });
                }

                let result = ForwardCalculationResult {
                    config_id: config_id.to_string(),
                    target_point: config.target_point.clone(),
                    result_value,
                    source_data: available_data,
                    execution_time,
                    status: CalculationStatus::Success,
                    error_message: None,
                };

                self.add_to_history(result.clone());
                Ok(result)
            }
            Err(e) => {
                Ok(ForwardCalculationResult {
                    config_id: config_id.to_string(),
                    target_point: config.target_point.clone(),
                    result_value: match config.target_point.telemetry_type {
                        TelemetryType::Signaling | TelemetryType::Control => CalculationValue::Boolean(false),
                        TelemetryType::Telemetry | TelemetryType::Setpoint => CalculationValue::Numeric(0.0),
                    },
                    source_data: available_data,
                    execution_time,
                    status: CalculationStatus::ExpressionError,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }

    /// 批量执行所有计算
    pub async fn execute_all_calculations(
        &mut self,
        all_source_data: &HashMap<String, CalculationValue>,
    ) -> Vec<ForwardCalculationResult> {
        let mut results = Vec::new();
        
        for config_id in self.configs.keys().cloned().collect::<Vec<_>>() {
            match self.execute_calculation(&config_id, all_source_data).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("Failed to execute calculation {}: {}", config_id, e);
                }
            }
        }
        
        results
    }

    /// 添加到历史记录
    fn add_to_history(&mut self, result: ForwardCalculationResult) {
        self.results_history.push(result);
        
        // 保持历史记录大小限制
        if self.results_history.len() > self.max_history_size {
            self.results_history.remove(0);
        }
    }

    /// 获取计算历史
    pub fn get_calculation_history(&self, config_id: Option<&str>) -> Vec<&ForwardCalculationResult> {
        match config_id {
            Some(id) => self.results_history.iter().filter(|r| r.config_id == id).collect(),
            None => self.results_history.iter().collect(),
        }
    }

    /// 清空历史记录
    pub fn clear_history(&mut self) {
        self.results_history.clear();
    }

    /// 创建逻辑运算示例配置
    pub fn create_logic_example_configs() -> Vec<ForwardCalculationConfig> {
        vec![
            // 与运算示例：两个开关都打开时，设备启动
            ForwardCalculationConfig::new(
                "device_start_logic".to_string(),
                "Device Start Logic".to_string(),
                TelemetryPointId::new(TelemetryType::Control, 3001),
                "switch1 AND switch2".to_string(),
                {
                    let mut map = HashMap::new();
                    map.insert("switch1".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1001));
                    map.insert("switch2".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1002));
                    map
                },
            ),
            
            // 或运算示例：任一报警触发时，总报警激活
            ForwardCalculationConfig::new(
                "alarm_or_logic".to_string(),
                "Alarm OR Logic".to_string(),
                TelemetryPointId::new(TelemetryType::Signaling, 2001),
                "temp_alarm OR pressure_alarm OR flow_alarm".to_string(),
                {
                    let mut map = HashMap::new();
                    map.insert("temp_alarm".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1010));
                    map.insert("pressure_alarm".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1011));
                    map.insert("flow_alarm".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1012));
                    map
                },
            ),
            
            // 非运算示例：设备停止信号的反向逻辑
            ForwardCalculationConfig::new(
                "device_running_status".to_string(),
                "Device Running Status".to_string(),
                TelemetryPointId::new(TelemetryType::Signaling, 2002),
                "NOT stop_signal".to_string(),
                {
                    let mut map = HashMap::new();
                    map.insert("stop_signal".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1020));
                    map
                },
            ),
            
            // 复合逻辑运算示例：复杂的设备控制逻辑
            ForwardCalculationConfig::new(
                "complex_control_logic".to_string(),
                "Complex Control Logic".to_string(),
                TelemetryPointId::new(TelemetryType::Control, 3002),
                "(manual_mode OR (auto_mode AND temp > 50)) AND NOT emergency_stop".to_string(),
                {
                    let mut map = HashMap::new();
                    map.insert("manual_mode".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1030));
                    map.insert("auto_mode".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1031));
                    map.insert("temp".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1032));
                    map.insert("emergency_stop".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1033));
                    map
                },
            ),
            
            // 数值计算转换为逻辑信号示例
            ForwardCalculationConfig::new(
                "temperature_high_alarm".to_string(),
                "Temperature High Alarm".to_string(),
                TelemetryPointId::new(TelemetryType::Signaling, 2003),
                "temperature > 80".to_string(),
                {
                    let mut map = HashMap::new();
                    map.insert("temperature".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1040));
                    map
                },
            ),
        ]
    }
}

impl Default for EnhancedForwardCalculationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ForwardCalculationManager for EnhancedForwardCalculationEngine {
    async fn add_forward_calculation(&mut self, config: ForwardCalculationConfig) -> Result<()> {
        self.add_config(config)
    }

    async fn remove_forward_calculation(&mut self, config_id: &str) -> Result<bool> {
        Ok(self.remove_config(config_id))
    }

    async fn get_forward_calculation(&self, config_id: &str) -> Result<Option<ForwardCalculationConfig>> {
        Ok(self.get_config(config_id).cloned())
    }

    async fn execute_forward_calculations(
        &mut self,
        source_data: &HashMap<String, CalculationValue>,
    ) -> Result<Vec<ForwardCalculationResult>> {
        Ok(self.execute_all_calculations(source_data).await)
    }

    async fn get_calculation_history(
        &self,
        config_id: Option<&str>,
    ) -> Result<Vec<ForwardCalculationResult>> {
        Ok(self.get_calculation_history(config_id).into_iter().cloned().collect())
    }
}

#[cfg(test)]
mod forward_calculation_tests {
    use super::*;

    #[test]
    fn test_enhanced_expression_evaluator_logical_operations() {
        let mut evaluator = EnhancedExpressionEvaluator::new();
        
        // 设置布尔变量
        evaluator.set_variable("a", CalculationValue::Boolean(true));
        evaluator.set_variable("b", CalculationValue::Boolean(false));
        
        // 测试逻辑与运算
        assert_eq!(evaluator.evaluate("a AND b").unwrap().as_boolean(), Some(false));
        assert_eq!(evaluator.evaluate("a AND true").unwrap().as_boolean(), Some(true));
        
        // 测试逻辑或运算
        assert_eq!(evaluator.evaluate("a OR b").unwrap().as_boolean(), Some(true));
        assert_eq!(evaluator.evaluate("false OR false").unwrap().as_boolean(), Some(false));
        
        // 测试逻辑非运算
        assert_eq!(evaluator.evaluate("NOT a").unwrap().as_boolean(), Some(false));
        assert_eq!(evaluator.evaluate("NOT b").unwrap().as_boolean(), Some(true));
    }

    #[test]
    fn test_enhanced_expression_evaluator_comparison_operations() {
        let mut evaluator = EnhancedExpressionEvaluator::new();
        
        // 设置数值变量
        evaluator.set_variable("x", CalculationValue::Numeric(10.0));
        evaluator.set_variable("y", CalculationValue::Numeric(5.0));
        
        // 测试比较运算
        assert_eq!(evaluator.evaluate("x > y").unwrap().as_boolean(), Some(true));
        assert_eq!(evaluator.evaluate("x < y").unwrap().as_boolean(), Some(false));
        assert_eq!(evaluator.evaluate("x >= 10").unwrap().as_boolean(), Some(true));
        assert_eq!(evaluator.evaluate("y <= 5").unwrap().as_boolean(), Some(true));
        assert_eq!(evaluator.evaluate("x == 10").unwrap().as_boolean(), Some(true));
        assert_eq!(evaluator.evaluate("x != y").unwrap().as_boolean(), Some(true));
    }

    #[test]
    fn test_enhanced_expression_evaluator_complex_expressions() {
        let mut evaluator = EnhancedExpressionEvaluator::new();
        
        // 设置变量
        evaluator.set_variable("temp", CalculationValue::Numeric(75.0));
        evaluator.set_variable("pressure", CalculationValue::Numeric(120.0));
        evaluator.set_variable("manual_mode", CalculationValue::Boolean(false));
        evaluator.set_variable("auto_mode", CalculationValue::Boolean(true));
        evaluator.set_variable("emergency", CalculationValue::Boolean(false));
        
        // 测试复合表达式
        let complex_expr = "(manual_mode OR (auto_mode AND temp > 70)) AND NOT emergency";
        let result = evaluator.evaluate(complex_expr).unwrap();
        assert_eq!(result.as_boolean(), Some(true));
        
        // 另一个复合表达式
        let alarm_expr = "(temp > 80 OR pressure > 150) AND NOT manual_mode";
        let alarm_result = evaluator.evaluate(alarm_expr).unwrap();
        assert_eq!(alarm_result.as_boolean(), Some(false)); // temp <= 80 且 pressure <= 150
    }

    #[test]
    fn test_telemetry_point_id_parsing() {
        // 测试正确的解析
        let point_id = TelemetryPointId::from_string("telemetry:1001").unwrap();
        assert_eq!(point_id.telemetry_type, TelemetryType::Telemetry);
        assert_eq!(point_id.point_id, 1001);
        
        let signal_id = TelemetryPointId::from_string("signaling:2002").unwrap();
        assert_eq!(signal_id.telemetry_type, TelemetryType::Signaling);
        assert_eq!(signal_id.point_id, 2002);
        
        // 测试字符串转换
        let id_str = point_id.to_string();
        assert_eq!(id_str, "Remote Measurement:1001");
        
        // 测试错误格式
        assert!(TelemetryPointId::from_string("invalid").is_err());
        assert!(TelemetryPointId::from_string("unknown:123").is_err());
        assert!(TelemetryPointId::from_string("telemetry:abc").is_err());
    }

    #[test]
    fn test_forward_calculation_config_creation() {
        let source_points = {
            let mut map = HashMap::new();
            map.insert("switch1".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1001));
            map.insert("switch2".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1002));
            map
        };

        let config = ForwardCalculationConfig::new(
            "test_logic".to_string(),
            "Test Logic".to_string(),
            TelemetryPointId::new(TelemetryType::Control, 3001),
            "switch1 AND switch2".to_string(),
            source_points,
        );

        assert_eq!(config.id, "test_logic");
        assert_eq!(config.target_point.telemetry_type, TelemetryType::Control);
        assert_eq!(config.target_point.point_id, 3001);
        assert_eq!(config.expression, "switch1 AND switch2");
        assert_eq!(config.source_points.len(), 2);
    }

    #[test]
    fn test_forward_calculation_config_legacy_conversion() {
        let legacy_sources = {
            let mut map = HashMap::new();
            map.insert("temp1".to_string(), 1001);
            map.insert("temp2".to_string(), 1002);
            map
        };

        let config = ForwardCalculationConfig::from_legacy(
            "legacy_calc".to_string(),
            "Legacy Calculation".to_string(),
            5000,
            TelemetryType::Telemetry,
            "temp1 + temp2".to_string(),
            legacy_sources,
        );

        assert_eq!(config.target_point.point_id, 5000);
        assert_eq!(config.target_point.telemetry_type, TelemetryType::Telemetry);
        assert_eq!(config.source_points.len(), 2);
        
        // 检查向后兼容性字段
        assert_eq!(config.target_point_id, Some(5000));
        assert_eq!(config.target_telemetry_type, Some(TelemetryType::Telemetry));
    }

    #[tokio::test]
    async fn test_enhanced_forward_calculation_engine_logical_operations() {
        let mut engine = EnhancedForwardCalculationEngine::new();

        // 添加逻辑与运算配置
        let config = ForwardCalculationConfig::new(
            "and_logic".to_string(),
            "AND Logic Test".to_string(),
            TelemetryPointId::new(TelemetryType::Control, 3001),
            "pump_ready AND valve_open".to_string(),
            {
                let mut map = HashMap::new();
                map.insert("pump_ready".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1001));
                map.insert("valve_open".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1002));
                map
            },
        );

        engine.add_config(config).unwrap();

        // 测试数据：两个条件都为真
        let mut source_data = HashMap::new();
        source_data.insert("signaling:1001".to_string(), CalculationValue::Boolean(true));  // pump_ready
        source_data.insert("signaling:1002".to_string(), CalculationValue::Boolean(true));  // valve_open

        let result = engine.execute_calculation("and_logic", &source_data).await.unwrap();
        assert_eq!(result.status, CalculationStatus::Success);
        assert_eq!(result.result_value.as_boolean(), Some(true));

        // 测试数据：一个条件为假
        source_data.insert("signaling:1002".to_string(), CalculationValue::Boolean(false)); // valve_open = false

        let result = engine.execute_calculation("and_logic", &source_data).await.unwrap();
        assert_eq!(result.status, CalculationStatus::Success);
        assert_eq!(result.result_value.as_boolean(), Some(false));
    }

    #[tokio::test]
    async fn test_enhanced_forward_calculation_engine_complex_logic() {
        let mut engine = EnhancedForwardCalculationEngine::new();

        // 添加复合逻辑配置
        let config = ForwardCalculationConfig::new(
            "complex_logic".to_string(),
            "Complex Logic Test".to_string(),
            TelemetryPointId::new(TelemetryType::Control, 3002),
            "(manual_mode OR (auto_mode AND temp > 50)) AND NOT emergency_stop".to_string(),
            {
                let mut map = HashMap::new();
                map.insert("manual_mode".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1010));
                map.insert("auto_mode".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1011));
                map.insert("temp".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1012));
                map.insert("emergency_stop".to_string(), TelemetryPointId::new(TelemetryType::Signaling, 1013));
                map
            },
        );

        engine.add_config(config).unwrap();

        // 测试场景1：手动模式，应该为真
        let mut source_data = HashMap::new();
        source_data.insert("signaling:1010".to_string(), CalculationValue::Boolean(true));   // manual_mode
        source_data.insert("signaling:1011".to_string(), CalculationValue::Boolean(false));  // auto_mode
        source_data.insert("telemetry:1012".to_string(), CalculationValue::Numeric(45.0));   // temp
        source_data.insert("signaling:1013".to_string(), CalculationValue::Boolean(false));  // emergency_stop

        let result = engine.execute_calculation("complex_logic", &source_data).await.unwrap();
        assert_eq!(result.status, CalculationStatus::Success);
        assert_eq!(result.result_value.as_boolean(), Some(true));

        // 测试场景2：自动模式且温度高，应该为真
        source_data.insert("signaling:1010".to_string(), CalculationValue::Boolean(false));  // manual_mode = false
        source_data.insert("signaling:1011".to_string(), CalculationValue::Boolean(true));   // auto_mode = true
        source_data.insert("telemetry:1012".to_string(), CalculationValue::Numeric(75.0));   // temp = 75 > 50

        let result = engine.execute_calculation("complex_logic", &source_data).await.unwrap();
        assert_eq!(result.status, CalculationStatus::Success);
        assert_eq!(result.result_value.as_boolean(), Some(true));

        // 测试场景3：紧急停止激活，应该为假
        source_data.insert("signaling:1013".to_string(), CalculationValue::Boolean(true));   // emergency_stop = true

        let result = engine.execute_calculation("complex_logic", &source_data).await.unwrap();
        assert_eq!(result.status, CalculationStatus::Success);
        assert_eq!(result.result_value.as_boolean(), Some(false));
    }

    #[tokio::test]
    async fn test_forward_calculation_type_validation() {
        let mut engine = EnhancedForwardCalculationEngine::new();

        // 创建一个返回数值但目标是逻辑信号的配置（类型不匹配）
        let config = ForwardCalculationConfig::new(
            "type_mismatch".to_string(),
            "Type Mismatch Test".to_string(),
            TelemetryPointId::new(TelemetryType::Signaling, 2001), // 期望布尔值
            "temp1 + temp2".to_string(), // 但表达式返回数值
            {
                let mut map = HashMap::new();
                map.insert("temp1".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1001));
                map.insert("temp2".to_string(), TelemetryPointId::new(TelemetryType::Telemetry, 1002));
                map
            },
        );

        engine.add_config(config).unwrap();

        let mut source_data = HashMap::new();
        source_data.insert("telemetry:1001".to_string(), CalculationValue::Numeric(25.0));
        source_data.insert("telemetry:1002".to_string(), CalculationValue::Numeric(30.0));

        let result = engine.execute_calculation("type_mismatch", &source_data).await.unwrap();
        assert_eq!(result.status, CalculationStatus::TypeMismatch);
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_logic_example_configs() {
        let configs = EnhancedForwardCalculationEngine::create_logic_example_configs();
        assert_eq!(configs.len(), 5);

        // 验证第一个配置：设备启动逻辑
        let device_start = &configs[0];
        assert_eq!(device_start.id, "device_start_logic");
        assert_eq!(device_start.expression, "switch1 AND switch2");
        assert_eq!(device_start.target_point.telemetry_type, TelemetryType::Control);

        // 验证第二个配置：报警或逻辑
        let alarm_or = &configs[1];
        assert_eq!(alarm_or.id, "alarm_or_logic");
        assert!(alarm_or.expression.contains("OR"));
        assert_eq!(alarm_or.target_point.telemetry_type, TelemetryType::Signaling);

        // 验证第三个配置：非运算逻辑
        let not_logic = &configs[2];
        assert_eq!(not_logic.id, "device_running_status");
        assert!(not_logic.expression.contains("NOT"));

        // 验证第四个配置：复合逻辑
        let complex = &configs[3];
        assert_eq!(complex.id, "complex_control_logic");
        assert!(complex.expression.contains("AND"));
        assert!(complex.expression.contains("OR"));
        assert!(complex.expression.contains("NOT"));

        // 验证第五个配置：数值转逻辑
        let temp_alarm = &configs[4];
        assert_eq!(temp_alarm.id, "temperature_high_alarm");
        assert!(temp_alarm.expression.contains(">"));
    }
}


