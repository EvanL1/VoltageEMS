//! Modbus 协议核心实现
//!
//! 集成了协议处理、轮询机制和批量优化功能
//! 注意：当前版本为临时实现，专注于编译通过

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::core::combase::{
    ChannelCommand, ChannelStatus, ComBase, CommandSubscriber, CommandSubscriberConfig, PointData,
    PointDataMap, RedisValue, TelemetryType,
};
use crate::core::config::types::ChannelConfig;
use crate::plugins::core::PluginStorage;
use crate::utils::error::{ComSrvError, Result};

use super::connection::{ConnectionParams, ModbusConnectionManager, ModbusMode as ConnectionMode};
use super::transport::{ModbusFrameProcessor, ModbusMode};
use super::types::{ModbusPoint, ModbusPollingConfig};

/// 将字符串遥测类型转换为TelemetryType枚举
fn telemetry_type_from_string(type_str: &str) -> TelemetryType {
    match type_str {
        "Measurement" => TelemetryType::Measurement, // 遥测 → "m"
        "Signal" => TelemetryType::Signal,           // 遥信 → "s"
        "Control" => TelemetryType::Control,         // 遥控 → "c"
        "Adjustment" => TelemetryType::Adjustment,   // 遥调 → "a"
        _ => {
            debug!(
                "Unknown telemetry type '{}', defaulting to Telemetry",
                type_str
            );
            TelemetryType::Measurement // 默认值
        }
    }
}

/// Modbus 协议核心引擎
pub struct ModbusCore {
    /// 帧处理器
    frame_processor: ModbusFrameProcessor,
    /// 轮询配置
    _polling_config: ModbusPollingConfig,
    /// 点位映射表
    _points: HashMap<String, ModbusPoint>,
}

impl ModbusCore {
    /// 创建新的 Modbus 核心引擎
    pub fn new(mode: ModbusMode, polling_config: ModbusPollingConfig) -> Self {
        Self {
            frame_processor: ModbusFrameProcessor::new(mode),
            _polling_config: polling_config,
            _points: HashMap::new(),
        }
    }

    /// 设置点位映射表
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

    // TODO: 实现完整的轮询和批量读取功能
    // 当前暂时注释掉复杂的实现以通过编译
}

/// Modbus 协议实现，实现 ComBase trait
pub struct ModbusProtocol {
    /// 协议名称
    name: String,
    /// 通道ID
    channel_id: u16,
    /// 通道配置
    channel_config: Option<ChannelConfig>,

    /// 核心组件
    core: Arc<Mutex<ModbusCore>>,
    connection_manager: Arc<ModbusConnectionManager>,
    storage: Arc<Mutex<Option<Arc<dyn PluginStorage>>>>,

    /// 命令处理
    command_subscriber: Option<CommandSubscriber>,
    command_rx: Option<mpsc::Receiver<ChannelCommand>>,

    /// 状态管理
    is_connected: Arc<RwLock<bool>>,
    status: Arc<RwLock<ChannelStatus>>,

    /// 任务管理
    polling_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    command_handle: Arc<RwLock<Option<JoinHandle<()>>>>,

    /// 轮询配置
    polling_config: ModbusPollingConfig,
    /// 点位映射
    points: Arc<RwLock<Vec<ModbusPoint>>>,
}

impl ModbusProtocol {
    /// 创建新的 Modbus 协议实例
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

        let core = ModbusCore::new(mode, polling_config.clone());
        let connection_manager =
            Arc::new(ModbusConnectionManager::new(conn_mode, connection_params));

        Ok(Self {
            name: channel_config.name.clone(),
            channel_id: channel_config.id,
            channel_config: Some(channel_config),
            core: Arc::new(Mutex::new(core)),
            connection_manager,
            storage: Arc::new(Mutex::new(None)),
            command_subscriber: None,
            command_rx: None,
            is_connected: Arc::new(RwLock::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::default())),
            polling_handle: Arc::new(RwLock::new(None)),
            command_handle: Arc::new(RwLock::new(None)),
            polling_config,
            points: Arc::new(RwLock::new(Vec::new())),
        })
    }
}

#[async_trait]
impl ComBase for ModbusProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol_type(&self) -> &str {
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

        // Step 2: 初始化存储
        info!(
            "Channel {} - Step 2: Initializing storage",
            channel_config.id
        );
        match crate::plugins::core::DefaultPluginStorage::from_env().await {
            Ok(storage) => {
                *self.storage.lock().await = Some(Arc::new(storage) as Arc<dyn PluginStorage>);
                info!(
                    "Channel {} - Step 2 completed: Storage initialized successfully",
                    self.channel_id
                );
            }
            Err(e) => {
                error!(
                    "Channel {} - Step 2 failed: Storage initialization error: {}",
                    self.channel_id, e
                );
            }
        }

        // Step 3: 加载和解析点位配置
        info!(
            "Channel {} - Step 3: Loading point configurations",
            channel_config.id
        );
        let mut modbus_points = Vec::new();

        // 合并所有四种遥测类型的点位进行处理
        let all_points = vec![
            &channel_config.measurement_points,
            &channel_config.signal_points,
            &channel_config.control_points,
            &channel_config.adjustment_points,
        ];

        let total_configured_points = channel_config.measurement_points.len()
            + channel_config.signal_points.len()
            + channel_config.control_points.len()
            + channel_config.adjustment_points.len();

        info!("Channel {} - Step 3: Processing {} configured points ({} measurement, {} signal, {} control, {} adjustment)", 
            channel_config.id,
            total_configured_points,
            channel_config.measurement_points.len(),
            channel_config.signal_points.len(),
            channel_config.control_points.len(),
            channel_config.adjustment_points.len()
        );

        for point_map in all_points {
            for point in point_map.values() {
                // 直接从protocol_params中读取各个字段
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
                        let data_format = point
                            .protocol_params
                            .get("data_format")
                            .unwrap_or(&"uint16".to_string())
                            .clone();

                        let modbus_point = ModbusPoint {
                            point_id: point.point_id.to_string(),
                            slave_id,
                            function_code,
                            register_address,
                            data_format: data_format.clone(),
                            register_count: point
                                .protocol_params
                                .get("register_count")
                                .and_then(|v| v.parse::<u16>().ok())
                                .unwrap_or(1),
                            byte_order: point.protocol_params.get("byte_order").cloned(),
                        };

                        debug!(
                            "Loaded Modbus point: id={}, slave={}, func={}, addr={}, format={}, bit_pos={:?}",
                            point.point_id,
                            slave_id,
                            function_code,
                            register_address,
                            data_format,
                            point.protocol_params.get("bit_position")
                        );

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

        // Step 4: 设置点位到核心和本地存储
        info!(
            "Channel {} - Step 4: Setting up {} points in storage",
            channel_config.id,
            modbus_points.len()
        );
        {
            let mut core = self.core.lock().await;
            core.set_points(modbus_points.clone());
        }
        *self.points.write().await = modbus_points.clone();

        self.status.write().await.points_count = self.points.read().await.len();

        info!(
            "Channel {} - Step 4 completed: Successfully processed {} out of {} configured points for Modbus protocol",
            channel_config.id,
            modbus_points.len(),
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

        // 建立连接
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

        // 创建命令订阅器
        if let Some(ref config) = self.channel_config {
            if let Some(redis_url) = config.parameters.get("redis_url").and_then(|v| v.as_str()) {
                let (cmd_tx, cmd_rx) = mpsc::channel(100);
                let subscriber = CommandSubscriber::new(
                    CommandSubscriberConfig {
                        channel_id: self.channel_id,
                        redis_url: redis_url.to_string(),
                    },
                    cmd_tx,
                )
                .await?;

                self.command_subscriber = Some(subscriber);
                self.command_rx = Some(cmd_rx);
            }
        }

        // 启动周期性任务（轮询等）
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

        // 停止所有任务
        self.stop_periodic_tasks().await?;

        // 断开连接
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

        // 根据遥测类型过滤点位
        let points = self.points.read().await;
        let channel_config = self
            .channel_config
            .as_ref()
            .ok_or_else(|| ComSrvError::config("Channel configuration not initialized"))?;

        for point in points.iter() {
            // 根据遥测类型从对应的HashMap中查找点位
            if let Ok(point_id) = point.point_id.parse::<u32>() {
                // 根据telemetry_type选择正确的HashMap
                let config_point = match telemetry_type {
                    "Measurement" => channel_config.measurement_points.get(&point_id),
                    "Signal" => channel_config.signal_points.get(&point_id),
                    "Control" => channel_config.control_points.get(&point_id),
                    "Adjustment" => channel_config.adjustment_points.get(&point_id),
                    _ => None,
                };

                if let Some(config_point) = config_point {
                    // TODO: 实际的 Modbus 读取逻辑
                    // 这里暂时返回模拟数据
                    let value = RedisValue::Float(rand::random::<f64>() * 100.0);
                    let point_data = PointData {
                        value,
                        timestamp: chrono::Utc::now().timestamp() as u64,
                    };
                    result.insert(config_point.point_id, point_data);
                }
            }
        }

        // 更新状态
        self.status.write().await.last_update = chrono::Utc::now().timestamp() as u64;
        self.status.write().await.success_count += 1;

        Ok(result)
    }

    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        let mut results = Vec::new();

        for (point_id, value) in commands {
            // TODO: 实际的 Modbus 写操作
            debug!(
                "Executing control command: point {}, value {:?}",
                point_id, value
            );
            results.push((point_id, true));
        }

        Ok(results)
    }

    async fn adjustment(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        if !self.is_connected() {
            return Err(ComSrvError::NotConnected);
        }

        let mut results = Vec::new();

        for (point_id, value) in adjustments {
            // TODO: 实际的 Modbus 写操作
            debug!(
                "Executing adjustment command: point {}, value {:?}",
                point_id, value
            );
            results.push((point_id, true));
        }

        Ok(results)
    }

    // 四遥分离架构下，update_points方法已移除，点位配置在initialize阶段直接加载

    async fn start_periodic_tasks(&self) -> Result<()> {
        info!(
            "Starting Modbus periodic tasks for channel {}",
            self.channel_id
        );

        // 启动命令订阅
        if let Some(ref _subscriber) = &self.command_subscriber {
            // 命令订阅将在协议初始化时启动
            debug!("Command subscriber ready for channel {}", self.channel_id);
        }

        // 启动轮询任务
        if self.polling_config.enabled {
            let channel_id = self.channel_id;
            let polling_interval = self.polling_config.default_interval_ms;
            let connection_manager = self.connection_manager.clone();
            let points = self.points.clone();
            let status = self.status.clone();
            let storage = self.storage.clone();
            let is_connected = self.is_connected.clone();
            let channel_config = self.channel_config.clone();
            let polling_config_clone = self.polling_config.clone();

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

                    // Read all configured points
                    let points_to_read = points.read().await.clone();
                    if points_to_read.is_empty() {
                        debug!("No points configured for channel {}", channel_id);
                        continue;
                    }

                    let mut success_count = 0;
                    let mut error_count = 0;

                    // Filter points to only read Measurement and Signal types
                    // Control and Adjustment should only be written via command channels
                    let points_count = points_to_read.len();
                    let filtered_points: Vec<ModbusPoint> = if let Some(ref config) = channel_config
                    {
                        points_to_read
                            .into_iter()
                            .filter(|point| {
                                if let Ok(point_id) = point.point_id.parse::<u32>() {
                                    // 检查点位是否在 measurement_points 或 signal_points 中
                                    // 只允许遥测和遥信类型进行轮询
                                    if config.measurement_points.contains_key(&point_id) {
                                        true
                                    } else if config.signal_points.contains_key(&point_id) {
                                        true
                                    } else if config.control_points.contains_key(&point_id)
                                        || config.adjustment_points.contains_key(&point_id)
                                    {
                                        // 遥控和遥调不允许轮询读取
                                        false
                                    } else {
                                        // If not found in any config, default to allow reading
                                        debug!(
                                            "Point {} not found in config, allowing read",
                                            point_id
                                        );
                                        true
                                    }
                                } else {
                                    debug!("Invalid point_id format: {}, skipping", point.point_id);
                                    false
                                }
                            })
                            .collect()
                    } else {
                        // If no config available, read all points (legacy behavior)
                        debug!("No channel config available, reading all points");
                        points_to_read
                    };

                    if filtered_points.len() != points_count {
                        debug!(
                            "Filtered polling points: {} → {} (skipped Control/Adjustment types)",
                            points_count,
                            filtered_points.len()
                        );
                    }

                    // Group filtered points by slave ID and function code for batch reading
                    let mut grouped_points: HashMap<(u8, u8), Vec<ModbusPoint>> = HashMap::new();
                    for point in filtered_points {
                        let key = (point.slave_id, point.function_code);
                        grouped_points.entry(key).or_default().push(point);
                    }

                    // Read each group
                    for ((slave_id, function_code), group_points) in grouped_points {
                        // Create a temporary frame processor for this connection
                        let mode = match connection_manager.mode() {
                            ConnectionMode::Tcp => ModbusMode::Tcp,
                            ConnectionMode::Rtu => ModbusMode::Rtu,
                        };
                        let mut frame_processor = ModbusFrameProcessor::new(mode);

                        // Get max_batch_size from polling config, default to 100
                        let max_batch_size = polling_config_clone.batch_config.max_batch_size;

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

                                // Store values if storage is available
                                if let Some(storage_ref) = &*storage.lock().await {
                                    debug!("Storage available, storing {} values", values.len());
                                    for (point_id_str, value) in values {
                                        // Convert point_id from string to u32
                                        if let Ok(point_id) = point_id_str.parse::<u32>() {
                                            // Convert RedisValue to f64
                                            let float_value = match value {
                                                RedisValue::Float(f) => f,
                                                RedisValue::Integer(i) => i as f64,
                                                _ => continue, // Skip non-numeric values
                                            };

                                            // Get point configuration and apply data processing
                                            let (telemetry_type, processed_value) = if let Some(
                                                ref config,
                                            ) =
                                                channel_config
                                            {
                                                // 从对应的HashMap中查找点位配置
                                                let config_point = if let Some(point) =
                                                    config.measurement_points.get(&point_id)
                                                {
                                                    Some((point, TelemetryType::Measurement))
                                                } else if let Some(point) =
                                                    config.signal_points.get(&point_id)
                                                {
                                                    Some((point, TelemetryType::Signal))
                                                } else if let Some(point) =
                                                    config.control_points.get(&point_id)
                                                {
                                                    Some((point, TelemetryType::Control))
                                                } else {
                                                    config.adjustment_points.get(&point_id).map(
                                                        |point| (point, TelemetryType::Adjustment),
                                                    )
                                                };

                                                if let Some((config_point, telemetry_type)) =
                                                    config_point
                                                {
                                                    // Apply scaling parameters if configured
                                                    let processed_value = if let Some(ref scaling) =
                                                        config_point.scaling
                                                    {
                                                        let mut value = float_value;

                                                        // Apply scale and offset: processed = raw * scale + offset
                                                        value =
                                                            value * scaling.scale + scaling.offset;

                                                        debug!("Applied scaling to point {}: raw={:.6}, scale={:.6}, offset={:.6}, processed={:.6}", 
                                                               point_id, float_value, scaling.scale, scaling.offset, value);
                                                        value
                                                    } else {
                                                        // No scaling configured, use raw value
                                                        float_value
                                                    };

                                                    (telemetry_type, processed_value)
                                                } else {
                                                    debug!("Point {} not found in config, using default Measurement", point_id);
                                                    (TelemetryType::Measurement, float_value)
                                                }
                                            } else {
                                                debug!("No channel config available, using default Measurement");
                                                (TelemetryType::Measurement, float_value)
                                            };

                                            // Store the processed value with raw value metadata
                                            if let Err(e) = storage_ref
                                                .write_point_with_raw(
                                                    channel_id,
                                                    &telemetry_type,
                                                    point_id,
                                                    processed_value,
                                                    float_value, // Keep original raw value for reference
                                                )
                                                .await
                                            {
                                                error!(
                                                    "Failed to store point {} value: {}",
                                                    point_id, e
                                                );
                                            }
                                        }
                                    }
                                } else {
                                    error!("Storage not initialized for channel {}, skipping data storage", channel_id);
                                }
                            }
                            Err(e) => {
                                error_count += group_points.len();
                                error!(
                                    "Failed to read modbus group (slave={}, func={}): {}",
                                    slave_id, function_code, e
                                );
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

        // 停止命令订阅
        if let Some(ref _subscriber) = &self.command_subscriber {
            // 命令订阅将在协议停止时自动清理
            debug!(
                "Cleaning up command subscriber for channel {}",
                self.channel_id
            );
        }

        // 停止轮询任务
        if let Some(handle) = self.polling_handle.write().await.take() {
            handle.abort();
            info!("Polling task stopped for channel {}", self.channel_id);
        }

        // 停止命令处理任务
        if let Some(handle) = self.command_handle.write().await.take() {
            handle.abort();
            info!(
                "Command handler task stopped for channel {}",
                self.channel_id
            );
        }

        Ok(())
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
    let mut sorted_points = points.to_vec();
    sorted_points.sort_by_key(|p| p.register_address);

    let mut results = Vec::new();
    let mut current_batch = Vec::new();
    let mut batch_start_address = sorted_points[0].register_address;

    for point in sorted_points {
        // Check if this point can be added to the current batch
        let gap = point.register_address.saturating_sub(
            batch_start_address + current_batch.len() as u16 * point.register_count,
        );

        // Calculate the total registers in current batch if we add this point
        let batch_end_if_added = point.register_address + point.register_count;
        let batch_registers_if_added = (batch_end_if_added - batch_start_address) as usize;

        // Check both gap and batch size constraints
        if current_batch.is_empty()
            || (gap <= 5 && batch_registers_if_added <= max_batch_size as usize)
        {
            current_batch.push(point.clone());
        } else {
            // Read current batch
            let batch_results = read_modbus_batch(
                connection_manager,
                frame_processor,
                slave_id,
                function_code,
                batch_start_address,
                &current_batch,
                channel_config,
                max_batch_size,
            )
            .await?;
            results.extend(batch_results);

            // Start new batch
            current_batch.clear();
            current_batch.push(point.clone());
            batch_start_address = point.register_address;
        }
    }

    // Read final batch
    if !current_batch.is_empty() {
        let batch_results = read_modbus_batch(
            connection_manager,
            frame_processor,
            slave_id,
            function_code,
            batch_start_address,
            &current_batch,
            channel_config,
            max_batch_size,
        )
        .await?;
        results.extend(batch_results);
    }

    Ok(results)
}

/// Read a batch of consecutive Modbus registers with automatic splitting for large batches
async fn read_modbus_batch(
    connection_manager: &Arc<ModbusConnectionManager>,
    frame_processor: &mut ModbusFrameProcessor,
    slave_id: u8,
    function_code: u8,
    start_address: u16,
    points: &[ModbusPoint],
    channel_config: Option<&ChannelConfig>,
    max_batch_size: u16,
) -> Result<Vec<(String, RedisValue)>> {
    if points.is_empty() {
        return Ok(Vec::new());
    }

    // Calculate total registers to read
    let last_point = points.last().unwrap();
    let total_registers =
        (last_point.register_address - start_address + last_point.register_count) as usize;

    // Collect all register values by reading in batches
    let mut all_register_values = Vec::new();
    let mut current_offset = 0;

    // Read registers in chunks no larger than max_batch_size
    while current_offset < total_registers {
        let batch_size = std::cmp::min(max_batch_size as usize, total_registers - current_offset);
        let batch_start = start_address + current_offset as u16;

        debug!(
            "Reading Modbus batch: slave={}, func={}, start={}, count={} (offset={}/{})",
            slave_id, function_code, batch_start, batch_size, current_offset, total_registers
        );

        // Build Modbus PDU for this batch
        let pdu = match function_code {
            3 => build_read_holding_registers_pdu(batch_start, batch_size as u16),
            4 => build_read_input_registers_pdu(batch_start, batch_size as u16),
            _ => {
                return Err(ComSrvError::ProtocolError(format!(
                    "Unsupported function code: {}",
                    function_code
                )))
            }
        };

        // Build complete frame with proper header (MBAP for TCP, CRC for RTU)
        let request = frame_processor.build_frame(slave_id, &pdu);

        // Send request and receive response
        connection_manager.send(&request).await?;

        let mut response = vec![0u8; 256]; // Maximum Modbus frame size
        let bytes_read = connection_manager
            .receive(&mut response, Duration::from_secs(5))
            .await?;
        response.truncate(bytes_read);

        // Parse response frame
        let (received_unit_id, pdu) = frame_processor.parse_frame(&response)?;

        // Verify unit ID matches
        if received_unit_id != slave_id {
            return Err(ComSrvError::ProtocolError(format!(
                "Unit ID mismatch: expected {}, got {}",
                slave_id, received_unit_id
            )));
        }

        // Parse PDU to extract register values for this batch
        let batch_register_values = parse_modbus_pdu(&pdu, function_code)?;

        // Verify we received the expected number of registers
        if batch_register_values.len() != batch_size {
            warn!(
                "Received {} registers, expected {} for batch at address {}",
                batch_register_values.len(),
                batch_size,
                batch_start
            );
        }

        // Append to our complete register collection
        all_register_values.extend(batch_register_values);
        current_offset += batch_size;
    }

    // Extract values for each point from the complete register collection
    let mut results = Vec::new();
    for point in points {
        let offset = (point.register_address - start_address) as usize;
        if offset + point.register_count as usize <= all_register_values.len() {
            let registers = &all_register_values[offset..offset + point.register_count as usize];

            // Get bit_position from channel configuration if available
            let bit_position = if let Some(config) = channel_config {
                if let Ok(point_id) = point.point_id.parse::<u32>() {
                    // 从四个HashMap中查找点位配置
                    let config_point = config
                        .measurement_points
                        .get(&point_id)
                        .or_else(|| config.signal_points.get(&point_id))
                        .or_else(|| config.control_points.get(&point_id))
                        .or_else(|| config.adjustment_points.get(&point_id));

                    config_point.and_then(|config_point| {
                        config_point
                            .protocol_params
                            .get("bit_position")
                            .and_then(|pos_str| pos_str.parse::<u8>().ok())
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let value = decode_register_value(registers, &point.data_format, bit_position)?;
            results.push((point.point_id.clone(), value));
        } else {
            warn!(
                "Point {} at address {} is out of range (offset={}, registers_available={})",
                point.point_id,
                point.register_address,
                offset,
                all_register_values.len()
            );
        }
    }

    Ok(results)
}

/// Build Modbus PDU for reading holding registers (FC 3)
fn build_read_holding_registers_pdu(start_address: u16, quantity: u16) -> Vec<u8> {
    vec![
        0x03, // Function code
        (start_address >> 8) as u8,
        (start_address & 0xFF) as u8,
        (quantity >> 8) as u8,
        (quantity & 0xFF) as u8,
    ]
}

/// Build Modbus PDU for reading input registers (FC 4)
fn build_read_input_registers_pdu(start_address: u16, quantity: u16) -> Vec<u8> {
    vec![
        0x04, // Function code
        (start_address >> 8) as u8,
        (start_address & 0xFF) as u8,
        (quantity >> 8) as u8,
        (quantity & 0xFF) as u8,
    ]
}

/// Parse Modbus PDU and extract register values
fn parse_modbus_pdu(pdu: &[u8], function_code: u8) -> Result<Vec<u16>> {
    if pdu.len() < 3 {
        return Err(ComSrvError::ProtocolError("PDU too short".to_string()));
    }

    if pdu[0] != function_code {
        return Err(ComSrvError::ProtocolError(format!(
            "Function code mismatch: expected {}, got {}",
            function_code, pdu[0]
        )));
    }

    let byte_count = pdu[1] as usize;
    if pdu.len() < 2 + byte_count {
        return Err(ComSrvError::ProtocolError(
            "Incomplete PDU data".to_string(),
        ));
    }

    let mut registers = Vec::new();
    for i in (2..2 + byte_count).step_by(2) {
        let value = ((pdu[i] as u16) << 8) | (pdu[i + 1] as u16);
        registers.push(value);
    }

    Ok(registers)
}

/// Decode register values based on data format
fn decode_register_value(
    registers: &[u16],
    format: &str,
    bit_position: Option<u8>,
) -> Result<RedisValue> {
    match format {
        "bool" => {
            if registers.is_empty() {
                return Err(ComSrvError::ProtocolError(
                    "No registers for bool".to_string(),
                ));
            }
            let bit_pos = bit_position.unwrap_or(0);
            if bit_pos > 15 {
                return Err(ComSrvError::ProtocolError(format!(
                    "Invalid bit position: {} (must be 0-15)",
                    bit_pos
                )));
            }
            // Extract bit from register: bit 0 is LSB, bit 15 is MSB
            let register_value = registers[0];
            let bit_value = (register_value >> bit_pos) & 0x01;
            debug!(
                "Bit extraction: register=0x{:04X}, bit_pos={}, bit_value={}",
                register_value, bit_pos, bit_value
            );
            Ok(RedisValue::Integer(bit_value as i64))
        }
        "uint16" => {
            if registers.is_empty() {
                return Err(ComSrvError::ProtocolError(
                    "No registers for uint16".to_string(),
                ));
            }
            Ok(RedisValue::Integer(registers[0] as i64))
        }
        "int16" => {
            if registers.is_empty() {
                return Err(ComSrvError::ProtocolError(
                    "No registers for int16".to_string(),
                ));
            }
            Ok(RedisValue::Integer(registers[0] as i16 as i64))
        }
        "uint32" | "uint32_be" => {
            if registers.len() < 2 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for uint32".to_string(),
                ));
            }
            let value = ((registers[0] as u32) << 16) | (registers[1] as u32);
            Ok(RedisValue::Integer(value as i64))
        }
        "float32" | "float32_be" => {
            if registers.len() < 2 {
                return Err(ComSrvError::ProtocolError(
                    "Not enough registers for float32".to_string(),
                ));
            }
            let bytes = [
                (registers[0] >> 8) as u8,
                (registers[0] & 0xFF) as u8,
                (registers[1] >> 8) as u8,
                (registers[1] & 0xFF) as u8,
            ];
            let value = f32::from_be_bytes(bytes);
            Ok(RedisValue::Float(value as f64))
        }
        _ => Err(ComSrvError::ProtocolError(format!(
            "Unsupported data format: {}",
            format
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_type_from_string() {
        assert_eq!(
            telemetry_type_from_string("Measurement"),
            TelemetryType::Measurement
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
            TelemetryType::Measurement
        );
    }

    #[test]
    fn test_decode_register_value_bool_bitwise() {
        // 测试按位解析功能

        // 测试案例1：寄存器值 0b1011 0101 (0xB5 = 181)
        // 位0: 1, 位1: 0, 位2: 1, 位3: 0, 位4: 1, 位5: 1, 位6: 0, 位7: 1
        let register_value = 0xB5; // 181 in decimal, 10110101 in binary
        let registers = vec![register_value];

        // 测试位0 (LSB)
        let result = decode_register_value(&registers, "bool", Some(0)).unwrap();
        assert_eq!(result, RedisValue::Integer(1)); // 位0 = 1

        // 测试位1
        let result = decode_register_value(&registers, "bool", Some(1)).unwrap();
        assert_eq!(result, RedisValue::Integer(0)); // 位1 = 0

        // 测试位2
        let result = decode_register_value(&registers, "bool", Some(2)).unwrap();
        assert_eq!(result, RedisValue::Integer(1)); // 位2 = 1

        // 测试位3
        let result = decode_register_value(&registers, "bool", Some(3)).unwrap();
        assert_eq!(result, RedisValue::Integer(0)); // 位3 = 0

        // 测试位7 (MSB in byte)
        let result = decode_register_value(&registers, "bool", Some(7)).unwrap();
        assert_eq!(result, RedisValue::Integer(1)); // 位7 = 1

        // 测试高字节位15 (MSB)
        let high_bit_register = 0x8000; // 只有最高位是1
        let high_registers = vec![high_bit_register];
        let result = decode_register_value(&high_registers, "bool", Some(15)).unwrap();
        assert_eq!(result, RedisValue::Integer(1)); // 位15 = 1

        // 测试位15在低值寄存器中
        let result = decode_register_value(&registers, "bool", Some(15)).unwrap();
        assert_eq!(result, RedisValue::Integer(0)); // 位15 = 0 (因为0xB5没有设置第15位)
    }

    #[test]
    fn test_decode_register_value_bool_edge_cases() {
        let registers = vec![0x0000]; // 全0寄存器

        // 测试全0寄存器的所有位都应该是0
        for bit_pos in 0..16 {
            let result = decode_register_value(&registers, "bool", Some(bit_pos)).unwrap();
            assert_eq!(
                result,
                RedisValue::Integer(0),
                "Bit {} should be 0",
                bit_pos
            );
        }

        let registers = vec![0xFFFF]; // 全1寄存器

        // 测试全1寄存器的所有位都应该是1
        for bit_pos in 0..16 {
            let result = decode_register_value(&registers, "bool", Some(bit_pos)).unwrap();
            assert_eq!(
                result,
                RedisValue::Integer(1),
                "Bit {} should be 1",
                bit_pos
            );
        }

        // 测试错误情况：bit_position超出范围
        let result = decode_register_value(&registers, "bool", Some(16));
        assert!(result.is_err());

        // 测试错误情况：空寄存器
        let empty_registers = vec![];
        let result = decode_register_value(&empty_registers, "bool", Some(0));
        assert!(result.is_err());

        // 测试默认bit_position (应该是0)
        let registers = vec![0x0001]; // 只有位0是1
        let result = decode_register_value(&registers, "bool", None).unwrap();
        assert_eq!(result, RedisValue::Integer(1)); // 默认位0 = 1
    }

    #[test]
    fn test_decode_register_value_other_formats() {
        // 确保其他数据格式仍然正常工作
        let registers = vec![0x1234];

        // 测试uint16
        let result = decode_register_value(&registers, "uint16", None).unwrap();
        assert_eq!(result, RedisValue::Integer(0x1234));

        // 测试int16
        let result = decode_register_value(&registers, "int16", None).unwrap();
        assert_eq!(result, RedisValue::Integer(0x1234 as i16 as i64));

        // 测试float32需要2个寄存器
        let float_registers = vec![0x4000, 0x0000]; // 2.0 in IEEE 754
        let result = decode_register_value(&float_registers, "float32", None).unwrap();
        if let RedisValue::Float(f) = result {
            assert!((f - 2.0).abs() < 0.0001);
        } else {
            panic!("Expected float value");
        }
    }
}
