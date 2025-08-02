//! `InfluxDB` 客户端模块
//!
//! 提供基础的 `InfluxDB` 操作功能，包括：
//! - HTTP 客户端
//! - 线协议构建
//! - 查询支持

mod builder;
mod client;

pub use builder::{FieldValue, LineProtocolBuilder};
pub use client::InfluxClient;

// 配置结构由各服务自定义
