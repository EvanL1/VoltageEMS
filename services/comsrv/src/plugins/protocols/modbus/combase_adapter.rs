//! ComBase Adapter for Unified Modbus Client
//!
//! This module provides an adapter to bridge the unified ModbusClient trait
//! with the ComBase trait required by the plugin system.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::core::combase::{
    ChannelCommand, ChannelStatus, ComBase, PointData, PointDataMap, RedisValue, TelemetryType,
};
use crate::core::config::types::{
    ChannelConfig, TelemetryType as ConfigTelemetryType, UnifiedPointMapping,
};
use crate::core::transport::Transport;
use crate::plugins::core::{DefaultPluginStorage, PluginStorage};
use crate::utils::error::{ComSrvError, Result};

use super::client_impl::ModbusClientImpl;
use super::client_trait::ModbusClient as ModbusClientTrait;
use super::modbus_polling::{ModbusPoint, ModbusPollingConfig};

/// ComBase adapter for unified Modbus client
///
/// This adapter bridges the unified ModbusClient trait with the ComBase trait,
/// allowing the new client implementation to work with the existing plugin system.
pub struct ModbusComBaseAdapter<T: Transport> {
    /// Unified Modbus client implementation
    client: Arc<RwLock<ModbusClientImpl<T>>>,
    /// Channel information
    channel_id: u16,
    channel_name: String,
    /// Protocol type string
    protocol_type: String,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Command receiver for handling remote control commands
    command_rx: Option<tokio::sync::mpsc::Receiver<ChannelCommand>>,
    /// Plugin storage for data persistence
    storage: Arc<Mutex<Option<Arc<dyn PluginStorage>>>>,
    /// Polling configuration
    polling_config: ModbusPollingConfig,
    /// Modbus points to poll
    modbus_points: Vec<ModbusPoint>,
    /// Channel configuration (contains combined_points)
    channel_config: Option<ChannelConfig>,
}

/// Convert config TelemetryType to framework TelemetryType
fn convert_telemetry_type(config_type: &ConfigTelemetryType) -> TelemetryType {
    match config_type {
        ConfigTelemetryType::Telemetry => TelemetryType::Telemetry,
        ConfigTelemetryType::Signal => TelemetryType::Signal,
        ConfigTelemetryType::Control => TelemetryType::Control,
        ConfigTelemetryType::Adjustment => TelemetryType::Adjustment,
    }
}

/// Parse telemetry type from string
fn parse_telemetry_type(type_str: &str) -> ConfigTelemetryType {
    match type_str.to_lowercase().as_str() {
        "telemetry" | "yc" => ConfigTelemetryType::Telemetry,
        "signal" | "yx" => ConfigTelemetryType::Signal,
        "control" | "yk" => ConfigTelemetryType::Control,
        "adjustment" | "yt" => ConfigTelemetryType::Adjustment,
        _ => ConfigTelemetryType::Telemetry, // Default
    }
}

impl<T: Transport + 'static> ModbusComBaseAdapter<T> {
    /// Create a new ComBase adapter
    ///
    /// # Arguments
    /// * `client` - Unified Modbus client implementation
    /// * `channel_id` - Channel identifier
    /// * `channel_name` - Channel name
    /// * `protocol_type` - Protocol type string (e.g., "ModbusTcp", "ModbusRtu")
    ///
    /// # Returns
    /// * `Self` - New adapter instance
    pub fn new(
        client: ModbusClientImpl<T>,
        channel_id: u16,
        channel_name: String,
        protocol_type: String,
    ) -> Self {
        Self {
            client: Arc::new(RwLock::new(client)),
            channel_id,
            channel_name,
            protocol_type,
            running: Arc::new(RwLock::new(false)),
            command_rx: None,
            storage: Arc::new(Mutex::new(None)),
            polling_config: ModbusPollingConfig::default(),
            modbus_points: Vec::new(),
            channel_config: None,
        }
    }

    /// Set channel configuration
    pub fn set_channel_config(&mut self, config: ChannelConfig) {
        self.channel_config = Some(config);
    }

    /// Set polling configuration
    pub fn set_polling_config(&mut self, config: ModbusPollingConfig) {
        self.polling_config = config;
    }

    /// Convert Modbus data to PointData
    fn create_point_data(
        &self,
        point_id: String,
        name: String,
        value: String,
        telemetry_type: TelemetryType,
        unit: String,
        description: String,
    ) -> PointData {
        PointData {
            id: point_id,
            name,
            value,
            timestamp: chrono::Utc::now(),
            unit,
            description,
            telemetry_type: Some(telemetry_type),
            channel_id: Some(self.channel_id),
        }
    }

    /// Handle channel command
    async fn handle_command(&mut self, command: ChannelCommand) -> Result<()> {
        match command {
            ChannelCommand::Control {
                command_id,
                point_id,
                value,
                timestamp: _,
            } => {
                debug!(
                    "[ModbusAdapter] Processing control command {}: point {} = {}",
                    command_id, point_id, value
                );

                // Convert to boolean for coil control
                let coil_value = value != 0.0;

                // Use default slave ID of 1 (should be configurable in real implementation)
                let slave_id = 1u8;

                let mut client = self.client.write().await;
                client
                    .write_single_coil(slave_id, point_id as u16, coil_value)
                    .await
                    .map_err(|e| {
                        error!(
                            "[ModbusAdapter] Failed to execute control command {}: {}",
                            command_id, e
                        );
                        e
                    })?;

                info!(
                    "[ModbusAdapter] Successfully executed control command {}: point {} = {}",
                    command_id, point_id, value
                );
            }
            ChannelCommand::Adjustment {
                command_id,
                point_id,
                value,
                timestamp: _,
            } => {
                debug!(
                    "[ModbusAdapter] Processing adjustment command {}: point {} = {}",
                    command_id, point_id, value
                );

                // Use default slave ID of 1 (should be configurable in real implementation)
                let slave_id = 1u8;

                let mut client = self.client.write().await;
                client
                    .write_single_register(slave_id, point_id as u16, value as u16)
                    .await
                    .map_err(|e| {
                        error!(
                            "[ModbusAdapter] Failed to execute adjustment command {}: {}",
                            command_id, e
                        );
                        e
                    })?;

                info!(
                    "[ModbusAdapter] Successfully executed adjustment command {}: point {} = {}",
                    command_id, point_id, value
                );
            }
        }

        Ok(())
    }

    /// Create Modbus points from channel configuration
    fn create_modbus_points(&self) -> Vec<ModbusPoint> {
        let mut points = Vec::new();

        if let Some(config) = &self.channel_config {
            for point in &config.combined_points {
                // Parse address from protocol_params (format: "slave_id:function_code:register_address")
                if let Some(address) = point.protocol_params.get("address") {
                    let parts: Vec<&str> = address.split(':').collect();
                    if parts.len() >= 3 {
                        if let (Ok(slave_id), Ok(function_code), Ok(register_address)) = (
                            parts[0].parse::<u8>(),
                            parts[1].parse::<u8>(),
                            parts[2].parse::<u16>(),
                        ) {
                            let modbus_point = ModbusPoint {
                                point_id: point.point_id.to_string(),
                                telemetry_type: parse_telemetry_type(&point.telemetry_type),
                                slave_id,
                                function_code,
                                register_address,
                                data_format: point.data_type.clone(),
                                register_count: 1, // Default, could be configured
                                byte_order: None,
                            };
                            points.push(modbus_point);
                        }
                    }
                }
            }
        }

        points
    }

    /// Start polling task
    async fn start_polling_task(&self) {
        let client = self.client.clone();
        let storage = self.storage.clone();
        let channel_id = self.channel_id;
        let channel_name = self.channel_name.clone();
        let running = self.running.clone();
        let polling_interval = Duration::from_millis(self.polling_config.default_interval_ms);
        let modbus_points = self.create_modbus_points();
        debug!(
            "[ModbusAdapter] Created {} modbus points for polling on channel {}",
            modbus_points.len(),
            channel_name
        );

        if modbus_points.is_empty() {
            warn!(
                "[ModbusAdapter] No points configured for polling on channel {}",
                channel_name
            );
            return;
        }

        info!(
            "[ModbusAdapter] Starting polling task for channel {} with {} points, interval: {}ms",
            channel_name,
            modbus_points.len(),
            self.polling_config.default_interval_ms
        );

        tokio::spawn(async move {
            let mut poll_interval = interval(polling_interval);

            while *running.read().await {
                poll_interval.tick().await;

                // Group points by slave_id and function_code for batch reading
                let mut grouped_points: HashMap<(u8, u8), Vec<&ModbusPoint>> = HashMap::new();
                for point in &modbus_points {
                    grouped_points
                        .entry((point.slave_id, point.function_code))
                        .or_insert_with(Vec::new)
                        .push(point);
                }

                for ((slave_id, function_code), points) in grouped_points {
                    // Read data for this group
                    let mut client = client.write().await;

                    match function_code {
                        3 | 4 => {
                            // Read holding/input registers
                            for point in points {
                                match client
                                    .read_holding_registers(
                                        slave_id,
                                        point.register_address,
                                        point.register_count,
                                    )
                                    .await
                                {
                                    Ok(values) => {
                                        if let Some(value) = values.first() {
                                            let raw_value = *value as f64;

                                            // Store raw value to Redis via plugin storage
                                            // 协议层提供原始值和缩放参数，存储层负责计算和存储
                                            if let Some(storage) = &*storage.lock().await {
                                                let framework_type =
                                                    convert_telemetry_type(&point.telemetry_type);
                                                let point_id = point.point_id.parse().unwrap_or(0);

                                                // 查找点位配置中的缩放参数
                                                // 默认不进行缩放
                                                let (scale, offset) = (1.0, 0.0);

                                                let _ = storage
                                                    .write_point_with_scaling(
                                                        channel_id,
                                                        &framework_type,
                                                        point_id,
                                                        raw_value,
                                                        scale,
                                                        offset,
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        debug!(
                                            "[ModbusAdapter] Failed to read register {} from slave {}: {}",
                                            point.register_address, slave_id, e
                                        );
                                    }
                                }
                            }
                        }
                        1 | 2 => {
                            // Read coils/discrete inputs
                            for point in points {
                                match client.read_coils(slave_id, point.register_address, 1).await {
                                    Ok(values) => {
                                        if let Some(value) = values.first() {
                                            let float_value = if *value { 1.0 } else { 0.0 };

                                            // Store to Redis via plugin storage
                                            if let Some(storage) = &*storage.lock().await {
                                                let framework_type =
                                                    convert_telemetry_type(&point.telemetry_type);
                                                let point_id = point.point_id.parse().unwrap_or(0);

                                                // 布尔值通常不需要缩放，但保持接口一致
                                                // 默认不进行缩放
                                                let (scale, offset) = (1.0, 0.0);

                                                let _ = storage
                                                    .write_point_with_scaling(
                                                        channel_id,
                                                        &framework_type,
                                                        point_id,
                                                        float_value,
                                                        scale,
                                                        offset,
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        debug!(
                                            "[ModbusAdapter] Failed to read coil {} from slave {}: {}",
                                            point.register_address, slave_id, e
                                        );
                                    }
                                }
                            }
                        }
                        _ => {
                            warn!(
                                "[ModbusAdapter] Unsupported function code {} for polling",
                                function_code
                            );
                        }
                    }
                }
            }

            info!(
                "[ModbusAdapter] Polling task stopped for channel {}",
                channel_name
            );
        });
    }
}

impl<T: Transport> std::fmt::Debug for ModbusComBaseAdapter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModbusComBaseAdapter")
            .field("channel_id", &self.channel_id)
            .field("channel_name", &self.channel_name)
            .field("protocol_type", &self.protocol_type)
            .field("running", &self.running)
            .finish()
    }
}

#[async_trait]
impl<T: Transport + 'static> ComBase for ModbusComBaseAdapter<T> {
    fn name(&self) -> &str {
        &self.channel_name
    }

    fn protocol_type(&self) -> &str {
        &self.protocol_type
    }

    fn is_connected(&self) -> bool {
        // 使用 block_on 来同步访问 async 状态
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.client.read().await.is_connected().await })
        })
    }

    async fn initialize(&mut self, _channel_config: &ChannelConfig) -> Result<()> {
        // Initialize already done in constructor
        Ok(())
    }

    async fn connect(&mut self) -> Result<()> {
        info!(
            "[ModbusAdapter] Starting Modbus client for channel {}",
            self.channel_name
        );

        // Connect the underlying Modbus client
        {
            let mut client = self.client.write().await;
            info!("[ModbusAdapter] DEBUG: About to call client.connect()");
            client.connect().await.map_err(|e| {
                error!("[ModbusAdapter] Failed to connect: {}", e);
                e
            })?;
            info!("[ModbusAdapter] DEBUG: client.connect() completed successfully");
        }

        info!("[ModbusAdapter] DEBUG: Connection completed, continuing with startup");

        // Initialize storage asynchronously (don't block on Redis connection)
        info!(
            "[ModbusAdapter] Initializing storage for channel {}",
            self.channel_name
        );
        let storage_clone = self.storage.clone();
        let channel_name_clone = self.channel_name.clone();
        tokio::spawn(async move {
            info!(
                "[ModbusAdapter] Starting Redis connection for channel {} with 5s timeout",
                channel_name_clone
            );

            // Add retry mechanism for Redis connection
            let mut retry_count = 0;
            const MAX_RETRIES: u32 = 3;
            let mut base_delay = std::time::Duration::from_secs(1);

            loop {
                let redis_timeout = std::time::Duration::from_secs(5);
                let redis_task = DefaultPluginStorage::from_env();

                match tokio::time::timeout(redis_timeout, redis_task).await {
                    Ok(Ok(s)) => {
                        let mut storage = storage_clone.lock().await;
                        *storage = Some(Arc::new(s) as Arc<dyn PluginStorage>);
                        info!(
                            "[ModbusAdapter] Plugin storage initialized for channel {} after {} retries",
                            channel_name_clone, retry_count
                        );
                        break;
                    }
                    Ok(Err(e)) => {
                        error!(
                            "[ModbusAdapter] Failed to create storage for channel {} (attempt {}/{}): {}",
                            channel_name_clone, retry_count + 1, MAX_RETRIES, e
                        );
                    }
                    Err(_) => {
                        error!(
                            "[ModbusAdapter] Redis connection timed out for channel {} (attempt {}/{})",
                            channel_name_clone, retry_count + 1, MAX_RETRIES
                        );
                    }
                }

                retry_count += 1;
                if retry_count >= MAX_RETRIES {
                    warn!(
                        "[ModbusAdapter] Failed to connect to Redis after {} attempts for channel {}, data will not be persisted",
                        MAX_RETRIES, channel_name_clone
                    );
                    break;
                }

                // Exponential backoff
                info!(
                    "[ModbusAdapter] Retrying Redis connection for channel {} in {:?}...",
                    channel_name_clone, base_delay
                );
                tokio::time::sleep(base_delay).await;
                base_delay *= 2; // Double the delay for next retry
            }
        });

        // Mark as running
        info!(
            "[ModbusAdapter] Marking channel {} as running",
            self.channel_name
        );
        *self.running.write().await = true;

        // Start polling task (it spawns its own background task)
        info!(
            "[ModbusAdapter] Starting polling task for channel {}",
            self.channel_name
        );
        self.start_polling_task().await;
        info!(
            "[ModbusAdapter] Polling task started for channel {}",
            self.channel_name
        );

        // Start command processing task if command receiver is available
        if let Some(mut command_rx) = self.command_rx.take() {
            let client_clone = self.client.clone();
            let running_clone = self.running.clone();
            let channel_name = self.channel_name.clone();

            tokio::spawn(async move {
                info!(
                    "[ModbusAdapter] Starting command processing for channel {}",
                    channel_name
                );

                while *running_clone.read().await {
                    // Use timeout to avoid blocking indefinitely
                    match tokio::time::timeout(std::time::Duration::from_secs(1), command_rx.recv())
                        .await
                    {
                        Ok(Some(command)) => {
                            debug!("[ModbusAdapter] Received command: {:?}", command);

                            // Handle the command
                            match command {
                                ChannelCommand::Control {
                                    command_id,
                                    point_id,
                                    value,
                                    ..
                                } => {
                                    let mut client = client_clone.write().await;
                                    let result = client
                                        .write_single_coil(1, point_id as u16, value != 0.0)
                                        .await;

                                    match result {
                                        Ok(_) => info!("[ModbusAdapter] Control command {} executed successfully", command_id),
                                        Err(e) => error!("[ModbusAdapter] Control command {} failed: {}", command_id, e),
                                    }
                                }
                                ChannelCommand::Adjustment {
                                    command_id,
                                    point_id,
                                    value,
                                    ..
                                } => {
                                    let mut client = client_clone.write().await;
                                    let result = client
                                        .write_single_register(1, point_id as u16, value as u16)
                                        .await;

                                    match result {
                                        Ok(_) => info!("[ModbusAdapter] Adjustment command {} executed successfully", command_id),
                                        Err(e) => error!("[ModbusAdapter] Adjustment command {} failed: {}", command_id, e),
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            debug!(
                                "[ModbusAdapter] Command channel closed for channel {}",
                                channel_name
                            );
                            break;
                        }
                        Err(_) => {
                            // Timeout - continue loop
                            continue;
                        }
                    }
                }

                info!(
                    "[ModbusAdapter] Command processing stopped for channel {}",
                    channel_name
                );
            });
        }

        info!(
            "[ModbusAdapter] Modbus client started successfully for channel {}",
            self.channel_name
        );
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!(
            "[ModbusAdapter] Stopping Modbus client for channel {}",
            self.channel_name
        );

        // Mark as not running
        *self.running.write().await = false;

        // Disconnect the underlying Modbus client
        {
            let mut client = self.client.write().await;
            client.disconnect().await.map_err(|e| {
                error!("[ModbusAdapter] Failed to disconnect: {}", e);
                e
            })?;
        }

        info!(
            "[ModbusAdapter] Modbus client stopped successfully for channel {}",
            self.channel_name
        );
        Ok(())
    }

    async fn get_status(&self) -> ChannelStatus {
        let client = self.client.read().await;
        let diagnostics = client.get_diagnostics().await;
        let connected = client.is_connected().await;

        ChannelStatus {
            id: self.channel_id.to_string(),
            connected,
            last_response_time: diagnostics
                .get("avg_response_time_ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            last_error: if connected {
                String::new()
            } else {
                "Not connected".to_string()
            },
            last_update_time: chrono::Utc::now(),
            error_count: diagnostics
                .get("failed_requests")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        }
    }

    async fn read_four_telemetry(&self, telemetry_type: &str) -> Result<PointDataMap> {
        // 实现四遥数据读取
        let mut data_map = PointDataMap::new();

        // 根据 telemetry_type 读取相应数据
        match telemetry_type {
            "m" | "telemetry" => {
                // 读取遥测数据
                // TODO: 实现实际的 Modbus 读取逻辑
            }
            "s" | "signal" => {
                // 读取遥信数据
            }
            "c" | "control" => {
                // 读取遥控状态
            }
            "a" | "adjustment" => {
                // 读取遥调数据
            }
            _ => {
                return Err(ComSrvError::InvalidParameter(format!(
                    "Unknown telemetry type: {}",
                    telemetry_type
                )));
            }
        }

        Ok(data_map)
    }

    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        let mut results = Vec::new();
        let mut client = self.client.write().await;

        for (point_id, value) in commands {
            // Convert RedisValue to bool for coil control
            let coil_value = match value {
                RedisValue::Bool(b) => b,
                RedisValue::Integer(i) => i != 0,
                RedisValue::Float(f) => f != 0.0,
                RedisValue::String(s) => s == "true" || s == "1",
                RedisValue::Null => false,
            };

            // Use default slave ID of 1 (should be configurable)
            let slave_id = 1u8;

            match client
                .write_single_coil(slave_id, point_id as u16, coil_value)
                .await
            {
                Ok(_) => results.push((point_id, true)),
                Err(_) => results.push((point_id, false)),
            }
        }

        Ok(results)
    }

    async fn adjustment(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        let mut results = Vec::new();
        let mut client = self.client.write().await;

        for (point_id, value) in adjustments {
            // Convert RedisValue to u16 for register write
            let register_value = match value {
                RedisValue::Integer(i) => i as u16,
                RedisValue::Float(f) => f as u16,
                RedisValue::String(s) => s.parse::<u16>().unwrap_or(0),
                _ => 0,
            };

            // Use default slave ID of 1 (should be configurable)
            let slave_id = 1u8;

            match client
                .write_single_register(slave_id, point_id as u16, register_value)
                .await
            {
                Ok(_) => results.push((point_id, true)),
                Err(_) => results.push((point_id, false)),
            }
        }

        Ok(results)
    }

    async fn update_points(&mut self, _mappings: Vec<UnifiedPointMapping>) -> Result<()> {
        // TODO: 实现点位更新逻辑
        Ok(())
    }
}
