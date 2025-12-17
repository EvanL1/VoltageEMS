//! DI/DO Protocol - Service Layer Implementation
//!
//! Integrates the low-level `voltage_protocols::dido` module with comsrv's
//! `ComClient` trait, providing ChannelLogger, polling management, and
//! runtime configuration handling.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::core::channels::traits::{ChannelLogger, ConnectionState};
use crate::core::channels::{
    ChannelStatus, ComBase, ComClient, PointData, PointDataMap, ProtocolValue,
};
use crate::core::config::{FourRemote, RuntimeChannelConfig};
use crate::error::Result;

// Import from voltage-protocols library
use voltage_protocols::dido::{DiDoController, GpioPoint};

/// DI/DO protocol client - service layer wrapper
pub struct DiDoProtocol {
    name: Arc<str>,
    channel_id: u32,
    connection_state: Arc<RwLock<ConnectionState>>,
    logger: ChannelLogger,

    // Low-level controller from voltage-protocols
    controller: Arc<RwLock<DiDoController>>,

    // Polling configuration
    poll_interval_ms: u64,

    // Polling task management
    poll_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    shutdown_tx: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl DiDoProtocol {
    /// Create from RuntimeChannelConfig
    pub fn from_runtime_config(runtime_config: &RuntimeChannelConfig) -> Result<Self> {
        let channel_id = runtime_config.id();
        let name = runtime_config.name().to_string();

        let logger = ChannelLogger::new(channel_id, name.clone());

        // Parse parameters from base.parameters
        let gpio_base_path = runtime_config
            .base
            .parameters
            .get("gpio_base_path")
            .and_then(|v| v.as_str())
            .unwrap_or("/sys/class/gpio");

        let poll_interval_ms = runtime_config
            .base
            .parameters
            .get("di_poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(200);

        logger.log_init(
            "DI/DO",
            &format!(
                "Creating DI/DO protocol: gpio_base={}, poll_interval={}ms",
                gpio_base_path, poll_interval_ms
            ),
        );

        // Create low-level controller
        let controller = DiDoController::with_sysfs(gpio_base_path);

        Ok(Self {
            name: name.into(),
            channel_id,
            connection_state: Arc::new(RwLock::new(ConnectionState::Uninitialized)),
            logger,
            controller: Arc::new(RwLock::new(controller)),
            poll_interval_ms,
            poll_handle: Arc::new(RwLock::new(None)),
            shutdown_tx: Arc::new(RwLock::new(None)),
        })
    }

    /// Start background polling task
    async fn start_polling(&self) {
        let controller = Arc::clone(&self.controller);
        let poll_interval = self.poll_interval_ms;
        let channel_id = self.channel_id;

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        let handle = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_millis(poll_interval));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        controller.read().await.poll_di().await;
                    }
                    _ = &mut shutdown_rx => {
                        debug!("DI/DO channel {} polling stopped", channel_id);
                        break;
                    }
                }
            }
        });

        *self.poll_handle.write().await = Some(handle);
    }

    /// Stop background polling task
    async fn stop_polling(&self) {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
        }

        if let Some(handle) = self.poll_handle.write().await.take() {
            handle.abort();
        }
    }
}

impl std::fmt::Debug for DiDoProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiDoProtocol")
            .field("name", &self.name)
            .field("channel_id", &self.channel_id)
            .finish()
    }
}

#[async_trait]
impl ComBase for DiDoProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn get_channel_id(&self) -> u32 {
        self.channel_id
    }

    async fn get_status(&self) -> ChannelStatus {
        let state = *self.connection_state.read().await;
        ChannelStatus {
            is_connected: matches!(state, ConnectionState::Connected),
            last_update: chrono::Utc::now().timestamp(),
        }
    }

    async fn initialize(&mut self, runtime_config: Arc<RuntimeChannelConfig>) -> Result<()> {
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Initializing;
        }

        self.logger.log_init(
            "DI/DO",
            &format!(
                "Initializing DI/DO protocol for channel {}",
                self.channel_id
            ),
        );

        let mut controller = self.controller.write().await;

        // Load Signal points (DI) from gpio_mappings where telemetry_type = "S"
        for mapping in runtime_config
            .gpio_mappings
            .iter()
            .filter(|m| m.telemetry_type == "S")
        {
            // Get reverse flag from signal_points (DI only)
            let reverse = runtime_config
                .signal_points
                .iter()
                .find(|p| p.base.point_id == mapping.point_id)
                .map(|p| p.reverse)
                .unwrap_or(false);

            controller.add_di_point(GpioPoint {
                point_id: mapping.point_id,
                gpio_number: mapping.gpio_number,
                reverse,
            });
        }

        // Load Control points (DO) from gpio_mappings where telemetry_type = "C"
        for mapping in runtime_config
            .gpio_mappings
            .iter()
            .filter(|m| m.telemetry_type == "C")
        {
            controller.add_do_point(GpioPoint {
                point_id: mapping.point_id,
                gpio_number: mapping.gpio_number,
                reverse: false, // DO outputs don't use reverse logic
            });
        }

        info!(
            "DI/DO channel {} loaded {} DI points, {} DO points",
            self.channel_id,
            controller.di_count(),
            controller.do_count()
        );

        self.logger.log_config(
            "DI/DO",
            "points",
            &format!(
                "DI: {}, DO: {}",
                controller.di_count(),
                controller.do_count()
            ),
        );

        Ok(())
    }

    async fn read_four_telemetry(&self, telemetry_type: FourRemote) -> Result<PointDataMap> {
        let mut result = std::collections::HashMap::new();
        let timestamp = chrono::Utc::now().timestamp();
        let controller = self.controller.read().await;

        match telemetry_type {
            FourRemote::Signal => {
                // Read DI cache
                let cache = controller.read_di_cache().await;
                for (point_id, value) in cache {
                    result.insert(
                        point_id,
                        PointData {
                            value: ProtocolValue::Bool(value),
                            timestamp,
                        },
                    );
                }
            },
            FourRemote::Control => {
                // Read DO cache
                let cache = controller.read_do_cache().await;
                for (point_id, value) in cache {
                    result.insert(
                        point_id,
                        PointData {
                            value: ProtocolValue::Bool(value),
                            timestamp,
                        },
                    );
                }
            },
            _ => {
                // Telemetry and Adjustment not supported for DI/DO
            },
        }

        Ok(result)
    }
}

#[async_trait]
impl ComClient for DiDoProtocol {
    fn is_connected(&self) -> bool {
        // DI/DO is "connected" as long as GPIO driver is available
        // Check synchronously via a simple flag or assume available
        true
    }

    async fn connect(&mut self) -> Result<()> {
        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connecting;
            self.logger
                .log_status(old_state, ConnectionState::Connecting, "Starting DI/DO");
        }

        // Check GPIO availability
        let controller = self.controller.read().await;
        if !controller.is_available() {
            warn!(
                "GPIO driver not available, running in simulation mode for channel {}",
                self.channel_id
            );
        }
        drop(controller);

        // Start DI polling
        self.start_polling().await;

        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connected;
            self.logger.log_status(
                old_state,
                ConnectionState::Connected,
                "DI/DO protocol ready",
            );
        }

        info!(
            "DI/DO channel {} connected, polling every {}ms",
            self.channel_id, self.poll_interval_ms
        );

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Closed;
            self.logger
                .log_status(old_state, ConnectionState::Closed, "Stopping DI/DO");
        }

        // Stop polling
        self.stop_polling().await;

        info!("DI/DO channel {} disconnected", self.channel_id);

        Ok(())
    }

    async fn control(&mut self, commands: Vec<(u32, ProtocolValue)>) -> Result<Vec<(u32, bool)>> {
        let mut results = Vec::new();
        let controller = self.controller.read().await;

        for (point_id, value) in commands {
            // Convert value to bool
            let bool_value = match value {
                ProtocolValue::Bool(b) => b,
                ProtocolValue::Integer(i) => i != 0,
                ProtocolValue::Float(f) => f != 0.0,
                _ => false,
            };

            // Write to GPIO via controller
            match controller.write_do(point_id, bool_value).await {
                Ok(()) => {
                    results.push((point_id, true));
                },
                Err(e) => {
                    warn!("Control failed for point {}: {}", point_id, e);
                    results.push((point_id, false));
                },
            }
        }

        self.logger.log_protocol_message(
            "CONTROL",
            &[],
            &format!(
                "Executed {} DO commands",
                results.iter().filter(|(_, ok)| *ok).count()
            ),
        );

        Ok(results)
    }

    async fn adjustment(
        &mut self,
        _adjustments: Vec<(u32, ProtocolValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        // DI/DO protocol does not support adjustment (analog output)
        warn!("DI/DO protocol does not support adjustment commands");
        Ok(vec![])
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use crate::core::config::{ChannelConfig, ChannelCore};
    use std::collections::HashMap;

    fn create_test_runtime_config(id: u32) -> RuntimeChannelConfig {
        let mut parameters = HashMap::new();
        parameters.insert(
            "gpio_base_path".to_string(),
            serde_json::json!("/tmp/test_gpio"),
        );
        parameters.insert("di_poll_interval_ms".to_string(), serde_json::json!(100));

        let config = ChannelConfig {
            core: ChannelCore {
                id,
                name: format!("Test DI/DO {}", id),
                description: Some("Test DI/DO protocol".to_string()),
                protocol: "di_do".to_string(),
                enabled: true,
            },
            parameters,
            logging: Default::default(),
        };

        RuntimeChannelConfig::from_base(config)
    }

    #[tokio::test]
    async fn test_dido_protocol_creation() {
        let runtime_config = create_test_runtime_config(100);
        let protocol = DiDoProtocol::from_runtime_config(&runtime_config);

        assert!(protocol.is_ok());
        let protocol = protocol.unwrap();
        assert_eq!(protocol.name(), "Test DI/DO 100");
        assert_eq!(protocol.get_channel_id(), 100);
    }

    #[tokio::test]
    async fn test_dido_basic_flow() {
        let runtime_config = create_test_runtime_config(100);
        let mut protocol = DiDoProtocol::from_runtime_config(&runtime_config).unwrap();

        // Initialize (will work even without GPIO hardware)
        let result = protocol.initialize(Arc::new(runtime_config)).await;
        assert!(result.is_ok());

        // Connect (will warn about missing GPIO but continue)
        let result = protocol.connect().await;
        assert!(result.is_ok());

        // Read signal data (empty since no points configured)
        let signals = protocol
            .read_four_telemetry(FourRemote::Signal)
            .await
            .unwrap();
        assert!(signals.is_empty());

        // Disconnect
        let result = protocol.disconnect().await;
        assert!(result.is_ok());
    }

    /// Create test config with GPIO mappings
    fn create_test_config_with_mappings(id: u32) -> RuntimeChannelConfig {
        use crate::core::config::{ControlPoint, GpioMapping, Point, SignalPoint};

        let mut config = create_test_runtime_config(id);

        // Add Signal points (DI)
        config.signal_points.push(SignalPoint {
            base: Point {
                point_id: 1,
                signal_name: "DI1_RunStatus".to_string(),
                description: Some("Running status indicator".to_string()),
                unit: None,
            },
            reverse: false,
        });
        config.signal_points.push(SignalPoint {
            base: Point {
                point_id: 2,
                signal_name: "DI2_FaultStatus".to_string(),
                description: Some("Fault status indicator".to_string()),
                unit: None,
            },
            reverse: true, // with reverse logic
        });

        // Add Control points (DO)
        config.control_points.push(ControlPoint {
            base: Point {
                point_id: 1,
                signal_name: "DO1_StartCmd".to_string(),
                description: Some("Start command output".to_string()),
                unit: None,
            },
            reverse: false,
            control_type: "momentary".to_string(),
            on_value: 1,
            off_value: 0,
            pulse_duration_ms: None,
        });

        // Add GPIO mappings
        config.gpio_mappings.push(GpioMapping {
            channel_id: id,
            point_id: 1,
            telemetry_type: "S".to_string(),
            gpio_number: 496,
        });
        config.gpio_mappings.push(GpioMapping {
            channel_id: id,
            point_id: 2,
            telemetry_type: "S".to_string(),
            gpio_number: 497,
        });
        config.gpio_mappings.push(GpioMapping {
            channel_id: id,
            point_id: 1,
            telemetry_type: "C".to_string(),
            gpio_number: 504,
        });

        config
    }

    #[tokio::test]
    async fn test_read_four_telemetry_signal() {
        let runtime_config = create_test_config_with_mappings(4);
        let mut protocol = DiDoProtocol::from_runtime_config(&runtime_config).unwrap();

        // Initialize - this loads GPIO mappings
        let result = protocol.initialize(Arc::new(runtime_config)).await;
        assert!(result.is_ok());

        // Connect (succeeds even without GPIO, enters simulation mode)
        let result = protocol.connect().await;
        assert!(result.is_ok());

        // Read Signal data - may be empty or have default values in simulation mode
        let signals = protocol
            .read_four_telemetry(FourRemote::Signal)
            .await
            .unwrap();

        // Signal data type should be Bool
        for point_data in signals.values() {
            match &point_data.value {
                ProtocolValue::Bool(_) => {},
                _ => panic!("Signal value should be Bool type"),
            }
        }

        protocol.disconnect().await.unwrap();
    }

    #[tokio::test]
    async fn test_read_four_telemetry_control() {
        let runtime_config = create_test_config_with_mappings(4);
        let mut protocol = DiDoProtocol::from_runtime_config(&runtime_config).unwrap();

        protocol.initialize(Arc::new(runtime_config)).await.unwrap();
        protocol.connect().await.unwrap();

        // Read Control data (DO status)
        let controls = protocol
            .read_four_telemetry(FourRemote::Control)
            .await
            .unwrap();

        // Control data type should be Bool
        for point_data in controls.values() {
            match &point_data.value {
                ProtocolValue::Bool(_) => {},
                _ => panic!("Control value should be Bool type"),
            }
        }

        protocol.disconnect().await.unwrap();
    }

    #[tokio::test]
    async fn test_control_command_execution() {
        let runtime_config = create_test_config_with_mappings(4);
        let mut protocol = DiDoProtocol::from_runtime_config(&runtime_config).unwrap();

        protocol.initialize(Arc::new(runtime_config)).await.unwrap();
        protocol.connect().await.unwrap();

        // Execute control command
        // Note: command may fail without GPIO but should not panic
        let results = protocol
            .control(vec![(1, ProtocolValue::Bool(true))])
            .await
            .unwrap();

        // Result should contain point ID
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
        // May fail in simulation environment, which is expected

        protocol.disconnect().await.unwrap();
    }
}
