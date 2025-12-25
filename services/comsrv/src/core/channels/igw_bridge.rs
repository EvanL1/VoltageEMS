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
use tracing::{debug, error, warn};

use igw::core::data::{DataBatch, DataType};
use igw::core::error::GatewayError;
use igw::core::logging::{
    ChannelLogConfig, ChannelLogHandler, LoggableProtocol, TracingLogHandler,
};
use igw::core::point::{
    ByteOrder, DataFormat, ModbusAddress, PointConfig, ProtocolAddress, TransformConfig,
    VirtualAddress,
};
use igw::core::traits::{AdjustmentCommand, ControlCommand, WriteResult};
use igw::protocols::modbus::{ModbusChannel, ModbusChannelConfig, ReconnectConfig};
use igw::protocols::virtual_channel::{VirtualChannel, VirtualChannelConfig};
use igw::{ConnectionState, Protocol, ProtocolClient};

#[cfg(all(target_os = "linux", feature = "gpio"))]
use igw::protocols::gpio::{GpioChannel, GpioChannelConfig, GpioDirection, GpioPinConfig};

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
            Self::Gpio(_) => {}, // GpioChannel doesn't implement LoggableProtocol
        }
    }

    /// Set the log configuration.
    ///
    /// Only ModbusChannel supports logging; VirtualChannel and GpioChannel are no-ops.
    pub fn set_log_config(&mut self, config: ChannelLogConfig) {
        match self {
            Self::Virtual(_) => {}, // VirtualChannel doesn't implement LoggableProtocol
            Self::Modbus(c) => c.set_log_config(config),
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            Self::Gpio(_) => {}, // GpioChannel doesn't implement LoggableProtocol
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
    async fn run_command_executor(
        protocol: Arc<RwLock<ProtocolClientImpl>>,
        mut command_rx: mpsc::Receiver<ChannelCommand>,
        channel_id: u32,
    ) {
        debug!("Ch{} igw command executor started", channel_id);

        while let Some(cmd) = command_rx.recv().await {
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

/// Convert RuntimeChannelConfig to IGW PointConfig list for Modbus.
///
/// Unlike the virtual channel conversion which uses point IDs as addresses,
/// this function uses the actual Modbus mappings (slave_id, function_code, register_address).
/// It also looks up the corresponding point to get scale/offset/reverse for TransformConfig.
pub fn convert_to_modbus_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<PointConfig> {
    let mut configs = Vec::with_capacity(runtime_config.modbus_mappings.len());

    for mapping in &runtime_config.modbus_mappings {
        // Determine data type from telemetry_type
        let data_type = match mapping.telemetry_type.as_str() {
            "T" | "telemetry" => DataType::Telemetry,
            "S" | "signal" => DataType::Signal,
            "C" | "control" => DataType::Control,
            "A" | "adjustment" => DataType::Adjustment,
            _ => DataType::Telemetry, // Default to telemetry
        };

        // Parse data format from string
        let format = parse_data_format(&mapping.data_type);

        // Parse byte order from string
        let byte_order = parse_byte_order(&mapping.byte_order);

        // Build ModbusAddress
        let modbus_addr = ModbusAddress {
            slave_id: mapping.slave_id,
            function_code: mapping.function_code,
            register: mapping.register_address,
            format,
            byte_order,
            bit_position: if mapping.bit_position > 0 {
                Some(mapping.bit_position)
            } else {
                None
            },
        };

        // Look up the corresponding point to get transform parameters
        let transform = match data_type {
            DataType::Telemetry => runtime_config
                .telemetry_points
                .iter()
                .find(|p| p.base.point_id == mapping.point_id)
                .map(|p| TransformConfig {
                    scale: p.scale,
                    offset: p.offset,
                    reverse: p.reverse,
                    ..Default::default()
                })
                .unwrap_or_default(),
            DataType::Signal => runtime_config
                .signal_points
                .iter()
                .find(|p| p.base.point_id == mapping.point_id)
                .map(|p| TransformConfig {
                    reverse: p.reverse,
                    ..Default::default()
                })
                .unwrap_or_default(),
            DataType::Control => runtime_config
                .control_points
                .iter()
                .find(|p| p.base.point_id == mapping.point_id)
                .map(|p| TransformConfig {
                    reverse: p.reverse,
                    ..Default::default()
                })
                .unwrap_or_default(),
            DataType::Adjustment => runtime_config
                .adjustment_points
                .iter()
                .find(|p| p.base.point_id == mapping.point_id)
                .map(|p| TransformConfig {
                    scale: p.scale,
                    offset: p.offset,
                    ..Default::default()
                })
                .unwrap_or_default(),
        };

        let point_config = PointConfig::new(
            mapping.point_id,
            data_type,
            ProtocolAddress::Modbus(modbus_addr),
        )
        .with_transform(transform);

        configs.push(point_config);
    }

    configs
}

/// Parse data format string to DataFormat enum.
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
/// Note: Only available on Linux with `gpio` feature enabled.
/// Storage is handled by the service layer (ChannelManager) after polling.
///
/// # Arguments
///
/// * `_channel_id` - Unique channel identifier
/// * `runtime_config` - Channel configuration containing GPIO pin mappings
#[cfg(all(target_os = "linux", feature = "gpio"))]
pub fn create_gpio_channel(
    _channel_id: u32,
    runtime_config: &RuntimeChannelConfig,
) -> ProtocolClientImpl {
    use std::time::Duration;

    let mut gpio_config = GpioChannelConfig::new();

    // Get default chip from channel parameters or use "gpiochip0"
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
        .and_then(|v| v.as_u64())
    {
        gpio_config = gpio_config.with_poll_interval(Duration::from_millis(interval_ms));
    }

    // Configure DI pins from signal points (遥信)
    for pt in &runtime_config.signal_points {
        if let Some(gpio_num) = runtime_config.get_gpio_mapping(pt.base.point_id) {
            let pin_config =
                GpioPinConfig::digital_input(default_chip, gpio_num, pt.base.point_id.to_string())
                    .with_active_low(pt.reverse);

            gpio_config = gpio_config.add_pin(pin_config);
        }
    }

    // Configure DO pins from control points (遥控)
    for pt in &runtime_config.control_points {
        if let Some(gpio_num) = runtime_config.get_gpio_mapping(pt.base.point_id) {
            let pin_config =
                GpioPinConfig::digital_output(default_chip, gpio_num, pt.base.point_id.to_string())
                    .with_active_low(pt.reverse);

            gpio_config = gpio_config.add_pin(pin_config);
        }
    }

    ProtocolClientImpl::Gpio(GpioChannel::new(gpio_config))
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
            },
            reverse: false,
        });

        config.control_points.push(ControlPoint {
            base: Point {
                point_id: 30,
                signal_name: "switch".to_string(),
                description: None,
                unit: None,
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

    #[test]
    fn test_convert_to_modbus_point_configs() {
        use crate::core::config::ModbusMapping;

        let mut runtime_config = create_test_runtime_config();

        // Add Modbus mappings
        runtime_config.modbus_mappings = vec![
            ModbusMapping {
                channel_id: 1,
                point_id: 100,
                telemetry_type: "T".to_string(),
                slave_id: 1,
                function_code: 3,
                register_address: 0,
                data_type: "float32".to_string(),
                byte_order: "ABCD".to_string(),
                bit_position: 0,
            },
            ModbusMapping {
                channel_id: 1,
                point_id: 101,
                telemetry_type: "S".to_string(),
                slave_id: 1,
                function_code: 1,
                register_address: 10,
                data_type: "bool".to_string(),
                byte_order: "ABCD".to_string(),
                bit_position: 5,
            },
        ];

        let configs = convert_to_modbus_point_configs(&runtime_config);

        assert_eq!(configs.len(), 2);

        // Check first point (telemetry, float32)
        let pt1 = configs.iter().find(|c| c.id == 100).unwrap();
        assert_eq!(pt1.data_type, DataType::Telemetry);
        if let ProtocolAddress::Modbus(addr) = &pt1.address {
            assert_eq!(addr.slave_id, 1);
            assert_eq!(addr.function_code, 3);
            assert_eq!(addr.register, 0);
            assert_eq!(addr.format, DataFormat::Float32);
            assert_eq!(addr.byte_order, ByteOrder::Abcd);
        } else {
            panic!("Expected ModbusAddress");
        }

        // Check second point (signal, bool with bit_position)
        let pt2 = configs.iter().find(|c| c.id == 101).unwrap();
        assert_eq!(pt2.data_type, DataType::Signal);
        if let ProtocolAddress::Modbus(addr) = &pt2.address {
            assert_eq!(addr.slave_id, 1);
            assert_eq!(addr.function_code, 1);
            assert_eq!(addr.register, 10);
            assert_eq!(addr.format, DataFormat::Bool);
            assert_eq!(addr.bit_position, Some(5));
        } else {
            panic!("Expected ModbusAddress");
        }
    }

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
