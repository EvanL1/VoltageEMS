//! 框架核心模块
//!
//! 整合了基础trait定义、类型定义和默认实现

use crate::core::config::{ChannelConfig, TelemetryType};
use crate::plugins::core::PluginStorage;
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
// ============================================================================
// Redis值类型定义
// ============================================================================

/// Redis值类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedisValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
}

/// 通道命令枚举
#[derive(Debug, Clone)]
pub enum ChannelCommand {
    /// 控制命令 (YK)
    Control {
        command_id: String,
        point_id: u32,
        value: f64,
        timestamp: i64,
    },
    /// 调节命令 (YT)
    Adjustment {
        command_id: String,
        point_id: u32,
        value: f64,
        timestamp: i64,
    },
}

// ============================================================================
// 基础类型定义（来自types.rs）
// ============================================================================

/// 通道操作状态和健康信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelStatus {
    pub is_connected: bool,
    pub last_error: Option<String>,
    pub last_update: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub reconnect_count: u64,
    pub points_count: usize,
    pub last_read_duration_ms: Option<u64>,
    pub average_read_duration_ms: Option<f64>,
}

/// 点位数据结构 - 使用 combase 的包装类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    pub value: RedisValue,
    pub timestamp: u64,
}

/// 扩展的点位数据（用于API和展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedPointData {
    pub id: String,
    pub name: String,
    pub value: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub unit: String,
    pub description: String,
    pub telemetry_type: Option<TelemetryType>,
    pub channel_id: Option<u16>,
}

impl Default for PointData {
    fn default() -> Self {
        Self {
            value: RedisValue::Float(0.0),
            timestamp: 0,
        }
    }
}

/// 点位映射表
pub type PointDataMap = HashMap<u32, PointData>;

/// 测试用的通道参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestChannelParams {
    pub initial_value: f64,
    pub increment: f64,
    pub interval_ms: u64,
}

impl Default for TestChannelParams {
    fn default() -> Self {
        Self {
            initial_value: 0.0,
            increment: 1.0,
            interval_ms: 1000,
        }
    }
}

// ============================================================================
// 核心Trait定义（来自traits.rs）
// ============================================================================

/// 主通信服务trait
///
/// 此trait定义了所有通信协议实现必须提供的核心接口
#[async_trait]
pub trait ComBase: Send + Sync {
    /// 获取实现名称
    fn name(&self) -> &str;

    /// 获取协议类型
    fn protocol_type(&self) -> &str;

    /// 检查连接状态
    fn is_connected(&self) -> bool;

    /// 获取通道状态
    async fn get_status(&self) -> ChannelStatus;

    /// 初始化通道
    async fn initialize(&mut self, channel_config: &ChannelConfig) -> Result<()>;

    /// 连接到目标系统
    async fn connect(&mut self) -> Result<()>;

    /// 断开连接
    async fn disconnect(&mut self) -> Result<()>;

    /// 读取四遥数据
    async fn read_four_telemetry(&self, telemetry_type: &str) -> Result<PointDataMap>;

    /// 执行控制命令
    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>>;

    /// 执行调节命令
    async fn adjustment(&mut self, adjustments: Vec<(u32, RedisValue)>)
        -> Result<Vec<(u32, bool)>>;

    // 四遥分离架构下，不再需要update_points方法，点位配置在initialize阶段直接加载

    /// 启动周期性任务
    async fn start_periodic_tasks(&self) -> Result<()> {
        Ok(())
    }

    /// 停止周期性任务
    async fn stop_periodic_tasks(&self) -> Result<()> {
        Ok(())
    }

    /// 获取诊断信息
    async fn get_diagnostics(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "name": self.name(),
            "protocol": self.protocol_type(),
            "connected": self.is_connected()
        }))
    }
}

/// 四遥操作trait
#[async_trait]
pub trait FourTelemetryOperations: Send + Sync {
    async fn read_yc(&self) -> Result<PointDataMap>;
    async fn read_yx(&self) -> Result<PointDataMap>;
    async fn execute_yk(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>>;
    async fn execute_yt(&mut self, adjustments: Vec<(u32, RedisValue)>)
        -> Result<Vec<(u32, bool)>>;
}

/// 连接管理trait
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn reconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
    async fn check_connection(&self) -> Result<bool>;
}

/// 配置验证trait
pub trait ConfigValidator {
    fn validate_config(config: &serde_json::Value) -> Result<()>;
}

/// 协议数据包解析器trait
#[async_trait]
pub trait ProtocolPacketParser: Send + Sync {
    fn protocol_name(&self) -> &'static str {
        "Unknown"
    }
    async fn parse_packet(&self, data: &[u8]) -> Result<PacketParseResult>;
    async fn build_packet(&self, data: &PointDataMap) -> Result<Vec<u8>>;
}

/// 数据包解析结果
#[derive(Debug, Clone)]
pub enum PacketParseResult {
    TelemetryData(PointDataMap),
    ControlResponse(Vec<(u32, bool)>),
    Error(String),
}

// ============================================================================
// 默认实现（来自base.rs）
// ============================================================================

/// 默认协议实现
///
/// `提供ComBase` trait的参考实现
pub struct DefaultProtocol {
    name: String,
    protocol_type: String,
    status: Arc<RwLock<ChannelStatus>>,
    is_connected: Arc<RwLock<bool>>,
    channel_config: Option<ChannelConfig>,
    // 四遥分离架构下，不再需要统一的point_mappings
    storage: Option<Arc<Mutex<Box<dyn PluginStorage>>>>,
}

impl DefaultProtocol {
    /// 创建新实例
    pub fn new(name: String, protocol_type: String) -> Self {
        Self {
            name,
            protocol_type,
            status: Arc::new(RwLock::new(ChannelStatus::default())),
            is_connected: Arc::new(RwLock::new(false)),
            channel_config: None,
            // 四遥分离架构下，不再需要统一的point_mappings
            storage: None,
        }
    }

    /// 设置存储后端
    pub fn with_storage(mut self, storage: Box<dyn PluginStorage>) -> Self {
        self.storage = Some(Arc::new(Mutex::new(storage)));
        self
    }

    /// 更新状态信息
    async fn update_status<F>(&self, updater: F)
    where
        F: FnOnce(&mut ChannelStatus),
    {
        let mut status = self.status.write().await;
        updater(&mut status);
        status.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    // 四遥分离架构下，不再需要get_mappings方法
}

#[async_trait]
impl ComBase for DefaultProtocol {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol_type(&self) -> &str {
        &self.protocol_type
    }

    fn is_connected(&self) -> bool {
        // 使用 try_read 避免在异步环境中阻塞
        self.is_connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn get_status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }

    async fn initialize(&mut self, channel_config: &ChannelConfig) -> Result<()> {
        self.channel_config = Some(channel_config.clone());

        let point_count = channel_config
            .parameters
            .get("point_count")
            .and_then(serde_yaml::Value::as_u64)
            .unwrap_or(0) as usize;

        self.update_status(|status| {
            status.points_count = point_count;
        })
        .await;

        Ok(())
    }

    async fn connect(&mut self) -> Result<()> {
        *self.is_connected.write().await = true;

        self.update_status(|status| {
            status.is_connected = true;
            status.last_error = None;
        })
        .await;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        *self.is_connected.write().await = false;

        self.update_status(|status| {
            status.is_connected = false;
        })
        .await;

        Ok(())
    }

    async fn read_four_telemetry(&self, _telemetry_type: &str) -> Result<PointDataMap> {
        if !<Self as ComBase>::is_connected(self) {
            return Err(ComSrvError::NotConnected);
        }

        // 四遥分离架构下，DefaultProtocol仅提供基础实现
        // 实际协议应该重写此方法以提供真实数据
        Ok(HashMap::new())
    }

    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        if !<Self as ComBase>::is_connected(self) {
            return Err(ComSrvError::NotConnected);
        }

        // 模拟控制执行
        let results = commands
            .into_iter()
            .map(|(point_id, _value)| (point_id, true))
            .collect();

        Ok(results)
    }

    async fn adjustment(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        if !<Self as ComBase>::is_connected(self) {
            return Err(ComSrvError::NotConnected);
        }

        // 模拟调节执行
        let results = adjustments
            .into_iter()
            .map(|(point_id, _value)| (point_id, true))
            .collect();

        Ok(results)
    }

    // 四遥分离架构下，update_points方法已移除
}

impl std::fmt::Debug for DefaultProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultProtocol")
            .field("name", &self.name)
            .field("protocol_type", &self.protocol_type)
            .field("is_connected", &self.is_connected)
            .field("channel_config", &self.channel_config)
            .finish()
    }
}

#[async_trait]
impl FourTelemetryOperations for DefaultProtocol {
    async fn read_yc(&self) -> Result<PointDataMap> {
        self.read_four_telemetry("m").await
    }

    async fn read_yx(&self) -> Result<PointDataMap> {
        self.read_four_telemetry("s").await
    }

    async fn execute_yk(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        self.control(commands).await
    }

    async fn execute_yt(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        self.adjustment(adjustments).await
    }
}

#[async_trait]
impl ConnectionManager for DefaultProtocol {
    async fn connect(&mut self) -> Result<()> {
        ComBase::connect(self).await
    }

    async fn disconnect(&mut self) -> Result<()> {
        ComBase::disconnect(self).await
    }

    async fn reconnect(&mut self) -> Result<()> {
        <Self as ConnectionManager>::disconnect(self).await?;
        <Self as ConnectionManager>::connect(self).await
    }

    fn is_connected(&self) -> bool {
        ComBase::is_connected(self)
    }

    async fn check_connection(&self) -> Result<bool> {
        Ok(<Self as ComBase>::is_connected(self))
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_protocol() {
        let mut protocol = DefaultProtocol::new("test".to_string(), "default".to_string());

        assert_eq!(protocol.name(), "test");
        assert_eq!(protocol.protocol_type(), "default");
        assert!(!ComBase::is_connected(&protocol));

        // 测试连接
        ComBase::connect(&mut protocol).await.unwrap();
        assert!(ComBase::is_connected(&protocol));

        // 测试状态
        let status = protocol.get_status().await;
        assert!(status.is_connected);
        assert_eq!(status.error_count, 0);
    }

    #[test]
    fn test_point_data_default() {
        let point = PointData::default();
        assert_eq!(point.timestamp, 0);
        match point.value {
            RedisValue::Float(v) => assert_eq!(v, 0.0),
            _ => panic!("Expected float value"),
        }
    }
}
