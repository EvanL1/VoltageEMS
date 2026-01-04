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
//!         ├─ protocol: Box<dyn ChannelRuntime> (dynamic dispatch via async_trait)
//!         ├─ store: RedisDataStore (service layer storage)
//!         └─ poll_once() → protocol.poll_once() → store.write_batch()
//! ```

use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use igw::core::point::{
    ByteOrder, DataFormat, ModbusAddress, PointConfig, ProtocolAddress, TransformConfig,
    VirtualAddress,
};
use igw::core::traits::PollResult;
use igw::protocols::modbus::{ModbusChannel, ModbusChannelConfig, ReconnectConfig};
use igw::protocols::virtual_channel::{VirtualChannel, VirtualChannelConfig};

// ChannelRuntime trait and wrappers from igw
use igw::gateway::wrappers::{ModbusRuntime, VirtualRuntime};
use igw::gateway::ChannelRuntime;

#[cfg(all(feature = "can", target_os = "linux"))]
use igw::gateway::wrappers::CanRuntime;
#[cfg(all(feature = "can", target_os = "linux"))]
use igw::protocols::can::{CanClient, CanConfig, CanPoint};

#[cfg(all(target_os = "linux", feature = "gpio"))]
use igw::gateway::wrappers::GpioRuntime;
#[cfg(all(target_os = "linux", feature = "gpio"))]
use igw::protocols::gpio::{GpioChannel, GpioChannelConfig, GpioPinConfig};

use crate::core::channels::traits::ChannelCommand;
use crate::core::channels::types::ChannelStatus;
use crate::core::config::RuntimeChannelConfig;
use crate::store::RedisDataStore;
use voltage_model::PointType;
use voltage_rtdb::Rtdb;

// ============================================================================
// IgwChannelWrapper - Protocol wrapper with storage integration
// ============================================================================

/// Wrapper for IGW protocol clients that integrates with comsrv's command system.
///
/// This wrapper:
/// - Holds an IGW `ChannelRuntime` implementation (dynamic dispatch via `async_trait`)
/// - Spawns a background task to process incoming commands
/// - Provides access to the underlying protocol for status queries
/// - Starts a polling task for data acquisition
pub struct IgwChannelWrapper<R: Rtdb> {
    /// The IGW protocol client (Box<dyn ChannelRuntime> for dynamic dispatch)
    protocol: Arc<RwLock<Box<dyn ChannelRuntime>>>,
    /// Channel ID
    channel_id: u32,
    /// Data store for persisting polled data
    store: Arc<RedisDataStore<R>>,
    /// Command executor task handle (used for cleanup on disconnect)
    executor_handle: Option<tokio::task::JoinHandle<()>>,
    /// Polling task handle (used for cleanup on disconnect)
    polling_handle: Option<tokio::task::JoinHandle<()>>,
}

impl<R: Rtdb> IgwChannelWrapper<R> {
    /// Create a new IGW channel wrapper with command processing, storage, and polling.
    ///
    /// Polling is always enabled - all channels need periodic data acquisition.
    ///
    /// # Arguments
    /// * `protocol` - The protocol client implementation (Box<dyn ChannelRuntime>)
    /// * `channel_id` - Unique channel identifier
    /// * `store` - Data store for persisting polled data
    /// * `command_rx` - Receiver for control commands
    /// * `poll_interval_ms` - Polling interval in milliseconds
    pub fn new(
        protocol: Box<dyn ChannelRuntime>,
        channel_id: u32,
        store: Arc<RedisDataStore<R>>,
        command_rx: mpsc::Receiver<ChannelCommand>,
        poll_interval_ms: u64,
    ) -> Self {
        let protocol = Arc::new(RwLock::new(protocol));
        let protocol_clone = Arc::clone(&protocol);

        // Spawn command executor task
        let executor_handle = tokio::spawn(async move {
            Self::run_command_executor(protocol_clone, command_rx, channel_id).await;
        });

        // Start polling task with configured interval
        let protocol_clone = Arc::clone(&protocol);
        let store_clone = Arc::clone(&store);
        let polling_handle = Some(tokio::spawn(async move {
            run_polling_task(protocol_clone, store_clone, channel_id, poll_interval_ms).await;
        }));

        info!(
            "Ch{} started polling task (interval: {}ms)",
            channel_id, poll_interval_ms
        );

        Self {
            protocol,
            channel_id,
            store,
            executor_handle: Some(executor_handle),
            polling_handle,
        }
    }

    /// Poll once and write data to store.
    ///
    /// This is the main data acquisition method:
    /// 1. Call protocol.poll_once() to get PollResult from device
    /// 2. Write the batch to RedisDataStore (with transformations and routing)
    pub async fn poll_once(&self) -> crate::error::Result<usize> {
        let mut protocol = self.protocol.write().await;
        let result: PollResult = protocol.poll_once().await;

        // Check failures first before moving data
        if result.has_failures() {
            warn!(
                "Ch{} poll partial failures: {:?}",
                self.channel_id, result.failures
            );
        }

        let count = result.data.len();
        if count > 0 {
            self.store
                .write_batch(self.channel_id, result.data)
                .await
                .map_err(|e| crate::error::ComSrvError::storage(e.to_string()))?;
        }

        Ok(count)
    }

    /// Get the protocol client for status queries.
    pub fn protocol(&self) -> &Arc<RwLock<Box<dyn ChannelRuntime>>> {
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

    /// Shutdown all background tasks (polling and command executor).
    ///
    /// This method aborts the polling and executor tasks to prevent resource leaks
    /// when a channel is removed or reconfigured via hot-reload.
    pub fn shutdown(&mut self) {
        // Abort polling task
        if let Some(handle) = self.polling_handle.take() {
            if !handle.is_finished() {
                info!("Ch{} aborting polling task", self.channel_id);
                handle.abort();
            }
        }

        // Abort executor task
        if let Some(handle) = self.executor_handle.take() {
            if !handle.is_finished() {
                info!("Ch{} aborting executor task", self.channel_id);
                handle.abort();
            }
        }
    }

    /// Disconnect the protocol client and shutdown background tasks.
    ///
    /// This method ensures proper cleanup when a channel is removed:
    /// 1. First aborts all background tasks (polling, executor)
    /// 2. Then disconnects the underlying protocol
    pub async fn disconnect(&mut self) -> crate::error::Result<()> {
        // First shutdown background tasks to prevent orphaned tasks
        self.shutdown();

        // Then disconnect protocol
        let mut protocol = self.protocol.write().await;
        protocol
            .disconnect()
            .await
            .map_err(|e| crate::error::ComSrvError::ConnectionError(e.to_string()))
    }

    /// Check if connected.
    ///
    /// Note: ChannelRuntime doesn't expose connection state directly.
    /// We use diagnostics as a proxy - if diagnostics succeed, we assume connected.
    pub async fn is_connected(&self) -> bool {
        let protocol = self.protocol.read().await;
        // If diagnostics returns Ok, we consider it connected
        protocol.diagnostics().await.is_ok()
    }

    /// Run the command executor loop.
    ///
    /// Uses ChannelRuntime's `write_control` and `write_adjustment` which
    /// take `&[(u32, f64)]` tuples instead of command structs.
    async fn run_command_executor(
        protocol: Arc<RwLock<Box<dyn ChannelRuntime>>>,
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
                    // ChannelRuntime uses (u32, f64) tuples
                    match protocol_guard.write_control(&[(point_id, value)]).await {
                        Ok(success_count) => {
                            if success_count > 0 {
                                debug!("Ch{} control pt{} = {} ok", channel_id, point_id, value);
                            } else {
                                warn!("Ch{} control pt{} = {} failed", channel_id, point_id, value);
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
                    // ChannelRuntime uses (u32, f64) tuples
                    match protocol_guard.write_adjustment(&[(point_id, value)]).await {
                        Ok(success_count) => {
                            if success_count > 0 {
                                debug!("Ch{} adjustment pt{} = {} ok", channel_id, point_id, value);
                            } else {
                                warn!(
                                    "Ch{} adjustment pt{} = {} failed",
                                    channel_id, point_id, value
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

/// Run the polling task for all channels.
///
/// Periodically calls poll_once() to retrieve data and write to store.
/// The polling interval is configurable via `poll_interval_ms`.
async fn run_polling_task<R: Rtdb>(
    protocol: Arc<RwLock<Box<dyn ChannelRuntime>>>,
    store: Arc<RedisDataStore<R>>,
    channel_id: u32,
    poll_interval_ms: u64,
) {
    info!(
        "Ch{} polling task started (interval: {}ms)",
        channel_id, poll_interval_ms
    );

    // Wait a bit for the connection to be established
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Use configured poll interval
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(poll_interval_ms));

    // Track previous error count to detect new errors
    let mut prev_error_count: u64 = 0;

    loop {
        interval.tick().await;

        // Poll data using ChannelRuntime interface
        let mut protocol_guard = protocol.write().await;
        let result: PollResult = protocol_guard.poll_once().await;

        // Log partial failures from poll result (before moving data)
        let failure_count = result.failures.len();
        if failure_count > 0 {
            warn!(
                "Ch{} partial read failure: {} points failed",
                channel_id, failure_count
            );
        }

        let count = result.data.len();
        if count > 0 {
            debug!("Ch{} polling got {} data points", channel_id, count);
            if let Err(e) = store.write_batch(channel_id, result.data).await {
                error!("Ch{} failed to write to Redis: {}", channel_id, e);
            }
        }

        // Check diagnostics for accumulated errors
        if let Ok(diag) = protocol_guard.diagnostics().await {
            if diag.error_count > prev_error_count {
                let new_errors = diag.error_count - prev_error_count;
                warn!(
                    "Ch{} accumulated errors: {} new errors, last error: {:?}",
                    channel_id, new_errors, diag.last_error
                );
                prev_error_count = diag.error_count;
            }
        }
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
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id,
/// avoiding collisions when different types share the same original point_id.
pub fn convert_to_igw_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<PointConfig> {
    let mut configs = Vec::new();

    // Convert telemetry points with scale/offset transformation
    for pt in &runtime_config.telemetry_points {
        let internal_id = PointType::Telemetry.to_internal_id(pt.base.point_id);
        configs.push(
            PointConfig::new(
                internal_id,
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
        let internal_id = PointType::Signal.to_internal_id(pt.base.point_id);
        configs.push(
            PointConfig::new(
                internal_id,
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
        let internal_id = PointType::Control.to_internal_id(pt.base.point_id);
        configs.push(
            PointConfig::new(
                internal_id,
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
        let internal_id = PointType::Adjustment.to_internal_id(pt.base.point_id);
        configs.push(
            PointConfig::new(
                internal_id,
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
/// Extracts Modbus mapping information from each point's embedded protocol_mappings JSON field.
/// This replaces the old approach of using separate modbus_mappings collection.
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id.
pub fn convert_to_modbus_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<PointConfig> {
    let mut configs = Vec::new();

    // Helper to parse modbus config from protocol_mappings JSON
    // Returns: (slave_id, function_code, register, data_type, byte_order, bit_position)
    fn parse_modbus_mapping(
        json_str: &str,
        point_id: u32,
    ) -> Option<(u8, u8, u16, String, String, Option<u8>)> {
        let v: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "Point {} has invalid protocol_mappings JSON: {}",
                    point_id, e
                );
                return None;
            },
        };

        if !v.is_object() {
            warn!(
                "Point {} has invalid protocol_mappings (expected JSON object): {}",
                point_id, v
            );
            return None;
        }

        fn parse_u64_field(v: &serde_json::Value, key: &str) -> Option<u64> {
            let raw = v.get(key)?;
            raw.as_u64()
                .or_else(|| raw.as_i64().and_then(|n| u64::try_from(n).ok()))
                .or_else(|| raw.as_str().and_then(|s| s.parse::<u64>().ok()))
        }

        let slave_id: u8 = match parse_u64_field(&v, "slave_id").and_then(|n| u8::try_from(n).ok())
        {
            Some(n) => n,
            None => {
                warn!(
                    "Point {} protocol_mappings missing/invalid 'slave_id': {}",
                    point_id, v
                );
                return None;
            },
        };

        let function_code: u8 =
            match parse_u64_field(&v, "function_code").and_then(|n| u8::try_from(n).ok()) {
                Some(n) => n,
                None => {
                    warn!(
                        "Point {} protocol_mappings missing/invalid 'function_code': {}",
                        point_id, v
                    );
                    return None;
                },
            };

        let register: u16 =
            match parse_u64_field(&v, "register_address").and_then(|n| u16::try_from(n).ok()) {
                Some(n) => n,
                None => {
                    warn!(
                        "Point {} protocol_mappings missing/invalid 'register_address': {}",
                        point_id, v
                    );
                    return None;
                },
            };

        Some((
            slave_id,
            function_code,
            register,
            v.get("data_type")
                .and_then(|x| x.as_str())
                .unwrap_or("uint16")
                .to_string(),
            v.get("byte_order")
                .and_then(|x| x.as_str())
                .unwrap_or("ABCD")
                .to_string(),
            // bit_position: None means not set, Some(0) means bit 0
            parse_u64_field(&v, "bit_position").and_then(|n| u8::try_from(n).ok()),
        ))
    }

    // Process telemetry points
    for point in &runtime_config.telemetry_points {
        if let Some(ref mappings_json) = point.base.protocol_mappings {
            if let Some((
                slave_id,
                function_code,
                register,
                data_type_str,
                byte_order_str,
                bit_pos,
            )) = parse_modbus_mapping(mappings_json, point.base.point_id)
            {
                let internal_id = PointType::Telemetry.to_internal_id(point.base.point_id);
                let modbus_addr = ModbusAddress {
                    slave_id,
                    function_code,
                    register,
                    format: parse_data_format(&data_type_str),
                    byte_order: parse_byte_order(&byte_order_str),
                    bit_position: bit_pos,
                };
                let transform = TransformConfig {
                    scale: point.scale,
                    offset: point.offset,
                    reverse: point.reverse,
                    ..Default::default()
                };
                let config = PointConfig::new(internal_id, ProtocolAddress::Modbus(modbus_addr))
                    .with_transform(transform);
                configs.push(config);
            }
        }
    }

    // Process signal points
    for point in &runtime_config.signal_points {
        if let Some(ref mappings_json) = point.base.protocol_mappings {
            if let Some((
                slave_id,
                function_code,
                register,
                data_type_str,
                byte_order_str,
                bit_pos,
            )) = parse_modbus_mapping(mappings_json, point.base.point_id)
            {
                let internal_id = PointType::Signal.to_internal_id(point.base.point_id);
                let modbus_addr = ModbusAddress {
                    slave_id,
                    function_code,
                    register,
                    format: parse_data_format(&data_type_str),
                    byte_order: parse_byte_order(&byte_order_str),
                    bit_position: bit_pos,
                };
                let transform = TransformConfig {
                    reverse: point.reverse,
                    ..Default::default()
                };
                let config = PointConfig::new(internal_id, ProtocolAddress::Modbus(modbus_addr))
                    .with_transform(transform);
                configs.push(config);
            }
        }
    }

    // Process control points
    for point in &runtime_config.control_points {
        if let Some(ref mappings_json) = point.base.protocol_mappings {
            if let Some((
                slave_id,
                function_code,
                register,
                data_type_str,
                byte_order_str,
                bit_pos,
            )) = parse_modbus_mapping(mappings_json, point.base.point_id)
            {
                let internal_id = PointType::Control.to_internal_id(point.base.point_id);
                let modbus_addr = ModbusAddress {
                    slave_id,
                    function_code,
                    register,
                    format: parse_data_format(&data_type_str),
                    byte_order: parse_byte_order(&byte_order_str),
                    bit_position: bit_pos,
                };
                let transform = TransformConfig {
                    reverse: point.reverse,
                    ..Default::default()
                };
                let config = PointConfig::new(internal_id, ProtocolAddress::Modbus(modbus_addr))
                    .with_transform(transform);
                configs.push(config);
            }
        }
    }

    // Process adjustment points
    for point in &runtime_config.adjustment_points {
        if let Some(ref mappings_json) = point.base.protocol_mappings {
            if let Some((
                slave_id,
                function_code,
                register,
                data_type_str,
                byte_order_str,
                bit_pos,
            )) = parse_modbus_mapping(mappings_json, point.base.point_id)
            {
                let internal_id = PointType::Adjustment.to_internal_id(point.base.point_id);
                let modbus_addr = ModbusAddress {
                    slave_id,
                    function_code,
                    register,
                    format: parse_data_format(&data_type_str),
                    byte_order: parse_byte_order(&byte_order_str),
                    bit_position: bit_pos,
                };
                let transform = TransformConfig {
                    scale: point.scale,
                    offset: point.offset,
                    ..Default::default()
                };
                let config = PointConfig::new(internal_id, ProtocolAddress::Modbus(modbus_addr))
                    .with_transform(transform);
                configs.push(config);
            }
        }
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

/// Create an IGW VirtualChannel wrapped as ChannelRuntime.
///
/// Note: The channel no longer holds a store reference. Storage is handled
/// by the service layer (ChannelManager) after polling.
pub fn create_virtual_channel(
    channel_id: u32,
    channel_name: &str,
    point_configs: Vec<PointConfig>,
) -> Box<dyn ChannelRuntime> {
    let config = VirtualChannelConfig::new(channel_name).with_points(point_configs);
    let channel = VirtualChannel::new(config);

    Box::new(VirtualRuntime::new(
        channel_id,
        channel_name.to_string(),
        channel,
    ))
}

/// Create an IGW ModbusChannel for TCP mode wrapped as ChannelRuntime.
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
) -> Box<dyn ChannelRuntime> {
    use igw::core::logging::{ChannelLogConfig, LoggableProtocol, TracingLogHandler};

    let address = format!("{}:{}", host, port);

    let config = ModbusChannelConfig::tcp(&address)
        .with_points(point_configs)
        .with_reconnect(ReconnectConfig::default());

    let mut channel = ModbusChannel::new(config, channel_id);
    // Enable tracing logs before wrapping
    channel.set_log_handler(Arc::new(TracingLogHandler));
    channel.set_log_config(ChannelLogConfig::default());

    Box::new(ModbusRuntime::new(
        channel_id,
        format!("modbus_tcp_{}", channel_id),
        channel,
    ))
}

/// Create an IGW ModbusChannel for RTU (serial) mode wrapped as ChannelRuntime.
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
) -> Box<dyn ChannelRuntime> {
    use igw::core::logging::{ChannelLogConfig, LoggableProtocol, TracingLogHandler};

    let config = ModbusChannelConfig::rtu(device, baud_rate)
        .with_points(point_configs)
        .with_reconnect(ReconnectConfig::default());

    let mut channel = ModbusChannel::new(config, channel_id);
    // Enable tracing logs before wrapping
    channel.set_log_handler(Arc::new(TracingLogHandler));
    channel.set_log_config(ChannelLogConfig::default());

    Box::new(ModbusRuntime::new(
        channel_id,
        format!("modbus_rtu_{}", channel_id),
        channel,
    ))
}

/// Create an IGW GpioChannel for digital I/O wrapped as ChannelRuntime.
///
/// Note: Only available on Linux with `gpio` feature enabled.
/// Storage is handled by the service layer (ChannelManager) after polling.
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id.
/// This is critical for GPIO where Signal (DI) and Control (DO) often share
/// the same original point_id range (e.g., 1-8).
///
/// # Arguments
///
/// * `channel_id` - Unique channel identifier
/// * `runtime_config` - Channel configuration containing GPIO pin mappings
#[cfg(all(target_os = "linux", feature = "gpio"))]
pub fn create_gpio_channel(
    channel_id: u32,
    runtime_config: &RuntimeChannelConfig,
) -> Box<dyn ChannelRuntime> {
    use std::time::Duration;

    // Use sysfs driver - simpler and works directly with global GPIO numbers
    let mut gpio_config = GpioChannelConfig::new_sysfs("/sys/class/gpio");

    // Get poll interval from parameters
    if let Some(interval_ms) = runtime_config
        .base
        .parameters
        .get("poll_interval_ms")
        .and_then(|v| v.as_u64())
    {
        gpio_config = gpio_config.with_poll_interval(Duration::from_millis(interval_ms));
    }

    // Helper to parse gpio_number from protocol_mappings JSON
    // Expected format: {"gpio_number": 496, ...}
    let parse_gpio_number = |protocol_mappings: &Option<String>| -> Option<u32> {
        let json_str = protocol_mappings.as_ref()?;
        let json: serde_json::Value = serde_json::from_str(json_str).ok()?;
        json.get("gpio_number")?.as_u64().map(|n| n as u32)
    };

    // Configure DI pins from signal points (using sysfs with global GPIO numbers)
    // Use internal_id to avoid collision with control points
    for pt in &runtime_config.signal_points {
        if let Some(gpio_num) = parse_gpio_number(&pt.base.protocol_mappings) {
            let internal_id = PointType::Signal.to_internal_id(pt.base.point_id);
            let pin_config = GpioPinConfig::digital_input_sysfs(gpio_num, internal_id)
                .with_active_low(pt.reverse);

            gpio_config = gpio_config.add_pin(pin_config);
        }
    }

    // Configure DO pins from control points (using sysfs with global GPIO numbers)
    // Use internal_id to avoid collision with signal points
    for pt in &runtime_config.control_points {
        if let Some(gpio_num) = parse_gpio_number(&pt.base.protocol_mappings) {
            let internal_id = PointType::Control.to_internal_id(pt.base.point_id);
            let pin_config = GpioPinConfig::digital_output_sysfs(gpio_num, internal_id)
                .with_active_low(pt.reverse);

            gpio_config = gpio_config.add_pin(pin_config);
        }
    }

    let channel = GpioChannel::new(gpio_config);
    Box::new(GpioRuntime::new(
        channel_id,
        format!("gpio_{}", channel_id),
        channel,
    ))
}

// ============================================================================
// ChannelImpl - Unified IGW-based channel implementation
// ============================================================================

/// Channel implementation wrapping IGW ChannelRuntime.
///
/// All protocols (Modbus TCP/RTU, Virtual, GPIO, CAN) use IGW's ChannelRuntime trait.
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

/// Drop implementation for defensive cleanup.
///
/// Ensures background tasks are aborted even if the wrapper is dropped
/// without an explicit call to `disconnect()`. This prevents task leaks
/// in edge cases where the channel is dropped unexpectedly.
impl<R: Rtdb> Drop for IgwChannelWrapper<R> {
    fn drop(&mut self) {
        if self.executor_handle.is_some() || self.polling_handle.is_some() {
            warn!(
                "Ch{} IgwChannelWrapper dropped without explicit cleanup, aborting tasks",
                self.channel_id
            );
            self.shutdown();
        }
    }
}

// ============================================================================
// CAN Channel Creation
// ============================================================================

/// CAN protocol mapping from protocol_mappings JSON field
#[cfg(all(feature = "can", target_os = "linux"))]
#[derive(Debug, Clone, serde::Deserialize)]
struct CanProtocolMapping {
    can_id: u32,
    start_bit: u32,
    bit_length: u32,
    #[serde(default = "default_can_data_type")]
    data_type: String,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default)]
    offset: f64,
}

#[cfg(all(feature = "can", target_os = "linux"))]
fn default_can_data_type() -> String {
    "uint16".to_string()
}

#[cfg(all(feature = "can", target_os = "linux"))]
fn default_scale() -> f64 {
    1.0
}

#[cfg(all(feature = "can", target_os = "linux"))]
/// Convert RuntimeChannelConfig to IGW CanPoint list for CAN protocol.
///
/// Parses CAN configuration from each point's protocol_mappings JSON field.
/// Scale and offset are applied during decoding in the protocol layer.
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id.
pub fn convert_to_can_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<CanPoint> {
    let mut configs = Vec::new();

    // Helper to parse protocol_mappings JSON and create CanPoint
    let parse_can_point =
        |internal_id: u32, protocol_mappings: &Option<String>| -> Option<CanPoint> {
            let json_str = protocol_mappings.as_ref()?;
            let mapping: CanProtocolMapping = serde_json::from_str(json_str)
                .map_err(|e| {
                    tracing::warn!(
                        internal_id,
                        error = %e,
                        "Failed to parse CAN protocol_mappings JSON"
                    );
                    e
                })
                .ok()?;

            Some(CanPoint {
                point_id: internal_id,
                can_id: mapping.can_id,
                byte_offset: (mapping.start_bit / 8) as u8,
                bit_position: (mapping.start_bit % 8) as u8,
                bit_length: mapping.bit_length as u8,
                data_type: mapping.data_type,
                scale: mapping.scale,
                offset: mapping.offset,
            })
        };

    // Collect from all point types with internal_id encoding
    for pt in &runtime_config.telemetry_points {
        let internal_id = PointType::Telemetry.to_internal_id(pt.base.point_id);
        if let Some(can_point) = parse_can_point(internal_id, &pt.base.protocol_mappings) {
            configs.push(can_point);
        }
    }
    for pt in &runtime_config.signal_points {
        let internal_id = PointType::Signal.to_internal_id(pt.base.point_id);
        if let Some(can_point) = parse_can_point(internal_id, &pt.base.protocol_mappings) {
            configs.push(can_point);
        }
    }
    for pt in &runtime_config.control_points {
        let internal_id = PointType::Control.to_internal_id(pt.base.point_id);
        if let Some(can_point) = parse_can_point(internal_id, &pt.base.protocol_mappings) {
            configs.push(can_point);
        }
    }
    for pt in &runtime_config.adjustment_points {
        let internal_id = PointType::Adjustment.to_internal_id(pt.base.point_id);
        if let Some(can_point) = parse_can_point(internal_id, &pt.base.protocol_mappings) {
            configs.push(can_point);
        }
    }

    configs
}

#[cfg(all(feature = "can", target_os = "linux"))]
/// Convert runtime CAN mappings to IGW PointConfig format (for RedisDataStore).
///
/// This conversion is used to register points with the data store for proper
/// data transformation and storage.
/// Parses CAN configuration from each point's protocol_mappings JSON field.
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id.
pub fn convert_can_to_igw_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<PointConfig> {
    use igw::core::point::ProtocolAddress;

    let mut configs = Vec::new();

    // Helper to build protocol address from CAN mapping
    let build_protocol_addr = |protocol_mappings: &Option<String>| -> Option<ProtocolAddress> {
        let json_str = protocol_mappings.as_ref()?;
        let mapping: CanProtocolMapping = serde_json::from_str(json_str).ok()?;
        Some(ProtocolAddress::Generic(format!(
            "can_id:0x{:X},start_bit:{},len:{}",
            mapping.can_id, mapping.start_bit, mapping.bit_length
        )))
    };

    // Telemetry points
    for pt in &runtime_config.telemetry_points {
        if let Some(protocol_addr) = build_protocol_addr(&pt.base.protocol_mappings) {
            let internal_id = PointType::Telemetry.to_internal_id(pt.base.point_id);
            let transform = TransformConfig {
                scale: pt.scale,
                offset: pt.offset,
                reverse: pt.reverse,
                ..Default::default()
            };
            let config = PointConfig::new(internal_id, protocol_addr).with_transform(transform);
            configs.push(config);
        }
    }

    // Signal points
    for pt in &runtime_config.signal_points {
        if let Some(protocol_addr) = build_protocol_addr(&pt.base.protocol_mappings) {
            let internal_id = PointType::Signal.to_internal_id(pt.base.point_id);
            let transform = TransformConfig {
                reverse: pt.reverse,
                ..Default::default()
            };
            let config = PointConfig::new(internal_id, protocol_addr).with_transform(transform);
            configs.push(config);
        }
    }

    // Control points
    for pt in &runtime_config.control_points {
        if let Some(protocol_addr) = build_protocol_addr(&pt.base.protocol_mappings) {
            let internal_id = PointType::Control.to_internal_id(pt.base.point_id);
            let transform = TransformConfig {
                reverse: pt.reverse,
                ..Default::default()
            };
            let config = PointConfig::new(internal_id, protocol_addr).with_transform(transform);
            configs.push(config);
        }
    }

    // Adjustment points
    for pt in &runtime_config.adjustment_points {
        if let Some(protocol_addr) = build_protocol_addr(&pt.base.protocol_mappings) {
            let internal_id = PointType::Adjustment.to_internal_id(pt.base.point_id);
            let transform = TransformConfig {
                scale: pt.scale,
                offset: pt.offset,
                ..Default::default()
            };
            let config = PointConfig::new(internal_id, protocol_addr).with_transform(transform);
            configs.push(config);
        }
    }

    configs
}

#[cfg(all(feature = "can", target_os = "linux"))]
/// Create an IGW CAN channel with the given configuration wrapped as ChannelRuntime.
///
/// This function creates a CanClient from igw library with the specified
/// CAN interface and point configurations.
pub fn create_can_channel(
    channel_id: u32,
    can_interface: &str,
    points: Vec<CanPoint>,
) -> Box<dyn ChannelRuntime> {
    let config = CanConfig {
        can_interface: can_interface.to_string(),
        bitrate: 250000,
        rx_poll_interval_ms: 50,
        data_read_interval_ms: 1000,
    };

    let mut client = CanClient::new(config);
    client.add_points(points);

    Box::new(CanRuntime::new(
        channel_id,
        format!("can_{}", channel_id),
        client,
    ))
}

// ============================================================================
// Unit Tests
// ============================================================================

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
        use voltage_model::PointType;

        let runtime_config = create_test_runtime_config();
        let configs = convert_to_igw_point_configs(&runtime_config);

        assert_eq!(configs.len(), 4);

        // Check telemetry point - now uses internal_id
        let telemetry_internal = PointType::Telemetry.to_internal_id(10);
        let telemetry = configs.iter().find(|c| c.id == telemetry_internal).unwrap();
        assert_eq!(telemetry.name, Some("temperature".to_string()));

        // Check signal point exists with internal_id
        let signal_internal = PointType::Signal.to_internal_id(20);
        assert!(configs.iter().any(|c| c.id == signal_internal));

        // Check control point exists with internal_id
        let control_internal = PointType::Control.to_internal_id(30);
        assert!(configs.iter().any(|c| c.id == control_internal));

        // Check adjustment point exists with internal_id
        let adjustment_internal = PointType::Adjustment.to_internal_id(40);
        assert!(configs.iter().any(|c| c.id == adjustment_internal));
    }

    #[test]
    fn test_convert_to_modbus_point_configs() {
        // Create a runtime config with embedded protocol_mappings
        let base_config = ChannelConfig {
            core: ChannelCore {
                id: 1,
                name: "test_modbus".to_string(),
                description: None,
                protocol: "modbus_tcp".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: Default::default(),
        };
        let mut runtime_config = RuntimeChannelConfig::from_base(base_config);

        // Add telemetry point with embedded Modbus mapping
        runtime_config.telemetry_points.push(TelemetryPoint {
            base: Point {
                point_id: 100,
                signal_name: "voltage".to_string(),
                description: None,
                unit: Some("V".to_string()),
                protocol_mappings: Some(r#"{"slave_id":1,"function_code":3,"register_address":0,"data_type":"float32","byte_order":"ABCD"}"#.to_string()),
            },
            scale: 1.0,
            offset: 0.0,
            data_type: "float32".to_string(),
            reverse: false,
        });

        // Add signal point with embedded Modbus mapping (with bit_position)
        runtime_config.signal_points.push(SignalPoint {
            base: Point {
                point_id: 101,
                signal_name: "status".to_string(),
                description: None,
                unit: None,
                protocol_mappings: Some(r#"{"slave_id":1,"function_code":1,"register_address":10,"data_type":"bool","byte_order":"ABCD","bit_position":5}"#.to_string()),
            },
            reverse: false,
        });

        use voltage_model::PointType;

        let configs = convert_to_modbus_point_configs(&runtime_config);

        assert_eq!(configs.len(), 2);

        // Check first point (telemetry, float32) - now uses internal_id encoding
        let telemetry_internal = PointType::Telemetry.to_internal_id(100);
        let pt1 = configs.iter().find(|c| c.id == telemetry_internal).unwrap();
        if let ProtocolAddress::Modbus(addr) = &pt1.address {
            assert_eq!(addr.slave_id, 1);
            assert_eq!(addr.function_code, 3);
            assert_eq!(addr.register, 0);
            assert_eq!(addr.format, DataFormat::Float32);
            assert_eq!(addr.byte_order, ByteOrder::Abcd);
        } else {
            panic!("Expected ModbusAddress");
        }

        // Check second point (signal, bool with bit_position) - now uses internal_id encoding
        let signal_internal = PointType::Signal.to_internal_id(101);
        let pt2 = configs.iter().find(|c| c.id == signal_internal).unwrap();
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
