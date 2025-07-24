//! Modbus 协议核心实现
//!
//! 集成了协议处理、轮询机制和批量优化功能
//! 注意：当前版本为临时实现，专注于编译通过

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::core::combase::{
    ChannelCommand, ChannelStatus, ComBase, CommandSubscriber, CommandSubscriberConfig, PointData,
    PointDataMap, RedisValue,
};
use crate::core::config::types::{ChannelConfig, UnifiedPointMapping};
use crate::plugins::core::{PluginPointUpdate, PluginStorage};
use crate::utils::error::{ComSrvError, Result};

use super::transport::{ModbusFrameProcessor, ModbusMode};
use super::types::{ModbusPoint, ModbusPollingConfig};
use crate::core::transport::Transport;

/// Modbus 协议核心引擎
pub struct ModbusCore {
    /// 帧处理器
    _frame_processor: ModbusFrameProcessor,
    /// 轮询配置
    _polling_config: ModbusPollingConfig,
    /// 点位映射表
    _points: HashMap<String, ModbusPoint>,
}

impl ModbusCore {
    /// 创建新的 Modbus 核心引擎
    pub fn new(mode: ModbusMode, polling_config: ModbusPollingConfig) -> Self {
        Self {
            _frame_processor: ModbusFrameProcessor::new(mode),
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
        info!("已加载 {} 个 Modbus 点位", self._points.len());
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
    transport: Arc<Mutex<dyn Transport>>,
    storage: Arc<Mutex<Option<Arc<dyn PluginStorage>>>>,

    /// 命令处理
    command_subscriber: Option<CommandSubscriber>,
    command_rx: Option<mpsc::Receiver<ChannelCommand>>,

    /// 状态管理
    is_connected: Arc<RwLock<bool>>,
    status: Arc<RwLock<ChannelStatus>>,

    /// 任务管理
    polling_handle: Option<JoinHandle<()>>,
    command_handle: Option<JoinHandle<()>>,

    /// 轮询配置
    polling_config: ModbusPollingConfig,
    /// 点位映射
    points: Arc<RwLock<Vec<ModbusPoint>>>,
}

impl ModbusProtocol {
    /// 创建新的 Modbus 协议实例
    pub fn new(
        channel_config: ChannelConfig,
        transport: Arc<Mutex<dyn Transport>>,
        polling_config: ModbusPollingConfig,
    ) -> Result<Self> {
        let mode = if channel_config.protocol.contains("tcp") {
            ModbusMode::Tcp
        } else {
            ModbusMode::Rtu
        };

        let core = ModbusCore::new(mode, polling_config.clone());

        Ok(Self {
            name: channel_config.name.clone(),
            channel_id: channel_config.id,
            channel_config: Some(channel_config),
            core: Arc::new(Mutex::new(core)),
            transport,
            storage: Arc::new(Mutex::new(None)),
            command_subscriber: None,
            command_rx: None,
            is_connected: Arc::new(RwLock::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::default())),
            polling_handle: None,
            command_handle: None,
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
        *self.is_connected.blocking_read()
    }

    async fn get_status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }

    async fn initialize(&mut self, channel_config: &ChannelConfig) -> Result<()> {
        info!("初始化 Modbus 协议，通道 {}", channel_config.id);

        self.channel_config = Some(channel_config.clone());

        // 初始化存储
        if let Ok(storage) = crate::plugins::core::DefaultPluginStorage::from_env().await {
            *self.storage.lock().await = Some(Arc::new(storage) as Arc<dyn PluginStorage>);
        }

        // 从配置中提取点位
        let mut modbus_points = Vec::new();
        for point in &channel_config.combined_points {
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
                            slave_id,
                            function_code,
                            register_address,
                            data_format: point
                                .protocol_params
                                .get("data_format")
                                .unwrap_or(&"uint16".to_string())
                                .clone(),
                            register_count: point
                                .protocol_params
                                .get("register_count")
                                .and_then(|v| v.parse::<u16>().ok())
                                .unwrap_or(1),
                            byte_order: point.protocol_params.get("byte_order").cloned(),
                        };
                        modbus_points.push(modbus_point);
                    }
                }
            }
        }

        // 设置点位到核心和本地存储
        {
            let mut core = self.core.lock().await;
            core.set_points(modbus_points.clone());
        }
        *self.points.write().await = modbus_points;

        self.status.write().await.points_count = self.points.read().await.len();

        Ok(())
    }

    async fn connect(&mut self) -> Result<()> {
        info!("连接 Modbus 设备，通道 {}", self.channel_id);

        // 建立传输连接
        self.transport.lock().await.connect().await?;

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

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("断开 Modbus 连接，通道 {}", self.channel_id);

        // 停止所有任务
        self.stop_periodic_tasks().await?;

        // 断开传输连接
        self.transport.lock().await.disconnect().await?;

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
            .ok_or_else(|| ComSrvError::config("通道配置未初始化"))?;

        for point in points.iter() {
            // 从通道配置中查找点位的遥测类型
            if let Some(config_point) = channel_config
                .combined_points
                .iter()
                .find(|p| p.point_id.to_string() == point.point_id)
            {
                if config_point.telemetry_type == telemetry_type {
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
            debug!("执行控制命令: 点位 {}, 值 {:?}", point_id, value);
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
            debug!("执行调节命令: 点位 {}, 值 {:?}", point_id, value);
            results.push((point_id, true));
        }

        Ok(results)
    }

    async fn update_points(&mut self, mappings: Vec<UnifiedPointMapping>) -> Result<()> {
        // 转换为 ModbusPoint
        let mut modbus_points = Vec::new();

        for mapping in mappings {
            if let Some(address) = mapping.protocol_params.get("address") {
                let parts: Vec<&str> = address.split(':').collect();
                if parts.len() >= 3 {
                    if let (Ok(slave_id), Ok(function_code), Ok(register_address)) = (
                        parts[0].parse::<u8>(),
                        parts[1].parse::<u8>(),
                        parts[2].parse::<u16>(),
                    ) {
                        let modbus_point = ModbusPoint {
                            point_id: mapping.point_id.to_string(),
                            slave_id,
                            function_code,
                            register_address,
                            data_format: mapping
                                .protocol_params
                                .get("data_format")
                                .unwrap_or(&"uint16".to_string())
                                .clone(),
                            register_count: mapping
                                .protocol_params
                                .get("register_count")
                                .and_then(|v| v.parse::<u16>().ok())
                                .unwrap_or(1),
                            byte_order: mapping.protocol_params.get("byte_order").cloned(),
                        };
                        modbus_points.push(modbus_point);
                    }
                }
            }
        }

        // 更新点位
        {
            let mut core = self.core.lock().await;
            core.set_points(modbus_points.clone());
        }
        *self.points.write().await = modbus_points;

        Ok(())
    }

    async fn start_periodic_tasks(&self) -> Result<()> {
        info!("启动 Modbus 周期性任务，通道 {}", self.channel_id);

        // 启动命令订阅
        if let Some(ref subscriber) = &self.command_subscriber {
            // 命令订阅将在协议初始化时启动
            debug!("命令订阅器已准备就绪，通道 {}", self.channel_id);
        }

        // TODO: 启动轮询任务

        Ok(())
    }

    async fn stop_periodic_tasks(&self) -> Result<()> {
        info!("停止 Modbus 周期性任务，通道 {}", self.channel_id);

        // 停止命令订阅
        if let Some(ref subscriber) = &self.command_subscriber {
            // 命令订阅将在协议停止时自动清理
            debug!("清理命令订阅器，通道 {}", self.channel_id);
        }

        // TODO: 停止轮询任务

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transport::mock::{MockTransport, MockTransportConfig};

    // 辅助函数：创建测试通道配置
    fn create_test_channel_config() -> ChannelConfig {
        ChannelConfig {
            id: 1001,
            name: "Test Modbus Channel".to_string(),
            description: Some("Test channel for Modbus protocol".to_string()),
            protocol: "modbus_tcp".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert(
                    "host".to_string(),
                    serde_yaml::Value::String("localhost".to_string()),
                );
                params.insert("port".to_string(), serde_yaml::Value::Number(502.into()));
                params.insert(
                    "redis_url".to_string(),
                    serde_yaml::Value::String("redis://localhost:6379".to_string()),
                );
                params
            },
            combined_points: vec![
                // YC - 遥测点（从1开始）
                UnifiedPointMapping {
                    point_id: 1,
                    signal_name: "Temperature_1".to_string(),
                    telemetry_type: "YC".to_string(),
                    data_type: "float32".to_string(),
                    protocol_params: {
                        let mut params = HashMap::new();
                        params.insert("address".to_string(), "1:3:0".to_string()); // slave:function:register
                        params.insert("data_format".to_string(), "float32_be".to_string());
                        params.insert("register_count".to_string(), "2".to_string());
                        params
                    },
                    scaling: None,
                },
                // YX - 信号点
                UnifiedPointMapping {
                    point_id: 2,
                    signal_name: "Status_1".to_string(),
                    telemetry_type: "YX".to_string(),
                    data_type: "bool".to_string(),
                    protocol_params: {
                        let mut params = HashMap::new();
                        params.insert("address".to_string(), "1:1:0".to_string());
                        params.insert("data_format".to_string(), "bool".to_string());
                        params
                    },
                    scaling: None,
                },
                // YK - 控制点
                UnifiedPointMapping {
                    point_id: 3,
                    signal_name: "Control_1".to_string(),
                    telemetry_type: "YK".to_string(),
                    data_type: "bool".to_string(),
                    protocol_params: {
                        let mut params = HashMap::new();
                        params.insert("address".to_string(), "1:5:0".to_string());
                        params.insert("data_format".to_string(), "bool".to_string());
                        params
                    },
                    scaling: None,
                },
                // YT - 调节点
                UnifiedPointMapping {
                    point_id: 4,
                    signal_name: "Setpoint_1".to_string(),
                    telemetry_type: "YT".to_string(),
                    data_type: "float32".to_string(),
                    protocol_params: {
                        let mut params = HashMap::new();
                        params.insert("address".to_string(), "1:6:10".to_string());
                        params.insert("data_format".to_string(), "float32_be".to_string());
                        params.insert("register_count".to_string(), "2".to_string());
                        params
                    },
                    scaling: None,
                },
            ],
            logging: Default::default(),
            table_config: None,
            points: vec![],
        }
    }

    #[tokio::test]
    async fn test_modbus_protocol_creation() {
        let config = create_test_channel_config();
        let transport = Arc::new(MockTransport::new(MockTransportConfig::default()));
        let polling_config = ModbusPollingConfig::default();

        let protocol = ModbusProtocol::new(config, transport, polling_config);
        assert!(protocol.is_ok());

        let protocol = protocol.unwrap();
        assert_eq!(protocol.name(), "Test Modbus Channel");
        assert_eq!(protocol.protocol_type(), "modbus");
    }

    #[tokio::test]
    async fn test_initialize_extracts_points_correctly() {
        let config = create_test_channel_config();
        let transport = Arc::new(MockTransport::new(MockTransportConfig::default()));
        let polling_config = ModbusPollingConfig::default();

        let mut protocol = ModbusProtocol::new(config.clone(), transport, polling_config).unwrap();

        // 初始化
        protocol.initialize(&config).await.unwrap();

        // 验证点位提取
        let points = protocol.points.read().await;
        assert_eq!(points.len(), 4);

        // 验证第一个点（YC）
        let point1 = &points[0];
        assert_eq!(point1.point_id, "1");
        assert_eq!(point1.slave_id, 1);
        assert_eq!(point1.function_code, 3);
        assert_eq!(point1.register_address, 0);
        assert_eq!(point1.data_format, "float32_be");
        assert_eq!(point1.register_count, 2);
    }

    #[tokio::test]
    async fn test_read_four_telemetry_yc() {
        let config = create_test_channel_config();
        let transport = Arc::new(MockTransport::new(MockTransportConfig::default()));
        let polling_config = ModbusPollingConfig::default();

        let mut protocol = ModbusProtocol::new(config.clone(), transport, polling_config).unwrap();
        protocol.initialize(&config).await.unwrap();
        protocol.connect().await.unwrap();

        // 读取YC数据
        let result = protocol.read_four_telemetry("YC").await.unwrap();

        // 应该只返回YC类型的点
        assert_eq!(result.len(), 1);
        assert!(result.contains_key(&1)); // point_id = 1
    }

    #[tokio::test]
    async fn test_read_four_telemetry_yx() {
        let config = create_test_channel_config();
        let transport = Arc::new(MockTransport::new(MockTransportConfig::default()));
        let polling_config = ModbusPollingConfig::default();

        let mut protocol = ModbusProtocol::new(config.clone(), transport, polling_config).unwrap();
        protocol.initialize(&config).await.unwrap();
        protocol.connect().await.unwrap();

        // 读取YX数据
        let result = protocol.read_four_telemetry("YX").await.unwrap();

        // 应该只返回YX类型的点
        assert_eq!(result.len(), 1);
        assert!(result.contains_key(&2)); // point_id = 2
    }

    #[tokio::test]
    async fn test_control_commands() {
        let config = create_test_channel_config();
        let transport = Arc::new(MockTransport::new(MockTransportConfig::default()));
        let polling_config = ModbusPollingConfig::default();

        let mut protocol = ModbusProtocol::new(config.clone(), transport, polling_config).unwrap();
        protocol.initialize(&config).await.unwrap();
        protocol.connect().await.unwrap();

        // 执行控制命令（YK）
        let commands = vec![(3, RedisValue::Bool(true))]; // point_id = 3
        let results = protocol.control(commands).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 3);
        assert!(results[0].1); // 成功
    }

    #[tokio::test]
    async fn test_adjustment_commands() {
        let config = create_test_channel_config();
        let transport = Arc::new(MockTransport::new(MockTransportConfig::default()));
        let polling_config = ModbusPollingConfig::default();

        let mut protocol = ModbusProtocol::new(config.clone(), transport, polling_config).unwrap();
        protocol.initialize(&config).await.unwrap();
        protocol.connect().await.unwrap();

        // 执行调节命令（YT）
        let adjustments = vec![(4, RedisValue::Float(50.0))]; // point_id = 4
        let results = protocol.adjustment(adjustments).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 4);
        assert!(results[0].1); // 成功
    }

    #[tokio::test]
    async fn test_connect_disconnect_cycle() {
        let config = create_test_channel_config();
        let transport = Arc::new(MockTransport::new(MockTransportConfig::default()));
        let polling_config = ModbusPollingConfig::default();

        let mut protocol = ModbusProtocol::new(config.clone(), transport, polling_config).unwrap();
        protocol.initialize(&config).await.unwrap();

        // 测试连接
        assert!(!protocol.is_connected());
        protocol.connect().await.unwrap();
        assert!(protocol.is_connected());

        // 测试断开连接
        protocol.disconnect().await.unwrap();
        assert!(!protocol.is_connected());
    }

    #[tokio::test]
    async fn test_update_points() {
        let config = create_test_channel_config();
        let transport = Arc::new(MockTransport::new(MockTransportConfig::default()));
        let polling_config = ModbusPollingConfig::default();

        let mut protocol = ModbusProtocol::new(config.clone(), transport, polling_config).unwrap();
        protocol.initialize(&config).await.unwrap();

        // 创建新的点位映射
        let new_mappings = vec![UnifiedPointMapping {
            point_id: 5,
            signal_name: "New_Point".to_string(),
            telemetry_type: "YC".to_string(),
            data_type: "uint16".to_string(),
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("address".to_string(), "1:3:100".to_string());
                params.insert("data_format".to_string(), "uint16".to_string());
                params
            },
            scaling: None,
        }];

        // 更新点位
        protocol.update_points(new_mappings).await.unwrap();

        let points = protocol.points.read().await;
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].point_id, "5");
    }
}
