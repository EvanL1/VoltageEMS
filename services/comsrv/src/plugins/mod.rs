//! Protocol Plugin System
//!
//! Provides a flexible plugin architecture, supporting dynamic loading,
//! configuration management and standardized interfaces for protocol implementations

pub mod grpc;
pub mod protocols;
pub mod registry;
pub mod traits;

// Re-export core types from registry module
pub use registry::{telemetry_type_to_redis, PluginManager, PluginPointUpdate, PluginRegistry};

// Re-export interface definitions from traits module
pub use traits::{
    create_plugin_instance, CliArgument, CliCommand, CliSubcommand, ConfigGenerator,
    ConfigParameter, ConfigSchema, ConfigSection, ConfigTemplate, ConfigValidator,
    DependencyCondition, EnumValue, ParameterDependency, ParameterType, ParameterValidation,
    PluginFactory, ProtocolMetadata, ProtocolPlugin, ValidationResult, ValidationRule,
};

// Re-export macros
pub use crate::{protocol_plugin, register_plugin};

// Factory functions for built-in protocols
fn create_modbus_tcp_plugin() -> Box<dyn traits::ProtocolPlugin> {
    Box::new(protocols::modbus::ModbusTcpPlugin)
}

fn create_modbus_rtu_plugin() -> Box<dyn traits::ProtocolPlugin> {
    Box::new(protocols::modbus::ModbusRtuPlugin)
}

fn create_virt_plugin() -> Box<dyn traits::ProtocolPlugin> {
    Box::new(protocols::virt::VirtPlugin::new())
}

/// Initialize plugin system and register built-in protocols
pub fn init_plugin_system() -> crate::utils::Result<()> {
    use crate::plugins::registry::get_plugin_registry;

    tracing::info!("Initializing protocol plugin system");

    // Get the plugin registry
    let registry = get_plugin_registry();
    let mut reg = registry.write().map_err(|e| {
        crate::utils::ComSrvError::InternalError(format!("Failed to acquire registry lock: {}", e))
    })?;

    // Register built-in protocol factories
    // Modbus TCP
    reg.register_factory("modbus_tcp".to_string(), create_modbus_tcp_plugin)?;

    // Modbus RTU
    reg.register_factory("modbus_rtu".to_string(), create_modbus_rtu_plugin)?;

    // Virtual protocol
    reg.register_factory("virt".to_string(), create_virt_plugin)?;

    tracing::info!(
        "Plugin system initialized with {} protocols",
        reg.list_protocol_factories().len()
    );

    Ok(())
}
