//! # Modern Configuration Management Module
//!
//! This module provides streamlined configuration management with both legacy compatibility
//! and modern distributed configuration support.
//!
//! ## Features
//!
//! - **Multi-source configuration**: File → Environment → CLI arguments
//! - **Distributed configuration**: Integration with config-framework service
//! - **Local caching**: High-performance local configuration cache
//! - **Hot reload**: Runtime configuration updates and notifications
//! - **Type-safe**: Compile-time validation
//! - **Format support**: YAML, TOML, JSON auto-detection
//! - **Graceful fallback**: Automatic fallback to local config when service unavailable
//! - **Migration support**: Seamless migration from legacy configurations
//!
//! ## Architecture
//!
//! ```
//! ┌─────────────────────────────────────────────────────────────┐
//! │                Modern Config Manager                        │
//! ├─────────────────┬─────────────────┬─────────────────────────┤
//! │ Config Client   │ Local Cache     │ Legacy Adapter          │
//! │ (HTTP/WS)       │ (Memory/Disk)   │ (Backward Compatibility)│
//! └─────────────────┴─────────────────┴─────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use comsrv::core::config::{ModernConfigManager, ConfigManagerConfig};
//!
//! // Create modern config manager with service integration
//! let config = ConfigManagerConfig {
//!     config_service_url: Some("http://config-service:8080".to_string()),
//!     enable_config_service: true,
//!     enable_local_cache: true,
//!     ..Default::default()
//! };
//! let manager = ModernConfigManager::new(config).await?;
//!
//! // Get configuration with automatic fallback
//! let app_config: AppConfig = manager.get_config_with_fallback("app_config").await?;
//! ```

// Core modules
pub mod config_manager;
pub mod types;
pub mod point;
pub mod loaders;

// Modern distributed configuration modules
pub mod client;
pub mod cache;
pub mod manager;
pub mod migration;

// Re-export legacy ConfigManager for backward compatibility
pub use config_manager::ConfigManager;

// Re-export modern ConfigManager
pub use manager::{ModernConfigManager, ConfigManagerConfig, ConfigManagerMode, ConfigHealthStatus};

// Re-export client types
pub use client::{
    UnifiedConfigClient, ConfigClient, ConfigChangeEvent, ConfigAction,
    ConfigClientError, ConfigClientResult,
};

// Re-export cache types
pub use cache::{
    UnifiedConfigCache, CacheConfig, CacheEntry, ConfigCache,
    MemoryCache, PersistenceCache, VersionCache,
};

// Re-export migration types
pub use migration::{
    ConfigMigrationManager, MigrationConfig, MigrationResult,
    LegacyConfigAdapter, MigrationStrategy, MigrationPlan,
};

// Re-export types
pub use types::{
    ChannelConfig, ChannelParameters, ProtocolType,
    ServiceConfig, ApiConfig, RedisConfig,
    FourTelemetryFiles, ChannelLoggingConfig,
    TelemetryType, ProtocolAddress, DataType,
    ScalingConfig, ValidationConfig, UnifiedPointMapping,
    AppConfig,
};

// Re-export point
pub use point::Point;

// Re-export all loaders
pub use loaders::*;

/// 配置管理器 trait（向后兼容）
#[async_trait::async_trait]
pub trait ConfigManagerTrait: Send + Sync {
    /// 获取应用配置
    async fn app_config(&self) -> Result<AppConfig, Box<dyn std::error::Error + Send + Sync>>;
    
    /// 重新加载配置
    async fn reload(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

// 向后兼容的配置管理器工厂
pub struct ConfigManagerFactory;

impl ConfigManagerFactory {
    /// 创建配置管理器（根据环境自动选择类型）
    pub async fn create(config_path: Option<&str>) -> Result<Box<dyn ConfigManagerTrait>, Box<dyn std::error::Error + Send + Sync>> {
        // 检查是否启用了现代配置管理
        let enable_modern = std::env::var("VOLTAGE_CONFIG_MODERN")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        if enable_modern {
            // 使用现代配置管理器
            let config_service_url = std::env::var("VOLTAGE_CONFIG_SERVICE_URL").ok();
            
            let config = ConfigManagerConfig {
                config_service_url: config_service_url.clone(),
                enable_config_service: config_service_url.is_some(),
                local_config_path: config_path.map(|p| p.to_string()),
                ..Default::default()
            };

            let manager = ModernConfigManager::new(config).await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            
            Ok(Box::new(manager))
        } else {
            // 使用遗留配置管理器
            let path = config_path.unwrap_or("config/comsrv.yaml");
            let legacy_manager = ConfigManager::from_file(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            
            Ok(Box::new(LegacyConfigManagerWrapper { manager: legacy_manager }))
        }
    }
}

/// 遗留配置管理器包装器（向后兼容）
struct LegacyConfigManagerWrapper {
    manager: ConfigManager,
}

#[async_trait::async_trait]
impl ConfigManagerTrait for LegacyConfigManagerWrapper {
    async fn app_config(&self) -> Result<AppConfig, Box<dyn std::error::Error + Send + Sync>> {
        // 转换遗留的 config_manager::AppConfig 到 types::AppConfig
        let legacy_config = self.manager.config();
        
        // 简单的转换实现（实际项目中可能需要更复杂的映射）
        let app_config = AppConfig {
            version: "2.0".to_string(),
            service: legacy_config.service.clone(),
            channels: legacy_config.channels.clone(),
            defaults: legacy_config.defaults.clone(),
        };
        
        Ok(app_config)
    }

    async fn reload(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 遗留配置管理器重新加载逻辑
        // TODO: 实现遗留配置管理器的重新加载
        Ok(())
    }
}
