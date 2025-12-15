//! Telemetry synchronization and data transformation module
//!
//! Handles background synchronization of telemetry data to Redis,
//! and provides unified data transformation interface for all four-telemetry types.

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use super::point_config::RuntimeConfigProvider;
use crate::core::channels::traits::TelemetryBatch;
use crate::error::{ComSrvError, Result};
use crate::storage::PluginPointUpdate;
use common::FourRemote;

// ============================================================================
// Point Transformation (merged from point_transformer.rs)
// ============================================================================

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
                    tracing::warn!("Linear: scale=0, passthrough");
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

// ============================================================================
// Telemetry Synchronization
// ============================================================================

/// Transform telemetry batch using provided config provider
///
/// Internal helper function for code reuse between public API and async task
fn transform_telemetry_batch(
    config_provider: &Arc<RuntimeConfigProvider>,
    telemetry_batch: TelemetryBatch,
) -> Vec<PluginPointUpdate> {
    let channel_id = telemetry_batch.channel_id;
    let mut updates = Vec::new();

    // Process telemetry data with linear transformation
    for (point_id, raw_value, _timestamp) in telemetry_batch.telemetry {
        let transformer =
            config_provider.get_transformer(channel_id, &FourRemote::Telemetry, point_id);

        let processed_value = transformer.transform(raw_value, TransformDirection::DeviceToSystem);

        // Debug log: show raw → value transformation
        debug!(
            "[T] Point {}: raw={:.2} → value={:.2}",
            point_id, raw_value, processed_value
        );

        let update = PluginPointUpdate {
            telemetry_type: crate::core::config::FourRemote::Telemetry,
            point_id,
            value: processed_value,
            raw_value: Some(raw_value),
        };
        updates.push(update);
    }

    // Process signal data with boolean transformation
    for (point_id, raw_value, _timestamp) in telemetry_batch.signal {
        let transformer =
            config_provider.get_transformer(channel_id, &FourRemote::Signal, point_id);

        let processed_value = transformer.transform(raw_value, TransformDirection::DeviceToSystem);

        // Debug log: show raw → value transformation
        debug!(
            "[S] Point {}: raw={:.2} → value={:.2}",
            point_id, raw_value, processed_value
        );

        let update = PluginPointUpdate {
            telemetry_type: crate::core::config::FourRemote::Signal,
            point_id,
            value: processed_value,
            raw_value: Some(raw_value),
        };
        updates.push(update);
    }

    updates
}

/// Telemetry sync manager
pub struct TelemetrySync {
    rtdb: Arc<dyn voltage_rtdb::Rtdb>,
    routing_cache: Arc<voltage_rtdb::RoutingCache>,
    sync_task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    data_receiver: Arc<Mutex<Option<mpsc::Receiver<TelemetryBatch>>>>,
    data_sender: mpsc::Sender<TelemetryBatch>,
    /// Point configuration provider for data transformation
    config_provider: Arc<RuntimeConfigProvider>,
}

impl TelemetrySync {
    /// Create new telemetry sync manager with configuration provider and routing cache
    pub fn new(
        rtdb: Arc<dyn voltage_rtdb::Rtdb>,
        routing_cache: Arc<voltage_rtdb::RoutingCache>,
        config_provider: Arc<RuntimeConfigProvider>,
    ) -> Self {
        let (data_sender, data_receiver) = mpsc::channel(1000);

        Self {
            rtdb,
            routing_cache,
            sync_task_handle: Arc::new(RwLock::new(None)),
            data_receiver: Arc::new(Mutex::new(Some(data_receiver))),
            data_sender,
            config_provider,
        }
    }

    /// Get the data sender for protocols to use
    pub fn get_sender(&self) -> mpsc::Sender<TelemetryBatch> {
        self.data_sender.clone()
    }

    /// Get the sync task handle
    pub fn get_handle(&self) -> Arc<RwLock<Option<JoinHandle<()>>>> {
        self.sync_task_handle.clone()
    }

    /// Transform telemetry batch to plugin point updates
    ///
    /// This method applies configured transformations (scale/offset/reverse) to raw data.
    /// Extracted for reusability and testing.
    pub fn transform_batch(&self, telemetry_batch: TelemetryBatch) -> Vec<PluginPointUpdate> {
        transform_telemetry_batch(&self.config_provider, telemetry_batch)
    }

    /// Start telemetry sync task
    pub async fn start_telemetry_sync_task(&self) -> Result<()> {
        debug!("Sync starting");

        // Take the receiver from the manager
        let receiver = {
            let mut receiver_opt = self.data_receiver.lock().await;
            receiver_opt.take()
        };

        let Some(mut receiver) = receiver else {
            return Err(ComSrvError::InvalidOperation(
                "Data receiver already taken".to_string(),
            ));
        };

        // Clone necessary references for the task
        let rtdb = Arc::clone(&self.rtdb);
        let routing_cache = Arc::clone(&self.routing_cache);
        let sync_handle = self.sync_task_handle.clone();
        let config_provider = Arc::clone(&self.config_provider);

        // Spawn the telemetry sync task
        let task_handle = tokio::spawn(async move {
            debug!("Sync task running");

            // Create storage manager from existing rtdb and routing cache
            let storage = crate::storage::StorageManager::from_rtdb(rtdb, routing_cache);

            // Start the WriteBuffer flush task (runs in background)
            let _flush_handle = storage.start_flush_task();
            info!(
                "WriteBuffer flush task started (interval: {}ms)",
                storage.write_buffer().config().flush_interval_ms
            );

            while let Some(telemetry_batch) = receiver.recv().await {
                let channel_id = telemetry_batch.channel_id;

                // Transform batch using shared logic
                let updates = transform_telemetry_batch(&config_provider, telemetry_batch);

                // Batch update if there are updates (now buffered, not direct to Redis)
                if !updates.is_empty() {
                    if let Err(e) = storage.batch_update_and_publish(channel_id, updates).await {
                        error!("Ch{} sync err: {}", channel_id, e);
                    }
                }
            }

            // Graceful shutdown: flush any remaining buffered writes
            debug!("Sync task ending, flushing remaining writes...");
            match storage.shutdown().await {
                Ok(flushed) => {
                    if flushed > 0 {
                        info!("WriteBuffer final flush: {} fields", flushed);
                    }
                },
                Err(e) => {
                    error!("WriteBuffer final flush failed: {}", e);
                },
            }

            debug!("Sync task ended");
        });

        // Store the task handle
        let mut handle = sync_handle.write().await;
        *handle = Some(task_handle);

        info!("Sync started");

        Ok(())
    }

    /// Stop telemetry sync task
    pub async fn stop_telemetry_sync_task(&self) -> Result<()> {
        let mut handle = self.sync_task_handle.write().await;
        if let Some(task_handle) = handle.take() {
            debug!("Sync stopping");
            task_handle.abort();
            debug!("Sync stopped");
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::core::channels::point_config::RuntimeConfigProvider;
    use crate::core::config::ChannelConfig;
    use crate::core::config::RuntimeChannelConfig;
    use crate::core::config::{Point, SignalPoint, TelemetryPoint};
    use std::collections::HashMap;

    use voltage_rtdb::helpers::create_test_rtdb;

    fn create_test_runtime_config() -> RuntimeChannelConfig {
        let base_config = ChannelConfig {
            core: crate::core::config::ChannelCore {
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

        // Add telemetry point: scale=0.1, offset=0.0
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

        // Add signal point: reverse=true
        runtime_config.signal_points.push(SignalPoint {
            base: Point {
                point_id: 2,
                signal_name: "Status".to_string(),
                description: None,
                unit: None,
            },
            reverse: true,
        });

        runtime_config
    }

    #[tokio::test]
    async fn test_transform_telemetry_linear() {
        // Setup
        let config_provider = Arc::new(RuntimeConfigProvider::new());
        let runtime_config = create_test_runtime_config();
        config_provider.load_channel_config(&runtime_config).await;

        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let sync = TelemetrySync::new(rtdb, routing_cache, config_provider);

        // Create test batch with raw value
        let batch = TelemetryBatch {
            channel_id: 1001,
            telemetry: vec![(1, 6693.0, 0)],
            signal: vec![],
        };

        // Transform (calls real code)
        let updates = sync.transform_batch(batch);

        // Verify transformation
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].point_id, 1);
        assert!((updates[0].value - 669.3).abs() < 0.0001); // 6693.0 * 0.1 (floating point)
        assert_eq!(updates[0].raw_value, Some(6693.0));
    }

    #[tokio::test]
    async fn test_transform_signal_reverse() {
        // Setup
        let config_provider = Arc::new(RuntimeConfigProvider::new());
        let runtime_config = create_test_runtime_config();
        config_provider.load_channel_config(&runtime_config).await;

        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let sync = TelemetrySync::new(rtdb, routing_cache, config_provider);

        // Create test batch with signal data
        let batch = TelemetryBatch {
            channel_id: 1001,
            telemetry: vec![],
            signal: vec![(2, 0.0, 0)], // raw = 0, reverse=true
        };

        // Transform
        let updates = sync.transform_batch(batch);

        // Verify boolean reversal
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].value, 1.0); // reversed from 0
        assert_eq!(updates[0].raw_value, Some(0.0));
    }

    #[tokio::test]
    async fn test_transform_batch_mixed() {
        // Setup
        let config_provider = Arc::new(RuntimeConfigProvider::new());
        let runtime_config = create_test_runtime_config();
        config_provider.load_channel_config(&runtime_config).await;

        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let sync = TelemetrySync::new(rtdb, routing_cache, config_provider);

        // Create batch with both telemetry and signal
        let batch = TelemetryBatch {
            channel_id: 1001,
            telemetry: vec![(1, 5000.0, 0)], // → 500.0
            signal: vec![(2, 1.0, 0)],       // → 0.0 (reversed)
        };

        // Transform
        let updates = sync.transform_batch(batch);

        // Verify both types processed correctly
        assert_eq!(updates.len(), 2);

        // Verify telemetry
        let telemetry_update = &updates[0];
        assert_eq!(telemetry_update.value, 500.0);
        assert_eq!(telemetry_update.raw_value, Some(5000.0));

        // Verify signal
        let signal_update = &updates[1];
        assert_eq!(signal_update.value, 0.0);
        assert_eq!(signal_update.raw_value, Some(1.0));
    }

    #[tokio::test]
    async fn test_transform_with_offset() {
        // Setup with scale=0.1, offset=10.0
        let config_provider = Arc::new(RuntimeConfigProvider::new());

        let base_config = ChannelConfig {
            core: crate::core::config::ChannelCore {
                id: 1002,
                name: "Test Channel 2".to_string(),
                description: None,
                protocol: "modbus_tcp".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: Default::default(),
        };

        let mut runtime_config = RuntimeChannelConfig::from_base(base_config);
        runtime_config.telemetry_points.push(TelemetryPoint {
            base: Point {
                point_id: 1,
                signal_name: "Pressure".to_string(),
                description: None,
                unit: Some("kPa".to_string()),
            },
            scale: 0.1,
            offset: 10.0, // with offset
            data_type: "float32".to_string(),
            reverse: false,
        });

        config_provider.load_channel_config(&runtime_config).await;

        let rtdb = create_test_rtdb();
        let routing_cache = Arc::new(voltage_rtdb::RoutingCache::new());
        let sync = TelemetrySync::new(rtdb, routing_cache, config_provider);

        // Create test batch
        let batch = TelemetryBatch {
            channel_id: 1002,
            telemetry: vec![(1, 1000.0, 0)],
            signal: vec![],
        };

        // Transform
        let updates = sync.transform_batch(batch);

        // Verify: 1000 * 0.1 + 10 = 110.0
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].value, 110.0);
        assert_eq!(updates[0].raw_value, Some(1000.0));
    }
}
