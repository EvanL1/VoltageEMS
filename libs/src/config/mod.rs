//! 配置工具模块
//!
//! 提供统一的环境变量处理和配置加载功能

pub mod loader;
pub mod utils;

pub use loader::{ConfigError, ConfigLoader};
pub use utils::{get_env_with_fallback, get_global_log_level, get_global_redis_url};
