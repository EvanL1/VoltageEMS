//! Configuration type re-exports from voltage-config
//!
//! This module now re-exports types from the voltage-config library
//! to maintain a single source of truth for configuration structures

// Re-export all comsrv configuration types from voltage-config
pub use voltage_config::comsrv::{
    AdjustmentPoint, ChannelConfig, ChannelLoggingConfig, ComsrvConfig, ControlPoint, Point,
    SignalPoint, TelemetryPoint,
};

// Re-export common configuration types
pub use voltage_config::common::{ApiConfig, BaseServiceConfig, LoggingConfig, RedisConfig};

// Import serde for local types
use serde::{Deserialize, Serialize};

// Legacy aliases for backward compatibility
pub type AppConfig = ComsrvConfig;
pub type ServiceConfig = BaseServiceConfig;

// Note: ProtocolType enum has been removed in favor of using String
// This provides more flexibility during development and allows for
// arbitrary protocol names without modifying the enum.

// Unify Four-Remote type with workspace definition
pub use voltage_config::common::FourRemote;

/// Scaling information for data conversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingInfo {
    pub scale: f64,
    pub offset: f64,
}

impl Default for ScalingInfo {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset: 0.0,
        }
    }
}

// Helper functions for default values (if needed for compatibility)
pub fn default_true() -> bool {
    true
}

pub fn default_false() -> bool {
    false
}

// Adapter types for compatibility with existing code
// These help bridge the gap between voltage-config structures and existing implementation

// Re-export RuntimeChannelConfig from voltage-config
pub use voltage_config::comsrv::RuntimeChannelConfig;

// Re-export protocol mapping structures from voltage-config
pub use voltage_config::comsrv::{CanMapping, ModbusMapping, VirtualMapping};

// For backward compatibility, provide a type alias
// This allows existing code to continue working while we migrate
pub type ChannelConfigCompat = RuntimeChannelConfig;
