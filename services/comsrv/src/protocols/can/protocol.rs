//! Simplified CAN Protocol Implementation for initial compilation
//!
//! This is a minimal implementation to get the CAN plugin compiling.
//! Full features will be added incrementally.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::core::combase::traits::{
    ChannelLogger, ChannelStatus, ComBase, ComClient, ConnectionState, PointDataMap, RedisValue,
};
use crate::core::config::{ChannelConfig, FourRemote, RuntimeChannelConfig};
use crate::utils::error::{ComSrvError, Result};

use super::transport::CanTransport;
use super::types::{CanConfig, CanMappingCollection};

/// Simplified CAN Protocol implementation
pub struct CanProtocol {
    channel_config: Arc<ChannelConfig>,
    _can_config: CanConfig,
    transport: Arc<RwLock<CanTransport>>,
    is_connected: Arc<AtomicBool>,
    status: Arc<RwLock<ChannelStatus>>,
    connection_state: Arc<RwLock<ConnectionState>>,

    // Channel logger for unified logging
    logger: ChannelLogger,

    // Data storage
    telemetry_data: Arc<RwLock<PointDataMap>>,
    signal_data: Arc<RwLock<PointDataMap>>,

    // Mappings
    _mappings: CanMappingCollection,
}

impl CanProtocol {
    /// Create new CAN protocol instance
    pub fn new(channel_config: ChannelConfig, can_config: CanConfig) -> Result<Self> {
        let logger = ChannelLogger::new(
            channel_config.id() as u32,
            channel_config.name().to_string(),
        );

        logger.log_init(
            "CAN",
            &format!(
                "Initializing CAN protocol for channel {} on interface {}",
                channel_config.id(),
                can_config.interface
            ),
        );

        // Log configuration details
        logger.log_config("CAN", "interface", &can_config.interface);
        if let Some(bitrate) = can_config.bitrate {
            logger.log_config("CAN", "bitrate", &bitrate.to_string());
        }
        logger.log_config(
            "CAN",
            "filters_count",
            &can_config.filters.len().to_string(),
        );

        // Create transport
        let transport = CanTransport::new(
            &can_config.interface,
            can_config.bitrate,
            &can_config.filters,
        )?;

        // Load mappings from channel configuration
        let mappings = Self::load_mappings_from_config(&channel_config)?;
        logger.log_init(
            "CAN",
            &format!(
                "Loaded CAN mappings - T:{}, S:{}, C:{}, A:{}, Total by CAN ID: {}",
                mappings.telemetry.len(),
                mappings.signal.len(),
                mappings.control.len(),
                mappings.adjustment.len(),
                mappings.by_can_id.len()
            ),
        );

        Ok(Self {
            channel_config: Arc::new(channel_config),
            _can_config: can_config,
            transport: Arc::new(RwLock::new(transport)),
            is_connected: Arc::new(AtomicBool::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::default())),
            connection_state: Arc::new(RwLock::new(ConnectionState::Uninitialized)),
            logger,
            telemetry_data: Arc::new(RwLock::new(HashMap::new())),
            signal_data: Arc::new(RwLock::new(HashMap::new())),
            _mappings: mappings,
        })
    }

    /// Create from RuntimeChannelConfig
    pub fn from_runtime_config(
        runtime_config: &crate::core::config::RuntimeChannelConfig,
    ) -> Result<Self> {
        let channel_config = (*runtime_config.base).clone();

        // Extract CAN parameters from channel config
        let params = &channel_config.parameters;

        // Parse or use default polling config
        let polling = if let Some(polling_value) = params.get("polling") {
            serde_json::from_value(polling_value.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse CAN polling config: {}", e))?
        } else {
            // Use Default trait instead of json! macro
            super::types::CanPollingConfig::default()
        };

        // Manually construct CanConfig
        // Note: filters will be populated from runtime_config.can_mappings during initialize()
        let can_config = super::types::CanConfig {
            interface: params
                .get("interface")
                .and_then(|v| v.as_str())
                .unwrap_or("can0")
                .to_string(),
            polling,
            filters: Vec::new(), // Populated during initialize from can_mappings
            bitrate: params
                .get("bitrate")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32),
        };

        Self::new(channel_config, can_config)
    }

    /// Load mappings from channel configuration
    fn load_mappings_from_config(_channel_config: &ChannelConfig) -> Result<CanMappingCollection> {
        // Points are loaded from SQLite at runtime via RuntimeChannelConfig
        debug!("Loading CAN mappings - points will be loaded from SQLite at runtime");
        Ok(CanMappingCollection::default())
    }
}

#[async_trait]
impl ComBase for CanProtocol {
    fn name(&self) -> &str {
        "CAN Protocol"
    }

    fn get_channel_id(&self) -> u16 {
        self.channel_config.id()
    }

    async fn get_status(&self) -> ChannelStatus {
        *self.status.read().await
    }

    async fn initialize(&mut self, runtime_config: Arc<RuntimeChannelConfig>) -> Result<()> {
        // Update connection state
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Initializing;
        }

        let channel_id = runtime_config.id();

        self.logger.log_init(
            "CAN",
            &format!("Starting initialization for channel {}", channel_id),
        );

        // Count points with and without mappings
        let mut total_points = 0;
        let mut mapped_points = 0;
        let mut skipped_points: Vec<u32> = Vec::new();

        // Check telemetry points
        for point in &runtime_config.telemetry_points {
            total_points += 1;
            if runtime_config
                .can_mappings
                .iter()
                .any(|m| m.point_id == point.base.point_id)
            {
                mapped_points += 1;
            } else {
                debug!(
                    "Channel {} telemetry point {} - skipped, no CAN mapping found",
                    channel_id, point.base.point_id
                );
                skipped_points.push(point.base.point_id);
            }
        }

        // Check signal points
        for point in &runtime_config.signal_points {
            total_points += 1;
            if runtime_config
                .can_mappings
                .iter()
                .any(|m| m.point_id == point.base.point_id)
            {
                mapped_points += 1;
            } else {
                debug!(
                    "Channel {} signal point {} - skipped, no CAN mapping found",
                    channel_id, point.base.point_id
                );
                skipped_points.push(point.base.point_id);
            }
        }

        // Check control points
        for point in &runtime_config.control_points {
            total_points += 1;
            if runtime_config
                .can_mappings
                .iter()
                .any(|m| m.point_id == point.base.point_id)
            {
                mapped_points += 1;
            } else {
                debug!(
                    "Channel {} control point {} - skipped, no CAN mapping found",
                    channel_id, point.base.point_id
                );
                skipped_points.push(point.base.point_id);
            }
        }

        // Check adjustment points
        for point in &runtime_config.adjustment_points {
            total_points += 1;
            if runtime_config
                .can_mappings
                .iter()
                .any(|m| m.point_id == point.base.point_id)
            {
                mapped_points += 1;
            } else {
                debug!(
                    "Channel {} adjustment point {} - skipped, no CAN mapping found",
                    channel_id, point.base.point_id
                );
                skipped_points.push(point.base.point_id);
            }
        }

        self.logger.log_init(
            "CAN",
            &format!(
                "Loaded {} CAN mappings for {} total points ({} mapped, {} skipped)",
                runtime_config.can_mappings.len(),
                total_points,
                mapped_points,
                skipped_points.len()
            ),
        );

        // Output summary if any points were skipped
        if !skipped_points.is_empty() {
            tracing::warn!(
                "Channel {} CAN initialization: {} points skipped due to missing CAN mappings",
                channel_id,
                skipped_points.len()
            );
            debug!(
                "Channel {} skipped points IDs: {:?}",
                channel_id, skipped_points
            );
        }

        self.logger.log_init(
            "CAN",
            "Initialization completed successfully (connection will be established later)",
        );

        Ok(())
    }

    async fn read_four_telemetry(&self, telemetry_type: FourRemote) -> Result<PointDataMap> {
        match telemetry_type {
            FourRemote::Telemetry => Ok(self.telemetry_data.read().await.clone()),
            FourRemote::Signal => Ok(self.signal_data.read().await.clone()),
            _ => Ok(HashMap::new()),
        }
    }
}

#[async_trait]
impl ComClient for CanProtocol {
    fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
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
                "Initiating CAN bus connection",
            );
        }

        self.logger.log_connect(
            "CAN",
            &self._can_config.interface,
            &format!("Channel {} opening CAN interface", self.channel_config.id()),
        );

        // Open CAN interface
        match self.transport.write().await.open().await {
            Ok(_) => {
                self.is_connected.store(true, Ordering::Relaxed);
                self.status.write().await.is_connected = true;

                // Update connection state to connected
                {
                    let old_state = *self.connection_state.read().await;
                    let mut state = self.connection_state.write().await;
                    *state = ConnectionState::Connected;
                    self.logger.log_status(
                        old_state,
                        ConnectionState::Connected,
                        "CAN interface opened successfully",
                    );
                }

                Ok(())
            },
            Err(e) => {
                // Update connection state to disconnected
                {
                    let old_state = *self.connection_state.read().await;
                    let mut state = self.connection_state.write().await;
                    *state = ConnectionState::Disconnected;
                    self.logger.log_status(
                        old_state,
                        ConnectionState::Disconnected,
                        &format!("CAN interface failed to open: {}", e),
                    );
                }
                Err(e)
            },
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        // Update connection state
        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Closed;
            self.logger
                .log_status(old_state, ConnectionState::Closed, "Closing CAN interface");
        }

        // Close CAN interface
        self.transport.write().await.close().await?;

        self.is_connected.store(false, Ordering::Relaxed);
        self.status.write().await.is_connected = false;

        self.logger.log_init(
            "CAN",
            &format!(
                "Channel {} CAN interface closed successfully",
                self.channel_config.id()
            ),
        );

        Ok(())
    }

    async fn control(&mut self, _commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        // Control commands not yet implemented for CAN
        Ok(vec![])
    }

    async fn adjustment(
        &mut self,
        _adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        // Adjustment commands not yet implemented for CAN
        Ok(vec![])
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::core::config::types::ChannelLoggingConfig;
    use crate::protocols::can::types::{CanBatchConfig, CanPollingConfig};
    use serde_json::json;

    fn create_test_channel_config() -> ChannelConfig {
        let mut parameters = HashMap::new();
        parameters.insert("interface".to_string(), json!("vcan0"));
        parameters.insert("bitrate".to_string(), json!(500000));
        parameters.insert(
            "filters".to_string(),
            json!(["0x100", "0x200-0x2FF", "0x300:0xFF00"]),
        );

        ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
                id: 1,
                name: "Test CAN Channel".to_string(),
                description: Some("Test CAN protocol".to_string()),
                protocol: "can".to_string(),
                enabled: true,
            },
            parameters,
            logging: ChannelLoggingConfig::default(),
        }
    }

    #[test]
    fn test_can_config_from_channel_config() {
        let channel_config = create_test_channel_config();
        let can_config = CanConfig::from_channel_config(&channel_config);

        assert!(can_config.is_ok());
        let config = can_config.unwrap();
        assert_eq!(config.interface, "vcan0");
        assert_eq!(config.bitrate, Some(500000));
        assert_eq!(config.filters.len(), 3);
    }

    #[test]
    fn test_can_config_missing_interface() {
        let mut channel_config = create_test_channel_config();
        channel_config.parameters.remove("interface");

        let result = CanConfig::from_channel_config(&channel_config);
        assert!(result.is_err());
    }

    #[test]
    fn test_can_config_default_polling() {
        let channel_config = create_test_channel_config();
        let can_config = CanConfig::from_channel_config(&channel_config).unwrap();

        assert!(can_config.polling.enabled);
        assert_eq!(can_config.polling.interval_ms, 1000);
        assert_eq!(can_config.polling.timeout_ms, 5000);
        assert_eq!(can_config.polling.max_retries, 3);
    }

    #[test]
    fn test_filter_parsing_single_id() {
        use crate::protocols::can::types::parse_filter_string;

        let filter = parse_filter_string("0x100");
        assert!(filter.is_some());
        let filter = filter.unwrap();
        assert_eq!(filter.can_id, 0x100);
        assert_eq!(filter.mask, 0xFFFFFFFF);
    }

    #[test]
    fn test_filter_parsing_range() {
        use crate::protocols::can::types::parse_filter_string;

        let filter = parse_filter_string("0x200-0x2FF");
        assert!(filter.is_some());
        let filter = filter.unwrap();
        assert_eq!(filter.can_id, 0x200);
    }

    #[test]
    fn test_filter_parsing_id_mask() {
        use crate::protocols::can::types::parse_filter_string;

        let filter = parse_filter_string("0x300:0xFF00");
        assert!(filter.is_some());
        let filter = filter.unwrap();
        assert_eq!(filter.can_id, 0x300);
        assert_eq!(filter.mask, 0xFF00);
    }

    #[test]
    fn test_can_protocol_basic_properties() {
        let channel_config = create_test_channel_config();
        let can_config = CanConfig::from_channel_config(&channel_config).unwrap();

        // Note: CanProtocol::new() may fail without virtual CAN device
        // We test the config parsing separately
        match CanProtocol::new(channel_config.clone(), can_config) {
            Ok(protocol) => {
                assert_eq!(protocol.name(), "CAN Protocol");
                assert_eq!(protocol.get_channel_id(), 1);
                assert!(!protocol.is_connected());
            },
            Err(_) => {
                // Skip test if CAN device not available
                println!("Skipping CAN protocol test - no virtual CAN device available");
            },
        }
    }

    #[tokio::test]
    async fn test_read_four_telemetry_empty() {
        let channel_config = create_test_channel_config();
        let can_config = CanConfig::from_channel_config(&channel_config).unwrap();

        match CanProtocol::new(channel_config, can_config) {
            Ok(protocol) => {
                // Test reading telemetry (should be empty initially)
                let telemetry = protocol.read_four_telemetry(FourRemote::Telemetry).await;
                assert!(telemetry.is_ok());
                assert_eq!(telemetry.unwrap().len(), 0);

                // Test reading signal
                let signal = protocol.read_four_telemetry(FourRemote::Signal).await;
                assert!(signal.is_ok());
                assert_eq!(signal.unwrap().len(), 0);

                // Test reading unsupported types
                let control = protocol.read_four_telemetry(FourRemote::Control).await;
                assert!(control.is_ok());
                assert_eq!(control.unwrap().len(), 0);
            },
            Err(_) => {
                println!("Skipping test - no virtual CAN device");
            },
        }
    }

    #[tokio::test]
    async fn test_control_when_not_connected() {
        let channel_config = create_test_channel_config();
        let can_config = CanConfig::from_channel_config(&channel_config).unwrap();

        match CanProtocol::new(channel_config, can_config) {
            Ok(mut protocol) => {
                assert!(!protocol.is_connected());

                // Test control command when not connected
                let result = protocol.control(vec![(1, RedisValue::Bool(true))]).await;
                assert!(result.is_err());
                assert!(matches!(result.unwrap_err(), ComSrvError::NotConnected));

                // Test adjustment command when not connected
                let result = protocol
                    .adjustment(vec![(1, RedisValue::Float(100.0))])
                    .await;
                assert!(result.is_err());
                assert!(matches!(result.unwrap_err(), ComSrvError::NotConnected));
            },
            Err(_) => {
                println!("Skipping test - no virtual CAN device");
            },
        }
    }

    #[tokio::test]
    async fn test_status_query() {
        let channel_config = create_test_channel_config();
        let can_config = CanConfig::from_channel_config(&channel_config).unwrap();

        match CanProtocol::new(channel_config, can_config) {
            Ok(protocol) => {
                let status = protocol.get_status().await;
                assert!(!status.is_connected);
            },
            Err(_) => {
                println!("Skipping test - no virtual CAN device");
            },
        }
    }

    #[tokio::test]
    async fn test_initialization() {
        let channel_config = create_test_channel_config();
        let can_config = CanConfig::from_channel_config(&channel_config).unwrap();

        match CanProtocol::new(channel_config.clone(), can_config) {
            Ok(mut protocol) => {
                let runtime_config = Arc::new(RuntimeChannelConfig::from_base(channel_config));
                let result = protocol.initialize(runtime_config).await;
                assert!(result.is_ok());
            },
            Err(_) => {
                println!("Skipping test - no virtual CAN device");
            },
        }
    }

    #[test]
    fn test_can_mapping_collection_default() {
        let collection = CanMappingCollection::default();
        assert_eq!(collection.telemetry.len(), 0);
        assert_eq!(collection.signal.len(), 0);
        assert_eq!(collection.control.len(), 0);
        assert_eq!(collection.adjustment.len(), 0);
        assert_eq!(collection.by_can_id.len(), 0);
    }

    #[test]
    fn test_hex_id_parsing() {
        use crate::protocols::can::types::parse_hex_id;

        assert_eq!(parse_hex_id("0x100"), Some(0x100));
        assert_eq!(parse_hex_id("0X100"), Some(0x100));
        assert_eq!(parse_hex_id("256"), Some(256));
        assert_eq!(parse_hex_id("  0x1FF  "), Some(0x1FF));
        assert!(parse_hex_id("invalid").is_none());
    }

    #[test]
    fn test_polling_config_defaults() {
        let config = CanPollingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_interval_ms, 1000);
    }

    #[test]
    fn test_batch_config_defaults() {
        let config = CanBatchConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_batch_size, 20);
        assert_eq!(config.max_wait_ms, 100);
    }
}
