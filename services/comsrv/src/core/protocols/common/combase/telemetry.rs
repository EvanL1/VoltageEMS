//! Telemetry Types and Four-Telemetry Operations
//!
//! This module contains all telemetry-related data structures and types
//! used for the four telemetry operations: measurement, signaling, control, and regulation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Four telemetry types classification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Get Chinese name
    pub fn chinese_name(&self) -> &'static str {
        match self {
            TelemetryType::Telemetry => "遥测",
            TelemetryType::Signaling => "遥信",
            TelemetryType::Control => "遥控",
            TelemetryType::Setpoint => "遥调",
        }
    }

    /// Get English name
    pub fn english_name(&self) -> &'static str {
        match self {
            TelemetryType::Telemetry => "Measurement",
            TelemetryType::Signaling => "Signaling",
            TelemetryType::Control => "Control",
            TelemetryType::Setpoint => "Regulation",
        }
    }

    /// Check if this type is readable
    pub fn is_readable(&self) -> bool {
        matches!(self, TelemetryType::Telemetry | TelemetryType::Signaling)
    }

    /// Check if this type is writable
    pub fn is_writable(&self) -> bool {
        matches!(self, TelemetryType::Control | TelemetryType::Setpoint)
    }

    /// Check if this type is analog
    pub fn is_analog(&self) -> bool {
        matches!(self, TelemetryType::Telemetry | TelemetryType::Setpoint)
    }

    /// Check if this type is digital
    pub fn is_digital(&self) -> bool {
        matches!(self, TelemetryType::Signaling | TelemetryType::Control)
    }
}

/// Extended measurement point data
#[derive(Debug, Clone)]
pub struct MeasurementPoint {
    /// Current analog value (in engineering units)
    pub value: f64,
    /// Engineering unit (℃, bar, m³/h, etc.)
    pub unit: String,
    /// Measurement timestamp
    pub timestamp: DateTime<Utc>,
}

/// Extended signaling point data
#[derive(Debug, Clone)]
pub struct SignalingPoint {
    /// Current digital status
    pub status: bool,
    /// Status description text
    pub status_text: String,
    /// Status change timestamp
    pub timestamp: DateTime<Utc>,
}

/// Extended control point data
#[derive(Debug, Clone)]
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

/// Extended regulation point data
#[derive(Debug, Clone)]
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

/// General execution status
#[derive(Debug, Clone, PartialEq)]
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

/// Control operation execution status
#[derive(Debug, Clone, PartialEq)]
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

/// Regulation operation execution status
#[derive(Debug, Clone, PartialEq)]
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

/// Point value type for unified handling
#[derive(Debug, Clone)]
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
    /// Get the telemetry type for this value
    pub fn telemetry_type(&self) -> TelemetryType {
        match self {
            PointValueType::Analog(_) | PointValueType::Measurement(_) => TelemetryType::Telemetry,
            PointValueType::Digital(_) | PointValueType::Signaling(_) => TelemetryType::Signaling,
            PointValueType::Control(_) => TelemetryType::Control,
            PointValueType::Regulation(_) => TelemetryType::Setpoint,
        }
    }

    /// Try to extract analog value
    pub fn as_analog(&self) -> Option<f64> {
        match self {
            PointValueType::Analog(val) => Some(*val),
            PointValueType::Measurement(point) => Some(point.value),
            PointValueType::Regulation(point) => Some(point.current_value),
            _ => None,
        }
    }

    /// Try to extract digital value
    pub fn as_digital(&self) -> Option<bool> {
        match self {
            PointValueType::Digital(val) => Some(*val),
            PointValueType::Signaling(point) => Some(point.status),
            PointValueType::Control(point) => Some(point.current_state),
            _ => None,
        }
    }
}

/// Remote operation type definition
#[derive(Debug, Clone)]
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
    /// Get telemetry type for this operation
    pub fn telemetry_type(&self) -> TelemetryType {
        match self {
            RemoteOperationType::Control { .. } | RemoteOperationType::ExtendedControl { .. } => {
                TelemetryType::Control
            }
            RemoteOperationType::Regulation { .. } | RemoteOperationType::ExtendedRegulation { .. } => {
                TelemetryType::Setpoint
            }
        }
    }

    /// Validate the operation parameters
    pub fn validate(&self) -> crate::utils::Result<()> {
        use crate::utils::ComSrvError;
        match self {
            RemoteOperationType::ExtendedRegulation {
                target_value,
                min_value,
                max_value,
                ..
            } => {
                if let (Some(min), Some(max)) = (min_value, max_value) {
                    if min >= max {
                        return Err(ComSrvError::InvalidParameter("min_value must be less than max_value".to_string()));
                    }
                    if target_value < min || target_value > max {
                        return Err(ComSrvError::InvalidParameter(format!("target_value {} is out of range [{}, {}]", target_value, min, max)));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
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
    /// Operator information
    pub operator: Option<String>,
    /// Operation description
    pub description: Option<String>,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
}

/// Remote operation response
#[derive(Debug, Clone)]
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

/// Extended telemetry metadata
#[derive(Debug, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_type_properties() {
        assert!(TelemetryType::Telemetry.is_readable());
        assert!(!TelemetryType::Telemetry.is_writable());
        assert!(TelemetryType::Telemetry.is_analog());
        assert!(!TelemetryType::Telemetry.is_digital());

        assert!(TelemetryType::Signaling.is_readable());
        assert!(!TelemetryType::Signaling.is_writable());
        assert!(!TelemetryType::Signaling.is_analog());
        assert!(TelemetryType::Signaling.is_digital());

        assert!(!TelemetryType::Control.is_readable());
        assert!(TelemetryType::Control.is_writable());
        assert!(!TelemetryType::Control.is_analog());
        assert!(TelemetryType::Control.is_digital());

        assert!(!TelemetryType::Setpoint.is_readable());
        assert!(TelemetryType::Setpoint.is_writable());
        assert!(TelemetryType::Setpoint.is_analog());
        assert!(!TelemetryType::Setpoint.is_digital());
    }

    #[test]
    fn test_point_value_type_extraction() {
        let analog_value = PointValueType::Analog(123.45);
        assert_eq!(analog_value.as_analog(), Some(123.45));
        assert_eq!(analog_value.as_digital(), None);

        let digital_value = PointValueType::Digital(true);
        assert_eq!(digital_value.as_analog(), None);
        assert_eq!(digital_value.as_digital(), Some(true));
    }

    #[test]
    fn test_remote_operation_validation() {
        let valid_operation = RemoteOperationType::ExtendedRegulation {
            target_value: 50.0,
            operator: "test".to_string(),
            description: None,
            min_value: Some(0.0),
            max_value: Some(100.0),
            step_size: None,
        };
        assert!(valid_operation.validate().is_ok());

        let invalid_operation = RemoteOperationType::ExtendedRegulation {
            target_value: 150.0,
            operator: "test".to_string(),
            description: None,
            min_value: Some(0.0),
            max_value: Some(100.0),
            step_size: None,
        };
        assert!(invalid_operation.validate().is_err());
    }
} 