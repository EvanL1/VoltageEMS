//! 数据处理模块
//!
//! 负责统一处理所有协议的数据转换逻辑，包括：
//! - scale/offset 计算
//! - reverse 逻辑（布尔值反转）
//! - 数据类型转换

use crate::core::config::types::{ScalingInfo, TelemetryType};

/// 处理点位数据值
///
/// 根据遥测类型和缩放信息处理原始数据值
///
/// # Arguments
/// * `raw_value` - 从协议读取的原始值
/// * `telemetry_type` - 遥测类型（遥测、遥信、遥控、遥调）
/// * `scaling` - 可选的缩放信息
///
/// # Returns
/// 处理后的值
pub fn process_point_value(
    raw_value: f64,
    telemetry_type: &TelemetryType,
    scaling: Option<&ScalingInfo>,
) -> f64 {
    let mut processed_value = raw_value;

    if let Some(scaling_info) = scaling {
        // 对于遥测和遥调类型，应用 scale 和 offset
        match telemetry_type {
            TelemetryType::Measurement | TelemetryType::Adjustment => {
                processed_value = raw_value * scaling_info.scale + scaling_info.offset;
            }
            // 对于遥信和遥控类型，检查是否需要反转
            TelemetryType::Signal | TelemetryType::Control => {
                if let Some(true) = scaling_info.reverse {
                    // 反转逻辑：0->1, 非0->0
                    processed_value = if raw_value == 0.0 { 1.0 } else { 0.0 };
                }
            }
        }
    }

    processed_value
}

/// 批量处理点位数据
///
/// # Arguments
/// * `points` - 点位ID到原始值的映射
/// * `telemetry_type` - 遥测类型
/// * `scaling_map` - 点位ID到缩放信息的映射
///
/// # Returns
/// 处理后的点位数据映射
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
    fn test_measurement_scaling() {
        let scaling = ScalingInfo {
            scale: 0.1,
            offset: 2.0,
            unit: Some("°C".to_string()),
            reverse: None,
        };

        let result = process_point_value(100.0, &TelemetryType::Measurement, Some(&scaling));
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

        // 测试 0 -> 1
        let result = process_point_value(0.0, &TelemetryType::Signal, Some(&scaling));
        assert_eq!(result, 1.0);

        // 测试 1 -> 0
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
        assert_eq!(result, 1.0); // 不反转
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
            reverse: None, // adjustment 不使用 reverse
        };

        let result = process_point_value(15.0, &TelemetryType::Adjustment, Some(&scaling));
        assert_eq!(result, 100.0); // 15 * 10 - 50 = 100
    }

    #[test]
    fn test_no_scaling() {
        let result = process_point_value(42.0, &TelemetryType::Measurement, None);
        assert_eq!(result, 42.0); // 无缩放时返回原值
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

        // 测试遥测批处理
        let result = process_point_batch(&points, &TelemetryType::Measurement, &scaling_map);
        assert_eq!(result[0], (1, 10.0)); // 100 * 0.1
        assert_eq!(result[1], (2, 1.0)); // 遥测不使用 reverse

        // 测试遥信批处理
        let result = process_point_batch(&points, &TelemetryType::Signal, &scaling_map);
        assert_eq!(result[0], (1, 100.0)); // 遥信不使用 scale
        assert_eq!(result[1], (2, 0.0)); // reverse: 1 -> 0
    }
}
