//! 插件核心模块
//!
//! 包含插件管理器、注册表和存储功能的核心实现

use async_trait::async_trait;
use once_cell::sync::Lazy;
use semver::Version;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tracing::{debug, info, warn};

use super::traits::{PluginFactory, ProtocolMetadata, ProtocolPlugin};
use crate::core::config::TelemetryType;
use crate::storage::{
    PointData, PointStorage as VoltagePointStorage, PointUpdate as VoltagePointUpdate,
    PublisherConfig, RetryConfig, Storage,
};
use crate::utils::error::{ComSrvError as Error, Result};

// ============================================================================
// 插件注册表
// ============================================================================

/// 全局插件注册表实例
static PLUGIN_REGISTRY: Lazy<Arc<RwLock<PluginRegistry>>> =
    Lazy::new(|| Arc::new(RwLock::new(PluginRegistry::new())));

/// 获取全局插件注册表
pub fn get_plugin_registry() -> Arc<RwLock<PluginRegistry>> {
    PLUGIN_REGISTRY.clone()
}

/// 插件注册表，管理所有已注册的协议插件
#[derive(Debug)]
pub struct PluginRegistry {
    /// 已注册的插件
    plugins: HashMap<String, PluginEntry>,
    /// 插件工厂函数
    factories: HashMap<String, PluginFactory>,
    /// 插件加载顺序
    load_order: Vec<String>,
}

/// 已注册插件的条目
#[allow(dead_code)]
struct PluginEntry {
    plugin: Box<dyn ProtocolPlugin>,
    registered_at: SystemTime,
    enabled: bool,
    metadata: ProtocolMetadata,
}

impl std::fmt::Debug for PluginEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginEntry")
            .field("plugin", &"<ProtocolPlugin>")
            .field("registered_at", &self.registered_at)
            .field("enabled", &self.enabled)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl PluginRegistry {
    /// 创建新的插件注册表
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            factories: HashMap::new(),
            load_order: Vec::new(),
        }
    }

    /// 获取全局注册表实例
    pub fn global() -> Arc<RwLock<Self>> {
        PLUGIN_REGISTRY.clone()
    }

    /// 注册插件
    pub fn register_plugin(&mut self, plugin: Box<dyn ProtocolPlugin>) -> Result<()> {
        let metadata = plugin.metadata();
        let plugin_id = metadata.id.clone();

        if self.plugins.contains_key(&plugin_id) {
            return Err(Error::ConfigError(format!(
                "Plugin '{}' is already registered",
                plugin_id
            )));
        }

        // 验证版本号
        if let Err(e) = Version::parse(&metadata.version) {
            return Err(Error::ConfigError(format!(
                "Invalid version '{}' for plugin '{}': {}",
                metadata.version, plugin_id, e
            )));
        }

        info!(
            "Registering protocol plugin: {} v{} - {}",
            metadata.id, metadata.version, metadata.description
        );

        let entry = PluginEntry {
            plugin,
            registered_at: SystemTime::now(),
            enabled: true,
            metadata: metadata.clone(),
        };

        self.plugins.insert(plugin_id.clone(), entry);
        self.load_order.push(plugin_id);

        Ok(())
    }

    /// 注册插件工厂
    pub fn register_factory(&mut self, plugin_id: &str, factory: PluginFactory) -> Result<()> {
        debug!("Registering factory for plugin: {}", plugin_id);
        self.factories.insert(plugin_id.to_string(), factory);
        Ok(())
    }

    /// 获取插件工厂
    pub fn get_factory(&self, plugin_id: &str) -> Option<&PluginFactory> {
        self.factories.get(plugin_id)
    }

    /// 获取插件元数据
    pub fn get_plugin_metadata(&self, plugin_id: &str) -> Option<ProtocolMetadata> {
        self.plugins
            .get(plugin_id)
            .map(|entry| entry.metadata.clone())
    }

    /// 列出所有插件ID
    pub fn list_plugin_ids(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> PluginStatistics {
        let total_plugins = self.plugins.len();
        let enabled_plugins = self.plugins.values().filter(|e| e.enabled).count();

        let plugins_by_type = HashMap::new();

        PluginStatistics {
            total_plugins,
            enabled_plugins,
            plugins_by_type,
        }
    }
}

/// 插件统计信息
#[derive(Debug)]
pub struct PluginStatistics {
    pub total_plugins: usize,
    pub enabled_plugins: usize,
    pub plugins_by_type: HashMap<String, usize>,
}

// ============================================================================
// 插件管理器
// ============================================================================

/// 插件管理器，协调插件操作
#[derive(Debug)]
pub struct PluginManager;

impl PluginManager {
    /// 初始化插件系统
    pub fn initialize() -> Result<()> {
        info!("Initializing plugin system...");

        // 加载内置插件
        discovery::load_all_plugins()?;

        // 获取统计信息
        let registry = PluginRegistry::global();
        let stats = registry.read().unwrap().get_statistics();

        info!(
            "Plugin system initialized: {} plugins loaded ({} enabled)",
            stats.total_plugins, stats.enabled_plugins
        );

        Ok(())
    }

    /// 列出所有可用的插件
    pub fn list_plugins() -> Vec<String> {
        let registry = PluginRegistry::global();
        let reg = registry.read().unwrap();
        reg.list_plugin_ids()
    }

    /// 获取插件信息
    pub fn get_plugin_info(plugin_id: &str) -> Option<String> {
        let registry = PluginRegistry::global();
        let reg = registry.read().unwrap();

        reg.get_plugin_metadata(plugin_id).map(|metadata| {
            format!(
                "Plugin: {} ({})\nVersion: {}\nDescription: {}\nAuthor: {}\nFeatures: {:?}",
                metadata.name,
                metadata.id,
                metadata.version,
                metadata.description,
                metadata.author,
                metadata.features
            )
        })
    }

    /// 启用插件
    pub fn enable_plugin(plugin_id: &str) -> Result<()> {
        let registry = PluginRegistry::global();
        let mut reg = registry.write().unwrap();

        if let Some(entry) = reg.plugins.get_mut(plugin_id) {
            entry.enabled = true;
            info!("Plugin '{}' enabled", plugin_id);
            Ok(())
        } else {
            Err(Error::ConfigError(format!(
                "Plugin '{}' not found",
                plugin_id
            )))
        }
    }

    /// 禁用插件
    pub fn disable_plugin(plugin_id: &str) -> Result<()> {
        let registry = PluginRegistry::global();
        let mut reg = registry.write().unwrap();

        if let Some(entry) = reg.plugins.get_mut(plugin_id) {
            entry.enabled = false;
            warn!("Plugin '{}' disabled", plugin_id);
            Ok(())
        } else {
            Err(Error::ConfigError(format!(
                "Plugin '{}' not found",
                plugin_id
            )))
        }
    }
}

// ============================================================================
// 插件存储
// ============================================================================

// 类型常量定义
const TYPE_MEASUREMENT: &str = "m";
const TYPE_SIGNAL: &str = "s";
const TYPE_CONTROL: &str = "c";
const TYPE_ADJUSTMENT: &str = "a";

/// 将TelemetryType转换为Redis存储的类型缩写
pub fn telemetry_type_to_redis(telemetry_type: &TelemetryType) -> &'static str {
    match telemetry_type {
        TelemetryType::Telemetry => TYPE_MEASUREMENT,
        TelemetryType::Signal => TYPE_SIGNAL,
        TelemetryType::Control => TYPE_CONTROL,
        TelemetryType::Adjustment => TYPE_ADJUSTMENT,
    }
}

/// 插件点位更新数据
#[derive(Debug, Clone)]
pub struct PluginPointUpdate {
    pub channel_id: u16,
    pub telemetry_type: TelemetryType,
    pub point_id: u32,
    pub value: f64,
    pub timestamp: i64,
    pub quality: u8,
    pub raw_value: Option<f64>,
}

/// 插件点位配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginPointConfig {
    pub name: String,
    pub unit: String,
    pub scale: f64,
    pub offset: f64,
    pub description: Option<String>,
}

/// 插件存储trait
#[async_trait]
pub trait PluginStorage: Send + Sync {
    /// 写入单个点位数据
    async fn write_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// 写入点位数据（带原始值）
    async fn write_point_with_raw(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
        _raw_value: f64,
    ) -> Result<()> {
        // 默认实现：只写入处理后的值
        self.write_point(channel_id, telemetry_type, point_id, value)
            .await
    }

    /// 写入点位数据（带缩放参数）
    async fn write_point_with_scaling(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        raw_value: f64,
        scale: f64,
        offset: f64,
    ) -> Result<()> {
        let value = raw_value * scale + offset;
        self.write_point_with_raw(channel_id, telemetry_type, point_id, value, raw_value)
            .await
    }

    /// 批量写入点位数据
    async fn write_points(&self, updates: Vec<PluginPointUpdate>) -> Result<()>;

    /// 读取单个点位数据
    async fn read_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>>;

    /// 写入点位配置
    async fn write_config(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        config: PluginPointConfig,
    ) -> Result<()>;

    /// 初始化点位
    async fn initialize_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<()>;
}

/// 默认插件存储实现
pub struct DefaultPluginStorage {
    storage: Arc<Storage>,
}

impl DefaultPluginStorage {
    /// 创建新的存储实例
    pub async fn new(redis_url: String) -> Result<Self> {
        let storage = Storage::with_config(
            &redis_url,
            RetryConfig::default(),
            Some(PublisherConfig::default()),
        )
        .await?;

        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// 从已有的Storage创建
    pub fn from_storage(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// 从环境变量创建
    pub async fn from_env() -> Result<Self> {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        Self::new(redis_url).await
    }
}

#[async_trait]
impl PluginStorage for DefaultPluginStorage {
    async fn write_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        self.storage
            .write_point(channel_id, point_type, point_id, value)
            .await
    }

    async fn write_point_with_raw(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
        raw_value: f64,
    ) -> Result<()> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        self.storage
            .write_point_with_metadata(channel_id, point_type, point_id, value, Some(raw_value))
            .await
    }

    async fn write_points(&self, updates: Vec<PluginPointUpdate>) -> Result<()> {
        let voltage_updates: Vec<VoltagePointUpdate> = updates
            .into_iter()
            .map(|update| {
                let point_type = telemetry_type_to_redis(&update.telemetry_type);
                VoltagePointUpdate {
                    channel_id: update.channel_id,
                    point_type: point_type.to_string(),
                    point_id: update.point_id,
                    data: PointData {
                        value: update.value,
                        timestamp: update.timestamp,
                        quality: update.quality,
                    },
                    raw_value: update.raw_value,
                }
            })
            .collect();

        self.storage.write_batch(voltage_updates).await
    }

    async fn read_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        match self
            .storage
            .read_point(channel_id, point_type, point_id)
            .await?
        {
            Some(data) => Ok(Some((data.value, data.timestamp))),
            None => Ok(None),
        }
    }

    async fn write_config(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        config: PluginPointConfig,
    ) -> Result<()> {
        let point_type = telemetry_type_to_redis(telemetry_type);
        let key = format!("cfg:{}:{}:{}", channel_id, point_type, point_id);
        let value =
            serde_json::to_string(&config).map_err(|e| Error::SerializationError(e.to_string()))?;

        // 临时实现，后续可以扩展Storage接口
        Ok(())
    }

    async fn initialize_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<()> {
        // 写入初始值0
        self.write_point(channel_id, telemetry_type, point_id, 0.0)
            .await
    }
}

// ============================================================================
// 插件发现模块
// ============================================================================

pub mod discovery {
    use super::*;

    /// 加载所有插件
    pub fn load_all_plugins() -> Result<()> {
        // 加载内置协议插件
        #[cfg(feature = "modbus")]
        load_modbus_plugin()?;

        #[cfg(feature = "iec60870")]
        load_iec60870_plugin()?;

        #[cfg(feature = "can")]
        load_can_plugin()?;

        // 加载虚拟协议插件（用于测试）
        load_virt_plugin()?;

        Ok(())
    }

    #[cfg(feature = "modbus")]
    fn load_modbus_plugin() -> Result<()> {
        use crate::plugins::protocols::modbus;
        let registry = PluginRegistry::global();
        let mut reg = registry.write().unwrap();

        let plugin = Box::new(modbus::ModbusPlugin::new());
        reg.register_plugin(plugin)?;
        reg.register_factory("modbus_tcp", modbus::create_plugin)?;
        reg.register_factory("modbus_rtu", modbus::create_plugin)?;

        Ok(())
    }

    #[cfg(feature = "iec60870")]
    fn load_iec60870_plugin() -> Result<()> {
        use crate::plugins::protocols::iec60870;
        let registry = PluginRegistry::global();
        let mut reg = registry.write().unwrap();

        let plugin = Box::new(iec60870::Iec60870Plugin::new());
        reg.register_plugin(plugin)?;
        reg.register_factory("iec60870", iec60870::create_plugin)?;

        Ok(())
    }

    #[cfg(feature = "can")]
    fn load_can_plugin() -> Result<()> {
        use crate::plugins::protocols::can;
        let registry = PluginRegistry::global();
        let mut reg = registry.write().unwrap();

        let plugin = Box::new(can::CanPlugin::new());
        reg.register_plugin(plugin)?;
        reg.register_factory("can", can::create_plugin)?;

        Ok(())
    }

    fn load_virt_plugin() -> Result<()> {
        use crate::plugins::protocols::virt;
        let registry = PluginRegistry::global();
        let mut reg = registry.write().unwrap();

        let plugin = Box::new(virt::VirtPlugin::new());
        reg.register_plugin(plugin)?;
        reg.register_factory("virt", virt::create_plugin)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new();

        // 测试插件注册
        struct TestPlugin;
        impl ProtocolPlugin for TestPlugin {
            fn metadata(&self) -> ProtocolMetadata {
                ProtocolMetadata {
                    id: "test".to_string(),
                    name: "Test Plugin".to_string(),
                    version: "1.0.0".to_string(),
                    author: "Test Author".to_string(),
                    description: "Test Description".to_string(),
                    license: "Apache-2.0".to_string(),
                    features: vec![],
                    dependencies: HashMap::new(),
                }
            }
        }

        let plugin = Box::new(TestPlugin);
        assert!(registry.register_plugin(plugin).is_ok());

        // 测试重复注册
        let plugin2 = Box::new(TestPlugin);
        assert!(registry.register_plugin(plugin2).is_err());
    }

    #[test]
    fn test_telemetry_type_conversion() {
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Telemetry), "m");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Signal), "s");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Control), "c");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Adjustment), "a");
    }
}
