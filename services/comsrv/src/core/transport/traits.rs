//! Transport Layer Traits
//!
//! This module defines the core traits and types for the transport layer,
//! providing a unified interface for different physical communication mechanisms.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{Duration, SystemTime};
use thiserror::Error;

/// Transport layer error types
#[derive(Error, Debug, Clone)]
pub enum TransportError {
    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Connection lost
    #[error("Connection lost: {0}")]
    ConnectionLost(String),

    /// Send operation failed
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// Receive operation failed  
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    /// Timeout occurred
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(String),

    /// Protocol-specific error
    #[error("Protocol error: {0}")]
    ProtocolError(String),
}

/// Connection state for transports
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Transport is disconnected
    Disconnected,
    /// Transport is attempting to connect
    Connecting,
    /// Transport is connected and ready
    Connected,
    /// Transport has encountered an error
    Error,
}

/// Transport statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportStats {
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Number of successful connections
    pub connection_attempts: u64,
    /// Number of successful connections
    pub successful_connections: u64,
    /// Number of failed connections
    pub failed_connections: u64,
    /// Number of disconnections
    pub disconnections: u64,
    /// Last successful connection time
    pub last_connection: Option<SystemTime>,
    /// Transport uptime
    pub uptime: Duration,
    /// Current connection state
    pub connection_state: ConnectionState,
}

impl TransportStats {
    /// Create new transport statistics
    pub fn new() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            connection_attempts: 0,
            successful_connections: 0,
            failed_connections: 0,
            disconnections: 0,
            last_connection: None,
            uptime: Duration::new(0, 0),
            connection_state: ConnectionState::Disconnected,
        }
    }

    /// Record a connection attempt
    pub fn record_connection_attempt(&mut self) {
        self.connection_attempts += 1;
    }

    /// Record a successful connection
    pub fn record_successful_connection(&mut self) {
        self.successful_connections += 1;
        self.last_connection = Some(SystemTime::now());
        self.connection_state = ConnectionState::Connected;
    }

    /// Record a failed connection
    pub fn record_failed_connection(&mut self) {
        self.failed_connections += 1;
        self.connection_state = ConnectionState::Error;
    }

    /// Record a disconnection
    pub fn record_disconnection(&mut self) {
        self.disconnections += 1;
        self.connection_state = ConnectionState::Disconnected;
    }

    /// Record bytes sent
    pub fn record_bytes_sent(&mut self, bytes: usize) {
        self.bytes_sent += bytes as u64;
    }

    /// Record bytes received
    pub fn record_bytes_received(&mut self, bytes: usize) {
        self.bytes_received += bytes as u64;
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for TransportStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Transport configuration trait
pub trait TransportConfig: Send + Sync + fmt::Debug + Clone {
    /// Get configuration name/identifier
    fn name(&self) -> &str;

    /// Validate configuration parameters
    fn validate(&self) -> std::result::Result<(), TransportError>;

    /// Get timeout duration
    fn timeout(&self) -> Duration;

    /// Get retry configuration
    fn max_retries(&self) -> u32;
}

/// Core transport trait defining the interface for all transport implementations
#[async_trait]
pub trait Transport: Send + Sync + fmt::Debug {
    /// Get transport type identifier
    fn transport_type(&self) -> &str;

    /// Get human-readable transport name
    fn name(&self) -> &str;

    /// Connect to the remote endpoint
    ///
    /// # Returns
    ///
    /// `Ok(())` if connection successful, `Err` otherwise
    async fn connect(&mut self) -> std::result::Result<(), TransportError>;

    /// Disconnect from the remote endpoint
    ///
    /// # Returns
    ///
    /// `Ok(())` if disconnection successful, `Err` otherwise
    async fn disconnect(&mut self) -> std::result::Result<(), TransportError>;

    /// Send data to the remote endpoint
    ///
    /// # Arguments
    ///
    /// * `data` - Data bytes to send
    ///
    /// # Returns
    ///
    /// `Ok(bytes_sent)` if successful, `Err` otherwise
    async fn send(&mut self, data: &[u8]) -> std::result::Result<usize, TransportError>;

    /// Receive data from the remote endpoint
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to store received data
    /// * `timeout` - Optional timeout for the receive operation
    ///
    /// # Returns
    ///
    /// `Ok(bytes_received)` if successful, `Err` otherwise
    async fn receive(
        &mut self,
        buffer: &mut [u8],
        timeout: Option<Duration>,
    ) -> std::result::Result<usize, TransportError>;

    /// Check if transport is currently connected
    ///
    /// # Returns
    ///
    /// `true` if connected, `false` otherwise
    async fn is_connected(&self) -> bool;

    /// Get current connection state
    ///
    /// # Returns
    ///
    /// Current connection state
    async fn connection_state(&self) -> ConnectionState;

    /// Get transport statistics
    ///
    /// # Returns
    ///
    /// Current transport statistics
    async fn stats(&self) -> TransportStats;

    /// Reset transport statistics
    async fn reset_stats(&mut self);

    /// Close the transport and clean up resources
    ///
    /// This method should be called when the transport is no longer needed.
    /// It performs any necessary cleanup and ensures resources are properly released.
    async fn close(&mut self) -> std::result::Result<(), TransportError> {
        self.disconnect().await
    }

    /// Get transport-specific diagnostic information
    ///
    /// # Returns
    ///
    /// Key-value pairs of diagnostic information
    async fn diagnostics(&self) -> std::collections::HashMap<String, String> {
        let mut diag = std::collections::HashMap::new();
        diag.insert(
            "transport_type".to_string(),
            self.transport_type().to_string(),
        );
        diag.insert("name".to_string(), self.name().to_string());
        diag.insert(
            "connected".to_string(),
            self.is_connected().await.to_string(),
        );
        diag.insert(
            "connection_state".to_string(),
            format!("{:?}", self.connection_state().await),
        );
        diag
    }
}

/// Transport factory trait for creating transport instances
#[async_trait]
pub trait TransportBuilder: Send + Sync {
    /// Transport configuration type
    type Config: TransportConfig;
    /// Transport implementation type
    type Transport: Transport;

    /// Create a new transport instance with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Transport configuration
    ///
    /// # Returns
    ///
    /// New transport instance
    async fn build(
        &self,
        config: Self::Config,
    ) -> std::result::Result<Self::Transport, TransportError>;

    /// Validate configuration without creating transport
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration to validate
    ///
    /// # Returns
    ///
    /// `Ok(())` if valid, `Err` with details if invalid
    fn validate_config(&self, config: &Self::Config) -> std::result::Result<(), TransportError> {
        config.validate()
    }

    /// Get default configuration for this transport type
    fn default_config(&self) -> Self::Config;
}

/// Implementation of Transport trait for Box<dyn Transport>
/// This allows Box<dyn Transport> to be used where Transport trait is required
#[async_trait]
impl Transport for Box<dyn Transport> {
    fn transport_type(&self) -> &str {
        self.as_ref().transport_type()
    }

    fn name(&self) -> &str {
        self.as_ref().name()
    }

    async fn connect(&mut self) -> std::result::Result<(), TransportError> {
        self.as_mut().connect().await
    }

    async fn disconnect(&mut self) -> std::result::Result<(), TransportError> {
        self.as_mut().disconnect().await
    }

    async fn send(&mut self, data: &[u8]) -> std::result::Result<usize, TransportError> {
        self.as_mut().send(data).await
    }

    async fn receive(
        &mut self,
        buffer: &mut [u8],
        timeout: Option<Duration>,
    ) -> std::result::Result<usize, TransportError> {
        self.as_mut().receive(buffer, timeout).await
    }

    async fn is_connected(&self) -> bool {
        self.as_ref().is_connected().await
    }

    async fn connection_state(&self) -> ConnectionState {
        self.as_ref().connection_state().await
    }

    async fn stats(&self) -> TransportStats {
        self.as_ref().stats().await
    }

    async fn reset_stats(&mut self) {
        self.as_mut().reset_stats().await
    }

    async fn close(&mut self) -> std::result::Result<(), TransportError> {
        self.as_mut().close().await
    }

    async fn diagnostics(&self) -> std::collections::HashMap<String, String> {
        self.as_ref().diagnostics().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_stats() {
        let mut stats = TransportStats::new();
        assert_eq!(stats.connection_attempts, 0);
        assert_eq!(stats.connection_state, ConnectionState::Disconnected);

        stats.record_connection_attempt();
        assert_eq!(stats.connection_attempts, 1);

        stats.record_successful_connection();
        assert_eq!(stats.successful_connections, 1);
        assert_eq!(stats.connection_state, ConnectionState::Connected);
        assert!(stats.last_connection.is_some());

        stats.record_bytes_sent(100);
        stats.record_bytes_received(50);
        assert_eq!(stats.bytes_sent, 100);
        assert_eq!(stats.bytes_received, 50);

        stats.record_disconnection();
        assert_eq!(stats.disconnections, 1);
        assert_eq!(stats.connection_state, ConnectionState::Disconnected);
    }

    #[test]
    fn test_transport_error() {
        let error = TransportError::ConnectionFailed("Test error".to_string());
        assert!(error.to_string().contains("Connection failed"));
        assert!(error.to_string().contains("Test error"));
    }
}
