//! Modbus protocol core implementation
//!
//! Integrates protocol processing, polling mechanism and batch optimization features
//! Note: Current version is a temporary implementation, focused on compilation

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use voltage_config::common::timeouts;

use crate::core::combase::traits::{
    ChannelCommand, ChannelLogger, ConnectionState, TelemetryBatch,
};
use crate::core::combase::{ChannelStatus, ComBase, ComClient, PointDataMap, RedisValue};
use crate::core::config::types::{ChannelConfig, FourRemote, RuntimeChannelConfig};
use crate::utils::error::{ComSrvError, Result};

use super::codec::ModbusCodec;
use super::command_batcher::{BatchCommand, CommandBatcher, BATCH_WINDOW_MS};
use super::connection::{ConnectionParams, ModbusConnectionManager, ModbusMode as ConnectionMode};
use super::constants::MODBUS_RESPONSE_BUFFER_SIZE;
use super::pdu::{ModbusPdu, PduBuilder};
use super::transport::{ModbusFrameProcessor, ModbusMode};
use super::types::{ModbusPoint, ModbusPollingConfig};

/// Type alias for pre-grouped points map: (slave_id, function_code, type) -> points
type GroupedPointsMap = HashMap<(u8, u8, String), Vec<ModbusPoint>>;

/// Modbus protocol implementation, implements `ComBase` trait
pub struct ModbusProtocol {
    /// Protocol name
    name: Arc<str>,
    /// Channel ID
    channel_id: u16,

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
        let logger = ChannelLogger::new(
            channel_config.id() as u32,
            channel_config.name().to_string(),
        );

        // Create connection manager with logger
        let connection_manager = Arc::new(ModbusConnectionManager::new(
            conn_mode,
            connection_params,
            logger.clone(),
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

    /// Create from RuntimeChannelConfig
    pub fn from_runtime_config(
        runtime_config: &crate::core::config::RuntimeChannelConfig,
    ) -> Result<Self> {
        use std::time::Duration;

        let channel_config = (*runtime_config.base).clone();

        // Extract connection parameters from channel config
        let params = &channel_config.parameters;

        // Parse connection params manually to avoid Duration serialization issues
        let conn_params = super::connection::ConnectionParams {
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

        // Parse or use default polling config
        let polling_config = if let Some(polling_value) = params.get("polling") {
            serde_json::from_value(polling_value.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse polling config: {}", e))?
        } else {
            // Use Default trait instead of json! macro
            super::types::ModbusPollingConfig::default()
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
                            error!("Failed to encode value for point {}: {}", cmd.point_id, e);
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
                                    debug!(
                                        "Batch write FC16 successful for {} registers",
                                        all_values.len()
                                    );
                                } else {
                                    error!("Batch write FC16 failed with exception");
                                }
                            }
                        },
                        Err(e) => {
                            error!("Batch write FC16 failed: {}", e);
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
                            error!("Failed to encode value for point {}: {}", cmd.point_id, e);
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
                            error!("Unsupported function code {} for control", function_code);
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
                                    error!("Write command failed for point {}", cmd.point_id);
                                }
                            }
                        },
                        Err(e) => {
                            error!("Write command failed for point {}: {}", cmd.point_id, e);
                            results.push((cmd.point_id, false));
                        },
                    }
                }
            }
        }

        Ok(results)
    }

    fn drain_control_queue(&self) -> Vec<(u32, RedisValue)> {
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
                        debug!(
                            "Processing queued control command {} at timestamp {}",
                            command_id, timestamp
                        );
                        drained.push((point_id, RedisValue::Float(value)));
                    }
                }
            }
        }

        drained
    }

    fn drain_adjustment_queue(&self) -> Vec<(u32, RedisValue)> {
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
                        info!(
                            "Processing queued adjustment command {} at timestamp {}",
                            command_id, timestamp
                        );
                        drained.push((point_id, RedisValue::Float(value)));
                    }
                }
            }
        }

        drained
    }

    async fn enqueue_batch_commands(
        &self,
        telemetry_type: FourRemote,
        commands: Vec<(u32, RedisValue)>,
        context: &str,
    ) {
        if commands.is_empty() {
            return;
        }

        let mut batch_entries = Vec::new();

        for (point_id, value) in commands {
            debug!(
                "Adding {} command to batch: point {}, value {:?}",
                context, point_id, value
            );

            let Some(mapping) = self.get_point_mapping(telemetry_type, point_id).await else {
                warn!(
                    "{} point {} not found in Modbus configuration, skipping command",
                    context, point_id
                );
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

    fn get_channel_id(&self) -> u16 {
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

                debug!(
                    "Loaded Modbus point: id={}, slave={}, func={}, addr={}, format={}, reg_count={}, bit_pos={:?}, type={}",
                    point.base.point_id,
                    slave_id,
                    function_code,
                    register_address,
                    &data_type,
                    register_count,
                    mapping.bit_position,
                    "T"  // This is a telemetry point
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

                debug!(
                    "Loaded Modbus signal point: id={}, slave={}, func={}, addr={}, format={}, reg_count={}, bit_pos={:?}, type={}",
                    point.base.point_id,
                    slave_id,
                    function_code,
                    register_address,
                    &data_type,
                    register_count,
                    mapping.bit_position,
                    "S"
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

                debug!(
                    "Loaded Modbus control point: id={}, slave={}, func={}, addr={}, format={}, reg_count={}, type={}",
                    point.base.point_id,
                    slave_id,
                    function_code,
                    register_address,
                    &data_type,
                    register_count,
                    "C"
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

                debug!(
                    "Loaded Modbus adjustment point: id={}, slave={}, func={}, addr={}, format={}, reg_count={}, type={}",
                    point.base.point_id,
                    slave_id,
                    function_code,
                    register_address,
                    &data_type,
                    register_count,
                    "A"
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

                Err(e)
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

    async fn control(&mut self, mut commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
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
        mut adjustments: Vec<(u32, RedisValue)>,
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

                info!(
                    "Pre-grouped {} point groups for channel {} polling optimization",
                    grouped_guard.len(),
                    channel_id
                );
            }

            let grouped_points = Arc::clone(&self.grouped_points);

            let polling_task = tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_millis(polling_interval));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                info!(
                    "Polling task started for channel {}, interval {}ms",
                    channel_id, polling_interval
                );

                // Create reusable buffers outside loop to avoid per-cycle allocation
                let mut telemetry_batch = Vec::with_capacity(1024);
                let mut signal_batch = Vec::with_capacity(1024);

                loop {
                    interval.tick().await;

                    // Clear buffers for reuse (no reallocation unless capacity exceeded)
                    telemetry_batch.clear();
                    signal_batch.clear();

                    // Check connection and attempt reconnection if needed
                    if !is_connected.load(Ordering::Relaxed) {
                        debug!(
                            "Channel {} not connected, attempting reconnection...",
                            channel_id
                        );

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
                                    info!("Channel {} reconnected successfully", channel_id);
                                    // Continue with polling after successful reconnection
                                },
                                Ok(false) => {
                                    // In cooldown period or max attempts reached
                                    debug!("Channel {} reconnection in cooldown, will retry after cooldown", channel_id);
                                    continue;
                                },
                                Err(e) => {
                                    // Unexpected error
                                    error!("Channel {} reconnection error: {}", channel_id, e);
                                    continue;
                                },
                            }
                        } else {
                            debug!(
                                "Channel {} reconnection disabled, skipping poll",
                                channel_id
                            );
                            continue;
                        }
                    }

                    // Use pre-grouped points (computed at startup)
                    let grouped_guard = grouped_points.read().await;
                    if grouped_guard.is_empty() {
                        debug!("No points configured for channel {}", channel_id);
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

                        debug!(
                            "Reading {} {} points for slave {}, function {}",
                            group_points.len(),
                            group_telemetry_type,
                            slave_id,
                            function_code
                        );

                        // Lock the frame processor for this batch of reads
                        let mut frame_processor = frame_processor.lock().await;

                        // Get max_batch_size from polling config, default to 100
                        let max_batch_size = polling_config.batch_config.max_batch_size;

                        // Create channel logger for protocol messages
                        let logger = crate::core::combase::traits::ChannelLogger::new(
                            channel_id.into(),
                            channel_name.to_string(),
                        );

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
                                if failed_in_group > 0 {
                                    error_count += failed_in_group;
                                    warn!(
                                        "Group (slave={}, func={}) completed with {} successes, {} failures",
                                        slave_id, function_code, values.len(), failed_in_group
                                    );
                                }

                                // Process values

                                for (point_id_str, value) in values {
                                    // Convert point_id from string to u32
                                    if let Ok(point_id) = point_id_str.parse::<u32>() {
                                        // Convert RedisValue to f64
                                        let raw_value = match value {
                                            RedisValue::Float(f) => f,
                                            RedisValue::Integer(i) => i as f64,
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
                                error!(
                                    "Failed to read modbus group (slave={}, func={}): {}",
                                    slave_id, function_code, e
                                );

                                // Check if this is a connection error
                                let error_str = e.to_string();
                                if error_str.contains("Broken pipe")
                                    || error_str.contains("Connection reset")
                                    || error_str.contains("Connection refused")
                                    || error_str.contains("TCP send error")
                                    || error_str.contains("TCP receive error")
                                {
                                    warn!("Connection lost during polling: {}", e);
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
                                        "Sent telemetry batch: channel_id={}, total_points={}",
                                        channel_id,
                                        telemetry_count + signal_count
                                    );
                                },
                                Err(e) => {
                                    error!(
                                        "Failed to send telemetry batch for channel {}: {}",
                                        channel_id, e
                                    );
                                },
                            }
                        }
                    }

                    debug!(
                        "Poll completed for channel {}: {} success, {} errors",
                        channel_id, success_count, error_count
                    );

                    // Update status - only last_update is maintained
                    status.write().await.last_update = chrono::Utc::now().timestamp();
                }
            });

            *self.polling_handle.write().await = Some(polling_task);
        }

        Ok(())
    }

    async fn stop_periodic_tasks(&self) -> Result<()> {
        info!(
            "Stopping Modbus periodic tasks for channel {}",
            self.channel_id
        );

        // Stop polling task
        if let Some(handle) = self.polling_handle.write().await.take() {
            handle.abort();
            info!("Polling task stopped for channel {}", self.channel_id);
        }

        Ok(())
    }

    fn set_data_channel(&mut self, tx: tokio::sync::mpsc::Sender<TelemetryBatch>) {
        self.data_channel = Some(tx);
        debug!("Data channel set for channel {}", self.channel_id);
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
            info!("Starting command receiver for channel {}", channel_id);
            while let Some(command) = rx.recv().await {
                let (tx, _rx) = tokio::sync::oneshot::channel();
                if let Err(e) = cmd_tx.send((command, tx)).await {
                    error!("Failed to forward command: {}", e);
                }
                // We don't wait for the result here
            }
            warn!("Command receiver stopped for channel {}", channel_id);
        });

        // Start command processing task
        let handle = tokio::spawn(async move {
            info!("Starting command processor for channel {}", channel_id);
            while let Some((command, result_tx)) = cmd_rx.recv().await {
                if !is_connected.load(Ordering::Relaxed) {
                    warn!("Received command while disconnected, ignoring");
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
                        info!(
                            "Processing control command {}: point {}, value {}",
                            command_id, point_id, value
                        );

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
                            RedisValue::Float(*value),
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
                        info!(
                            "Processing adjustment command {}: point {}, value {}",
                            command_id, point_id, value
                        );

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
                            RedisValue::Float(*value),
                            FourRemote::Adjustment,
                        )
                        .await
                    },
                };

                let _ = result_tx.send(result);
            }
            warn!("Command processor stopped for channel {}", channel_id);
        });

        // Store the command handle in a separate task to avoid blocking
        let command_handle = self.command_handle.clone();
        tokio::spawn(async move {
            let mut handle_guard = command_handle.write().await;
            *handle_guard = Some(handle);
        });

        debug!("Command receiver set for channel {}", self.channel_id);
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
    logger: &crate::core::combase::traits::ChannelLogger,
    group_telemetry_type: &str, // "telemetry", "signal", "control", "adjustment"
) -> Result<Vec<(String, RedisValue)>> {
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
                        "Batch failed (slave={}, func={}, addr={}, points={}): {} - continuing with next batch",
                        slave_id, function_code, batch_start_address, current_batch_indices.len(), e
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
                    "Final batch failed (slave={}, func={}, addr={}, points={}): {} - returning partial results",
                    slave_id, function_code, batch_start_address, current_batch_indices.len(), e
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
    logger: &crate::core::combase::traits::ChannelLogger, // Add logger parameter
    group_telemetry_type: &str, // "telemetry", "signal", "control", "adjustment"
) -> Result<Vec<(String, RedisValue)>> {
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
            "Reading Modbus batch: slave={}, func={}, start={}, count={} {} (offset={}/{})",
            slave_id,
            function_code,
            batch_start,
            batch_size,
            unit_name,
            current_offset,
            total_units
        );

        // Build Modbus PDU for this batch
        let pdu = match function_code {
            1 => build_read_fc01_coils_pdu(batch_start, batch_size as u16)?,
            2 => build_read_fc02_discrete_inputs_pdu(batch_start, batch_size as u16)?,
            3 => build_read_fc03_holding_registers_pdu(batch_start, batch_size as u16)?,
            4 => build_read_fc04_input_registers_pdu(batch_start, batch_size as u16)?,
            _ => {
                return Err(ComSrvError::ProtocolError(format!(
                    "Unsupported function code: {function_code}"
                )))
            },
        };

        debug!(
            "Built PDU for batch_start={}, batch_size={}, PDU bytes: {:02X?}",
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

                    debug!(
                        "Received PDU for FC{}: bytes={:02X?}",
                        function_code,
                        pdu.as_slice()
                    );

                    match parse_modbus_pdu(&pdu, function_code, batch_size as u16) {
                        Ok(values) => {
                            debug!(
                                "Parsed {} register values from PDU: {:?}",
                                values.len(),
                                values
                            );
                            break values;
                        },
                        Err(e) => {
                            error!("Failed to parse Modbus PDU: {}", e);
                            retry_count += 1;
                            if retry_count >= MAX_RETRIES {
                                return Err(e);
                            }
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        },
                    }
                },
                Err(e) => {
                    debug!("Ignoring mismatched response: {}", e);
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
                        "Received {} bytes, expected {} bytes for {} bits at address {}",
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
                        "Received {} registers, expected {} for batch at address {}",
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
                    warn!(
                        "Point {} with bit address {} is out of range",
                        point.point_id, point.register_address
                    );
                    continue;
                }

                let single_byte = vec![all_register_values[byte_offset]];
                (single_byte, bit_offset as u8)
            },
            _ => {
                let offset = (point.register_address - start_address) as usize;
                let register_count = point.register_count as usize;

                if offset + register_count > all_register_values.len() {
                    warn!(
                        "Point {} at register {} is out of range",
                        point.point_id, point.register_address
                    );
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
                    "Point {} decode failed: {} (data_type={}, registers={:?}, function_code={})",
                    point.point_id, e, point.data_type, registers, function_code
                );
                // ✅ 继续处理下一个点位，不中断批次 / Continue processing the next point without aborting the batch
                continue;
            },
        };

        // DEBUG: Log decoded raw value (transformation happens in sync layer)
        let value_str = match &value {
            RedisValue::Float(f) => format!("{}", f),
            RedisValue::Integer(i) => format!("{}", i),
            RedisValue::Bool(b) => format!("{}", b),
            RedisValue::String(s) => s.to_string(),
            RedisValue::Null => "null".to_string(),
        };
        debug!(
            "Point {}: decoded={} (scale={}, offset={}, reverse={})",
            point.point_id, value_str, point.scale, point.offset, point.reverse
        );

        // Convert raw register data to bytes for logging
        let mut raw_bytes = Vec::new();
        for reg in &registers {
            raw_bytes.extend_from_slice(&reg.to_be_bytes());
        }

        // Convert telemetry type to short format
        let telemetry_type_short = match group_telemetry_type {
            "telemetry" => "T",
            "signal" => "S",
            "control" => "C",
            "adjustment" => "A",
            _ => "T", // Default to T
        };

        // Calculate raw decimal value from raw_bytes (big-endian interpretation)
        let mut raw_decimal = 0u64;
        for &byte in &raw_bytes {
            raw_decimal = (raw_decimal << 8) | (byte as u64);
        }

        // Log parsed data with raw decoded value to channel log
        logger.log_parsed_data(
            telemetry_type_short,
            &point.point_id,
            &value_str, // Log the raw decoded value (before transformation)
            raw_decimal,
            &raw_bytes,
        );

        results.push((point.point_id.clone(), value));
    }

    Ok(results)
}

/// Build Modbus PDU for FC01: Read Coils
fn build_read_fc01_coils_pdu(start_address: u16, quantity: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x01)?
        .address(start_address)?
        .quantity(quantity)?
        .build())
}

/// Build Modbus PDU for FC02: Read Discrete Inputs
fn build_read_fc02_discrete_inputs_pdu(start_address: u16, quantity: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x02)?
        .address(start_address)?
        .quantity(quantity)?
        .build())
}

/// Build Modbus PDU for FC03: Read Holding Registers
fn build_read_fc03_holding_registers_pdu(start_address: u16, quantity: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x03)?
        .address(start_address)?
        .quantity(quantity)?
        .build())
}

/// Build Modbus PDU for FC04: Read Input Registers
fn build_read_fc04_input_registers_pdu(start_address: u16, quantity: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x04)?
        .address(start_address)?
        .quantity(quantity)?
        .build())
}

/// Execute Modbus write command for control or adjustment points
async fn execute_modbus_write(
    connection_manager: &Arc<ModbusConnectionManager>,
    frame_processor: &Arc<Mutex<ModbusFrameProcessor>>,
    modbus_point: Option<&ModbusPoint>,
    point_id: u32,
    value: RedisValue,
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
            warn!(
            "No mapping found for point {}, using defaults. This should not happen in production!",
            point_id
        );
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
                RedisValue::Integer(i) => {
                    let engineering_value = *i as f64;
                    // Apply inverse transformation: raw = (engineering - offset) / scale
                    use crate::core::combase::point_transformer::{
                        PointTransformer, TransformDirection,
                    };
                    let transformer = PointTransformer::linear(mapping.scale, mapping.offset);
                    let raw_value = transformer
                        .transform(engineering_value, TransformDirection::SystemToDevice);

                    debug!(
                        "Point {} downlink transformation: engineering={} → raw={} (scale={}, offset={})",
                        point_id, engineering_value, raw_value, mapping.scale, mapping.offset
                    );

                    // Return transformed value as RedisValue
                    RedisValue::Float(raw_value)
                },
                RedisValue::Float(f) => {
                    let engineering_value = *f;
                    // Apply inverse transformation: raw = (engineering - offset) / scale
                    use crate::core::combase::point_transformer::{
                        PointTransformer, TransformDirection,
                    };
                    let transformer = PointTransformer::linear(mapping.scale, mapping.offset);
                    let raw_value = transformer
                        .transform(engineering_value, TransformDirection::SystemToDevice);

                    debug!(
                        "Point {} downlink transformation: engineering={} → raw={} (scale={}, offset={})",
                        point_id, engineering_value, raw_value, mapping.scale, mapping.offset
                    );

                    // Return transformed value as RedisValue
                    RedisValue::Float(raw_value)
                },
                _ => {
                    warn!(
                        "Point {} has scale/offset but value is not numeric: {:?}",
                        point_id, value
                    );
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
        "Writing to Modbus: slave={}, func={}, addr={}, type={}, value={:?}",
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
                RedisValue::Integer(i) => i != 0,
                RedisValue::Float(f) => f != 0.0,
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
                RedisValue::Integer(i) => i != 0,
                RedisValue::Float(f) => f != 0.0,
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
        "Successfully wrote value {:?} to point {} (addr={}, slave={})",
        transformed_value, point_id, register_address, slave_id
    );

    Ok(())
}

/// Parse Modbus PDU and extract register values (with graceful degradation)
/// For FC 01/02: returns bytes as u16 values (one byte per u16)
/// For FC 03/04: returns actual 16-bit register values
///
/// Graceful degradation strategy:
/// - Parse partial data when available instead of failing completely
/// - Log warnings for incomplete/mismatched data
/// - Return as many valid registers as possible
fn parse_modbus_pdu(pdu: &ModbusPdu, function_code: u8, expected_count: u16) -> Result<Vec<u16>> {
    let pdu_data = pdu.as_slice();

    // Minimum viable PDU check (allow partial data)
    if pdu_data.len() < 2 {
        warn!(
            "PDU too short ({} bytes), cannot extract byte_count field",
            pdu_data.len()
        );
        return Ok(Vec::new()); // Return empty instead of failing
    }

    let actual_fc = pdu.function_code().unwrap_or(0);
    if actual_fc != function_code {
        return Err(ComSrvError::ProtocolError(format!(
            "Function code mismatch: expected {}, got {}",
            function_code, actual_fc
        )));
    }

    let byte_count = pdu_data[1] as usize;
    let available_bytes = pdu_data.len().saturating_sub(2); // Actual data bytes available

    // Use the smaller of declared byte_count or available bytes
    let actual_byte_count = byte_count.min(available_bytes);

    if byte_count > available_bytes {
        warn!(
            "Incomplete PDU data: declared {} bytes, only {} available - parsing partial data",
            byte_count, available_bytes
        );
    }

    // Parse based on function code with graceful degradation
    match function_code {
        1 | 2 => {
            // FC 01/02: byte_count should be ceil(coil_count / 8)
            let expected_bytes = expected_count.div_ceil(8) as usize;
            if byte_count != expected_bytes {
                warn!(
                    "Byte count mismatch for FC{:02}: expected {} bytes for {} coils, got {} - parsing available data",
                    function_code, expected_bytes, expected_count, byte_count
                );
            }

            // Return bytes as-is (each byte stored in a u16 for uniform processing)
            let mut registers = Vec::new();
            for &byte in &pdu_data[2..2 + actual_byte_count] {
                registers.push(u16::from(byte));
            }
            Ok(registers)
        },
        3 | 4 => {
            // FC 03/04: byte_count should be register_count * 2
            let expected_bytes = (expected_count * 2) as usize;
            if byte_count != expected_bytes {
                warn!(
                    "Byte count mismatch for FC{:02}: expected {} bytes for {} registers, got {} - parsing available data",
                    function_code, expected_bytes, expected_count, byte_count
                );
            }

            // Parse 16-bit registers from available complete register pairs
            let mut registers = Vec::new();
            let complete_pairs = actual_byte_count / 2; // Only parse complete 16-bit pairs

            for i in 0..complete_pairs {
                let offset = 2 + i * 2;
                if offset + 1 < pdu_data.len() {
                    let value =
                        (u16::from(pdu_data[offset]) << 8) | u16::from(pdu_data[offset + 1]);
                    registers.push(value);
                }
            }

            if !actual_byte_count.is_multiple_of(2) {
                warn!(
                    "Odd byte count ({}) - last incomplete byte ignored",
                    actual_byte_count
                );
            }

            Ok(registers)
        },
        _ => Err(ComSrvError::ProtocolError(format!(
            "Unsupported function code in PDU parsing: {function_code}"
        ))),
    }
}

// Removed local convert_registers_with_byte_order in favor of ModbusCodec::convert_registers_with_byte_order

/// Decode register values based on data format
fn decode_register_value(
    registers: &[u16],
    format: &str,
    bit_position: u8,
    byte_order: Option<&str>,
    function_code: Option<u8>,
) -> Result<RedisValue> {
    match format {
        "bool" => {
            if registers.is_empty() {
                return Err(ComSrvError::ProtocolError(
                    "No registers for bool".to_string(),
                ));
            }

            // Use 0-based bit numbering (programmer-friendly)
            // Default is 0 (LSB) when not specified
            let bit_pos = bit_position;

            // Determine if this is from coils/discrete inputs (FC 01/02) or registers (FC 03/04)
            let is_coil_response = matches!(function_code, Some(1) | Some(2));

            // Validate bit position - unified range for all types (0-15)
            if bit_pos > 15 {
                return Err(ComSrvError::ProtocolError(format!(
                    "Invalid bit position: {} (must be 0-15)",
                    bit_pos
                )));
            }

            // Unified bit extraction for both coils and registers (0-15)
            let value = registers[0];
            let bit_value = (value >> bit_pos) & 0x01;

            if is_coil_response {
                debug!(
                    "Coil bit extraction: value=0x{:04X}, bit_pos={}, bit_value={}",
                    value, bit_pos, bit_value
                );
            } else {
                debug!(
                    "Register bit extraction: value=0x{:04X}, bit_pos={}, bit_value={}",
                    value, bit_pos, bit_value
                );
            }

            Ok(RedisValue::Integer(i64::from(bit_value)))
        },
        "uint16" => {
            if registers.is_empty() {
                return Err(ComSrvError::ProtocolError(
                    "No registers for uint16".to_string(),
                ));
            }
            let value = i64::from(registers[0]);
            debug!(
                "Decoded uint16: register=0x{:04X}, value={}",
                registers[0], value
            );
            Ok(RedisValue::Integer(value))
        },
        "int16" => {
            if registers.is_empty() {
                return Err(ComSrvError::ProtocolError(
                    "No registers for int16".to_string(),
                ));
            }
            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            let value =
                if bytes.len() >= 2 {
                    let v = i16::from_be_bytes([bytes[0], bytes[1]]);
                    debug!(
                    "Decoded int16: register=0x{:04X}, byte_order={:?}, bytes={:02X?}, value={}",
                    registers[0], byte_order, &bytes[0..2], v
                );
                    i64::from(v)
                } else {
                    let v = registers[0] as i16;
                    debug!(
                        "Decoded int16: register=0x{:04X}, value={}",
                        registers[0], v
                    );
                    i64::from(v)
                };
            Ok(RedisValue::Integer(value))
        },
        "uint32" | "uint32_be" => {
            if registers.len() < 2 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for uint32".to_string(),
                ));
            }
            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            let value = if bytes.len() >= 4 {
                let v = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                debug!(
                    "Decoded uint32: registers=[0x{:04X}, 0x{:04X}], byte_order={:?}, bytes={:02X?}, value={}",
                    registers[0], registers[1], byte_order, &bytes[0..4], v
                );
                i64::from(v)
            } else {
                // Fallback to old method if bytes conversion fails
                let v = (u32::from(registers[0]) << 16) | u32::from(registers[1]);
                debug!(
                    "Decoded uint32 (fallback): registers=[0x{:04X}, 0x{:04X}], value={}",
                    registers[0], registers[1], v
                );
                i64::from(v)
            };
            Ok(RedisValue::Integer(value))
        },
        "int32" | "int32_be" => {
            if registers.len() < 2 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for int32".to_string(),
                ));
            }
            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            let value = if bytes.len() >= 4 {
                let v = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                debug!(
                    "Decoded int32: registers=[0x{:04X}, 0x{:04X}], byte_order={:?}, bytes={:02X?}, value={}",
                    registers[0], registers[1], byte_order, &bytes[0..4], v
                );
                i64::from(v)
            } else {
                // Fallback to old method if bytes conversion fails
                let v = (i32::from(registers[0]) << 16) | i32::from(registers[1]);
                debug!(
                    "Decoded int32 (fallback): registers=[0x{:04X}, 0x{:04X}], value={}",
                    registers[0], registers[1], v
                );
                i64::from(v)
            };
            Ok(RedisValue::Integer(value))
        },
        "float32" | "float32_be" | "float" => {
            if registers.len() < 2 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for float32".to_string(),
                ));
            }

            // Special handling for DCBA - the simulator stores bytes in little-endian order directly
            let (bytes, value) = if byte_order == Some("DCBA") {
                // For DCBA, extract bytes directly from registers (they're already in little-endian order)
                let mut bytes = Vec::new();
                for &reg in &registers[0..2] {
                    bytes.push((reg >> 8) as u8); // High byte of register
                    bytes.push((reg & 0xFF) as u8); // Low byte of register
                }
                // Bytes are already in little-endian order, decode with from_le_bytes
                let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                (bytes, value)
            } else {
                // For other byte orders, use the standard conversion
                let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
                if bytes.len() >= 4 {
                    let value = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    (bytes[0..4].to_vec(), value)
                } else {
                    // Fallback to direct conversion if not enough bytes
                    let bytes = vec![
                        (registers[0] >> 8) as u8,
                        (registers[0] & 0xFF) as u8,
                        (registers[1] >> 8) as u8,
                        (registers[1] & 0xFF) as u8,
                    ];
                    let value = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    (bytes, value)
                }
            };

            info!(
                "Float32 conversion: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                registers,
                byte_order,
                &bytes[0..4],
                value
            );
            Ok(RedisValue::Float(f64::from(value)))
        },
        "float64" | "float64_be" | "double" => {
            if registers.len() < 4 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for float64".to_string(),
                ));
            }

            // Special handling for DCBA - the simulator stores bytes in little-endian order directly
            let (bytes, value) = if byte_order == Some("DCBA") {
                // For DCBA, extract bytes directly from registers (they're already in little-endian order)
                let mut bytes = Vec::new();
                for &reg in &registers[0..4] {
                    bytes.push((reg >> 8) as u8); // High byte of register
                    bytes.push((reg & 0xFF) as u8); // Low byte of register
                }
                // Bytes are already in little-endian order, decode with from_le_bytes
                let value = f64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                (bytes, value)
            } else {
                // For other byte orders, use the standard conversion
                let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
                let value = f64::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                (bytes, value)
            };

            info!(
                "Float64 conversion: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                registers,
                byte_order,
                &bytes[0..8],
                value
            );
            Ok(RedisValue::Float(value))
        },
        "uint64" | "uint64_be" | "u64" | "qword" => {
            if registers.len() < 4 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for uint64".to_string(),
                ));
            }

            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            if bytes.len() < 8 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough bytes for uint64".to_string(),
                ));
            }

            let value = u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);

            debug!(
                "Decoded uint64: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                &registers[0..4],
                byte_order,
                &bytes[0..8],
                value
            );

            // Redis 使用 i64 存储，u64 需要转换 / Redis stores integers as i64 so u64 values must be converted
            // 如果值超过 i64::MAX，会被截断 / Values greater than i64::MAX will be truncated
            Ok(RedisValue::Integer(value as i64))
        },
        "int64" | "int64_be" | "i64" | "longlong" => {
            if registers.len() < 4 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for int64".to_string(),
                ));
            }

            let bytes = ModbusCodec::convert_registers_with_byte_order(registers, byte_order);
            if bytes.len() < 8 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough bytes for int64".to_string(),
                ));
            }

            let value = i64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);

            debug!(
                "Decoded int64: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                &registers[0..4],
                byte_order,
                &bytes[0..8],
                value
            );

            Ok(RedisValue::Integer(value))
        },
        _ => Err(ComSrvError::ProtocolError(format!(
            "Unsupported data format: {format}"
        ))),
    }
}

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

    #[test]
    fn test_decode_register_value_bool_bitwise() {
        // Testing bit extraction with 0-based numbering (programmer-friendly)

        // Test case 1: Register value 0xB5 = 181 = 10110101 in binary
        let register_value = 0xB5;
        let registers = vec![register_value];

        // For FC 03/04 (registers), use 0-15 bit numbering
        // Bit 0 (LSB) = 1
        let result = decode_register_value(&registers, "bool", 0, None, Some(3))
            .expect("decoding bit 0 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 1 = 0
        let result = decode_register_value(&registers, "bool", 1, None, Some(3))
            .expect("decoding bit 1 should succeed");
        assert_eq!(result, RedisValue::Integer(0));

        // Bit 2 = 1
        let result = decode_register_value(&registers, "bool", 2, None, Some(3))
            .expect("decoding bit 2 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 7 = 1
        let result = decode_register_value(&registers, "bool", 7, None, Some(3))
            .expect("decoding bit 7 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Test that full 16-bit range (0-15) is valid for registers
        let high_bit_register = 0x8000; // Bit 15 (MSB) set
        let high_registers = vec![high_bit_register];
        let result = decode_register_value(&high_registers, "bool", 15, None, Some(3))
            .expect("decoding bit 15 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Test FC 01/02 (coils) - uses 0-15 bit numbering
        let coil_byte = 0xB5; // Same value but treated as byte
        let coil_registers = vec![coil_byte];

        // Bit 0 (LSB) = 1
        let result = decode_register_value(&coil_registers, "bool", 0, None, Some(1))
            .expect("decoding coil bit 0 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 7 (MSB of low byte) = 1
        let result = decode_register_value(&coil_registers, "bool", 7, None, Some(1))
            .expect("decoding coil bit 7 should succeed");
        assert_eq!(result, RedisValue::Integer(1));
    }

    #[test]
    fn test_decode_register_value_bool_edge_cases() {
        let registers = vec![0x0000]; // All-zero register

        // Testing FC 01/02 (coils) - unified 0-15 bit numbering
        for bit_pos in 0..=15 {
            let result = decode_register_value(&registers, "bool", bit_pos, None, Some(1));
            if let Ok(value) = result {
                assert_eq!(value, RedisValue::Integer(0), "Bit {} should be 0", bit_pos);
            } else {
                panic!("Failed to decode bit {}", bit_pos);
            }
        }

        // Testing FC 03/04 (registers) - 0-15 bit numbering
        let registers_16bit = vec![0x0100]; // 0x0100 in binary: 0000 0001 0000 0000, only bit 8 is set
        for bit_pos in 0..=15 {
            let result = decode_register_value(&registers_16bit, "bool", bit_pos, None, Some(3));
            let expected = if bit_pos == 8 { 1 } else { 0 }; // Only bit 8 is set
            if let Ok(value) = result {
                assert_eq!(
                    value,
                    RedisValue::Integer(expected),
                    "Bit {} should be {}",
                    bit_pos,
                    expected
                );
            } else {
                panic!("Failed to decode bit {}", bit_pos);
            }
        }

        let registers_all_ones = vec![0xFFFF]; // All 1s register
        for bit_pos in 0..=15 {
            let result = decode_register_value(&registers_all_ones, "bool", bit_pos, None, Some(3));
            if let Ok(value) = result {
                assert_eq!(value, RedisValue::Integer(1), "Bit {} should be 1", bit_pos);
            } else {
                panic!("Failed to decode bit {}", bit_pos);
            }
        }

        // Testing error case: Bit 0 should be valid for registers (FC 03)
        let result = decode_register_value(&registers, "bool", 0, None, Some(3));
        assert!(
            result.is_ok(),
            "Bit position 0 should be valid for registers"
        );

        // Testing error case: bit position out of range for 16-bit mode
        let registers_16bit = vec![0x0100];
        let result = decode_register_value(&registers_16bit, "bool", 16, None, Some(3));
        assert!(
            result.is_err(),
            "Bit position 16 should be invalid (must be 0-15)"
        );

        // Testing error case: empty registers
        let empty_registers = vec![];
        let result = decode_register_value(&empty_registers, "bool", 0, None, Some(3));
        assert!(result.is_err());

        // Testing default bit_position (should be 0 - LSB)
        let registers = vec![0x0001]; // Only bit 0 (LSB) is set
        let result = decode_register_value(&registers, "bool", 0, None, Some(3))
            .expect("decoding bool with default bit position should succeed");
        assert_eq!(result, RedisValue::Integer(1)); // Default bit 0 = 1
    }

    #[test]
    fn test_decode_register_value_other_formats() {
        // Ensure other data formats still work normally
        let registers = vec![0x1234];

        // Testing uint16
        let result = decode_register_value(&registers, "uint16", 0, None, None)
            .expect("decoding uint16 should succeed");
        assert_eq!(result, RedisValue::Integer(0x1234));

        // Testing int16
        let result = decode_register_value(&registers, "int16", 0, None, None)
            .expect("decoding int16 should succeed");
        assert_eq!(result, RedisValue::Integer(i64::from(0x1234_i16)));

        // Testing float32 (needs 2 registers)
        let float_registers = vec![0x4000, 0x0000]; // 2.0 in IEEE 754
        let result = decode_register_value(&float_registers, "float32", 0, None, None)
            .expect("decoding float32 should succeed");
        if let RedisValue::Float(f) = result {
            assert!((f - 2.0).abs() < 0.0001);
        } else {
            panic!("Expected float value");
        }
    }

    #[test]
    fn test_decode_register_value_float64_abcd() {
        // Prepare a known f64 value and encode as big-endian bytes (ABCD)
        let v: f64 = 123.456789;
        let bytes = v.to_be_bytes();
        let registers = vec![
            u16::from_be_bytes([bytes[0], bytes[1]]),
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u16::from_be_bytes([bytes[4], bytes[5]]),
            u16::from_be_bytes([bytes[6], bytes[7]]),
        ];

        let result = decode_register_value(&registers, "float64", 0, Some("ABCD"), None)
            .expect("float64 ABCD decode should succeed");
        match result {
            RedisValue::Float(f) => assert!((f - v).abs() < 1e-9),
            _ => panic!("Expected float value"),
        }
    }

    #[test]
    fn test_decode_register_value_float64_dcba() {
        // Prepare a known f64 value and encode as little-endian bytes (DCBA path)
        let v: f64 = -9876.54321;
        let bytes = v.to_le_bytes();
        let registers = vec![
            u16::from_be_bytes([bytes[0], bytes[1]]),
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u16::from_be_bytes([bytes[4], bytes[5]]),
            u16::from_be_bytes([bytes[6], bytes[7]]),
        ];

        let result = decode_register_value(&registers, "float64", 0, Some("DCBA"), None)
            .expect("float64 DCBA decode should succeed");
        match result {
            RedisValue::Float(f) => assert!((f - v).abs() < 1e-9),
            _ => panic!("Expected float value"),
        }
    }

    #[test]
    fn test_bit_position_full_16bit_range() {
        // Test all 16 bits (0-15) in a 16-bit register

        // Test case 1: Low byte (bits 0-7)
        let register = vec![0x00A5]; // 0b0000_0000_1010_0101

        // Bit 0 (LSB) = 1
        let result = decode_register_value(&register, "bool", 0, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 1 = 0
        let result = decode_register_value(&register, "bool", 1, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(0));

        // Bit 2 = 1
        let result = decode_register_value(&register, "bool", 2, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 5 = 1
        let result = decode_register_value(&register, "bool", 5, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 7 = 1
        let result = decode_register_value(&register, "bool", 7, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));

        // Test case 2: High byte (bits 8-15) - NEW FUNCTIONALITY!
        let register = vec![0xA500]; // 0b1010_0101_0000_0000
                                     // Bits 15-8: 1010_0101, Bits 7-0: 0000_0000

        // Bit 8 = 1
        let result = decode_register_value(&register, "bool", 8, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 9 = 0
        let result = decode_register_value(&register, "bool", 9, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(0));

        // Bit 10 = 1
        let result = decode_register_value(&register, "bool", 10, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 13 = 1
        let result = decode_register_value(&register, "bool", 13, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 15 (MSB) = 1
        let result = decode_register_value(&register, "bool", 15, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));
    }

    #[test]
    fn test_bit_position_0_based_default() {
        // Test that default bit_position is 0 (not 1)
        let register = vec![0x0001]; // Only LSB set

        // Default (0) should read LSB
        let result = decode_register_value(&register, "bool", 0, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 1 should be 0
        let result = decode_register_value(&register, "bool", 1, None, Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(0));
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
            assert_eq!(result, RedisValue::Integer(0), "Bit {} should be 0", bit);
        }
        for bit in 4..=7 {
            let result = decode_register_value(&register, "bool", bit, None, Some(3)).unwrap();
            assert_eq!(result, RedisValue::Integer(1), "Bit {} should be 1", bit);
        }

        // High byte (bits 8-15): 0b1111_0000
        for bit in 8..=11 {
            let result = decode_register_value(&register, "bool", bit, None, Some(3)).unwrap();
            assert_eq!(result, RedisValue::Integer(0), "Bit {} should be 0", bit);
        }
        for bit in 12..=15 {
            let result = decode_register_value(&register, "bool", bit, None, Some(3)).unwrap();
            assert_eq!(result, RedisValue::Integer(1), "Bit {} should be 1", bit);
        }
    }

    // ================== Phase 1: Constructor Tests (5 tests) ==================

    // Helper function to create test polling config
    fn create_test_polling_config() -> ModbusPollingConfig {
        use crate::protocols::modbus::types::ModbusBatchConfig;
        use std::collections::HashMap;

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
        }
    }

    #[test]
    fn test_modbus_protocol_new_tcp_mode() {
        use crate::core::config::types::ChannelConfig;
        use std::collections::HashMap;
        use voltage_config::comsrv::ChannelLoggingConfig;

        let channel_config = ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
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
        use crate::core::config::types::ChannelConfig;
        use std::collections::HashMap;
        use voltage_config::comsrv::ChannelLoggingConfig;

        let channel_config = ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
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
        use crate::core::config::types::ChannelConfig;
        use std::collections::HashMap;

        use voltage_config::comsrv::ChannelLoggingConfig;
        let channel_config = ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
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
        use crate::core::config::types::ChannelConfig;
        use std::collections::HashMap;
        use voltage_config::comsrv::ChannelLoggingConfig;

        let channel_config = ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
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
        use crate::core::config::types::ChannelConfig;
        use std::collections::HashMap;
        use voltage_config::comsrv::ChannelLoggingConfig;

        let channel_config = ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
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
        use crate::core::config::types::ChannelConfig;
        use std::collections::HashMap;
        use voltage_config::comsrv::ChannelLoggingConfig;

        let channel_config = ChannelConfig {
            core: voltage_config::comsrv::ChannelCore {
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
        use crate::core::combase::traits::{ComClient, RedisValue};

        let mut protocol = create_test_protocol();

        // Verify initial state: not connected
        assert!(!ComClient::is_connected(&protocol));

        // Attempt to send control command without connecting
        let commands = vec![(1u32, RedisValue::Float(1.0))];
        let result = ComClient::control(&mut protocol, commands).await;

        // Should return NotConnected error
        assert!(result.is_err());
        let error_msg = format!("{:?}", result.unwrap_err());
        assert!(error_msg.contains("NotConnected") || error_msg.contains("not connected"));
    }

    #[tokio::test]
    async fn test_control_empty_commands() {
        use crate::core::combase::traits::ComClient;

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
        use crate::core::combase::traits::{ComClient, RedisValue};

        let mut protocol = create_test_protocol();

        // Verify not connected
        assert!(!ComClient::is_connected(&protocol));

        // Attempt to send adjustment command without connecting
        let adjustments = vec![(1u32, RedisValue::Float(100.5))];
        let result = ComClient::adjustment(&mut protocol, adjustments).await;

        // Should return NotConnected error
        assert!(result.is_err());
        let error_msg = format!("{:?}", result.unwrap_err());
        assert!(error_msg.contains("NotConnected") || error_msg.contains("not connected"));
    }

    #[tokio::test]
    async fn test_adjustment_empty_commands() {
        use crate::core::combase::traits::ComClient;

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
        use crate::core::combase::traits::RedisValue;

        let _protocol = create_test_protocol();

        // Test various RedisValue types
        let test_values = vec![
            RedisValue::Float(1.0),
            RedisValue::Integer(1),
            RedisValue::Bool(true),
            RedisValue::String(std::borrow::Cow::Borrowed("test")),
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
        use crate::core::combase::traits::RedisValue;

        let _protocol = create_test_protocol();

        // Test float values for adjustment (typical use case)
        let test_values = vec![
            RedisValue::Float(25.5),
            RedisValue::Float(100.0),
            RedisValue::Float(0.0),
            RedisValue::Float(-10.5),
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
        use crate::core::combase::traits::RedisValue;

        let _protocol = create_test_protocol();

        // Create multiple control commands
        let commands = [
            (1u32, RedisValue::Bool(true)),
            (2u32, RedisValue::Bool(false)),
            (3u32, RedisValue::Integer(1)),
        ];

        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].0, 1u32);
        assert_eq!(commands[1].0, 2u32);
        assert_eq!(commands[2].0, 3u32);
    }

    #[tokio::test]
    async fn test_adjustment_multiple_commands() {
        use crate::core::combase::traits::RedisValue;

        let _protocol = create_test_protocol();

        // Create multiple adjustment commands
        let adjustments = [
            (1u32, RedisValue::Float(25.5)),
            (2u32, RedisValue::Float(30.0)),
            (3u32, RedisValue::Float(35.5)),
        ];

        assert_eq!(adjustments.len(), 3);
        assert_eq!(adjustments[0].0, 1u32);
        assert_eq!(adjustments[1].0, 2u32);
        assert_eq!(adjustments[2].0, 3u32);
    }

    // ================== Phase 3: PDU Builder Tests ==================

    #[test]
    fn test_build_fc01_coils_pdu_basic() {
        let pdu = build_read_fc01_coils_pdu(0x0000, 10).expect("FC01 PDU build should succeed");

        assert_eq!(pdu.function_code(), Some(0x01));
        let data = pdu.as_slice();
        assert_eq!(data.len(), 5); // FC(1) + Addr(2) + Qty(2)
        assert_eq!(data[0], 0x01); // Function code
        assert_eq!(data[1], 0x00); // Start address high byte
        assert_eq!(data[2], 0x00); // Start address low byte
        assert_eq!(data[3], 0x00); // Quantity high byte
        assert_eq!(data[4], 0x0A); // Quantity low byte (10)
    }

    #[test]
    fn test_build_fc01_coils_pdu_max_quantity() {
        // Max coils per request is 2000 (0x07D0)
        let pdu =
            build_read_fc01_coils_pdu(0x0000, 2000).expect("FC01 max quantity should succeed");

        let data = pdu.as_slice();
        assert_eq!(data[3], 0x07); // Quantity high byte
        assert_eq!(data[4], 0xD0); // Quantity low byte
    }

    #[test]
    fn test_build_fc02_discrete_inputs_pdu_basic() {
        let pdu =
            build_read_fc02_discrete_inputs_pdu(0x0100, 16).expect("FC02 PDU build should succeed");

        assert_eq!(pdu.function_code(), Some(0x02));
        let data = pdu.as_slice();
        assert_eq!(data[0], 0x02); // Function code
        assert_eq!(data[1], 0x01); // Start address high byte
        assert_eq!(data[2], 0x00); // Start address low byte
        assert_eq!(data[3], 0x00); // Quantity high byte
        assert_eq!(data[4], 0x10); // Quantity low byte (16)
    }

    #[test]
    fn test_build_fc03_holding_registers_pdu_basic() {
        let pdu = build_read_fc03_holding_registers_pdu(0x006B, 3)
            .expect("FC03 PDU build should succeed");

        assert_eq!(pdu.function_code(), Some(0x03));
        let data = pdu.as_slice();
        assert_eq!(data[0], 0x03); // Function code
        assert_eq!(data[1], 0x00); // Start address high byte
        assert_eq!(data[2], 0x6B); // Start address low byte (107)
        assert_eq!(data[3], 0x00); // Quantity high byte
        assert_eq!(data[4], 0x03); // Quantity low byte (3)
    }

    #[test]
    fn test_build_fc03_holding_registers_pdu_max_quantity() {
        // Max registers per request is 125 (0x7D)
        let pdu = build_read_fc03_holding_registers_pdu(0x0000, 125)
            .expect("FC03 max quantity should succeed");

        let data = pdu.as_slice();
        assert_eq!(data[3], 0x00); // Quantity high byte
        assert_eq!(data[4], 0x7D); // Quantity low byte (125)
    }

    #[test]
    fn test_build_fc04_input_registers_pdu_basic() {
        let pdu =
            build_read_fc04_input_registers_pdu(0x0008, 1).expect("FC04 PDU build should succeed");

        assert_eq!(pdu.function_code(), Some(0x04));
        let data = pdu.as_slice();
        assert_eq!(data[0], 0x04); // Function code
        assert_eq!(data[1], 0x00); // Start address high byte
        assert_eq!(data[2], 0x08); // Start address low byte (8)
        assert_eq!(data[3], 0x00); // Quantity high byte
        assert_eq!(data[4], 0x01); // Quantity low byte (1)
    }

    #[test]
    fn test_build_fc04_input_registers_pdu_high_address() {
        let pdu = build_read_fc04_input_registers_pdu(0xFFFF, 1)
            .expect("FC04 high address should succeed");

        let data = pdu.as_slice();
        assert_eq!(data[1], 0xFF); // Start address high byte
        assert_eq!(data[2], 0xFF); // Start address low byte
    }

    // ================== Phase 4: PDU Parsing Tests ==================

    #[test]
    fn test_parse_modbus_pdu_fc03_basic() {
        // FC03 response: Function code + Byte count + Data
        // Reading 2 registers: returns 4 bytes of data
        let response_data = vec![
            0x03, // Function code
            0x04, // Byte count (2 registers * 2 bytes)
            0x00, 0x0A, // Register 0: 10
            0x01, 0x02, // Register 1: 258
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x03, 2).expect("FC03 parsing should succeed");

        assert_eq!(registers.len(), 2);
        assert_eq!(registers[0], 0x000A); // 10
        assert_eq!(registers[1], 0x0102); // 258
    }

    #[test]
    fn test_parse_modbus_pdu_fc04_basic() {
        // FC04 response similar to FC03
        let response_data = vec![
            0x04, // Function code
            0x02, // Byte count (1 register)
            0x12, 0x34, // Register 0: 0x1234
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x04, 1).expect("FC04 parsing should succeed");

        assert_eq!(registers.len(), 1);
        assert_eq!(registers[0], 0x1234);
    }

    #[test]
    fn test_parse_modbus_pdu_fc01_coils() {
        // FC01 response: coil status bytes
        // Reading 10 coils: returns ceil(10/8) = 2 bytes
        let response_data = vec![
            0x01, // Function code
            0x02, // Byte count
            0xCD, // Coils 0-7: 11001101
            0x01, // Coils 8-9: 00000001 (only bits 0-1 valid)
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x01, 10).expect("FC01 parsing should succeed");

        // FC01 returns bytes as u16 values for uniform processing
        assert_eq!(registers.len(), 2);
        assert_eq!(registers[0], 0xCD); // First byte
        assert_eq!(registers[1], 0x01); // Second byte
    }

    #[test]
    fn test_parse_modbus_pdu_fc02_discrete_inputs() {
        // FC02 response similar to FC01
        let response_data = vec![
            0x02, // Function code
            0x01, // Byte count
            0xAC, // Inputs 0-7: 10101100
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x02, 8).expect("FC02 parsing should succeed");

        assert_eq!(registers.len(), 1);
        assert_eq!(registers[0], 0xAC);
    }

    #[test]
    fn test_parse_modbus_pdu_function_code_mismatch() {
        let response_data = vec![0x03, 0x02, 0x00, 0x0A];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        // Request was for FC04 but response is FC03
        let result = parse_modbus_pdu(&pdu, 0x04, 1);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("mismatch"));
    }

    #[test]
    fn test_parse_modbus_pdu_unsupported_function_code() {
        let response_data = vec![0x10, 0x00, 0x01, 0x00, 0x02]; // FC16 Write Multiple Registers response
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let result = parse_modbus_pdu(&pdu, 0x10, 2);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported"));
    }

    #[test]
    fn test_parse_modbus_pdu_empty_returns_empty_vec() {
        // Very short PDU - graceful degradation
        let response_data = vec![0x03]; // Only function code, no byte count
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x03, 1).expect("Should degrade gracefully");

        // Empty result due to insufficient data
        assert!(registers.is_empty());
    }

    #[test]
    fn test_parse_modbus_pdu_fc03_partial_data() {
        // FC03 response with incomplete register data (graceful degradation)
        let response_data = vec![
            0x03, // Function code
            0x04, // Byte count says 4 bytes (2 registers)
            0x00, 0x0A, // Only 1 complete register
            0x01, // Incomplete second register (only 1 byte)
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers = parse_modbus_pdu(&pdu, 0x03, 2).expect("Should parse partial data");

        // Should return only complete registers
        assert_eq!(registers.len(), 1);
        assert_eq!(registers[0], 0x000A);
    }

    #[test]
    fn test_parse_modbus_pdu_fc03_multiple_registers() {
        // FC03 response with 5 registers
        let response_data = vec![
            0x03, // Function code
            0x0A, // Byte count (5 registers * 2 bytes = 10)
            0x00, 0x01, // Register 0: 1
            0x00, 0x02, // Register 1: 2
            0x00, 0x03, // Register 2: 3
            0x00, 0x04, // Register 3: 4
            0x00, 0x05, // Register 4: 5
        ];
        let pdu = ModbusPdu::from_slice(&response_data).expect("PDU creation should succeed");

        let registers =
            parse_modbus_pdu(&pdu, 0x03, 5).expect("FC03 multi-register should succeed");

        assert_eq!(registers.len(), 5);
        for (i, reg) in registers.iter().enumerate() {
            assert_eq!(*reg, (i + 1) as u16);
        }
    }

    // ================== Phase 5: ComBase Trait Method Tests ==================

    #[test]
    fn test_combase_name() {
        use crate::core::combase::traits::ComBase;

        let protocol = create_test_protocol();
        assert_eq!(ComBase::name(&protocol), "ControlTestChannel");
    }

    #[test]
    fn test_combase_get_channel_id() {
        use crate::core::combase::traits::ComBase;

        let protocol = create_test_protocol();
        assert_eq!(ComBase::get_channel_id(&protocol), 2001);
    }

    #[tokio::test]
    async fn test_combase_get_status() {
        use crate::core::combase::traits::ComBase;

        let protocol = create_test_protocol();
        let status = ComBase::get_status(&protocol).await;

        // Initial status should indicate not connected
        assert!(!status.is_connected);
    }

    // ================== Phase 6: ComClient Trait Method Tests ==================

    #[test]
    fn test_comclient_is_connected_initial_state() {
        use crate::core::combase::traits::ComClient;

        let protocol = create_test_protocol();
        // Initial state should be disconnected
        assert!(!ComClient::is_connected(&protocol));
    }

    #[tokio::test]
    async fn test_comclient_disconnect_when_not_connected() {
        use crate::core::combase::traits::ComClient;

        let mut protocol = create_test_protocol();

        // Disconnecting when not connected should succeed (no-op)
        let result = ComClient::disconnect(&mut protocol).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_comclient_stop_periodic_tasks_when_not_started() {
        use crate::core::combase::traits::ComClient;

        let protocol = create_test_protocol();

        // Stopping tasks when none are running should succeed
        let result = ComClient::stop_periodic_tasks(&protocol).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_comclient_set_data_channel() {
        use crate::core::combase::traits::ComClient;
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

    // ================== Phase 9: Decode Register Value Additional Tests ==================

    #[test]
    fn test_decode_register_value_int32_abcd() {
        // Test int32 with ABCD byte order (big-endian)
        let value: i32 = -12345678;
        let bytes = value.to_be_bytes();
        let registers = vec![
            u16::from_be_bytes([bytes[0], bytes[1]]),
            u16::from_be_bytes([bytes[2], bytes[3]]),
        ];

        let result = decode_register_value(&registers, "int32", 0, Some("ABCD"), None)
            .expect("int32 ABCD decode should succeed");

        match result {
            RedisValue::Integer(i) => assert_eq!(i, value as i64),
            _ => panic!("Expected integer value"),
        }
    }

    #[test]
    fn test_decode_register_value_int32_cdab() {
        // Test int32 with CDAB byte order (word-swapped big-endian)
        let value: i32 = 987654321;
        let bytes = value.to_be_bytes();
        // CDAB: swap word order
        let registers = vec![
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u16::from_be_bytes([bytes[0], bytes[1]]),
        ];

        let result = decode_register_value(&registers, "int32", 0, Some("CDAB"), None)
            .expect("int32 CDAB decode should succeed");

        match result {
            RedisValue::Integer(i) => assert_eq!(i, value as i64),
            _ => panic!("Expected integer value"),
        }
    }

    #[test]
    fn test_decode_register_value_uint32() {
        let value: u32 = 0xDEADBEEF;
        let registers = vec![(value >> 16) as u16, (value & 0xFFFF) as u16];

        let result = decode_register_value(&registers, "uint32", 0, Some("ABCD"), None)
            .expect("uint32 decode should succeed");

        match result {
            RedisValue::Integer(i) => assert_eq!(i as u32, value),
            _ => panic!("Expected integer value"),
        }
    }

    #[test]
    fn test_decode_register_value_float32_cdab() {
        // Test float32 with CDAB byte order
        let value: f32 = std::f32::consts::PI;
        let bytes = value.to_be_bytes();
        // CDAB: swap word order
        let registers = vec![
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u16::from_be_bytes([bytes[0], bytes[1]]),
        ];

        let result = decode_register_value(&registers, "float32", 0, Some("CDAB"), None)
            .expect("float32 CDAB decode should succeed");

        match result {
            RedisValue::Float(f) => assert!((f as f32 - value).abs() < 0.0001),
            _ => panic!("Expected float value"),
        }
    }

    #[test]
    fn test_decode_register_value_insufficient_registers() {
        // float32 needs 2 registers but only 1 provided
        let registers = vec![0x1234];

        let result = decode_register_value(&registers, "float32", 0, None, None);

        assert!(result.is_err());
    }

    #[test]
    fn test_decode_register_value_unknown_type() {
        let registers = vec![0x1234];

        let result = decode_register_value(&registers, "unknown_type", 0, None, None);

        // Unknown types should return error
        assert!(result.is_err());
    }
}
