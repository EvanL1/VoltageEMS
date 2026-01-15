//! Modbus protocol adapter.
//!
//! This module provides the `ModbusChannel` adapter that integrates
//! `voltage_modbus` with the protocol layer's `Protocol` and `ProtocolClient` traits.
//!
//! # Example
//!
//! ```rust,ignore
//! use crate::protocols::prelude::*;
//! use crate::protocols::adapters::modbus::{ModbusChannel, ModbusChannelConfig};
//!
//! // Create channel without DataStore dependency
//! let config = ModbusChannelConfig::tcp("192.168.1.100:502")
//!     .with_connect_timeout(Duration::from_secs(5));
//!
//! let mut channel = ModbusChannel::new(config);
//! channel.connect().await?;
//!
//! // Service layer polls and handles storage
//! let batch = channel.poll_once().await?;
//! store.write_batch(channel_id, &batch).await?;
//! ```

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tracing::debug;
use voltage_modbus::{DeviceLimits, ModbusClient, ModbusTcpClient};

#[cfg(feature = "modbus")]
use voltage_modbus::ModbusRtuClient;

use crate::protocols::core::data::{DataBatch, DataPoint, Value};
use crate::protocols::core::diagnostics::AtomicDiagnostics;
use crate::protocols::core::error::{GatewayError, Result};
use crate::protocols::core::logging::{
    ChannelLogConfig, ChannelLogHandler, ErrorContext, LogContext, LoggableProtocol,
};
use crate::protocols::core::metadata::{
    DriverMetadata, HasMetadata, ParameterMetadata, ParameterType,
};
use serde::Deserialize;

use crate::protocols::adapters::command_batcher::{BatchCommand, CommandBatcher};
use crate::protocols::core::point::{
    ByteOrder, DataFormat, ModbusAddress, PointConfig, ProtocolAddress,
};
use crate::protocols::core::traits::{
    AdjustmentCommand, CommunicationMode, ConnectionState, ControlCommand, DataEventReceiver,
    Diagnostics, PointFailure, PollResult, Protocol, ProtocolCapabilities, ProtocolClient,
    WriteResult,
};
use crate::protocols::gateway::ChannelRuntime;
use async_trait::async_trait;

// Type alias for grouped points: (slave_id, function_code) -> Arc<Vec<PointConfig>>
// Arc wrapping enables O(1) clone when releasing lock for async I/O
type GroupedPoints = HashMap<(u8, u8), Arc<Vec<PointConfig>>>;

// ============================================================================
// Strongly-typed mapping configs for JSON deserialization
// ============================================================================

/// Modbus point mapping configuration (deserialized from protocol_mappings JSON).
///
/// # Required Fields
/// - `register_address`: The Modbus register address (0-based). This field is **required**
///   and deserialization will fail if missing.
///
/// # Optional Fields
/// - `slave_id`: Unit/slave ID (default: 1)
/// - `function_code`: Modbus function code (default: 3 = holding registers)
/// - `data_type`: Data format (default: uint16)
/// - `byte_order`: Byte order for multi-byte values (default: ABCD)
/// - `bit_position`: Bit position for boolean extraction from register (0-15)
///
/// # Example JSON
/// ```json
/// {
///     "slave_id": 1,
///     "register_address": 100,
///     "data_type": "float32",
///     "byte_order": "ABCD"
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ModbusMappingConfig {
    /// Slave/unit ID (default: 1).
    #[serde(default = "default_slave_id")]
    pub slave_id: u8,

    /// Modbus function code (default: 3).
    /// - 1: Read Coils
    /// - 2: Read Discrete Inputs
    /// - 3: Read Holding Registers
    /// - 4: Read Input Registers
    #[serde(default = "default_function_code")]
    pub function_code: u8,

    /// Register address (0-based). **Required field**.
    pub register_address: u16,

    /// Data format for register interpretation.
    #[serde(default)]
    pub data_type: DataFormat,

    /// Byte order for multi-byte values.
    #[serde(default)]
    pub byte_order: ByteOrder,

    /// Bit position for boolean values (0-15).
    #[serde(default)]
    pub bit_position: Option<u8>,
}

fn default_slave_id() -> u8 {
    1
}

fn default_function_code() -> u8 {
    3
}

impl ModbusMappingConfig {
    /// Convert to ModbusAddress.
    pub fn to_modbus_address(&self) -> ModbusAddress {
        ModbusAddress {
            slave_id: self.slave_id,
            function_code: self.function_code,
            register: self.register_address,
            format: self.data_type,
            byte_order: self.byte_order,
            bit_position: self.bit_position,
        }
    }
}

/// Modbus channel parameters configuration (deserialized from parameters_json).
///
/// # TCP Mode
/// ```json
/// {
///     "host": "192.168.1.100",
///     "port": 502
/// }
/// ```
///
/// # RTU Mode
/// ```json
/// {
///     "device": "/dev/ttyUSB0",
///     "baud_rate": 9600
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ModbusChannelParamsConfig {
    // TCP mode fields
    /// Target host (TCP mode).
    #[serde(default)]
    pub host: Option<String>,

    /// Target port (TCP mode, default: 502).
    #[serde(default = "default_modbus_port")]
    pub port: u16,

    // RTU mode fields
    /// Serial device path (RTU mode).
    #[serde(default)]
    pub device: Option<String>,

    /// Baud rate (RTU mode, default: 9600).
    #[serde(default = "default_baud_rate")]
    pub baud_rate: u32,

    // Common fields
    /// Connect timeout in milliseconds (default: 5000).
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,

    /// I/O timeout in milliseconds (default: 3000).
    #[serde(default = "default_io_timeout_ms")]
    pub io_timeout_ms: u64,

    /// Maximum registers per batch read (default: 125).
    #[serde(default = "default_max_batch_size_config")]
    pub max_batch_size: u16,

    /// Maximum gap between registers to allow merging (default: 10).
    #[serde(default = "default_max_gap_config")]
    pub max_gap: u16,
}

fn default_modbus_port() -> u16 {
    502
}

fn default_baud_rate() -> u32 {
    9600
}

fn default_connect_timeout_ms() -> u64 {
    5000
}

fn default_io_timeout_ms() -> u64 {
    3000
}

fn default_max_batch_size_config() -> u16 {
    125
}

fn default_max_gap_config() -> u16 {
    10
}

impl ModbusChannelParamsConfig {
    /// Check if this is a TCP configuration.
    pub fn is_tcp(&self) -> bool {
        self.host.is_some()
    }

    /// Check if this is an RTU configuration.
    pub fn is_rtu(&self) -> bool {
        self.device.is_some()
    }

    /// Get TCP address string (host:port).
    pub fn tcp_address(&self) -> Option<String> {
        self.host.as_ref().map(|h| format!("{}:{}", h, self.port))
    }

    /// Convert to ModbusChannelConfig.
    ///
    /// Note: Points must be set separately via `with_points()`.
    pub fn to_channel_config(&self) -> ModbusChannelConfig {
        if self.is_tcp() {
            ModbusChannelConfig::tcp(self.tcp_address().unwrap_or_default())
                .with_connect_timeout(std::time::Duration::from_millis(self.connect_timeout_ms))
                .with_io_timeout(std::time::Duration::from_millis(self.io_timeout_ms))
                .with_max_batch_size(self.max_batch_size)
                .with_max_gap(self.max_gap)
        } else if let Some(device) = &self.device {
            ModbusChannelConfig::rtu(device, self.baud_rate)
                .with_io_timeout(std::time::Duration::from_millis(self.io_timeout_ms))
                .with_max_batch_size(self.max_batch_size)
                .with_max_gap(self.max_gap)
        } else {
            // Default to TCP with empty address (will fail on connect)
            ModbusChannelConfig::tcp("")
        }
    }
}

/// Default maximum registers per batch read
const DEFAULT_MAX_BATCH_SIZE: u16 = 125;

/// Default maximum gap between registers to allow merging
const DEFAULT_MAX_GAP: u16 = 10;

/// Default reconnect cooldown in milliseconds (60 seconds)
const DEFAULT_RECONNECT_COOLDOWN_MS: u64 = 60_000;

/// Default maximum reconnect attempts (0 = unlimited)
const DEFAULT_MAX_RECONNECT_ATTEMPTS: u32 = 0;

/// Default consecutive zero-data cycles before triggering reconnect
const DEFAULT_ZERO_DATA_THRESHOLD: u32 = 5;

/// Reconnect configuration for automatic connection recovery.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Cooldown period after disconnect before reconnect attempts (in ms)
    pub cooldown_ms: u64,
    /// Maximum reconnect attempts (0 = unlimited)
    pub max_attempts: u32,
    /// Consecutive zero-data polling cycles before triggering reconnect
    pub zero_data_threshold: u32,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            cooldown_ms: DEFAULT_RECONNECT_COOLDOWN_MS,
            max_attempts: DEFAULT_MAX_RECONNECT_ATTEMPTS,
            zero_data_threshold: DEFAULT_ZERO_DATA_THRESHOLD,
        }
    }
}

impl ReconnectConfig {
    /// Create a new reconnect configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set cooldown period.
    pub fn with_cooldown_ms(mut self, ms: u64) -> Self {
        self.cooldown_ms = ms;
        self
    }

    /// Set maximum reconnect attempts.
    pub fn with_max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Set zero-data threshold.
    pub fn with_zero_data_threshold(mut self, threshold: u32) -> Self {
        self.zero_data_threshold = threshold;
        self
    }
}

/// Connection mode for Modbus channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectionMode {
    /// TCP/IP connection (default)
    #[default]
    Tcp,
    /// RTU serial port connection
    #[cfg(feature = "modbus")]
    Rtu,
}

/// Unified Modbus client wrapper for TCP and RTU transports.
///
/// This enum provides a transport-agnostic interface for Modbus operations,
/// allowing the same application code to work with both TCP and RTU connections.
pub enum ModbusClientWrapper {
    /// TCP client
    Tcp(ModbusTcpClient),
    /// RTU client (requires `modbus-rtu` feature)
    #[cfg(feature = "modbus")]
    Rtu(ModbusRtuClient),
}

impl ModbusClientWrapper {
    /// Read coils (FC01)
    pub async fn read_01(
        &mut self,
        slave_id: u8,
        address: u16,
        quantity: u16,
    ) -> voltage_modbus::ModbusResult<Vec<bool>> {
        match self {
            Self::Tcp(client) => client.read_01(slave_id, address, quantity).await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.read_01(slave_id, address, quantity).await,
        }
    }

    /// Read discrete inputs (FC02)
    pub async fn read_02(
        &mut self,
        slave_id: u8,
        address: u16,
        quantity: u16,
    ) -> voltage_modbus::ModbusResult<Vec<bool>> {
        match self {
            Self::Tcp(client) => client.read_02(slave_id, address, quantity).await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.read_02(slave_id, address, quantity).await,
        }
    }

    /// Read holding registers (FC03)
    pub async fn read_03(
        &mut self,
        slave_id: u8,
        address: u16,
        quantity: u16,
    ) -> voltage_modbus::ModbusResult<Vec<u16>> {
        match self {
            Self::Tcp(client) => client.read_03(slave_id, address, quantity).await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.read_03(slave_id, address, quantity).await,
        }
    }

    /// Read input registers (FC04)
    pub async fn read_04(
        &mut self,
        slave_id: u8,
        address: u16,
        quantity: u16,
    ) -> voltage_modbus::ModbusResult<Vec<u16>> {
        match self {
            Self::Tcp(client) => client.read_04(slave_id, address, quantity).await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.read_04(slave_id, address, quantity).await,
        }
    }

    /// Batch read holding registers (FC03) with automatic chunking.
    ///
    /// Automatically splits requests exceeding protocol limits (125 registers)
    /// into multiple smaller requests.
    pub async fn read_03_batch(
        &mut self,
        slave_id: u8,
        address: u16,
        quantity: u16,
        limits: &DeviceLimits,
    ) -> voltage_modbus::ModbusResult<Vec<u16>> {
        match self {
            Self::Tcp(client) => {
                client
                    .read_03_batch(slave_id, address, quantity, limits)
                    .await
            },
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => {
                client
                    .read_03_batch(slave_id, address, quantity, limits)
                    .await
            },
        }
    }

    /// Batch read input registers (FC04) with automatic chunking.
    ///
    /// Automatically splits requests exceeding protocol limits (125 registers)
    /// into multiple smaller requests.
    pub async fn read_04_batch(
        &mut self,
        slave_id: u8,
        address: u16,
        quantity: u16,
        limits: &DeviceLimits,
    ) -> voltage_modbus::ModbusResult<Vec<u16>> {
        match self {
            Self::Tcp(client) => {
                client
                    .read_04_batch(slave_id, address, quantity, limits)
                    .await
            },
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => {
                client
                    .read_04_batch(slave_id, address, quantity, limits)
                    .await
            },
        }
    }

    /// Write single coil (FC05)
    pub async fn write_05(
        &mut self,
        slave_id: u8,
        address: u16,
        value: bool,
    ) -> voltage_modbus::ModbusResult<()> {
        match self {
            Self::Tcp(client) => client.write_05(slave_id, address, value).await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.write_05(slave_id, address, value).await,
        }
    }

    /// Write single register (FC06)
    pub async fn write_06(
        &mut self,
        slave_id: u8,
        address: u16,
        value: u16,
    ) -> voltage_modbus::ModbusResult<()> {
        match self {
            Self::Tcp(client) => client.write_06(slave_id, address, value).await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.write_06(slave_id, address, value).await,
        }
    }

    /// Write multiple coils (FC0F)
    pub async fn write_0f(
        &mut self,
        slave_id: u8,
        address: u16,
        values: &[bool],
    ) -> voltage_modbus::ModbusResult<()> {
        match self {
            Self::Tcp(client) => client.write_0f(slave_id, address, values).await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.write_0f(slave_id, address, values).await,
        }
    }

    /// Write multiple registers (FC10)
    pub async fn write_10(
        &mut self,
        slave_id: u8,
        address: u16,
        values: &[u16],
    ) -> voltage_modbus::ModbusResult<()> {
        match self {
            Self::Tcp(client) => client.write_10(slave_id, address, values).await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.write_10(slave_id, address, values).await,
        }
    }

    /// Close the connection
    pub async fn close(&mut self) -> voltage_modbus::ModbusResult<()> {
        match self {
            Self::Tcp(client) => client.close().await,
            #[cfg(feature = "modbus")]
            Self::Rtu(client) => client.close().await,
        }
    }
}

/// Modbus channel configuration.
#[derive(Debug, Clone)]
pub struct ModbusChannelConfig {
    /// Connection mode (TCP or RTU)
    pub connection_mode: ConnectionMode,

    /// Target address for TCP (e.g., "192.168.1.100:502")
    pub address: String,

    /// Connection timeout (TCP only)
    pub connect_timeout: Duration,

    /// I/O operation timeout
    pub io_timeout: Duration,

    /// RTU serial device path (e.g., "/dev/ttyUSB0")
    #[cfg(feature = "modbus")]
    pub rtu_device: String,

    /// RTU baud rate (e.g., 9600, 19200, 115200)
    #[cfg(feature = "modbus")]
    pub baud_rate: u32,

    /// Point configurations
    pub points: Vec<PointConfig>,

    /// Maximum registers per batch read (default: 125)
    pub max_batch_size: u16,

    /// Maximum gap between registers to allow merging (default: 10)
    pub max_gap: u16,

    /// Reconnect configuration
    pub reconnect: ReconnectConfig,
}

impl ModbusChannelConfig {
    /// Create a TCP configuration.
    pub fn tcp(address: impl Into<String>) -> Self {
        Self {
            connection_mode: ConnectionMode::Tcp,
            address: address.into(),
            connect_timeout: Duration::from_secs(5),
            io_timeout: Duration::from_secs(3),
            #[cfg(feature = "modbus")]
            rtu_device: String::new(),
            #[cfg(feature = "modbus")]
            baud_rate: 9600,
            points: Vec::new(),
            max_batch_size: DEFAULT_MAX_BATCH_SIZE,
            max_gap: DEFAULT_MAX_GAP,
            reconnect: ReconnectConfig::default(),
        }
    }

    /// Create an RTU (serial) configuration.
    ///
    /// # Arguments
    ///
    /// * `device` - Serial device path (e.g., "/dev/ttyUSB0", "COM1")
    /// * `baud_rate` - Baud rate (e.g., 9600, 19200, 115200)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = ModbusChannelConfig::rtu("/dev/ttyUSB0", 9600);
    /// ```
    #[cfg(feature = "modbus")]
    pub fn rtu(device: impl Into<String>, baud_rate: u32) -> Self {
        Self {
            connection_mode: ConnectionMode::Rtu,
            address: String::new(), // Not used for RTU
            connect_timeout: Duration::from_secs(5),
            io_timeout: Duration::from_secs(3),
            rtu_device: device.into(),
            baud_rate,
            points: Vec::new(),
            max_batch_size: DEFAULT_MAX_BATCH_SIZE,
            max_gap: DEFAULT_MAX_GAP,
            reconnect: ReconnectConfig::default(),
        }
    }

    /// Set connection timeout.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set I/O timeout.
    pub fn with_io_timeout(mut self, timeout: Duration) -> Self {
        self.io_timeout = timeout;
        self
    }

    /// Add point configurations.
    pub fn with_points(mut self, points: Vec<PointConfig>) -> Self {
        self.points = points;
        self
    }

    /// Set maximum batch size for register reads.
    pub fn with_max_batch_size(mut self, size: u16) -> Self {
        self.max_batch_size = size;
        self
    }

    /// Set maximum gap for merging consecutive registers.
    pub fn with_max_gap(mut self, gap: u16) -> Self {
        self.max_gap = gap;
        self
    }

    /// Set reconnect configuration.
    pub fn with_reconnect(mut self, config: ReconnectConfig) -> Self {
        self.reconnect = config;
        self
    }
}

/// Modbus channel adapter.
///
/// This struct wraps a `voltage_modbus::ModbusTcpClient` and implements
/// the protocol layer's `Protocol` and `ProtocolClient` traits.
///
/// # Architecture (Protocol Layer Only)
///
/// `ModbusChannel` is a pure protocol implementation that handles device communication.
/// It does NOT handle data storage - that responsibility belongs to the service layer.
///
/// Use `poll_once()` to perform a single poll cycle and get the data as a `DataBatch`.
/// The caller (service layer) is responsible for storing the data.
///
/// # Example
///
/// ```rust,ignore
/// // Service layer calls poll_once() and handles storage
/// loop {
///     let batch = channel.poll_once().await?;
///     store.write_batch(channel_id, &batch).await?;
/// }
/// ```
pub struct ModbusChannel {
    config: ModbusChannelConfig,
    /// Channel identifier for logging.
    channel_id: u32,
    /// Channel display name.
    name: String,
    /// Modbus client (owned, no lock needed - channel loop has exclusive access).
    /// Uses ModbusClientWrapper to support both TCP and RTU transports.
    client: Option<ModbusClientWrapper>,
    state: Arc<std::sync::RwLock<ConnectionState>>,
    diagnostics: Arc<AtomicDiagnostics>,

    // === Polling support ===
    /// Pre-grouped points by (slave_id, function_code) - initialized once, read many
    grouped_points: OnceLock<GroupedPoints>,
    /// Polling interval in milliseconds
    polling_interval_ms: u64,

    // === Command batching ===
    /// Command batcher for optimizing write operations (owned, no lock needed).
    command_batcher: CommandBatcher,

    // === Logging ===
    /// Logging context for channel events
    log_context: Arc<LogContext>,
}

// ChannelDiagnostics replaced with AtomicDiagnostics from protocols::core

/// Default polling interval in milliseconds
const DEFAULT_POLLING_INTERVAL_MS: u64 = 1000;

impl ModbusChannel {
    /// Create a new Modbus channel.
    ///
    /// # Arguments
    /// - `config`: Channel configuration including address and points
    /// - `channel_id`: Unique identifier for this channel (used in logs)
    /// - `name`: Channel display name
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = ModbusChannelConfig::tcp("192.168.1.100:502")
    ///     .with_points(points);
    /// let mut channel = ModbusChannel::new(config, 1, "modbus_tcp_1".to_string());
    /// channel.connect().await?;
    ///
    /// // Poll and get data (service layer stores it)
    /// let batch = channel.poll_once().await?;
    /// ```
    pub fn new(config: ModbusChannelConfig, channel_id: u32, name: String) -> Self {
        Self {
            config,
            channel_id,
            name,
            client: None,
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Disconnected)),
            diagnostics: Arc::new(AtomicDiagnostics::new()),
            grouped_points: OnceLock::new(),
            polling_interval_ms: DEFAULT_POLLING_INTERVAL_MS,
            command_batcher: CommandBatcher::new(),
            log_context: Arc::new(LogContext::new(channel_id)),
        }
    }

    /// Set polling interval.
    pub fn with_polling_interval(mut self, interval_ms: u64) -> Self {
        self.polling_interval_ms = interval_ms;
        self
    }

    /// Set connection state (internal helper).
    fn set_state(&self, state: ConnectionState) {
        if let Ok(mut s) = self.state.write() {
            *s = state;
        }
    }

    /// Get connection state.
    fn get_state(&self) -> ConnectionState {
        self.state
            .read()
            .map(|s| *s)
            .unwrap_or(ConnectionState::Error)
    }

    /// Get the point configurations.
    pub fn points(&self) -> &[PointConfig] {
        &self.config.points
    }

    /// Read a single Modbus address and convert to DataPoint.
    #[allow(dead_code)]
    async fn read_modbus_point(&mut self, point: &PointConfig) -> Result<DataPoint> {
        let client = self.client.as_mut().ok_or(GatewayError::NotConnected)?;

        let modbus_addr = match &point.address {
            ProtocolAddress::Modbus(addr) => addr,
            _ => return Err(GatewayError::Config("Invalid address type".into())),
        };

        // Read registers based on function code
        let value = match modbus_addr.function_code {
            1 => {
                // Read coils (FC01)
                let coils = client
                    .read_01(modbus_addr.slave_id, modbus_addr.register, 1)
                    .await
                    .map_err(|e| {
                        GatewayError::Protocol(format!(
                            "FC01 slave {} reg {}: {}",
                            modbus_addr.slave_id, modbus_addr.register, e
                        ))
                    })?;
                Value::Bool(coils.first().copied().unwrap_or(false))
            },
            2 => {
                // Read discrete inputs (FC02)
                let inputs = client
                    .read_02(modbus_addr.slave_id, modbus_addr.register, 1)
                    .await
                    .map_err(|e| {
                        GatewayError::Protocol(format!(
                            "FC02 slave {} reg {}: {}",
                            modbus_addr.slave_id, modbus_addr.register, e
                        ))
                    })?;
                Value::Bool(inputs.first().copied().unwrap_or(false))
            },
            3 => {
                // Read holding registers (FC03)
                let count = modbus_addr.format.register_count();
                let regs = client
                    .read_03(modbus_addr.slave_id, modbus_addr.register, count)
                    .await
                    .map_err(|e| {
                        GatewayError::Protocol(format!(
                            "FC03 slave {} reg {}: {}",
                            modbus_addr.slave_id, modbus_addr.register, e
                        ))
                    })?;
                decode_registers(
                    &regs,
                    modbus_addr.format,
                    modbus_addr.byte_order,
                    modbus_addr.bit_position,
                )?
            },
            4 => {
                // Read input registers (FC04)
                let count = modbus_addr.format.register_count();
                let regs = client
                    .read_04(modbus_addr.slave_id, modbus_addr.register, count)
                    .await
                    .map_err(|e| {
                        GatewayError::Protocol(format!(
                            "FC04 slave {} reg {}: {}",
                            modbus_addr.slave_id, modbus_addr.register, e
                        ))
                    })?;
                decode_registers(
                    &regs,
                    modbus_addr.format,
                    modbus_addr.byte_order,
                    modbus_addr.bit_position,
                )?
            },
            _ => {
                return Err(GatewayError::Unsupported(format!(
                    "Function code {} not supported for read",
                    modbus_addr.function_code
                )))
            },
        };

        // Apply transform
        let transformed_value = apply_transform(value, &point.transform);

        Ok(DataPoint::new(
            point.id,
            point.point_type,
            transformed_value,
        ))
    }

    /// Record an error in diagnostics.
    fn record_error(&self, error: String) {
        self.diagnostics.record_error(error);
    }

    /// Pre-group points by (slave_id, function_code) for polling optimization.
    ///
    /// All configured points are included. The application layer determines
    /// which points should be polled based on their SCADA type.
    ///
    /// Points within each group are pre-sorted by register address to avoid
    /// repeated sorting during each poll cycle.
    ///
    /// Uses OnceLock for thread-safe lazy initialization - only computed once.
    fn get_or_init_grouped_points(&self) -> &GroupedPoints {
        self.grouped_points.get_or_init(|| {
            // First build temporary groups without Arc
            let mut temp_groups: HashMap<(u8, u8), Vec<PointConfig>> = HashMap::new();

            for point in &self.config.points {
                // Extract Modbus address
                if let ProtocolAddress::Modbus(addr) = &point.address {
                    let key = (addr.slave_id, addr.function_code);
                    temp_groups.entry(key).or_default().push(point.clone());
                }
            }

            // Pre-sort each group by register address (avoids sorting on every poll)
            for points in temp_groups.values_mut() {
                points.sort_by_key(|p| {
                    if let ProtocolAddress::Modbus(addr) = &p.address {
                        addr.register
                    } else {
                        u16::MAX // Non-Modbus addresses go to end (shouldn't happen)
                    }
                });
            }

            // Wrap each group in Arc for O(1) clone during polling
            let groups: GroupedPoints = temp_groups
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect();

            debug!(
                "[{}] grouped {} points into {} groups",
                self.config.address,
                self.config.points.len(),
                groups.len()
            );

            groups
        })
    }

    /// Read a group of points with the same slave_id and function_code.
    ///
    /// Uses batch reading optimization: consecutive registers are read in single requests.
    /// Returns a list of (point_id, DataPoint) tuples for successfully read points.
    async fn read_point_group(
        client: &mut ModbusClientWrapper,
        points: &[PointConfig],
        max_batch_size: u16,
        max_gap: u16,
    ) -> Vec<(u32, DataPoint)> {
        if points.is_empty() {
            return Vec::new();
        }

        // Get slave_id and function_code from first point (all points in group share these)
        let (slave_id, function_code) = match &points[0].address {
            ProtocolAddress::Modbus(addr) => (addr.slave_id, addr.function_code),
            _ => return Vec::new(),
        };

        // For coils/discrete inputs (FC01/FC02), read individually (simpler logic)
        if function_code == 1 || function_code == 2 {
            return Self::read_coils_individually(client, points, slave_id, function_code).await;
        }

        // For registers (FC03/FC04), use batch optimization
        Self::read_registers_batched(
            client,
            points,
            slave_id,
            function_code,
            max_batch_size,
            max_gap,
        )
        .await
    }

    /// Read coils or discrete inputs individually (FC01/FC02).
    async fn read_coils_individually(
        client: &mut ModbusClientWrapper,
        points: &[PointConfig],
        slave_id: u8,
        function_code: u8,
    ) -> Vec<(u32, DataPoint)> {
        let mut results = Vec::with_capacity(points.len());

        for point in points {
            let modbus_addr = match &point.address {
                ProtocolAddress::Modbus(addr) => addr,
                _ => continue,
            };

            let value_result = match function_code {
                1 => client
                    .read_01(slave_id, modbus_addr.register, 1)
                    .await
                    .map(|coils| Value::Bool(coils.first().copied().unwrap_or(false))),
                2 => client
                    .read_02(slave_id, modbus_addr.register, 1)
                    .await
                    .map(|inputs| Value::Bool(inputs.first().copied().unwrap_or(false))),
                _ => continue,
            };

            if let Ok(value) = value_result {
                let transformed = apply_transform(value, &point.transform);
                results.push((
                    point.id,
                    DataPoint::new(point.id, point.point_type, transformed),
                ));
            }
        }

        results
    }

    /// Read registers in batches (FC03/FC04).
    ///
    /// Groups consecutive registers (within max_gap) and reads them in single requests.
    async fn read_registers_batched(
        client: &mut ModbusClientWrapper,
        points: &[PointConfig],
        slave_id: u8,
        function_code: u8,
        max_batch_size: u16,
        max_gap: u16,
    ) -> Vec<(u32, DataPoint)> {
        // Points are pre-sorted by register address in group_points_for_polling()
        // Just extract register info without re-sorting
        let sorted_points: Vec<_> = points
            .iter()
            .filter_map(|p| {
                if let ProtocolAddress::Modbus(addr) = &p.address {
                    Some((addr.register, addr.format.register_count(), p))
                } else {
                    None
                }
            })
            .collect();

        if sorted_points.is_empty() {
            return Vec::new();
        }

        // Build segments of consecutive registers
        let segments = Self::build_register_segments(&sorted_points, max_gap, max_batch_size);

        // Read each segment
        let mut results = Vec::with_capacity(points.len());

        for segment in segments {
            let batch_result = Self::read_register_segment(
                client,
                slave_id,
                function_code,
                &segment,
                max_batch_size,
            )
            .await;

            match batch_result {
                Ok(batch_results) => results.extend(batch_results),
                Err(e) => {
                    debug!(
                        "FC{:02} slave {} batch read @{}-{} failed: {}",
                        function_code,
                        slave_id,
                        segment.start_address,
                        segment.end_address.saturating_sub(1),
                        e
                    );
                },
            }
        }

        results
    }

    /// Build segments of consecutive registers for batch reading.
    #[allow(clippy::disallowed_methods)] // unwrap on Option that is guaranteed Some
    fn build_register_segments<'a>(
        sorted_points: &[(u16, u16, &'a PointConfig)],
        max_gap: u16,
        max_batch_size: u16,
    ) -> Vec<RegisterSegment<'a>> {
        let mut segments = Vec::new();
        let mut current_segment: Option<RegisterSegment> = None;

        for &(addr, count, point) in sorted_points {
            match &mut current_segment {
                None => {
                    // Start new segment
                    current_segment = Some(RegisterSegment {
                        start_address: addr,
                        end_address: addr + count,
                        points: vec![(addr, count, point)],
                    });
                },
                Some(seg) => {
                    let gap = addr.saturating_sub(seg.end_address);
                    let new_total = (addr + count).saturating_sub(seg.start_address);

                    // Check if we can extend current segment
                    if gap <= max_gap && new_total <= max_batch_size {
                        seg.end_address = addr + count;
                        seg.points.push((addr, count, point));
                    } else {
                        // Segment complete, start new one
                        segments.push(current_segment.take().unwrap());
                        current_segment = Some(RegisterSegment {
                            start_address: addr,
                            end_address: addr + count,
                            points: vec![(addr, count, point)],
                        });
                    }
                },
            }
        }

        // Add last segment
        if let Some(seg) = current_segment {
            segments.push(seg);
        }

        segments
    }

    /// Read a segment of consecutive registers and decode individual points.
    ///
    /// Uses batch read methods for automatic chunking protection - if the segment
    /// somehow exceeds protocol limits (125 registers), requests are automatically split.
    #[allow(clippy::needless_lifetimes)]
    async fn read_register_segment<'a>(
        client: &mut ModbusClientWrapper,
        slave_id: u8,
        function_code: u8,
        segment: &RegisterSegment<'a>,
        max_batch_size: u16,
    ) -> std::result::Result<Vec<(u32, DataPoint)>, voltage_modbus::ModbusError> {
        let total_registers = segment.end_address - segment.start_address;

        // Create device limits for batch reading protection
        let limits = DeviceLimits::new().with_max_read_registers(max_batch_size);

        // Read all registers in segment using batch methods for extra safety
        let registers = match function_code {
            3 => {
                client
                    .read_03_batch(slave_id, segment.start_address, total_registers, &limits)
                    .await?
            },
            4 => {
                client
                    .read_04_batch(slave_id, segment.start_address, total_registers, &limits)
                    .await?
            },
            _ => return Ok(Vec::new()),
        };

        // Decode each point from the register buffer
        let mut results = Vec::with_capacity(segment.points.len());

        for &(addr, count, point) in &segment.points {
            let offset = (addr - segment.start_address) as usize;
            let end = offset + count as usize;

            if end <= registers.len() {
                let point_regs = &registers[offset..end];

                if let ProtocolAddress::Modbus(modbus_addr) = &point.address {
                    if let Ok(value) = decode_registers(
                        point_regs,
                        modbus_addr.format,
                        modbus_addr.byte_order,
                        modbus_addr.bit_position,
                    ) {
                        let transformed = apply_transform(value, &point.transform);
                        results.push((
                            point.id,
                            DataPoint::new(point.id, point.point_type, transformed),
                        ));
                    }
                }
            }
        }

        Ok(results)
    }
}

impl HasMetadata for ModbusChannel {
    #[allow(clippy::disallowed_methods)] // json! macro
    fn metadata() -> DriverMetadata {
        DriverMetadata {
            name: "modbus_tcp",
            display_name: "Modbus TCP",
            description: "Industrial Modbus TCP protocol for reading/writing registers and coils.",
            is_recommended: true,
            example_config: serde_json::json!({
                "host": "192.168.1.100",
                "port": 502,
                "slave_id": 1,
                "connect_timeout_ms": 5000,
                "read_timeout_ms": 3000,
                "polling_interval_ms": 1000,
                "max_batch_size": 125,
                "max_reconnect_attempts": 5
            }),
            parameters: vec![
                ParameterMetadata::required(
                    "host",
                    "Host",
                    "Modbus device IP address or hostname",
                    ParameterType::String,
                ),
                ParameterMetadata::optional(
                    "port",
                    "Port",
                    "Modbus TCP port",
                    ParameterType::Integer,
                    serde_json::json!(502),
                ),
                ParameterMetadata::optional(
                    "slave_id",
                    "Slave ID",
                    "Modbus slave/unit ID (1-247)",
                    ParameterType::Integer,
                    serde_json::json!(1),
                ),
                ParameterMetadata::optional(
                    "connect_timeout_ms",
                    "Connect Timeout (ms)",
                    "Connection timeout in milliseconds",
                    ParameterType::Integer,
                    serde_json::json!(5000),
                ),
                ParameterMetadata::optional(
                    "read_timeout_ms",
                    "Read Timeout (ms)",
                    "Read operation timeout in milliseconds",
                    ParameterType::Integer,
                    serde_json::json!(3000),
                ),
                ParameterMetadata::optional(
                    "polling_interval_ms",
                    "Polling Interval (ms)",
                    "Polling interval in milliseconds",
                    ParameterType::Integer,
                    serde_json::json!(1000),
                ),
                ParameterMetadata::optional(
                    "max_batch_size",
                    "Max Batch Size",
                    "Maximum registers per batch read (max 125)",
                    ParameterType::Integer,
                    serde_json::json!(125),
                ),
                ParameterMetadata::optional(
                    "max_reconnect_attempts",
                    "Max Reconnect Attempts",
                    "Maximum reconnection attempts before giving up",
                    ParameterType::Integer,
                    serde_json::json!(5),
                ),
            ],
        }
    }
}

/// A segment of consecutive registers to be read in one batch.
struct RegisterSegment<'a> {
    start_address: u16,
    end_address: u16,
    points: Vec<(u16, u16, &'a PointConfig)>, // (address, count, point)
}

// ============================================================
// Command Batching Support
// ============================================================

impl ModbusChannel {
    /// Queue an adjustment command for batched execution.
    ///
    /// Commands are collected and executed when:
    /// - Batch window expires (20ms), or
    /// - Maximum batch size reached (100 commands)
    ///
    /// Call `check_and_execute_batch()` periodically to trigger execution.
    pub fn queue_adjustment(&mut self, adj: &AdjustmentCommand) -> Result<()> {
        // Find point config
        let point = self
            .config
            .points
            .iter()
            .find(|p| p.id == adj.id)
            .ok_or_else(|| GatewayError::InvalidAddress(format!("Point {} not found", adj.id)))?;

        let modbus_addr = match &point.address {
            ProtocolAddress::Modbus(addr) => addr, // Borrow instead of clone
            _ => {
                return Err(GatewayError::InvalidAddress(
                    "Non-Modbus address type".into(),
                ))
            },
        };

        // Apply reverse transform to get raw value
        let raw_value = reverse_transform(adj.value, &point.transform)?;

        // Create batch command (fields are Copy types, no clone needed)
        let batch_cmd = BatchCommand {
            point_id: adj.id,
            value: Value::Float(raw_value),
            slave_id: modbus_addr.slave_id,
            function_code: if modbus_addr.format.register_count() > 1 {
                16
            } else {
                6
            },
            register_address: modbus_addr.register,
            data_format: modbus_addr.format,
            byte_order: modbus_addr.byte_order,
        };

        // Add to batcher
        self.command_batcher.add_command(batch_cmd);

        Ok(())
    }

    /// Check if pending commands should be executed, and execute if so.
    ///
    /// Returns `Some(WriteResult)` if commands were executed, `None` otherwise.
    /// Call this periodically (e.g., in a polling loop) to flush batched commands.
    pub async fn check_and_execute_batch(&mut self) -> Result<Option<WriteResult>> {
        let should_execute =
            self.command_batcher.should_execute() && self.command_batcher.pending_count() > 0;

        if should_execute {
            Ok(Some(self.execute_batched_commands().await?))
        } else {
            Ok(None)
        }
    }

    /// Execute all pending batched commands with FC16 optimization.
    ///
    /// This method takes all pending commands from the batcher and executes them.
    /// For commands with consecutive register addresses on the same slave,
    /// it uses FC16 (Write Multiple Registers) to batch them into a single request.
    pub async fn execute_batched_commands(&mut self) -> Result<WriteResult> {
        let batches = self.command_batcher.take_commands();

        if batches.is_empty() {
            return Ok(WriteResult {
                success_count: 0,
                failures: Vec::new(),
            });
        }

        // Pre-calculate total commands for capacity hint
        let total_commands: usize = batches.values().map(|v| v.len()).sum();
        let mut success_count = 0;
        let mut failures = Vec::with_capacity(total_commands);

        // Get client reference (no lock needed - we own it)
        let client = self.client.as_mut().ok_or(GatewayError::NotConnected)?;

        for ((slave_id, fc), commands) in batches {
            // FC16 optimization: merge consecutive addresses into single write
            if fc == 16 && commands.len() > 1 && CommandBatcher::are_strictly_consecutive(&commands)
            {
                // Merge all commands into single FC16 request
                match Self::execute_merged_fc16(client, slave_id, &commands, &mut failures).await {
                    Ok(count) => success_count += count,
                    Err(e) => {
                        // If merged write fails, record for all commands
                        let err_msg = format!(
                            "FC16 slave {} regs [{}-{}]: {}",
                            slave_id,
                            commands[0].register_address,
                            commands.last().map(|c| c.register_address).unwrap_or(0),
                            e
                        );
                        for cmd in &commands {
                            failures.push((cmd.point_id, err_msg.clone()));
                        }
                    },
                }
            } else {
                // Execute commands individually
                for cmd in commands {
                    let result = match cmd.data_format {
                        DataFormat::UInt16 | DataFormat::Int16 | DataFormat::Bool => {
                            let value = cmd.value.as_f64().unwrap_or(0.0) as u16;
                            client
                                .write_06(cmd.slave_id, cmd.register_address, value)
                                .await
                        },
                        DataFormat::UInt32 | DataFormat::Int32 | DataFormat::Float32 => {
                            let raw_value = cmd.value.as_f64().unwrap_or(0.0);
                            match encode_value(raw_value, cmd.data_format, cmd.byte_order) {
                                Ok(regs) => {
                                    client
                                        .write_10(cmd.slave_id, cmd.register_address, &regs)
                                        .await
                                },
                                Err(e) => {
                                    failures.push((
                                        cmd.point_id,
                                        format!(
                                            "Encode error for slave {} reg {}: {}",
                                            cmd.slave_id, cmd.register_address, e
                                        ),
                                    ));
                                    continue;
                                },
                            }
                        },
                        _ => {
                            failures.push((cmd.point_id, "Unsupported format".into()));
                            continue;
                        },
                    };

                    match result {
                        Ok(_) => success_count += 1,
                        Err(e) => failures.push((
                            cmd.point_id,
                            format!(
                                "Write slave {} reg {}: {}",
                                cmd.slave_id, cmd.register_address, e
                            ),
                        )),
                    }
                }
            }
        }

        // Update diagnostics (lock-free atomic operations)
        self.diagnostics.add_write(success_count as u64);
        if !failures.is_empty() {
            self.diagnostics.add_error(failures.len() as u64);
            // Note: We don't use record_error here to avoid double-counting
            // last_error is updated but error_count is already incremented above
        }

        Ok(WriteResult {
            success_count,
            failures,
        })
    }

    /// Execute merged FC16 (Write Multiple Registers) for consecutive addresses.
    ///
    /// This combines multiple single-register writes into one multi-register write,
    /// significantly reducing network round-trips when writing adjacent registers.
    ///
    /// Note: This is an associated function (no `&self`) to avoid borrow conflicts.
    async fn execute_merged_fc16(
        client: &mut ModbusClientWrapper,
        slave_id: u8,
        commands: &[BatchCommand],
        failures: &mut Vec<(u32, String)>,
    ) -> std::result::Result<usize, voltage_modbus::ModbusError> {
        // Record initial failures count to calculate only new failures from this function
        let initial_failures = failures.len();

        // Sort by index to avoid cloning all commands
        let mut indices: Vec<usize> = (0..commands.len()).collect();
        indices.sort_by_key(|&i| commands[i].register_address);

        // Pre-calculate total register count for allocation
        let total_regs: usize = commands
            .iter()
            .map(|c| c.data_format.register_count() as usize)
            .sum();

        // Build merged register buffer with pre-allocated capacity
        let start_addr = commands[indices[0]].register_address;
        let mut registers = Vec::with_capacity(total_regs);

        for &i in &indices {
            let cmd = &commands[i];
            let raw_value = cmd.value.as_f64().unwrap_or(0.0);
            match encode_value(raw_value, cmd.data_format, cmd.byte_order) {
                Ok(regs) => registers.extend(regs),
                Err(e) => {
                    failures.push((cmd.point_id, e.to_string()));
                    // Continue building buffer, but record failure
                },
            }
        }

        if registers.is_empty() {
            return Ok(0);
        }

        // Execute merged FC16
        debug!(
            "FC16 merge: {} commands → {} registers starting at {}",
            commands.len(),
            registers.len(),
            start_addr
        );

        client.write_10(slave_id, start_addr, &registers).await?;

        // Count successful writes (only counting failures added in this function)
        let encode_failures = failures.len() - initial_failures;
        Ok(commands.len().saturating_sub(encode_failures))
    }
}

impl ProtocolCapabilities for ModbusChannel {
    fn name(&self) -> &'static str {
        match self.config.connection_mode {
            ConnectionMode::Tcp => "Modbus TCP",
            #[cfg(feature = "modbus")]
            ConnectionMode::Rtu => "Modbus RTU",
        }
    }

    fn supported_modes(&self) -> &[CommunicationMode] {
        &[CommunicationMode::Polling]
    }

    fn version(&self) -> &'static str {
        "1.0"
    }
}

impl LoggableProtocol for ModbusChannel {
    fn set_log_handler(&mut self, handler: Arc<dyn ChannelLogHandler>) {
        // Create a new LogContext with the handler
        let new_ctx = LogContext::new(self.channel_id)
            .with_handler(handler)
            .with_config(self.log_context.config().clone());
        self.log_context = Arc::new(new_ctx);
    }

    fn set_log_config(&mut self, config: ChannelLogConfig) {
        // We need to recreate the LogContext since it's behind Arc
        // For simplicity, we use Arc::make_mut pattern or recreate
        if let Some(ctx) = Arc::get_mut(&mut self.log_context) {
            ctx.set_config(config);
        } else {
            // Clone and update
            let mut new_ctx = (*self.log_context).clone();
            new_ctx.set_config(config);
            self.log_context = Arc::new(new_ctx);
        }
    }

    fn log_config(&self) -> &ChannelLogConfig {
        self.log_context.config()
    }
}

impl Protocol for ModbusChannel {
    fn connection_state(&self) -> ConnectionState {
        self.get_state()
    }

    #[allow(clippy::disallowed_methods)] // json! macro
    async fn diagnostics(&self) -> Result<Diagnostics> {
        let state = self.get_state();

        Ok(Diagnostics {
            protocol: ProtocolCapabilities::name(self).to_string(),
            connection_state: state,
            read_count: self.diagnostics.read_count(),
            write_count: self.diagnostics.write_count(),
            error_count: self.diagnostics.error_count(),
            last_error: self.diagnostics.last_error(),
            extra: serde_json::json!({
                "address": self.config.address,
                "points": self.config.points.len(),
            }),
        })
    }
}

impl ProtocolClient for ModbusChannel {
    async fn connect(&mut self) -> Result<()> {
        let start_time = std::time::Instant::now();
        let old_state = self.get_state();
        self.set_state(ConnectionState::Connecting);

        // Log state change
        self.log_context
            .log_state_changed(old_state, ConnectionState::Connecting)
            .await;

        // Get endpoint for logging - use Cow to avoid clone for TCP
        let endpoint: std::borrow::Cow<'_, str> = match self.config.connection_mode {
            ConnectionMode::Tcp => std::borrow::Cow::Borrowed(&self.config.address),
            #[cfg(feature = "modbus")]
            ConnectionMode::Rtu => std::borrow::Cow::Owned(format!(
                "{}@{}",
                self.config.rtu_device, self.config.baud_rate
            )),
        };

        // Create client based on connection mode
        let connect_result = match self.config.connection_mode {
            ConnectionMode::Tcp => {
                // TCP connection
                match ModbusTcpClient::from_address(
                    &self.config.address,
                    self.config.connect_timeout,
                )
                .await
                {
                    Ok(client) => Ok(ModbusClientWrapper::Tcp(client)),
                    Err(e) => Err(GatewayError::Connection(e.to_string())),
                }
            },
            #[cfg(feature = "modbus")]
            ConnectionMode::Rtu => {
                // RTU serial connection
                match ModbusRtuClient::new(&self.config.rtu_device, self.config.baud_rate) {
                    Ok(client) => Ok(ModbusClientWrapper::Rtu(client)),
                    Err(e) => Err(GatewayError::Connection(e.to_string())),
                }
            },
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match connect_result {
            Ok(wrapper) => {
                self.client = Some(wrapper);
                self.set_state(ConnectionState::Connected);

                // Log successful connection - deref Cow to &str
                self.log_context
                    .log_connected(&*endpoint, duration_ms)
                    .await;
                self.log_context
                    .log_state_changed(ConnectionState::Connecting, ConnectionState::Connected)
                    .await;

                // Note: grouped_points is lazily initialized via OnceLock on first poll
                Ok(())
            },
            Err(e) => {
                self.set_state(ConnectionState::Error);
                let err_msg = e.to_string();
                self.record_error(err_msg.clone());

                // Log error
                self.log_context
                    .log_error(&err_msg, ErrorContext::Connection)
                    .await;
                self.log_context
                    .log_state_changed(ConnectionState::Connecting, ConnectionState::Error)
                    .await;

                Err(e)
            },
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        let old_state = self.get_state();

        if let Some(mut client) = self.client.take() {
            let _ = client.close().await;
        }
        self.set_state(ConnectionState::Disconnected);

        // Log disconnection
        self.log_context.log_disconnected(None).await;
        self.log_context
            .log_state_changed(old_state, ConnectionState::Disconnected)
            .await;

        Ok(())
    }

    async fn poll_once(&mut self) -> PollResult {
        let start_time = std::time::Instant::now();

        // Check client availability first (before immutable borrow for grouped points)
        if self.client.is_none() {
            self.log_context
                .log_error("Not connected", ErrorContext::Polling)
                .await;
            // Return failed result for all configured points
            let failures: Vec<_> = self
                .config
                .points
                .iter()
                .map(|p| PointFailure::new(p.id, "Not connected"))
                .collect();
            return PollResult::failed(failures);
        }

        // Get grouped points (lazily initialized via OnceLock - zero overhead after first call)
        // Clone to owned Vec to release the borrow before getting mutable client reference
        let groups: Vec<_> = self
            .get_or_init_grouped_points()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        // Get mutable client reference using if-let to avoid expect()
        // This is safer than expect() as it handles the edge case gracefully
        let Some(client) = self.client.as_mut() else {
            // This should not happen due to the check above, but handle it gracefully
            self.log_context
                .log_error("Client unavailable after check", ErrorContext::Polling)
                .await;
            let failures: Vec<_> = self
                .config
                .points
                .iter()
                .map(|p| PointFailure::new(p.id, "Client unavailable"))
                .collect();
            return PollResult::failed(failures);
        };

        let mut batch = DataBatch::default();
        let mut read_count = 0u64;
        let mut error_count = 0u64;

        // Pre-calculate total points for capacity hint
        let total_points: usize = groups.iter().map(|(_, pts)| pts.len()).sum();
        let mut failures = Vec::with_capacity(total_points);

        for ((_slave_id, _fc), points) in groups.iter() {
            let results = Self::read_point_group(
                client,
                points,
                self.config.max_batch_size,
                self.config.max_gap,
            )
            .await;

            if results.is_empty() && !points.is_empty() {
                // Read returned no results - record failures for all points in group
                error_count += 1;
                for point in points.iter() {
                    failures.push(PointFailure::new(point.id, "Read failed - no response"));
                }
            }

            for (_point_id, data_point) in results {
                batch.add(data_point);
                read_count += 1;
            }
        }

        // Update diagnostics (lock-free atomic operations)
        self.diagnostics.add_read(read_count);
        self.diagnostics.add_error(error_count);

        let duration_ms = start_time.elapsed().as_millis() as u64;

        debug!(
            "[{}] poll_once: read {} points, {} failures",
            self.config.address,
            batch.len(),
            failures.len()
        );

        // Log poll cycle (pass count instead of cloning batch)
        self.log_context
            .log_poll_cycle(
                batch.len(),
                duration_ms,
                read_count as usize,
                error_count as usize,
            )
            .await;

        if failures.is_empty() {
            PollResult::success(batch)
        } else {
            PollResult::partial(batch, failures)
        }
    }

    async fn write_control(&mut self, commands: &[ControlCommand]) -> Result<WriteResult> {
        let start_time = std::time::Instant::now();
        let mut success_count = 0;
        let mut failures: Vec<(u32, String)> = Vec::with_capacity(commands.len());

        // Get client reference (no lock needed - we own it)
        let client = match self.client.as_mut() {
            Some(c) => c,
            None => {
                let err = GatewayError::NotConnected;
                self.log_context
                    .log_control_write(
                        commands,
                        Err(err.to_string()),
                        start_time.elapsed().as_millis() as u64,
                    )
                    .await;
                return Err(err);
            },
        };

        for cmd in commands {
            // Find point config
            let point = self.config.points.iter().find(|p| p.id == cmd.id);

            let point = match point {
                Some(p) => p,
                None => {
                    failures.push((cmd.id, "Point not found".into()));
                    continue;
                },
            };

            let modbus_addr = match &point.address {
                ProtocolAddress::Modbus(addr) => addr, // Borrow instead of clone
                _ => {
                    failures.push((cmd.id, "Invalid address type".into()));
                    continue;
                },
            };

            // Apply reverse transform for control (boolean inversion)
            let value = point.transform.apply_bool(cmd.value);

            // Write based on function_code from mapping
            let result = match modbus_addr.function_code {
                5 => {
                    // FC05: Write Single Coil (bool)
                    client
                        .write_05(modbus_addr.slave_id, modbus_addr.register, value)
                        .await
                },
                6 => {
                    // FC06: Write Single Register (u16)
                    // Convert bool to 0/1
                    let reg_value = if value { 1u16 } else { 0u16 };
                    client
                        .write_06(modbus_addr.slave_id, modbus_addr.register, reg_value)
                        .await
                },
                16 => {
                    // FC16: Write Multiple Registers
                    let reg_value = if value { 1u16 } else { 0u16 };
                    client
                        .write_10(modbus_addr.slave_id, modbus_addr.register, &[reg_value])
                        .await
                },
                fc => {
                    failures.push((
                        cmd.id,
                        format!("Unsupported function code {} for control", fc),
                    ));
                    continue;
                },
            };

            match result {
                Ok(_) => {
                    success_count += 1;
                },
                Err(e) => {
                    // Store error directly in failures, no separate Vec needed
                    failures.push((
                        cmd.id,
                        format!(
                            "Control write slave {} reg {}: {}",
                            modbus_addr.slave_id, modbus_addr.register, e
                        ),
                    ));
                },
            }
        }

        // Update diagnostics (lock-free atomic operations)
        self.diagnostics.add_write(success_count as u64);
        let error_count = failures.len();
        if error_count > 0 {
            self.diagnostics.add_error(error_count as u64);
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let result = WriteResult {
            success_count,
            failures,
        };

        // Log control write
        self.log_context
            .log_control_write(commands, Ok(result.clone()), duration_ms)
            .await;

        Ok(result)
    }

    async fn write_adjustment(&mut self, adjustments: &[AdjustmentCommand]) -> Result<WriteResult> {
        let start_time = std::time::Instant::now();
        let mut success_count = 0;
        let mut failures: Vec<(u32, String)> = Vec::with_capacity(adjustments.len());

        // Get client reference (no lock needed - we own it)
        let client = match self.client.as_mut() {
            Some(c) => c,
            None => {
                let err = GatewayError::NotConnected;
                self.log_context
                    .log_adjustment_write(
                        adjustments,
                        Err(err.to_string()),
                        start_time.elapsed().as_millis() as u64,
                    )
                    .await;
                return Err(err);
            },
        };

        for adj in adjustments {
            // Find point config
            let point = self.config.points.iter().find(|p| p.id == adj.id);

            let point = match point {
                Some(p) => p,
                None => {
                    failures.push((adj.id, "Point not found".into()));
                    continue;
                },
            };

            let modbus_addr = match &point.address {
                ProtocolAddress::Modbus(addr) => addr, // Borrow instead of clone
                _ => {
                    failures.push((adj.id, "Invalid address type".into()));
                    continue;
                },
            };

            // Apply reverse transform to get raw value
            let raw_value = match reverse_transform(adj.value, &point.transform) {
                Ok(v) => v,
                Err(e) => {
                    failures.push((adj.id, e.to_string()));
                    continue;
                },
            };

            // Encode and write register(s)
            let result = match modbus_addr.format {
                DataFormat::UInt16 | DataFormat::Int16 => {
                    let value = raw_value as u16;
                    client
                        .write_06(modbus_addr.slave_id, modbus_addr.register, value)
                        .await
                },
                DataFormat::UInt32 | DataFormat::Int32 | DataFormat::Float32 => {
                    let regs = encode_value(raw_value, modbus_addr.format, modbus_addr.byte_order)?;
                    client
                        .write_10(modbus_addr.slave_id, modbus_addr.register, &regs)
                        .await
                },
                _ => {
                    failures.push((adj.id, "Unsupported format for write".into()));
                    continue;
                },
            };

            match result {
                Ok(_) => {
                    success_count += 1;
                },
                Err(e) => {
                    // Store error directly in failures, no separate Vec needed
                    failures.push((
                        adj.id,
                        format!(
                            "Adjust write slave {} reg {}: {}",
                            modbus_addr.slave_id, modbus_addr.register, e
                        ),
                    ));
                },
            }
        }

        // Update diagnostics (lock-free atomic operations)
        self.diagnostics.add_write(success_count as u64);
        let error_count = failures.len();
        if error_count > 0 {
            self.diagnostics.add_error(error_count as u64);
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let result = WriteResult {
            success_count,
            failures,
        };

        // Log adjustment write
        self.log_context
            .log_adjustment_write(adjustments, Ok(result.clone()), duration_ms)
            .await;

        Ok(result)
    }
}

/// Decode Modbus registers to a Value.
fn decode_registers(
    regs: &[u16],
    format: crate::protocols::core::point::DataFormat,
    byte_order: crate::protocols::core::point::ByteOrder,
    bit_position: Option<u8>,
) -> Result<Value> {
    use crate::protocols::codec::byte_order::decode_registers as codec_decode;

    codec_decode(regs, format, byte_order, bit_position)
}

/// Encode a Value to Modbus registers.
fn encode_value(
    value: f64,
    format: crate::protocols::core::point::DataFormat,
    byte_order: crate::protocols::core::point::ByteOrder,
) -> Result<Vec<u16>> {
    use crate::protocols::codec::byte_order::encode_registers;

    encode_registers(&Value::Float(value), format, byte_order)
}

/// Apply transform to a value.
fn apply_transform(
    value: Value,
    transform: &crate::protocols::core::point::TransformConfig,
) -> Value {
    match value {
        Value::Float(v) => Value::Float(transform.apply(v)),
        Value::Integer(v) => Value::Float(transform.apply(v as f64)),
        Value::Bool(v) => Value::Bool(transform.apply_bool(v)),
        other => other,
    }
}

/// Reverse transform to get raw value.
fn reverse_transform(
    value: f64,
    transform: &crate::protocols::core::point::TransformConfig,
) -> crate::protocols::core::error::Result<f64> {
    transform.reverse_apply(value)
}

// ============================================================================
// ChannelRuntime implementation (direct, no wrapper needed)
// ============================================================================

#[async_trait]
impl ChannelRuntime for ModbusChannel {
    fn id(&self) -> u32 {
        self.channel_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> &str {
        "modbus"
    }

    fn is_event_driven(&self) -> bool {
        false
    }

    async fn connect(&mut self) -> Result<()> {
        <Self as ProtocolClient>::connect(self).await
    }

    async fn disconnect(&mut self) -> Result<()> {
        <Self as ProtocolClient>::disconnect(self).await
    }

    async fn poll_once(&mut self) -> PollResult {
        <Self as ProtocolClient>::poll_once(self).await
    }

    async fn write_control(&mut self, commands: &[(u32, f64)]) -> Result<usize> {
        let cmds: Vec<_> = commands
            .iter()
            .map(|(id, value)| ControlCommand::latching(*id, *value != 0.0))
            .collect();
        let result = <Self as ProtocolClient>::write_control(self, &cmds).await?;
        Ok(result.success_count)
    }

    async fn write_adjustment(&mut self, adjustments: &[(u32, f64)]) -> Result<usize> {
        let adjs: Vec<_> = adjustments
            .iter()
            .map(|(id, value)| AdjustmentCommand::new(*id, *value))
            .collect();
        let result = <Self as ProtocolClient>::write_adjustment(self, &adjs).await?;
        Ok(result.success_count)
    }

    fn subscribe(&self) -> Option<DataEventReceiver> {
        None // Modbus is polling-only
    }

    async fn start_events(&mut self) -> Result<()> {
        Ok(()) // No-op for polling channel
    }

    async fn stop_events(&mut self) -> Result<()> {
        Ok(()) // No-op for polling channel
    }

    async fn diagnostics(&self) -> Result<Diagnostics> {
        <Self as Protocol>::diagnostics(self).await
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // unwrap in tests
mod tests {
    use super::*;

    #[test]
    fn test_modbus_channel_config() {
        let config = ModbusChannelConfig::tcp("127.0.0.1:502")
            .with_connect_timeout(Duration::from_secs(10))
            .with_io_timeout(Duration::from_secs(5));

        assert_eq!(config.address, "127.0.0.1:502");
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.io_timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_modbus_channel_capabilities() {
        let config = ModbusChannelConfig::tcp("127.0.0.1:502");
        let channel = ModbusChannel::new(config, 1, "test_modbus".to_string());

        // ProtocolCapabilities::name returns "Modbus TCP"
        assert_eq!(ProtocolCapabilities::name(&channel), "Modbus TCP");
        assert_eq!(channel.supported_modes(), &[CommunicationMode::Polling]);
    }

    #[test]
    fn test_polling_interval_builder() {
        let config = ModbusChannelConfig::tcp("127.0.0.1:502");
        let channel = ModbusChannel::new(config, 1, "test".to_string()).with_polling_interval(500);

        assert_eq!(channel.polling_interval_ms, 500);
    }

    #[test]
    fn test_reconnect_config_defaults() {
        let config = ReconnectConfig::default();

        assert_eq!(config.cooldown_ms, 60_000);
        assert_eq!(config.max_attempts, 0);
        assert_eq!(config.zero_data_threshold, 5);
    }

    #[test]
    fn test_reconnect_config_builder() {
        let config = ReconnectConfig::new()
            .with_cooldown_ms(30_000)
            .with_max_attempts(10)
            .with_zero_data_threshold(3);

        assert_eq!(config.cooldown_ms, 30_000);
        assert_eq!(config.max_attempts, 10);
        assert_eq!(config.zero_data_threshold, 3);
    }

    #[test]
    fn test_modbus_channel_with_reconnect() {
        let reconnect = ReconnectConfig::new().with_cooldown_ms(10_000);
        let config = ModbusChannelConfig::tcp("127.0.0.1:502").with_reconnect(reconnect);

        let channel = ModbusChannel::new(config, 1, "test".to_string());
        assert_eq!(channel.config.reconnect.cooldown_ms, 10_000);
    }

    #[test]
    fn test_tcp_config_connection_mode() {
        let config = ModbusChannelConfig::tcp("192.168.1.100:502");
        assert_eq!(config.connection_mode, ConnectionMode::Tcp);
        assert_eq!(config.address, "192.168.1.100:502");

        let channel = ModbusChannel::new(config, 1, "test_tcp".to_string());
        assert_eq!(ProtocolCapabilities::name(&channel), "Modbus TCP");
    }

    #[cfg(feature = "modbus")]
    #[test]
    fn test_rtu_config() {
        let config = ModbusChannelConfig::rtu("/dev/ttyUSB0", 9600);

        assert_eq!(config.connection_mode, ConnectionMode::Rtu);
        assert_eq!(config.rtu_device, "/dev/ttyUSB0");
        assert_eq!(config.baud_rate, 9600);

        let channel = ModbusChannel::new(config, 1, "test_rtu".to_string());
        // ProtocolCapabilities::name returns protocol name, ChannelRuntime::name returns instance name
        assert_eq!(ProtocolCapabilities::name(&channel), "Modbus RTU");
    }
}
