//! IGW Bridge Module
//!
//! This module provides integration between comsrv's channel management
//! and IGW protocol implementations.
//!
//! # Architecture
//!
//! Protocol layer (igw) is now separated from storage. IgwChannelWrapper
//! handles the poll-then-store pattern:
//!
//! ```text
//! ChannelManager
//!         ↓ create_*_channel() + IgwChannelWrapper::new()
//! IgwChannelWrapper
//!         ├─ protocol: ProtocolClientImpl (enum-based, replaces dyn ProtocolClient)
//!         ├─ store: RedisDataStore (service layer storage)
//!         └─ poll_once() → protocol.poll_once() → store.write_batch()
//! ```

use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
#[cfg(all(target_os = "linux", feature = "gpio"))]
use tracing::info;
use tracing::{debug, error, warn};

use igw::core::data::{DataBatch, DataType};
use igw::core::error::GatewayError;
use igw::core::logging::{
    ChannelLogConfig, ChannelLogHandler, LoggableProtocol, TracingLogHandler,
};
use igw::core::point::{
    ByteOrder, DataFormat, PointConfig, ProtocolAddress, TransformConfig, VirtualAddress,
};
use igw::core::traits::{AdjustmentCommand, ControlCommand, WriteResult};
use igw::protocols::modbus::{ModbusChannel, ModbusChannelConfig, ReconnectConfig};
use igw::protocols::virtual_channel::{VirtualChannel, VirtualChannelConfig};
use igw::{ConnectionState, Protocol, ProtocolClient};

#[cfg(all(target_os = "linux", feature = "gpio"))]
use igw::protocols::gpio::{GpioChannel, GpioChannelConfig, GpioPinConfig};

use crate::core::channels::traits::ChannelCommand;
use crate::core::channels::types::ChannelStatus;
use crate::core::config::RuntimeChannelConfig;
use crate::store::RedisDataStore;
use voltage_rtdb::Rtdb;

// ============================================================================
// ProtocolClientImpl - Enum-based protocol dispatch (replaces Box<dyn ProtocolClient>)
// ============================================================================

/// Enum-based protocol client implementation.
///
/// This replaces `Box<dyn ProtocolClient>` to work with igw 0.2.2's AFIT
/// (Async Functions In Traits) which makes `ProtocolClient` not dyn-compatible.
///
/// Benefits:
/// - Compile-time type safety
/// - No vtable overhead
/// - Exhaustive match ensures all protocols are handled
pub enum ProtocolClientImpl {
    /// Virtual channel for testing and simulation
    Virtual(VirtualChannel),
    /// Modbus TCP or RTU channel
    Modbus(ModbusChannel),
    /// GPIO channel for digital I/O (Linux only)
    #[cfg(all(target_os = "linux", feature = "gpio"))]
    Gpio(GpioChannel),
}

impl ProtocolClientImpl {
    /// Poll once to get data from the device.
    pub async fn poll_once(&mut self) -> Result<DataBatch, GatewayError> {
        match self {
            Self::Virtual(c) => c.poll_once().await,
            Self::Modbus(c) => c.poll_once().await,
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(c) => c.poll_once().await,
        }
    }

    /// Connect to the device.
    pub async fn connect(&mut self) -> Result<(), GatewayError> {
        match self {
            Self::Virtual(c) => c.connect().await,
            Self::Modbus(c) => c.connect().await,
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(c) => c.connect().await,
        }
    }

    /// Disconnect from the device.
    pub async fn disconnect(&mut self) -> Result<(), GatewayError> {
        match self {
            Self::Virtual(c) => c.disconnect().await,
            Self::Modbus(c) => c.disconnect().await,
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(c) => c.disconnect().await,
        }
    }

    /// Get the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        match self {
            Self::Virtual(c) => c.connection_state(),
            Self::Modbus(c) => c.connection_state(),
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(c) => c.connection_state(),
        }
    }

    /// Write control commands to the device.
    pub async fn write_control(
        &mut self,
        commands: &[ControlCommand],
    ) -> Result<WriteResult, GatewayError> {
        match self {
            Self::Virtual(c) => c.write_control(commands).await,
            Self::Modbus(c) => c.write_control(commands).await,
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(c) => c.write_control(commands).await,
        }
    }

    /// Write adjustment commands to the device.
    pub async fn write_adjustment(
        &mut self,
        commands: &[AdjustmentCommand],
    ) -> Result<WriteResult, GatewayError> {
        match self {
            Self::Virtual(c) => c.write_adjustment(commands).await,
            Self::Modbus(c) => c.write_adjustment(commands).await,
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(c) => c.write_adjustment(commands).await,
        }
    }

    /// Set the log handler for protocol-level logging.
    ///
    /// Only ModbusChannel supports logging; VirtualChannel and GpioChannel are no-ops.
    pub fn set_log_handler(&mut self, handler: Arc<dyn ChannelLogHandler>) {
        match self {
            Self::Virtual(_) => {}, // VirtualChannel doesn't implement LoggableProtocol
            Self::Modbus(c) => c.set_log_handler(handler),
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(c) => c.set_log_handler(handler),
        }
    }

    /// Set the log configuration.
    ///
    /// ModbusChannel and GpioChannel support logging; VirtualChannel is no-op.
    pub fn set_log_config(&mut self, config: ChannelLogConfig) {
        match self {
            Self::Virtual(_) => {}, // VirtualChannel doesn't implement LoggableProtocol
            Self::Modbus(c) => c.set_log_config(config),
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(c) => c.set_log_config(config),
        }
    }

    /// Configure logging with TracingLogHandler (default config).
    ///
    /// This is a convenience method that sets up tracing integration
    /// for ModbusChannel. VirtualChannel is a no-op.
    pub fn enable_tracing_logs(&mut self) {
        self.set_log_handler(Arc::new(TracingLogHandler));
        self.set_log_config(ChannelLogConfig::default());
    }
}

// ============================================================================
// IgwChannelWrapper - Protocol wrapper with storage integration
// ============================================================================

/// Wrapper for IGW protocol clients that integrates with comsrv's command system.
///
/// This wrapper:
/// - Holds an IGW ProtocolClient implementation (enum-based)
/// - Spawns a background task to process incoming commands
/// - Provides access to the underlying protocol for status queries
pub struct IgwChannelWrapper<R: Rtdb> {
    /// The IGW protocol client (enum-based for AFIT compatibility)
    protocol: Arc<RwLock<ProtocolClientImpl>>,
    /// Channel ID
    channel_id: u32,
    /// Data store for persisting polled data
    store: Arc<RedisDataStore<R>>,
    /// Command executor task handle
    _executor_handle: Option<tokio::task::JoinHandle<()>>,
}

impl<R: Rtdb> IgwChannelWrapper<R> {
    /// Create a new IGW channel wrapper with command processing and storage.
    pub fn new(
        protocol: ProtocolClientImpl,
        channel_id: u32,
        store: Arc<RedisDataStore<R>>,
        command_rx: mpsc::Receiver<ChannelCommand>,
    ) -> Self {
        let protocol = Arc::new(RwLock::new(protocol));
        let protocol_clone = Arc::clone(&protocol);

        // Spawn command executor task
        let executor_handle = tokio::spawn(async move {
            Self::run_command_executor(protocol_clone, command_rx, channel_id).await;
        });

        Self {
            protocol,
            channel_id,
            store,
            _executor_handle: Some(executor_handle),
        }
    }

    /// Poll once and write data to store.
    ///
    /// This is the main data acquisition method:
    /// 1. Call protocol.poll_once() to get DataBatch from device
    /// 2. Write the batch to RedisDataStore (with transformations and routing)
    pub async fn poll_once(&self) -> crate::error::Result<usize> {
        let mut protocol = self.protocol.write().await;
        let batch = protocol
            .poll_once()
            .await
            .map_err(|e| crate::error::ComSrvError::ProtocolError(e.to_string()))?;

        let count = batch.len();
        if count > 0 {
            self.store
                .write_batch(self.channel_id, batch)
                .await
                .map_err(|e| crate::error::ComSrvError::storage(e.to_string()))?;
            // Note: igw already logs poll results at debug level
        }

        Ok(count)
    }

    /// Get the protocol client for status queries.
    pub fn protocol(&self) -> &Arc<RwLock<ProtocolClientImpl>> {
        &self.protocol
    }

    /// Get channel ID.
    pub fn channel_id(&self) -> u32 {
        self.channel_id
    }

    /// Connect the protocol client.
    pub async fn connect(&self) -> crate::error::Result<()> {
        let mut protocol = self.protocol.write().await;
        protocol
            .connect()
            .await
            .map_err(|e| crate::error::ComSrvError::ConnectionError(e.to_string()))
    }

    /// Disconnect the protocol client.
    pub async fn disconnect(&self) -> crate::error::Result<()> {
        let mut protocol = self.protocol.write().await;
        protocol
            .disconnect()
            .await
            .map_err(|e| crate::error::ComSrvError::ConnectionError(e.to_string()))
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        let protocol = self.protocol.read().await;
        protocol.connection_state().is_connected()
    }

    /// Run the command executor loop.
    ///
    /// This loop waits for connection before processing commands to avoid race conditions
    /// where commands arrive before connect() is called.
    async fn run_command_executor(
        protocol: Arc<RwLock<ProtocolClientImpl>>,
        mut command_rx: mpsc::Receiver<ChannelCommand>,
        channel_id: u32,
    ) {
        debug!("Ch{} igw command executor started", channel_id);

        // Wait for connection before processing commands (max 30 retries * 100ms = 3s)
        const MAX_CONNECT_WAIT_RETRIES: u32 = 30;
        const CONNECT_WAIT_INTERVAL_MS: u64 = 100;

        while let Some(cmd) = command_rx.recv().await {
            // Wait for connection if not connected
            let mut connected = false;
            for attempt in 0..MAX_CONNECT_WAIT_RETRIES {
                {
                    let protocol_guard = protocol.read().await;
                    if protocol_guard.connection_state().is_connected() {
                        connected = true;
                        break;
                    }
                }
                if attempt == 0 {
                    debug!(
                        "Ch{} waiting for connection before executing command...",
                        channel_id
                    );
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(CONNECT_WAIT_INTERVAL_MS))
                    .await;
            }

            if !connected {
                error!(
                    "Ch{} command dropped: connection timeout after {}ms",
                    channel_id,
                    MAX_CONNECT_WAIT_RETRIES as u64 * CONNECT_WAIT_INTERVAL_MS
                );
                continue;
            }

            let mut protocol_guard = protocol.write().await;

            match cmd {
                ChannelCommand::Control {
                    point_id, value, ..
                } => {
                    let commands = vec![ControlCommand::latching(point_id, value != 0.0)];
                    match protocol_guard.write_control(&commands).await {
                        Ok(result) => {
                            if result.is_success() {
                                debug!("Ch{} control pt{} = {} ok", channel_id, point_id, value);
                            } else {
                                warn!(
                                    "Ch{} control pt{} partial: {:?}",
                                    channel_id, point_id, result.failures
                                );
                            }
                        },
                        Err(e) => {
                            error!("Ch{} control pt{} err: {}", channel_id, point_id, e);
                        },
                    }
                },
                ChannelCommand::Adjustment {
                    point_id, value, ..
                } => {
                    let adjustments = vec![AdjustmentCommand::new(point_id, value)];
                    match protocol_guard.write_adjustment(&adjustments).await {
                        Ok(result) => {
                            if result.is_success() {
                                debug!("Ch{} adjustment pt{} = {} ok", channel_id, point_id, value);
                            } else {
                                warn!(
                                    "Ch{} adjustment pt{} partial: {:?}",
                                    channel_id, point_id, result.failures
                                );
                            }
                        },
                        Err(e) => {
                            error!("Ch{} adjustment pt{} err: {}", channel_id, point_id, e);
                        },
                    }
                },
            }
        }

        debug!("Ch{} igw command executor stopped", channel_id);
    }
}

// ============================================================================
// Point Configuration Conversion
// ============================================================================

/// Convert RuntimeChannelConfig to IGW PointConfig list.
///
/// This function sets up TransformConfig for each point type:
/// - Telemetry: scale/offset transformation
/// - Signal: reverse boolean transformation
/// - Control: reverse boolean transformation
/// - Adjustment: scale/offset transformation
pub fn convert_to_igw_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<PointConfig> {
    let mut configs = Vec::new();

    // Convert telemetry points with scale/offset transformation
    for pt in &runtime_config.telemetry_points {
        configs.push(
            PointConfig::new(
                pt.base.point_id,
                DataType::Telemetry,
                ProtocolAddress::Virtual(VirtualAddress::new(pt.base.point_id.to_string())),
            )
            .with_name(&pt.base.signal_name)
            .with_transform(TransformConfig {
                scale: pt.scale,
                offset: pt.offset,
                reverse: pt.reverse,
                ..Default::default()
            }),
        );
    }

    // Convert signal points with reverse transformation
    for pt in &runtime_config.signal_points {
        configs.push(
            PointConfig::new(
                pt.base.point_id,
                DataType::Signal,
                ProtocolAddress::Virtual(VirtualAddress::new(pt.base.point_id.to_string())),
            )
            .with_name(&pt.base.signal_name)
            .with_transform(TransformConfig {
                reverse: pt.reverse,
                ..Default::default()
            }),
        );
    }

    // Convert control points with reverse transformation
    for pt in &runtime_config.control_points {
        configs.push(
            PointConfig::new(
                pt.base.point_id,
                DataType::Control,
                ProtocolAddress::Virtual(VirtualAddress::new(pt.base.point_id.to_string())),
            )
            .with_name(&pt.base.signal_name)
            .with_transform(TransformConfig {
                reverse: pt.reverse,
                ..Default::default()
            }),
        );
    }

    // Convert adjustment points with scale/offset transformation
    for pt in &runtime_config.adjustment_points {
        configs.push(
            PointConfig::new(
                pt.base.point_id,
                DataType::Adjustment,
                ProtocolAddress::Virtual(VirtualAddress::new(pt.base.point_id.to_string())),
            )
            .with_name(&pt.base.signal_name)
            .with_transform(TransformConfig {
                scale: pt.scale,
                offset: pt.offset,
                ..Default::default()
            }),
        );
    }

    configs
}

// Note: convert_to_modbus_point_configs was removed - modbus mappings are now
// embedded in each point's protocol_mappings JSON field

/// Parse data format string to DataFormat enum.
#[allow(dead_code)] // Used in tests, may be needed for future Modbus support
fn parse_data_format(s: &str) -> DataFormat {
    match s.to_lowercase().as_str() {
        "bool" | "boolean" => DataFormat::Bool,
        "uint16" | "u16" => DataFormat::UInt16,
        "int16" | "i16" => DataFormat::Int16,
        "uint32" | "u32" => DataFormat::UInt32,
        "int32" | "i32" => DataFormat::Int32,
        "float32" | "f32" | "float" => DataFormat::Float32,
        "float64" | "f64" | "double" => DataFormat::Float64,
        "uint64" | "u64" => DataFormat::UInt64,
        "int64" | "i64" => DataFormat::Int64,
        _ => DataFormat::UInt16, // Default
    }
}

/// Parse byte order string to ByteOrder enum.
#[allow(dead_code)] // Used in tests, may be needed for future Modbus support
fn parse_byte_order(s: &str) -> ByteOrder {
    match s.to_uppercase().as_str() {
        "ABCD" | "BIG_ENDIAN" | "BE" => ByteOrder::Abcd,
        "DCBA" | "LITTLE_ENDIAN" | "LE" => ByteOrder::Dcba,
        "BADC" | "WORD_SWAP" => ByteOrder::Badc,
        "CDAB" | "BYTE_SWAP" => ByteOrder::Cdab,
        _ => ByteOrder::Abcd, // Default to big-endian
    }
}

// ============================================================================
// Channel Factory Functions
// ============================================================================

/// Create an IGW VirtualChannel.
///
/// Note: The channel no longer holds a store reference. Storage is handled
/// by the service layer (ChannelManager) after polling.
pub fn create_virtual_channel(
    _channel_id: u32,
    channel_name: &str,
    point_configs: Vec<PointConfig>,
) -> ProtocolClientImpl {
    let config = VirtualChannelConfig::new(channel_name).with_points(point_configs);

    ProtocolClientImpl::Virtual(VirtualChannel::new(config))
}

/// Create an IGW ModbusChannel for TCP mode.
///
/// Note: The channel no longer holds a store reference. Storage is handled
/// by the service layer (ChannelManager) after polling.
///
/// # Arguments
///
/// * `channel_id` - Unique channel identifier (used for logging in igw 0.2.2+)
/// * `host` - Modbus TCP server host address
/// * `port` - Modbus TCP server port
/// * `point_configs` - Point configurations with Modbus addresses
pub fn create_modbus_channel(
    channel_id: u32,
    host: &str,
    port: u16,
    point_configs: Vec<PointConfig>,
) -> ProtocolClientImpl {
    let address = format!("{}:{}", host, port);

    let config = ModbusChannelConfig::tcp(&address)
        .with_points(point_configs)
        .with_reconnect(ReconnectConfig::default());

    let mut client = ProtocolClientImpl::Modbus(ModbusChannel::new(config, channel_id));
    client.enable_tracing_logs();
    client
}

/// Create an IGW ModbusChannel for RTU (serial) mode.
///
/// Note: The channel no longer holds a store reference. Storage is handled
/// by the service layer (ChannelManager) after polling.
///
/// # Arguments
///
/// * `channel_id` - Unique channel identifier (used for logging in igw 0.2.2+)
/// * `device` - Serial device path (e.g., "/dev/ttyUSB0" on Linux)
/// * `baud_rate` - Serial baud rate (e.g., 9600, 19200, 115200)
/// * `point_configs` - Point configurations with Modbus addresses
pub fn create_modbus_rtu_channel(
    channel_id: u32,
    device: &str,
    baud_rate: u32,
    point_configs: Vec<PointConfig>,
) -> ProtocolClientImpl {
    let config = ModbusChannelConfig::rtu(device, baud_rate)
        .with_points(point_configs)
        .with_reconnect(ReconnectConfig::default());

    let mut client = ProtocolClientImpl::Modbus(ModbusChannel::new(config, channel_id));
    client.enable_tracing_logs();
    client
}

/// Create an IGW GpioChannel for digital I/O.
///
/// Supports two driver modes:
/// - `gpiod` (default): Modern chardev interface using `/dev/gpiochipN`
/// - `sysfs`: Legacy interface using `/sys/class/gpio/`
///
/// # Configuration Parameters
///
/// | Parameter | Description | Default |
/// |-----------|-------------|---------|
/// | `driver` | Driver type: "gpiod" or "sysfs" | "gpiod" |
/// | `gpio_chip` | GPIO chip name (gpiod only) | "gpiochip0" |
/// | `gpio_base_path` | Sysfs base path (sysfs only) | "/sys/class/gpio" |
/// | `poll_interval_ms` | Polling interval in ms | 100 |
///
/// # Arguments
///
/// * `channel_id` - Unique channel identifier (for logging)
/// * `runtime_config` - Channel configuration containing GPIO pin mappings
#[cfg(all(target_os = "linux", feature = "gpio"))]
pub fn create_gpio_channel(
    channel_id: u32,
    runtime_config: &RuntimeChannelConfig,
) -> ProtocolClientImpl {
    use std::time::Duration;

    // Determine driver type from configuration
    let driver_str = runtime_config
        .base
        .parameters
        .get("driver")
        .and_then(|v| v.as_str())
        .unwrap_or("gpiod");

    let use_sysfs = driver_str == "sysfs" || driver_str == "sysfs_gpio";

    // Create config with appropriate driver
    let mut gpio_config = if use_sysfs {
        let base_path = runtime_config
            .base
            .parameters
            .get("gpio_base_path")
            .and_then(|v| v.as_str())
            .unwrap_or("/sys/class/gpio");
        GpioChannelConfig::new_sysfs(base_path)
    } else {
        GpioChannelConfig::new() // gpiod (default)
    };

    // Get default chip from channel parameters (for gpiod mode)
    let default_chip = runtime_config
        .base
        .parameters
        .get("gpio_chip")
        .and_then(|v| v.as_str())
        .unwrap_or("gpiochip0");

    // Get poll interval from parameters
    if let Some(interval_ms) = runtime_config
        .base
        .parameters
        .get("poll_interval_ms")
        .or_else(|| runtime_config.base.parameters.get("di_poll_interval_ms"))
        .and_then(|v| v.as_u64())
    {
        gpio_config = gpio_config.with_poll_interval(Duration::from_millis(interval_ms));
    }

    // Helper to extract gpio_number directly from a point's protocol_mappings JSON.
    // This is the correct pattern: extract from the current point being iterated,
    // not via cross-type search. point_id is only unique within a point type.
    fn extract_gpio_from_mappings(protocol_mappings: &Option<String>) -> Option<u32> {
        protocol_mappings.as_ref().and_then(|json_str| {
            serde_json::from_str::<serde_json::Value>(json_str)
                .ok()
                .and_then(|v| v.get("gpio_number").and_then(|n| n.as_u64()))
                .map(|n| n as u32)
        })
    }

    // Configure DI pins from signal points (digital inputs)
    for pt in &runtime_config.signal_points {
        if let Some(gpio_num) = extract_gpio_from_mappings(&pt.base.protocol_mappings) {
            let pin_config = if use_sysfs {
                // Sysfs: use global GPIO number
                GpioPinConfig::digital_input_sysfs(gpio_num, pt.base.point_id)
                    .with_active_low(pt.reverse)
            } else {
                // Gpiod: use chip + offset
                GpioPinConfig::digital_input(default_chip, gpio_num, pt.base.point_id)
                    .with_active_low(pt.reverse)
            };
            gpio_config = gpio_config.add_pin(pin_config);
        }
    }

    // Configure DO pins from control points (digital outputs)
    info!(
        "Ch{} gpio: {} control_points",
        channel_id,
        runtime_config.control_points.len()
    );
    let mut pin_count = 0;
    for pt in &runtime_config.control_points {
        let gpio_num = extract_gpio_from_mappings(&pt.base.protocol_mappings);
        info!(
            "Ch{} gpio DO: pt{} -> gpio={:?} (mappings={:?})",
            channel_id, pt.base.point_id, gpio_num, pt.base.protocol_mappings
        );
        if let Some(gpio_num) = gpio_num {
            pin_count += 1;
            let pin_config = if use_sysfs {
                // Sysfs: use global GPIO number
                GpioPinConfig::digital_output_sysfs(gpio_num, pt.base.point_id)
                    .with_active_low(pt.reverse)
            } else {
                // Gpiod: use chip + offset
                GpioPinConfig::digital_output(default_chip, gpio_num, pt.base.point_id)
                    .with_active_low(pt.reverse)
            };
            gpio_config = gpio_config.add_pin(pin_config);
        }
    }
    info!("Ch{} gpio: added {} output pins", channel_id, pin_count);

    // Create channel and setup logging
    let mut channel = GpioChannel::new(gpio_config);
    channel.set_channel_id(channel_id);

    let mut client = ProtocolClientImpl::Gpio(channel);
    client.enable_tracing_logs();
    client
}

// ============================================================================
// ChannelImpl - Unified IGW-based channel implementation
// ============================================================================

/// Channel implementation wrapping IGW ProtocolClient.
///
/// All protocols (Modbus TCP/RTU, Virtual) now use IGW implementations.
/// The wrapper is held as Arc<RwLock<...>> for shared ownership and interior mutability.
pub type ChannelImpl<R> = Arc<RwLock<IgwChannelWrapper<R>>>;

/// Extension methods for ChannelImpl.
impl<R: Rtdb> IgwChannelWrapper<R> {
    /// Get channel status.
    pub async fn get_status(&self) -> ChannelStatus {
        ChannelStatus {
            is_connected: self.is_connected().await,
            last_update: chrono::Utc::now().timestamp(),
        }
    }

    /// Get diagnostics information.
    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap
    pub async fn get_diagnostics(&self) -> crate::error::Result<serde_json::Value> {
        let is_connected = self.is_connected().await;
        Ok(serde_json::json!({
            "protocol_type": "igw",
            "connected": is_connected,
            "channel_id": self.channel_id()
        }))
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::core::config::{
        AdjustmentPoint, ChannelConfig, ChannelCore, ControlPoint, Point, SignalPoint,
        TelemetryPoint,
    };
    use std::collections::HashMap;

    fn create_test_runtime_config() -> RuntimeChannelConfig {
        let base_config = ChannelConfig {
            core: ChannelCore {
                id: 1,
                name: "test_channel".to_string(),
                description: None,
                protocol: "virtual".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: Default::default(),
        };
        let mut config = RuntimeChannelConfig::from_base(base_config);

        config.telemetry_points.push(TelemetryPoint {
            base: Point {
                point_id: 10,
                signal_name: "temperature".to_string(),
                description: None,
                unit: Some("C".to_string()),
                protocol_mappings: None,
            },
            scale: 1.0,
            offset: 0.0,
            data_type: "float32".to_string(),
            reverse: false,
        });

        config.signal_points.push(SignalPoint {
            base: Point {
                point_id: 20,
                signal_name: "status".to_string(),
                description: None,
                unit: None,
                protocol_mappings: None,
            },
            reverse: false,
        });

        config.control_points.push(ControlPoint {
            base: Point {
                point_id: 30,
                signal_name: "switch".to_string(),
                description: None,
                unit: None,
                protocol_mappings: None,
            },
            reverse: false,
            control_type: "latching".to_string(),
            on_value: 1,
            off_value: 0,
            pulse_duration_ms: None,
        });

        config.adjustment_points.push(AdjustmentPoint {
            base: Point {
                point_id: 40,
                signal_name: "setpoint".to_string(),
                description: None,
                unit: Some("C".to_string()),
                protocol_mappings: None,
            },
            min_value: None,
            max_value: None,
            step: 1.0,
            data_type: "float32".to_string(),
            scale: 1.0,
            offset: 0.0,
        });

        config
    }

    #[test]
    fn test_convert_to_igw_point_configs() {
        let runtime_config = create_test_runtime_config();
        let configs = convert_to_igw_point_configs(&runtime_config);

        assert_eq!(configs.len(), 4);

        // Check telemetry point
        let telemetry = configs.iter().find(|c| c.id == 10).unwrap();
        assert_eq!(telemetry.data_type, DataType::Telemetry);
        assert_eq!(telemetry.name, Some("temperature".to_string()));

        // Check signal point
        let signal = configs.iter().find(|c| c.id == 20).unwrap();
        assert_eq!(signal.data_type, DataType::Signal);

        // Check control point
        let control = configs.iter().find(|c| c.id == 30).unwrap();
        assert_eq!(control.data_type, DataType::Control);

        // Check adjustment point
        let adjustment = configs.iter().find(|c| c.id == 40).unwrap();
        assert_eq!(adjustment.data_type, DataType::Adjustment);
    }

    // test_convert_to_modbus_point_configs removed - function was deleted
    // Modbus mappings are now embedded in each point's protocol_mappings JSON field

    #[test]
    fn test_parse_data_format() {
        assert_eq!(parse_data_format("bool"), DataFormat::Bool);
        assert_eq!(parse_data_format("FLOAT32"), DataFormat::Float32);
        assert_eq!(parse_data_format("uint16"), DataFormat::UInt16);
        assert_eq!(parse_data_format("Int32"), DataFormat::Int32);
    }

    #[test]
    fn test_parse_byte_order() {
        assert_eq!(parse_byte_order("ABCD"), ByteOrder::Abcd);
        assert_eq!(parse_byte_order("big_endian"), ByteOrder::Abcd);
        assert_eq!(parse_byte_order("CDAB"), ByteOrder::Cdab);
        assert_eq!(parse_byte_order("DCBA"), ByteOrder::Dcba);
    }
}
