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

    // ========================================================================
    // Internal ID Encoding/Decoding for point_id collision avoidance
    // ========================================================================

    /// Offset between point type ranges (~1 billion points per type).
    ///
    /// This ensures different point types can use the same original point_id
    /// without colliding in the internal representation.
    ///
    /// # Layout
    /// - Telemetry: 0x00000000 - 0x3FFFFFFF
    /// - Signal:    0x40000000 - 0x7FFFFFFF
    /// - Control:   0x80000000 - 0xBFFFFFFF
    /// - Adjustment: 0xC0000000 - 0xFFFFFFFF
    pub const OFFSET: u32 = u32::MAX / 4; // 0x3FFFFFFF ≈ 1.07 billion

    /// Get the type offset for this point type.
    ///
    /// # Examples
    /// ```
    /// # use voltage_model::PointType;
    /// assert_eq!(PointType::Telemetry.type_offset(), 0);
    /// assert_eq!(PointType::Signal.type_offset(), PointType::OFFSET);
    /// assert_eq!(PointType::Control.type_offset(), PointType::OFFSET * 2);
    /// ```
    pub fn type_offset(&self) -> u32 {
        match self {
            PointType::Telemetry => 0,
            PointType::Signal => Self::OFFSET,
            PointType::Control => Self::OFFSET * 2,
            PointType::Adjustment => Self::OFFSET * 3,
        }
    }

    /// Convert an original point_id to an internal_id that encodes the type.
    ///
    /// Used when building protocol configurations (igw_bridge.rs).
    ///
    /// # Examples
    /// ```
    /// # use voltage_model::PointType;
    /// // Signal point_id=1 becomes internal_id = 0x40000001
    /// let internal = PointType::Signal.to_internal_id(1);
    /// assert_eq!(internal, PointType::OFFSET + 1);
    /// ```
    pub fn to_internal_id(&self, point_id: u32) -> u32 {
        point_id + self.type_offset()
    }

    /// Decode an internal_id back to (PointType, original_point_id).
    ///
    /// Used when writing data to Redis (redis_store.rs).
    ///
    /// # Examples
    /// ```
    /// # use voltage_model::PointType;
    /// let internal = PointType::Signal.to_internal_id(5);
    /// let (pt, id) = PointType::from_internal_id(internal);
    /// assert_eq!(pt, PointType::Signal);
    /// assert_eq!(id, 5);
    /// ```
    pub fn from_internal_id(internal_id: u32) -> (Self, u32) {
        let type_index = internal_id / Self::OFFSET;
        let original_id = internal_id % Self::OFFSET;
        let point_type = match type_index {
            0 => PointType::Telemetry,
            1 => PointType::Signal,
            2 => PointType::Control,
            _ => PointType::Adjustment, // 3 or overflow wraps to Adjustment
        };
        (point_type, original_id)
    }
}

impl fmt::Display for PointType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PointType {
    type Err = String;

    /// Parse PointType from string (case-insensitive, zero allocation for valid inputs)
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Direct pattern matching for common cases (zero allocation)
        match s {
            "T" | "t" | "YC" | "yc" | "Yc" | "yC" => Ok(PointType::Telemetry),
            "S" | "s" | "YX" | "yx" | "Yx" | "yX" => Ok(PointType::Signal),
            "C" | "c" | "YK" | "yk" | "Yk" | "yK" => Ok(PointType::Control),
            "A" | "a" | "YT" | "yt" | "Yt" | "yT" => Ok(PointType::Adjustment),
            _ => Err(format!(
                "Invalid PointType: '{}'. Valid values: T/YC, S/YX, C/YK, A/YT",
                s
            )),
        }
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
    fn test_internal_id_roundtrip() {
        // Test all point types with various point_ids
        for (pt, original_id) in [
            (PointType::Telemetry, 1),
            (PointType::Telemetry, 100),
            (PointType::Signal, 1),
            (PointType::Signal, 8),
            (PointType::Control, 1),
            (PointType::Control, 8),
            (PointType::Adjustment, 1),
            (PointType::Adjustment, 1000),
        ] {
            let internal = pt.to_internal_id(original_id);
            let (recovered_type, recovered_id) = PointType::from_internal_id(internal);
            assert_eq!(
                recovered_type, pt,
                "Type mismatch for {:?} id={}",
                pt, original_id
            );
            assert_eq!(
                recovered_id, original_id,
                "ID mismatch for {:?} id={}",
                pt, original_id
            );
        }
    }

    #[test]
    fn test_internal_id_no_collision() {
        // Signal point_id=1 and Control point_id=1 should have different internal IDs
        let signal_internal = PointType::Signal.to_internal_id(1);
        let control_internal = PointType::Control.to_internal_id(1);
        assert_ne!(
            signal_internal, control_internal,
            "Signal and Control internal IDs should differ"
        );

        // Verify they decode correctly
        let (s_type, s_id) = PointType::from_internal_id(signal_internal);
        let (c_type, c_id) = PointType::from_internal_id(control_internal);
        assert_eq!(s_type, PointType::Signal);
        assert_eq!(c_type, PointType::Control);
        assert_eq!(s_id, 1);
        assert_eq!(c_id, 1);
    }

    #[test]
    fn test_type_offset_values() {
        // Verify offsets are non-overlapping
        assert_eq!(PointType::Telemetry.type_offset(), 0);
        assert_eq!(PointType::Signal.type_offset(), PointType::OFFSET);
        assert_eq!(PointType::Control.type_offset(), PointType::OFFSET * 2);
        assert_eq!(PointType::Adjustment.type_offset(), PointType::OFFSET * 3);

        // Verify OFFSET is large enough (about 1 billion) - use const_assert or static check
        // PointType::OFFSET = u32::MAX / 4 = 0x3FFFFFFF ≈ 1.07 billion
        // This is verified at compile time by the constant definition
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
