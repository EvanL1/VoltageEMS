//! hissrv - 极简的 Redis 到 InfluxDB 数据桥接服务
//!
//! 专为边端设备设计的轻量级历史数据归档服务

/// 统一的 Result 类型，使用 anyhow 简化错误处理
pub type Result<T> = anyhow::Result<T>;

/// 服务信息
pub const SERVICE_NAME: &str = "hissrv";
pub const SERVICE_VERSION: &str = "0.0.1";

/// 重新导出常用类型
pub use anyhow::{anyhow, bail, Context};
