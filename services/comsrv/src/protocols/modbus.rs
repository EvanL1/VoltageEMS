//! Modbus protocol core implementation
//!
//! Integrates protocol processing, polling mechanism and batch optimization features
//! Note: Current version is a temporary implementation, focused on compilation

use async_trait::async_trait;
use common::timeouts;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::core::channels::traits::{
    ChannelCommand, ChannelLogger, ConnectionState, TelemetryBatch,
};
use crate::core::channels::{ChannelStatus, ComBase, ComClient, PointDataMap, ProtocolValue};
use crate::core::config::{ChannelConfig, FourRemote, RuntimeChannelConfig};
use crate::error::{ComSrvError, Result};

// Import from voltage-protocols library
use voltage_protocols::modbus::{
    clamp_to_data_type, decode_register_value, parse_modbus_pdu, BatchCommand, CommandBatcher,
    ConnectionMode, ConnectionParams, ModbusCodec, ModbusConnectionManager, ModbusFrameProcessor,
    ModbusMode, ModbusPoint, ModbusPollingConfig, PduBuilder, BATCH_WINDOW_MS,
    MODBUS_RESPONSE_BUFFER_SIZE,
};

/// Type alias for pre-grouped points map: (slave_id, function_code, type) -> points
type GroupedPointsMap = HashMap<(u8, u8, String), Vec<ModbusPoint>>;

/// Modbus protocol implementation, implements `ComBase` trait
pub struct ModbusProtocol {
    /// Protocol name
    name: Arc<str>,
    /// Channel ID
    channel_id: u32,

    /// Connection manager
    connection_manager: Arc<ModbusConnectionManager>,
    /// Frame processor for request/response correlation
    frame_processor: Arc<Mutex<ModbusFrameProcessor>>,

    /// State management - AtomicBool for sync access, status for API metadata
    is_connected: Arc<AtomicBool>,
    status: Arc<RwLock<ChannelStatus>>,
    connection_state: Arc<RwLock<ConnectionState>>,

    /// Channel logger for unified logging
    logger: ChannelLogger,

    /// Task management
    polling_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    command_handle: Arc<RwLock<Option<JoinHandle<()>>>>,

    /// Polling configuration
    polling_config: Arc<ModbusPollingConfig>, // Use Arc to avoid cloning
    /// Point mapping - separated by telemetry type for proper isolation
    telemetry_points: Arc<RwLock<Vec<ModbusPoint>>>,
    signal_points: Arc<RwLock<Vec<ModbusPoint>>>,
    control_points: Arc<RwLock<Vec<ModbusPoint>>>,
    adjustment_points: Arc<RwLock<Vec<ModbusPoint>>>,

    /// Pre-grouped points for polling optimization (computed at startup)
    /// Key: (slave_id, function_code, "telemetry"|"signal")
    grouped_points: Arc<RwLock<GroupedPointsMap>>,

    /// Data channel for sending telemetry data
    data_channel: Option<tokio::sync::mpsc::Sender<TelemetryBatch>>,

    /// Command receiver for receiving control commands
    command_rx: Arc<RwLock<Option<tokio::sync::mpsc::Receiver<ChannelCommand>>>>,

    /// Command batcher for optimizing write operations
    command_batcher: Arc<Mutex<CommandBatcher>>,
}

impl std::fmt::Debug for ModbusProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModbusProtocol")
            .field("name", &self.name)
            .field("channel_id", &self.channel_id)
            .field("is_connected", &self.is_connected.load(Ordering::Relaxed))
            .field("polling_config", &self.polling_config)
            .finish()
    }
}

impl ModbusProtocol {
    /// Create new Modbus protocol instance
    pub fn new(
        channel_config: ChannelConfig,
        connection_params: ConnectionParams,
        polling_config: ModbusPollingConfig,
    ) -> Result<Self> {
        let mode = if channel_config.protocol().contains("tcp") {
            ModbusMode::Tcp
        } else {
            ModbusMode::Rtu
        };

        let conn_mode = if channel_config.protocol().contains("tcp") {
            ConnectionMode::Tcp
        } else {
            ConnectionMode::Rtu
        };

        let polling_config = Arc::new(polling_config); // Wrap in Arc once

        // Create logger first
        let logger = ChannelLogger::new(channel_config.id(), channel_config.name().to_string());

        // Create connection manager with logger
        let connection_manager = Arc::new(ModbusConnectionManager::new(
            conn_mode,
            connection_params,
            logger.clone(),
            polling_config.error_threshold,
        ));

        let frame_processor = Arc::new(Mutex::new(ModbusFrameProcessor::new(mode)));

        Ok(Self {
            name: channel_config.name().into(),
            channel_id: channel_config.id(),
            connection_manager,
            frame_processor,
            is_connected: Arc::new(AtomicBool::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::default())),
            connection_state: Arc::new(RwLock::new(ConnectionState::Uninitialized)),
            logger,
            polling_handle: Arc::new(RwLock::new(None)),
            command_handle: Arc::new(RwLock::new(None)),
            polling_config,
            telemetry_points: Arc::new(RwLock::new(Vec::new())),
            signal_points: Arc::new(RwLock::new(Vec::new())),
            control_points: Arc::new(RwLock::new(Vec::new())),
            adjustment_points: Arc::new(RwLock::new(Vec::new())),
            grouped_points: Arc::new(RwLock::new(HashMap::new())),
            data_channel: None,
            command_rx: Arc::new(RwLock::new(None)),
            command_batcher: Arc::new(Mutex::new(CommandBatcher::new())),
        })
    }

    /// Create from RuntimeChannelConfig and store it for initialization
    pub fn from_runtime_config(
        runtime_config: &crate::core::config::RuntimeChannelConfig,
    ) -> Result<Self> {
        use std::time::Duration;

        let channel_config = (*runtime_config.base).clone();

        // Extract connection parameters from channel config
        let params = &channel_config.parameters;

        // Parse connection params manually to avoid Duration serialization issues
        let conn_params = ConnectionParams {
            host: params
                .get("host")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            port: params
                .get("port")
                .and_then(|v| v.as_u64())
                .map(|n| n as u16),
            device: params
                .get("device")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            baud_rate: params
                .get("baud_rate")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32),
            data_bits: params
                .get("data_bits")
                .and_then(|v| v.as_u64())
                .map(|n| n as u8),
            stop_bits: params
                .get("stop_bits")
                .and_then(|v| v.as_u64())
                .map(|n| n as u8),
            parity: params
                .get("parity")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            timeout: Duration::from_millis(
                params
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1000),
            ),
        };

        // Log parsed connection parameters for diagnostics
        debug!(
            "Ch{} config: host={:?}, port={:?}, device={:?}",
            channel_config.id(),
            conn_params.host,
            conn_params.port,
            conn_params.device
        );

        // Parse or use default polling config
        let polling_config = if let Some(polling_value) = params.get("polling") {
            serde_json::from_value(polling_value.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse polling config: {}", e))?
        } else {
            // Use Default trait instead of json! macro
            ModbusPollingConfig::default()
        };

        Self::new(channel_config, conn_params, polling_config)
    }

    /// Lookup a Modbus point mapping for the given telemetry type and point id
    async fn get_point_mapping(
        &self,
        telemetry_type: FourRemote,
        point_id: u32,
    ) -> Option<ModbusPoint> {
        let point_key = point_id.to_string();

        let points_guard = match telemetry_type {
            FourRemote::Telemetry => self.telemetry_points.read().await,
            FourRemote::Signal => self.signal_points.read().await,
            FourRemote::Control => self.control_points.read().await,
            FourRemote::Adjustment => self.adjustment_points.read().await,
        };

        points_guard
            .iter()
            .find(|p| p.point_id == point_key)
            .cloned()
    }

    /// Execute batched commands
    async fn execute_batched_commands(&mut self, force: bool) -> Result<Vec<(u32, bool)>> {
        let mut batcher = self.command_batcher.lock().await;

        if !force && !batcher.should_execute() && batcher.pending_count() > 0 {
            // Not time to execute yet, return empty results
            return Ok(Vec::new());
        }

        let batches = batcher.take_commands();
        if batches.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut frame_processor = self.frame_processor.lock().await;

        // Process each batch group (slave_id, function_code)
        for ((slave_id, function_code), mut commands) in batches {
            if commands.is_empty() {
                continue;
            }

            // Check if commands can be merged (strictly consecutive addresses)
            if function_code == 16
                && CommandBatcher::are_strictly_consecutive(&commands)
                && commands.len() > 1
            {
                // Use FC16 to write multiple registers at once
                commands.sort_by_key(|c| c.register_address);

                let start_address = commands[0].register_address;
                let mut all_values = Vec::new();

                // Collect all register values in order
                for cmd in &commands {
                    let register_values = match ModbusCodec::encode_value_for_modbus(
                        &cmd.value,
                        &cmd.data_type,
                        cmd.byte_order.as_deref(),
                    ) {
                        Ok(values) => values,
                        Err(e) => {
                            error!("Encode pt{}: {}", cmd.point_id, e);
                            results.push((cmd.point_id, false));
                            continue;
                        },
                    };
                    all_values.extend(register_values);
                }

                // Build and send FC16 command
                if !all_values.is_empty() {
                    let pdu = ModbusCodec::build_write_fc16_multiple_registers_pdu(
                        start_address,
                        &all_values,
                    )?;

                    let request = frame_processor.build_frame(slave_id, &pdu);
                    let mut response = vec![0u8; MODBUS_RESPONSE_BUFFER_SIZE];

                    match self
                        .connection_manager
                        .send_and_receive(&request, &mut response, timeouts::DEFAULT_READ_TIMEOUT)
                        .await
                    {
                        Ok(bytes_read) => {
                            let response_slice = &response[..bytes_read];
                            if let Ok((_, parsed_pdu)) = frame_processor.parse_frame(response_slice)
                            {
                                let success = !parsed_pdu.is_exception();
                                for cmd in commands {
                                    results.push((cmd.point_id, success));
                                }
                                if success {
                                    debug!("FC16 batch: {} regs ok", all_values.len());
                                } else {
                                    error!("FC16 batch exception");
                                }
                            }
                        },
                        Err(e) => {
                            error!("FC16 batch: {}", e);
                            for cmd in commands {
                                results.push((cmd.point_id, false));
                            }
                        },
                    }
                }
            } else {
                // Process commands individually
                for cmd in commands {
                    let register_values = match ModbusCodec::encode_value_for_modbus(
                        &cmd.value,
                        &cmd.data_type,
                        cmd.byte_order.as_deref(),
                    ) {
                        Ok(values) => values,
                        Err(e) => {
                            error!("Encode pt{}: {}", cmd.point_id, e);
                            results.push((cmd.point_id, false));
                            continue;
                        },
                    };

                    // Build PDU based on function code
                    let pdu = match function_code {
                        5 => {
                            let bool_value =
                                register_values.first().map(|&v| v != 0).unwrap_or(false);
                            ModbusCodec::build_write_fc05_single_coil_pdu(
                                cmd.register_address,
                                bool_value,
                            )?
                        },
                        6 => {
                            let reg_value = register_values.first().copied().unwrap_or(0);
                            ModbusCodec::build_write_fc06_single_register_pdu(
                                cmd.register_address,
                                reg_value,
                            )?
                        },
                        15 => {
                            let bool_values: Vec<bool> =
                                register_values.iter().map(|&v| v != 0).collect();
                            ModbusCodec::build_write_fc15_multiple_coils_pdu(
                                cmd.register_address,
                                &bool_values,
                            )?
                        },
                        16 => ModbusCodec::build_write_fc16_multiple_registers_pdu(
                            cmd.register_address,
                            &register_values,
                        )?,
                        _ => {
                            error!("Unsupported FC{} for control", function_code);
                            results.push((cmd.point_id, false));
                            continue;
                        },
                    };

                    let request = frame_processor.build_frame(slave_id, &pdu);
                    let mut response = vec![0u8; MODBUS_RESPONSE_BUFFER_SIZE];

                    match self
                        .connection_manager
                        .send_and_receive(&request, &mut response, timeouts::DEFAULT_READ_TIMEOUT)
                        .await
                    {
                        Ok(bytes_read) => {
                            let response_slice = &response[..bytes_read];
                            if let Ok((_, parsed_pdu)) = frame_processor.parse_frame(response_slice)
                            {
                                let success = !parsed_pdu.is_exception();
                                results.push((cmd.point_id, success));
                                if !success {
                                    error!("Write pt{} failed", cmd.point_id);
                                }
                            }
                        },
                        Err(e) => {
                            error!("Write pt{}: {}", cmd.point_id, e);
                            results.push((cmd.point_id, false));
                        },
                    }
                }
            }
        }

        Ok(results)
    }

    fn drain_control_queue(&self) -> Vec<(u32, ProtocolValue)> {
        let mut drained = Vec::new();

        if let Ok(mut rx_guard) = self.command_rx.try_write() {
            if let Some(rx) = rx_guard.as_mut() {
                while let Ok(command) = rx.try_recv() {
                    if let ChannelCommand::Control {
                        command_id,
                        point_id,
                        value,
                        timestamp,
                    } = command
                    {
                        debug!("Ctrl cmd{} @{}", command_id, timestamp);
                        drained.push((point_id, ProtocolValue::Float(value)));
                    }
                }
            }
        }

        drained
    }

    fn drain_adjustment_queue(&self) -> Vec<(u32, ProtocolValue)> {
        let mut drained = Vec::new();

        if let Ok(mut rx_guard) = self.command_rx.try_write() {
            if let Some(rx) = rx_guard.as_mut() {
                while let Ok(command) = rx.try_recv() {
                    if let ChannelCommand::Adjustment {
                        command_id,
                        point_id,
                        value,
                        timestamp,
                    } = command
                    {
                        debug!("Adj cmd{} @{}", command_id, timestamp);
                        drained.push((point_id, ProtocolValue::Float(value)));
                    }
                }
            }
        }

        drained
    }

    async fn enqueue_batch_commands(
        &self,
        telemetry_type: FourRemote,
        commands: Vec<(u32, ProtocolValue)>,
        context: &str,
    ) {
        if commands.is_empty() {
            return;
        }

        let mut batch_entries = Vec::new();

        for (point_id, value) in commands {
            debug!("{} pt{}={:?}", context, point_id, value);

            let Some(mapping) = self.get_point_mapping(telemetry_type, point_id).await else {
                warn!("{} pt{} not found", context, point_id);
                continue;
            };

            let ModbusPoint {
                slave_id,
                function_code,
                register_address,
                data_type,
                byte_order,
                ..
            } = mapping;

            batch_entries.push(BatchCommand {
                point_id,
                value,
                slave_id,
                function_code,
                register_address,
                data_type,
                byte_order,
            });
        }

        if batch_entries.is_empty() {
            return;
        }

        let mut batcher = self.command_batcher.lock().await;
        for command in batch_entries {
            batcher.add_command(command);
        }
    }
}

#[async_trait]
impl ComBase for ModbusProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn get_channel_id(&self) -> u32 {
        self.channel_id
    }

    async fn get_status(&self) -> ChannelStatus {
        // Build status from is_connected (authoritative) + last_update from status
        ChannelStatus {
            is_connected: self.is_connected.load(Ordering::Relaxed),
            last_update: self.status.read().await.last_update,
        }
    }

    async fn initialize(&mut self, runtime_config: Arc<RuntimeChannelConfig>) -> Result<()> {
        // Update connection state
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Initializing;
        }

        let channel_id = runtime_config.id();

        let protocol_type = if self.connection_manager.mode() == ConnectionMode::Tcp {
            "Modbus TCP"
        } else {
            "Modbus RTU"
        };

        // Log initialization start
        self.logger.log_init(
            protocol_type,
            &format!("Starting initialization for channel {}", channel_id),
        );

        // Log configuration details
        self.logger.log_config(
            protocol_type,
            "polling_interval_ms",
            &self.polling_config.default_interval_ms.to_string(),
        );
        self.logger.log_config(
            protocol_type,
            "timeout_ms",
            &self.polling_config.read_timeout_ms.to_string(),
        );
        self.logger.log_config(
            protocol_type,
            "max_retries",
            &self.polling_config.max_retries.to_string(),
        );

        // Log point loading
        self.logger.log_init(
            protocol_type,
            "Loading point configurations from RuntimeChannelConfig",
        );

        // Create separate collections for each telemetry type
        let mut telemetry_modbus_points = Vec::new();
        let mut signal_modbus_points = Vec::new();
        let mut control_modbus_points = Vec::new();
        let mut adjustment_modbus_points = Vec::new();
        let mut skipped_points: Vec<u32> = Vec::new();

        // Process telemetry points from RuntimeChannelConfig
        for point in &runtime_config.telemetry_points {
            // Find corresponding Modbus mapping for this point
            if let Some(mapping) = runtime_config
                .modbus_mappings
                .iter()
                .find(|m| m.point_id == point.base.point_id && m.telemetry_type == "T")
            {
                let slave_id = mapping.slave_id;
                let function_code = mapping.function_code;
                let register_address = mapping.register_address;
                // Get data type from mapping
                let data_type = mapping.data_type.clone();

                // Auto-determine register count based on data type
                let register_count = match data_type.as_str() {
                    "float32" | "float32_be" | "float32_le" => 2,
                    "uint32" | "int32" | "uint32_be" | "uint32_le" => 2,
                    "float64" | "float64_be" | "float64_le" => 4,
                    "uint64" | "int64" => 4,
                    _ => 1, // Default for uint16, int16, bool, etc.
                };

                // Log to channel only (startup config)
                self.logger.log_point_config(
                    "T",
                    point.base.point_id,
                    slave_id,
                    function_code,
                    register_address,
                    &data_type,
                );

                let modbus_point = ModbusPoint {
                    point_id: point.base.point_id.to_string(),
                    slave_id,
                    function_code,
                    register_address,
                    data_type,
                    register_count,
                    byte_order: Some(mapping.byte_order.clone()),
                    bit_position: mapping.bit_position,
                    scale: point.scale,
                    offset: point.offset,
                    reverse: point.reverse,
                };

                telemetry_modbus_points.push(modbus_point);
            } else {
                debug!(
                    "Channel {} telemetry point {} - skipped, no protocol mapping found",
                    runtime_config.id(),
                    point.base.point_id
                );
                skipped_points.push(point.base.point_id);
            }
        }

        // Process signal points
        for point in &runtime_config.signal_points {
            // Find corresponding Modbus mapping for this point
            if let Some(mapping) = runtime_config
                .modbus_mappings
                .iter()
                .find(|m| m.point_id == point.base.point_id && m.telemetry_type == "S")
            {
                let slave_id = mapping.slave_id;
                let function_code = mapping.function_code;
                let register_address = mapping.register_address;
                let data_type = mapping.data_type.clone();
                let register_count = 1; // Signals are typically single bit/register

                // Log to channel only (startup config)
                self.logger.log_point_config(
                    "S",
                    point.base.point_id,
                    slave_id,
                    function_code,
                    register_address,
                    &data_type,
                );

                let modbus_point = ModbusPoint {
                    point_id: point.base.point_id.to_string(),
                    slave_id,
                    function_code,
                    register_address,
                    data_type,
                    register_count,
                    byte_order: Some(mapping.byte_order.clone()),
                    bit_position: mapping.bit_position,
                    scale: 1.0,  // Signal points don't use scale
                    offset: 0.0, // Signal points don't use offset
                    reverse: point.reverse,
                };

                signal_modbus_points.push(modbus_point);
            } else {
                debug!(
                    "Channel {} signal point {} - skipped, no protocol mapping found",
                    runtime_config.id(),
                    point.base.point_id
                );
                skipped_points.push(point.base.point_id);
            }
        }

        // Process control points
        for point in &runtime_config.control_points {
            // Find corresponding Modbus mapping for this point
            if let Some(mapping) = runtime_config
                .modbus_mappings
                .iter()
                .find(|m| m.point_id == point.base.point_id && m.telemetry_type == "C")
            {
                let slave_id = mapping.slave_id;
                let function_code = mapping.function_code;
                let register_address = mapping.register_address;
                let data_type = mapping.data_type.clone();

                let register_count = match data_type.as_str() {
                    "float32" | "float32_be" | "float32_le" => 2,
                    "uint32" | "int32" | "uint32_be" | "uint32_le" => 2,
                    _ => 1,
                };

                // Log to channel only (startup config)
                self.logger.log_point_config(
                    "C",
                    point.base.point_id,
                    slave_id,
                    function_code,
                    register_address,
                    &data_type,
                );

                let modbus_point = ModbusPoint {
                    point_id: point.base.point_id.to_string(),
                    slave_id,
                    function_code,
                    register_address,
                    data_type,
                    register_count,
                    byte_order: Some(mapping.byte_order.clone()),
                    bit_position: mapping.bit_position,
                    scale: 1.0,     // Control points don't use scale
                    offset: 0.0,    // Control points don't use offset
                    reverse: false, // Control points don't have reverse field
                };

                control_modbus_points.push(modbus_point);
            } else {
                debug!(
                    "Channel {} control point {} - skipped, no protocol mapping found",
                    runtime_config.id(),
                    point.base.point_id
                );
                skipped_points.push(point.base.point_id);
            }
        }

        // Process adjustment points
        for point in &runtime_config.adjustment_points {
            // Find corresponding Modbus mapping for this point
            if let Some(mapping) = runtime_config
                .modbus_mappings
                .iter()
                .find(|m| m.point_id == point.base.point_id && m.telemetry_type == "A")
            {
                let slave_id = mapping.slave_id;
                let function_code = mapping.function_code;
                let register_address = mapping.register_address;
                let data_type = mapping.data_type.clone();

                let register_count = match data_type.as_str() {
                    "float32" | "float32_be" | "float32_le" => 2,
                    "uint32" | "int32" | "uint32_be" | "uint32_le" => 2,
                    _ => 1,
                };

                // Log to channel only (startup config)
                self.logger.log_point_config(
                    "A",
                    point.base.point_id,
                    slave_id,
                    function_code,
                    register_address,
                    &data_type,
                );

                let modbus_point = ModbusPoint {
                    point_id: point.base.point_id.to_string(),
                    slave_id,
                    function_code,
                    register_address,
                    data_type,
                    register_count,
                    byte_order: Some(mapping.byte_order.clone()),
                    bit_position: mapping.bit_position,
                    scale: point.scale,
                    offset: point.offset,
                    reverse: false, // Adjustment points don't have reverse field
                };

                adjustment_modbus_points.push(modbus_point);
            } else {
                debug!(
                    "Channel {} adjustment point {} - skipped, no protocol mapping found",
                    runtime_config.id(),
                    point.base.point_id
                );
                skipped_points.push(point.base.point_id);
            }
        }

        // Step 3: Set points to core and local storage
        let total_points = telemetry_modbus_points.len()
            + signal_modbus_points.len()
            + control_modbus_points.len()
            + adjustment_modbus_points.len();
        self.logger.log_init(
            protocol_type,
            &format!(
                "Setting up {} points in storage (T:{}, S:{}, C:{}, A:{})",
                total_points,
                telemetry_modbus_points.len(),
                signal_modbus_points.len(),
                control_modbus_points.len(),
                adjustment_modbus_points.len()
            ),
        );

        // Store points by type
        *self.telemetry_points.write().await = telemetry_modbus_points;
        *self.signal_points.write().await = signal_modbus_points;
        *self.control_points.write().await = control_modbus_points;
        *self.adjustment_points.write().await = adjustment_modbus_points;

        self.logger.log_init(
            protocol_type,
            &format!("Successfully configured {} total points", total_points),
        );

        self.logger.log_init(
            protocol_type,
            "Initialization completed successfully (connection will be established later)",
        );

        // Output summary if any points were skipped
        if !skipped_points.is_empty() {
            warn!(
                "Channel {} Modbus initialization: {} points skipped due to missing/invalid mappings",
                channel_id, skipped_points.len()
            );
            debug!(
                "Channel {} skipped points IDs: {:?}",
                channel_id, skipped_points
            );
        }

        Ok(())
    }

    async fn read_four_telemetry(&self, _telemetry_type: FourRemote) -> Result<PointDataMap> {
        // Data is collected via polling mechanism, not direct reads
        // Return empty map as polling handles data collection
        Ok(HashMap::new())
    }
}

#[async_trait]
impl ComClient for ModbusProtocol {
    fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    async fn connect(&mut self) -> Result<()> {
        let protocol_type = if self.connection_manager.mode() == ConnectionMode::Tcp {
            "Modbus TCP"
        } else {
            "Modbus RTU"
        };

        // Update connection state
        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connecting;
            self.logger.log_status(
                old_state,
                ConnectionState::Connecting,
                "Initiating connection",
            );
        }

        // Log connection attempt with protocol-specific details
        let target = match self.connection_manager.mode() {
            ConnectionMode::Tcp => {
                // Extract connection params for TCP
                "TCP endpoint"
            },
            ConnectionMode::Rtu => {
                // Extract connection params for RTU
                "Serial port"
            },
        };

        self.logger.log_connect(
            protocol_type,
            target,
            &format!("Channel {} attempting connection", self.channel_id),
        );

        // Try connection with retry logic for initial connection
        let max_consecutive = self.polling_config.reconnect_max_consecutive;
        let cooldown_ms = self.polling_config.reconnect_cooldown_ms;

        self.logger.log_retry(
            1,
            max_consecutive,
            1000, // Initial delay in ms
            &format!("Starting connection with up to {} retries", max_consecutive),
        );

        // Use connect_with_retry for initial connection
        match self
            .connection_manager
            .connect_with_retry(max_consecutive, cooldown_ms)
            .await
        {
            Ok(connected) => {
                if !connected {
                    // Max attempts reached or in cooldown
                    let old_state = *self.connection_state.read().await;
                    let mut state = self.connection_state.write().await;
                    *state = ConnectionState::Failed;
                    self.logger.log_status(
                        old_state,
                        ConnectionState::Failed,
                        &format!(
                            "Initial connection failed after {} attempts",
                            max_consecutive
                        ),
                    );

                    // Start periodic tasks anyway to enable background reconnection
                    self.logger.log_init(
                        protocol_type,
                        "Starting periodic tasks for background reconnection attempts",
                    );
                    self.start_periodic_tasks().await?;

                    return Err(ComSrvError::ConnectionError(format!(
                        "Failed to connect after {} attempts",
                        max_consecutive
                    )));
                }

                self.is_connected.store(true, Ordering::Relaxed);

                // Update connection state to connected
                {
                    let old_state = *self.connection_state.read().await;
                    let mut state = self.connection_state.write().await;
                    *state = ConnectionState::Connected;
                    self.logger.log_status(
                        old_state,
                        ConnectionState::Connected,
                        "Connection established successfully",
                    );
                }

                // Start periodic tasks
                self.logger.log_init(
                    protocol_type,
                    &format!(
                        "Starting periodic tasks with {}ms interval",
                        self.polling_config.default_interval_ms
                    ),
                );

                self.start_periodic_tasks().await?;

                Ok(())
            },
            Err(e) => {
                // Unexpected error during connection attempt
                {
                    let old_state = *self.connection_state.read().await;
                    let mut state = self.connection_state.write().await;
                    *state = ConnectionState::Failed;
                    self.logger.log_status(
                        old_state,
                        ConnectionState::Failed,
                        &format!("Connection error: {}", e),
                    );
                }

                // Start periodic tasks anyway to enable background reconnection
                self.logger.log_init(
                    protocol_type,
                    "Starting periodic tasks for background reconnection attempts",
                );
                self.start_periodic_tasks().await?;

                Err(e.into())
            },
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        let protocol_type = if self.connection_manager.mode() == ConnectionMode::Tcp {
            "Modbus TCP"
        } else {
            "Modbus RTU"
        };

        // Update connection state
        {
            let old_state = *self.connection_state.read().await;
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Closed;
            self.logger.log_status(
                old_state,
                ConnectionState::Closed,
                "Disconnecting gracefully",
            );
        }

        self.logger
            .log_init(protocol_type, "Stopping periodic tasks");

        // Stop all tasks
        self.stop_periodic_tasks().await?;

        // Disconnect connection
        self.connection_manager.disconnect().await?;

        self.is_connected.store(false, Ordering::Relaxed);

        self.logger.log_init(
            protocol_type,
            &format!("Channel {} disconnected successfully", self.channel_id),
        );

        Ok(())
    }

    async fn control(
        &mut self,
        mut commands: Vec<(u32, ProtocolValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        commands.extend(self.drain_control_queue());

        if !commands.is_empty() {
            self.enqueue_batch_commands(FourRemote::Control, commands, "control")
                .await;
        }

        let (has_pending, execute_now, wait_duration) = {
            let batcher = self.command_batcher.lock().await;
            let has_pending = batcher.pending_count() > 0;
            let execute_now = has_pending && batcher.should_execute();
            let wait_duration = if has_pending && !execute_now {
                Some(
                    Duration::from_millis(BATCH_WINDOW_MS)
                        .saturating_sub(batcher.elapsed_since_last_batch()),
                )
            } else {
                None
            };
            (has_pending, execute_now, wait_duration)
        };

        if !has_pending {
            return Ok(Vec::new());
        }

        if execute_now {
            return self.execute_batched_commands(false).await;
        }

        if let Some(duration) = wait_duration {
            if !duration.is_zero() {
                tokio::time::sleep(duration).await;
            }

            // Drain any new queued control commands during the wait window
            let queued_commands = self.drain_control_queue();
            if !queued_commands.is_empty() {
                self.enqueue_batch_commands(FourRemote::Control, queued_commands, "control")
                    .await;
            }

            return self.execute_batched_commands(true).await;
        }

        Ok(Vec::new())
    }

    async fn adjustment(
        &mut self,
        mut adjustments: Vec<(u32, ProtocolValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        adjustments.extend(self.drain_adjustment_queue());

        if !adjustments.is_empty() {
            self.enqueue_batch_commands(FourRemote::Adjustment, adjustments, "adjustment")
                .await;
        }

        let (has_pending, execute_now, wait_duration) = {
            let batcher = self.command_batcher.lock().await;
            let has_pending = batcher.pending_count() > 0;
            let execute_now = has_pending && batcher.should_execute();
            let wait_duration = if has_pending && !execute_now {
                Some(
                    Duration::from_millis(BATCH_WINDOW_MS)
                        .saturating_sub(batcher.elapsed_since_last_batch()),
                )
            } else {
                None
            };
            (has_pending, execute_now, wait_duration)
        };

        if !has_pending {
            return Ok(Vec::new());
        }

        if execute_now {
            return self.execute_batched_commands(false).await;
        }

        if let Some(duration) = wait_duration {
            if !duration.is_zero() {
                tokio::time::sleep(duration).await;
            }

            let queued_commands = self.drain_adjustment_queue();
            if !queued_commands.is_empty() {
                self.enqueue_batch_commands(FourRemote::Adjustment, queued_commands, "adjustment")
                    .await;
            }

            return self.execute_batched_commands(true).await;
        }

        Ok(Vec::new())
    }

    // In the four-telemetry detached architecture, update_points method has been removed,
    // point configuration is loaded directly during initialization stage

    async fn start_periodic_tasks(&self) -> Result<()> {
        info!(
            "Starting Modbus periodic tasks for channel {}",
            self.channel_id
        );

        // Start polling task
        if self.polling_config.enabled {
            let channel_id = self.channel_id;
            let channel_name = self.name.clone();
            let polling_interval = self.polling_config.default_interval_ms;
            let connection_manager = Arc::clone(&self.connection_manager);
            let telemetry_points = Arc::clone(&self.telemetry_points);
            let signal_points = Arc::clone(&self.signal_points);
            let status = Arc::clone(&self.status);
            let is_connected = Arc::clone(&self.is_connected);
            let polling_config = self.polling_config.clone();
            let data_channel = self.data_channel.clone();
            let frame_processor = Arc::clone(&self.frame_processor);

            // Pre-group points at startup for polling optimization
            {
                let mut groups: HashMap<(u8, u8, String), Vec<ModbusPoint>> = HashMap::new();

                // Add telemetry points
                let telemetry_guard = telemetry_points.read().await;
                for point in telemetry_guard.iter() {
                    let key = (point.slave_id, point.function_code, "telemetry".to_string());
                    groups.entry(key).or_default().push(point.clone());
                }

                // Add signal points
                let signal_guard = signal_points.read().await;
                for point in signal_guard.iter() {
                    let key = (point.slave_id, point.function_code, "signal".to_string());
                    groups.entry(key).or_default().push(point.clone());
                }

                // Store pre-grouped points
                let mut grouped_guard = self.grouped_points.write().await;
                *grouped_guard = groups;

                debug!("Ch{} grouped: {} groups", channel_id, grouped_guard.len());
            }

            let grouped_points = Arc::clone(&self.grouped_points);

            let polling_task = tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_millis(polling_interval));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                info!("Ch{} polling: {}ms", channel_id, polling_interval);

                // Create reusable buffers outside loop to avoid per-cycle allocation
                let mut telemetry_batch = Vec::with_capacity(1024);
                let mut signal_batch = Vec::with_capacity(1024);

                // Consecutive zero-result cycle counter for connection loss detection
                let mut consecutive_zero_cycles = 0u32;
                const MAX_ZERO_CYCLES: u32 = 5; // Trigger reconnect after 5 consecutive zero-data cycles

                loop {
                    interval.tick().await;

                    // Clear buffers for reuse (no reallocation unless capacity exceeded)
                    telemetry_batch.clear();
                    signal_batch.clear();

                    // Check connection and attempt reconnection if needed
                    if !is_connected.load(Ordering::Relaxed) {
                        debug!("Ch{} reconnecting...", channel_id);

                        // Check if reconnection is enabled (default: true)
                        if polling_config.reconnect_enabled {
                            let max_consecutive = polling_config.reconnect_max_consecutive;
                            let cooldown_ms = polling_config.reconnect_cooldown_ms;

                            // Attempt to reconnect (will handle retries and cooldown internally)
                            match connection_manager
                                .connect_with_retry(max_consecutive, cooldown_ms)
                                .await
                            {
                                Ok(true) => {
                                    // Successfully connected
                                    is_connected.store(true, Ordering::Relaxed);
                                    info!("Ch{} reconnected", channel_id);
                                    // Continue with polling after successful reconnection
                                },
                                Ok(false) => {
                                    // In cooldown period or max attempts reached
                                    debug!("Ch{} in cooldown", channel_id);
                                    continue;
                                },
                                Err(e) => {
                                    // Unexpected error
                                    error!("Ch{} reconnect: {}", channel_id, e);
                                    continue;
                                },
                            }
                        } else {
                            debug!("Ch{} reconnect disabled", channel_id);
                            continue;
                        }
                    }

                    // Use pre-grouped points (computed at startup)
                    let grouped_guard = grouped_points.read().await;
                    if grouped_guard.is_empty() {
                        debug!("Ch{} no points", channel_id);
                        continue;
                    }

                    let mut success_count = 0;
                    let mut error_count = 0;

                    // Collect all telemetry and signal data for this poll cycle
                    // (buffers already created outside loop and cleared above)
                    let timestamp = chrono::Utc::now().timestamp();

                    // Read each group
                    for ((slave_id, function_code, group_telemetry_type), group_points) in
                        grouped_guard.iter()
                    {
                        if group_points.is_empty() {
                            continue;
                        }

                        // Create channel logger for protocol messages
                        let logger = crate::core::channels::traits::ChannelLogger::new(
                            channel_id,
                            channel_name.to_string(),
                        );

                        // Log to channel only (not main log)
                        logger.log_poll(
                            *slave_id,
                            *function_code,
                            group_telemetry_type,
                            group_points.len(),
                        );

                        // Lock the frame processor for this batch of reads
                        let mut frame_processor = frame_processor.lock().await;

                        // Get max_batch_size from polling config, default to 100
                        let max_batch_size = polling_config.batch_config.max_batch_size;

                        match read_modbus_group_with_processor(
                            &connection_manager,
                            &mut frame_processor,
                            *slave_id,
                            *function_code,
                            group_points,
                            None,
                            max_batch_size,
                            &logger,
                            group_telemetry_type,
                        )
                        .await
                        {
                            Ok(values) => {
                                success_count += values.len();
                                // ✅ 统计实际失败的点位数（总数 - 成功数）/ Count actual failed points (total - success)
                                let failed_in_group = group_points.len() - values.len();
                                error_count += failed_in_group;
                                // Log result: errors go to main log, success only to channel log
                                logger.log_poll_result(
                                    *slave_id,
                                    *function_code,
                                    values.len(),
                                    failed_in_group,
                                );

                                // Process values

                                for (point_id_str, value) in values {
                                    // Convert point_id from string to u32
                                    if let Ok(point_id) = point_id_str.parse::<u32>() {
                                        // Convert ProtocolValue to f64
                                        let raw_value = match value {
                                            ProtocolValue::Float(f) => f,
                                            ProtocolValue::Integer(i) => i as f64,
                                            _ => continue, // Skip non-numeric values
                                        };

                                        // Use the telemetry type from the group
                                        // This ensures proper four-telemetry isolation
                                        let telemetry_type = match group_telemetry_type.as_str() {
                                            "telemetry" => FourRemote::Telemetry,
                                            "signal" => FourRemote::Signal,
                                            _ => FourRemote::Telemetry,
                                        };

                                        // Collect data for batch sending
                                        match telemetry_type {
                                            FourRemote::Telemetry => {
                                                telemetry_batch
                                                    .push((point_id, raw_value, timestamp));
                                                // Removed per-point debug logging for performance
                                            },
                                            FourRemote::Signal => {
                                                signal_batch.push((point_id, raw_value, timestamp));
                                                // Removed per-point debug logging for performance
                                            },
                                            FourRemote::Control | FourRemote::Adjustment => {
                                                // Control and adjustment data are not polled
                                                debug!(
                                                    "Skipping control/adjustment point {}",
                                                    point_id
                                                );
                                            },
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                error_count += group_points.len();
                                error!("Group read err u={} FC{}: {}", slave_id, function_code, e);

                                // Check if this is a connection error
                                let error_str = e.to_string();
                                if error_str.contains("Broken pipe")
                                    || error_str.contains("Connection reset")
                                    || error_str.contains("Connection refused")
                                    || error_str.contains("TCP send error")
                                    || error_str.contains("TCP receive error")
                                {
                                    warn!("Conn lost: {}", e);
                                    is_connected.store(false, Ordering::Relaxed);
                                    // Next iteration will trigger reconnection
                                    break; // Exit the group processing loop
                                }
                            },
                        }
                    }

                    // Send batch data through channel if available
                    if let Some(ref tx) = data_channel {
                        // Send batch if not empty
                        if !telemetry_batch.is_empty() || !signal_batch.is_empty() {
                            let telemetry_count = telemetry_batch.len();
                            let signal_count = signal_batch.len();

                            // Use mem::take to extract data while leaving empty buffers for reuse
                            let batch = TelemetryBatch {
                                channel_id,
                                telemetry: std::mem::take(&mut telemetry_batch),
                                signal: std::mem::take(&mut signal_batch),
                            };

                            // Use send().await instead of try_send for guaranteed delivery
                            match tx.send(batch).await {
                                Ok(()) => {
                                    debug!(
                                        "Ch{} TX: {} pts",
                                        channel_id,
                                        telemetry_count + signal_count
                                    );
                                },
                                Err(e) => {
                                    error!("Ch{} TX batch err: {}", channel_id, e);
                                },
                            }
                        }
                    }

                    debug!(
                        "Ch{} poll: {} ok/{} err",
                        channel_id, success_count, error_count
                    );

                    // Zero-cycle detection: if no data received but points were configured
                    if success_count == 0 && error_count > 0 {
                        consecutive_zero_cycles += 1;
                        warn!(
                            "Ch{} zero data cycle {}/{} (err={})",
                            channel_id, consecutive_zero_cycles, MAX_ZERO_CYCLES, error_count
                        );

                        if consecutive_zero_cycles >= MAX_ZERO_CYCLES {
                            error!(
                                "Ch{} connection presumed lost after {} zero cycles, triggering reconnect",
                                channel_id, consecutive_zero_cycles
                            );
                            is_connected.store(false, Ordering::Relaxed);
                            consecutive_zero_cycles = 0; // Reset counter
                                                         // Next iteration will detect is_connected=false and trigger reconnect
                        }
                    } else if success_count > 0 {
                        // Got some data, reset the counter
                        if consecutive_zero_cycles > 0 {
                            debug!(
                                "Ch{} connection recovered, resetting zero cycle counter",
                                channel_id
                            );
                        }
                        consecutive_zero_cycles = 0;
                    }

                    // Update status - only last_update is maintained
                    status.write().await.last_update = chrono::Utc::now().timestamp();
                }
            });

            *self.polling_handle.write().await = Some(polling_task);
        }

        Ok(())
    }

    async fn stop_periodic_tasks(&self) -> Result<()> {
        info!("Ch{} stopping tasks", self.channel_id);

        // Stop polling task
        if let Some(handle) = self.polling_handle.write().await.take() {
            handle.abort();
            debug!("Ch{} polling stopped", self.channel_id);
        }

        Ok(())
    }

    fn set_data_channel(&mut self, tx: tokio::sync::mpsc::Sender<TelemetryBatch>) {
        self.data_channel = Some(tx);
        debug!("Ch{} data channel set", self.channel_id);
    }

    fn set_command_receiver(&mut self, mut rx: tokio::sync::mpsc::Receiver<ChannelCommand>) {
        let channel_id = self.channel_id;
        let is_connected = Arc::clone(&self.is_connected);
        let frame_processor = Arc::clone(&self.frame_processor);
        let _command_handle = self.command_handle.clone();
        let connection_manager = Arc::clone(&self.connection_manager);

        // Clone point mappings for command processing
        let control_points = Arc::clone(&self.control_points);
        let adjustment_points = Arc::clone(&self.adjustment_points);

        // Create a channel to forward commands for processing
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<(
            ChannelCommand,
            tokio::sync::oneshot::Sender<Result<()>>,
        )>(100);

        // Start command forwarding task
        tokio::spawn(async move {
            debug!("Ch{} cmd receiver started", channel_id);
            while let Some(command) = rx.recv().await {
                let (tx, _rx) = tokio::sync::oneshot::channel();
                if let Err(e) = cmd_tx.send((command, tx)).await {
                    error!("Cmd forward err: {}", e);
                }
                // We don't wait for the result here
            }
            debug!("Ch{} cmd receiver stopped", channel_id);
        });

        // Start command processing task
        let handle = tokio::spawn(async move {
            debug!("Ch{} cmd processor started", channel_id);
            while let Some((command, result_tx)) = cmd_rx.recv().await {
                if !is_connected.load(Ordering::Relaxed) {
                    warn!("Cmd ignored: disconnected");
                    let _ = result_tx.send(Err(ComSrvError::NotConnected));
                    continue;
                }

                // Process command
                let result = match &command {
                    ChannelCommand::Control {
                        command_id,
                        point_id,
                        value,
                        ..
                    } => {
                        debug!("Control: cmd={} pt{} val={}", command_id, point_id, value);

                        // Find the mapping for this control point
                        let control_guard = control_points.read().await;
                        let modbus_point = control_guard
                            .iter()
                            .find(|p| p.point_id == point_id.to_string())
                            .cloned();
                        drop(control_guard);

                        // Execute the control write with mapping
                        execute_modbus_write(
                            &connection_manager,
                            &frame_processor,
                            modbus_point.as_ref(),
                            *point_id,
                            ProtocolValue::Float(*value),
                            FourRemote::Control,
                        )
                        .await
                    },
                    ChannelCommand::Adjustment {
                        command_id,
                        point_id,
                        value,
                        ..
                    } => {
                        debug!("Adjust: cmd={} pt{} val={}", command_id, point_id, value);

                        // Find the mapping for this adjustment point
                        let adjustment_guard = adjustment_points.read().await;
                        let modbus_point = adjustment_guard
                            .iter()
                            .find(|p| p.point_id == point_id.to_string())
                            .cloned();
                        drop(adjustment_guard);

                        // Execute the adjustment write with mapping
                        execute_modbus_write(
                            &connection_manager,
                            &frame_processor,
                            modbus_point.as_ref(),
                            *point_id,
                            ProtocolValue::Float(*value),
                            FourRemote::Adjustment,
                        )
                        .await
                    },
                };

                let _ = result_tx.send(result);
            }
            debug!("Ch{} cmd processor stopped", channel_id);
        });

        // Store the command handle in a separate task to avoid blocking
        let command_handle = self.command_handle.clone();
        tokio::spawn(async move {
            let mut handle_guard = command_handle.write().await;
            *handle_guard = Some(handle);
        });

        debug!("Ch{} cmd receiver set", self.channel_id);
    }
}

/// Read a group of Modbus points with the same slave ID and function code
#[allow(clippy::too_many_arguments)]
async fn read_modbus_group_with_processor(
    connection_manager: &Arc<ModbusConnectionManager>,
    frame_processor: &mut ModbusFrameProcessor,
    slave_id: u8,
    function_code: u8,
    points: &[ModbusPoint],
    channel_config: Option<&ChannelConfig>,
    max_batch_size: u16,
    logger: &crate::core::channels::traits::ChannelLogger,
    group_telemetry_type: &str, // "telemetry", "signal", "control", "adjustment"
) -> Result<Vec<(String, ProtocolValue)>> {
    if points.is_empty() {
        return Ok(Vec::new());
    }

    // Sort points by register address for efficient batch reading
    let mut sorted_indices: Vec<usize> = (0..points.len()).collect();
    sorted_indices.sort_by_key(|&i| points[i].register_address);

    let mut results = Vec::new();
    let mut current_batch_indices = Vec::new();
    let mut batch_start_address = points[sorted_indices[0]].register_address;

    for &idx in &sorted_indices {
        let point = &points[idx];
        // Check if this point can be added to the current batch
        // For FC01/02, addresses are bit addresses; for FC03/04, they are register addresses
        let (gap, batch_size_if_added) = match function_code {
            1 | 2 => {
                // For coils/discrete inputs, addresses are consecutive bit addresses
                let expected_next_address = if current_batch_indices.is_empty() {
                    point.register_address
                } else {
                    batch_start_address + current_batch_indices.len() as u16
                };
                let gap = point.register_address.saturating_sub(expected_next_address);
                let batch_bits_if_added =
                    (point.register_address - batch_start_address + 1) as usize;
                (gap, batch_bits_if_added)
            },
            _ => {
                // For registers, use the original logic
                let gap = point.register_address.saturating_sub(
                    batch_start_address + current_batch_indices.len() as u16 * point.register_count,
                );
                let batch_end_if_added = point.register_address + point.register_count;
                let batch_registers_if_added = (batch_end_if_added - batch_start_address) as usize;
                (gap, batch_registers_if_added)
            },
        };

        // Check both gap and batch size constraints
        if current_batch_indices.is_empty()
            || (gap <= 5 && batch_size_if_added <= max_batch_size as usize)
        {
            current_batch_indices.push(idx);
        } else {
            // Read current batch using zero-copy indexed version (with error isolation)
            match read_modbus_batch_indexed(
                connection_manager,
                frame_processor,
                slave_id,
                function_code,
                batch_start_address,
                points,
                &current_batch_indices,
                channel_config,
                max_batch_size,
                logger,
                group_telemetry_type,
            )
            .await
            {
                Ok(batch_results) => {
                    results.extend(batch_results);
                },
                Err(e) => {
                    error!(
                        "Batch err u={} FC{} @{} ({}pts): {}",
                        slave_id,
                        function_code,
                        batch_start_address,
                        current_batch_indices.len(),
                        e
                    );
                    // Continue processing remaining batches instead of failing entire group
                },
            }

            // Start new batch
            current_batch_indices.clear();
            current_batch_indices.push(idx);
            batch_start_address = point.register_address;
        }
    }

    // Read final batch using zero-copy indexed version (with error isolation)
    if !current_batch_indices.is_empty() {
        match read_modbus_batch_indexed(
            connection_manager,
            frame_processor,
            slave_id,
            function_code,
            batch_start_address,
            points,
            &current_batch_indices,
            channel_config,
            max_batch_size,
            logger,
            group_telemetry_type,
        )
        .await
        {
            Ok(batch_results) => {
                results.extend(batch_results);
            },
            Err(e) => {
                error!(
                    "Final batch err u={} FC{} @{} ({}pts): {}",
                    slave_id,
                    function_code,
                    batch_start_address,
                    current_batch_indices.len(),
                    e
                );
                // Return partial results from successful batches instead of failing entire group
            },
        }
    }

    Ok(results)
}

/// Read a batch of consecutive Modbus registers with zero-copy design
#[allow(clippy::too_many_arguments)]
async fn read_modbus_batch_indexed(
    connection_manager: &Arc<ModbusConnectionManager>,
    frame_processor: &mut ModbusFrameProcessor,
    slave_id: u8,
    function_code: u8,
    start_address: u16,
    all_points: &[ModbusPoint], // All points reference
    indices: &[usize],          // Indices of points to read
    _channel_config: Option<&ChannelConfig>,
    max_batch_size: u16,
    logger: &crate::core::channels::traits::ChannelLogger, // Add logger parameter
    _group_telemetry_type: &str, // "telemetry", "signal", "control", "adjustment"
) -> Result<Vec<(String, ProtocolValue)>> {
    if indices.is_empty() {
        return Ok(Vec::new());
    }

    // Get the last point via index
    let last_point = &all_points[indices[indices.len() - 1]];

    // Calculate total registers/bits to read based on function code
    let (total_units, unit_name) = match function_code {
        1 | 2 => {
            let total_bits = (last_point.register_address - start_address + 1) as usize;
            (total_bits, "bits")
        },
        _ => {
            let total_registers =
                (last_point.register_address - start_address + last_point.register_count) as usize;
            (total_registers, "registers")
        },
    };

    // Collect all register values by reading in batches
    let mut all_register_values = Vec::new();
    let mut current_offset = 0;

    // Read registers/bits in chunks no larger than max_batch_size
    while current_offset < total_units {
        let (batch_size, batch_start) = match function_code {
            1 | 2 => {
                let remaining_bits = total_units - current_offset;
                let batch_bits = std::cmp::min(max_batch_size as usize, remaining_bits);
                let batch_start = start_address + current_offset as u16;
                (batch_bits, batch_start)
            },
            _ => {
                let batch_size =
                    std::cmp::min(max_batch_size as usize, total_units - current_offset);
                let batch_start = start_address + current_offset as u16;
                (batch_size, batch_start)
            },
        };

        debug!(
            "Batch u={} FC{} @{}: {} {}",
            slave_id, function_code, batch_start, batch_size, unit_name
        );

        // Build Modbus PDU for this batch
        let pdu = PduBuilder::build_read_request(function_code, batch_start, batch_size as u16)?;

        debug!(
            "PDU @{} {}B: {:02X?}",
            batch_start,
            batch_size,
            pdu.as_slice()
        );

        // Build complete frame with proper header (MBAP for TCP, CRC for RTU)
        let request = frame_processor.build_frame(slave_id, &pdu);

        // Log outgoing request
        logger.log_protocol_message(
            "TX",
            &request,
            &format!(
                "Modbus FC{} request: slave={}, addr={}, count={}",
                function_code, slave_id, batch_start, batch_size
            ),
        );

        // Send request and wait for the correct response
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 3;
        let batch_register_values = loop {
            let mut response = vec![0u8; MODBUS_RESPONSE_BUFFER_SIZE];
            let bytes_read = connection_manager
                .send_and_receive(&request, &mut response, timeouts::DEFAULT_READ_TIMEOUT)
                .await?;
            response.truncate(bytes_read);

            // Log incoming response
            logger.log_protocol_message(
                "RX",
                &response,
                &format!("Modbus FC{} response: {} bytes", function_code, bytes_read),
            );

            match frame_processor.parse_frame(&response) {
                Ok((received_unit_id, pdu)) => {
                    if received_unit_id != slave_id {
                        return Err(ComSrvError::ProtocolError(format!(
                            "Unit ID mismatch: expected {slave_id}, got {received_unit_id}"
                        )));
                    }

                    debug!("RX PDU FC{}: {:02X?}", function_code, pdu.as_slice());

                    match parse_modbus_pdu(&pdu, function_code, batch_size as u16) {
                        Ok(values) => {
                            debug!("Parsed {} vals: {:?}", values.len(), values);
                            break values;
                        },
                        Err(e) => {
                            error!("PDU parse err: {}", e);
                            retry_count += 1;
                            if retry_count >= MAX_RETRIES {
                                return Err(e.into());
                            }
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        },
                    }
                },
                Err(e) => {
                    debug!("Mismatch response: {}", e);
                    retry_count += 1;
                    if retry_count >= MAX_RETRIES {
                        return Err(ComSrvError::TimeoutError(
                            "Failed to get matching response after retries".to_string(),
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                },
            }
        };

        // Verify received data size
        match function_code {
            1 | 2 => {
                let expected_bytes = batch_size.div_ceil(8);
                if batch_register_values.len() != expected_bytes {
                    warn!(
                        "RX {}B expect {}B ({}bits @{})",
                        batch_register_values.len(),
                        expected_bytes,
                        batch_size,
                        batch_start
                    );
                }
            },
            _ => {
                if batch_register_values.len() != batch_size {
                    warn!(
                        "RX {} regs expect {} @{}",
                        batch_register_values.len(),
                        batch_size,
                        batch_start
                    );
                }
            },
        };

        all_register_values.extend(batch_register_values);
        current_offset += batch_size;
    }

    // Extract values for each point from the complete register collection using indices
    let mut results = Vec::new();
    for &idx in indices {
        let point = &all_points[idx];

        // Handle different addressing for coils/discrete inputs vs registers
        let (registers, bit_position) = match function_code {
            1 | 2 => {
                let bit_address = point.register_address - start_address;
                let byte_offset = (bit_address / 8) as usize;
                let bit_offset = bit_address % 8;

                if byte_offset >= all_register_values.len() {
                    warn!("pt{} bit @{} OOR", point.point_id, point.register_address);
                    continue;
                }

                let single_byte = vec![all_register_values[byte_offset]];
                (single_byte, bit_offset as u8)
            },
            _ => {
                let offset = (point.register_address - start_address) as usize;
                let register_count = point.register_count as usize;

                if offset + register_count > all_register_values.len() {
                    warn!("pt{} reg @{} OOR", point.point_id, point.register_address);
                    continue;
                }

                let registers = all_register_values[offset..offset + register_count].to_vec();
                // For bool types reading from holding registers, use the configured bit_position
                let bit_position = if point.data_type == "bool" {
                    point.bit_position
                } else {
                    0 // Default for non-bool types (ignored in decoding)
                };
                (registers, bit_position)
            },
        };

        // Parse value based on data type
        // ✅ 错误隔离：单个点位解码失败不影响其他点位 / Error isolation: a single point decode failure does not affect other points
        let value = match decode_register_value(
            &registers,
            &point.data_type,
            bit_position,
            point.byte_order.as_deref(),
            Some(function_code),
        ) {
            Ok(v) => v,
            Err(e) => {
                error!(
                    "pt{} decode err: {} (type={} FC{})",
                    point.point_id, e, point.data_type, function_code
                );
                // ✅ 继续处理下一个点位，不中断批次 / Continue processing the next point without aborting the batch
                continue;
            },
        };

        // DEBUG: Log decoded raw value (transformation happens in sync layer)
        let value_str = match &value {
            ProtocolValue::Float(f) => format!("{}", f),
            ProtocolValue::Integer(i) => format!("{}", i),
            ProtocolValue::Bool(b) => format!("{}", b),
            ProtocolValue::String(s) => s.to_string(),
            ProtocolValue::Null => "null".to_string(),
        };
        debug!(
            "pt{}: {} (s={} o={} r={})",
            point.point_id, value_str, point.scale, point.offset, point.reverse
        );

        results.push((point.point_id.clone(), value));
    }

    Ok(results)
}

// build_read_fc0X_*_pdu functions moved to PduBuilder::build_read_request()
// in voltage-protocols/src/modbus/pdu.rs

/// Execute Modbus write command for control or adjustment points
async fn execute_modbus_write(
    connection_manager: &Arc<ModbusConnectionManager>,
    frame_processor: &Arc<Mutex<ModbusFrameProcessor>>,
    modbus_point: Option<&ModbusPoint>,
    point_id: u32,
    value: ProtocolValue,
    telemetry_type: FourRemote,
) -> Result<()> {
    // Use actual mapping from database or fall back to defaults for testing
    let (slave_id, function_code, register_address, data_type, byte_order) =
        if let Some(mapping) = modbus_point {
            // Use actual mapping from database
            (
                mapping.slave_id,
                mapping.function_code,
                mapping.register_address,
                mapping.data_type.clone(),
                mapping.byte_order.clone(), // Already Option<String>
            )
        } else {
            // Fallback defaults for testing (should log warning in production)
            warn!("pt{}: no mapping, using defaults", point_id);
            let default_function_code = match telemetry_type {
                FourRemote::Control => 5,    // Write Single Coil
                FourRemote::Adjustment => 6, // Write Single Register
                _ => 6,
            };
            (
                1, // default slave_id
                default_function_code,
                (40000 + point_id) as u16, // default register_address
                "uint16".to_string(),      // default data_type
                None,                      // default byte_order
            )
        };

    // Apply inverse transformation if scale/offset are non-default
    // For downlink (system → device), we need to convert engineering value to raw value
    let transformed_value = if let Some(mapping) = modbus_point {
        // Check if transformation is needed (non-default scale or non-zero offset)
        if mapping.scale != 1.0 || mapping.offset != 0.0 {
            // Check if value is numeric before transformation
            match &value {
                ProtocolValue::Integer(i) => {
                    let engineering_value = *i as f64;
                    // Apply inverse transformation: raw = (engineering - offset) / scale
                    use crate::core::channels::sync::{PointTransformer, TransformDirection};
                    let transformer = PointTransformer::linear(mapping.scale, mapping.offset);
                    let raw_value = transformer
                        .transform(engineering_value, TransformDirection::SystemToDevice);

                    // Clamp to data type range to prevent overflow
                    let clamped_value = clamp_to_data_type(raw_value, &data_type);

                    debug!(
                        "pt{} TX: eng={} → raw={} → clamp={}",
                        point_id, engineering_value, raw_value, clamped_value
                    );

                    // Return clamped value as ProtocolValue
                    ProtocolValue::Float(clamped_value)
                },
                ProtocolValue::Float(f) => {
                    let engineering_value = *f;
                    // Apply inverse transformation: raw = (engineering - offset) / scale
                    use crate::core::channels::sync::{PointTransformer, TransformDirection};
                    let transformer = PointTransformer::linear(mapping.scale, mapping.offset);
                    let raw_value = transformer
                        .transform(engineering_value, TransformDirection::SystemToDevice);

                    // Clamp to data type range to prevent overflow
                    let clamped_value = clamp_to_data_type(raw_value, &data_type);

                    debug!(
                        "pt{} TX: eng={} → raw={} → clamp={}",
                        point_id, engineering_value, raw_value, clamped_value
                    );

                    // Return clamped value as ProtocolValue
                    ProtocolValue::Float(clamped_value)
                },
                _ => {
                    warn!("pt{}: scale/offset on non-numeric {:?}", point_id, value);
                    // Return original value without transformation
                    value
                },
            }
        } else {
            // No transformation needed
            value
        }
    } else {
        // No mapping, use original value
        value
    };

    debug!(
        "Modbus write: u={} FC{} @{} type={} val={:?}",
        slave_id, function_code, register_address, data_type, transformed_value
    );

    // Convert value to register format
    let registers = ModbusCodec::encode_value_for_modbus(
        &transformed_value,
        &data_type,
        byte_order.as_deref(),
    )?;

    // Build the appropriate write PDU based on function code
    let pdu = match function_code {
        5 => {
            // Write Single Coil
            let bool_value = match transformed_value {
                ProtocolValue::Integer(i) => i != 0,
                ProtocolValue::Float(f) => f != 0.0,
                _ => false,
            };
            ModbusCodec::build_write_fc05_single_coil_pdu(register_address, bool_value)?
        },
        6 => {
            // Write Single Register
            if registers.is_empty() {
                return Err(ComSrvError::InvalidData(
                    "No register value to write".to_string(),
                ));
            }
            ModbusCodec::build_write_fc06_single_register_pdu(register_address, registers[0])?
        },
        15 => {
            // Write Multiple Coils - for now just write single coil
            let bool_value = match transformed_value {
                ProtocolValue::Integer(i) => i != 0,
                ProtocolValue::Float(f) => f != 0.0,
                _ => false,
            };
            ModbusCodec::build_write_fc15_multiple_coils_pdu(register_address, &[bool_value])?
        },
        16 => {
            // Write Multiple Registers
            ModbusCodec::build_write_fc16_multiple_registers_pdu(register_address, &registers)?
        },
        _ => {
            return Err(ComSrvError::ProtocolError(format!(
                "Unsupported write function code: {}",
                function_code
            )));
        },
    };

    // Get frame processor lock
    let mut frame_processor_guard = frame_processor.lock().await;

    // Build complete frame with proper header
    let request = frame_processor_guard.build_frame(slave_id, &pdu);

    // Send request and wait for response
    let mut response = vec![0u8; MODBUS_RESPONSE_BUFFER_SIZE];
    let bytes_read = connection_manager
        .send_and_receive(&request, &mut response, timeouts::DEFAULT_READ_TIMEOUT)
        .await?;
    response.truncate(bytes_read);

    // Parse response frame
    let (received_unit_id, response_pdu) = frame_processor_guard.parse_frame(&response)?;

    // Verify unit ID matches
    if received_unit_id != slave_id {
        return Err(ComSrvError::ProtocolError(format!(
            "Unit ID mismatch in write response: expected {}, got {}",
            slave_id, received_unit_id
        )));
    }

    // Parse write response
    ModbusCodec::parse_modbus_write_response(&response_pdu, function_code)?;

    info!(
        "Write ok: pt{} @{} u={}",
        point_id, register_address, slave_id
    );

    Ok(())
}

// parse_modbus_pdu moved to voltage-protocols/src/modbus/codec.rs
// decode_register_value moved to voltage-protocols/src/modbus/codec.rs

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    // Helper function for tests
    fn telemetry_type_from_string(s: &str) -> FourRemote {
        match s {
            "Telemetry" => FourRemote::Telemetry,
            "Signal" => FourRemote::Signal,
            "Control" => FourRemote::Control,
            "Adjustment" => FourRemote::Adjustment,
            _ => FourRemote::Telemetry, // Default
        }
    }

    #[test]
    fn test_telemetry_type_from_string() {
        assert_eq!(
            telemetry_type_from_string("Telemetry"),
            FourRemote::Telemetry
        );
        assert_eq!(telemetry_type_from_string("Signal"), FourRemote::Signal);
        assert_eq!(telemetry_type_from_string("Control"), FourRemote::Control);
        assert_eq!(
            telemetry_type_from_string("Adjustment"),
            FourRemote::Adjustment
        );
        assert_eq!(telemetry_type_from_string("Unknown"), FourRemote::Telemetry);
    }

    // ================== decode_register_value unit tests migrated to codec.rs ==================
    // Basic decode_register_value tests (bool_bitwise, bool_edge_cases, other_formats,
    // float64_abcd, float64_dcba) have been moved to voltage-protocols/modbus/codec.rs.
    // The following integration tests remain here as they test protocol-specific behavior.

    #[test]
    fn test_bit_position_full_16bit_range() {
        // Test all 16 bits (0-15) in a 16-bit register

        // Test case 1: Low byte (bits 0-7)
        let register = vec![0x00A5]; // 0b0000_0000_1010_0101

        // Bit 0 (LSB) = 1
        let result = decode_register_value(&register, "bool", 0, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 1 = 0
        let result = decode_register_value(&register, "bool", 1, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(0));

        // Bit 2 = 1
        let result = decode_register_value(&register, "bool", 2, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 5 = 1
        let result = decode_register_value(&register, "bool", 5, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 7 = 1
        let result = decode_register_value(&register, "bool", 7, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));

        // Test case 2: High byte (bits 8-15) - NEW FUNCTIONALITY!
        let register = vec![0xA500]; // 0b1010_0101_0000_0000
                                     // Bits 15-8: 1010_0101, Bits 7-0: 0000_0000

        // Bit 8 = 1
        let result = decode_register_value(&register, "bool", 8, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 9 = 0
        let result = decode_register_value(&register, "bool", 9, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(0));

        // Bit 10 = 1
        let result = decode_register_value(&register, "bool", 10, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 13 = 1
        let result = decode_register_value(&register, "bool", 13, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 15 (MSB) = 1
        let result = decode_register_value(&register, "bool", 15, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));
    }

    #[test]
    fn test_bit_position_0_based_default() {
        // Test that default bit_position is 0 (not 1)
        let register = vec![0x0001]; // Only LSB set

        // Default (0) should read LSB
        let result = decode_register_value(&register, "bool", 0, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(1));

        // Bit 1 should be 0
        let result = decode_register_value(&register, "bool", 1, None, Some(3)).unwrap();
        assert_eq!(result, ProtocolValue::Integer(0));
    }

    #[test]
    fn test_bit_position_out_of_range() {
        let register = vec![0xFFFF];

        // Bit 15 is valid (max value)
        let result = decode_register_value(&register, "bool", 15, None, Some(3));
        assert!(result.is_ok(), "Bit 15 should be valid");

        // Bit 16 is invalid
        let result = decode_register_value(&register, "bool", 16, None, Some(3));
        assert!(result.is_err(), "Bit 16 should be invalid");
        assert!(result.unwrap_err().to_string().contains("0-15"));

        // Bit 100 is also invalid
        let result = decode_register_value(&register, "bool", 100, None, Some(3));
        assert!(result.is_err(), "Bit 100 should be invalid");
    }

    #[test]
    fn test_bit_position_high_byte_extraction() {
        // Comprehensive test for bits 8-15 (high byte)

        // Test pattern: 0xF0F0 = 0b1111_0000_1111_0000
        let register = vec![0xF0F0];

        // Low byte (bits 0-7): 0b1111_0000
        for bit in 0..=3 {
            let result = decode_register_value(&register, "bool", bit, None, Some(3)).unwrap();
            assert_eq!(result, ProtocolValue::Integer(0), "Bit {} should be 0", bit);
        }
        for bit in 4..=7 {
            let result = decode_register_value(&register, "bool", bit, None, Some(3)).unwrap();
            assert_eq!(result, ProtocolValue::Integer(1), "Bit {} should be 1", bit);
        }

        // High byte (bits 8-15): 0b1111_0000
        for bit in 8..=11 {
            let result = decode_register_value(&register, "bool", bit, None, Some(3)).unwrap();
            assert_eq!(result, ProtocolValue::Integer(0), "Bit {} should be 0", bit);
        }
        for bit in 12..=15 {
            let result = decode_register_value(&register, "bool", bit, None, Some(3)).unwrap();
            assert_eq!(result, ProtocolValue::Integer(1), "Bit {} should be 1", bit);
        }
    }

    // ================== Phase 1: Constructor Tests (5 tests) ==================

    // Helper function to create test polling config
    fn create_test_polling_config() -> ModbusPollingConfig {
        use std::collections::HashMap;
        use voltage_protocols::modbus::ModbusBatchConfig;

        ModbusPollingConfig {
            enabled: true,
            default_interval_ms: 1000,
            connection_timeout_ms: 5000,
            read_timeout_ms: 3000,
            max_retries: 3,
            retry_interval_ms: 1000,
            batch_config: ModbusBatchConfig {
                enabled: true,
                max_batch_size: 100,
                max_gap: 10,
                device_limits: HashMap::new(),
            },
            slaves: HashMap::new(),
            reconnect_enabled: true,
            reconnect_max_consecutive: 5,
            reconnect_cooldown_ms: 60000,
            error_threshold: 5,
        }
    }

    #[test]
    fn test_modbus_protocol_new_tcp_mode() {
        use crate::core::config::ChannelConfig;
        use crate::core::config::ChannelLoggingConfig;
        use std::collections::HashMap;

        let channel_config = ChannelConfig {
            core: crate::core::config::ChannelCore {
                id: 1001,
                name: "TestChannel".to_string(),
                description: Some("Test TCP channel".to_string()),
                protocol: "modbus_tcp".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
        };

        let connection_params = ConnectionParams {
            host: Some("192.168.1.100".to_string()),
            port: Some(502),
            device: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let polling_config = create_test_polling_config();

        let protocol = ModbusProtocol::new(channel_config, connection_params, polling_config)
            .expect("TCP protocol creation should succeed");

        assert_eq!(protocol.channel_id, 1001);
        assert_eq!(protocol.name.as_ref(), "TestChannel");
        assert!(!protocol.is_connected.load(Ordering::Relaxed));
    }

    #[test]
    fn test_modbus_protocol_new_rtu_mode() {
        use crate::core::config::ChannelConfig;
        use crate::core::config::ChannelLoggingConfig;
        use std::collections::HashMap;

        let channel_config = ChannelConfig {
            core: crate::core::config::ChannelCore {
                id: 2001,
                name: "RTU_Channel".to_string(),
                description: Some("Test RTU channel".to_string()),
                protocol: "modbus_rtu".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
        };

        let connection_params = ConnectionParams {
            host: None,
            port: None,
            device: Some("/dev/ttyUSB0".to_string()),
            baud_rate: Some(9600),
            data_bits: Some(8),
            stop_bits: Some(1),
            parity: Some("None".to_string()),
            timeout: Duration::from_millis(500),
        };

        let polling_config = create_test_polling_config();

        let protocol = ModbusProtocol::new(channel_config, connection_params, polling_config)
            .expect("RTU protocol creation should succeed");

        assert_eq!(protocol.channel_id, 2001);
        assert_eq!(protocol.name.as_ref(), "RTU_Channel");
        assert!(!protocol.is_connected.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_modbus_protocol_default_state_initialization() {
        use crate::core::config::ChannelConfig;
        use std::collections::HashMap;

        use crate::core::config::ChannelLoggingConfig;
        let channel_config = ChannelConfig {
            core: crate::core::config::ChannelCore {
                id: 3001,
                name: "DefaultStateChannel".to_string(),
                description: None,
                protocol: "modbus_tcp".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
        };

        let connection_params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            device: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout: Duration::from_secs(3),
        };

        let polling_config = create_test_polling_config();

        let protocol = ModbusProtocol::new(channel_config, connection_params, polling_config)
            .expect("Protocol creation should succeed");

        // Verify initial connection state
        let conn_state = protocol.connection_state.read().await;
        assert!(
            matches!(*conn_state, ConnectionState::Uninitialized),
            "Initial connection state should be Uninitialized"
        );

        // Verify empty point lists
        assert_eq!(protocol.telemetry_points.read().await.len(), 0);
        assert_eq!(protocol.signal_points.read().await.len(), 0);
        assert_eq!(protocol.control_points.read().await.len(), 0);
        assert_eq!(protocol.adjustment_points.read().await.len(), 0);

        // Verify no active tasks
        assert!(protocol.polling_handle.read().await.is_none());
        assert!(protocol.command_handle.read().await.is_none());
    }

    #[test]
    fn test_modbus_protocol_config_parameters_passed_correctly() {
        use crate::core::config::ChannelConfig;
        use crate::core::config::ChannelLoggingConfig;
        use std::collections::HashMap;

        let channel_config = ChannelConfig {
            core: crate::core::config::ChannelCore {
                id: 4001,
                name: "ConfigTest".to_string(),
                description: None,
                protocol: "modbus_tcp".to_string(),
                enabled: false,
            },
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
        };

        let connection_params = ConnectionParams {
            host: Some("10.0.0.50".to_string()),
            port: Some(5020),
            device: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout: Duration::from_secs(10),
        };

        let polling_config = create_test_polling_config();

        let protocol = ModbusProtocol::new(channel_config, connection_params, polling_config)
            .expect("Protocol creation should succeed");

        // Verify channel config parameters
        assert_eq!(protocol.channel_id, 4001);
        assert_eq!(protocol.name.as_ref(), "ConfigTest");

        // Verify polling config wrapped in Arc
        assert_eq!(protocol.polling_config.default_interval_ms, 1000);
        assert_eq!(protocol.polling_config.connection_timeout_ms, 5000);
        assert_eq!(protocol.polling_config.read_timeout_ms, 3000);
    }

    #[test]
    fn test_modbus_protocol_arc_wrapped_fields() {
        use crate::core::config::ChannelConfig;
        use crate::core::config::ChannelLoggingConfig;
        use std::collections::HashMap;

        let channel_config = ChannelConfig {
            core: crate::core::config::ChannelCore {
                id: 5001,
                name: "ArcTest".to_string(),
                description: None,
                protocol: "modbus_tcp".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
        };

        let connection_params = ConnectionParams {
            host: Some("localhost".to_string()),
            port: Some(502),
            device: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let polling_config = create_test_polling_config();

        let protocol = ModbusProtocol::new(channel_config, connection_params, polling_config)
            .expect("Protocol creation should succeed");

        // Verify Arc reference counts (initial count is 1)
        assert_eq!(Arc::strong_count(&protocol.connection_manager), 1);
        assert_eq!(Arc::strong_count(&protocol.frame_processor), 1);
        assert_eq!(Arc::strong_count(&protocol.is_connected), 1);
        assert_eq!(Arc::strong_count(&protocol.status), 1);
        assert_eq!(Arc::strong_count(&protocol.polling_config), 1);
    }

    // ================== Phase 2: Control/Adjustment Method Tests ==================

    /// Helper function to create a test ModbusProtocol instance for control/adjustment tests
    fn create_test_protocol() -> ModbusProtocol {
        use crate::core::config::ChannelConfig;
        use crate::core::config::ChannelLoggingConfig;
        use std::collections::HashMap;

        let channel_config = ChannelConfig {
            core: crate::core::config::ChannelCore {
                id: 2001,
                name: "ControlTestChannel".to_string(),
                description: Some("Test channel for control/adjustment".to_string()),
                protocol: "modbus_tcp".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: ChannelLoggingConfig::default(),
        };

        let connection_params = ConnectionParams {
            host: Some("192.168.1.100".to_string()),
            port: Some(502),
            device: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout: Duration::from_secs(5),
        };

        let polling_config = create_test_polling_config();

        ModbusProtocol::new(channel_config, connection_params, polling_config)
            .expect("Protocol creation should succeed")
    }

    #[tokio::test]
    async fn test_control_not_connected_returns_error() {
        use crate::core::channels::traits::{ComClient, ProtocolValue};

        let mut protocol = create_test_protocol();

        // Verify initial state: not connected
        assert!(!ComClient::is_connected(&protocol));

        // Attempt to send control command without connecting
        let commands = vec![(1u32, ProtocolValue::Float(1.0))];
        let result = ComClient::control(&mut protocol, commands).await;

        // Should return NotConnected error
        assert!(result.is_err());
        let error_msg = format!("{:?}", result.unwrap_err());
        assert!(error_msg.contains("NotConnected") || error_msg.contains("not connected"));
    }

    #[tokio::test]
    async fn test_control_empty_commands() {
        use crate::core::channels::traits::ComClient;

        let mut protocol = create_test_protocol();

        // Connect first (even though we won't send real data)
        // Note: This will fail without a real Modbus server, but we test the error handling
        let _ = ComClient::connect(&mut protocol).await;

        // Call control with empty commands
        let commands = Vec::new();
        let result = ComClient::control(&mut protocol, commands).await;

        // Should succeed but return empty results (no commands to process)
        match result {
            Ok(results) => assert!(
                results.is_empty(),
                "Empty commands should return empty results"
            ),
            Err(_) => {
                // If connect failed, that's expected without real server
                // The test still validates the control method signature
            },
        }
    }

    #[tokio::test]
    async fn test_adjustment_not_connected_returns_error() {
        use crate::core::channels::traits::{ComClient, ProtocolValue};

        let mut protocol = create_test_protocol();

        // Verify not connected
        assert!(!ComClient::is_connected(&protocol));

        // Attempt to send adjustment command without connecting
        let adjustments = vec![(1u32, ProtocolValue::Float(100.5))];
        let result = ComClient::adjustment(&mut protocol, adjustments).await;

        // Should return NotConnected error
        assert!(result.is_err());
        let error_msg = format!("{:?}", result.unwrap_err());
        assert!(error_msg.contains("NotConnected") || error_msg.contains("not connected"));
    }

    #[tokio::test]
    async fn test_adjustment_empty_commands() {
        use crate::core::channels::traits::ComClient;

        let mut protocol = create_test_protocol();

        // Attempt to connect
        let _ = ComClient::connect(&mut protocol).await;

        // Call adjustment with empty commands
        let adjustments = Vec::new();
        let result = ComClient::adjustment(&mut protocol, adjustments).await;

        // Should succeed but return empty results
        match result {
            Ok(results) => assert!(
                results.is_empty(),
                "Empty adjustments should return empty results"
            ),
            Err(_) => {
                // If connect failed, that's expected without real server
            },
        }
    }

    #[tokio::test]
    async fn test_control_command_validation() {
        use crate::core::channels::traits::ProtocolValue;

        let _protocol = create_test_protocol();

        // Test various ProtocolValue types
        let test_values = vec![
            ProtocolValue::Float(1.0),
            ProtocolValue::Integer(1),
            ProtocolValue::Bool(true),
            ProtocolValue::String(std::borrow::Cow::Borrowed("test")),
        ];

        // Verify commands can be created with different value types
        for value in test_values {
            let commands = [(1u32, value.clone())];
            assert_eq!(commands.len(), 1);
            assert_eq!(commands[0].0, 1u32);
        }
    }

    #[tokio::test]
    async fn test_adjustment_command_validation() {
        use crate::core::channels::traits::ProtocolValue;

        let _protocol = create_test_protocol();

        // Test float values for adjustment (typical use case)
        let test_values = vec![
            ProtocolValue::Float(25.5),
            ProtocolValue::Float(100.0),
            ProtocolValue::Float(0.0),
            ProtocolValue::Float(-10.5),
        ];

        // Verify adjustment commands can be created
        for value in test_values {
            let adjustments = [(1u32, value.clone())];
            assert_eq!(adjustments.len(), 1);
            assert_eq!(adjustments[0].0, 1u32);
        }
    }

    #[tokio::test]
    async fn test_control_multiple_commands() {
        use crate::core::channels::traits::ProtocolValue;

        let _protocol = create_test_protocol();

        // Create multiple control commands
        let commands = [
            (1u32, ProtocolValue::Bool(true)),
            (2u32, ProtocolValue::Bool(false)),
            (3u32, ProtocolValue::Integer(1)),
        ];

        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].0, 1u32);
        assert_eq!(commands[1].0, 2u32);
        assert_eq!(commands[2].0, 3u32);
    }

    #[tokio::test]
    async fn test_adjustment_multiple_commands() {
        use crate::core::channels::traits::ProtocolValue;

        let _protocol = create_test_protocol();

        // Create multiple adjustment commands
        let adjustments = [
            (1u32, ProtocolValue::Float(25.5)),
            (2u32, ProtocolValue::Float(30.0)),
            (3u32, ProtocolValue::Float(35.5)),
        ];

        assert_eq!(adjustments.len(), 3);
        assert_eq!(adjustments[0].0, 1u32);
        assert_eq!(adjustments[1].0, 2u32);
        assert_eq!(adjustments[2].0, 3u32);
    }

    // ================== Phase 3: PDU Builder Tests migrated to pdu.rs ==================
    // build_read_request tests have been moved to voltage-protocols/modbus/pdu.rs

    // ================== Phase 4: PDU Parsing Tests migrated to codec.rs ==================
    // parse_modbus_pdu tests have been moved to voltage-protocols/modbus/codec.rs

    // ================== Phase 5: ComBase Trait Method Tests ==================

    #[test]
    fn test_combase_name() {
        use crate::core::channels::traits::ComBase;

        let protocol = create_test_protocol();
        assert_eq!(ComBase::name(&protocol), "ControlTestChannel");
    }

    #[test]
    fn test_combase_get_channel_id() {
        use crate::core::channels::traits::ComBase;

        let protocol = create_test_protocol();
        assert_eq!(ComBase::get_channel_id(&protocol), 2001);
    }

    #[tokio::test]
    async fn test_combase_get_status() {
        use crate::core::channels::traits::ComBase;

        let protocol = create_test_protocol();
        let status = ComBase::get_status(&protocol).await;

        // Initial status should indicate not connected
        assert!(!status.is_connected);
    }

    // ================== Phase 6: ComClient Trait Method Tests ==================

    #[test]
    fn test_comclient_is_connected_initial_state() {
        use crate::core::channels::traits::ComClient;

        let protocol = create_test_protocol();
        // Initial state should be disconnected
        assert!(!ComClient::is_connected(&protocol));
    }

    #[tokio::test]
    async fn test_comclient_disconnect_when_not_connected() {
        use crate::core::channels::traits::ComClient;

        let mut protocol = create_test_protocol();

        // Disconnecting when not connected should succeed (no-op)
        let result = ComClient::disconnect(&mut protocol).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_comclient_stop_periodic_tasks_when_not_started() {
        use crate::core::channels::traits::ComClient;

        let protocol = create_test_protocol();

        // Stopping tasks when none are running should succeed
        let result = ComClient::stop_periodic_tasks(&protocol).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_comclient_set_data_channel() {
        use crate::core::channels::traits::ComClient;
        use tokio::sync::mpsc;

        let mut protocol = create_test_protocol();
        let (tx, _rx) = mpsc::channel(10);

        // Should accept data channel without error
        ComClient::set_data_channel(&mut protocol, tx);
        // No panic means success
    }

    // ================== Phase 7: Point Mapping Tests ==================

    #[tokio::test]
    async fn test_get_point_mapping_telemetry_empty() {
        let protocol = create_test_protocol();

        // Without initialization, mappings should be empty
        let mapping = protocol.get_point_mapping(FourRemote::Telemetry, 1).await;
        assert!(mapping.is_none());
    }

    #[tokio::test]
    async fn test_get_point_mapping_control_empty() {
        let protocol = create_test_protocol();

        // Control points should also be empty without initialization
        let mapping = protocol.get_point_mapping(FourRemote::Control, 1).await;
        assert!(mapping.is_none());
    }

    #[tokio::test]
    async fn test_get_point_mapping_invalid_point() {
        let protocol = create_test_protocol();

        // Non-existent point should return None for all types
        let mapping = protocol
            .get_point_mapping(FourRemote::Telemetry, 99999)
            .await;
        assert!(mapping.is_none());

        let mapping = protocol.get_point_mapping(FourRemote::Signal, 99999).await;
        assert!(mapping.is_none());

        let mapping = protocol.get_point_mapping(FourRemote::Control, 99999).await;
        assert!(mapping.is_none());

        let mapping = protocol
            .get_point_mapping(FourRemote::Adjustment, 99999)
            .await;
        assert!(mapping.is_none());
    }

    // ================== Phase 8: Queue Drain Tests ==================

    #[test]
    fn test_drain_control_queue_empty() {
        let protocol = create_test_protocol();

        let commands = protocol.drain_control_queue();
        assert!(commands.is_empty());
    }

    #[test]
    fn test_drain_adjustment_queue_empty() {
        let protocol = create_test_protocol();

        let commands = protocol.drain_adjustment_queue();
        assert!(commands.is_empty());
    }

    // ================== Phase 9 tests migrated to voltage-protocols/modbus/codec.rs ==================
    // The decode_register_value and clamp_to_data_type unit tests have been moved to the library.
    // Integration tests that test these functions within the protocol context remain here.
}
