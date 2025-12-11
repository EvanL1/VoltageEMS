//! Point configuration provider module
//!
//! Provides configuration lookup for point transformers

use super::sync::PointTransformer;
use crate::core::config::RuntimeChannelConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;
use voltage_config::common::FourRemote;

/// Runtime configuration provider
///
/// Loads point configurations from RuntimeChannelConfig and provides transformers
pub struct RuntimeConfigProvider {
    /// Transformer cache: (channel_id, telemetry_type, point_id) → transformer
    #[allow(clippy::type_complexity)]
    transformers: Arc<RwLock<HashMap<(u32, String, u32), Arc<PointTransformer>>>>,
    /// Fallback passthrough transformer
    passthrough: Arc<PointTransformer>,
}

impl RuntimeConfigProvider {
    /// Create a new runtime configuration provider
    pub fn new() -> Self {
        Self {
            transformers: Arc::new(RwLock::new(HashMap::new())),
            passthrough: Arc::new(PointTransformer::passthrough()),
        }
    }

    /// Load channel configuration from RuntimeChannelConfig
    ///
    /// Extracts scale/offset/reverse from point configurations and creates transformers
    pub async fn load_channel_config(&self, runtime_config: &RuntimeChannelConfig) {
        let channel_id = runtime_config.id();
        let mut transformers = self.transformers.write().await;

        // Load Telemetry points (linear transformation)
        for point in &runtime_config.telemetry_points {
            let key = (channel_id, "T".to_string(), point.base.point_id);

            // Create linear transformer with scale/offset
            let transformer: Arc<PointTransformer> =
                Arc::new(PointTransformer::linear(point.scale, point.offset));

            transformers.insert(key, transformer);
        }

        // Load Signal points (boolean transformation)
        for point in &runtime_config.signal_points {
            let key = (channel_id, "S".to_string(), point.base.point_id);

            // Create boolean transformer with reverse flag
            let transformer: Arc<PointTransformer> =
                Arc::new(PointTransformer::boolean(point.reverse));

            transformers.insert(key, transformer);
        }

        // Load Control points (boolean transformation)
        for point in &runtime_config.control_points {
            let key = (channel_id, "C".to_string(), point.base.point_id);

            // Create boolean transformer (assume control points don't have reverse yet)
            // TODO: Add reverse field to ControlPoint if needed
            let transformer: Arc<PointTransformer> = Arc::new(PointTransformer::boolean(false));

            transformers.insert(key, transformer);
        }

        // Load Adjustment points (linear transformation, supports bidirectional)
        for point in &runtime_config.adjustment_points {
            let key = (channel_id, "A".to_string(), point.base.point_id);

            // Create linear transformer with scale/offset
            let transformer: Arc<PointTransformer> =
                Arc::new(PointTransformer::linear(point.scale, point.offset));

            transformers.insert(key, transformer);
        }

        debug!(
            "Ch{} trans: {} (T:{} S:{} C:{} A:{})",
            channel_id,
            runtime_config.telemetry_points.len()
                + runtime_config.signal_points.len()
                + runtime_config.control_points.len()
                + runtime_config.adjustment_points.len(),
            runtime_config.telemetry_points.len(),
            runtime_config.signal_points.len(),
            runtime_config.control_points.len(),
            runtime_config.adjustment_points.len()
        );
    }

    /// Clear all transformers for a specific channel (for hot reload)
    pub async fn clear_channel_config(&self, channel_id: u32) {
        let mut transformers = self.transformers.write().await;
        transformers.retain(|(ch_id, _, _), _| *ch_id != channel_id);

        debug!("Ch{} transformers cleared", channel_id);
    }

    /// Get statistics
    pub async fn get_stats(&self) -> HashMap<String, usize> {
        let transformers = self.transformers.read().await;

        let mut stats = HashMap::new();
        let mut t_count = 0;
        let mut s_count = 0;
        let mut c_count = 0;
        let mut a_count = 0;

        for ((_ch_id, telemetry_type, _point_id), _) in transformers.iter() {
            match telemetry_type.as_str() {
                "T" => t_count += 1,
                "S" => s_count += 1,
                "C" => c_count += 1,
                "A" => a_count += 1,
                _ => {},
            }
        }

        stats.insert("total".to_string(), transformers.len());
        stats.insert("telemetry".to_string(), t_count);
        stats.insert("signal".to_string(), s_count);
        stats.insert("control".to_string(), c_count);
        stats.insert("adjustment".to_string(), a_count);

        stats
    }
}

impl Default for RuntimeConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeConfigProvider {
    /// Get transformer for a specific point
    ///
    /// # Arguments
    /// * `channel_id` - Channel ID
    /// * `telemetry_type` - Four-telemetry type (T/S/C/A)
    /// * `point_id` - Point ID
    ///
    /// # Returns
    /// Arc to point transformer (returns PassthroughTransformer if not found)
    pub fn get_transformer(
        &self,
        channel_id: u32,
        telemetry_type: &FourRemote,
        point_id: u32,
    ) -> Arc<PointTransformer> {
        // Convert FourRemote to string key
        let type_key = match telemetry_type {
            FourRemote::Telemetry => "T",
            FourRemote::Signal => "S",
            FourRemote::Control => "C",
            FourRemote::Adjustment => "A",
        };

        let key = (channel_id, type_key.to_string(), point_id);

        // Try to read lock and get transformer (non-blocking)
        if let Ok(transformers) = self.transformers.try_read() {
            if let Some(transformer) = transformers.get(&key) {
                return Arc::clone(transformer);
            }
        }

        // If not found or lock failed, return passthrough
        Arc::clone(&self.passthrough)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::core::config::{AdjustmentPoint, Point, SignalPoint, TelemetryPoint};
    use voltage_config::comsrv::ChannelConfig;

    fn create_test_runtime_config() -> RuntimeChannelConfig {
        let base_config = ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
                id: 1001,
                name: "Test Channel".to_string(),
                description: None,
                protocol: "modbus_tcp".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: Default::default(),
        };

        let mut runtime_config = RuntimeChannelConfig::from_base(base_config);

        // Add telemetry point with scale/offset
        runtime_config.telemetry_points.push(TelemetryPoint {
            base: Point {
                point_id: 1,
                signal_name: "Temperature".to_string(),
                description: None,
                unit: Some("°C".to_string()),
            },
            scale: 0.1,
            offset: 0.0,
            data_type: "float32".to_string(),
            reverse: false,
        });

        // Add signal point with reverse
        runtime_config.signal_points.push(SignalPoint {
            base: Point {
                point_id: 2,
                signal_name: "Status".to_string(),
                description: None,
                unit: None,
            },
            reverse: true,
        });

        // Add adjustment point
        runtime_config.adjustment_points.push(AdjustmentPoint {
            base: Point {
                point_id: 3,
                signal_name: "Setpoint".to_string(),
                description: None,
                unit: Some("°C".to_string()),
            },
            min_value: None,
            max_value: None,
            step: 1.0,
            data_type: "float32".to_string(),
            scale: 0.1,
            offset: 0.0,
        });

        runtime_config
    }

    #[tokio::test]
    async fn test_load_channel_config() {
        let provider = RuntimeConfigProvider::new();
        let runtime_config = create_test_runtime_config();

        provider.load_channel_config(&runtime_config).await;

        let stats = provider.get_stats().await;
        assert_eq!(*stats.get("total").unwrap(), 3);
        assert_eq!(*stats.get("telemetry").unwrap(), 1);
        assert_eq!(*stats.get("signal").unwrap(), 1);
        assert_eq!(*stats.get("adjustment").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_get_telemetry_transformer() {
        let provider = RuntimeConfigProvider::new();
        let runtime_config = create_test_runtime_config();
        provider.load_channel_config(&runtime_config).await;

        let transformer = provider.get_transformer(1001, &FourRemote::Telemetry, 1);

        // Test transformation
        use super::super::sync::TransformDirection;
        let result = transformer.transform(6693.0, TransformDirection::DeviceToSystem);
        assert!((result - 669.3).abs() < 0.0001); // Use approximate comparison for floating point
    }

    #[tokio::test]
    async fn test_get_signal_transformer() {
        let provider = RuntimeConfigProvider::new();
        let runtime_config = create_test_runtime_config();
        provider.load_channel_config(&runtime_config).await;

        let transformer = provider.get_transformer(1001, &FourRemote::Signal, 2);

        // Test boolean reversal
        use super::super::sync::TransformDirection;
        assert_eq!(
            transformer.transform(0.0, TransformDirection::DeviceToSystem),
            1.0
        );
        assert_eq!(
            transformer.transform(1.0, TransformDirection::DeviceToSystem),
            0.0
        );
    }

    #[tokio::test]
    async fn test_get_nonexistent_transformer() {
        let provider = RuntimeConfigProvider::new();

        // Should return passthrough transformer
        let transformer = provider.get_transformer(999, &FourRemote::Telemetry, 999);

        use super::super::sync::TransformDirection;
        let result = transformer.transform(123.45, TransformDirection::DeviceToSystem);
        assert_eq!(result, 123.45); // Passthrough
    }

    #[tokio::test]
    async fn test_clear_channel_config() {
        let provider = RuntimeConfigProvider::new();
        let runtime_config = create_test_runtime_config();
        provider.load_channel_config(&runtime_config).await;

        let stats_before = provider.get_stats().await;
        assert_eq!(*stats_before.get("total").unwrap(), 3);

        provider.clear_channel_config(1001).await;

        let stats_after = provider.get_stats().await;
        assert_eq!(*stats_after.get("total").unwrap(), 0);
    }
}
