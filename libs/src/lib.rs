//! `VoltageEMS` 基础library
//!
//! 提供allserving共享的基础function，package括：
//! - Redis client
//! - `InfluxDB` client
//! - monitoring和健康checking
//! - errorprocessing
//! - loggingfunction

// functionmodular
#[cfg(feature = "redis")]
pub mod redis;

#[cfg(feature = "influxdb")]
pub mod influxdb;

// 通用modular
pub mod config;
pub mod error;
pub mod logging;
pub mod types;
pub mod utils;

// 预import常用type
pub mod prelude {
    pub use crate::error::{Error, Result};

    #[cfg(feature = "redis")]
    pub use crate::redis::RedisClient;

    #[cfg(feature = "influxdb")]
    pub use crate::influxdb::InfluxClient;

    pub use crate::types::*;
}
