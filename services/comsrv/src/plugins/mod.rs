//! 协议插件系统
//!
//! 提供灵活的插件架构，支持协议实现的动态加载、配置管理和标准化接口

pub mod core;
pub mod grpc;
pub mod protocols;
pub mod traits;

// 从core重新导出核心类型
pub use core::{
    discovery, telemetry_type_to_redis, DefaultPluginStorage, PluginManager, PluginPointConfig,
    PluginPointUpdate, PluginRegistry, PluginStatistics, PluginStorage,
};

// 从traits重新导出接口定义
pub use traits::{
    create_plugin_instance, CliArgument, CliCommand, CliSubcommand, ConfigGenerator,
    ConfigParameter, ConfigSchema, ConfigSection, ConfigTemplate, ConfigValidator,
    DependencyCondition, EnumValue, ParameterDependency, ParameterType, ParameterValidation,
    PluginFactory, ProtocolMetadata, ProtocolPlugin, ValidationResult, ValidationRule,
};

// 重新导出宏
pub use crate::{protocol_plugin, register_plugin};

/// 初始化插件系统
pub fn init_plugin_system() -> crate::utils::Result<()> {
    tracing::info!("Initializing protocol plugin system");

    // 加载内置插件
    discovery::load_all_plugins()?;

    // 记录已加载的插件
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
