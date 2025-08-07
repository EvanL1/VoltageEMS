//! Plugin core module
//!
//! Contains core implementation of plugin manager, registry and storage functionality

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
    PointData, PointStorage as VoltagePointStorage, PointUpdate as VoltagePointUpdate, RetryConfig,
    RtdbStorage,
};
use crate::utils::error::{ComSrvError as Error, Result};

// ============================================================================
// Plugin registry
// ============================================================================

/// Global plugin registry instance
static PLUGIN_REGISTRY: Lazy<Arc<RwLock<PluginRegistry>>> =
    Lazy::new(|| Arc::new(RwLock::new(PluginRegistry::new())));

/// Get global plugin registry
pub fn get_plugin_registry() -> Arc<RwLock<PluginRegistry>> {
    PLUGIN_REGISTRY.clone()
}

/// Plugin registry, manages all registered protocol plugins
#[derive(Debug, Default)]
pub struct PluginRegistry {
    /// Registered plugins
    plugins: HashMap<String, PluginEntry>,
    /// Plugin factory functions
    factories: HashMap<String, PluginFactory>,
    /// Plugin load order
    load_order: Vec<String>,
}

/// Registered plugin entry
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
    /// Create new plugin registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Get global registry instance
    pub fn global() -> Arc<RwLock<Self>> {
        PLUGIN_REGISTRY.clone()
    }

    /// Register plugin
    pub fn register_plugin(&mut self, plugin: Box<dyn ProtocolPlugin>) -> Result<()> {
        let metadata = plugin.metadata();
        let plugin_id = metadata.id.clone();

        if self.plugins.contains_key(&plugin_id) {
            return Err(Error::ConfigError(format!(
                "Plugin '{plugin_id}' is already registered"
            )));
        }

        // Validate version number
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

    /// Register plugin factory
    pub fn register_factory(&mut self, plugin_id: &str, factory: PluginFactory) -> Result<()> {
        debug!("Registering factory for plugin: {}", plugin_id);
        self.factories.insert(plugin_id.to_string(), factory);
        Ok(())
    }

    /// Get plugin factory
    pub fn get_factory(&self, plugin_id: &str) -> Option<&PluginFactory> {
        self.factories.get(plugin_id)
    }

    /// Get plugin metadata
    pub fn get_plugin_metadata(&self, plugin_id: &str) -> Option<ProtocolMetadata> {
        self.plugins
            .get(plugin_id)
            .map(|entry| entry.metadata.clone())
    }

    /// List all plugin IDs
    pub fn list_plugin_ids(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Get statistics
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

/// Plugin statistics
#[derive(Debug)]
pub struct PluginStatistics {
    pub total_plugins: usize,
    pub enabled_plugins: usize,
    pub plugins_by_type: HashMap<String, usize>,
}

// ============================================================================
// Plugin manager
// ============================================================================

/// Plugin manager, coordinates plugin operations
#[derive(Debug)]
pub struct PluginManager;

impl PluginManager {
    /// Initialize plugin system
    pub fn initialize() -> Result<()> {
        info!("Initializing plugin system...");

        // Load built-in plugins
        discovery::load_all_plugins()?;

        // Get statistics
        let registry = PluginRegistry::global();
        let stats = registry
            .read()
            .expect("plugin registry lock should not be poisoned")
            .get_statistics();

        info!(
            "Plugin system initialized: {} plugins loaded ({} enabled)",
            stats.total_plugins, stats.enabled_plugins
        );

        Ok(())
    }

    /// List all available plugins
    pub fn list_plugins() -> Vec<String> {
        let registry = PluginRegistry::global();
        let reg = registry
            .read()
            .expect("plugin registry lock should not be poisoned");
        reg.list_plugin_ids()
    }

    /// Get plugin information
    pub fn get_plugin_info(plugin_id: &str) -> Option<String> {
        let registry = PluginRegistry::global();
        let reg = registry
            .read()
            .expect("plugin registry lock should not be poisoned");

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

    /// Enable plugin
    pub fn enable_plugin(plugin_id: &str) -> Result<()> {
        let registry = PluginRegistry::global();
        let mut reg = registry
            .write()
            .expect("plugin registry lock should not be poisoned");

        if let Some(entry) = reg.plugins.get_mut(plugin_id) {
            entry.enabled = true;
            info!("Plugin '{}' enabled", plugin_id);
            Ok(())
        } else {
            Err(Error::ConfigError(format!(
                "Plugin '{plugin_id}' not found"
            )))
        }
    }

    /// Disable plugin
    pub fn disable_plugin(plugin_id: &str) -> Result<()> {
        let registry = PluginRegistry::global();
        let mut reg = registry
            .write()
            .expect("plugin registry lock should not be poisoned");

        if let Some(entry) = reg.plugins.get_mut(plugin_id) {
            entry.enabled = false;
            warn!("Plugin '{}' disabled", plugin_id);
            Ok(())
        } else {
            Err(Error::ConfigError(format!(
                "Plugin '{plugin_id}' not found"
            )))
        }
    }
}

// ============================================================================
// Plugin storage
// ============================================================================

// Type constant definitions
const TYPE_MEASUREMENT: &str = "T";
const TYPE_SIGNAL: &str = "S";
const TYPE_CONTROL: &str = "C";
const TYPE_ADJUSTMENT: &str = "A";

/// `Convert TelemetryType to Redis storage type abbreviation`
pub fn telemetry_type_to_redis(telemetry_type: &TelemetryType) -> &'static str {
    match telemetry_type {
        TelemetryType::Telemetry => TYPE_MEASUREMENT,
        TelemetryType::Signal => TYPE_SIGNAL,
        TelemetryType::Control => TYPE_CONTROL,
        TelemetryType::Adjustment => TYPE_ADJUSTMENT,
    }
}

/// Plugin point update data
#[derive(Debug, Clone)]
pub struct PluginPointUpdate {
    pub channel_id: u16,
    pub telemetry_type: TelemetryType,
    pub point_id: u32,
    pub value: f64,
    pub timestamp: i64,
    pub raw_value: Option<f64>,
}

/// Plugin point configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginPointConfig {
    pub name: String,
    pub unit: String,
    pub scale: f64,
    pub offset: f64,
    pub description: Option<String>,
}

/// Plugin storage trait
#[async_trait]
pub trait PluginStorage: Send + Sync {
    /// Write single point data
    async fn write_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// Write point data (with raw value)
    async fn write_point_with_raw(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        value: f64,
        _raw_value: f64,
    ) -> Result<()> {
        // Default implementation: only write processed value
        self.write_point(channel_id, telemetry_type, point_id, value)
            .await
    }

    /// Write point data (with scaling parameters)
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

    /// Batch write point data
    async fn write_points(&self, updates: Vec<PluginPointUpdate>) -> Result<()>;

    /// Read single point data
    async fn read_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>>;

    /// Write point configuration
    async fn write_config(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
        config: PluginPointConfig,
    ) -> Result<()>;

    /// Initialize point
    async fn initialize_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<()>;
}

/// Default plugin storage implementation
#[derive(Debug)]
pub struct DefaultPluginStorage {
    storage: Arc<RtdbStorage>,
}

impl DefaultPluginStorage {
    /// Create new storage instance
    pub async fn new(redis_url: String) -> Result<Self> {
        let storage = RtdbStorage::with_config(&redis_url, RetryConfig::default()).await?;

        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// `Create from existing RtdbStorage`
    pub fn from_storage(storage: Arc<RtdbStorage>) -> Self {
        Self { storage }
    }

    /// Create from environment variables
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
                        value: update.value.into(),
                        timestamp: update.timestamp,
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
            Some(data) => Ok(Some((data.value.into(), data.timestamp))),
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
        let _key = format!("cfg:{channel_id}:{point_type}:{point_id}");
        let _value =
            serde_json::to_string(&config).map_err(|e| Error::SerializationError(e.to_string()))?;

        // Temporary implementation, can extend Storage interface later
        Ok(())
    }

    async fn initialize_point(
        &self,
        channel_id: u16,
        telemetry_type: &TelemetryType,
        point_id: u32,
    ) -> Result<()> {
        // Write initial value 0
        self.write_point(channel_id, telemetry_type, point_id, 0.0)
            .await
    }
}

// ============================================================================
// Plugin discovery module
// ============================================================================

pub mod discovery {
    use super::{PluginRegistry, Result};

    /// Load all plugins
    pub fn load_all_plugins() -> Result<()> {
        // Load built-in protocol plugins
        #[cfg(feature = "modbus")]
        load_modbus_plugin()?;

        // Load virtual protocol plugin (for testing)
        load_virt_plugin()?;

        Ok(())
    }

    #[cfg(feature = "modbus")]
    fn load_modbus_plugin() -> Result<()> {
        use crate::plugins::protocols::modbus;
        let registry = PluginRegistry::global();
        let mut reg = registry
            .write()
            .expect("plugin registry lock should not be poisoned");

        let plugin = Box::new(modbus::ModbusTcpPlugin);
        reg.register_plugin(plugin)?;
        reg.register_factory("modbus_tcp", modbus::create_plugin)?;
        reg.register_factory("modbus_rtu", modbus::create_plugin)?;

        Ok(())
    }

    fn load_virt_plugin() -> Result<()> {
        use crate::plugins::protocols::virt;
        let registry = PluginRegistry::global();
        let mut reg = registry
            .write()
            .expect("plugin registry lock should not be poisoned");

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

        // Test plugin registration
        struct TestPlugin;
        #[async_trait::async_trait]
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

            fn config_template(&self) -> Vec<crate::plugins::traits::ConfigTemplate> {
                vec![]
            }

            fn validate_config(&self, _config: &HashMap<String, serde_json::Value>) -> Result<()> {
                Ok(())
            }

            async fn create_instance(
                &self,
                _channel_config: crate::core::config::types::ChannelConfig,
            ) -> Result<Box<dyn crate::core::combase::ComBase>> {
                Err(crate::ComSrvError::protocol("Test plugin not implemented"))
            }
        }

        let plugin = Box::new(TestPlugin);
        assert!(registry.register_plugin(plugin).is_ok());

        // Test duplicate registration
        let plugin2 = Box::new(TestPlugin);
        assert!(registry.register_plugin(plugin2).is_err());
    }

    #[test]
    fn test_telemetry_type_conversion() {
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Telemetry), "T");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Signal), "S");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Control), "C");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Adjustment), "A");
    }
}
