//! Virtual Protocol Implementation
//!
//! Provides a virtual protocol for testing purposes without requiring actual hardware.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::channels::traits::{ChannelLogger, ConnectionState};
use crate::core::channels::{
    ChannelStatus, ComBase, ComClient, PointData, PointDataMap, ProtocolValue,
};
#[cfg(test)]
use crate::core::config::ChannelConfig;
use crate::core::config::{FourRemote, RuntimeChannelConfig};
use crate::error::Result;

/// Virtual protocol client for testing
pub struct VirtualProtocol {
    name: Arc<str>,
    channel_id: u16,
    running: Arc<RwLock<bool>>,
    connection_state: Arc<RwLock<ConnectionState>>,

    // Channel logger for unified logging
    logger: ChannelLogger,

    // Simulated data storage
    telemetry_data: Arc<RwLock<Vec<f64>>>,
    signal_data: Arc<RwLock<Vec<bool>>>,
}

impl VirtualProtocol {
    pub fn new(channel_config: crate::core::config::ChannelConfig) -> Result<Self> {
        let logger = ChannelLogger::new(
            channel_config.id() as u32,
            channel_config.name().to_string(),
        );

        logger.log_init(
            "Virtual",
            &format!(
                "Creating virtual protocol for channel {}",
                channel_config.id()
            ),
        );

        Ok(Self {
            name: channel_config.name().into(),
            channel_id: channel_config.id(),
            running: Arc::new(RwLock::new(false)),
            connection_state: Arc::new(RwLock::new(ConnectionState::Uninitialized)),
            logger,
            telemetry_data: Arc::new(RwLock::new(vec![0.0; 100])),
            signal_data: Arc::new(RwLock::new(vec![false; 100])),
        })
    }

    /// Create from RuntimeChannelConfig
    pub fn from_runtime_config(
        runtime_config: &crate::core::config::RuntimeChannelConfig,
    ) -> Result<Self> {
        // VirtualProtocol doesn't need protocol-specific parameters
        // Just extract the base config and use existing new() method
        let channel_config = (*runtime_config.base).clone();
        Self::new(channel_config)
    }
}

impl std::fmt::Debug for VirtualProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualProtocol")
            .field("name", &self.name)
            .field("channel_id", &self.channel_id)
            .field("running", &self.running)
            .finish()
    }
}

#[async_trait]
impl ComBase for VirtualProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn get_channel_id(&self) -> u16 {
        self.channel_id
    }

    async fn get_status(&self) -> ChannelStatus {
        ChannelStatus {
            is_connected: true,
            last_update: chrono::Utc::now().timestamp(),
        }
    }

    async fn initialize(&mut self, runtime_config: Arc<RuntimeChannelConfig>) -> Result<()> {
        // Update connection state
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Initializing;
        }

        self.logger.log_init(
            "Virtual",
            &format!(
                "Initializing virtual protocol for channel {}",
                runtime_config.id()
            ),
        );

        // Virtual protocol doesn't need to load points - it simulates data
        // Storage is now handled directly through Redis
        self.logger.log_config(
            "Virtual",
            "simulated_points",
            "200 (100 telemetry + 100 signal)",
        );

        Ok(())
    }

    async fn read_four_telemetry(&self, telemetry_type: FourRemote) -> Result<PointDataMap> {
        let mut result = std::collections::HashMap::new();
        let timestamp = chrono::Utc::now().timestamp();

        match telemetry_type {
            FourRemote::Telemetry => {
                let data = self.telemetry_data.read().await;
                for (i, value) in data.iter().enumerate() {
                    result.insert(
                        i as u32,
                        PointData {
                            value: ProtocolValue::Float(*value),
                            timestamp,
                        },
                    );
                }
            },
            FourRemote::Signal => {
                let data = self.signal_data.read().await;
                for (i, value) in data.iter().enumerate() {
                    result.insert(
                        i as u32,
                        PointData {
                            value: ProtocolValue::Bool(*value),
                            timestamp,
                        },
                    );
                }
            },
            _ => {},
        }

        Ok(result)
    }
}

#[async_trait]
impl ComClient for VirtualProtocol {
    fn is_connected(&self) -> bool {
        true // Virtual protocol always connected
    }

    async fn connect(&mut self) -> Result<()> {
        // Update connection state
        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connecting;
            self.logger.log_status(
                old_state,
                ConnectionState::Connecting,
                "Starting virtual connection",
            );
        }

        self.logger.log_connect(
            "Virtual",
            "simulation",
            &format!("Channel {} activating virtual protocol", self.channel_id),
        );

        *self.running.write().await = true;

        // Virtual protocol is always "connected" instantly
        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connected;
            self.logger.log_status(
                old_state,
                ConnectionState::Connected,
                "Virtual connection established",
            );
        }

        // Virtual channels keep static data and no longer generate sinusoids or random signals.
        // Telemetry defaults to 0 and signals default to false; external control writes update them.
        self.logger.log_init(
            "Virtual",
            "Virtual protocol ready - static data mode enabled",
        );

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        // Update connection state
        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Closed;
            self.logger.log_status(
                old_state,
                ConnectionState::Closed,
                "Stopping virtual protocol",
            );
        }

        *self.running.write().await = false;

        self.logger.log_init(
            "Virtual",
            &format!("Channel {} virtual protocol stopped", self.channel_id),
        );

        Ok(())
    }

    async fn control(&mut self, commands: Vec<(u32, ProtocolValue)>) -> Result<Vec<(u32, bool)>> {
        let mut results = Vec::new();
        let mut signals = self.signal_data.write().await;

        for (point_id, value) in &commands {
            if *point_id > 0 && *point_id <= signals.len() as u32 {
                let idx = (*point_id - 1) as usize;
                signals[idx] = match value {
                    ProtocolValue::Bool(b) => *b,
                    ProtocolValue::Integer(i) => *i != 0,
                    ProtocolValue::Float(f) => *f != 0.0,
                    _ => false,
                };
                results.push((*point_id, true));
            } else {
                results.push((*point_id, false));
            }
        }

        // Log control commands
        self.logger.log_protocol_message(
            "CONTROL",
            &[],
            &format!(
                "Executed {} control commands to virtual device",
                commands.len()
            ),
        );

        Ok(results)
    }

    async fn adjustment(
        &mut self,
        adjustments: Vec<(u32, ProtocolValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        let mut results = Vec::new();
        let mut data = self.telemetry_data.write().await;

        for (point_id, value) in adjustments {
            if point_id > 0 && point_id <= data.len() as u32 {
                let idx = (point_id - 1) as usize;
                data[idx] = match value {
                    ProtocolValue::Float(f) => f,
                    ProtocolValue::Integer(i) => i as f64,
                    ProtocolValue::String(s) => s.parse().unwrap_or(0.0),
                    _ => 0.0,
                };
                results.push((point_id, true));
            } else {
                results.push((point_id, false));
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use std::borrow::Cow;
    use std::collections::HashMap;

    fn create_test_channel_config(id: u16) -> ChannelConfig {
        ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
                id,
                name: format!("Test Channel {}", id),
                description: Some("Test virtual protocol".to_string()),
                protocol: "virtual".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_virtual_protocol_creation() {
        let config = create_test_channel_config(1);
        let protocol = VirtualProtocol::new(config);

        assert!(protocol.is_ok());
        let protocol = protocol.unwrap();
        assert_eq!(protocol.name(), "Test Channel 1");
        assert_eq!(protocol.get_channel_id(), 1);
    }

    #[tokio::test]
    async fn test_virtual_protocol_basic_flow() {
        let config = create_test_channel_config(1);
        let mut protocol = VirtualProtocol::new(config).unwrap();

        // Test connection
        assert!(protocol.connect().await.is_ok());
        assert!(protocol.is_connected());

        // Test reading telemetry
        let telemetry = protocol
            .read_four_telemetry(FourRemote::Telemetry)
            .await
            .expect("telemetry read should succeed");
        assert_eq!(telemetry.len(), 100);

        // Test control
        let commands = vec![(1, ProtocolValue::Bool(true))];
        let results = protocol.control(commands).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].1);

        // Test disconnection
        assert!(protocol.disconnect().await.is_ok());
    }

    #[tokio::test]
    async fn test_read_signal_data() {
        let config = create_test_channel_config(2);
        let mut protocol = VirtualProtocol::new(config).unwrap();
        protocol.connect().await.unwrap();

        // Read signal data
        let signals = protocol
            .read_four_telemetry(FourRemote::Signal)
            .await
            .unwrap();

        assert_eq!(signals.len(), 100);

        // Verify all signals are initially false
        for (_point_id, point_data) in signals.iter() {
            match point_data.value {
                ProtocolValue::Bool(b) => assert!(!b),
                _ => panic!("Expected bool value"),
            }
        }
    }

    #[tokio::test]
    async fn test_adjustment_commands() {
        let config = create_test_channel_config(3);
        let mut protocol = VirtualProtocol::new(config).unwrap();
        protocol.connect().await.unwrap();

        // Test adjustment with float value
        let adjustments = vec![
            (1, ProtocolValue::Float(123.45)),
            (2, ProtocolValue::Integer(100)),
            (3, ProtocolValue::String(Cow::Borrowed("99.99"))),
        ];

        let results = protocol.adjustment(adjustments).await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|(_, success)| *success));

        // Verify the adjusted values (note: read returns 0-based keys)
        let telemetry = protocol
            .read_four_telemetry(FourRemote::Telemetry)
            .await
            .unwrap();
        match telemetry.get(&0).unwrap().value {
            // point_id 1 → key 0
            ProtocolValue::Float(f) => assert!((f - 123.45).abs() < 0.001),
            _ => panic!("Expected float value"),
        }
        match telemetry.get(&1).unwrap().value {
            // point_id 2 → key 1
            ProtocolValue::Float(f) => assert!((f - 100.0).abs() < 0.001),
            _ => panic!("Expected float value"),
        }
        match telemetry.get(&2).unwrap().value {
            // point_id 3 → key 2
            ProtocolValue::Float(f) => assert!((f - 99.99).abs() < 0.001),
            _ => panic!("Expected float value"),
        }
    }

    #[tokio::test]
    async fn test_control_with_different_value_types() {
        let config = create_test_channel_config(4);
        let mut protocol = VirtualProtocol::new(config).unwrap();
        protocol.connect().await.unwrap();

        // Test control with different ProtocolValue types
        let commands = vec![
            (1, ProtocolValue::Bool(true)),
            (2, ProtocolValue::Integer(1)),
            (3, ProtocolValue::Float(1.5)),
            (4, ProtocolValue::Integer(0)),
        ];

        let results = protocol.control(commands).await.unwrap();
        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|(_, success)| *success));

        // Verify signal states (note: read returns 0-based keys)
        let signals = protocol
            .read_four_telemetry(FourRemote::Signal)
            .await
            .unwrap();
        assert!(matches!(
            signals.get(&0).unwrap().value, // point_id 1 → key 0
            ProtocolValue::Bool(true)
        ));
        assert!(matches!(
            signals.get(&1).unwrap().value, // point_id 2 → key 1
            ProtocolValue::Bool(true)
        ));
        assert!(matches!(
            signals.get(&2).unwrap().value, // point_id 3 → key 2
            ProtocolValue::Bool(true)
        ));
        assert!(matches!(
            signals.get(&3).unwrap().value, // point_id 4 → key 3
            ProtocolValue::Bool(false)
        ));
    }

    #[tokio::test]
    async fn test_invalid_point_id_boundaries() {
        let config = create_test_channel_config(5);
        let mut protocol = VirtualProtocol::new(config).unwrap();
        protocol.connect().await.unwrap();

        // Test control with invalid point IDs
        let commands = vec![
            (0, ProtocolValue::Bool(true)),    // point_id 0 (invalid)
            (101, ProtocolValue::Bool(true)),  // point_id > 100 (out of range)
            (1000, ProtocolValue::Bool(true)), // way out of range
        ];

        let results = protocol.control(commands).await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|(_, success)| !*success)); // All should fail

        // Test adjustment with invalid point IDs
        let adjustments = vec![
            (0, ProtocolValue::Float(100.0)),
            (101, ProtocolValue::Float(200.0)),
        ];

        let results = protocol.adjustment(adjustments).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, success)| !*success)); // All should fail
    }

    #[tokio::test]
    async fn test_status_query() {
        let config = create_test_channel_config(6);
        let protocol = VirtualProtocol::new(config).unwrap();

        let status = protocol.get_status().await;

        assert!(status.is_connected);
        assert!(status.last_update > 0);
    }

    #[tokio::test]
    async fn test_initialization() {
        let config = create_test_channel_config(7);
        let mut protocol = VirtualProtocol::new(config.clone()).unwrap();

        let runtime_config = Arc::new(RuntimeChannelConfig::from_base(config));

        let result = protocol.initialize(runtime_config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_read_unsupported_telemetry_type() {
        let config = create_test_channel_config(8);
        let mut protocol = VirtualProtocol::new(config).unwrap();
        protocol.connect().await.unwrap();

        // Test reading control/adjustment types (should return empty)
        let control = protocol
            .read_four_telemetry(FourRemote::Control)
            .await
            .unwrap();
        assert_eq!(control.len(), 0);

        let adjustment = protocol
            .read_four_telemetry(FourRemote::Adjustment)
            .await
            .unwrap();
        assert_eq!(adjustment.len(), 0);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let config = create_test_channel_config(9);
        let protocol = Arc::new(tokio::sync::RwLock::new(
            VirtualProtocol::new(config).unwrap(),
        ));

        // Connect first
        protocol.write().await.connect().await.unwrap();

        // Spawn multiple concurrent read operations
        let mut handles = vec![];
        for _ in 0..10 {
            let protocol_clone = Arc::clone(&protocol);
            handles.push(tokio::spawn(async move {
                let proto = protocol_clone.read().await;
                proto.read_four_telemetry(FourRemote::Telemetry).await
            }));
        }

        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 100);
        }
    }

    #[tokio::test]
    async fn test_adjustment_string_parsing_errors() {
        let config = create_test_channel_config(10);
        let mut protocol = VirtualProtocol::new(config).unwrap();
        protocol.connect().await.unwrap();

        // Test adjustment with invalid string
        let adjustments = vec![(1, ProtocolValue::String(Cow::Borrowed("not_a_number")))];

        let results = protocol.adjustment(adjustments).await.unwrap();
        assert!(results[0].1); // Should succeed but value becomes 0.0

        // Verify the value is 0.0 (note: read returns 0-based keys)
        let telemetry = protocol
            .read_four_telemetry(FourRemote::Telemetry)
            .await
            .unwrap();
        match telemetry.get(&0).unwrap().value {
            // point_id 1 → key 0
            ProtocolValue::Float(f) => assert_eq!(f, 0.0),
            _ => panic!("Expected float value"),
        }
    }

    #[tokio::test]
    async fn test_multiple_connect_disconnect_cycles() {
        let config = create_test_channel_config(11);
        let mut protocol = VirtualProtocol::new(config).unwrap();

        // Test multiple connect/disconnect cycles
        for _ in 0..5 {
            assert!(protocol.connect().await.is_ok());
            assert!(protocol.is_connected());
            assert!(protocol.disconnect().await.is_ok());
        }
    }
}
