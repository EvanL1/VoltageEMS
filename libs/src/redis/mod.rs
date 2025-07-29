//! Redis 客户端模块
//!
//! 提供基础的 Redis 操作功能，包括：
//! - 异步客户端
//! - 基本的 get/set/pub/sub 操作
//! - 边端设备专用的轻量级客户端

mod client;
mod edge_redis;

pub use client::RedisClient;
pub use edge_redis::EdgeRedis;

// 重导出配置
pub use crate::config::RedisConfig;
