//! hissrv - Minimal Redis to InfluxDB data bridge service
//!
//! Lightweight historical data archival service designed for edge devices

/// Unified Result type using anyhow to simplify error handling
pub type Result<T> = anyhow::Result<T>;

/// Service information
pub const SERVICE_NAME: &str = "hissrv";
pub const SERVICE_VERSION: &str = "0.0.1";

/// Re-export commonly used types
pub use anyhow::{anyhow, bail, Context};
