//! Plugin Manager
//!
//! This module provides high-level plugin management functionality,
//! including plugin discovery, loading, and lifecycle management.

use tracing::{info, warn};

use super::plugin_registry::{discovery, PluginRegistry};
use crate::utils::Result;

/// Plugin manager for coordinating plugin operations
#[derive(Debug)]
pub struct PluginManager;

impl PluginManager {
    /// Initialize the plugin system
    pub fn initialize() -> Result<()> {
        info!("Initializing plugin system...");

        // Load all built-in and external plugins
        discovery::load_all_plugins()?;

        // Get registry statistics
        let registry = PluginRegistry::global();
        let stats = registry.read().unwrap().get_statistics();

        info!(
            "Plugin system initialized: {} plugins loaded ({} enabled)",
            stats.total_plugins, stats.enabled_plugins
        );

        Ok(())
    }

    /// List all available plugins
    pub fn list_plugins() -> Vec<String> {
        let registry = PluginRegistry::global();
        let reg = registry.read().unwrap();
        reg.list_plugin_ids()
    }

    /// Get plugin metadata
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

    /// Check if a plugin is available
    pub fn is_plugin_available(plugin_id: &str) -> bool {
        let registry = PluginRegistry::global();
        let mut reg = registry.write().unwrap();

        // Try to load from factory if not already loaded
        if let Err(e) = reg.load_plugin_from_factory(plugin_id) {
            warn!("Failed to load plugin '{}': {e}", plugin_id);
            false
        } else {
            true
        }
    }

    /// Enable or disable a plugin
    pub fn set_plugin_enabled(plugin_id: &str, enabled: bool) -> Result<()> {
        let registry = PluginRegistry::global();
        let mut reg = registry.write().unwrap();
        reg.set_plugin_enabled(plugin_id, enabled)
    }

    /// Get plugin CLI commands
    pub fn get_plugin_commands(plugin_id: &str) -> Vec<String> {
        let registry = PluginRegistry::global();
        let reg = registry.read().unwrap();

        if let Some(plugin) = reg.get_plugin(plugin_id) {
            plugin
                .cli_commands()
                .into_iter()
                .map(|cmd| format!("{}: {}", cmd.name, cmd.description))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Generate example configuration for a plugin
    pub fn generate_example_config(plugin_id: &str) -> Option<String> {
        let registry = PluginRegistry::global();
        let reg = registry.read().unwrap();

        if let Some(plugin) = reg.get_plugin(plugin_id) {
            let config = plugin.generate_example_config();
            Some(serde_yaml::to_string(&config).unwrap_or_default())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_initialization() {
        // Test that the plugin manager can be initialized
        assert!(PluginManager::initialize().is_ok());

        // Check that plugins are loaded
        let plugins = PluginManager::list_plugins();
        assert!(!plugins.is_empty());
    }
}
