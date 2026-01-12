//! Data quality codes for industrial protocols.
//!
//! Quality codes indicate the reliability and validity of data points.
//! This implementation is compatible with OPC UA quality codes.

use serde::{Deserialize, Serialize};

/// Data quality indicator.
///
/// Represents the quality/reliability of a data point value.
/// Based on OPC UA quality codes for maximum compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// Value is good and reliable
    #[default]
    Good,

    /// Value is bad/unreliable
    Bad,

    /// Value quality is uncertain
    Uncertain,

    /// Value is invalid (not applicable)
    Invalid,

    /// Communication with device lost
    NotConnected,

    /// Device failure detected
    DeviceFailure,

    /// Sensor failure detected
    SensorFailure,

    /// Communication failure
    CommFailure,

    /// Point is out of service
    OutOfService,

    /// Value has been manually substituted
    Substituted,

    /// Value overflow (out of range)
    Overflow,

    /// Value underflow (below range)
    Underflow,

    /// Configuration error
    ConfigError,

    /// Last known value (connection lost but value cached)
    LastKnown,
}

impl Quality {
    /// Check if the quality is good.
    #[inline]
    pub fn is_good(&self) -> bool {
        matches!(self, Self::Good)
    }

    /// Check if the quality is bad (any non-good status).
    #[inline]
    pub fn is_bad(&self) -> bool {
        !self.is_good()
    }

    /// Check if the quality indicates a connection problem.
    #[inline]
    pub fn is_connection_problem(&self) -> bool {
        matches!(
            self,
            Self::NotConnected | Self::CommFailure | Self::LastKnown
        )
    }

    /// Check if the quality indicates a device problem.
    #[inline]
    pub fn is_device_problem(&self) -> bool {
        matches!(self, Self::DeviceFailure | Self::SensorFailure)
    }

    /// Convert to OPC UA status code (subset).
    pub fn to_opc_status(&self) -> u32 {
        match self {
            Self::Good => 0x00000000,          // Good
            Self::Bad => 0x80000000,           // Bad
            Self::Uncertain => 0x40000000,     // Uncertain
            Self::Invalid => 0x80010000,       // BadInvalidState
            Self::NotConnected => 0x80080000,  // BadNotConnected
            Self::DeviceFailure => 0x80100000, // BadDeviceFailure
            Self::SensorFailure => 0x80110000, // BadSensorFailure
            Self::CommFailure => 0x80130000,   // BadCommunicationError
            Self::OutOfService => 0x80870000,  // BadOutOfService
            Self::Substituted => 0x40920000,   // UncertainSubstituteValue
            Self::Overflow => 0x80780000,      // BadDataEncodingInvalid (approx)
            Self::Underflow => 0x80780000,     // BadDataEncodingInvalid (approx)
            Self::ConfigError => 0x80890000,   // BadConfigurationError
            Self::LastKnown => 0x408F0000,     // UncertainLastUsableValue
        }
    }

    /// Create from OPC UA status code (simplified).
    pub fn from_opc_status(status: u32) -> Self {
        let severity = status & 0xC0000000;
        match severity {
            0x00000000 => Self::Good,
            0x40000000 => Self::Uncertain,
            _ => Self::Bad,
        }
    }

    /// Get a short description of this quality.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Good => "Good",
            Self::Bad => "Bad",
            Self::Uncertain => "Uncertain",
            Self::Invalid => "Invalid",
            Self::NotConnected => "Not Connected",
            Self::DeviceFailure => "Device Failure",
            Self::SensorFailure => "Sensor Failure",
            Self::CommFailure => "Communication Failure",
            Self::OutOfService => "Out of Service",
            Self::Substituted => "Substituted",
            Self::Overflow => "Overflow",
            Self::Underflow => "Underflow",
            Self::ConfigError => "Configuration Error",
            Self::LastKnown => "Last Known Value",
        }
    }
}

impl std::fmt::Display for Quality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_default() {
        assert_eq!(Quality::default(), Quality::Good);
    }

    #[test]
    fn test_quality_checks() {
        assert!(Quality::Good.is_good());
        assert!(!Quality::Bad.is_good());
        assert!(Quality::Bad.is_bad());
        assert!(Quality::NotConnected.is_connection_problem());
        assert!(Quality::DeviceFailure.is_device_problem());
    }

    #[test]
    fn test_opc_status_conversion() {
        assert_eq!(Quality::from_opc_status(0x00000000), Quality::Good);
        assert_eq!(Quality::from_opc_status(0x40000000), Quality::Uncertain);
        assert_eq!(Quality::from_opc_status(0x80000000), Quality::Bad);
    }
}
