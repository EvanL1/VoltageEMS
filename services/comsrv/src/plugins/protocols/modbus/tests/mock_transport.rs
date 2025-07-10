//! Mock transport implementation for testing
//!
//! Provides a controllable transport layer for unit testing without real network connections.

use crate::core::transport::traits::{ConnectionState, Transport, TransportError, TransportStats};
use crate::utils::hex::format_hex_pretty;
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Configuration for mock transport
#[derive(Debug, Clone)]
pub struct MockTransportConfig {
    /// Simulated connection success
    pub connect_success: bool,
    /// Simulated latency in milliseconds
    pub latency_ms: u64,
    /// Maximum message size
    pub max_message_size: usize,
    /// Fail after N operations (0 = never fail)
    pub fail_after_operations: usize,
    /// Timeout duration
    pub timeout: Duration,
}

impl Default for MockTransportConfig {
    fn default() -> Self {
        Self {
            connect_success: true,
            latency_ms: 0,
            max_message_size: 260,
            fail_after_operations: 0,
            timeout: Duration::from_secs(5),
        }
    }
}

impl crate::core::transport::traits::TransportConfig for MockTransportConfig {
    fn name(&self) -> &str {
        "mock_transport"
    }

    fn validate(&self) -> std::result::Result<(), TransportError> {
        Ok(())
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn max_retries(&self) -> u32 {
        3
    }
}

/// Mock transport state
#[derive(Debug)]
struct MockState {
    /// Queue of responses to return
    response_queue: VecDeque<Vec<u8>>,
    /// History of sent messages
    send_history: Vec<Vec<u8>>,
    /// Current operation count
    operation_count: usize,
    /// Connection state
    connected: bool,
    /// Statistics
    stats: TransportStats,
}

/// Mock transport implementation
#[derive(Debug)]
pub struct MockTransport {
    config: MockTransportConfig,
    state: Arc<Mutex<MockState>>,
}

impl MockTransport {
    /// Create a new mock transport
    pub fn new(config: MockTransportConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(MockState {
                response_queue: VecDeque::new(),
                send_history: Vec::new(),
                operation_count: 0,
                connected: false,
                stats: TransportStats::default(),
            })),
        }
    }

    /// Queue a response to be returned by next receive
    pub async fn queue_response(&self, response: Vec<u8>) {
        let mut state = self.state.lock().await;
        state.response_queue.push_back(response);
    }

    /// Queue multiple responses
    pub async fn queue_responses(&self, responses: Vec<Vec<u8>>) {
        let mut state = self.state.lock().await;
        for response in responses {
            state.response_queue.push_back(response);
        }
    }

    /// Get send history
    pub async fn get_send_history(&self) -> Vec<Vec<u8>> {
        let state = self.state.lock().await;
        state.send_history.clone()
    }

    /// Clear send history
    pub async fn clear_history(&self) {
        let mut state = self.state.lock().await;
        state.send_history.clear();
        state.operation_count = 0;
    }

    /// Set connection failure
    pub fn set_connect_failure(&mut self) {
        self.config.connect_success = false;
    }

    /// Simulate disconnect
    pub async fn simulate_disconnect(&self) {
        let mut state = self.state.lock().await;
        state.connected = false;
    }
}

#[async_trait]
impl Transport for MockTransport {
    fn transport_type(&self) -> &str {
        "mock"
    }

    fn name(&self) -> &str {
        "mock_transport"
    }

    async fn connect(&mut self) -> std::result::Result<(), TransportError> {
        debug!("[MockTransport] Attempting to establish connection...");

        if !self.config.connect_success {
            warn!("[MockTransport] Connection failed - simulated connection failure configuration");
            return Err(TransportError::ConnectionFailed(
                "Mock connection failed".to_string(),
            ));
        }

        let mut state = self.state.lock().await;
        state.connected = true;
        state.stats.record_connection_attempt();
        state.stats.record_successful_connection();

        debug!("[MockTransport] Connection successful");
        Ok(())
    }

    async fn disconnect(&mut self) -> std::result::Result<(), TransportError> {
        let mut state = self.state.lock().await;
        state.connected = false;
        state.stats.record_disconnection();
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> std::result::Result<usize, TransportError> {
        let mut state = self.state.lock().await;

        if !state.connected {
            warn!("[MockTransport] Send failed - not connected");
            return Err(TransportError::ConnectionLost("Not connected".to_string()));
        }

        // Check operation limit
        state.operation_count += 1;
        if self.config.fail_after_operations > 0
            && state.operation_count > self.config.fail_after_operations
        {
            warn!(
                "[MockTransport] Send failed - operation limit reached: {}",
                state.operation_count
            );
            return Err(TransportError::SendFailed(
                "Operation limit exceeded".to_string(),
            ));
        }

        // Check message size
        if data.len() > self.config.max_message_size {
            warn!(
                "[MockTransport] Send failed - message too large: {} > {}",
                data.len(),
                self.config.max_message_size
            );
            return Err(TransportError::SendFailed(format!(
                "Message too large: {} > {}",
                data.len(),
                self.config.max_message_size
            )));
        }

        // Record sent packet - INFO level shows raw packet content
        let hex_str = format_hex_pretty(data);
        debug!(hex_data = %hex_str, length = data.len(), direction = "send", "[MockTransport] Raw packet");

        // DEBUG level shows more detailed parsing information
        debug!(
            "[MockTransport] Send packet details - Hex: {}, ASCII: {:?}",
            format_hex_pretty(data),
            String::from_utf8_lossy(data)
        );

        // Simulate latency
        if self.config.latency_ms > 0 {
            debug!(
                "[MockTransport] Simulating latency: {}ms",
                self.config.latency_ms
            );
            drop(state);
            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.latency_ms)).await;
            state = self.state.lock().await;
        }

        // Store in history
        state.send_history.push(data.to_vec());
        state.stats.record_bytes_sent(data.len());

        Ok(data.len())
    }

    async fn receive(
        &mut self,
        buffer: &mut [u8],
        timeout: Option<Duration>,
    ) -> std::result::Result<usize, TransportError> {
        let mut state = self.state.lock().await;

        debug!("[MockTransport] Waiting for response...");

        if !state.connected {
            warn!("[MockTransport] Receive failed - not connected");
            return Err(TransportError::ConnectionLost("Not connected".to_string()));
        }

        // Check operation limit
        state.operation_count += 1;
        if self.config.fail_after_operations > 0
            && state.operation_count > self.config.fail_after_operations
        {
            warn!(
                "[MockTransport] Receive failed - operation limit reached: {}",
                state.operation_count
            );
            return Err(TransportError::ReceiveFailed(
                "Operation limit exceeded".to_string(),
            ));
        }

        // Get next response from queue
        if let Some(response) = state.response_queue.pop_front() {
            if response.len() > buffer.len() {
                warn!(
                    "[MockTransport] Receive failed - buffer too small: {} > {}",
                    response.len(),
                    buffer.len()
                );
                return Err(TransportError::ReceiveFailed(
                    "Buffer too small".to_string(),
                ));
            }

            // Simulate latency
            if self.config.latency_ms > 0 {
                debug!(
                    "[MockTransport] Simulating receive latency: {}ms",
                    self.config.latency_ms
                );
                drop(state);
                tokio::time::sleep(tokio::time::Duration::from_millis(self.config.latency_ms))
                    .await;
                state = self.state.lock().await;
            }

            buffer[..response.len()].copy_from_slice(&response);
            state.stats.record_bytes_received(response.len());

            // Record received packet - INFO level shows raw packet content
            let hex_str = format_hex_pretty(&response);
            debug!(hex_data = %hex_str, length = response.len(), direction = "recv", "[MockTransport] Raw packet");

            // DEBUG level shows more detailed parsing information
            debug!(
                "[MockTransport] Receive response details - Hex: {}, ASCII: {:?}",
                format_hex_pretty(&response),
                String::from_utf8_lossy(&response)
            );

            Ok(response.len())
        } else {
            // Simulate timeout
            warn!(
                "[MockTransport] Receive timeout - response queue empty, Timeout: {:?}",
                timeout.unwrap_or(Duration::from_secs(5))
            );
            Err(TransportError::Timeout("No response available".to_string()))
        }
    }

    async fn is_connected(&self) -> bool {
        let state = self.state.lock().await;
        state.connected
    }

    async fn connection_state(&self) -> ConnectionState {
        let state = self.state.lock().await;
        if state.connected {
            ConnectionState::Connected
        } else {
            ConnectionState::Disconnected
        }
    }

    async fn stats(&self) -> TransportStats {
        let state = self.state.lock().await;
        state.stats.clone()
    }

    async fn reset_stats(&mut self) {
        let mut state = self.state.lock().await;
        state.stats.reset();
    }

    async fn diagnostics(&self) -> HashMap<String, String> {
        let mut diag = HashMap::new();
        diag.insert(
            "transport_type".to_string(),
            self.transport_type().to_string(),
        );
        diag.insert("name".to_string(), self.name().to_string());
        diag.insert(
            "connected".to_string(),
            self.is_connected().await.to_string(),
        );
        diag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_transport_basic() {
        let config = MockTransportConfig::default();
        let mut transport = MockTransport::new(config);

        // Test connection
        assert!(transport.connect().await.is_ok());
        assert!(transport.is_connected().await);

        // Queue response
        transport.queue_response(vec![0x01, 0x02, 0x03]).await;

        // Test send
        let send_data = vec![0x10, 0x20, 0x30];
        assert!(transport.send(&send_data).await.is_ok());

        // Test receive
        let mut buffer = vec![0; 10];
        let len = transport.receive(&mut buffer, None).await.unwrap();
        assert_eq!(len, 3);
        assert_eq!(&buffer[..3], &[0x01, 0x02, 0x03]);

        // Check history
        let history = transport.get_send_history().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], send_data);

        // Test disconnect
        assert!(transport.disconnect().await.is_ok());
        assert!(!transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_mock_transport_failure() {
        let mut config = MockTransportConfig::default();
        config.fail_after_operations = 2;

        let mut transport = MockTransport::new(config);
        transport.connect().await.unwrap();

        // First two operations should succeed
        assert!(transport.send(&[0x01]).await.is_ok());
        assert!(transport.send(&[0x02]).await.is_ok());

        // Third operation should fail
        assert!(transport.send(&[0x03]).await.is_err());
    }
}
