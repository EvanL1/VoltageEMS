pub mod error;
pub mod loader;
pub mod traits;
pub mod types;
pub mod validation;
pub mod watchers;
pub mod base;
pub mod migration;

pub use error::{ConfigError, Result};
pub use loader::{ConfigLoader, ConfigLoaderBuilder};
pub use traits::{Configurable, ConfigSource, ConfigValidator};
pub use types::{ConfigFormat, ConfigPath, Environment};
pub use validation::{validate_config, ValidationRule, RegexRule, RangeRule};
pub use watchers::{ConfigWatcher, WatchEvent};
pub use base::{BaseServiceConfig, ServiceInfo, RedisConfig, LoggingConfig, MonitoringConfig, ServiceConfig};
pub use migration::{ConfigMigrator, ValidationResults};

pub mod prelude {
    pub use crate::{
        ConfigError, ConfigLoader, ConfigLoaderBuilder, Configurable, ConfigPath, ConfigSource,
        ConfigValidator, ConfigWatcher, Environment, Result, ValidationRule, WatchEvent,
        validate_config, ConfigFormat, RegexRule, RangeRule,
        BaseServiceConfig, ServiceInfo, RedisConfig, LoggingConfig, MonitoringConfig, ServiceConfig,
        ConfigMigrator, ValidationResults,
    };
}