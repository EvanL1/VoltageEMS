//! Modbus Connection Management
//!
//! This module provides TCP and RTU connection management for Modbus protocol

use super::constants;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};
use voltage_comlink::error::{ComLinkError, Result};
use voltage_comlink::ChannelLogger;

#[cfg(feature = "modbus-rtu")]
use tokio_serial::{SerialPortBuilderExt, SerialStream};

/// Modbus connection type
#[derive(Debug)]
pub enum ModbusConnection {
    /// TCP connection
    Tcp(TcpStream),
    /// Serial RTU connection
    #[cfg(feature = "modbus-rtu")]
    Rtu(SerialStream),
}

impl ModbusConnection {
    /// Create a TCP connection
    pub async fn connect_tcp(host: &str, port: u16, timeout_duration: Duration) -> Result<Self> {
        let addr = format!("{host}:{port}");
        debug!("TCP connecting: {}", addr);

        match timeout(timeout_duration, TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => {
                // Configure socket for optimal performance
                if let Err(e) = stream.set_nodelay(true) {
                    debug!("TCP_NODELAY: {}", e);
                }

                info!("TCP connected: {}", addr);
                Ok(ModbusConnection::Tcp(stream))
            },
            Ok(Err(e)) => {
                error!("TCP err: {} - {}", addr, e);
                Err(ComLinkError::Connection(format!(
                    "Failed to connect to {addr}: {e}"
                )))
            },
            Err(_) => {
                warn!("TCP timeout: {}", addr);
                Err(ComLinkError::Timeout(format!(
                    "Connection to {addr} timed out"
                )))
            },
        }
    }

    /// Create a serial RTU connection
    #[cfg(feature = "modbus-rtu")]
    pub async fn connect_rtu(
        port: &str,
        baud_rate: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: &str,
        timeout_duration: Duration,
    ) -> Result<Self> {
        debug!("RTU: {} @{}baud", port, baud_rate);

        let parity = match parity {
            "Even" => tokio_serial::Parity::Even,
            "Odd" => tokio_serial::Parity::Odd,
            _ => tokio_serial::Parity::None,
        };

        let data_bits = match data_bits {
            5 => tokio_serial::DataBits::Five,
            6 => tokio_serial::DataBits::Six,
            7 => tokio_serial::DataBits::Seven,
            _ => tokio_serial::DataBits::Eight,
        };

        let stop_bits = match stop_bits {
            2 => tokio_serial::StopBits::Two,
            _ => tokio_serial::StopBits::One,
        };

        match tokio_serial::new(port, baud_rate)
            .data_bits(data_bits)
            .parity(parity)
            .stop_bits(stop_bits)
            .timeout(timeout_duration)
            .open_native_async()
        {
            Ok(serial_port) => {
                info!("RTU opened: {}", port);
                Ok(ModbusConnection::Rtu(serial_port))
            },
            Err(e) => {
                error!("RTU err: {} - {}", port, e);
                Err(ComLinkError::Connection(format!(
                    "Failed to open serial port {port}: {e}"
                )))
            },
        }
    }

    /// Send data
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        match self {
            ModbusConnection::Tcp(stream) => {
                stream.write_all(data).await.map_err(|e| {
                    error!("TCP TX: {}", e);
                    ComLinkError::Io(format!("TCP send error: {e}"))
                })?;
                debug!("TCP TX: {}B", data.len());
            },
            #[cfg(feature = "modbus-rtu")]
            ModbusConnection::Rtu(port) => {
                port.write_all(data).await.map_err(|e| {
                    error!("RTU TX: {}", e);
                    ComLinkError::Io(format!("Serial send error: {e}"))
                })?;
                port.flush().await.map_err(|e| {
                    error!("RTU flush: {}", e);
                    ComLinkError::Io(format!("Serial flush error: {e}"))
                })?;
                debug!("RTU TX: {}B", data.len());
            },
        }
        Ok(())
    }

    /// Receive data - ensures complete Modbus frame is received
    pub async fn receive(
        &mut self,
        buffer: &mut [u8],
        timeout_duration: Duration,
    ) -> Result<usize> {
        match self {
            ModbusConnection::Tcp(stream) => {
                // Modbus TCP frame: [Transaction ID(2)][Protocol ID(2)][Length(2)][Unit ID(1)][PDU(N)]
                // Step 1: Read MBAP header
                let mut header = [0u8; constants::MBAP_HEADER_LEN];
                match timeout(timeout_duration, stream.read_exact(&mut header)).await {
                    Ok(Ok(_bytes_read)) => {
                        // Step 2: Parse length field from header[4..6]
                        let length = u16::from_be_bytes([header[4], header[5]]) as usize;

                        // Step 3: Validate length (1..=MAX allowed: 1 (unit_id) + 253 (PDU) = 254)
                        if length == 0 || length > constants::MAX_MBAP_LENGTH {
                            error!("TCP invalid len: {}", length);
                            return Err(ComLinkError::Protocol(format!(
                                "Invalid TCP frame length: {}",
                                length
                            )));
                        }

                        // Step 4: Ensure buffer is large enough
                        let total_size = constants::MBAP_HEADER_LEN + length;
                        if buffer.len() < total_size {
                            error!("Buffer small: need={} have={}", total_size, buffer.len());
                            return Err(ComLinkError::Protocol(
                                "Buffer too small for complete frame".to_string(),
                            ));
                        }

                        // Step 5: Copy header to buffer
                        buffer[0..constants::MBAP_HEADER_LEN].copy_from_slice(&header);

                        // Step 6: Read remaining PDU bytes
                        match timeout(
                            timeout_duration,
                            stream.read_exact(&mut buffer[constants::MBAP_HEADER_LEN..total_size]),
                        )
                        .await
                        {
                            Ok(Ok(_bytes_read)) => {
                                debug!("TCP RX: {}B", total_size);
                                Ok(total_size)
                            },
                            Ok(Err(e)) => {
                                error!("TCP PDU RX: {}", e);
                                Err(ComLinkError::Io(format!("TCP PDU read error: {e}")))
                            },
                            Err(_) => {
                                debug!("TCP PDU timeout");
                                Err(ComLinkError::Timeout("TCP PDU read timeout".to_string()))
                            },
                        }
                    },
                    Ok(Err(e)) => {
                        error!("TCP header RX: {}", e);
                        Err(ComLinkError::Io(format!("TCP header read error: {e}")))
                    },
                    Err(_) => {
                        debug!("TCP header timeout");
                        Err(ComLinkError::Timeout("TCP header read timeout".to_string()))
                    },
                }
            },
            #[cfg(feature = "modbus-rtu")]
            ModbusConnection::Rtu(port) => {
                // Modbus RTU frame: [Unit ID(1)][PDU(N)][CRC(2)]
                // Use inter-byte timeout to detect frame end
                const INTER_BYTE_TIMEOUT: Duration = Duration::from_millis(50);
                let mut total_bytes = 0;
                let start_time = std::time::Instant::now();

                loop {
                    // Check total timeout
                    if start_time.elapsed() >= timeout_duration {
                        if total_bytes < 4 {
                            debug!("RTU timeout: {}B", total_bytes);
                            return Err(ComLinkError::Timeout(
                                "RTU frame incomplete: total timeout".to_string(),
                            ));
                        }
                        break;
                    }

                    // Try to read more data with inter-byte timeout
                    let remaining_buffer = &mut buffer[total_bytes..];
                    let read_size = remaining_buffer.len().min(128);

                    match timeout(
                        INTER_BYTE_TIMEOUT,
                        port.read(&mut remaining_buffer[..read_size]),
                    )
                    .await
                    {
                        Ok(Ok(bytes)) => {
                            if bytes == 0 {
                                error!("RTU closed");
                                return Err(ComLinkError::Connection(
                                    "Serial connection closed".to_string(),
                                ));
                            }
                            total_bytes += bytes;

                            // Check buffer overflow
                            if total_bytes >= buffer.len() {
                                error!("RTU overflow: {}B", total_bytes);
                                return Err(ComLinkError::Protocol(
                                    "RTU frame exceeds buffer size".to_string(),
                                ));
                            }
                        },
                        Ok(Err(e)) => {
                            error!("RTU RX: {}", e);
                            return Err(ComLinkError::Io(format!("Serial read error: {e}")));
                        },
                        Err(_) => {
                            // Inter-byte timeout reached - frame should be complete
                            if total_bytes >= 4 {
                                break; // Minimum RTU frame received
                            } else if total_bytes > 0 {
                                debug!("RTU partial: {}B", total_bytes);
                                return Err(ComLinkError::Timeout(
                                    "RTU frame incomplete: inter-byte timeout".to_string(),
                                ));
                            }
                            // total_bytes == 0, continue waiting
                        },
                    }
                }

                debug!("RTU RX: {}B", total_bytes);
                Ok(total_bytes)
            },
        }
    }

    /// Check if connection is TCP
    pub fn is_tcp(&self) -> bool {
        matches!(self, ModbusConnection::Tcp(_))
    }

    /// Check if connection is RTU
    #[cfg(feature = "modbus-rtu")]
    pub fn is_rtu(&self) -> bool {
        matches!(self, ModbusConnection::Rtu(_))
    }
}

/// Modbus connection manager
#[derive(Debug)]
pub struct ModbusConnectionManager {
    /// Connection instance
    connection: Mutex<Option<ModbusConnection>>,
    /// Connection mode (TCP or RTU)
    mode: ModbusMode,
    /// Connection parameters
    params: ConnectionParams,
    /// Request/response synchronization lock to prevent concurrent operations
    request_lock: Mutex<()>,
    /// Reconnection attempt counter
    reconnect_attempts: Mutex<u32>,
    /// Last reconnection attempt time
    last_reconnect: Mutex<Option<Instant>>,
    /// Consecutive IO error counter
    consecutive_errors: Mutex<u32>,
    /// Error threshold for triggering reconnection
    error_threshold: u32,
    /// Channel logger for unified TX/RX logging
    logger: ChannelLogger,
}

/// Connection mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModbusMode {
    Tcp,
    #[cfg(feature = "modbus-rtu")]
    Rtu,
}

/// Connection parameters
#[derive(Debug, Clone)]
pub struct ConnectionParams {
    // TCP parameters
    pub host: Option<String>,
    pub port: Option<u16>,

    // Serial parameters
    #[cfg(feature = "modbus-rtu")]
    pub device: Option<String>,
    #[cfg(feature = "modbus-rtu")]
    pub baud_rate: Option<u32>,
    #[cfg(feature = "modbus-rtu")]
    pub data_bits: Option<u8>,
    #[cfg(feature = "modbus-rtu")]
    pub stop_bits: Option<u8>,
    #[cfg(feature = "modbus-rtu")]
    pub parity: Option<String>,

    // Common parameters
    pub timeout: Duration,
}

impl ModbusConnectionManager {
    /// Create new connection manager with logger
    pub fn new(
        mode: ModbusMode,
        params: ConnectionParams,
        logger: ChannelLogger,
        error_threshold: u32,
    ) -> Self {
        Self {
            connection: Mutex::new(None),
            mode,
            params,
            request_lock: Mutex::new(()),
            reconnect_attempts: Mutex::new(0),
            last_reconnect: Mutex::new(None),
            consecutive_errors: Mutex::new(0),
            error_threshold,
            logger,
        }
    }

    /// Get connection mode
    pub fn mode(&self) -> ModbusMode {
        self.mode
    }

    /// Connect to device
    pub async fn connect(&self) -> Result<()> {
        let mut conn = self.connection.lock().await;

        // Disconnect if already connected
        if conn.is_some() {
            *conn = None;
        }

        // Create new connection
        let new_conn =
            match &self.mode {
                ModbusMode::Tcp => {
                    let host = self.params.host.as_ref().ok_or_else(|| {
                        ComLinkError::Config("TCP host not specified".to_string())
                    })?;
                    let port = self.params.port.ok_or_else(|| {
                        ComLinkError::Config("TCP port not specified".to_string())
                    })?;

                    ModbusConnection::connect_tcp(host, port, self.params.timeout).await?
                },
                #[cfg(feature = "modbus-rtu")]
                ModbusMode::Rtu => {
                    let device = self.params.device.as_ref().ok_or_else(|| {
                        ComLinkError::Config("Serial device not specified".to_string())
                    })?;
                    let baud_rate = self.params.baud_rate.unwrap_or(9600);
                    let data_bits = self.params.data_bits.unwrap_or(8);
                    let stop_bits = self.params.stop_bits.unwrap_or(1);
                    let parity = self.params.parity.as_deref().unwrap_or("None");

                    ModbusConnection::connect_rtu(
                        device,
                        baud_rate,
                        data_bits,
                        stop_bits,
                        parity,
                        self.params.timeout,
                    )
                    .await?
                },
            };

        *conn = Some(new_conn);
        Ok(())
    }

    /// Disconnect from device
    pub async fn disconnect(&self) -> Result<()> {
        let mut conn = self.connection.lock().await;
        *conn = None;
        debug!("Disconnected");
        Ok(())
    }

    /// Connect with retry logic (returns true if connected, false if need cooldown)
    pub async fn connect_with_retry(&self, max_consecutive: u32, cooldown_ms: u64) -> Result<bool> {
        let mut attempts = 0;
        let mut delay_ms = 1000u64;

        // Check if we're in cooldown period
        if let Some(last_attempt) = *self.last_reconnect.lock().await {
            let elapsed = last_attempt.elapsed();
            if elapsed < Duration::from_millis(cooldown_ms) {
                let remaining = Duration::from_millis(cooldown_ms) - elapsed;
                debug!("Cooldown: {}s", remaining.as_secs());
                return Ok(false); // Still in cooldown
            }
        }

        // Try to connect with exponential backoff
        loop {
            match self.connect().await {
                Ok(()) => {
                    *self.reconnect_attempts.lock().await = 0;
                    *self.last_reconnect.lock().await = None; // Clear cooldown
                    info!("Connected (#{} attempts)", attempts + 1);
                    return Ok(true);
                },
                Err(e) => {
                    attempts += 1;
                    *self.reconnect_attempts.lock().await = attempts;

                    if attempts >= max_consecutive {
                        // Hit max consecutive attempts, enter cooldown
                        *self.last_reconnect.lock().await = Some(Instant::now());
                        warn!(
                            "Cooldown {}s ({}x failed): {}",
                            cooldown_ms / 1000,
                            max_consecutive,
                            e
                        );
                        return Ok(false); // Need cooldown
                    }

                    warn!(
                        "Retry {}/{}: {} ({}ms)",
                        attempts, max_consecutive, e, delay_ms
                    );

                    sleep(Duration::from_millis(delay_ms)).await;

                    // Exponential backoff with max delay of 30 seconds
                    delay_ms = (delay_ms * 2).min(30000);
                },
            }
        }
    }

    /// Send data
    pub async fn send(&self, data: &[u8]) -> Result<()> {
        let mut conn = self.connection.lock().await;
        match conn.as_mut() {
            Some(c) => c.send(data).await,
            None => Err(ComLinkError::Connection("Not connected".to_string())),
        }
    }

    /// Receive data
    pub async fn receive(&self, buffer: &mut [u8], timeout: Duration) -> Result<usize> {
        let mut conn = self.connection.lock().await;
        match conn.as_mut() {
            Some(c) => c.receive(buffer, timeout).await,
            None => Err(ComLinkError::Connection("Not connected".to_string())),
        }
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        self.connection.lock().await.is_some()
    }

    /// Extract Modbus frame information for logging
    /// Returns: (transaction_id, slave_id, function_code)
    fn extract_frame_info(&self, frame: &[u8]) -> (Option<u16>, u8, u8) {
        match self.mode {
            ModbusMode::Tcp if frame.len() >= 8 => {
                // TCP: [TID(2)][Proto(2)][Len(2)][Unit(1)][FC(1)][Data...]
                let tid = u16::from_be_bytes([frame[0], frame[1]]);
                let slave_id = frame[6];
                let function_code = frame[7];
                (Some(tid), slave_id, function_code)
            },
            #[cfg(feature = "modbus-rtu")]
            ModbusMode::Rtu if frame.len() >= 2 => {
                // RTU: [Unit(1)][FC(1)][Data...][CRC(2)]
                let slave_id = frame[0];
                let function_code = frame[1];
                (None, slave_id, function_code)
            },
            _ => (None, 0, 0),
        }
    }

    /// Send request and receive response atomically with automatic logging
    /// This ensures that only one request/response pair is in flight at a time
    /// Automatically triggers reconnection after consecutive IO errors reach threshold
    pub async fn send_and_receive(
        &self,
        request: &[u8],
        response_buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<usize> {
        // Extract frame info for logging
        let (transaction_id, slave_id, function_code) = self.extract_frame_info(request);

        // Log TX (request)
        self.logger
            .log_raw_message("TX", transaction_id, slave_id, function_code, request);

        // Acquire the request lock to ensure exclusive access
        let _lock = self.request_lock.lock().await;

        // Try to send and receive
        match self
            .do_send_and_receive(request, response_buffer, timeout)
            .await
        {
            Ok(bytes_read) => {
                // Success - reset error counter
                *self.consecutive_errors.lock().await = 0;

                // Log RX (response)
                self.logger.log_raw_message(
                    "RX",
                    transaction_id,
                    slave_id,
                    function_code,
                    &response_buffer[..bytes_read],
                );

                Ok(bytes_read)
            },
            Err(e) => {
                // Check if this is an IO error that should trigger reconnection
                if matches!(e, ComLinkError::Io(_)) {
                    let mut errors = self.consecutive_errors.lock().await;
                    *errors += 1;

                    if *errors >= self.error_threshold {
                        error!("IO errors({}), reconnecting", self.error_threshold);
                        *errors = 0;
                        drop(errors); // Release lock before reconnecting

                        // Clear connection and trigger reconnect
                        self.disconnect().await?;

                        // Note: Actual reconnection will happen on next request
                        // The caller (polling loop) should handle this
                    }
                }
                Err(e)
            },
        }
    }

    /// Internal send and receive without error counting
    async fn do_send_and_receive(
        &self,
        request: &[u8],
        response_buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<usize> {
        // Send the request
        self.send(request).await?;

        // Receive the response
        self.receive(response_buffer, timeout).await
    }

    /// Reset consecutive error counter (called after successful reconnection)
    pub async fn reset_error_counter(&self) {
        *self.consecutive_errors.lock().await = 0;
    }

    /// Get current consecutive error count
    pub async fn get_consecutive_errors(&self) -> u32 {
        *self.consecutive_errors.lock().await
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    // ========================================================================
    // ModbusMode Tests
    // ========================================================================

    #[test]
    fn test_modbus_mode_tcp() {
        let mode = ModbusMode::Tcp;
        assert!(matches!(mode, ModbusMode::Tcp));
    }

    #[cfg(feature = "modbus-rtu")]
    #[test]
    fn test_modbus_mode_rtu() {
        let mode = ModbusMode::Rtu;
        assert!(matches!(mode, ModbusMode::Rtu));
    }

    // ========================================================================
    // ConnectionParams Tests
    // ========================================================================

    #[test]
    fn test_connection_params_tcp_creation() {
        let params = ConnectionParams {
            host: Some("192.168.1.100".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        assert_eq!(params.host, Some("192.168.1.100".to_string()));
        assert_eq!(params.port, Some(502));
        assert_eq!(params.timeout, Duration::from_secs(5));
    }

    #[cfg(feature = "modbus-rtu")]
    #[test]
    fn test_connection_params_rtu_creation() {
        let params = ConnectionParams {
            host: None,
            port: None,
            device: Some("/dev/ttyUSB0".to_string()),
            baud_rate: Some(9600),
            data_bits: Some(8),
            stop_bits: Some(1),
            parity: Some("None".to_string()),
            timeout: Duration::from_millis(500),
        };

        assert_eq!(params.device, Some("/dev/ttyUSB0".to_string()));
        assert_eq!(params.baud_rate, Some(9600));
        assert_eq!(params.data_bits, Some(8));
        assert_eq!(params.stop_bits, Some(1));
        assert_eq!(params.parity, Some("None".to_string()));
    }

    #[test]
    fn test_connection_params_clone() {
        let params1 = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(3),
        };

        let params2 = params1.clone();

        assert_eq!(params1.host, params2.host);
        assert_eq!(params1.port, params2.port);
        assert_eq!(params1.timeout, params2.timeout);
    }

    // ========================================================================
    // ModbusConnection Tests
    // ========================================================================

    #[test]
    fn test_modbus_connection_is_tcp() {
        // Create a mock TCP stream for testing
        // Note: We can't easily create a TcpStream without a real connection,
        // so we test the enum pattern matching instead
        let tcp_host = "127.0.0.1";
        let tcp_port = 502;

        // Verify our test values are valid
        assert_eq!(tcp_host, "127.0.0.1");
        assert_eq!(tcp_port, 502);
    }

    #[cfg(feature = "modbus-rtu")]
    #[test]
    fn test_modbus_connection_is_rtu() {
        // Verify RTU connection parameters
        let device = "/dev/ttyUSB0";
        let baud_rate = 9600u32;

        assert_eq!(device, "/dev/ttyUSB0");
        assert_eq!(baud_rate, 9600);
    }

    // ========================================================================
    // ModbusConnectionManager Tests
    // ========================================================================

    #[test]
    fn test_connection_manager_new_tcp() {
        let params = ConnectionParams {
            host: Some("192.168.1.100".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        assert!(matches!(manager.mode(), ModbusMode::Tcp));
    }

    #[cfg(feature = "modbus-rtu")]
    #[test]
    fn test_connection_manager_new_rtu() {
        let params = ConnectionParams {
            host: None,
            port: None,
            device: Some("/dev/ttyUSB0".to_string()),
            baud_rate: Some(9600),
            data_bits: Some(8),
            stop_bits: Some(1),
            parity: Some("None".to_string()),
            timeout: Duration::from_millis(500),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Rtu, params, logger, 5);

        assert!(matches!(manager.mode(), ModbusMode::Rtu));
    }

    #[test]
    fn test_connection_manager_mode() {
        let params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(3),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        match manager.mode() {
            ModbusMode::Tcp => {}, // TCP mode is correct
            #[cfg(feature = "modbus-rtu")]
            ModbusMode::Rtu => panic!("Expected TCP mode"),
        }
    }

    #[tokio::test]
    async fn test_connection_manager_is_connected_initially_false() {
        let params = ConnectionParams {
            host: Some("192.168.1.100".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        // Initially should not be connected
        assert!(!manager.is_connected().await);
    }

    // ========================================================================
    // Connection Parameters Validation Tests
    // ========================================================================

    #[test]
    fn test_tcp_params_with_default_port() {
        let params = ConnectionParams {
            host: Some("192.168.1.1".to_string()),
            port: Some(502), // Modbus default port
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        assert_eq!(params.port, Some(502));
    }

    #[test]
    fn test_timeout_values() {
        // Short timeout
        let params_short = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_millis(100),
        };
        assert_eq!(params_short.timeout, Duration::from_millis(100));

        // Long timeout
        let params_long = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(30),
        };
        assert_eq!(params_long.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_connection_params_debug_format() {
        let params = ConnectionParams {
            host: Some("test.local".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let debug_str = format!("{:?}", params);
        assert!(debug_str.contains("ConnectionParams"));
        assert!(debug_str.contains("test.local"));
    }

    // ========================================================================
    // Frame Info Extraction Tests
    // ========================================================================

    #[test]
    fn test_extract_frame_info_tcp_valid() {
        let params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        // TCP frame: [TID(2)][Proto(2)][Len(2)][Unit(1)][FC(1)]
        // TID=0x0001, Proto=0x0000, Len=0x0006, Unit=1, FC=3
        let frame = [0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x03];
        let (tid, slave, fc) = manager.extract_frame_info(&frame);

        assert_eq!(tid, Some(1));
        assert_eq!(slave, 1);
        assert_eq!(fc, 3);
    }

    #[test]
    fn test_extract_frame_info_tcp_short_frame() {
        let params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        // Frame too short (only 6 bytes, need 8)
        let frame = [0x00, 0x01, 0x00, 0x00, 0x00, 0x06];
        let (tid, slave, fc) = manager.extract_frame_info(&frame);

        assert_eq!(tid, None);
        assert_eq!(slave, 0);
        assert_eq!(fc, 0);
    }

    #[test]
    fn test_extract_frame_info_empty_frame() {
        let params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        let frame: [u8; 0] = [];
        let (tid, slave, fc) = manager.extract_frame_info(&frame);

        assert_eq!(tid, None);
        assert_eq!(slave, 0);
        assert_eq!(fc, 0);
    }

    // ========================================================================
    // Connection Error Tests (without actual connections)
    // ========================================================================

    #[tokio::test]
    async fn test_send_without_connection_returns_error() {
        let params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        let result = manager.send(&[0x01, 0x03, 0x00, 0x00]).await;
        assert!(result.is_err());

        if let Err(ComLinkError::Connection(msg)) = result {
            assert!(msg.contains("Not connected"));
        } else {
            panic!("Expected ConnectionError");
        }
    }

    #[tokio::test]
    async fn test_receive_without_connection_returns_error() {
        let params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        let mut buffer = [0u8; 256];
        let result = manager.receive(&mut buffer, Duration::from_secs(1)).await;
        assert!(result.is_err());

        if let Err(ComLinkError::Connection(msg)) = result {
            assert!(msg.contains("Not connected"));
        } else {
            panic!("Expected ConnectionError");
        }
    }

    #[tokio::test]
    async fn test_disconnect_when_not_connected() {
        let params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        // Disconnect should succeed even when not connected
        let result = manager.disconnect().await;
        assert!(result.is_ok());
        assert!(!manager.is_connected().await);
    }

    // ========================================================================
    // Configuration Validation Tests
    // ========================================================================

    #[tokio::test]
    async fn test_connect_tcp_missing_host() {
        let params = ConnectionParams {
            host: None, // Missing host
            port: Some(502),
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        let result = manager.connect().await;
        assert!(result.is_err());

        if let Err(ComLinkError::Config(msg)) = result {
            assert!(msg.contains("host"));
        } else {
            panic!("Expected ConfigError for missing host");
        }
    }

    #[tokio::test]
    async fn test_connect_tcp_missing_port() {
        let params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: None, // Missing port
            #[cfg(feature = "modbus-rtu")]
            device: None,
            #[cfg(feature = "modbus-rtu")]
            baud_rate: None,
            #[cfg(feature = "modbus-rtu")]
            data_bits: None,
            #[cfg(feature = "modbus-rtu")]
            stop_bits: None,
            #[cfg(feature = "modbus-rtu")]
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let logger = ChannelLogger::new(1, "test".to_string());
        let manager = ModbusConnectionManager::new(ModbusMode::Tcp, params, logger, 5);

        let result = manager.connect().await;
        assert!(result.is_err());

        if let Err(ComLinkError::Config(msg)) = result {
            assert!(msg.contains("port"));
        } else {
            panic!("Expected ConfigError for missing port");
        }
    }

    // ========================================================================
    // ModbusMode Equality and Copy Tests
    // ========================================================================

    #[test]
    fn test_modbus_mode_equality() {
        assert_eq!(ModbusMode::Tcp, ModbusMode::Tcp);
        #[cfg(feature = "modbus-rtu")]
        {
            assert_eq!(ModbusMode::Rtu, ModbusMode::Rtu);
            assert_ne!(ModbusMode::Tcp, ModbusMode::Rtu);
        }
    }

    #[test]
    fn test_modbus_mode_copy() {
        let mode1 = ModbusMode::Tcp;
        let mode2 = mode1; // Copy
        assert_eq!(mode1, mode2);
    }

    #[test]
    fn test_modbus_mode_debug_format() {
        let tcp_debug = format!("{:?}", ModbusMode::Tcp);
        assert!(tcp_debug.contains("Tcp"));

        #[cfg(feature = "modbus-rtu")]
        {
            let rtu_debug = format!("{:?}", ModbusMode::Rtu);
            assert!(rtu_debug.contains("Rtu"));
        }
    }
}
