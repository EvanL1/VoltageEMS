//! Protocol Plugin System
//!
//! This module provides a flexible plugin architecture for protocol implementations,
//! enabling dynamic loading, configuration management, and standardized interfaces.

pub mod protocol_plugin;
pub mod plugin_registry;
pub mod config_template;
pub mod plugin_manager;

// Re-export main types
pub use protocol_plugin::{
    ProtocolPlugin, ProtocolMetadata, ConfigTemplate, ValidationRule,
    CliCommand, CliArgument, create_plugin_instance,
};
pub use plugin_registry::{PluginRegistry, PluginStatistics};
pub use config_template::{
    ConfigSchema, ConfigSection, ConfigParameter, ParameterType,
    ConfigValidator, ValidationResult, ConfigGenerator,
};
pub use plugin_manager::PluginManager;

// Re-export macros
pub use crate::{protocol_plugin, register_plugin};

/// Initialize the plugin system
pub fn init_plugin_system() -> crate::utils::Result<()> {
    tracing::info!("Initializing protocol plugin system");
    
    // Load built-in plugins
    plugin_registry::discovery::load_all_plugins()?;
    
    // Log loaded plugins
    let registry = PluginRegistry::global();
    let registry = registry.read().unwrap();
    let stats = registry.get_statistics();
    
    tracing::info!(
        "Plugin system initialized: {} plugins loaded ({} enabled)",
        stats.total_plugins,
        stats.enabled_plugins
    );
    
    Ok(())
}