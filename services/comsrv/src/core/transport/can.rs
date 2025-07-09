//! CAN Bus Transport Implementation
//!
//! This module provides CAN (Controller Area Network) transport implementation
//! for industrial automation and automotive applications.

#![cfg(all(target_os = "linux", feature = "can"))]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::traits::{
    ConnectionState, Transport, TransportBuilder, TransportConfig, TransportError, TransportStats,
};

/// CAN frame type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanFrameType {
    /// Standard 11-bit identifier
    Standard,
    /// Extended 29-bit identifier  
    Extended,
}

/// CAN bit rate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanBitRate {
    /// 125 kbps
    Kbps125,
    /// 250 kbps
    Kbps250,
    /// 500 kbps
    Kbps500,
    /// 1000 kbps (1 Mbps)
    Mbps1,
    /// Custom bit rate
    Custom(u32),
}

impl CanBitRate {
    /// Get bit rate value in bps
    pub fn to_bps(&self) -> u32 {
        match self {
            CanBitRate::Kbps125 => 125_000,
            CanBitRate::Kbps250 => 250_000,
            CanBitRate::Kbps500 => 500_000,
            CanBitRate::Mbps1 => 1_000_000,
            CanBitRate::Custom(rate) => *rate,
        }
    }
}

/// CAN frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanFrame {
    /// CAN identifier
    pub id: u32,
    /// Frame type (standard or extended)
    pub frame_type: CanFrameType,
    /// Data payload (0-8 bytes for CAN 2.0, 0-64 bytes for CAN FD)
    pub data: Vec<u8>,
    /// Remote transmission request
    pub rtr: bool,
}

impl CanFrame {
    /// Create new standard CAN frame
    pub fn new_standard(id: u16, data: Vec<u8>) -> Result<Self, TransportError> {
        if id > 0x7FF {
            return Err(TransportError::ConfigError(
                "Standard CAN ID must be <= 0x7FF".to_string(),
            ));
        }
        if data.len() > 8 {
            return Err(TransportError::ConfigError(
                "CAN 2.0 data must be <= 8 bytes".to_string(),
            ));
        }

        Ok(Self {
            id: id as u32,
            frame_type: CanFrameType::Standard,
            data,
            rtr: false,
        })
    }

    /// Create new extended CAN frame
    pub fn new_extended(id: u32, data: Vec<u8>) -> Result<Self, TransportError> {
        if id > 0x1FFFFFFF {
            return Err(TransportError::ConfigError(
                "Extended CAN ID must be <= 0x1FFFFFFF".to_string(),
            ));
        }
        if data.len() > 8 {
            return Err(TransportError::ConfigError(
                "CAN 2.0 data must be <= 8 bytes".to_string(),
            ));
        }

        Ok(Self {
            id,
            frame_type: CanFrameType::Extended,
            data,
            rtr: false,
        })
    }

    /// Create RTR frame
    pub fn new_rtr(id: u32, frame_type: CanFrameType) -> Self {
        Self {
            id,
            frame_type,
            data: Vec::new(),
            rtr: true,
        }
    }
}

/// CAN transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanTransportConfig {
    /// Transport name for identification
    pub name: String,
    /// CAN interface name (e.g., "can0", "vcan0")
    pub interface: String,
    /// CAN bit rate
    pub bit_rate: CanBitRate,
    /// Enable CAN FD (Flexible Data Rate)
    pub can_fd: bool,
    /// Connection timeout
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Receive buffer size
    pub recv_buffer_size: usize,
    /// Send buffer size  
    pub send_buffer_size: usize,
    /// CAN filters (optional)
    pub filters: Vec<CanFilter>,
}

/// CAN filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanFilter {
    /// Filter ID
    pub id: u32,
    /// Filter mask
    pub mask: u32,
    /// Apply to extended frames
    pub extended: bool,
}

impl Default for CanTransportConfig {
    fn default() -> Self {
        Self {
            name: "CAN Transport".to_string(),
            interface: "can0".to_string(),
            bit_rate: CanBitRate::Kbps500,
            can_fd: false,
            timeout: Duration::from_secs(5),
            max_retries: 3,
            recv_buffer_size: 1024,
            send_buffer_size: 1024,
            filters: Vec::new(),
        }
    }
}

impl TransportConfig for CanTransportConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn validate(&self) -> Result<(), TransportError> {
        if self.name.is_empty() {
            return Err(TransportError::ConfigError(
                "Name cannot be empty".to_string(),
            ));
        }

        if self.interface.is_empty() {
            return Err(TransportError::ConfigError(
                "Interface cannot be empty".to_string(),
            ));
        }

        if self.timeout.is_zero() {
            return Err(TransportError::ConfigError(
                "Timeout must be greater than zero".to_string(),
            ));
        }

        if self.recv_buffer_size == 0 || self.send_buffer_size == 0 {
            return Err(TransportError::ConfigError(
                "Buffer sizes must be greater than zero".to_string(),
            ));
        }

        Ok(())
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// CAN transport state
#[derive(Debug)]
struct CanTransportState {
    /// Whether the transport is connected
    connected: bool,
    /// Receive frame queue
    receive_queue: std::collections::VecDeque<CanFrame>,
    /// Transport statistics
    stats: TransportStats,
}

impl CanTransportState {
    fn new() -> Self {
        Self {
            connected: false,
            receive_queue: std::collections::VecDeque::new(),
            stats: TransportStats::new(),
        }
    }
}

/// CAN transport implementation
#[derive(Debug)]
pub struct CanTransport {
    /// Transport configuration
    config: CanTransportConfig,
    /// Internal state
    state: Arc<RwLock<CanTransportState>>,
    /// Connection start time for uptime calculation
    start_time: SystemTime,
}

impl CanTransport {
    /// Create new CAN transport with configuration
    pub fn new(config: CanTransportConfig) -> Result<Self, TransportError> {
        config.validate()?;

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(CanTransportState::new())),
            start_time: SystemTime::now(),
        })
    }

    /// Send CAN frame
    pub async fn send_frame(&self, frame: CanFrame) -> Result<(), TransportError> {
        let state = self.state.read().await;

        if !state.connected {
            return Err(TransportError::SendFailed("CAN not connected".to_string()));
        }
        drop(state);

        #[cfg(feature = "can")]
        {
            // In a real implementation, this would send the frame via socketcan
            debug!(
                "Sending CAN frame: ID=0x{:X}, Data={:?}",
                frame.id, frame.data
            );
        }
        #[cfg(not(feature = "can"))]
        {
            debug!(
                "Mock: Sending CAN frame: ID=0x{:X}, Data={:?}",
                frame.id, frame.data
            );
        }

        let mut state = self.state.write().await;
        state.stats.record_bytes_sent(frame.data.len() + 8); // 8 bytes for CAN header

        Ok(())
    }

    /// Receive CAN frame
    pub async fn receive_frame(&self) -> Result<Option<CanFrame>, TransportError> {
        let mut state = self.state.write().await;

        if !state.connected {
            return Err(TransportError::ReceiveFailed(
                "CAN not connected".to_string(),
            ));
        }

        if let Some(frame) = state.receive_queue.pop_front() {
            state.stats.record_bytes_received(frame.data.len() + 8);
            debug!(
                "Received CAN frame: ID=0x{:X}, Data={:?}",
                frame.id, frame.data
            );
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    /// Add frame to receive queue (for testing)
    #[cfg(test)]
    pub async fn add_receive_frame(&self, frame: CanFrame) {
        let mut state = self.state.write().await;
        state.receive_queue.push_back(frame);
    }

    /// Get interface name
    pub fn interface(&self) -> &str {
        &self.config.interface
    }

    /// Get bit rate
    pub fn bit_rate(&self) -> &CanBitRate {
        &self.config.bit_rate
    }

    /// Check if CAN FD is enabled
    pub fn is_can_fd_enabled(&self) -> bool {
        self.config.can_fd
    }
}

#[async_trait]
impl Transport for CanTransport {
    fn transport_type(&self) -> &str {
        "can"
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    async fn connect(&mut self) -> Result<(), TransportError> {
        let mut state = self.state.write().await;
        state.stats.record_connection_attempt();
        state.stats.connection_state = ConnectionState::Connecting;

        debug!("Connecting to CAN interface: {}", self.config.interface);

        #[cfg(feature = "can")]
        {
            // In a real implementation, this would:
            // 1. Open the CAN socket
            // 2. Bind to the interface
            // 3. Set bit rate and filters
            // 4. Start receiving frames
            info!(
                "CAN transport connected to interface: {}",
                self.config.interface
            );
        }
        #[cfg(not(feature = "can"))]
        {
            warn!("CAN feature not enabled, using mock implementation");
            info!(
                "Mock CAN transport connected to interface: {}",
                self.config.interface
            );
        }

        state.connected = true;
        state.stats.record_successful_connection();

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        let mut state = self.state.write().await;
        if state.connected {
            state.connected = false;
            state.stats.record_disconnection();
            info!(
                "CAN transport disconnected from interface: {}",
                self.config.interface
            );
        }
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        if data.len() < 4 {
            return Err(TransportError::SendFailed(
                "Invalid CAN frame format".to_string(),
            ));
        }

        // Parse CAN frame from raw data
        // Format: [ID(4 bytes)][data_len(1 byte)][data(0-8 bytes)]
        let id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let data_len = data[4] as usize;

        if data.len() < 5 + data_len {
            return Err(TransportError::SendFailed(
                "Incomplete CAN frame data".to_string(),
            ));
        }

        let frame_data = data[5..5 + data_len].to_vec();

        let frame = if id <= 0x7FF {
            CanFrame::new_standard(id as u16, frame_data)?
        } else {
            CanFrame::new_extended(id, frame_data)?
        };

        self.send_frame(frame).await?;
        Ok(data.len())
    }

    async fn receive(
        &mut self,
        buffer: &mut [u8],
        _timeout: Option<Duration>,
    ) -> Result<usize, TransportError> {
        if let Some(frame) = self.receive_frame().await? {
            // Serialize CAN frame to raw data
            // Format: [ID(4 bytes)][data_len(1 byte)][data(0-8 bytes)]
            if buffer.len() < 5 + frame.data.len() {
                return Err(TransportError::ReceiveFailed(
                    "Buffer too small for CAN frame".to_string(),
                ));
            }

            let id_bytes = frame.id.to_be_bytes();
            buffer[0..4].copy_from_slice(&id_bytes);
            buffer[4] = frame.data.len() as u8;
            buffer[5..5 + frame.data.len()].copy_from_slice(&frame.data);

            Ok(5 + frame.data.len())
        } else {
            Ok(0) // No frame available
        }
    }

    async fn is_connected(&self) -> bool {
        let state = self.state.read().await;
        state.connected
    }

    async fn connection_state(&self) -> ConnectionState {
        let state = self.state.read().await;
        state.stats.connection_state
    }

    async fn stats(&self) -> TransportStats {
        let state = self.state.read().await;
        let mut stats = state.stats.clone();

        // Update uptime
        if let Ok(elapsed) = self.start_time.elapsed() {
            stats.uptime = elapsed;
        }

        stats
    }

    async fn reset_stats(&mut self) {
        let mut state = self.state.write().await;
        state.stats.reset();
    }

    async fn diagnostics(&self) -> std::collections::HashMap<String, String> {
        let mut diag = std::collections::HashMap::new();
        let state = self.state.read().await;

        diag.insert(
            "transport_type".to_string(),
            self.transport_type().to_string(),
        );
        diag.insert("name".to_string(), self.name().to_string());
        diag.insert("connected".to_string(), state.connected.to_string());
        diag.insert(
            "connection_state".to_string(),
            format!("{:?}", state.stats.connection_state),
        );
        diag.insert("interface".to_string(), self.config.interface.clone());
        diag.insert(
            "bit_rate_bps".to_string(),
            self.config.bit_rate.to_bps().to_string(),
        );
        diag.insert("can_fd".to_string(), self.config.can_fd.to_string());
        diag.insert(
            "recv_buffer_size".to_string(),
            self.config.recv_buffer_size.to_string(),
        );
        diag.insert(
            "send_buffer_size".to_string(),
            self.config.send_buffer_size.to_string(),
        );
        diag.insert(
            "filter_count".to_string(),
            self.config.filters.len().to_string(),
        );
        diag.insert(
            "receive_queue_length".to_string(),
            state.receive_queue.len().to_string(),
        );

        diag
    }
}

/// CAN transport builder
#[derive(Debug, Default)]
pub struct CanTransportBuilder;

impl CanTransportBuilder {
    /// Create new CAN transport builder
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TransportBuilder for CanTransportBuilder {
    type Config = CanTransportConfig;
    type Transport = CanTransport;

    async fn build(&self, config: Self::Config) -> Result<Self::Transport, TransportError> {
        CanTransport::new(config)
    }

    fn default_config(&self) -> Self::Config {
        CanTransportConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_frame_creation() {
        // Test standard frame
        let frame = CanFrame::new_standard(0x123, vec![1, 2, 3, 4]);
        assert!(frame.is_ok());
        let frame = frame.unwrap();
        assert_eq!(frame.id, 0x123);
        assert!(matches!(frame.frame_type, CanFrameType::Standard));
        assert_eq!(frame.data, vec![1, 2, 3, 4]);

        // Test invalid standard ID
        assert!(CanFrame::new_standard(0x800, vec![]).is_err());

        // Test extended frame
        let frame = CanFrame::new_extended(0x12345678, vec![5, 6, 7, 8]);
        assert!(frame.is_ok());
        let frame = frame.unwrap();
        assert_eq!(frame.id, 0x12345678);
        assert!(matches!(frame.frame_type, CanFrameType::Extended));

        // Test invalid extended ID
        assert!(CanFrame::new_extended(0x20000000, vec![]).is_err());

        // Test RTR frame
        let rtr_frame = CanFrame::new_rtr(0x123, CanFrameType::Standard);
        assert!(rtr_frame.rtr);
        assert!(rtr_frame.data.is_empty());
    }

    #[test]
    fn test_can_config_validation() {
        let mut config = CanTransportConfig::default();
        assert!(config.validate().is_ok());

        config.name = "".to_string();
        assert!(config.validate().is_err());

        config.name = "Test CAN".to_string();
        config.interface = "".to_string();
        assert!(config.validate().is_err());

        config.interface = "can0".to_string();
        config.timeout = Duration::from_secs(0);
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_can_transport_creation() {
        let config = CanTransportConfig::default();
        let _transport = CanTransport::new(config);
        assert!(transport.is_ok());

        let _transport = transport.unwrap();
        assert_eq!(transport.transport_type(), "can");
        assert_eq!(transport.name(), "CAN Transport");
        assert_eq!(transport.interface(), "can0");
    }

    #[tokio::test]
    async fn test_can_transport_connect_disconnect() {
        let config = CanTransportConfig::default();
        let mut transport = CanTransport::new(config).unwrap();

        assert!(!transport.is_connected().await);

        assert!(transport.connect().await.is_ok());
        assert!(transport.is_connected().await);

        assert!(transport.disconnect().await.is_ok());
        assert!(!transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_can_frame_send_receive() {
        let config = CanTransportConfig::default();
        let mut transport = CanTransport::new(config).unwrap();
        transport.connect().await.unwrap();

        // Test frame sending
        let frame = CanFrame::new_standard(0x123, vec![1, 2, 3, 4]).unwrap();
        assert!(transport.send_frame(frame.clone()).await.is_ok());

        // Test frame receiving (mock)
        transport.add_receive_frame(frame).await;
        let received = transport.receive_frame().await.unwrap();
        assert!(received.is_some());

        let received_frame = received.unwrap();
        assert_eq!(received_frame.id, 0x123);
        assert_eq!(received_frame.data, vec![1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_can_transport_builder() {
        let builder = CanTransportBuilder::new();
        let config = builder.default_config();
        assert_eq!(config.interface, "can0");

        let _transport = builder.build(config).await;
        assert!(transport.is_ok());
    }

    #[test]
    fn test_can_bit_rate() {
        assert_eq!(CanBitRate::Kbps125.to_bps(), 125_000);
        assert_eq!(CanBitRate::Kbps250.to_bps(), 250_000);
        assert_eq!(CanBitRate::Kbps500.to_bps(), 500_000);
        assert_eq!(CanBitRate::Mbps1.to_bps(), 1_000_000);
        assert_eq!(CanBitRate::Custom(800_000).to_bps(), 800_000);
    }
}
