//! Common utilities for protocol plugins
//!
//! This module provides shared types and helper functions used across all protocol plugins.

use voltage_config::FourRemote;

/// Convert FourRemote type to Redis key suffix
///
/// Maps four-remote types to their single-character Redis key suffixes:
/// - Telemetry → "T"
/// - Signal → "S"
/// - Control → "C"
/// - Adjustment → "A"
///
/// # Example
/// ```
/// use voltage_config::comsrv::FourRemote;
/// use comsrv::plugins::common::telemetry_type_to_redis;
///
/// let suffix = telemetry_type_to_redis(&FourRemote::Telemetry);
/// assert_eq!(suffix, "T");
/// ```
pub fn telemetry_type_to_redis(telemetry_type: &FourRemote) -> &'static str {
    match telemetry_type {
        FourRemote::Telemetry => "T",
        FourRemote::Signal => "S",
        FourRemote::Control => "C",
        FourRemote::Adjustment => "A",
    }
}

/// Plugin point update for batch operations
///
/// Represents a single point update that will be written to Redis.
/// Used by storage manager for batch updates.
#[derive(Debug, Clone)]
pub struct PluginPointUpdate {
    /// Type of telemetry point (T/S/C/A)
    pub telemetry_type: FourRemote,
    /// Point identifier
    pub point_id: u32,
    /// Transformed value (after scale/offset/reverse)
    pub value: f64,
    /// Original raw value before transformation (optional)
    pub raw_value: Option<f64>,
}
