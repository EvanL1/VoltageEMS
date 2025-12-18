//! Core domain types for VoltageEMS
//!
//! This module contains fundamental types used across the system.

use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// Four Remote Point Types
// ============================================================================

/// Four Remote Point Types used in industrial SCADA systems
///
/// These types correspond to the standard IEC "Four Remote" classification:
/// - T (Telemetry): Analog measurements (YC in Chinese standards)
/// - S (Signal): Digital status (YX in Chinese standards)
/// - C (Control): Digital commands (YK in Chinese standards)
/// - A (Adjustment): Analog setpoints (YT in Chinese standards)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum PointType {
    /// T - Telemetry - Analog measurements (YC in IEC standards)
    #[serde(rename = "T", alias = "YC", alias = "yc", alias = "telemetry")]
    Telemetry,

    /// S - Signal - Digital status (YX in IEC standards)
    #[serde(rename = "S", alias = "YX", alias = "yx", alias = "signal")]
    Signal,

    /// C - Control - Digital commands (YK in IEC standards)
    #[serde(rename = "C", alias = "YK", alias = "yk", alias = "control")]
    Control,

    /// A - Adjustment - Analog setpoints (YT in IEC standards)
    #[serde(
        rename = "A",
        alias = "YT",
        alias = "yt",
        alias = "adjustment",
        alias = "setpoint"
    )]
    Adjustment,
}

impl PointType {
    /// Convert to Redis key suffix
    ///
    /// # Examples
    /// ```
    /// # use voltage_model::PointType;
    /// assert_eq!(PointType::Telemetry.as_str(), "T");
    /// assert_eq!(PointType::Signal.as_str(), "S");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            PointType::Telemetry => "T",
            PointType::Signal => "S",
            PointType::Control => "C",
            PointType::Adjustment => "A",
        }
    }

    /// Parse from string (convenience method, returns Option)
    ///
    /// This is a convenience wrapper around `str::parse()` that returns `Option`
    /// instead of `Result`. For full error information, use `str::parse()` directly.
    ///
    /// # Examples
    /// ```
    /// # use voltage_model::PointType;
    /// assert_eq!(PointType::from_str("T"), Some(PointType::Telemetry));
    /// assert_eq!(PointType::from_str("invalid"), None);
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// Check if this is a measurement type (T or S)
    pub fn is_measurement(&self) -> bool {
        matches!(self, PointType::Telemetry | PointType::Signal)
    }

    /// Check if this is an action type (C or A)
    pub fn is_action(&self) -> bool {
        matches!(self, PointType::Control | PointType::Adjustment)
    }

    /// Check if this is an analog type (T or A)
    pub fn is_analog(&self) -> bool {
        matches!(self, PointType::Telemetry | PointType::Adjustment)
    }

    /// Check if this is a digital type (S or C)
    pub fn is_digital(&self) -> bool {
        matches!(self, PointType::Signal | PointType::Control)
    }

    /// Check if this is an input type (T or S) - alias for is_measurement
    ///
    /// Input types are data that flows from devices to the system.
    pub fn is_input(&self) -> bool {
        self.is_measurement()
    }

    /// Check if this is an output type (C or A) - alias for is_action
    ///
    /// Output types are commands that flow from the system to devices.
    pub fn is_output(&self) -> bool {
        self.is_action()
    }
}

impl fmt::Display for PointType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PointType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let u = s.to_uppercase();
        match u.as_str() {
            "T" | "YC" => Ok(PointType::Telemetry),
            "S" | "YX" => Ok(PointType::Signal),
            "C" | "YK" => Ok(PointType::Control),
            "A" | "YT" => Ok(PointType::Adjustment),
            _ => Err(format!(
                "Invalid PointType: '{}'. Valid values: T/YC, S/YX, C/YK, A/YT",
                s
            )),
        }
    }
}

impl Default for PointType {
    fn default() -> Self {
        Self::Telemetry
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_point_type_as_str() {
        assert_eq!(PointType::Telemetry.as_str(), "T");
        assert_eq!(PointType::Signal.as_str(), "S");
        assert_eq!(PointType::Control.as_str(), "C");
        assert_eq!(PointType::Adjustment.as_str(), "A");
    }

    #[test]
    fn test_point_type_from_str() {
        // Standard codes
        assert_eq!(PointType::from_str("T"), Some(PointType::Telemetry));
        assert_eq!(PointType::from_str("S"), Some(PointType::Signal));
        assert_eq!(PointType::from_str("C"), Some(PointType::Control));
        assert_eq!(PointType::from_str("A"), Some(PointType::Adjustment));
        // IEC synonyms
        assert_eq!(PointType::from_str("YC"), Some(PointType::Telemetry));
        assert_eq!(PointType::from_str("YX"), Some(PointType::Signal));
        assert_eq!(PointType::from_str("YK"), Some(PointType::Control));
        assert_eq!(PointType::from_str("YT"), Some(PointType::Adjustment));
        // Case insensitive
        assert_eq!(PointType::from_str("yc"), Some(PointType::Telemetry));
        assert_eq!(PointType::from_str("t"), Some(PointType::Telemetry));
        // Invalid
        assert_eq!(PointType::from_str("invalid"), None);
    }

    #[test]
    fn test_point_type_categories() {
        assert!(PointType::Telemetry.is_measurement());
        assert!(PointType::Signal.is_measurement());
        assert!(!PointType::Control.is_measurement());
        assert!(!PointType::Adjustment.is_measurement());

        assert!(!PointType::Telemetry.is_action());
        assert!(!PointType::Signal.is_action());
        assert!(PointType::Control.is_action());
        assert!(PointType::Adjustment.is_action());

        assert!(PointType::Telemetry.is_analog());
        assert!(!PointType::Signal.is_analog());
        assert!(!PointType::Control.is_analog());
        assert!(PointType::Adjustment.is_analog());

        assert!(!PointType::Telemetry.is_digital());
        assert!(PointType::Signal.is_digital());
        assert!(PointType::Control.is_digital());
        assert!(!PointType::Adjustment.is_digital());
    }

    #[test]
    fn test_point_type_display() {
        assert_eq!(format!("{}", PointType::Telemetry), "T");
        assert_eq!(format!("{}", PointType::Signal), "S");
    }

    #[test]
    fn test_point_type_parse() {
        assert_eq!("T".parse::<PointType>().unwrap(), PointType::Telemetry);
        assert!("X".parse::<PointType>().is_err());
    }

    #[test]
    fn test_point_type_input_output() {
        // is_input is alias for is_measurement
        assert!(PointType::Telemetry.is_input());
        assert!(PointType::Signal.is_input());
        assert!(!PointType::Control.is_input());
        assert!(!PointType::Adjustment.is_input());

        // is_output is alias for is_action
        assert!(!PointType::Telemetry.is_output());
        assert!(!PointType::Signal.is_output());
        assert!(PointType::Control.is_output());
        assert!(PointType::Adjustment.is_output());
    }

    #[test]
    fn test_point_type_default() {
        assert_eq!(PointType::default(), PointType::Telemetry);
    }

    #[test]
    fn test_point_type_serde() {
        // Serialization
        assert_eq!(
            serde_json::to_string(&PointType::Telemetry).unwrap(),
            "\"T\""
        );
        assert_eq!(serde_json::to_string(&PointType::Signal).unwrap(), "\"S\"");

        // Deserialization with aliases
        assert_eq!(
            serde_json::from_str::<PointType>("\"T\"").unwrap(),
            PointType::Telemetry
        );
        assert_eq!(
            serde_json::from_str::<PointType>("\"YC\"").unwrap(),
            PointType::Telemetry
        );
        assert_eq!(
            serde_json::from_str::<PointType>("\"telemetry\"").unwrap(),
            PointType::Telemetry
        );
    }
}

// ============================================================================
// Point Role Types (Data Flow Direction)
// ============================================================================

/// Point role types indicating data flow direction
///
/// Unlike `PointType` which describes the type of data (T/S/C/A),
/// `PointRole` describes the direction of data flow:
/// - M (Measurement): Data flows from device → model (uplink)
/// - A (Action): Data flows from model → device (downlink)
///
/// # Usage
/// ```
/// # use voltage_model::PointRole;
/// let role = PointRole::Measurement;
/// assert_eq!(role.as_str(), "M");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum PointRole {
    /// Measurement point (M) - data flows from device to model
    #[serde(rename = "M")]
    Measurement,
    /// Action point (A) - data flows from model to device
    #[serde(rename = "A")]
    Action,
}

impl PointRole {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Measurement => "M",
            Self::Action => "A",
        }
    }
}

impl std::str::FromStr for PointRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "M" | "MEASUREMENT" => Ok(Self::Measurement),
            "A" | "ACTION" => Ok(Self::Action),
            _ => Err(format!("Unknown point role: {}", s)),
        }
    }
}

impl fmt::Display for PointRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for PointRole {
    fn default() -> Self {
        Self::Measurement
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod point_role_tests {
    use super::*;

    #[test]
    fn test_point_role_as_str() {
        assert_eq!(PointRole::Measurement.as_str(), "M");
        assert_eq!(PointRole::Action.as_str(), "A");
    }

    #[test]
    fn test_point_role_from_str() {
        assert_eq!("M".parse::<PointRole>().unwrap(), PointRole::Measurement);
        assert_eq!("A".parse::<PointRole>().unwrap(), PointRole::Action);
        assert_eq!(
            "measurement".parse::<PointRole>().unwrap(),
            PointRole::Measurement
        );
        assert!("X".parse::<PointRole>().is_err());
    }

    #[test]
    fn test_point_role_display() {
        assert_eq!(format!("{}", PointRole::Measurement), "M");
        assert_eq!(format!("{}", PointRole::Action), "A");
    }

    #[test]
    fn test_point_role_default() {
        assert_eq!(PointRole::default(), PointRole::Measurement);
    }
}
