//! `InfluxDB` clientmodular
//!
//! 提供基础的 `InfluxDB` operationfunction，package括：
//! - HTTP client
//! - 线protocolbuilding
//! - querysupporting

mod builder;
mod client;

pub use builder::{FieldValue, LineProtocolBuilder};
pub use client::InfluxClient;

// configuringstruct由各servingcustom
