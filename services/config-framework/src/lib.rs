pub mod base;
pub mod error;
pub mod loader;
pub mod migration;
pub mod sqlite_provider;
pub mod traits;
pub mod types;
pub mod validation;
pub mod watchers;

pub use base::{
    BaseServiceConfig, LoggingConfig, MonitoringConfig, RedisConfig, ServiceConfig, ServiceInfo,
};
pub use error::{ConfigError, Result};
pub use loader::{ConfigLoader, ConfigLoaderBuilder};
pub use migration::{ConfigMigrator, ValidationResults};
pub use sqlite_provider::{AsyncSqliteProvider, PointTableEntry, ProtocolMapping, SqliteProvider};
pub use traits::{ConfigSource, ConfigValidator, Configurable};
pub use types::{ConfigFormat, ConfigPath, Environment};
pub use validation::{validate_config, RangeRule, RegexRule, ValidationRule};
pub use watchers::{ConfigWatcher, WatchEvent};

/// Convenience function to load configuration from a file
pub fn load_config<T>(path: &std::path::Path) -> Result<T>
where
    T: serde::de::DeserializeOwned + Configurable + 'static,
{
    let loader = ConfigLoaderBuilder::new()
        .add_file(path.to_string_lossy().as_ref())
        .build()?;

    let config: T = loader.load()?;
    config.validate()?;
    Ok(config)
}

pub mod prelude {
    pub use crate::{
        validate_config, AsyncSqliteProvider, BaseServiceConfig, ConfigError, ConfigFormat,
        ConfigLoader, ConfigLoaderBuilder, ConfigMigrator, ConfigPath, ConfigSource,
        ConfigValidator, ConfigWatcher, Configurable, Environment, LoggingConfig, MonitoringConfig,
        PointTableEntry, ProtocolMapping, RangeRule, RedisConfig, RegexRule, Result, ServiceConfig,
        ServiceInfo, SqliteProvider, ValidationResults, ValidationRule, WatchEvent,
    };
}
