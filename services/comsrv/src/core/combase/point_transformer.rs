//! Point data transformer module
//!
//! Provides unified data transformation interface for all four-telemetry types
//! Supports bidirectional transformation (DeviceToSystem and SystemToDevice)

/// Data transformation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformDirection {
    /// Device → System (raw value → processed value)
    /// Used for uplink data (Telemetry, Signal)
    DeviceToSystem,
    /// System → Device (processed value → raw value)
    /// Used for downlink commands (Control, Adjustment)
    SystemToDevice,
}

/// Point data transformer
///
/// Provides unified interface for transforming point values in both directions.
/// Uses enum for static dispatch (better performance than trait objects).
#[derive(Debug, Clone)]
pub enum PointTransformer {
    /// Linear transformation: processed = raw * scale + offset
    ///
    /// Supports bidirectional transformation:
    /// - DeviceToSystem: processed = raw * scale + offset
    /// - SystemToDevice: raw = (processed - offset) / scale
    Linear {
        /// Scale factor
        scale: f64,
        /// Offset value
        offset: f64,
    },
    /// Boolean transformation with optional reversal
    ///
    /// Supports bidirectional transformation (symmetric):
    /// - If reverse=true: output = !input (0→1, 1→0)
    /// - If reverse=false: output = input (passthrough)
    Boolean {
        /// Whether to reverse the boolean value
        reverse: bool,
    },
    /// Passthrough transformer - returns input value unchanged
    ///
    /// Used for points without configured transformation
    Passthrough,
}

impl PointTransformer {
    /// Create a new linear transformer
    pub fn linear(scale: f64, offset: f64) -> Self {
        Self::Linear { scale, offset }
    }

    /// Create a new boolean transformer
    pub fn boolean(reverse: bool) -> Self {
        Self::Boolean { reverse }
    }

    /// Create a new passthrough transformer
    pub fn passthrough() -> Self {
        Self::Passthrough
    }

    /// Transform a point value
    ///
    /// # Arguments
    /// * `value` - Input value (raw or processed depending on direction)
    /// * `direction` - Transformation direction
    ///
    /// # Returns
    /// Transformed value
    pub fn transform(&self, value: f64, direction: TransformDirection) -> f64 {
        match (self, direction) {
            // Linear uplink: raw * scale + offset
            (Self::Linear { scale, offset }, TransformDirection::DeviceToSystem) => {
                value * scale + offset
            },
            // Linear downlink: (processed - offset) / scale
            (Self::Linear { scale, offset }, TransformDirection::SystemToDevice) => {
                if *scale != 0.0 {
                    (value - offset) / scale
                } else {
                    // Avoid division by zero
                    tracing::warn!(
                        "PointTransformer::Linear: scale is zero, returning original value"
                    );
                    value
                }
            },
            // Boolean transformation (symmetric in both directions)
            (Self::Boolean { reverse }, _) => {
                if *reverse {
                    if value == 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    value
                }
            },
            // Passthrough (no transformation)
            (Self::Passthrough, _) => value,
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_linear_transformer_uplink() {
        let transformer = PointTransformer::linear(0.1, 0.0);

        // Uplink: raw * scale + offset
        let processed = transformer.transform(6693.0, TransformDirection::DeviceToSystem);
        assert!((processed - 669.3).abs() < 0.0001); // Use approximate comparison for floating point
    }

    #[test]
    fn test_linear_transformer_downlink() {
        let transformer = PointTransformer::linear(0.1, 0.0);

        // Downlink: (processed - offset) / scale
        let raw = transformer.transform(669.3, TransformDirection::SystemToDevice);
        assert!((raw - 6693.0).abs() < 0.0001); // Use approximate comparison for floating point
    }

    #[test]
    fn test_linear_transformer_with_offset() {
        let transformer = PointTransformer::linear(0.1, 10.0);

        // Uplink: 1000 * 0.1 + 10 = 110
        let processed = transformer.transform(1000.0, TransformDirection::DeviceToSystem);
        assert_eq!(processed, 110.0);

        // Downlink: (110 - 10) / 0.1 = 1000
        let raw = transformer.transform(110.0, TransformDirection::SystemToDevice);
        assert_eq!(raw, 1000.0);
    }

    #[test]
    fn test_linear_transformer_zero_scale() {
        let transformer = PointTransformer::linear(0.0, 10.0);

        // Should return original value to avoid division by zero
        let raw = transformer.transform(100.0, TransformDirection::SystemToDevice);
        assert_eq!(raw, 100.0);
    }

    #[test]
    fn test_boolean_transformer_reverse() {
        let transformer = PointTransformer::boolean(true);

        // 0 → 1
        assert_eq!(
            transformer.transform(0.0, TransformDirection::DeviceToSystem),
            1.0
        );
        // 1 → 0
        assert_eq!(
            transformer.transform(1.0, TransformDirection::DeviceToSystem),
            0.0
        );

        // Symmetric in both directions
        assert_eq!(
            transformer.transform(0.0, TransformDirection::SystemToDevice),
            1.0
        );
    }

    #[test]
    fn test_boolean_transformer_no_reverse() {
        let transformer = PointTransformer::boolean(false);

        // Passthrough
        assert_eq!(
            transformer.transform(0.0, TransformDirection::DeviceToSystem),
            0.0
        );
        assert_eq!(
            transformer.transform(1.0, TransformDirection::DeviceToSystem),
            1.0
        );
    }

    #[test]
    fn test_passthrough_transformer() {
        let transformer = PointTransformer::passthrough();

        assert_eq!(
            transformer.transform(123.45, TransformDirection::DeviceToSystem),
            123.45
        );
        assert_eq!(
            transformer.transform(678.90, TransformDirection::SystemToDevice),
            678.90
        );
    }

    #[test]
    fn test_transformer_clone() {
        let transformer = PointTransformer::linear(0.1, 0.0);
        let cloned = transformer.clone();

        let result = cloned.transform(100.0, TransformDirection::DeviceToSystem);
        assert_eq!(result, 10.0);
    }
}
