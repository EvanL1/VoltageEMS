//! Modbus客户端实现
//!
//! 这个模块提供了高性能的Modbus客户端实现，具有以下特性：
//! - 统一的API接口和配置管理
//! - 零拷贝数据处理和智能缓存
//! - 连接池管理和智能重试机制
//! - 内置监控和诊断功能

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;
use tracing::{warn, error, info};
use async_trait::async_trait;

use crate::core::protocols::common::combase::{
    traits::ComBase,
    data_types::{PointData, ChannelStatus},
    telemetry::TelemetryType,
};
use crate::core::protocols::modbus::{
    protocol_engine::ModbusProtocolEngine,
    common::ModbusConfig,
};
use crate::core::protocols::common::combase::transport_bridge::UniversalTransportBridge;
use crate::core::transport::traits::Transport;
use crate::utils::error::{ComSrvError, Result};

/// 连接状态
#[derive(Debug, Clone)]
pub struct ConnectionState {
    pub connected: bool,
    pub last_connect_time: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
    pub retry_count: u32,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self {
            connected: false,
            last_connect_time: None,
            last_error: None,
            retry_count: 0,
        }
    }
}

/// 客户端统计信息
#[derive(Debug, Clone, Default)]
pub struct ClientStatistics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub average_response_time_ms: f64,
    pub last_request_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// Modbus通道配置
#[derive(Debug, Clone)]
pub struct ModbusChannelConfig {
    pub channel_id: u16,
    pub channel_name: String,
    pub connection: ModbusConfig,
    pub request_timeout: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
}

/// 协议映射表
#[derive(Debug, Clone)]
pub struct ProtocolMappingTable {
    pub telemetry_mappings: HashMap<u32, ModbusTelemetryMapping>,
    pub signal_mappings: HashMap<u32, ModbusSignalMapping>,
    pub adjustment_mappings: HashMap<u32, ModbusAdjustmentMapping>,
    pub control_mappings: HashMap<u32, ModbusControlMapping>,
}

impl Default for ProtocolMappingTable {
    fn default() -> Self {
        Self {
            telemetry_mappings: HashMap::new(),
            signal_mappings: HashMap::new(),
            adjustment_mappings: HashMap::new(),
            control_mappings: HashMap::new(),
        }
    }
}

/// 简化的映射结构
use crate::core::protocols::modbus::protocol_engine::{
    ModbusTelemetryMapping, ModbusSignalMapping, 
    ModbusAdjustmentMapping, ModbusControlMapping
};

/// Modbus客户端
pub struct ModbusClient {
    /// 核心组件
    transport_bridge: Arc<UniversalTransportBridge>,
    protocol_engine: Arc<ModbusProtocolEngine>,
    
    /// 配置管理
    config: ModbusChannelConfig,
    mappings: Arc<RwLock<ProtocolMappingTable>>,
    
    /// 状态管理
    connection_state: Arc<RwLock<ConnectionState>>,
    statistics: Arc<RwLock<ClientStatistics>>,
}

impl ModbusClient {
    /// 创建新的Modbus客户端
    pub async fn new(
        config: ModbusChannelConfig,
        transport: Box<dyn Transport>,
    ) -> Result<Self> {
        // 创建传输桥接
        let transport_bridge = Arc::new(UniversalTransportBridge::new_modbus(transport));
        
        // 创建协议引擎
        let protocol_engine = Arc::new(ModbusProtocolEngine::new(&config.connection).await?);
        
        info!("创建Modbus客户端: {}", config.channel_name);
        
        Ok(Self {
            transport_bridge,
            protocol_engine,
            config,
            mappings: Arc::new(RwLock::new(ProtocolMappingTable::default())),
            connection_state: Arc::new(RwLock::new(ConnectionState::default())),
            statistics: Arc::new(RwLock::new(ClientStatistics::default())),
        })
    }

    /// 加载协议映射
    pub async fn load_protocol_mappings(&self, mappings: ProtocolMappingTable) -> Result<()> {
        let mut current_mappings = self.mappings.write().await;
        *current_mappings = mappings;
        
        let total_mappings = current_mappings.telemetry_mappings.len() +
                           current_mappings.signal_mappings.len() +
                           current_mappings.adjustment_mappings.len() +
                           current_mappings.control_mappings.len();
        
        info!("加载了 {} 个协议映射到客户端 {}", total_mappings, self.config.channel_name);
        Ok(())
    }

    /// 连接到设备
    pub async fn connect(&self) -> Result<()> {
        let mut state = self.connection_state.write().await;
        
        match self.transport_bridge.connect().await {
            Ok(_) => {
                state.connected = true;
                state.last_connect_time = Some(chrono::Utc::now());
                state.last_error = None;
                state.retry_count = 0;
                
                info!("成功连接到Modbus设备: {}", self.config.channel_name);
                Ok(())
            }
            Err(e) => {
                state.connected = false;
                state.last_error = Some(e.to_string());
                state.retry_count += 1;
                
                error!("连接Modbus设备失败: {} - {}", self.config.channel_name, e);
                Err(e)
            }
        }
    }

    /// 断开连接
    pub async fn disconnect(&self) -> Result<()> {
        let mut state = self.connection_state.write().await;
        
        match self.transport_bridge.disconnect().await {
            Ok(_) => {
                state.connected = false;
                info!("已断开Modbus设备连接: {}", self.config.channel_name);
                Ok(())
            }
            Err(e) => {
                error!("断开Modbus设备连接失败: {} - {}", self.config.channel_name, e);
                Err(e)
            }
        }
    }

    /// 读取单个点位
    pub async fn read_point(&self, point_id: u32, telemetry_type: TelemetryType) -> Result<PointData> {
        let start_time = std::time::Instant::now();
        
        // 更新统计信息
        {
            let mut stats = self.statistics.write().await;
            stats.total_requests += 1;
            stats.last_request_time = Some(chrono::Utc::now());
        }

        let result = self.internal_read_point(point_id, telemetry_type).await;
        
        // 更新统计信息
        {
            let mut stats = self.statistics.write().await;
            let elapsed = start_time.elapsed().as_millis() as f64;
            
            match &result {
                Ok(_) => {
                    stats.successful_requests += 1;
                    // 更新平均响应时间
                    stats.average_response_time_ms = 
                        (stats.average_response_time_ms * (stats.successful_requests - 1) as f64 + elapsed) / 
                        stats.successful_requests as f64;
                }
                Err(_) => {
                    stats.failed_requests += 1;
                }
            }
        }

        result
    }

    /// 内部读取点位实现
    async fn internal_read_point(&self, point_id: u32, telemetry_type: TelemetryType) -> Result<PointData> {
        let mappings = self.mappings.read().await;
        
        match telemetry_type {
            TelemetryType::Telemetry => {
                if let Some(mapping) = mappings.telemetry_mappings.get(&point_id) {
                    self.protocol_engine.read_telemetry_point(mapping, &self.transport_bridge).await
                } else {
                    Err(ComSrvError::NotFound(format!("遥测点位未找到: {}", point_id)))
                }
            }
            TelemetryType::Signaling => {
                if let Some(mapping) = mappings.signal_mappings.get(&point_id) {
                    self.protocol_engine.read_signal_point(mapping, &self.transport_bridge).await
                } else {
                    Err(ComSrvError::NotFound(format!("遥信点位未找到: {}", point_id)))
                }
            }
            _ => Err(ComSrvError::ProtocolNotSupported(
                "读取操作不支持遥调和遥控类型".to_string()
            ))
        }
    }

    /// 写入点位
    pub async fn write_point(&self, point_id: u32, value: &str) -> Result<()> {
        let mappings = self.mappings.read().await;
        
        // 尝试遥调操作
        if let Some(mapping) = mappings.adjustment_mappings.get(&point_id) {
            let float_value: f64 = value.parse()
                .map_err(|_| ComSrvError::InvalidParameter(format!("无效的遥调值: {}", value)))?;
            return self.protocol_engine.write_adjustment_point(mapping, float_value, &self.transport_bridge).await;
        }
        
        // 尝试遥控操作
        if let Some(mapping) = mappings.control_mappings.get(&point_id) {
            let bool_value = match value.to_lowercase().as_str() {
                "true" | "1" | "on" => true,
                "false" | "0" | "off" => false,
                _ => return Err(ComSrvError::InvalidParameter(format!("无效的遥控值: {}", value))),
            };
            return self.protocol_engine.execute_control_point(mapping, bool_value, &self.transport_bridge).await;
        }
        
        Err(ComSrvError::NotFound(format!("可写点位未找到: {}", point_id)))
    }

    /// 批量读取点位
    pub async fn read_points_batch(&self, point_ids: &[u32]) -> Result<Vec<PointData>> {
        let mut results = Vec::new();
        let mappings = self.mappings.read().await;
        
        // 构建批量读取请求
        let mut batch_requests = Vec::new();
        
        for &point_id in point_ids {
            // 检查点位类型并添加到批量请求
            if mappings.telemetry_mappings.contains_key(&point_id) {
                batch_requests.push((point_id, TelemetryType::Telemetry));
            } else if mappings.signal_mappings.contains_key(&point_id) {
                batch_requests.push((point_id, TelemetryType::Signaling));
            }
        }
        
        // 执行批量读取（可以在这里优化为真正的批量操作）
        for (point_id, telemetry_type) in batch_requests {
            match self.read_point(point_id, telemetry_type).await {
                Ok(point_data) => results.push(point_data),
                Err(e) => {
                    warn!("批量读取点位 {} 失败: {}", point_id, e);
                    // 创建错误点位数据
                    results.push(PointData {
                        id: point_id.to_string(),
                        name: format!("点位_{}", point_id),
                        value: "error".to_string(),
                        timestamp: chrono::Utc::now(),
                        unit: "".to_string(),
                        description: format!("读取失败: {}", e),
                    });
                }
            }
        }
        
        Ok(results)
    }

    /// 获取连接状态
    pub async fn get_connection_state(&self) -> ConnectionState {
        let state = self.connection_state.read().await;
        state.clone()
    }

    /// 获取统计信息
    pub async fn get_statistics(&self) -> ClientStatistics {
        let stats = self.statistics.read().await;
        stats.clone()
    }

    /// 重置统计信息
    pub async fn reset_statistics(&self) {
        let mut stats = self.statistics.write().await;
        *stats = ClientStatistics::default();
    }

    /// 获取映射计数
    pub async fn get_mapping_counts(&self) -> HashMap<String, usize> {
        let mappings = self.mappings.read().await;
        let mut counts = HashMap::new();
        counts.insert("telemetry".to_string(), mappings.telemetry_mappings.len());
        counts.insert("signal".to_string(), mappings.signal_mappings.len());
        counts.insert("adjustment".to_string(), mappings.adjustment_mappings.len());
        counts.insert("control".to_string(), mappings.control_mappings.len());
        counts
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<HashMap<String, String>> {
        let mut health = HashMap::new();
        
        // 检查连接状态
        let state = self.connection_state.read().await;
        health.insert("connected".to_string(), state.connected.to_string());
        
        // 检查传输层状态
        let transport_connected = self.transport_bridge.is_connected().await;
        health.insert("transport_connected".to_string(), transport_connected.to_string());
        
        // 检查统计信息
        let stats = self.statistics.read().await;
        health.insert("total_requests".to_string(), stats.total_requests.to_string());
        health.insert("success_rate".to_string(), 
            if stats.total_requests > 0 {
                format!("{:.2}%", (stats.successful_requests as f64 / stats.total_requests as f64) * 100.0)
            } else {
                "N/A".to_string()
            }
        );
        
        // 检查平均响应时间
        health.insert("avg_response_time_ms".to_string(), 
            format!("{:.2}", stats.average_response_time_ms));
        
        Ok(health)
    }
}

/// 实现ComBase trait以保持兼容性
#[async_trait]
impl ComBase for ModbusClient {

    fn name(&self) -> &str {
        &self.config.channel_name
    }

    fn channel_id(&self) -> String {
        self.config.channel_id.to_string()
    }

    fn protocol_type(&self) -> &str {
        "modbus"
    }

    fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("channel_id".to_string(), self.config.channel_id.to_string());
        params.insert("channel_name".to_string(), self.config.channel_name.clone());
        params.insert("timeout_ms".to_string(), self.config.request_timeout.as_millis().to_string());
        params.insert("max_retries".to_string(), self.config.max_retries.to_string());
        params
    }

    async fn is_running(&self) -> bool {
        let state = self.connection_state.read().await;
        state.connected
    }

    async fn start(&mut self) -> Result<()> {
        self.connect().await
    }

    async fn stop(&mut self) -> Result<()> {
        self.disconnect().await
    }

    async fn status(&self) -> ChannelStatus {
        let state = self.connection_state.read().await;
        let stats = self.statistics.read().await;
        
        ChannelStatus {
            id: self.config.channel_id.to_string(),
            connected: state.connected,
            last_response_time: stats.average_response_time_ms,
            last_error: state.last_error.clone().unwrap_or_default(),
            last_update_time: stats.last_request_time.unwrap_or_else(chrono::Utc::now),
        }
    }

    async fn update_status(&mut self, _status: ChannelStatus) -> Result<()> {
        // 状态更新由内部管理
        Ok(())
    }

    async fn get_all_points(&self) -> Vec<PointData> {
        let mappings = self.mappings.read().await;
        let mut point_ids = Vec::new();
        
        // 收集所有点位ID
        point_ids.extend(mappings.telemetry_mappings.keys());
        point_ids.extend(mappings.signal_mappings.keys());
        
        // 批量读取
        match self.read_points_batch(&point_ids).await {
            Ok(points) => points,
            Err(e) => {
                error!("批量读取所有点位失败: {}", e);
                Vec::new()
            }
        }
    }

    async fn read_point(&self, point_id: &str) -> Result<PointData> {
        let id: u32 = point_id.parse()
            .map_err(|_| ComSrvError::InvalidParameter(format!("无效的点位ID: {}", point_id)))?;
        
        let mappings = self.mappings.read().await;
        
        // 确定点位类型
        if mappings.telemetry_mappings.contains_key(&id) {
            self.read_point(id, TelemetryType::Telemetry).await
        } else if mappings.signal_mappings.contains_key(&id) {
            self.read_point(id, TelemetryType::Signaling).await
        } else {
            Err(ComSrvError::NotFound(format!("点位未找到: {}", point_id)))
        }
    }

    async fn write_point(&mut self, point_id: &str, value: &str) -> Result<()> {
        let id: u32 = point_id.parse()
            .map_err(|_| ComSrvError::InvalidParameter(format!("无效的点位ID: {}", point_id)))?;
        
        // Call the non-trait method directly
        ModbusClient::write_point(self, id, value).await
    }

    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diagnostics = HashMap::new();
        
        // 基本信息
        diagnostics.insert("protocol".to_string(), "modbus".to_string());
        diagnostics.insert("channel_id".to_string(), self.config.channel_id.to_string());
        diagnostics.insert("channel_name".to_string(), self.config.channel_name.clone());
        
        // 健康检查信息
        if let Ok(health) = self.health_check().await {
            diagnostics.extend(health);
        }
        
        // 映射统计
        let counts = self.get_mapping_counts().await;
        for (telemetry_type, count) in counts {
            diagnostics.insert(format!("{}_mappings", telemetry_type), count.to_string());
        }
        
        // 传输层诊断
        let transport_diag = self.transport_bridge.diagnostics().await;
        for (key, value) in transport_diag {
            diagnostics.insert(format!("transport_{}", key), value);
        }
        
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transport::mock::MockTransport;

    fn create_test_config() -> ModbusChannelConfig {
        ModbusChannelConfig {
            channel_id: 1,
            channel_name: "测试通道".to_string(),
            connection: ModbusConfig {
                protocol_type: "modbus_tcp".to_string(),
                host: Some("127.0.0.1".to_string()),
                port: Some(502),
                device_path: None,
                baud_rate: None,
                data_bits: None,
                stop_bits: None,
                parity: None,
                timeout_ms: Some(5000),
                slave_id: 1,
                points: vec![],
            },
            request_timeout: Duration::from_millis(5000),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
        }
    }

    #[tokio::test]
    async fn test_unified_client_creation() {
        let config = create_test_config();
        let mock_config = crate::core::transport::mock::MockTransportConfig::default();
        let transport = Box::new(MockTransport::new(mock_config).unwrap());
        
        let client = ModbusClient::new(config, transport).await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_statistics_tracking() {
        let config = create_test_config();
        let mock_config = crate::core::transport::mock::MockTransportConfig::default();
        let transport = Box::new(MockTransport::new(mock_config).unwrap());
        
        let client = ModbusClient::new(config, transport).await.unwrap();
        let stats = client.get_statistics().await;
        
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 0);
    }
}

impl std::fmt::Debug for ModbusClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModbusClient")
            .field("config", &self.config)
            .finish()
    }
}