//! protocolpluginsystem
//!
//! 提供灵活的plugin架构，supportingprotocolimplement的dynamicloading、configuringmanaging和standard化interface

pub mod core;
pub mod grpc;
pub mod protocols;
pub mod traits;

// slavecore重新export核心type
pub use core::{telemetry_type_to_redis, PluginManager, PluginPointUpdate, PluginRegistry};

// slavetraits重新exportinterfacedefinition
pub use traits::{
    create_plugin_instance, CliArgument, CliCommand, CliSubcommand, ConfigGenerator,
    ConfigParameter, ConfigSchema, ConfigSection, ConfigTemplate, ConfigValidator,
    DependencyCondition, EnumValue, ParameterDependency, ParameterType, ParameterValidation,
    PluginFactory, ProtocolMetadata, ProtocolPlugin, ValidationResult, ValidationRule,
};

// 重新exportmacro
pub use crate::{protocol_plugin, register_plugin};

/// Initializepluginsystem
pub fn init_plugin_system() -> crate::utils::Result<()> {
    tracing::info!("Initializing protocol plugin system");

    // loading内置plugin
    // Discovery removed in simplification

    // record已loading的plugin
    // Statistics removed in simplification

    tracing::info!("Plugin system initialized");

    Ok(())
}
