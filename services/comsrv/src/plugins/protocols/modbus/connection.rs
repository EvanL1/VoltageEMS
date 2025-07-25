//! Modbus Connection Management
//!
//! This module provides TCP and RTU connection management for Modbus protocol

use crate::utils::error::{ComSrvError, Result};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, error, info, warn};

/// Modbus connection type
#[derive(Debug)]
pub enum ModbusConnection {
    /// TCP connection
    Tcp(TcpStream),
    /// Serial RTU connection
    Rtu(SerialStream),
}

impl ModbusConnection {
    /// Create a TCP connection
    pub async fn connect_tcp(host: &str, port: u16, timeout_duration: Duration) -> Result<Self> {
        let addr = format!("{}:{}", host, port);
        info!("Connecting to Modbus TCP endpoint: {}", addr);

        match timeout(timeout_duration, TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => {
                // Configure socket for optimal performance
                if let Err(e) = stream.set_nodelay(true) {
                    warn!("Failed to set TCP_NODELAY: {}", e);
                }

                info!("Successfully connected to Modbus TCP endpoint: {}", addr);
                Ok(ModbusConnection::Tcp(stream))
            }
            Ok(Err(e)) => {
                error!("Failed to connect to {}: {}", addr, e);
                Err(ComSrvError::ConnectionError(format!(
                    "Failed to connect to {}: {}",
                    addr, e
                )))
            }
            Err(_) => {
                warn!("Connection to {} timed out", addr);
                Err(ComSrvError::TimeoutError(format!(
                    "Connection to {} timed out",
                    addr
                )))
            }
        }
    }

    /// Create a serial RTU connection
    pub async fn connect_rtu(
        port: &str,
        baud_rate: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: &str,
        timeout_duration: Duration,
    ) -> Result<Self> {
        info!("Opening serial port: {} at {} baud", port, baud_rate);

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
                info!("Successfully opened serial port: {}", port);
                Ok(ModbusConnection::Rtu(serial_port))
            }
            Err(e) => {
                error!("Failed to open serial port {}: {}", port, e);
                Err(ComSrvError::ConnectionError(format!(
                    "Failed to open serial port {}: {}",
                    port, e
                )))
            }
        }
    }

    /// Send data
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        match self {
            ModbusConnection::Tcp(stream) => {
                stream.write_all(data).await.map_err(|e| {
                    error!("TCP send error: {}", e);
                    ComSrvError::IoError(format!("TCP send error: {}", e))
                })?;
                debug!("Sent {} bytes via TCP", data.len());
            }
            ModbusConnection::Rtu(port) => {
                port.write_all(data).await.map_err(|e| {
                    error!("Serial send error: {}", e);
                    ComSrvError::IoError(format!("Serial send error: {}", e))
                })?;
                port.flush().await.map_err(|e| {
                    error!("Serial flush error: {}", e);
                    ComSrvError::IoError(format!("Serial flush error: {}", e))
                })?;
                debug!("Sent {} bytes via serial", data.len());
            }
        }
        Ok(())
    }

    /// Receive data
    pub async fn receive(
        &mut self,
        buffer: &mut [u8],
        timeout_duration: Duration,
    ) -> Result<usize> {
        match self {
            ModbusConnection::Tcp(stream) => {
                match timeout(timeout_duration, stream.read(buffer)).await {
                    Ok(Ok(bytes)) => {
                        if bytes == 0 {
                            return Err(ComSrvError::ConnectionError(
                                "Connection closed by peer".to_string(),
                            ));
                        }
                        debug!("Received {} bytes via TCP", bytes);
                        Ok(bytes)
                    }
                    Ok(Err(e)) => {
                        error!("TCP receive error: {}", e);
                        Err(ComSrvError::IoError(format!("TCP receive error: {}", e)))
                    }
                    Err(_) => {
                        debug!("TCP receive timeout");
                        Err(ComSrvError::TimeoutError("TCP receive timeout".to_string()))
                    }
                }
            }
            ModbusConnection::Rtu(port) => {
                match timeout(timeout_duration, port.read(buffer)).await {
                    Ok(Ok(bytes)) => {
                        debug!("Received {} bytes via serial", bytes);
                        Ok(bytes)
                    }
                    Ok(Err(e)) => {
                        error!("Serial receive error: {}", e);
                        Err(ComSrvError::IoError(format!("Serial receive error: {}", e)))
                    }
                    Err(_) => {
                        debug!("Serial receive timeout");
                        Err(ComSrvError::TimeoutError(
                            "Serial receive timeout".to_string(),
                        ))
                    }
                }
            }
        }
    }

    /// Check if connection is TCP
    pub fn is_tcp(&self) -> bool {
        matches!(self, ModbusConnection::Tcp(_))
    }

    /// Check if connection is RTU
    pub fn is_rtu(&self) -> bool {
        matches!(self, ModbusConnection::Rtu(_))
    }
}

/// Modbus connection manager
pub struct ModbusConnectionManager {
    /// Connection instance
    connection: Mutex<Option<ModbusConnection>>,
    /// Connection mode (TCP or RTU)
    mode: ModbusMode,
    /// Connection parameters
    params: ConnectionParams,
}

/// Connection mode
#[derive(Debug, Clone)]
pub enum ModbusMode {
    Tcp,
    Rtu,
}

/// Connection parameters
#[derive(Debug, Clone)]
pub struct ConnectionParams {
    // TCP parameters
    pub host: Option<String>,
    pub port: Option<u16>,

    // Serial parameters
    pub device: Option<String>,
    pub baud_rate: Option<u32>,
    pub data_bits: Option<u8>,
    pub stop_bits: Option<u8>,
    pub parity: Option<String>,

    // Common parameters
    pub timeout: Duration,
}

impl ModbusConnectionManager {
    /// Create new connection manager
    pub fn new(mode: ModbusMode, params: ConnectionParams) -> Self {
        Self {
            connection: Mutex::new(None),
            mode,
            params,
        }
    }

    /// Connect to device
    pub async fn connect(&self) -> Result<()> {
        let mut conn = self.connection.lock().await;

        // Disconnect if already connected
        if conn.is_some() {
            *conn = None;
        }

        // Create new connection
        let new_conn = match &self.mode {
            ModbusMode::Tcp => {
                let host = self.params.host.as_ref().ok_or_else(|| {
                    ComSrvError::ConfigError("TCP host not specified".to_string())
                })?;
                let port = self.params.port.ok_or_else(|| {
                    ComSrvError::ConfigError("TCP port not specified".to_string())
                })?;

                ModbusConnection::connect_tcp(host, port, self.params.timeout).await?
            }
            ModbusMode::Rtu => {
                let device = self.params.device.as_ref().ok_or_else(|| {
                    ComSrvError::ConfigError("Serial device not specified".to_string())
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
            }
        };

        *conn = Some(new_conn);
        Ok(())
    }

    /// Disconnect from device
    pub async fn disconnect(&self) -> Result<()> {
        let mut conn = self.connection.lock().await;
        *conn = None;
        info!("Disconnected from Modbus device");
        Ok(())
    }

    /// Send data
    pub async fn send(&self, data: &[u8]) -> Result<()> {
        let mut conn = self.connection.lock().await;
        match conn.as_mut() {
            Some(c) => c.send(data).await,
            None => Err(ComSrvError::ConnectionError("Not connected".to_string())),
        }
    }

    /// Receive data
    pub async fn receive(&self, buffer: &mut [u8], timeout: Duration) -> Result<usize> {
        let mut conn = self.connection.lock().await;
        match conn.as_mut() {
            Some(c) => c.receive(buffer, timeout).await,
            None => Err(ComSrvError::ConnectionError("Not connected".to_string())),
        }
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        self.connection.lock().await.is_some()
    }

    /// Get connection mode
    pub fn mode(&self) -> &ModbusMode {
        &self.mode
    }
}
