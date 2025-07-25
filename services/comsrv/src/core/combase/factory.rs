//! 协议工厂模块
//!
//! 提供协议实例的创建、管理和生命周期控制

use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::core::combase::command::{CommandSubscriber, CommandSubscriberConfig};
use crate::core::combase::core::{ComBase, PointData};
use crate::core::config::{ChannelConfig, ChannelLoggingConfig, ConfigManager, ProtocolType};
use crate::utils::error::{ComSrvError, Result};
use std::str::FromStr;

/// 配置值类型（使用JSON进行内部处理）
pub type ConfigValue = serde_json::Value;

/// 动态通信客户端类型
pub type DynComClient = Arc<RwLock<Box<dyn ComBase>>>;

// ============================================================================
// 协议客户端工厂trait
// ============================================================================

/// 协议客户端工厂trait，用于可扩展的协议支持
#[async_trait]
pub trait ProtocolClientFactory: Send + Sync {
    /// 获取协议类型
    fn protocol_type(&self) -> ProtocolType;

    /// 创建协议客户端实例
    async fn create_client(
        &self,
        channel_config: &ChannelConfig,
        config_value: ConfigValue,
    ) -> Result<Box<dyn ComBase>>;

    /// 验证配置
    fn validate_config(&self, config: &ConfigValue) -> Result<()>;

    /// 获取配置模板
    fn get_config_template(&self) -> ConfigValue;

    /// 获取协议信息
    fn get_protocol_info(&self) -> serde_json::Value {
        serde_json::json!({
            "protocol_type": self.protocol_type(),
            "supports_batch": false,
            "supports_async": true
        })
    }
}

// ============================================================================
// 通道管理结构
// ============================================================================

/// 通道元数据
#[derive(Debug, Clone)]
struct ChannelMetadata {
    pub name: String,
    pub protocol_type: ProtocolType,
    pub created_at: std::time::Instant,
    pub last_accessed: Arc<RwLock<std::time::Instant>>,
}

/// 通道条目，组合通道和元数据
#[derive(Clone)]
struct ChannelEntry {
    channel: Arc<RwLock<Box<dyn ComBase>>>,
    metadata: ChannelMetadata,
    command_subscriber: Option<Arc<RwLock<CommandSubscriber>>>,
}

impl std::fmt::Debug for ChannelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelEntry")
            .field("metadata", &self.metadata)
            .finish()
    }
}

// ============================================================================
// 协议工厂主结构
// ============================================================================

/// 协议工厂，管理所有协议和通道
pub struct ProtocolFactory {
    /// 存储创建的通道
    channels: DashMap<u16, ChannelEntry, ahash::RandomState>,
    /// 协议工厂注册表
    protocol_factories: DashMap<ProtocolType, Arc<dyn ProtocolClientFactory>, ahash::RandomState>,
}

impl std::fmt::Debug for ProtocolFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtocolFactory")
            .field("channels", &self.channels.len())
            .field("protocol_factories", &self.protocol_factories.len())
            .finish()
    }
}

impl ProtocolFactory {
    /// 创建新的协议工厂
    pub fn new() -> Self {
        let factory = Self {
            channels: DashMap::with_hasher(ahash::RandomState::new()),
            protocol_factories: DashMap::with_hasher(ahash::RandomState::new()),
        };

        // 初始化插件系统
        let _ = crate::plugins::core::get_plugin_registry();

        factory
    }

    /// 注册协议工厂
    pub fn register_protocol_factory(&self, factory: Arc<dyn ProtocolClientFactory>) {
        let protocol_type = factory.protocol_type();
        self.protocol_factories.insert(protocol_type, factory);
        info!("Registered protocol factory for {protocol_type:?}");
    }

    /// 注销协议工厂
    pub fn unregister_protocol_factory(&self, protocol_type: &ProtocolType) -> Result<bool> {
        // 检查是否有活动通道使用此协议
        let active_channels: Vec<u16> = self
            .channels
            .iter()
            .filter_map(|entry| {
                let (id, channel_entry) = (entry.key(), entry.value());
                if channel_entry.metadata.protocol_type == *protocol_type {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        if !active_channels.is_empty() {
            return Err(ComSrvError::InvalidOperation(format!(
                "Cannot unregister protocol factory: {} channels are still active",
                active_channels.len()
            )));
        }

        match self.protocol_factories.remove(protocol_type) {
            Some(_) => {
                info!("Unregistered protocol factory for {protocol_type:?}");
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// 获取已注册的协议类型列表
    pub fn get_registered_protocols(&self) -> Vec<ProtocolType> {
        self.protocol_factories
            .iter()
            .map(|entry| *entry.key())
            .collect()
    }

    /// 创建通道
    pub async fn create_channel(
        &self,
        channel_config: &ChannelConfig,
        _config_manager: Option<&ConfigManager>,
    ) -> Result<Arc<RwLock<Box<dyn ComBase>>>> {
        let channel_id = channel_config.id;

        // 检查通道是否已存在
        if self.channels.contains_key(&channel_id) {
            return Err(ComSrvError::InvalidOperation(format!(
                "Channel {} already exists",
                channel_id
            )));
        }

        // 获取协议类型
        let protocol_type = ProtocolType::from_str(&channel_config.protocol)?;

        // 查找协议工厂
        let factory = self.protocol_factories.get(&protocol_type).ok_or_else(|| {
            ComSrvError::ConfigError(format!(
                "No factory registered for protocol: {:?}",
                protocol_type
            ))
        })?;

        // 将channel_config.parameters转换为ConfigValue
        let config_value = serde_json::to_value(&channel_config.parameters).map_err(|e| {
            ComSrvError::ConfigError(format!("Failed to convert parameters: {}", e))
        })?;

        // 验证配置
        factory.validate_config(&config_value)?;

        // 创建客户端实例
        let mut client = factory.create_client(channel_config, config_value).await?;

        // 初始化客户端
        client.initialize(channel_config).await?;

        // 加载点位映射
        // TODO: 实现点位映射加载逻辑
        // if let Some(config_mgr) = config_manager {
        //     let mappings = config_mgr.load_unified_mappings(channel_id).await?;
        //     client.update_points(mappings).await?;
        // }

        let channel_arc = Arc::new(RwLock::new(client));

        // 创建命令订阅器（如果需要）
        let enable_control = channel_config
            .parameters
            .get("enable_control")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let command_subscriber = if enable_control {
            let redis_url = channel_config
                .parameters
                .get("redis_url")
                .and_then(|v| v.as_str())
                .unwrap_or("redis://localhost:6379")
                .to_string();

            let config = CommandSubscriberConfig {
                channel_id,
                redis_url,
            };

            let (tx, _rx) = tokio::sync::mpsc::channel(100);
            let subscriber = CommandSubscriber::new(config, tx).await?;
            let subscriber_arc = Arc::new(RwLock::new(subscriber));

            // 启动订阅器
            let mut sub = subscriber_arc.write().await;
            sub.start().await?;
            drop(sub);

            Some(subscriber_arc)
        } else {
            None
        };

        // 创建通道条目
        let entry = ChannelEntry {
            channel: channel_arc.clone(),
            metadata: ChannelMetadata {
                name: channel_config.name.clone(),
                protocol_type,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
            command_subscriber,
        };

        // 插入通道
        self.channels.insert(channel_id, entry);

        info!(
            "Created channel {} with protocol {:?}",
            channel_id, protocol_type
        );
        Ok(channel_arc)
    }

    /// 获取通道
    pub async fn get_channel(&self, channel_id: u16) -> Option<Arc<RwLock<Box<dyn ComBase>>>> {
        self.channels.get(&channel_id).map(|entry| {
            // 更新最后访问时间
            let last_accessed = entry.metadata.last_accessed.clone();
            tokio::spawn(async move {
                let mut time = last_accessed.write().await;
                *time = std::time::Instant::now();
            });
            entry.channel.clone()
        })
    }

    /// 移除通道
    pub async fn remove_channel(&self, channel_id: u16) -> Result<()> {
        if let Some((_, entry)) = self.channels.remove(&channel_id) {
            // 停止命令订阅器
            if let Some(subscriber) = entry.command_subscriber {
                let mut sub = subscriber.write().await;
                sub.stop().await?;
            }

            // 断开连接
            let mut channel = entry.channel.write().await;
            channel.disconnect().await?;

            info!("Removed channel {}", channel_id);
            Ok(())
        } else {
            Err(ComSrvError::InvalidOperation(format!(
                "Channel {} not found",
                channel_id
            )))
        }
    }

    /// 获取所有通道ID
    pub fn get_channel_ids(&self) -> Vec<u16> {
        self.channels.iter().map(|entry| *entry.key()).collect()
    }

    /// 获取通道统计信息
    pub async fn get_channel_stats(&self, channel_id: u16) -> Option<ChannelStats> {
        if let Some(entry) = self.channels.get(&channel_id) {
            let channel = entry.channel.read().await;
            let status = channel.get_status().await;

            Some(ChannelStats {
                channel_id,
                name: entry.metadata.name.clone(),
                protocol_type: entry.metadata.protocol_type,
                is_connected: status.is_connected,
                created_at: entry.metadata.created_at,
                last_accessed: *entry.metadata.last_accessed.read().await,
                points_count: status.points_count,
                success_count: status.success_count,
                error_count: status.error_count,
            })
        } else {
            None
        }
    }

    /// 获取所有通道统计信息
    pub async fn get_all_channel_stats(&self) -> Vec<ChannelStats> {
        let mut stats = Vec::new();

        for entry in self.channels.iter() {
            let channel_id = *entry.key();
            if let Some(channel_stats) = self.get_channel_stats(channel_id).await {
                stats.push(channel_stats);
            }
        }

        stats
    }

    /// 清理所有通道
    pub async fn cleanup(&self) -> Result<()> {
        let channel_ids: Vec<u16> = self.get_channel_ids();

        for channel_id in channel_ids {
            if let Err(e) = self.remove_channel(channel_id).await {
                error!("Failed to remove channel {}: {}", channel_id, e);
            }
        }

        Ok(())
    }

    /// 获取通道数量
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// 获取运行中的通道数量
    pub async fn running_channel_count(&self) -> usize {
        let mut count = 0;
        for entry in self.channels.iter() {
            let channel = entry.value();
            let channel_guard = channel.channel.read().await;
            if channel_guard.is_connected() {
                count += 1;
            }
        }
        count
    }

    /// 获取通道元数据
    pub async fn get_channel_metadata(&self, channel_id: u16) -> Option<(String, String)> {
        self.channels.get(&channel_id).map(|entry| {
            let metadata = &entry.metadata;
            (
                metadata.name.clone(),
                format!("{:?}", metadata.protocol_type),
            )
        })
    }
}

impl Default for ProtocolFactory {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 辅助结构和函数
// ============================================================================

/// 通道统计信息
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel_id: u16,
    pub name: String,
    pub protocol_type: ProtocolType,
    pub is_connected: bool,
    pub created_at: std::time::Instant,
    pub last_accessed: std::time::Instant,
    pub points_count: usize,
    pub success_count: u64,
    pub error_count: u64,
}

/// 创建默认工厂
pub fn create_default_factory() -> ProtocolFactory {
    ProtocolFactory::new()
}

/// 创建带自定义协议的工厂
pub fn create_factory_with_custom_protocols(
    protocols: Vec<Arc<dyn ProtocolClientFactory>>,
) -> ProtocolFactory {
    let factory = ProtocolFactory::new();

    for protocol in protocols {
        factory.register_protocol_factory(protocol);
    }

    factory
}

// ============================================================================
// 测试支持
// ============================================================================

#[cfg(any(test, feature = "test-utils"))]
pub mod test_support {
    use super::*;
    use crate::core::combase::core::{ChannelStatus, DefaultProtocol, PointData, RedisValue};
    use std::sync::atomic::{AtomicBool, Ordering};

    /// 测试用的Mock通信基础实现
    pub struct MockComBase {
        name: String,
        channel_id: u16,
        protocol_type: String,
        is_connected: AtomicBool,
        status: Arc<RwLock<ChannelStatus>>,
    }

    impl MockComBase {
        pub fn new(name: &str, channel_id: u16, protocol_type: &str) -> Self {
            Self {
                name: name.to_string(),
                channel_id,
                protocol_type: protocol_type.to_string(),
                is_connected: AtomicBool::new(false),
                status: Arc::new(RwLock::new(ChannelStatus::default())),
            }
        }
    }

    #[async_trait]
    impl ComBase for MockComBase {
        fn name(&self) -> &str {
            &self.name
        }

        fn protocol_type(&self) -> &str {
            &self.protocol_type
        }

        fn is_connected(&self) -> bool {
            self.is_connected.load(Ordering::Relaxed)
        }

        async fn get_status(&self) -> ChannelStatus {
            self.status.read().await.clone()
        }

        async fn initialize(&mut self, _channel_config: &ChannelConfig) -> Result<()> {
            Ok(())
        }

        async fn connect(&mut self) -> Result<()> {
            self.is_connected.store(true, Ordering::Relaxed);
            let mut status = self.status.write().await;
            status.is_connected = true;
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<()> {
            self.is_connected.store(false, Ordering::Relaxed);
            let mut status = self.status.write().await;
            status.is_connected = false;
            Ok(())
        }

        async fn read_four_telemetry(
            &self,
            _telemetry_type: &str,
        ) -> Result<HashMap<u32, PointData>> {
            Ok(HashMap::new())
        }

        async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
            Ok(commands.into_iter().map(|(id, _)| (id, true)).collect())
        }

        async fn adjustment(
            &mut self,
            adjustments: Vec<(u32, RedisValue)>,
        ) -> Result<Vec<(u32, bool)>> {
            Ok(adjustments.into_iter().map(|(id, _)| (id, true)).collect())
        }

        async fn update_points(
            &mut self,
            _mappings: Vec<crate::core::config::UnifiedPointMapping>,
        ) -> Result<()> {
            Ok(())
        }
    }

    /// Mock协议工厂
    pub struct MockProtocolFactory;

    #[async_trait]
    impl ProtocolClientFactory for MockProtocolFactory {
        fn protocol_type(&self) -> ProtocolType {
            ProtocolType::ModbusTcp
        }

        async fn create_client(
            &self,
            channel_config: &ChannelConfig,
            _config_value: ConfigValue,
        ) -> Result<Box<dyn ComBase>> {
            Ok(Box::new(MockComBase::new(
                &channel_config.name,
                channel_config.id,
                "mock",
            )))
        }

        fn validate_config(&self, _config: &ConfigValue) -> Result<()> {
            Ok(())
        }

        fn get_config_template(&self) -> ConfigValue {
            serde_json::json!({
                "type": "mock",
                "parameters": {}
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[tokio::test]
    async fn test_protocol_factory_creation() {
        let factory = ProtocolFactory::new();
        assert_eq!(factory.get_channel_ids().len(), 0);
        assert_eq!(factory.get_registered_protocols().len(), 0);
    }

    #[tokio::test]
    async fn test_register_protocol() {
        let factory = ProtocolFactory::new();
        let mock_factory = Arc::new(MockProtocolFactory);

        factory.register_protocol_factory(mock_factory);

        let protocols = factory.get_registered_protocols();
        assert_eq!(protocols.len(), 1);
        assert_eq!(protocols[0], ProtocolType::ModbusTcp);
    }

    #[tokio::test]
    async fn test_create_channel() {
        let factory = ProtocolFactory::new();
        let mock_factory = Arc::new(MockProtocolFactory);
        factory.register_protocol_factory(mock_factory);

        let channel_config = ChannelConfig {
            id: 1,
            name: "Test Channel".to_string(),
            protocol: "modbus_tcp".to_string(),
            parameters: HashMap::new(),
            enabled: true,
            enable_control: Some(false),
            redis_url: None,
            table_config: None,
            csv_base_path: None,
            point_count: Some(10),
            logging: ChannelLoggingConfig::default(),
        };

        let channel = factory.create_channel(&channel_config, None).await.unwrap();

        assert_eq!(factory.get_channel_ids(), vec![1]);

        let channel_guard = channel.read().await;
        assert_eq!(channel_guard.name(), "Test Channel");
        assert_eq!(channel_guard.protocol_type(), "mock");
    }
}
