//! Mock Transport for Testing
//!
//! This module provides a mock transport implementation for testing protocol
//! logic without requiring actual network or serial connections.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::debug;

use super::traits::{
    ConnectionState, Transport, TransportBuilder, TransportConfig, TransportError, TransportStats,
};

/// Mock transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockTransportConfig {
    /// Transport name for identification
    pub name: String,
    /// Simulated connection delay
    pub connection_delay: Duration,
    /// Simulated send delay
    pub send_delay: Duration,
    /// Simulated receive delay
    pub receive_delay: Duration,
    /// Whether connections should fail
    pub should_fail_connection: bool,
    /// Whether send operations should fail
    pub should_fail_send: bool,
    /// Whether receive operations should fail
    pub should_fail_receive: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Connection timeout
    pub timeout: Duration,
}

impl Default for MockTransportConfig {
    fn default() -> Self {
        Self {
            name: "Mock Transport".to_string(),
            connection_delay: Duration::from_millis(10),
            send_delay: Duration::from_millis(1),
            receive_delay: Duration::from_millis(1),
            should_fail_connection: false,
            should_fail_send: false,
            should_fail_receive: false,
            max_retries: 3,
            timeout: Duration::from_secs(5),
        }
    }
}

impl TransportConfig for MockTransportConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn validate(&self) -> Result<(), TransportError> {
        if self.name.is_empty() {
            return Err(TransportError::ConfigError(
                "Name cannot be empty".to_string(),
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

/// Mock transport state
#[derive(Debug)]
struct MockTransportState {
    /// Whether the transport is connected
    connected: bool,
    /// Queue of data to be received
    receive_queue: VecDeque<Vec<u8>>,
    /// History of sent data
    sent_data: Vec<Vec<u8>>,
    /// Transport statistics
    stats: TransportStats,
}

impl MockTransportState {
    fn new() -> Self {
        Self {
            connected: false,
            receive_queue: VecDeque::new(),
            sent_data: Vec::new(),
            stats: TransportStats::new(),
        }
    }
}

/// Mock transport implementation
#[derive(Debug)]
pub struct MockTransport {
    /// Transport configuration
    config: MockTransportConfig,
    /// Internal state
    state: Arc<RwLock<MockTransportState>>,
    /// Creation time for uptime calculation
    start_time: SystemTime,
}

impl MockTransport {
    /// Create new mock transport with configuration
    pub fn new(config: MockTransportConfig) -> Result<Self, TransportError> {
        config.validate()?;

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(MockTransportState::new())),
            start_time: SystemTime::now(),
        })
    }

    /// Add data to the receive queue (for testing)
    pub async fn add_receive_data(&self, data: Vec<u8>) {
        let mut state = self.state.write().await;
        state.receive_queue.push_back(data);
    }

    /// Get all sent data (for testing)
    pub async fn get_sent_data(&self) -> Vec<Vec<u8>> {
        let state = self.state.read().await;
        state.sent_data.clone()
    }

    /// Clear all sent data (for testing)
    pub async fn clear_sent_data(&self) {
        let mut state = self.state.write().await;
        state.sent_data.clear();
    }

    /// Set connection failure mode (for testing)
    pub async fn set_connection_failure(&mut self, should_fail: bool) {
        self.config.should_fail_connection = should_fail;
    }

    /// Set send failure mode (for testing)
    pub async fn set_send_failure(&mut self, should_fail: bool) {
        self.config.should_fail_send = should_fail;
    }

    /// Set receive failure mode (for testing)
    pub async fn set_receive_failure(&mut self, should_fail: bool) {
        self.config.should_fail_receive = should_fail;
    }
}

#[async_trait]
impl Transport for MockTransport {
    fn transport_type(&self) -> &str {
        "mock"
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    async fn connect(&mut self) -> Result<(), TransportError> {
        let mut state = self.state.write().await;
        state.stats.record_connection_attempt();
        state.stats.connection_state = ConnectionState::Connecting;
        drop(state);

        debug!(
            "Mock transport connecting with delay: {:?}",
            self.config.connection_delay
        );

        // Simulate connection delay
        tokio::time::sleep(self.config.connection_delay).await;

        let mut state = self.state.write().await;

        if self.config.should_fail_connection {
            state.stats.record_failed_connection();
            return Err(TransportError::ConnectionFailed(
                "Mock connection failure".to_string(),
            ));
        }

        state.connected = true;
        state.stats.record_successful_connection();
        debug!("Mock transport connected successfully");

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        let mut state = self.state.write().await;
        if state.connected {
            state.connected = false;
            state.stats.record_disconnection();
            debug!("Mock transport disconnected");
        }
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        let mut state = self.state.write().await;

        if !state.connected {
            return Err(TransportError::SendFailed("Not connected".to_string()));
        }

        if self.config.should_fail_send {
            state.stats.connection_state = ConnectionState::Error;
            return Err(TransportError::SendFailed("Mock send failure".to_string()));
        }

        drop(state);

        // Simulate send delay
        tokio::time::sleep(self.config.send_delay).await;

        let mut state = self.state.write().await;
        let bytes_sent = data.len();
        state.sent_data.push(data.to_vec());
        state.stats.record_bytes_sent(bytes_sent);

        debug!("Mock transport sent {} bytes", bytes_sent);
        Ok(bytes_sent)
    }

    async fn receive(
        &mut self,
        buffer: &mut [u8],
        _timeout: Option<Duration>,
    ) -> Result<usize, TransportError> {
        let mut state = self.state.write().await;

        if !state.connected {
            return Err(TransportError::ReceiveFailed("Not connected".to_string()));
        }

        if self.config.should_fail_receive {
            state.stats.connection_state = ConnectionState::Error;
            return Err(TransportError::ReceiveFailed(
                "Mock receive failure".to_string(),
            ));
        }

        if let Some(data) = state.receive_queue.pop_front() {
            drop(state);

            // Simulate receive delay
            tokio::time::sleep(self.config.receive_delay).await;

            let bytes_to_copy = std::cmp::min(data.len(), buffer.len());
            buffer[..bytes_to_copy].copy_from_slice(&data[..bytes_to_copy]);

            let mut state = self.state.write().await;
            state.stats.record_bytes_received(bytes_to_copy);

            debug!("Mock transport received {} bytes", bytes_to_copy);
            Ok(bytes_to_copy)
        } else {
            // No data available
            Ok(0)
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
        diag.insert(
            "connection_delay_ms".to_string(),
            self.config.connection_delay.as_millis().to_string(),
        );
        diag.insert(
            "send_delay_ms".to_string(),
            self.config.send_delay.as_millis().to_string(),
        );
        diag.insert(
            "receive_delay_ms".to_string(),
            self.config.receive_delay.as_millis().to_string(),
        );
        diag.insert(
            "should_fail_connection".to_string(),
            self.config.should_fail_connection.to_string(),
        );
        diag.insert(
            "should_fail_send".to_string(),
            self.config.should_fail_send.to_string(),
        );
        diag.insert(
            "should_fail_receive".to_string(),
            self.config.should_fail_receive.to_string(),
        );
        diag.insert(
            "receive_queue_length".to_string(),
            state.receive_queue.len().to_string(),
        );
        diag.insert(
            "sent_data_count".to_string(),
            state.sent_data.len().to_string(),
        );

        diag
    }
}

/// Mock transport builder
#[derive(Debug, Default)]
pub struct MockTransportBuilder;

impl MockTransportBuilder {
    /// Create new mock transport builder
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TransportBuilder for MockTransportBuilder {
    type Config = MockTransportConfig;
    type Transport = MockTransport;

    async fn build(&self, config: Self::Config) -> Result<Self::Transport, TransportError> {
        MockTransport::new(config)
    }

    fn default_config(&self) -> Self::Config {
        MockTransportConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_config_validation() {
        let mut config = MockTransportConfig::default();
        assert!(config.validate().is_ok());

        config.name = "".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_mock_transport_creation() {
        let config = MockTransportConfig::default();
        let _transport = MockTransport::new(config);
        assert!(_transport.is_ok());

        let _transport = _transport.unwrap();
        assert_eq!(_transport.transport_type(), "mock");
        assert_eq!(_transport.name(), "Mock Transport");
    }

    #[tokio::test]
    async fn test_mock_transport_connect_disconnect() {
        let config = MockTransportConfig::default();
        let mut transport = MockTransport::new(config).unwrap();

        assert!(!transport.is_connected().await);

        assert!(transport.connect().await.is_ok());
        assert!(transport.is_connected().await);

        assert!(transport.disconnect().await.is_ok());
        assert!(!transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_mock_transport_send_receive() {
        let config = MockTransportConfig::default();
        let mut transport = MockTransport::new(config).unwrap();

        // Connect first
        transport.connect().await.unwrap();

        // Add data to receive queue
        transport.add_receive_data(vec![1, 2, 3, 4]).await;

        // Send data
        let send_data = vec![0xAA, 0xBB];
        let bytes_sent = transport.send(&send_data).await.unwrap();
        assert_eq!(bytes_sent, 2);

        // Verify sent data
        let sent_data = transport.get_sent_data().await;
        assert_eq!(sent_data.len(), 1);
        assert_eq!(sent_data[0], send_data);

        // Receive data
        let mut buffer = [0u8; 10];
        let bytes_received = transport.receive(&mut buffer, None).await.unwrap();
        assert_eq!(bytes_received, 4);
        assert_eq!(&buffer[..4], &[1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_mock_transport_connection_failure() {
        let mut config = MockTransportConfig::default();
        config.should_fail_connection = true;

        let mut transport = MockTransport::new(config).unwrap();

        let result = transport.connect().await;
        assert!(result.is_err());
        assert!(!transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_mock_transport_send_failure() {
        let mut config = MockTransportConfig::default();
        config.should_fail_send = true;

        let mut transport = MockTransport::new(config).unwrap();
        transport.connect().await.unwrap();

        let send_data = vec![1, 2, 3];
        let result = transport.send(&send_data).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_transport_stats() {
        let config = MockTransportConfig::default();
        let mut transport = MockTransport::new(config).unwrap();

        let stats = transport.stats().await;
        assert_eq!(stats.connection_attempts, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.bytes_received, 0);

        transport.connect().await.unwrap();
        let stats = transport.stats().await;
        assert_eq!(stats.connection_attempts, 1);
        assert_eq!(stats.successful_connections, 1);
    }

    #[tokio::test]
    async fn test_mock_transport_builder() {
        let builder = MockTransportBuilder::new();
        let config = builder.default_config();
        assert_eq!(config.name, "Mock Transport");

        let _transport = builder.build(config).await;
        assert!(transport.is_ok());
    }
}
