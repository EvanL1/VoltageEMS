//! Serial Transport Implementation
//!
//! This module provides a serial port-based transport implementation that abstracts
//! serial communication details from protocol logic.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, error, info, warn};

use super::traits::{
    ConnectionState, Transport, TransportBuilder, TransportConfig, TransportError, TransportStats,
};

/// Serial port configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialTransportConfig {
    /// Serial port path (e.g., "/dev/ttyUSB0", "COM1")
    pub port: String,
    /// Baud rate
    pub baud_rate: u32,
    /// Data bits (5, 6, 7, 8)
    pub data_bits: u8,
    /// Stop bits (1, 2)
    pub stop_bits: u8,
    /// Parity ("None", "Even", "Odd")
    pub parity: String,
    /// Flow control ("None", "Software", "Hardware")
    pub flow_control: String,
    /// Connection timeout
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Read timeout for individual operations
    pub read_timeout: Duration,
    /// Write timeout for individual operations
    pub write_timeout: Duration,
}

impl Default for SerialTransportConfig {
    fn default() -> Self {
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: "None".to_string(),
            flow_control: "None".to_string(),
            timeout: Duration::from_secs(10),
            max_retries: 3,
            read_timeout: Duration::from_millis(1000),
            write_timeout: Duration::from_millis(1000),
        }
    }
}

impl TransportConfig for SerialTransportConfig {
    fn name(&self) -> &str {
        "serial"
    }

    fn validate(&self) -> Result<(), TransportError> {
        if self.port.is_empty() {
            return Err(TransportError::ConfigError(
                "Port path cannot be empty".to_string(),
            ));
        }

        if self.baud_rate == 0 {
            return Err(TransportError::ConfigError(
                "Baud rate must be greater than zero".to_string(),
            ));
        }

        if ![5, 6, 7, 8].contains(&self.data_bits) {
            return Err(TransportError::ConfigError(
                "Data bits must be 5, 6, 7, or 8".to_string(),
            ));
        }

        if ![1, 2].contains(&self.stop_bits) {
            return Err(TransportError::ConfigError(
                "Stop bits must be 1 or 2".to_string(),
            ));
        }

        if !["None", "Even", "Odd"].contains(&self.parity.as_str()) {
            return Err(TransportError::ConfigError(
                "Parity must be None, Even, or Odd".to_string(),
            ));
        }

        if !["None", "Software", "Hardware"].contains(&self.flow_control.as_str()) {
            return Err(TransportError::ConfigError(
                "Flow control must be None, Software, or Hardware".to_string(),
            ));
        }

        if self.timeout.is_zero() {
            return Err(TransportError::ConfigError(
                "Timeout must be greater than zero".to_string(),
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

/// Serial transport implementation
#[derive(Debug)]
pub struct SerialTransport {
    /// Transport configuration
    config: SerialTransportConfig,
    /// Serial port connection
    connection: Arc<RwLock<Option<SerialStream>>>,
    /// Transport statistics
    stats: Arc<RwLock<TransportStats>>,
    /// Connection start time for uptime calculation
    start_time: SystemTime,
}

impl SerialTransport {
    /// Create new serial transport with configuration
    pub fn new(config: SerialTransportConfig) -> Result<Self, TransportError> {
        config.validate()?;

        Ok(Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(TransportStats::new())),
            start_time: SystemTime::now(),
        })
    }

    /// Convert string parity to tokio_serial parity
    fn parse_parity(&self) -> tokio_serial::Parity {
        match self.config.parity.as_str() {
            "Even" => tokio_serial::Parity::Even,
            "Odd" => tokio_serial::Parity::Odd,
            _ => tokio_serial::Parity::None,
        }
    }

    /// Convert string flow control to tokio_serial flow control
    fn parse_flow_control(&self) -> tokio_serial::FlowControl {
        match self.config.flow_control.as_str() {
            "Software" => tokio_serial::FlowControl::Software,
            "Hardware" => tokio_serial::FlowControl::Hardware,
            _ => tokio_serial::FlowControl::None,
        }
    }

    /// Convert data bits to tokio_serial data bits
    fn parse_data_bits(&self) -> tokio_serial::DataBits {
        match self.config.data_bits {
            5 => tokio_serial::DataBits::Five,
            6 => tokio_serial::DataBits::Six,
            7 => tokio_serial::DataBits::Seven,
            8 => tokio_serial::DataBits::Eight,
            _ => tokio_serial::DataBits::Eight,
        }
    }

    /// Convert stop bits to tokio_serial stop bits
    fn parse_stop_bits(&self) -> tokio_serial::StopBits {
        match self.config.stop_bits {
            2 => tokio_serial::StopBits::Two,
            _ => tokio_serial::StopBits::One,
        }
    }
}

#[async_trait]
impl Transport for SerialTransport {
    fn transport_type(&self) -> &str {
        "serial"
    }

    fn name(&self) -> &str {
        "Serial Transport"
    }

    async fn connect(&mut self) -> Result<(), TransportError> {
        let mut stats = self.stats.write().await;
        stats.record_connection_attempt();
        stats.connection_state = ConnectionState::Connecting;
        drop(stats);

        debug!("Opening serial port: {}", self.config.port);

        // Build serial port configuration
        let port_result = tokio_serial::new(&self.config.port, self.config.baud_rate)
            .data_bits(self.parse_data_bits())
            .parity(self.parse_parity())
            .stop_bits(self.parse_stop_bits())
            .flow_control(self.parse_flow_control())
            .timeout(self.config.read_timeout)
            .open_native_async();

        match port_result {
            Ok(mut port) => {
                // Set timeouts
                #[cfg(unix)]
                port.set_exclusive(false).map_err(|e| {
                    TransportError::IoError(format!("Failed to set exclusive mode: {e}"))
                })?;

                // Store the connection
                let mut conn = self.connection.write().await;
                *conn = Some(port);
                drop(conn);

                // Update statistics
                let mut stats = self.stats.write().await;
                stats.record_successful_connection();

                info!("Successfully opened serial port: {}", self.config.port);
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to open serial port {}: {e}", self.config.port);
                error!("{error_msg}");

                let mut stats = self.stats.write().await;
                stats.record_failed_connection();

                Err(TransportError::ConnectionFailed(error_msg))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        let mut conn = self.connection.write().await;
        if conn.take().is_some() {
            // Serial port is automatically closed when dropped
            let mut stats = self.stats.write().await;
            stats.record_disconnection();
            info!("Closed serial port: {}", self.config.port);
        }
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        use tokio::io::AsyncWriteExt;

        let mut conn = self.connection.write().await;
        match conn.as_mut() {
            Some(port) => {
                let send_operation = async {
                    port.write_all(data).await?;
                    port.flush().await?;
                    Ok::<_, std::io::Error>(data.len())
                };

                match timeout(self.config.write_timeout, send_operation).await {
                    Ok(Ok(bytes_sent)) => {
                        drop(conn);

                        // Update statistics
                        let mut stats = self.stats.write().await;
                        stats.record_bytes_sent(bytes_sent);

                        debug!(hex_data = %data.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "), length = bytes_sent, direction = "send", "[Serial Transport] Raw packet");
                        debug!("Sent {} bytes via serial port", bytes_sent);
                        Ok(bytes_sent)
                    }
                    Ok(Err(e)) => {
                        let error_msg = format!("Failed to send data: {e}");
                        error!("{error_msg}");

                        // Connection might be broken, remove it
                        *conn = None;
                        drop(conn);

                        let mut stats = self.stats.write().await;
                        stats.connection_state = ConnectionState::Error;

                        Err(TransportError::SendFailed(error_msg))
                    }
                    Err(_) => {
                        let error_msg = format!(
                            "Send operation timed out after {:?}",
                            self.config.write_timeout
                        );
                        warn!("{error_msg}");
                        Err(TransportError::Timeout(error_msg))
                    }
                }
            }
            None => Err(TransportError::SendFailed(
                "Serial port not connected".to_string(),
            )),
        }
    }

    async fn receive(
        &mut self,
        buffer: &mut [u8],
        timeout_duration: Option<Duration>,
    ) -> Result<usize, TransportError> {
        use tokio::io::AsyncReadExt;

        let mut conn = self.connection.write().await;
        match conn.as_mut() {
            Some(port) => {
                let receive_timeout = timeout_duration.unwrap_or(self.config.read_timeout);

                match timeout(receive_timeout, port.read(buffer)).await {
                    Ok(Ok(bytes_read)) => {
                        if bytes_read == 0 {
                            // No data available
                            return Ok(0);
                        }

                        drop(conn);

                        // Update statistics
                        let mut stats = self.stats.write().await;
                        stats.record_bytes_received(bytes_read);

                        debug!(hex_data = %buffer[..bytes_read].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "), length = bytes_read, direction = "recv", "[Serial Transport] Raw packet");
                        debug!("Received {} bytes via serial port", bytes_read);
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
                        debug!("{error_msg}"); // Debug level for timeout, as it's often expected
                        Err(TransportError::Timeout(error_msg))
                    }
                }
            }
            None => Err(TransportError::ReceiveFailed(
                "Serial port not connected".to_string(),
            )),
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
        diag.insert("port".to_string(), self.config.port.clone());
        diag.insert("baud_rate".to_string(), self.config.baud_rate.to_string());
        diag.insert("data_bits".to_string(), self.config.data_bits.to_string());
        diag.insert("stop_bits".to_string(), self.config.stop_bits.to_string());
        diag.insert("parity".to_string(), self.config.parity.clone());
        diag.insert("flow_control".to_string(), self.config.flow_control.clone());
        diag.insert(
            "timeout_ms".to_string(),
            self.config.timeout.as_millis().to_string(),
        );
        diag.insert(
            "read_timeout_ms".to_string(),
            self.config.read_timeout.as_millis().to_string(),
        );
        diag.insert(
            "write_timeout_ms".to_string(),
            self.config.write_timeout.as_millis().to_string(),
        );
        diag.insert(
            "max_retries".to_string(),
            self.config.max_retries.to_string(),
        );
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

/// Serial transport builder
#[derive(Debug, Default)]
pub struct SerialTransportBuilder;

impl SerialTransportBuilder {
    /// Create new serial transport builder
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TransportBuilder for SerialTransportBuilder {
    type Config = SerialTransportConfig;
    type Transport = SerialTransport;

    async fn build(&self, config: Self::Config) -> Result<Self::Transport, TransportError> {
        SerialTransport::new(config)
    }

    fn default_config(&self) -> Self::Config {
        SerialTransportConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_config_validation() {
        let mut config = SerialTransportConfig::default();
        assert!(config.validate().is_ok());

        config.port = "".to_string();
        assert!(config.validate().is_err());

        config.port = "/dev/ttyUSB0".to_string();
        config.baud_rate = 0;
        assert!(config.validate().is_err());

        config.baud_rate = 9600;
        config.data_bits = 9; // Invalid
        assert!(config.validate().is_err());

        config.data_bits = 8;
        config.stop_bits = 3; // Invalid
        assert!(config.validate().is_err());

        config.stop_bits = 1;
        config.parity = "Invalid".to_string();
        assert!(config.validate().is_err());

        config.parity = "None".to_string();
        config.flow_control = "Invalid".to_string();
        assert!(config.validate().is_err());

        config.flow_control = "None".to_string();
        config.timeout = Duration::from_secs(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_serial_transport_creation() {
        let config = SerialTransportConfig::default();
        let _transport = SerialTransport::new(config);
        assert!(transport.is_ok());

        let _transport = transport.unwrap();
        assert_eq!(transport.transport_type(), "serial");
        assert_eq!(transport.name(), "Serial Transport");
    }

    #[tokio::test]
    async fn test_serial_transport_not_connected_initially() {
        let config = SerialTransportConfig::default();
        let _transport = SerialTransport::new(config).unwrap();

        assert!(!transport.is_connected().await);
        assert_eq!(
            transport.connection_state().await,
            ConnectionState::Disconnected
        );
    }

    #[tokio::test]
    async fn test_serial_transport_stats() {
        let config = SerialTransportConfig::default();
        let mut transport = SerialTransport::new(config).unwrap();

        let stats = transport.stats().await;
        assert_eq!(stats.connection_attempts, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.bytes_received, 0);

        transport.reset_stats().await;
        let stats = transport.stats().await;
        assert_eq!(stats.connection_attempts, 0);
    }

    #[tokio::test]
    async fn test_serial_transport_builder() {
        let builder = SerialTransportBuilder::new();
        let config = builder.default_config();
        assert_eq!(config.port, "/dev/ttyUSB0");
        assert_eq!(config.baud_rate, 9600);

        let _transport = builder.build(config).await;
        assert!(transport.is_ok());
    }

    #[test]
    fn test_serial_config_parsing() {
        let config = SerialTransportConfig {
            parity: "Even".to_string(),
            flow_control: "Hardware".to_string(),
            data_bits: 7,
            stop_bits: 2,
            ..Default::default()
        };

        let _transport = SerialTransport::new(config).unwrap();
        assert_eq!(transport.parse_parity(), tokio_serial::Parity::Even);
        assert_eq!(
            transport.parse_flow_control(),
            tokio_serial::FlowControl::Hardware
        );
        assert_eq!(transport.parse_data_bits(), tokio_serial::DataBits::Seven);
        assert_eq!(transport.parse_stop_bits(), tokio_serial::StopBits::Two);
    }
}
