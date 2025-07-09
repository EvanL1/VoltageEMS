//! Plugin Registry System
//!
//! This module provides centralized plugin registration and management,
//! including dynamic loading, version management, and lifecycle control.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;
use tracing::{info, warn, error};
use semver::Version;

use super::protocol_plugin::{ProtocolPlugin, ProtocolMetadata, PluginFactory};
use crate::utils::{Result, ComSrvError as Error};

/// Global plugin registry instance
static PLUGIN_REGISTRY: Lazy<Arc<RwLock<PluginRegistry>>> = Lazy::new(|| {
    Arc::new(RwLock::new(PluginRegistry::new()))
});

/// Plugin registry for managing protocol plugins
pub struct PluginRegistry {
    /// Registered plugins by ID
    plugins: HashMap<String, PluginEntry>,
    /// Plugin factories for dynamic instantiation
    factories: HashMap<String, PluginFactory>,
    /// Plugin load order for dependency resolution
    load_order: Vec<String>,
}

/// Entry for a registered plugin
struct PluginEntry {
    /// Plugin instance
    plugin: Box<dyn ProtocolPlugin>,
    /// Registration timestamp
    registered_at: std::time::SystemTime,
    /// Whether the plugin is enabled
    enabled: bool,
    /// Plugin metadata cache
    metadata: ProtocolMetadata,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            factories: HashMap::new(),
            load_order: Vec::new(),
        }
    }
    
    /// Register a plugin with the registry
    pub fn register_plugin(
        &mut self,
        plugin: Box<dyn ProtocolPlugin>,
    ) -> Result<()> {
        let metadata = plugin.metadata();
        let plugin_id = metadata.id.clone();
        
        // Check for duplicate registration
        if self.plugins.contains_key(&plugin_id) {
            return Err(Error::ConfigError(format!(
                "Plugin '{}' is already registered",
                plugin_id
            )));
        }
        
        // Validate plugin version
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
        
        // Create plugin entry
        let entry = PluginEntry {
            plugin,
            registered_at: std::time::SystemTime::now(),
            enabled: true,
            metadata,
        };
        
        self.plugins.insert(plugin_id.clone(), entry);
        self.load_order.push(plugin_id);
        
        Ok(())
    }
    
    /// Register a plugin factory for lazy loading
    pub fn register_factory(
        &mut self,
        plugin_id: String,
        factory: PluginFactory,
    ) -> Result<()> {
        if self.factories.contains_key(&plugin_id) {
            return Err(Error::ConfigError(format!(
                "Factory for plugin '{}' is already registered",
                plugin_id
            )));
        }
        
        info!("Registering plugin factory for: {}", plugin_id);
        self.factories.insert(plugin_id, factory);
        
        Ok(())
    }
    
    /// Get a plugin by ID
    pub fn get_plugin(&self, plugin_id: &str) -> Option<&dyn ProtocolPlugin> {
        self.plugins
            .get(plugin_id)
            .filter(|entry| entry.enabled)
            .map(|entry| entry.plugin.as_ref())
    }
    
    /// Get all registered plugins
    pub fn get_all_plugins(&self) -> Vec<&dyn ProtocolPlugin> {
        self.plugins
            .values()
            .filter(|entry| entry.enabled)
            .map(|entry| entry.plugin.as_ref())
            .collect()
    }
    
    /// Get plugin metadata
    pub fn get_plugin_metadata(&self, plugin_id: &str) -> Option<&ProtocolMetadata> {
        self.plugins
            .get(plugin_id)
            .map(|entry| &entry.metadata)
    }
    
    /// List all registered plugin IDs
    pub fn list_plugin_ids(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }
    
    /// Enable or disable a plugin
    pub fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool) -> Result<()> {
        match self.plugins.get_mut(plugin_id) {
            Some(entry) => {
                entry.enabled = enabled;
                info!("Plugin '{}' {}", plugin_id, if enabled { "enabled" } else { "disabled" });
                Ok(())
            }
            None => Err(Error::ConfigError(format!("Plugin '{}' not found", plugin_id))),
        }
    }
    
    /// Load a plugin from factory
    pub fn load_plugin_from_factory(&mut self, plugin_id: &str) -> Result<()> {
        if self.plugins.contains_key(plugin_id) {
            return Ok(()); // Already loaded
        }
        
        match self.factories.get(plugin_id) {
            Some(factory) => {
                let plugin = factory();
                self.register_plugin(plugin)
            }
            None => Err(Error::ConfigError(format!(
                "No factory registered for plugin '{}'",
                plugin_id
            ))),
        }
    }
    
    /// Unregister a plugin
    pub fn unregister_plugin(&mut self, plugin_id: &str) -> Result<()> {
        if self.plugins.remove(plugin_id).is_some() {
            self.load_order.retain(|id| id != plugin_id);
            info!("Unregistered plugin: {}", plugin_id);
            Ok(())
        } else {
            Err(Error::ConfigError(format!("Plugin '{}' not found", plugin_id)))
        }
    }
    
    /// Get plugin statistics
    pub fn get_statistics(&self) -> PluginStatistics {
        PluginStatistics {
            total_plugins: self.plugins.len(),
            enabled_plugins: self.plugins.values().filter(|e| e.enabled).count(),
            total_factories: self.factories.len(),
            plugin_types: self.count_plugin_types(),
        }
    }
    
    /// Count plugins by type (based on features)
    fn count_plugin_types(&self) -> HashMap<String, usize> {
        let mut types = HashMap::new();
        for entry in self.plugins.values() {
            for feature in &entry.metadata.features {
                *types.entry(feature.clone()).or_insert(0) += 1;
            }
        }
        types
    }
}

/// Plugin registry statistics
#[derive(Debug, Clone)]
pub struct PluginStatistics {
    pub total_plugins: usize,
    pub enabled_plugins: usize,
    pub total_factories: usize,
    pub plugin_types: HashMap<String, usize>,
}

/// Global registry access functions
impl PluginRegistry {
    /// Get the global plugin registry
    pub fn global() -> Arc<RwLock<PluginRegistry>> {
        PLUGIN_REGISTRY.clone()
    }
    
    /// Register a plugin globally
    pub fn register_global(plugin: Box<dyn ProtocolPlugin>) -> Result<()> {
        let mut registry = PLUGIN_REGISTRY.write().unwrap();
        registry.register_plugin(plugin)
    }
    
    /// Register a factory globally
    pub fn register_factory_global(plugin_id: String, factory: PluginFactory) -> Result<()> {
        let mut registry = PLUGIN_REGISTRY.write().unwrap();
        registry.register_factory(plugin_id, factory)
    }
    
    /// Get a plugin from the global registry
    pub fn get_global(plugin_id: &str) -> Option<Box<dyn ProtocolPlugin>> {
        // First check if factory exists
        let has_factory = {
            let registry = PLUGIN_REGISTRY.read().unwrap();
            registry.factories.contains_key(plugin_id)
        };
        
        if has_factory {
            // Create a new instance from factory
            let registry = PLUGIN_REGISTRY.read().unwrap();
            registry.factories.get(plugin_id).map(|factory| factory())
        } else {
            None
        }
    }
}

/// Plugin discovery and loading utilities
pub mod discovery {
    use super::*;
    use std::path::Path;
    
    /// Discover plugins in a directory
    pub fn discover_plugins(plugin_dir: &Path) -> Result<Vec<String>> {
        // TODO: Implement dynamic library loading
        warn!("Plugin discovery not yet implemented for directory: {:?}", plugin_dir);
        Ok(Vec::new())
    }
    
    /// Load all discovered plugins
    pub fn load_all_plugins() -> Result<()> {
        // Built-in plugin registration
        register_builtin_plugins()?;
        
        // TODO: Load external plugins
        
        Ok(())
    }
    
    /// Register built-in plugins
    fn register_builtin_plugins() -> Result<()> {
        info!("Registering built-in protocol plugins");
        
        // Register Modbus plugins
        {
            use crate::core::protocols::modbus::plugin::{ModbusTcpPlugin, ModbusRtuPlugin};
            PluginRegistry::register_factory_global(
                "modbus_tcp".to_string(),
                || Box::new(ModbusTcpPlugin::default()),
            )?;
            PluginRegistry::register_factory_global(
                "modbus_rtu".to_string(),
                || Box::new(ModbusRtuPlugin::default()),
            )?;
        }
        
        // Register IEC 60870-5-104 plugin
        {
            use crate::core::protocols::iec60870::plugin::Iec104Plugin;
            PluginRegistry::register_factory_global(
                "iec104".to_string(),
                || Box::new(Iec104Plugin::default()),
            )?;
        }
        
        // Register CAN plugin
        {
            use crate::core::protocols::can::plugin::CanPlugin;
            PluginRegistry::register_factory_global(
                "can".to_string(),
                || Box::new(CanPlugin::default()),
            )?;
        }
        
        // Register Virtual plugin
        {
            use crate::core::protocols::virt::plugin::VirtualPlugin;
            PluginRegistry::register_factory_global(
                "virtual".to_string(),
                || Box::new(VirtualPlugin::default()),
            )?;
        }
        
        Ok(())
    }
}

/// Macro to register a plugin at compile time
#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        #[ctor::ctor]
        fn register() {
            let plugin = Box::new(<$plugin_type>::default());
            let _ = $crate::core::plugins::plugin_registry::PluginRegistry::register_global(plugin);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new();
        
        // Test registration
        // TODO: Add test plugin implementation
    }
}