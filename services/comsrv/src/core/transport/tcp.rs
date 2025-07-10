//! TCP Transport Implementation
//!
//! This module provides a TCP-based transport implementation that abstracts
//! network communication details from protocol logic.

use crate::utils::hex::format_hex_pretty;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use super::traits::{
    ConnectionState, Transport, TransportBuilder, TransportConfig, TransportError, TransportStats,
};

/// TCP transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpTransportConfig {
    /// Remote host address
    pub host: String,
    /// Remote port number
    pub port: u16,
    /// Connection timeout
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Keep-alive configuration
    pub keep_alive: Option<Duration>,
    /// Socket receive buffer size
    pub recv_buffer_size: Option<usize>,
    /// Socket send buffer size
    pub send_buffer_size: Option<usize>,
    /// TCP no-delay (Nagle algorithm)
    pub no_delay: bool,
}

impl Default for TcpTransportConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 502,
            timeout: Duration::from_secs(10),
            max_retries: 3,
            keep_alive: Some(Duration::from_secs(60)),
            recv_buffer_size: None,
            send_buffer_size: None,
            no_delay: true,
        }
    }
}

impl TransportConfig for TcpTransportConfig {
    fn name(&self) -> &str {
        "tcp"
    }

    fn validate(&self) -> Result<(), TransportError> {
        if self.host.is_empty() {
            return Err(TransportError::ConfigError(
                "Host cannot be empty".to_string(),
            ));
        }

        if self.port == 0 {
            return Err(TransportError::ConfigError(
                "Port cannot be zero".to_string(),
            ));
        }

        if self.timeout.is_zero() {
            return Err(TransportError::ConfigError(
                "Timeout must be greater than zero".to_string(),
            ));
        }

        // No need to parse address here - it will be resolved during connection
        // This allows hostname support (e.g., "modbus_tcp_simulator" instead of IP)

        Ok(())
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// TCP transport implementation
#[derive(Debug)]
pub struct TcpTransport {
    /// Transport configuration
    config: TcpTransportConfig,
    /// TCP connection
    connection: Arc<RwLock<Option<TcpStream>>>,
    /// Transport statistics
    stats: Arc<RwLock<TransportStats>>,
    /// Connection start time for uptime calculation
    start_time: SystemTime,
}

impl TcpTransport {
    /// Create new TCP transport with configuration
    pub fn new(config: TcpTransportConfig) -> Result<Self, TransportError> {
        config.validate()?;

        Ok(Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(TransportStats::new())),
            start_time: SystemTime::now(),
        })
    }

    /// Get the socket address for connection
    fn socket_addr(&self) -> Result<String, TransportError> {
        Ok(format!("{}:{}", self.config.host, self.config.port))
    }

    /// Configure TCP socket options
    async fn configure_socket(&self, stream: &TcpStream) -> Result<(), TransportError> {
        // Configure socket options for better performance and reliability
        #[cfg(unix)]
        {
            use std::os::unix::io::{AsRawFd, FromRawFd};
            let socket = unsafe { socket2::Socket::from_raw_fd(stream.as_raw_fd()) };

            // Enable TCP keep-alive
            if let Err(e) = socket.set_keepalive(true) {
                warn!("Failed to set keep-alive: {e}");
            }

            // Set TCP_NODELAY to disable Nagle's algorithm
            if let Err(e) = socket.set_nodelay(true) {
                warn!("Failed to set TCP_NODELAY: {e}");
            }

            // Forget the socket to avoid closing the file descriptor
            std::mem::forget(socket);
        }

        #[cfg(windows)]
        {
            use std::os::windows::io::{AsRawSocket, FromRawSocket};
            let socket = unsafe { socket2::Socket::from_raw_socket(stream.as_raw_socket()) };

            // Enable TCP keep-alive
            if let Err(e) = socket.set_keepalive(true) {
                warn!("Failed to set keep-alive: {e}");
            }

            // Set TCP_NODELAY to disable Nagle's algorithm
            if let Err(e) = socket.set_nodelay(true) {
                warn!("Failed to set TCP_NODELAY: {e}");
            }

            // Forget the socket to avoid closing the socket handle
            std::mem::forget(socket);
        }

        Ok(())
    }
}

#[async_trait]
impl Transport for TcpTransport {
    fn transport_type(&self) -> &str {
        "tcp"
    }

    fn name(&self) -> &str {
        "TCP Transport"
    }

    async fn connect(&mut self) -> Result<(), TransportError> {
        let mut stats = self.stats.write().await;
        stats.record_connection_attempt();
        stats.connection_state = ConnectionState::Connecting;
        drop(stats);

        let addr = self.socket_addr()?;
        debug!("Connecting to TCP endpoint: {addr}");

        // Attempt connection with timeout
        let connection_result = timeout(self.config.timeout, TcpStream::connect(&addr)).await;

        match connection_result {
            Ok(Ok(stream)) => {
                // Configure socket options
                self.configure_socket(&stream).await?;

                // Store the connection
                let mut conn = self.connection.write().await;
                *conn = Some(stream);
                drop(conn);

                // Update statistics
                let mut stats = self.stats.write().await;
                stats.record_successful_connection();

                info!("Successfully connected to TCP endpoint: {addr}");
                Ok(())
            }
            Ok(Err(e)) => {
                let error_msg = format!("Failed to connect to {}: {e}", addr);
                error!("{error_msg}");

                let mut stats = self.stats.write().await;
                stats.record_failed_connection();

                Err(TransportError::ConnectionFailed(error_msg))
            }
            Err(_) => {
                let error_msg = format!("Connection to {} timed out", addr);
                warn!("{error_msg}");

                let mut stats = self.stats.write().await;
                stats.record_failed_connection();

                Err(TransportError::Timeout(error_msg))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        let mut conn = self.connection.write().await;
        if let Some(mut stream) = conn.take() {
            if let Err(e) = stream.shutdown().await {
                warn!("Error during TCP shutdown: {e}");
            }
        }

        let mut stats = self.stats.write().await;
        stats.record_disconnection();

        info!("Disconnected from TCP endpoint");
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        let mut conn = self.connection.write().await;
        match conn.as_mut() {
            Some(stream) => {
                match stream.write_all(data).await {
                    Ok(()) => {
                        let bytes_sent = data.len();
                        drop(conn);

                        // Update statistics
                        let mut stats = self.stats.write().await;
                        stats.record_bytes_sent(bytes_sent);

                        debug!(hex_data = %format_hex_pretty(data), length = bytes_sent, direction = "send", "[TCP Transport] Raw packet");
                        debug!("Sent {} bytes via TCP", bytes_sent);
                        Ok(bytes_sent)
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to send data: {e}");
                        error!("{error_msg}");

                        // Connection might be broken, remove it
                        *conn = None;
                        drop(conn);

                        let mut stats = self.stats.write().await;
                        stats.connection_state = ConnectionState::Error;

                        Err(TransportError::SendFailed(error_msg))
                    }
                }
            }
            None => Err(TransportError::SendFailed("Not connected".to_string())),
        }
    }

    async fn receive(
        &mut self,
        buffer: &mut [u8],
        timeout_duration: Option<Duration>,
    ) -> Result<usize, TransportError> {
        let mut conn = self.connection.write().await;
        match conn.as_mut() {
            Some(stream) => {
                let receive_timeout = timeout_duration.unwrap_or(self.config.timeout);

                match timeout(receive_timeout, stream.read(buffer)).await {
                    Ok(Ok(bytes_read)) => {
                        if bytes_read == 0 {
                            // Connection closed by peer
                            warn!("TCP connection closed by peer");
                            *conn = None;
                            drop(conn);

                            let mut stats = self.stats.write().await;
                            stats.record_disconnection();

                            return Err(TransportError::ConnectionLost(
                                "Connection closed by peer".to_string(),
                            ));
                        }

                        drop(conn);

                        // Update statistics
                        let mut stats = self.stats.write().await;
                        stats.record_bytes_received(bytes_read);

                        debug!(hex_data = %format_hex_pretty(&buffer[..bytes_read]), length = bytes_read, direction = "recv", "[TCP Transport] Raw packet");
                        debug!("Received {} bytes via TCP", bytes_read);
                        Ok(bytes_read)
                    }
                    Ok(Err(e)) => {
                        let error_msg = format!("Failed to receive data: {e}");
                        error!("{error_msg}");

                        // Connection might be broken, remove it
                        *conn = None;
                        drop(conn);

                        let mut stats = self.stats.write().await;
                        stats.connection_state = ConnectionState::Error;

                        Err(TransportError::ReceiveFailed(error_msg))
                    }
                    Err(_) => {
                        let error_msg =
                            format!("Receive operation timed out after {receive_timeout:?}");
                        warn!("{error_msg}");
                        Err(TransportError::Timeout(error_msg))
                    }
                }
            }
            None => Err(TransportError::ReceiveFailed("Not connected".to_string())),
        }
    }

    async fn is_connected(&self) -> bool {
        let conn = self.connection.read().await;
        conn.is_some()
    }

    async fn connection_state(&self) -> ConnectionState {
        let stats = self.stats.read().await;
        stats.connection_state
    }

    async fn stats(&self) -> TransportStats {
        let mut stats = self.stats.read().await.clone();

        // Update uptime
        if let Ok(elapsed) = self.start_time.elapsed() {
            stats.uptime = elapsed;
        }

        stats
    }

    async fn reset_stats(&mut self) {
        let mut stats = self.stats.write().await;
        stats.reset();
    }

    async fn diagnostics(&self) -> std::collections::HashMap<String, String> {
        let mut diag = std::collections::HashMap::new();

        diag.insert(
            "transport_type".to_string(),
            self.transport_type().to_string(),
        );
        diag.insert("name".to_string(), self.name().to_string());
        diag.insert("host".to_string(), self.config.host.clone());
        diag.insert("port".to_string(), self.config.port.to_string());
        diag.insert(
            "timeout_ms".to_string(),
            self.config.timeout.as_millis().to_string(),
        );
        diag.insert(
            "max_retries".to_string(),
            self.config.max_retries.to_string(),
        );
        diag.insert("no_delay".to_string(), self.config.no_delay.to_string());
        diag.insert(
            "connected".to_string(),
            self.is_connected().await.to_string(),
        );
        diag.insert(
            "connection_state".to_string(),
            format!("{:?}", self.connection_state().await),
        );

        if let Some(keep_alive) = self.config.keep_alive {
            diag.insert(
                "keep_alive_secs".to_string(),
                keep_alive.as_secs().to_string(),
            );
        }

        diag
    }
}

/// TCP transport builder
#[derive(Debug, Default)]
pub struct TcpTransportBuilder;

impl TcpTransportBuilder {
    /// Create new TCP transport builder
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TransportBuilder for TcpTransportBuilder {
    type Config = TcpTransportConfig;
    type Transport = TcpTransport;

    async fn build(&self, config: Self::Config) -> Result<Self::Transport, TransportError> {
        TcpTransport::new(config)
    }

    fn default_config(&self) -> Self::Config {
        TcpTransportConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[test]
    fn test_tcp_config_validation() {
        let mut config = TcpTransportConfig::default();
        assert!(config.validate().is_ok());

        config.host = "".to_string();
        assert!(config.validate().is_err());

        config.host = "127.0.0.1".to_string();
        config.port = 0;
        assert!(config.validate().is_err());

        config.port = 502;
        config.timeout = Duration::from_secs(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_tcp_transport_creation() {
        let config = TcpTransportConfig::default();
        let _transport = TcpTransport::new(config);
        assert!(_transport.is_ok());

        let _transport = _transport.unwrap();
        assert_eq!(_transport.transport_type(), "tcp");
        assert_eq!(_transport.name(), "TCP Transport");
    }

    #[tokio::test]
    async fn test_tcp_transport_not_connected_initially() {
        let config = TcpTransportConfig::default();
        let _transport = TcpTransport::new(config).unwrap();

        assert!(!_transport.is_connected().await);
        assert_eq!(
            _transport.connection_state().await,
            ConnectionState::Disconnected
        );
    }

    #[tokio::test]
    async fn test_tcp_transport_stats() {
        let config = TcpTransportConfig::default();
        let mut transport = TcpTransport::new(config).unwrap();

        let stats = transport.stats().await;
        assert_eq!(stats.connection_attempts, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.bytes_received, 0);

        transport.reset_stats().await;
        let stats = transport.stats().await;
        assert_eq!(stats.connection_attempts, 0);
    }

    #[tokio::test]
    async fn test_tcp_transport_builder() {
        let builder = TcpTransportBuilder::new();
        let config = builder.default_config();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 502);

        let _transport = builder.build(config).await;
        assert!(_transport.is_ok());
    }
}
