//! Redis clientmodular
//!
//! 提供基础的 Redis operationfunction，package括：
//! - asynchronousclient
//! - basic的 get/set/pub/sub operation
//! - edge端device专用的轻量级client

mod client;
mod edge_redis;

pub use client::RedisClient;
pub use edge_redis::EdgeRedis;

// Re-export commonly used types from redis crate
pub use redis::Msg;

// configuringstruct由各servingcustom
