//! Data processing module
//!
//! Responsible for unified processing of data conversion logic for all protocols, including:
//! - scale/offset calculation
//! - reverse logic (boolean value inversion)
//! - data type conversion

use crate::core::config::types::{ScalingInfo, TelemetryType};

/// Process point data value
///
/// Process raw data value based on telemetry type and scaling information
///
/// # Arguments
/// * `raw_value` - Raw value read from protocol
/// * `telemetry_type` - Telemetry type (telemetry, signal, control, adjustment)
/// * `scaling` - Optional scaling information
///
/// # Returns
/// Processed value
pub fn process_point_value(
    raw_value: f64,
    telemetry_type: &TelemetryType,
    scaling: Option<&ScalingInfo>,
) -> f64 {
    let mut processed_value = raw_value;

    if let Some(scaling_info) = scaling {
        // For telemetry and adjustment types, apply scale and offset
        match telemetry_type {
            TelemetryType::Telemetry | TelemetryType::Adjustment => {
                processed_value = raw_value * scaling_info.scale + scaling_info.offset;
            },
            // For signal and control types, check if reversal is needed
            TelemetryType::Signal | TelemetryType::Control => {
                if let Some(true) = scaling_info.reverse {
                    // Reversal logic: 0->1, non-0->0
                    processed_value = if raw_value == 0.0 { 1.0 } else { 0.0 };
                }
            },
        }
    }

    processed_value
}

/// Batch process point data
///
/// # Arguments
/// * `points` - Mapping from point ID to raw value
/// * `telemetry_type` - Telemetry type
/// * `scaling_map` - Mapping from point ID to scaling information
///
/// # Returns
/// Processed point data mapping
pub fn process_point_batch(
    points: &[(u32, f64)],
    telemetry_type: &TelemetryType,
    scaling_map: &std::collections::HashMap<u32, ScalingInfo>,
) -> Vec<(u32, f64)> {
    points
        .iter()
        .map(|(point_id, raw_value)| {
            let scaling = scaling_map.get(point_id);
            let processed_value = process_point_value(*raw_value, telemetry_type, scaling);
            (*point_id, processed_value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_scaling() {
        let scaling = ScalingInfo {
            scale: 0.1,
            offset: 2.0,
            unit: Some("°C".to_string()),
            reverse: None,
        };

        let result = process_point_value(100.0, &TelemetryType::Telemetry, Some(&scaling));
        assert_eq!(result, 12.0); // 100 * 0.1 + 2.0 = 12.0
    }

    #[test]
    fn test_signal_reverse_true() {
        let scaling = ScalingInfo {
            scale: 1.0,
            offset: 0.0,
            unit: None,
            reverse: Some(true),
        };

        // Test 0 -> 1
        let result = process_point_value(0.0, &TelemetryType::Signal, Some(&scaling));
        assert_eq!(result, 1.0);

        // Test 1 -> 0
        let result = process_point_value(1.0, &TelemetryType::Signal, Some(&scaling));
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_signal_no_reverse() {
        let scaling = ScalingInfo {
            scale: 1.0,
            offset: 0.0,
            unit: None,
            reverse: Some(false),
        };

        let result = process_point_value(1.0, &TelemetryType::Signal, Some(&scaling));
        assert_eq!(result, 1.0); // No reversal
    }

    #[test]
    fn test_control_reverse() {
        let scaling = ScalingInfo {
            scale: 1.0,
            offset: 0.0,
            unit: None,
            reverse: Some(true),
        };

        let result = process_point_value(1.0, &TelemetryType::Control, Some(&scaling));
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_adjustment_scaling() {
        let scaling = ScalingInfo {
            scale: 10.0,
            offset: -50.0,
            unit: Some("kW".to_string()),
            reverse: None, // adjustment doesn't use reverse
        };

        let result = process_point_value(15.0, &TelemetryType::Adjustment, Some(&scaling));
        assert_eq!(result, 100.0); // 15 * 10 - 50 = 100
    }

    #[test]
    fn test_no_scaling() {
        let result = process_point_value(42.0, &TelemetryType::Telemetry, None);
        assert_eq!(result, 42.0); // Return original value when no scaling
    }

    #[test]
    fn test_batch_processing() {
        let mut scaling_map = std::collections::HashMap::new();
        scaling_map.insert(
            1,
            ScalingInfo {
                scale: 0.1,
                offset: 0.0,
                unit: None,
                reverse: None,
            },
        );
        scaling_map.insert(
            2,
            ScalingInfo {
                scale: 1.0,
                offset: 0.0,
                unit: None,
                reverse: Some(true),
            },
        );

        let points = vec![(1, 100.0), (2, 1.0)];

        // Test telemetry batch processing
        let result = process_point_batch(&points, &TelemetryType::Telemetry, &scaling_map);
        assert_eq!(result[0], (1, 10.0)); // 100 * 0.1
        assert_eq!(result[1], (2, 1.0)); // Telemetry doesn't use reverse

        // Test signal batch processing
        let result = process_point_batch(&points, &TelemetryType::Signal, &scaling_map);
        assert_eq!(result[0], (1, 100.0)); // Signal doesn't use scale
        assert_eq!(result[1], (2, 0.0)); // reverse: 1 -> 0
    }
}
