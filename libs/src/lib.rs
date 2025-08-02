//! `VoltageEMS` 基础库
//!
//! 提供所有服务共享的基础功能，包括：
//! - Redis 客户端
//! - `InfluxDB` 客户端
//! - 监控和健康检查
//! - 错误处理
//! - 日志功能

// 功能模块
#[cfg(feature = "redis")]
pub mod redis;

#[cfg(feature = "influxdb")]
pub mod influxdb;

// 通用模块
pub mod config;
pub mod error;
pub mod logging;
pub mod types;
pub mod utils;

// 预导入常用类型
pub mod prelude {
    pub use crate::error::{Error, Result};

    #[cfg(feature = "redis")]
    pub use crate::redis::RedisClient;

    #[cfg(feature = "influxdb")]
    pub use crate::influxdb::InfluxClient;

    pub use crate::types::*;
}
