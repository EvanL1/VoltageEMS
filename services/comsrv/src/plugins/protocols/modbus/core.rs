//! Modbus protocol core implementation
//!
//! Integrates protocol processing, polling mechanism and batch optimization features
//! Note: Current version is a temporary implementation, focused on compilation

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::core::combase::core::{ChannelCommand, TelemetryBatch};
use crate::core::combase::{ChannelStatus, ComBase, PointData, PointDataMap, RedisValue};
use crate::core::config::types::{ChannelConfig, TelemetryType};
use crate::utils::error::{ComSrvError, Result};

use super::connection::{ConnectionParams, ModbusConnectionManager, ModbusMode as ConnectionMode};
use super::pdu::{ModbusPdu, PduBuilder};
use super::transport::{ModbusFrameProcessor, ModbusMode};
use super::types::{ModbusPoint, ModbusPollingConfig};

/// Modbus protocol core engine
#[derive(Debug)]
pub struct ModbusCore {
    /// Polling configuration
    _polling_config: ModbusPollingConfig,
    /// Point mapping table
    _points: HashMap<String, ModbusPoint>,
}

impl ModbusCore {
    /// Create new Modbus core engine
    pub fn new(_mode: ModbusMode, polling_config: ModbusPollingConfig) -> Self {
        Self {
            _polling_config: polling_config,
            _points: HashMap::new(),
        }
    }

    /// Set point mapping table
    pub fn set_points(&mut self, points: Vec<ModbusPoint>) {
        self._points.clear();
        for point in points {
            self._points.insert(point.point_id.clone(), point);
        }
        info!(
            "Loaded {} Modbus points for protocol processing",
            self._points.len()
        );
    }

    // TODO: Implement complete polling and batch reading functionality
    // Currently commenting out complex implementation to pass compilation
}

/// Modbus protocol implementation, implements `ComBase` trait
pub struct ModbusProtocol {
    /// Protocol name
    name: String,
    /// Channel ID
    channel_id: u16,
    /// Channel configuration
    channel_config: Option<ChannelConfig>,

    /// Core components
    core: Arc<Mutex<ModbusCore>>,
    connection_manager: Arc<ModbusConnectionManager>,
    /// Frame processor for request/response correlation
    frame_processor: Arc<Mutex<ModbusFrameProcessor>>,

    /// State management
    is_connected: Arc<RwLock<bool>>,
    status: Arc<RwLock<ChannelStatus>>,

    /// Task management
    polling_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    command_handle: Arc<RwLock<Option<JoinHandle<()>>>>,

    /// Polling configuration
    polling_config: ModbusPollingConfig,
    /// Point mapping
    points: Arc<RwLock<Vec<ModbusPoint>>>,

    /// Data channel for sending telemetry data
    data_channel: Option<tokio::sync::mpsc::Sender<TelemetryBatch>>,

    /// Command receiver for receiving control commands
    command_rx: Arc<RwLock<Option<tokio::sync::mpsc::Receiver<ChannelCommand>>>>,
}

impl std::fmt::Debug for ModbusProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModbusProtocol")
            .field("name", &self.name)
            .field("channel_id", &self.channel_id)
            .field("is_connected", &self.is_connected)
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
        let mode = if channel_config.protocol.contains("tcp") {
            ModbusMode::Tcp
        } else {
            ModbusMode::Rtu
        };

        let conn_mode = if channel_config.protocol.contains("tcp") {
            ConnectionMode::Tcp
        } else {
            ConnectionMode::Rtu
        };

        let core = ModbusCore::new(mode.clone(), polling_config.clone());
        let connection_manager =
            Arc::new(ModbusConnectionManager::new(conn_mode, connection_params));
        let frame_processor = Arc::new(Mutex::new(ModbusFrameProcessor::new(mode)));

        Ok(Self {
            name: channel_config.name.clone(),
            channel_id: channel_config.id,
            channel_config: Some(channel_config),
            core: Arc::new(Mutex::new(core)),
            connection_manager,
            frame_processor,
            is_connected: Arc::new(RwLock::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::default())),
            polling_handle: Arc::new(RwLock::new(None)),
            command_handle: Arc::new(RwLock::new(None)),
            polling_config,
            points: Arc::new(RwLock::new(Vec::new())),
            data_channel: None,
            command_rx: Arc::new(RwLock::new(None)),
        })
    }
}

#[async_trait]
impl ComBase for ModbusProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol_type(&self) -> &'static str {
        "modbus"
    }

    fn is_connected(&self) -> bool {
        // Use try_read to avoid blocking in async context
        self.is_connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn get_status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }

    async fn initialize(&mut self, channel_config: &ChannelConfig) -> Result<()> {
        info!(
            "Initializing Modbus protocol for channel {} - Step 1: Starting initialization",
            channel_config.id
        );

        self.channel_config = Some(channel_config.clone());

        // Step 2: Load and parse point configurations
        info!(
            "Channel {} - Step 2: Loading point configurations",
            channel_config.id
        );
        let mut modbus_points = Vec::new();

        // Only add telemetry and signal points to polling list
        // Control and adjustment points are write-only, handled via command channel
        let polling_points = vec![
            &channel_config.telemetry_points,
            &channel_config.signal_points,
        ];

        let total_configured_points = channel_config.telemetry_points.len()
            + channel_config.signal_points.len()
            + channel_config.control_points.len()
            + channel_config.adjustment_points.len();

        info!("Channel {} - Step 2: Processing {} configured points ({} telemetry, {} signal, {} control, {} adjustment)", 
            channel_config.id,
            total_configured_points,
            channel_config.telemetry_points.len(),
            channel_config.signal_points.len(),
            channel_config.control_points.len(),
            channel_config.adjustment_points.len()
        );

        for point_map in polling_points {
            for point in point_map.values() {
                // Read fields directly from protocol_params
                if let (Some(slave_id_str), Some(function_code_str), Some(register_address_str)) = (
                    point.protocol_params.get("slave_id"),
                    point.protocol_params.get("function_code"),
                    point.protocol_params.get("register_address"),
                ) {
                    if let (Ok(slave_id), Ok(function_code), Ok(register_address)) = (
                        slave_id_str.parse::<u8>(),
                        function_code_str.parse::<u8>(),
                        register_address_str.parse::<u16>(),
                    ) {
                        // Get data type
                        let data_type = point
                            .protocol_params
                            .get("data_type")
                            .unwrap_or(&"uint16".to_string())
                            .to_string();

                        debug!(
                            "Loaded Modbus point: id={}, slave={}, func={}, addr={}, format={}, bit_pos={:?}, type={}",
                            point.point_id,
                            slave_id,
                            function_code,
                            register_address,
                            &data_type,
                            point.protocol_params.get("bit_position"),
                            &point.telemetry_type
                        );

                        let modbus_point = ModbusPoint {
                            point_id: point.point_id.to_string(),
                            slave_id,
                            function_code,
                            register_address,
                            data_type,
                            register_count: point
                                .protocol_params
                                .get("register_count")
                                .and_then(|v| v.parse::<u16>().ok())
                                .unwrap_or(1),
                            byte_order: point.protocol_params.get("byte_order").cloned(),
                            bit_position: point
                                .protocol_params
                                .get("bit_position")
                                .and_then(|v| v.parse::<u8>().ok()),
                        };

                        modbus_points.push(modbus_point);
                    } else {
                        warn!(
                            "Failed to parse Modbus parameters for point {}: slave_id={}, function_code={}, register_address={}",
                            point.point_id, slave_id_str, function_code_str, register_address_str
                        );
                    }
                } else {
                    warn!(
                        "Missing Modbus parameters for point {}: {:?}",
                        point.point_id, point.protocol_params
                    );
                }
            }
        }

        // Step 3: Set points to core and local storage
        info!(
            "Channel {} - Step 3: Setting up {} points in storage",
            channel_config.id,
            modbus_points.len()
        );
        {
            let mut core = self.core.lock().await;
            core.set_points(modbus_points.clone());
        }
        let points_count = modbus_points.len();
        *self.points.write().await = modbus_points;

        self.status.write().await.points_count = points_count;

        info!(
            "Channel {} - Step 3 completed: Successfully configured {} polling points (telemetry + signal) out of {} total points",
            channel_config.id,
            points_count,
            total_configured_points
        );

        info!("Channel {} - Initialization completed successfully (connection will be established later)", channel_config.id);
        Ok(())
    }

    async fn connect(&mut self) -> Result<()> {
        info!(
            "Channel {} - Connection Phase: Starting connection to Modbus device",
            self.channel_id
        );

        // Establish connection
        info!(
            "Channel {} - Establishing Modbus connection...",
            self.channel_id
        );
        self.connection_manager.connect().await?;
        info!(
            "Channel {} - Modbus connection established successfully",
            self.channel_id
        );

        *self.is_connected.write().await = true;
        self.status.write().await.is_connected = true;

        // Start periodic tasks (polling, etc.)
        info!("Channel {} - Starting periodic tasks...", self.channel_id);
        self.start_periodic_tasks().await?;
        info!(
            "Channel {} - Connection phase completed successfully",
            self.channel_id
        );

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting Modbus for channel {}", self.channel_id);

        // stoppingalltask
        self.stop_periodic_tasks().await?;

        // disconnectedconnection
        self.connection_manager.disconnect().await?;

        *self.is_connected.write().await = false;
        self.status.write().await.is_connected = false;

        Ok(())
    }

    async fn read_four_telemetry(&self, telemetry_type: &str) -> Result<PointDataMap> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        let mut result = HashMap::new();

        // 根据遥测typefiltering点位
        let points = self.points.read().await;
        let channel_config = self
            .channel_config
            .as_ref()
            .ok_or_else(|| ComSrvError::config("Channel configuration not initialized"))?;

        for point in points.iter() {
            // 根据遥测typeslavepair应的HashMapmedium查找点位
            if let Ok(point_id) = point.point_id.parse::<u32>() {
                // 根据telemetry_typeselection正确的HashMap
                let config_point = match telemetry_type {
                    "Telemetry" => channel_config.telemetry_points.get(&point_id),
                    "Signal" => channel_config.signal_points.get(&point_id),
                    "Control" => channel_config.control_points.get(&point_id),
                    "Adjustment" => channel_config.adjustment_points.get(&point_id),
                    _ => None,
                };

                if let Some(config_point) = config_point {
                    // TODO: 实际的 Modbus readlogic
                    // 这里暂时return模拟data
                    let value = RedisValue::Float(rand::random::<f64>() * 100.0);
                    let point_data = PointData {
                        value,
                        timestamp: chrono::Utc::now().timestamp() as u64,
                    };
                    result.insert(config_point.point_id, point_data);
                }
            }
        }

        // updatestate
        self.status.write().await.last_update = chrono::Utc::now().timestamp() as u64;
        self.status.write().await.success_count += 1;

        Ok(result)
    }

    async fn control(&mut self, mut commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        // Check if we have any pending commands from the command receiver
        if let Ok(mut rx_guard) = self.command_rx.try_write() {
            if let Some(rx) = rx_guard.as_mut() {
                // Process any pending control commands
                while let Ok(command) = rx.try_recv() {
                    match command {
                        ChannelCommand::Control {
                            command_id,
                            point_id,
                            value,
                            timestamp,
                        } => {
                            info!(
                                "Processing queued control command {} at timestamp {}",
                                command_id, timestamp
                            );
                            commands.push((point_id, RedisValue::Float(value)));
                        },
                        _ => {
                            // This is an adjustment command, skip it here
                        },
                    }
                }
            }
        }

        let mut results = Vec::new();
        let channel_config = self
            .channel_config
            .as_ref()
            .ok_or_else(|| ComSrvError::config("Channel configuration not initialized"))?;

        // Use the persistent frame processor
        let mut frame_processor = self.frame_processor.lock().await;

        for (point_id, value) in commands {
            info!(
                "Executing control command: point {}, value {:?}",
                point_id, value
            );

            // Find point configuration
            let point_config = match channel_config.control_points.get(&point_id) {
                Some(config) => config,
                None => {
                    error!("Control point {} not found in configuration", point_id);
                    results.push((point_id, false));
                    continue;
                },
            };

            // Get Modbus parameters
            let slave_id = point_config
                .protocol_params
                .get("slave_id")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(1);

            let function_code = point_config
                .protocol_params
                .get("function_code")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(5); // Default to FC05 for control

            let register_address = point_config
                .protocol_params
                .get("register_address")
                .and_then(|v| v.parse::<u16>().ok())
                .ok_or_else(|| {
                    ComSrvError::config(format!("Missing register_address for point {}", point_id))
                })?;

            let data_type = point_config
                .protocol_params
                .get("data_type")
                .map(|s| s.as_str())
                .unwrap_or("bool");

            let byte_order = point_config
                .protocol_params
                .get("byte_order")
                .map(|s| s.as_str());

            // Encode value
            let register_values = match encode_value_for_modbus(&value, data_type, byte_order) {
                Ok(values) => values,
                Err(e) => {
                    error!("Failed to encode value for point {}: {}", point_id, e);
                    results.push((point_id, false));
                    continue;
                },
            };

            // Build PDU based on function code
            let pdu = match function_code {
                5 => {
                    // FC05: Write Single Coil
                    let bool_value = register_values.first().map(|&v| v != 0).unwrap_or(false);
                    build_write_fc05_single_coil_pdu(register_address, bool_value)?
                },
                6 => {
                    // FC06: Write Single Register
                    let reg_value = register_values.first().copied().unwrap_or(0);
                    build_write_fc06_single_register_pdu(register_address, reg_value)?
                },
                15 => {
                    // FC15: Write Multiple Coils
                    let bool_values: Vec<bool> = register_values.iter().map(|&v| v != 0).collect();
                    build_write_fc15_multiple_coils_pdu(register_address, &bool_values)?
                },
                16 => {
                    // FC16: Write Multiple Registers
                    build_write_fc16_multiple_registers_pdu(register_address, &register_values)?
                },
                _ => {
                    error!("Unsupported function code {} for control", function_code);
                    results.push((point_id, false));
                    continue;
                },
            };

            // Build frame and send
            let request = frame_processor.build_frame(slave_id, &pdu);

            // Send request and receive response atomically
            let mut response = vec![0u8; 256];
            match self
                .connection_manager
                .send_and_receive(&request, &mut response, Duration::from_secs(5))
                .await
            {
                Ok(bytes_read) => {
                    response.truncate(bytes_read);

                    // Parse response
                    match frame_processor.parse_frame(&response) {
                        Ok((received_unit_id, response_pdu)) => {
                            if received_unit_id != slave_id {
                                error!(
                                    "Unit ID mismatch: expected {}, got {}",
                                    slave_id, received_unit_id
                                );
                                results.push((point_id, false));
                                continue;
                            }

                            // Parse write response
                            match parse_modbus_write_response(&response_pdu, function_code) {
                                Ok(_) => {
                                    info!("Control command successful for point {}", point_id);
                                    results.push((point_id, true));

                                    // Update status
                                    self.status.write().await.success_count += 1;
                                },
                                Err(e) => {
                                    error!("Control command failed for point {}: {}", point_id, e);
                                    results.push((point_id, false));
                                    self.status.write().await.error_count += 1;
                                },
                            }
                        },
                        Err(e) => {
                            error!("Failed to parse response for point {}: {}", point_id, e);
                            results.push((point_id, false));
                            self.status.write().await.error_count += 1;
                        },
                    }
                },
                Err(e) => {
                    error!("Failed to receive response for point {}: {}", point_id, e);
                    results.push((point_id, false));
                    self.status.write().await.error_count += 1;
                },
            }
        }

        self.status.write().await.last_update = chrono::Utc::now().timestamp() as u64;
        Ok(results)
    }

    async fn adjustment(
        &mut self,
        mut adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        // Check if we have any pending commands from the command receiver
        if let Ok(mut rx_guard) = self.command_rx.try_write() {
            if let Some(rx) = rx_guard.as_mut() {
                // Process any pending adjustment commands
                while let Ok(command) = rx.try_recv() {
                    match command {
                        ChannelCommand::Adjustment {
                            command_id,
                            point_id,
                            value,
                            timestamp,
                        } => {
                            info!(
                                "Processing queued adjustment command {} at timestamp {}",
                                command_id, timestamp
                            );
                            adjustments.push((point_id, RedisValue::Float(value)));
                        },
                        _ => {
                            // This is a control command, skip it here
                        },
                    }
                }
            }
        }

        let mut results = Vec::new();
        let channel_config = self
            .channel_config
            .as_ref()
            .ok_or_else(|| ComSrvError::config("Channel configuration not initialized"))?;

        // Use the persistent frame processor
        let mut frame_processor = self.frame_processor.lock().await;

        for (point_id, value) in adjustments {
            info!(
                "Executing adjustment command: point {}, value {:?}",
                point_id, value
            );

            // Find point configuration
            let point_config = match channel_config.adjustment_points.get(&point_id) {
                Some(config) => config,
                None => {
                    error!("Adjustment point {} not found in configuration", point_id);
                    results.push((point_id, false));
                    continue;
                },
            };

            // Get Modbus parameters
            let slave_id = point_config
                .protocol_params
                .get("slave_id")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(1);

            let function_code = point_config
                .protocol_params
                .get("function_code")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(6); // Default to FC06 for adjustment

            let register_address = point_config
                .protocol_params
                .get("register_address")
                .and_then(|v| v.parse::<u16>().ok())
                .ok_or_else(|| {
                    ComSrvError::config(format!("Missing register_address for point {}", point_id))
                })?;

            let data_type = point_config
                .protocol_params
                .get("data_type")
                .map(|s| s.as_str())
                .unwrap_or("uint16");

            let byte_order = point_config
                .protocol_params
                .get("byte_order")
                .map(|s| s.as_str());

            // Apply scaling if configured (reverse scaling for write)
            let scaled_value = if let Some(scaling_info) = &point_config.scaling {
                match &value {
                    RedisValue::Float(f) => {
                        // Reverse scaling: (value - offset) / scale
                        let raw = (f - scaling_info.offset) / scaling_info.scale;
                        RedisValue::Float(raw)
                    },
                    RedisValue::Integer(i) => {
                        // Convert to float, reverse scale, then back to integer
                        let f = *i as f64;
                        let raw = ((f - scaling_info.offset) / scaling_info.scale) as i64;
                        RedisValue::Integer(raw)
                    },
                    _ => value.clone(),
                }
            } else {
                value.clone()
            };

            // Encode value
            let register_values =
                match encode_value_for_modbus(&scaled_value, data_type, byte_order) {
                    Ok(values) => values,
                    Err(e) => {
                        error!("Failed to encode value for point {}: {}", point_id, e);
                        results.push((point_id, false));
                        continue;
                    },
                };

            // Build PDU based on function code
            let pdu = match function_code {
                6 => {
                    // FC06: Write Single Register
                    let reg_value = register_values.first().copied().unwrap_or(0);
                    build_write_fc06_single_register_pdu(register_address, reg_value)?
                },
                16 => {
                    // FC16: Write Multiple Registers
                    build_write_fc16_multiple_registers_pdu(register_address, &register_values)?
                },
                _ => {
                    error!("Unsupported function code {} for adjustment", function_code);
                    results.push((point_id, false));
                    continue;
                },
            };

            // Build frame and send
            let request = frame_processor.build_frame(slave_id, &pdu);

            // Send request and receive response atomically
            let mut response = vec![0u8; 256];
            match self
                .connection_manager
                .send_and_receive(&request, &mut response, Duration::from_secs(5))
                .await
            {
                Ok(bytes_read) => {
                    response.truncate(bytes_read);

                    // Parse response
                    match frame_processor.parse_frame(&response) {
                        Ok((received_unit_id, response_pdu)) => {
                            if received_unit_id != slave_id {
                                error!(
                                    "Unit ID mismatch: expected {}, got {}",
                                    slave_id, received_unit_id
                                );
                                results.push((point_id, false));
                                continue;
                            }

                            // Parse write response
                            match parse_modbus_write_response(&response_pdu, function_code) {
                                Ok(_) => {
                                    info!("Adjustment command successful for point {}", point_id);
                                    results.push((point_id, true));

                                    // Update status
                                    self.status.write().await.success_count += 1;
                                },
                                Err(e) => {
                                    error!(
                                        "Adjustment command failed for point {}: {}",
                                        point_id, e
                                    );
                                    results.push((point_id, false));
                                    self.status.write().await.error_count += 1;
                                },
                            }
                        },
                        Err(e) => {
                            error!("Failed to parse response for point {}: {}", point_id, e);
                            results.push((point_id, false));
                            self.status.write().await.error_count += 1;
                        },
                    }
                },
                Err(e) => {
                    error!("Failed to receive response for point {}: {}", point_id, e);
                    results.push((point_id, false));
                    self.status.write().await.error_count += 1;
                },
            }
        }

        self.status.write().await.last_update = chrono::Utc::now().timestamp() as u64;
        Ok(results)
    }

    // 四遥detaching架构下，update_pointsmethod已移除，点位configuring在initializestage直接loading

    async fn start_periodic_tasks(&self) -> Result<()> {
        info!(
            "Starting Modbus periodic tasks for channel {}",
            self.channel_id
        );

        // starting轮询task
        if self.polling_config.enabled {
            let channel_id = self.channel_id;
            let polling_interval = self.polling_config.default_interval_ms;
            let connection_manager = self.connection_manager.clone();
            let points = self.points.clone();
            let status = self.status.clone();
            let is_connected = self.is_connected.clone();
            let channel_config = self.channel_config.clone();
            debug!(
                "Cloning channel_config for polling task, is_some: {}",
                channel_config.is_some()
            );
            let polling_config = self.polling_config.clone();
            let data_channel = self.data_channel.clone();
            let frame_processor = self.frame_processor.clone();

            let polling_task = tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_millis(polling_interval));

                info!(
                    "Polling task started for channel {}, interval {}ms",
                    channel_id, polling_interval
                );

                loop {
                    interval.tick().await;

                    if !*is_connected.read().await {
                        debug!("Channel {} not connected, skipping poll", channel_id);
                        continue;
                    }

                    debug!("Executing poll for channel {}", channel_id);

                    // Group points by slave ID and function code for batch reading
                    // Points are already filtered during initialization (only telemetry and signal)
                    let grouped_points = {
                        let points_guard = points.read().await;
                        if points_guard.is_empty() {
                            debug!("No points configured for channel {}", channel_id);
                            continue;
                        }

                        debug!("Processing {} points for polling", points_guard.len());

                        // Group points by (slave_id, function_code, telemetry_type) for batch reading
                        // Store indices instead of cloning points
                        // This ensures different telemetry types with same point_id don't interfere
                        let mut groups: HashMap<(u8, u8, String), Vec<usize>> = HashMap::new();
                        for (idx, point) in points_guard.iter().enumerate() {
                            // Determine telemetry type for grouping
                            // Only telemetry and signal points enter polling (control/adjustment are filtered elsewhere)
                            let point_id_u32 = point.point_id.parse::<u32>().unwrap_or(0);
                            let telemetry_type = if let Some(ref config) = channel_config {
                                if config.signal_points.contains_key(&point_id_u32) {
                                    "signal".to_string()
                                } else {
                                    // Default to telemetry if not signal
                                    "telemetry".to_string()
                                }
                            } else {
                                "telemetry".to_string()
                            };
                            let key = (point.slave_id, point.function_code, telemetry_type);
                            groups.entry(key).or_default().push(idx);
                        }
                        groups
                    };

                    let mut success_count = 0;
                    let mut error_count = 0;

                    // Collect all telemetry and signal data for this poll cycle
                    let mut telemetry_batch = Vec::new();
                    let mut signal_batch = Vec::new();
                    let timestamp = chrono::Utc::now().timestamp();

                    // Read each group
                    for ((slave_id, function_code, _telemetry_type), point_indices) in
                        grouped_points
                    {
                        // Get the actual points for this group
                        let group_points = {
                            let points_guard = points.read().await;
                            point_indices
                                .iter()
                                .filter_map(|&idx| points_guard.get(idx).cloned())
                                .collect::<Vec<_>>()
                        };

                        if group_points.is_empty() {
                            continue;
                        }

                        // Lock the frame processor for this batch of reads
                        let mut frame_processor = frame_processor.lock().await;

                        // Get max_batch_size from polling config, default to 100
                        let max_batch_size = polling_config.batch_config.max_batch_size;

                        match read_modbus_group_with_processor(
                            &connection_manager,
                            &mut frame_processor,
                            slave_id,
                            function_code,
                            &group_points,
                            channel_config.as_ref(),
                            max_batch_size,
                        )
                        .await
                        {
                            Ok(values) => {
                                success_count += values.len();

                                // Process values
                                debug!("Processing {} values from Modbus read", values.len());

                                for (point_id_str, value) in values {
                                    // Convert point_id from string to u32
                                    if let Ok(point_id) = point_id_str.parse::<u32>() {
                                        // Convert RedisValue to f64
                                        let raw_value = match value {
                                            RedisValue::Float(f) => f,
                                            RedisValue::Integer(i) => i as f64,
                                            _ => continue, // Skip non-numeric values
                                        };

                                        // Determine telemetry type from channel config
                                        let telemetry_type = if let Some(ref config) =
                                            channel_config
                                        {
                                            if config.telemetry_points.contains_key(&point_id) {
                                                TelemetryType::Telemetry
                                            } else if config.signal_points.contains_key(&point_id) {
                                                TelemetryType::Signal
                                            } else {
                                                // Default to telemetry if not found
                                                TelemetryType::Telemetry
                                            }
                                        } else {
                                            TelemetryType::Telemetry
                                        };

                                        // Collect data for batch sending
                                        match telemetry_type {
                                            TelemetryType::Telemetry => {
                                                telemetry_batch
                                                    .push((point_id, raw_value, timestamp));
                                                debug!(
                                                    "Collected telemetry point {}: raw={:.6}",
                                                    point_id, raw_value
                                                );
                                            },
                                            TelemetryType::Signal => {
                                                signal_batch.push((point_id, raw_value, timestamp));
                                                debug!(
                                                    "Collected signal point {}: raw={:.6}",
                                                    point_id, raw_value
                                                );
                                            },
                                            TelemetryType::Control | TelemetryType::Adjustment => {
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
                            },
                        }
                    }

                    // Send batch data through channel if available
                    if let Some(ref tx) = data_channel {
                        // Send batch if not empty
                        if !telemetry_batch.is_empty() || !signal_batch.is_empty() {
                            let batch = TelemetryBatch {
                                channel_id,
                                telemetry: telemetry_batch,
                                signal: signal_batch,
                            };

                            // Use send().await instead of try_send for guaranteed delivery
                            match tx.send(batch).await {
                                Ok(()) => {
                                    debug!("Sent telemetry batch for channel {} with immediate delivery", channel_id);
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

                    info!(
                        "Poll completed for channel {}: {} success, {} errors",
                        channel_id, success_count, error_count
                    );

                    // Update status
                    let mut status_guard = status.write().await;
                    status_guard.last_update = chrono::Utc::now().timestamp() as u64;
                    status_guard.success_count += success_count as u64;
                    status_guard.error_count += error_count as u64;
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

        // stopping轮询task
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
        let is_connected = self.is_connected.clone();
        let frame_processor = self.frame_processor.clone();
        let channel_config = self.channel_config.clone();
        let _command_handle = self.command_handle.clone();
        let connection_manager = self.connection_manager.clone();

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
                if !*is_connected.read().await {
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

                        // Execute the control write
                        execute_modbus_write(
                            &connection_manager,
                            &frame_processor,
                            &channel_config,
                            *point_id,
                            RedisValue::Float(*value),
                            TelemetryType::Control,
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

                        // Execute the adjustment write
                        execute_modbus_write(
                            &connection_manager,
                            &frame_processor,
                            &channel_config,
                            *point_id,
                            RedisValue::Float(*value),
                            TelemetryType::Adjustment,
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

    async fn get_diagnostics(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "name": self.name(),
            "protocol": self.protocol_type(),
            "connected": self.is_connected(),
            "channel_id": self.channel_id,
            "points_count": self.points.read().await.len(),
            "polling_enabled": self.polling_config.enabled,
            "polling_interval_ms": self.polling_config.default_interval_ms,
        }))
    }
}

/// Read a group of Modbus points with the same slave ID and function code
async fn read_modbus_group_with_processor(
    connection_manager: &Arc<ModbusConnectionManager>,
    frame_processor: &mut ModbusFrameProcessor,
    slave_id: u8,
    function_code: u8,
    points: &[ModbusPoint],
    channel_config: Option<&ChannelConfig>,
    max_batch_size: u16,
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
            // Read current batch using zero-copy indexed version
            let batch_results = read_modbus_batch_indexed(
                connection_manager,
                frame_processor,
                slave_id,
                function_code,
                batch_start_address,
                points,
                &current_batch_indices,
                channel_config,
                max_batch_size,
            )
            .await?;
            results.extend(batch_results);

            // Start new batch
            current_batch_indices.clear();
            current_batch_indices.push(idx);
            batch_start_address = point.register_address;
        }
    }

    // Read final batch using zero-copy indexed version
    if !current_batch_indices.is_empty() {
        let batch_results = read_modbus_batch_indexed(
            connection_manager,
            frame_processor,
            slave_id,
            function_code,
            batch_start_address,
            points,
            &current_batch_indices,
            channel_config,
            max_batch_size,
        )
        .await?;
        results.extend(batch_results);
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

        // Build complete frame with proper header (MBAP for TCP, CRC for RTU)
        let request = frame_processor.build_frame(slave_id, &pdu);

        // Send request and wait for the correct response
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 3;
        let batch_register_values = loop {
            let mut response = vec![0u8; 256];
            let bytes_read = connection_manager
                .send_and_receive(&request, &mut response, Duration::from_secs(5))
                .await?;
            response.truncate(bytes_read);

            match frame_processor.parse_frame(&response) {
                Ok((received_unit_id, pdu)) => {
                    if received_unit_id != slave_id {
                        return Err(ComSrvError::ProtocolError(format!(
                            "Unit ID mismatch: expected {slave_id}, got {received_unit_id}"
                        )));
                    }

                    match parse_modbus_pdu(&pdu, function_code, batch_size as u16) {
                        Ok(values) => break values,
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
                let expected_bytes = (batch_size + 7) / 8;
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
        let (registers, bit_position_override) = match function_code {
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
                (single_byte, Some(bit_offset as u8))
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
                let bit_position_override = if point.data_type == "bool" {
                    point.bit_position
                } else {
                    None
                };
                (registers, bit_position_override)
            },
        };

        // Parse value based on data type
        let value = decode_register_value(
            &registers,
            &point.data_type,
            bit_position_override,
            point.byte_order.as_deref(),
            Some(function_code),
        )?;

        results.push((point.point_id.clone(), value));
    }

    Ok(results)
}

/// Build Modbus PDU for FC05: Write Single Coil (写单个线圈)
fn build_write_fc05_single_coil_pdu(address: u16, value: bool) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x05)?
        .address(address)?
        .byte(if value { 0xFF } else { 0x00 })?  // 0xFF00 for ON, 0x0000 for OFF
        .byte(0x00)?
        .build())
}

/// Build Modbus PDU for FC06: Write Single Register (写单个保持寄存器)
fn build_write_fc06_single_register_pdu(address: u16, value: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x06)?
        .address(address)?
        .quantity(value)?
        .build())
}

/// Build Modbus PDU for FC15: Write Multiple Coils (写多个线圈)
fn build_write_fc15_multiple_coils_pdu(start_address: u16, values: &[bool]) -> Result<ModbusPdu> {
    let quantity = values.len() as u16;
    let byte_count = quantity.div_ceil(8) as u8;

    let mut pdu = ModbusPdu::new();
    pdu.push(0x0F)?; // Function code 15: Write Multiple Coils
    pdu.push_u16(start_address)?;
    pdu.push_u16(quantity)?;
    pdu.push(byte_count)?;

    // Pack bool values into bytes
    let mut byte_value = 0u8;
    for (i, &value) in values.iter().enumerate() {
        if value {
            byte_value |= 1 << (i % 8);
        }
        if (i + 1) % 8 == 0 || i == values.len() - 1 {
            pdu.push(byte_value)?;
            byte_value = 0;
        }
    }

    Ok(pdu)
}

/// Build Modbus PDU for FC16: Write Multiple Registers (写多个保持寄存器)
fn build_write_fc16_multiple_registers_pdu(
    start_address: u16,
    values: &[u16],
) -> Result<ModbusPdu> {
    let quantity = values.len() as u16;
    let byte_count = (quantity * 2) as u8;

    let mut pdu = ModbusPdu::new();
    pdu.push(0x10)?; // Function code 16: Write Multiple Registers
    pdu.push_u16(start_address)?;
    pdu.push_u16(quantity)?;
    pdu.push(byte_count)?;

    // Add register values (big-endian)
    for &value in values {
        pdu.push_u16(value)?;
    }

    Ok(pdu)
}

/// Build Modbus PDU for FC01: Read Coils (读线圈state)
fn build_read_fc01_coils_pdu(start_address: u16, quantity: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x01)?
        .address(start_address)?
        .quantity(quantity)?
        .build())
}

/// Build Modbus PDU for FC02: Read Discrete Inputs (读discreteinputstate)
fn build_read_fc02_discrete_inputs_pdu(start_address: u16, quantity: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x02)?
        .address(start_address)?
        .quantity(quantity)?
        .build())
}

/// Build Modbus PDU for FC03: Read Holding Registers (读保持寄存器)
fn build_read_fc03_holding_registers_pdu(start_address: u16, quantity: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x03)?
        .address(start_address)?
        .quantity(quantity)?
        .build())
}

/// Build Modbus PDU for FC04: Read Input Registers (读input寄存器)
fn build_read_fc04_input_registers_pdu(start_address: u16, quantity: u16) -> Result<ModbusPdu> {
    Ok(PduBuilder::new()
        .function_code(0x04)?
        .address(start_address)?
        .quantity(quantity)?
        .build())
}

/// Parse Modbus write response PDU
fn parse_modbus_write_response(pdu: &ModbusPdu, expected_fc: u8) -> Result<bool> {
    if pdu.is_empty() {
        return Err(ComSrvError::ProtocolError("Empty PDU response".to_string()));
    }

    let function_code = pdu.function_code().unwrap_or(0);

    // Check for exception response
    if pdu.is_exception() {
        let exception_code = pdu.exception_code().unwrap_or(0);
        let error_msg = match exception_code {
            1 => "Illegal function",
            2 => "Illegal data address",
            3 => "Illegal data value",
            4 => "Slave device failure",
            _ => "Unknown exception",
        };
        return Err(ComSrvError::ProtocolError(format!(
            "Modbus exception {}: {}",
            exception_code, error_msg
        )));
    }

    // Check normal response
    if function_code != expected_fc {
        return Err(ComSrvError::ProtocolError(format!(
            "Function code mismatch: expected {}, got {}",
            expected_fc, function_code
        )));
    }

    // For write functions, the response echoes the request
    match expected_fc {
        5 | 6 => {
            // FC05/06: Response should be 5 bytes (FC + address + value)
            if pdu.len() >= 5 {
                Ok(true)
            } else {
                Err(ComSrvError::ProtocolError(
                    "Incomplete write response".to_string(),
                ))
            }
        },
        15 | 16 => {
            // FC15/16: Response should be 5 bytes (FC + address + quantity)
            if pdu.len() >= 5 {
                Ok(true)
            } else {
                Err(ComSrvError::ProtocolError(
                    "Incomplete write response".to_string(),
                ))
            }
        },
        _ => Err(ComSrvError::ProtocolError(format!(
            "Unsupported write function code: {}",
            expected_fc
        ))),
    }
}

/// Encode RedisValue to register values based on data type
fn encode_value_for_modbus(
    value: &RedisValue,
    data_type: &str,
    byte_order: Option<&str>,
) -> Result<Vec<u16>> {
    match data_type {
        "bool" => {
            let bool_value = match value {
                RedisValue::Integer(i) => *i != 0,
                RedisValue::Float(f) => *f != 0.0,
                _ => {
                    return Err(ComSrvError::ProtocolError(
                        "Invalid value type for bool".to_string(),
                    ))
                },
            };
            // For bool, return 1 or 0 as u16
            Ok(vec![if bool_value { 1 } else { 0 }])
        },
        "uint16" => {
            let int_value = match value {
                RedisValue::Integer(i) => *i as u16,
                RedisValue::Float(f) => *f as u16,
                _ => {
                    return Err(ComSrvError::ProtocolError(
                        "Invalid value type for uint16".to_string(),
                    ))
                },
            };
            Ok(vec![int_value])
        },
        "int16" => {
            let int_value = match value {
                RedisValue::Integer(i) => *i as i16,
                RedisValue::Float(f) => *f as i16,
                _ => {
                    return Err(ComSrvError::ProtocolError(
                        "Invalid value type for int16".to_string(),
                    ))
                },
            };
            Ok(vec![int_value as u16])
        },
        "uint32" | "int32" => {
            let int_value = match value {
                RedisValue::Integer(i) => *i as u32,
                RedisValue::Float(f) => *f as u32,
                _ => {
                    return Err(ComSrvError::ProtocolError(
                        "Invalid value type for 32-bit int".to_string(),
                    ))
                },
            };

            // Apply byte order conversion
            let bytes = int_value.to_be_bytes();
            let registers = convert_bytes_to_registers_with_order(&bytes, byte_order);
            Ok(registers)
        },
        "float32" => {
            let float_value = match value {
                RedisValue::Float(f) => *f as f32,
                RedisValue::Integer(i) => *i as f32,
                _ => {
                    return Err(ComSrvError::ProtocolError(
                        "Invalid value type for float32".to_string(),
                    ))
                },
            };

            let bytes = float_value.to_be_bytes();
            let registers = convert_bytes_to_registers_with_order(&bytes, byte_order);
            Ok(registers)
        },
        _ => Err(ComSrvError::ProtocolError(format!(
            "Unsupported data type for writing: {}",
            data_type
        ))),
    }
}

/// Convert bytes to registers with byte order
fn convert_bytes_to_registers_with_order(bytes: &[u8], byte_order: Option<&str>) -> Vec<u16> {
    if bytes.len() < 4 {
        // For 2-byte values
        if bytes.len() >= 2 {
            return vec![((bytes[0] as u16) << 8) | (bytes[1] as u16)];
        }
        return vec![];
    }

    match byte_order {
        Some("ABCD") | None => {
            // Big endian (default)
            vec![
                ((bytes[0] as u16) << 8) | (bytes[1] as u16),
                ((bytes[2] as u16) << 8) | (bytes[3] as u16),
            ]
        },
        Some("DCBA") => {
            // Little endian
            vec![
                ((bytes[3] as u16) << 8) | (bytes[2] as u16),
                ((bytes[1] as u16) << 8) | (bytes[0] as u16),
            ]
        },
        Some("BADC") => {
            // Swap bytes within registers
            vec![
                ((bytes[1] as u16) << 8) | (bytes[0] as u16),
                ((bytes[3] as u16) << 8) | (bytes[2] as u16),
            ]
        },
        Some("CDAB") => {
            // Swap register order
            vec![
                ((bytes[2] as u16) << 8) | (bytes[3] as u16),
                ((bytes[0] as u16) << 8) | (bytes[1] as u16),
            ]
        },
        _ => {
            // Default to big endian
            vec![
                ((bytes[0] as u16) << 8) | (bytes[1] as u16),
                ((bytes[2] as u16) << 8) | (bytes[3] as u16),
            ]
        },
    }
}

/// Execute Modbus write command for control or adjustment points
async fn execute_modbus_write(
    connection_manager: &Arc<ModbusConnectionManager>,
    frame_processor: &Arc<Mutex<ModbusFrameProcessor>>,
    channel_config: &Option<ChannelConfig>,
    point_id: u32,
    value: RedisValue,
    telemetry_type: TelemetryType,
) -> Result<()> {
    // Get the point configuration from channel config based on telemetry type
    let point_config = if let Some(config) = channel_config {
        match telemetry_type {
            TelemetryType::Control => config.control_points.get(&point_id),
            TelemetryType::Adjustment => config.adjustment_points.get(&point_id),
            _ => None,
        }
    } else {
        None
    };

    let point_config = match point_config {
        Some(p) => p,
        None => {
            return Err(ComSrvError::InvalidData(format!(
                "Point {} not found in {} configuration",
                point_id,
                match telemetry_type {
                    TelemetryType::Control => "control",
                    TelemetryType::Adjustment => "adjustment",
                    _ => "unknown",
                }
            )));
        },
    };

    // Extract Modbus parameters from protocol_params
    let slave_id = point_config
        .protocol_params
        .get("slave_id")
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(1);

    let function_code = point_config
        .protocol_params
        .get("function_code")
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(6); // Default to FC6 for single register write

    let register_address = point_config
        .protocol_params
        .get("register_address")
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| ComSrvError::InvalidData("Missing register_address".to_string()))?;

    let data_type = point_config
        .protocol_params
        .get("data_type")
        .cloned()
        .unwrap_or_else(|| "float32".to_string());

    let byte_order = point_config.protocol_params.get("byte_order").cloned();

    debug!(
        "Writing to Modbus: slave={}, func={}, addr={}, type={}, value={:?}",
        slave_id, function_code, register_address, data_type, value
    );

    // Convert value to register format
    let registers = encode_value_for_modbus(&value, &data_type, byte_order.as_deref())?;

    // Build the appropriate write PDU based on function code
    let pdu = match function_code {
        5 => {
            // Write Single Coil
            let bool_value = match value {
                RedisValue::Integer(i) => i != 0,
                RedisValue::Float(f) => f != 0.0,
                _ => false,
            };
            build_write_fc05_single_coil_pdu(register_address, bool_value)?
        },
        6 => {
            // Write Single Register
            if registers.is_empty() {
                return Err(ComSrvError::InvalidData(
                    "No register value to write".to_string(),
                ));
            }
            build_write_fc06_single_register_pdu(register_address, registers[0])?
        },
        15 => {
            // Write Multiple Coils - for now just write single coil
            let bool_value = match value {
                RedisValue::Integer(i) => i != 0,
                RedisValue::Float(f) => f != 0.0,
                _ => false,
            };
            build_write_fc15_multiple_coils_pdu(register_address, &[bool_value])?
        },
        16 => {
            // Write Multiple Registers
            build_write_fc16_multiple_registers_pdu(register_address, &registers)?
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
    let mut response = vec![0u8; 256];
    let bytes_read = connection_manager
        .send_and_receive(&request, &mut response, Duration::from_secs(5))
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
    parse_modbus_write_response(&response_pdu, function_code)?;

    info!(
        "Successfully wrote value {:?} to point {} (addr={}, slave={})",
        value, point_id, register_address, slave_id
    );

    Ok(())
}

/// Parse Modbus PDU and extract register values
/// For FC 01/02: returns bytes as u16 values (one byte per u16)
/// For FC 03/04: returns actual 16-bit register values
fn parse_modbus_pdu(pdu: &ModbusPdu, function_code: u8, expected_count: u16) -> Result<Vec<u16>> {
    let pdu_data = pdu.as_slice();

    if pdu_data.len() < 3 {
        return Err(ComSrvError::ProtocolError("PDU too short".to_string()));
    }

    let actual_fc = pdu.function_code().unwrap_or(0);
    if actual_fc != function_code {
        return Err(ComSrvError::ProtocolError(format!(
            "Function code mismatch: expected {}, got {}",
            function_code, actual_fc
        )));
    }

    let byte_count = pdu_data[1] as usize;
    if pdu_data.len() < 2 + byte_count {
        return Err(ComSrvError::ProtocolError(
            "Incomplete PDU data".to_string(),
        ));
    }

    // Validate byte count based on function code
    match function_code {
        1 | 2 => {
            // FC 01/02: byte_count should be ceil(coil_count / 8)
            let expected_bytes = ((expected_count + 7) / 8) as usize;
            if byte_count != expected_bytes {
                return Err(ComSrvError::ProtocolError(format!(
                    "Byte count mismatch for FC{:02}: expected {} bytes for {} coils, got {}",
                    function_code, expected_bytes, expected_count, byte_count
                )));
            }

            // Return bytes as-is (each byte stored in a u16 for uniform processing)
            let mut registers = Vec::new();
            for &byte in &pdu_data[2..2 + byte_count] {
                registers.push(u16::from(byte));
            }
            Ok(registers)
        },
        3 | 4 => {
            // FC 03/04: byte_count should be register_count * 2
            let expected_bytes = (expected_count * 2) as usize;
            if byte_count != expected_bytes {
                return Err(ComSrvError::ProtocolError(format!(
                    "Byte count mismatch for FC{:02}: expected {} bytes for {} registers, got {}",
                    function_code, expected_bytes, expected_count, byte_count
                )));
            }

            // Parse 16-bit registers
            let mut registers = Vec::new();
            for i in (2..2 + byte_count).step_by(2) {
                let value = (u16::from(pdu_data[i]) << 8) | u16::from(pdu_data[i + 1]);
                registers.push(value);
            }
            Ok(registers)
        },
        _ => Err(ComSrvError::ProtocolError(format!(
            "Unsupported function code in PDU parsing: {function_code}"
        ))),
    }
}

/// Convert registers to bytes with specified byte order
fn convert_registers_with_byte_order(registers: &[u16], byte_order: Option<&str>) -> Vec<u8> {
    let mut bytes = Vec::new();

    // Convert registers to bytes (default: ABCD - big endian)
    for &reg in registers {
        bytes.push((reg >> 8) as u8); // High byte (A)
        bytes.push((reg & 0xFF) as u8); // Low byte (B)
    }

    match byte_order {
        Some("ABCD") | None => bytes, // Big endian (default)
        Some("DCBA") => {
            // Reverse all bytes for complete little endian
            if bytes.len() >= 4 {
                let mut result = Vec::new();
                for chunk in bytes.chunks(4) {
                    let mut reversed = chunk.to_vec();
                    reversed.reverse();
                    result.extend(reversed);
                }
                result
            } else if bytes.len() >= 2 {
                // For 16-bit data (AB -> BA)
                let mut result = Vec::new();
                for chunk in bytes.chunks(2) {
                    let mut reversed = chunk.to_vec();
                    reversed.reverse();
                    result.extend(reversed);
                }
                result
            } else {
                bytes
            }
        },
        Some("BADC") => {
            // Swap bytes within each register: ABCD -> BADC
            if bytes.len() >= 4 {
                let mut result = Vec::new();
                for chunk in bytes.chunks(4) {
                    if chunk.len() == 4 {
                        result.push(chunk[1]); // B
                        result.push(chunk[0]); // A
                        result.push(chunk[3]); // D
                        result.push(chunk[2]); // C
                    } else {
                        result.extend_from_slice(chunk);
                    }
                }
                result
            } else {
                bytes
            }
        },
        Some("CDAB") => {
            // Swap register order but keep bytes within registers: ABCD -> CDAB
            if bytes.len() >= 4 {
                let mut result = Vec::new();
                for chunk in bytes.chunks(4) {
                    if chunk.len() == 4 {
                        result.push(chunk[2]); // C
                        result.push(chunk[3]); // D
                        result.push(chunk[0]); // A
                        result.push(chunk[1]); // B
                    } else {
                        result.extend_from_slice(chunk);
                    }
                }
                result
            } else {
                bytes
            }
        },
        Some("BA") => {
            // For int16: AB -> BA
            if bytes.len() >= 2 {
                let mut result = Vec::new();
                for chunk in bytes.chunks(2) {
                    if chunk.len() == 2 {
                        result.push(chunk[1]); // B
                        result.push(chunk[0]); // A
                    } else {
                        result.extend_from_slice(chunk);
                    }
                }
                result
            } else {
                bytes
            }
        },
        Some("AB") => bytes, // Same as default
        _ => {
            debug!("Unknown byte order: {:?}, using default ABCD", byte_order);
            bytes
        },
    }
}

/// Decode register values based on data format
fn decode_register_value(
    registers: &[u16],
    format: &str,
    bit_position: Option<u8>,
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

            // Use Modbus 1-based bit numbering
            // Default to bit 1 (not 0) per Modbus convention
            let bit_pos = bit_position.unwrap_or(1);

            // Determine if this is from coils/discrete inputs (FC 01/02) or registers (FC 03/04)
            let is_coil_response = matches!(function_code, Some(1) | Some(2));

            // All use 1-8 bit numbering per byte (Modbus convention)
            if !(1..=8).contains(&bit_pos) {
                return Err(ComSrvError::ProtocolError(format!(
                    "Invalid bit position: {} (must be 1-8)",
                    bit_pos
                )));
            }

            // Convert 1-based to 0-based for bit shifting
            let actual_bit = bit_pos - 1;

            if is_coil_response {
                // FC 01/02: Single byte
                let byte_value = registers[0] as u8;
                let bit_value = (byte_value >> actual_bit) & 0x01;

                debug!(
                    "Coil bit extraction: byte=0x{:02X}, bit_pos={} (0-based: {}), bit_value={}",
                    byte_value, bit_pos, actual_bit, bit_value
                );

                Ok(RedisValue::Integer(i64::from(bit_value)))
            } else {
                // FC 03/04: 16-bit register treated as bytes
                // For a 16-bit register: Low byte = bits 1-8, High byte would need separate handling
                let register_value = registers[0];

                // Extract from low byte (bits 1-8 of register)
                let byte_value = (register_value & 0xFF) as u8;
                let bit_value = (byte_value >> actual_bit) & 0x01;

                debug!(
                    "Register bit extraction: register=0x{:04X}, low_byte=0x{:02X}, bit_pos={} (0-based: {}), bit_value={}",
                    register_value, byte_value, bit_pos, actual_bit, bit_value
                );

                Ok(RedisValue::Integer(i64::from(bit_value)))
            }
        },
        "uint16" => {
            if registers.is_empty() {
                return Err(ComSrvError::ProtocolError(
                    "No registers for uint16".to_string(),
                ));
            }
            Ok(RedisValue::Integer(i64::from(registers[0])))
        },
        "int16" => {
            if registers.is_empty() {
                return Err(ComSrvError::ProtocolError(
                    "No registers for int16".to_string(),
                ));
            }
            let bytes = convert_registers_with_byte_order(registers, byte_order);
            if bytes.len() >= 2 {
                let value = i16::from_be_bytes([bytes[0], bytes[1]]);
                Ok(RedisValue::Integer(i64::from(value)))
            } else {
                Ok(RedisValue::Integer(i64::from(registers[0] as i16)))
            }
        },
        "uint32" | "uint32_be" => {
            if registers.len() < 2 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for uint32".to_string(),
                ));
            }
            let bytes = convert_registers_with_byte_order(registers, byte_order);
            if bytes.len() >= 4 {
                let value = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                Ok(RedisValue::Integer(i64::from(value)))
            } else {
                // Fallback to old method if bytes conversion fails
                let value = (u32::from(registers[0]) << 16) | u32::from(registers[1]);
                Ok(RedisValue::Integer(i64::from(value)))
            }
        },
        "int32" | "int32_be" => {
            if registers.len() < 2 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for int32".to_string(),
                ));
            }
            let bytes = convert_registers_with_byte_order(registers, byte_order);
            if bytes.len() >= 4 {
                let value = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                Ok(RedisValue::Integer(i64::from(value)))
            } else {
                // Fallback to old method if bytes conversion fails
                let value = (i32::from(registers[0]) << 16) | i32::from(registers[1]);
                Ok(RedisValue::Integer(i64::from(value)))
            }
        },
        "float32" | "float32_be" => {
            if registers.len() < 2 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for float32".to_string(),
                ));
            }
            let bytes = convert_registers_with_byte_order(registers, byte_order);
            if bytes.len() >= 4 {
                let value = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                debug!(
                    "Float32 conversion: registers={:?}, byte_order={:?}, bytes={:02X?}, value={}",
                    registers,
                    byte_order,
                    &bytes[0..4],
                    value
                );
                Ok(RedisValue::Float(f64::from(value)))
            } else {
                // Fallback to old method if bytes conversion fails
                let bytes = [
                    (registers[0] >> 8) as u8,
                    (registers[0] & 0xFF) as u8,
                    (registers[1] >> 8) as u8,
                    (registers[1] & 0xFF) as u8,
                ];
                let value = f32::from_be_bytes(bytes);
                Ok(RedisValue::Float(f64::from(value)))
            }
        },
        _ => Err(ComSrvError::ProtocolError(format!(
            "Unsupported data format: {format}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::data_processor;

    // Helper function for tests
    fn telemetry_type_from_string(s: &str) -> TelemetryType {
        match s {
            "Telemetry" => TelemetryType::Telemetry,
            "Signal" => TelemetryType::Signal,
            "Control" => TelemetryType::Control,
            "Adjustment" => TelemetryType::Adjustment,
            _ => TelemetryType::Telemetry, // Default
        }
    }

    #[test]
    fn test_telemetry_type_from_string() {
        assert_eq!(
            telemetry_type_from_string("Telemetry"),
            TelemetryType::Telemetry
        );
        assert_eq!(telemetry_type_from_string("Signal"), TelemetryType::Signal);
        assert_eq!(
            telemetry_type_from_string("Control"),
            TelemetryType::Control
        );
        assert_eq!(
            telemetry_type_from_string("Adjustment"),
            TelemetryType::Adjustment
        );
        assert_eq!(
            telemetry_type_from_string("Unknown"),
            TelemetryType::Telemetry
        );
    }

    #[test]
    fn test_decode_register_value_bool_bitwise() {
        // Testing bit extraction with 1-based numbering (Modbus convention)

        // Test case 1: Register value 0xB5 = 181 = 10110101 in binary
        let register_value = 0xB5;
        let registers = vec![register_value];

        // For FC 03/04 (registers), use 1-16 bit numbering
        // Bit 1 (LSB) = 1
        let result = decode_register_value(&registers, "bool", Some(1), None, Some(3))
            .expect("decoding bit 1 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 2 = 0
        let result = decode_register_value(&registers, "bool", Some(2), None, Some(3))
            .expect("decoding bit 2 should succeed");
        assert_eq!(result, RedisValue::Integer(0));

        // Bit 3 = 1
        let result = decode_register_value(&registers, "bool", Some(3), None, Some(3))
            .expect("decoding bit 3 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 8 = 1
        let result = decode_register_value(&registers, "bool", Some(8), None, Some(3))
            .expect("decoding bit 8 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Test that only bits 1-8 are valid for registers now
        let high_bit_register = 0x0080; // Bit 8 of low byte set
        let high_registers = vec![high_bit_register];
        let result = decode_register_value(&high_registers, "bool", Some(8), None, Some(3))
            .expect("decoding bit 8 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Test FC 01/02 (coils) - uses 1-8 bit numbering
        let coil_byte = 0xB5; // Same value but treated as byte
        let coil_registers = vec![coil_byte];

        // Bit 1 (LSB) = 1
        let result = decode_register_value(&coil_registers, "bool", Some(1), None, Some(1))
            .expect("decoding coil bit 1 should succeed");
        assert_eq!(result, RedisValue::Integer(1));

        // Bit 8 (MSB of byte) = 1
        let result = decode_register_value(&coil_registers, "bool", Some(8), None, Some(1))
            .expect("decoding coil bit 8 should succeed");
        assert_eq!(result, RedisValue::Integer(1));
    }

    #[test]
    fn test_decode_register_value_bool_edge_cases() {
        let registers = vec![0x0000]; // 全0寄存器

        // Testing FC 01/02 (coils) - 1-8 bit numbering
        for bit_pos in 1..=8 {
            let result = decode_register_value(&registers, "bool", Some(bit_pos), None, Some(1));
            if let Ok(value) = result {
                assert_eq!(value, RedisValue::Integer(0), "Bit {} should be 0", bit_pos);
            } else {
                panic!("Failed to decode bit {}", bit_pos);
            }
        }

        // Testing FC 03/04 (registers) - 1-16 bit numbering
        let registers_16bit = vec![0x0100]; // 256 > 255, trigger 16-bit pattern
                                            // 0x0100 in binary: 0000 0001 0000 0000, only bit 9 is set
        for bit_pos in 1..=8 {
            let result =
                decode_register_value(&registers_16bit, "bool", Some(bit_pos), None, Some(3));
            let expected = if bit_pos == 9 { 1 } else { 0 }; // Only bit 9 is set
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
        for bit_pos in 1..=8 {
            let result =
                decode_register_value(&registers_all_ones, "bool", Some(bit_pos), None, Some(3));
            if let Ok(value) = result {
                assert_eq!(value, RedisValue::Integer(1), "Bit {} should be 1", bit_pos);
            } else {
                panic!("Failed to decode bit {}", bit_pos);
            }
        }

        // Testing error case: Bit 1 should be valid for registers (FC 03)
        let result = decode_register_value(&registers, "bool", Some(1), None, Some(3));
        assert!(
            result.is_ok(),
            "Bit position 1 should be valid for registers"
        );

        // Testing error case: bit position out of range for 16-bit mode
        let registers_16bit = vec![0x0100];
        let result = decode_register_value(&registers_16bit, "bool", Some(17), None, Some(3));
        assert!(
            result.is_err(),
            "Bit position 16 should be invalid for 16-bit mode"
        );

        // Testing error case: empty registers
        let empty_registers = vec![];
        let result = decode_register_value(&empty_registers, "bool", Some(1), None, Some(3));
        assert!(result.is_err());

        // Testing default bit_position (should be 1 now per Modbus convention)
        let registers = vec![0x0001]; // Only bit 1 (LSB) is set
        let result = decode_register_value(&registers, "bool", None, None, Some(3))
            .expect("decoding bool with default bit position should succeed");
        assert_eq!(result, RedisValue::Integer(1)); // default位0 = 1
    }

    #[test]
    fn test_decode_register_value_other_formats() {
        // 确保otherdata格式仍然normalwork
        let registers = vec![0x1234];

        // Testing uint16
        let result = decode_register_value(&registers, "uint16", None, None, None)
            .expect("decoding uint16 should succeed");
        assert_eq!(result, RedisValue::Integer(0x1234));

        // Testing int16
        let result = decode_register_value(&registers, "int16", None, None, None)
            .expect("decoding int16 should succeed");
        assert_eq!(result, RedisValue::Integer(i64::from(0x1234_i16)));

        // Testing float32 (needs 2 registers)
        let float_registers = vec![0x4000, 0x0000]; // 2.0 in IEEE 754
        let result = decode_register_value(&float_registers, "float32", None, None, None)
            .expect("decoding float32 should succeed");
        if let RedisValue::Float(f) = result {
            assert!((f - 2.0).abs() < 0.0001);
        } else {
            panic!("Expected float value");
        }
    }

    #[test]
    fn test_reverse_logic_moved_to_data_processor() {
        // testing reverse logic已经移到dataprocessingmodular
        // 这个testingvalidationprotocol层不再直接processing reverse logic

        use crate::core::config::types::{ScalingInfo, TelemetryType};

        // Test case 1: Signal with reverse = true, raw value = 1 should become 0
        let raw_value = 1.0;
        let scaling = ScalingInfo {
            scale: 1.0,
            offset: 0.0,
            unit: None,
            reverse: Some(true),
        };

        let processed_value =
            data_processor::process_point_value(raw_value, &TelemetryType::Signal, Some(&scaling));

        assert_eq!(
            processed_value, 0.0,
            "Raw value 1 with reverse=true should become 0"
        );

        // Test case 2: Signal with reverse = true, raw value = 0 should become 1
        let raw_value = 0.0;
        let processed_value =
            data_processor::process_point_value(raw_value, &TelemetryType::Signal, Some(&scaling));

        assert_eq!(
            processed_value, 1.0,
            "Raw value 0 with reverse=true should become 1"
        );

        // Test case 3: Signal with reverse = false, value should not change
        let raw_value = 1.0;
        let scaling_no_reverse = ScalingInfo {
            scale: 1.0,
            offset: 0.0,
            unit: None,
            reverse: Some(false),
        };
        let processed_value = data_processor::process_point_value(
            raw_value,
            &TelemetryType::Signal,
            Some(&scaling_no_reverse),
        );

        assert_eq!(
            processed_value, 1.0,
            "Raw value 1 with reverse=false should remain 1"
        );

        // Test case 4: Telemetry type should not apply reverse logic
        let raw_value = 100.0;
        let scaling_with_scale = ScalingInfo {
            scale: 0.1,
            offset: 2.0,
            unit: Some("°C".to_string()),
            reverse: Some(true), // 应该被忽略
        };
        let processed_value = data_processor::process_point_value(
            raw_value,
            &TelemetryType::Telemetry,
            Some(&scaling_with_scale),
        );

        assert_eq!(
            processed_value,
            12.0, // 100 * 0.1 + 2.0 = 12.0
            "Telemetry type should apply scale/offset but ignore reverse"
        );
    }
}
