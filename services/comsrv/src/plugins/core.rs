//! Plugin core module
//!
//! Simplified plugin manager and registry functionality

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tracing::info;

use super::traits::{PluginFactory, ProtocolMetadata, ProtocolPlugin};
use crate::core::config::TelemetryType;
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
}

impl std::fmt::Debug for PluginEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginEntry")
            .field("registered_at", &self.registered_at)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl PluginRegistry {
    /// Create new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            factories: HashMap::new(),
            load_order: Vec::new(),
        }
    }

    /// Register a plugin
    pub fn register(&mut self, id: String, plugin: Box<dyn ProtocolPlugin>) -> Result<()> {
        if self.plugins.contains_key(&id) {
            return Err(Error::InternalError(format!(
                "Plugin with ID '{id}' is already registered"
            )));
        }

        info!("Registering plugin: {}", id);

        self.plugins.insert(
            id.clone(),
            PluginEntry {
                plugin,
                registered_at: SystemTime::now(),
                enabled: true,
            },
        );

        self.load_order.push(id);
        Ok(())
    }

    /// Register a plugin factory
    pub fn register_factory(
        &mut self,
        protocol_type: String,
        factory: PluginFactory,
    ) -> Result<()> {
        if self.factories.contains_key(&protocol_type) {
            return Err(Error::InternalError(format!(
                "Factory for protocol '{}' is already registered",
                protocol_type
            )));
        }

        info!("Registering factory for protocol: {}", protocol_type);
        self.factories.insert(protocol_type, factory);
        Ok(())
    }

    /// Get a plugin by ID
    pub fn get(&self, id: &str) -> Option<&dyn ProtocolPlugin> {
        self.plugins.get(id).map(|entry| entry.plugin.as_ref())
    }

    /// Get a plugin factory
    pub fn get_factory(&self, protocol_type: &str) -> Option<&PluginFactory> {
        self.factories.get(protocol_type)
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<String> {
        self.load_order.clone()
    }

    /// Check if a plugin is registered
    pub fn is_registered(&self, id: &str) -> bool {
        self.plugins.contains_key(id)
    }

    /// Unregister a plugin
    pub fn unregister(&mut self, id: &str) -> Result<()> {
        if self.plugins.remove(id).is_none() {
            return Err(Error::InternalError(format!(
                "Plugin with ID '{id}' is not registered"
            )));
        }

        self.load_order.retain(|x| x != id);
        info!("Unregistered plugin: {}", id);
        Ok(())
    }
}

// ============================================================================
// Plugin manager
// ============================================================================

/// Plugin manager - simplified without storage abstractions
pub struct PluginManager;

impl PluginManager {
    /// Initialize plugin system
    pub fn initialize() -> Result<()> {
        info!("Initializing plugin system");

        // Register built-in protocol factories
        let _registry = PLUGIN_REGISTRY.write().unwrap();

        // Register Modbus factory - simplified placeholder
        // Real factories would be registered by the actual protocol modules

        Ok(())
    }

    /// Load a plugin by protocol type
    pub async fn load_plugin(
        protocol_type: &str,
        _config: serde_json::Value,
    ) -> Result<Box<dyn ProtocolPlugin>> {
        let registry = PLUGIN_REGISTRY.read().unwrap();

        let factory = registry.get_factory(protocol_type).ok_or_else(|| {
            Error::InternalError(format!("No factory for protocol: {}", protocol_type))
        })?;

        Ok(factory())
    }

    /// List available plugins
    pub fn list_plugins() -> Vec<String> {
        let registry = PLUGIN_REGISTRY.read().unwrap();
        registry.list_plugins()
    }

    /// Check if plugin is available
    pub fn is_plugin_available(id: &str) -> bool {
        let registry = PLUGIN_REGISTRY.read().unwrap();
        registry.is_registered(id)
    }

    /// Get plugin metadata
    pub fn get_plugin_metadata(id: &str) -> Option<ProtocolMetadata> {
        let registry = PLUGIN_REGISTRY.read().unwrap();
        registry.get(id).map(|plugin| plugin.metadata())
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Convert telemetry type to Redis type string
pub fn telemetry_type_to_redis(telemetry_type: &TelemetryType) -> &'static str {
    match telemetry_type {
        TelemetryType::Telemetry => "T",
        TelemetryType::Signal => "S",
        TelemetryType::Control => "C",
        TelemetryType::Adjustment => "A",
    }
}

/// Plugin point update for batch operations
#[derive(Debug, Clone)]
pub struct PluginPointUpdate {
    pub telemetry_type: TelemetryType,
    pub point_id: u32,
    pub value: f64,
    pub raw_value: Option<f64>,
}

// ============================================================================
// Test support
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry() {
        let registry = PluginRegistry::new();

        // Test registry is initially empty
        assert!(registry.get_factory("test").is_none());
        assert!(registry.get_factory("unknown").is_none());
        assert_eq!(registry.list_plugins().len(), 0);
    }

    #[test]
    fn test_telemetry_type_conversion() {
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Telemetry), "T");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Signal), "S");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Control), "C");
        assert_eq!(telemetry_type_to_redis(&TelemetryType::Adjustment), "A");
    }
}
